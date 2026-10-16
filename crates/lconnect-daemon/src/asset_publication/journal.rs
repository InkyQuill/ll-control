//! Durable scoped ownership. Only private links prove which inode may be removed.
use super::{check_root, lock, Created, CreatedKind, FileIdentity, Publication};
use crate::{
    persistence::PersistenceStore,
    profile_group::{GroupKind, GroupRevision},
};
use anyhow::{ensure, Context, Result};
use rustix::fs::{flock, FlockOperation};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const MAX_MANIFEST: u64 = 1024 * 1024;
const MAX_RECORDS: usize = 2048;
const MAX_SCOPES: usize = 64;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub device_key: String,
    pub operation: String,
    pub group: GroupKind,
    pub expected: GroupRevision,
    pub source_sha256: String,
}
impl Binding {
    fn validate(&self) -> Result<()> {
        crate::state_transaction::validate_key(&self.device_key)?;
        crate::profile_group::validate_operation_id(&self.operation)?;
        for digest in [&self.source_sha256, &self.expected.settings_sha256] {
            ensure!(
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                "invalid publication digest"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SlotBinding {
    pub operation: String,
    pub group: lconnect_ipc::ProfileGroup,
    pub slot: u8,
    pub expected_revision: u64,
    pub size: u64,
    pub source_sha256: String,
}
impl SlotBinding {
    fn validate(&self) -> Result<()> {
        crate::profile_group::validate_operation_id(&self.operation)?;
        self.group.validate_slot(self.slot)?;
        ensure!(
            (44..=lconnect_ipc::MAX_PROFILE_ARCHIVE_BYTES).contains(&self.size),
            "invalid slot archive size"
        );
        ensure!(
            self.expected_revision.checked_add(1).is_some(),
            "slot profile revision exhausted"
        );
        ensure!(
            self.source_sha256.len() == 64
                && self
                    .source_sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid slot archive digest"
        );
        Ok(())
    }
}
// Untagged full-group encoding preserves existing v1 manifests exactly. Version2
// is reserved for the distinct global named-slot target, without a device token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Target {
    FullGroup(Binding),
    NamedSlot(SlotBinding),
}
impl Target {
    fn version(&self) -> u32 {
        match self {
            Self::FullGroup(_) => 1,
            Self::NamedSlot(_) => 2,
        }
    }
    fn validate(&self) -> Result<()> {
        match self {
            Self::FullGroup(b) => b.validate(),
            Self::NamedSlot(b) => b.validate(),
        }
    }
    pub fn operation(&self) -> &str {
        match self {
            Self::FullGroup(b) => &b.operation,
            Self::NamedSlot(b) => &b.operation,
        }
    }
    fn confirmation(&self, store: &PersistenceStore) -> Result<Confirmation> {
        match self {
            Self::FullGroup(b) => {
                let settings = store.read(&b.device_key)?;
                if let Some(r) = settings.group_imports.iter().find(|r| r.id == b.operation) {
                    ensure!(
                        r.kind == b.group
                            && r.expected == b.expected
                            && r.source_sha256 == b.source_sha256,
                        "publication receipt binding mismatch"
                    );
                    Ok(Confirmation::Accepted)
                } else {
                    Ok(Confirmation::Missing)
                }
            }
            Self::NamedSlot(b) => {
                if let Some((r, _)) = store.profiles().recover_import(&b.operation)? {
                    ensure!(
                        r.group == b.group
                            && r.slot == b.slot
                            && r.expected_revision == b.expected_revision
                            && r.size == b.size,
                        "slot publication receipt binding mismatch"
                    );
                    match r.source_sha256 {
                        Some(hash) => {
                            ensure!(
                                hash == b.source_sha256,
                                "slot publication receipt digest mismatch"
                            );
                            Ok(Confirmation::Accepted)
                        }
                        None => Ok(Confirmation::Unverifiable),
                    }
                } else {
                    Ok(Confirmation::Missing)
                }
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirmation {
    Accepted,
    Missing,
    Unverifiable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotRecoveryNotice {
    pub operation: String,
    pub group: lconnect_ipc::ProfileGroup,
    pub slot: u8,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", content = "notice", rename_all = "snake_case")]
pub enum StartupNotice {
    FullGroup(RecoveryNotice),
    NamedSlot(SlotRecoveryNotice),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Preparing,
    Armed,
    Keep,
    Discard,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: CreatedKind,
    key: String,
    device: u64,
    inode: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    nonce: String,
    binding: Target,
    phase: Phase,
    entries: Vec<Entry>,
}
impl Manifest {
    fn validate(&self, nonce: &str) -> Result<()> {
        ensure!(
            self.version == self.binding.version() && self.nonce == nonce,
            "unknown/mismatched publication journal"
        );
        crate::profile_group::validate_operation_id(nonce)?;
        self.binding.validate()?;
        ensure!(
            self.entries.len() <= MAX_RECORDS,
            "publication journal record budget exceeded"
        );
        let mut keys = std::collections::BTreeSet::new();
        for e in &self.entries {
            crate::profile_group::validate_operation_id(&e.key)?;
            ensure!(
                e.key != crate::builtin_media::KEY,
                "reserved publication key"
            );
            ensure!(
                keys.insert((e.kind, e.key.clone())),
                "duplicate publication record"
            );
        }
        Ok(())
    }
}
/// A successful startup may retain an armed import whose receipt is unavailable
/// or evicted. This durable notice does not claim that Apply succeeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryNotice {
    pub operation: String,
    pub device_key: String,
    pub group: GroupKind,
    pub reason: String,
}
struct DirectoryLock(File);
impl DirectoryLock {
    fn open(path: &Path, try_only: bool) -> Result<Self> {
        let file: File = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?
        .into();
        flock(
            &file,
            if try_only {
                FlockOperation::NonBlockingLockExclusive
            } else {
                FlockOperation::LockExclusive
            },
        )?;
        Ok(Self(file))
    }
}
impl Drop for DirectoryLock {
    fn drop(&mut self) {
        let _ = flock(&self.0, FlockOperation::Unlock);
    }
}
pub(crate) struct Journal {
    root: PathBuf,
    directory: PathBuf,
    _lifetime: DirectoryLock,
    manifest: Mutex<Manifest>,
}
impl std::fmt::Debug for Journal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicationJournal")
            .field("directory", &self.directory)
            .finish_non_exhaustive()
    }
}
impl Journal {
    pub fn create(root: PathBuf, binding: Target) -> Result<Arc<Self>> {
        binding.validate()?;
        create_directory(&root)?;
        let _base = DirectoryLock::open(&root, false)?;
        let mut scopes = 0;
        for (i, e) in fs::read_dir(&root)?.enumerate() {
            ensure!(
                i < MAX_SCOPES * 3 + 16,
                "publication root entry budget exceeded"
            );
            let e = e?;
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("invalid publication name"))?;
            if matches!(
                name.as_str(),
                "recovery-report.json" | "slot-recovery-report.json"
            ) || atomic_temporary(&name)
            {
                ensure!(e.file_type()?.is_file(), "invalid publication metadata");
                continue;
            }
            crate::profile_group::validate_operation_id(
                name.strip_prefix("done-").unwrap_or(&name),
            )?;
            ensure!(e.file_type()?.is_dir(), "invalid publication directory");
            scopes += 1;
        }
        ensure!(
            scopes < MAX_SCOPES,
            "too many unresolved publication scopes"
        );
        let nonce = format!("{:032x}", rand::random::<u128>());
        let directory = root.join(&nonce);
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let lifetime = DirectoryLock::open(&directory, false)?;
        let manifest = Manifest {
            version: binding.version(),
            nonce,
            binding,
            phase: Phase::Preparing,
            entries: Vec::new(),
        };
        write_json(&directory.join("manifest.json"), &manifest)?;
        File::open(&root)?.sync_all()?;
        Ok(Arc::new(Self {
            root,
            directory,
            _lifetime: lifetime,
            manifest: Mutex::new(manifest),
        }))
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn record(&self, kind: CreatedKind, key: &str, file: &File) -> Result<()> {
        crate::profile_group::validate_operation_id(key)?;
        let mut current = lock(&self.manifest);
        ensure!(
            current.phase == Phase::Preparing,
            "publication already armed/resolved"
        );
        ensure!(
            current.entries.len() < MAX_RECORDS,
            "publication journal record budget exceeded"
        );
        ensure!(
            !current
                .entries
                .iter()
                .any(|e| e.kind == kind && e.key == key),
            "duplicate publication record"
        );
        ensure!(key != crate::builtin_media::KEY, "reserved publication key");
        let link = self
            .directory
            .join(format!("{}.owned", current.entries.len()));
        // Follow our own descriptor, never a caller-supplied source path. The
        // named candidate is still linked and on the same filesystem.
        rustix::fs::linkat(
            rustix::fs::CWD,
            format!("/proc/self/fd/{}", file.as_raw_fd()),
            rustix::fs::CWD,
            &link,
            rustix::fs::AtFlags::SYMLINK_FOLLOW,
        )?;
        let m = fs::symlink_metadata(&link)?;
        let original = file.metadata()?;
        ensure!(
            m.is_file()
                && !m.file_type().is_symlink()
                && m.dev() == original.dev()
                && m.ino() == original.ino(),
            "ownership link mismatch"
        );
        file.sync_all()?;
        File::open(&self.directory)?.sync_all()?;
        let mut next = current.clone();
        next.entries.push(Entry {
            kind,
            key: key.into(),
            device: original.dev(),
            inode: original.ino(),
        });
        next.validate(&next.nonce)?;
        write_json(&self.directory.join("manifest.json"), &next)?;
        *current = next;
        Ok(())
    }
    fn mark(&self, phase: Phase) -> Result<()> {
        let mut current = lock(&self.manifest);
        if phase == Phase::Armed {
            ensure!(
                current.phase == Phase::Preparing,
                "publication already armed/resolved"
            );
        }
        if phase == Phase::Discard {
            ensure!(
                current.phase != Phase::Keep,
                "retained publication cannot be discarded"
            );
        }
        let mut next = current.clone();
        next.phase = phase;
        write_json(&self.directory.join("manifest.json"), &next)?;
        *current = next;
        Ok(())
    }
    pub fn arm(&self) -> Result<()> {
        self.mark(Phase::Armed)
    }
    pub fn note_unknown(&self) -> Result<()> {
        let binding = lock(&self.manifest).binding.clone();
        let _base = DirectoryLock::open(&self.root, false)?;
        append_target_notice(&self.root, &binding)?;
        Ok(())
    }
    pub fn keep(&self) -> Result<()> {
        self.mark(Phase::Keep)
    }
    pub fn discard(&self) -> Result<()> {
        self.mark(Phase::Discard)
    }
    pub fn finish(&self) -> Result<()> {
        ensure!(
            matches!(lock(&self.manifest).phase, Phase::Keep | Phase::Discard),
            "unresolved publication cannot be removed"
        );
        let _base = DirectoryLock::open(&self.root, false)?;
        retire(&self.root, &self.directory)
    }
}
fn create_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    check_root(path)?;
    for ancestor in std::path::absolute(path)?.ancestors() {
        File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(
        bytes.len() as u64 <= MAX_MANIFEST,
        "publication journal too large"
    );
    let directory = path.parent().context("missing publication directory")?;
    let mut f = tempfile::NamedTempFile::new_in(directory)?;
    f.write_all(&bytes)?;
    f.as_file().sync_all()?;
    f.persist(path).map_err(|e| e.error)?;
    File::open(directory)?.sync_all()?;
    Ok(())
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink() && m.len() <= MAX_MANIFEST,
        "invalid publication journal file"
    );
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_MANIFEST,
        "publication journal too large"
    );
    Ok(serde_json::from_slice(&bytes)?)
}
fn check_contents(path: &Path) -> Result<()> {
    for (i, entry) in fs::read_dir(path)?.enumerate() {
        ensure!(
            i < MAX_RECORDS * 2 + 64,
            "publication directory budget exceeded"
        );
        let e = entry?;
        ensure!(e.file_type()?.is_file(), "nonregular publication entry");
    }
    Ok(())
}
fn retire(root: &Path, path: &Path) -> Result<()> {
    check_contents(path)?;
    let name = path
        .file_name()
        .context("missing publication nonce")?
        .to_str()
        .context("invalid publication nonce")?;
    let done = if name.starts_with("done-") {
        path.to_owned()
    } else {
        let done = root.join(format!("done-{name}"));
        ensure!(!done.try_exists()?, "publication retirement collision");
        fs::rename(path, &done)?;
        File::open(root)?.sync_all()?;
        done
    };
    fs::remove_dir_all(done)?;
    File::open(root)?.sync_all()?;
    Ok(())
}
fn atomic_temporary(name: &str) -> bool {
    name.strip_prefix(".tmp")
        .is_some_and(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_alphanumeric()))
}
pub(crate) fn read_notices(root: &Path) -> Result<Vec<RecoveryNotice>> {
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => check_root(root)?,
    }

    let path = root.join("recovery-report.json");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let notices: Vec<RecoveryNotice> = read_json(&path)?;
    ensure!(notices.len() <= 64, "recovery report budget exceeded");
    for notice in &notices {
        crate::profile_group::validate_operation_id(&notice.operation)?;
        crate::state_transaction::validate_key(&notice.device_key)?;
        ensure!(notice.reason.len() <= 4096, "recovery reason too long");
    }
    Ok(notices)
}
fn append_notice(root: &Path, binding: &Binding) -> Result<RecoveryNotice> {
    let notice=RecoveryNotice {operation:binding.operation.clone(),device_key:binding.device_key.clone(),group:binding.group,reason:"Commit outcome is unconfirmed or its receipt was evicted; imported files retained, Apply not replayed by recovery".into()};
    let mut report = read_notices(root)?;
    report.retain(|n| {
        n.operation != notice.operation
            || n.device_key != notice.device_key
            || n.group != notice.group
    });
    report.push(notice.clone());
    if report.len() > 64 {
        report.remove(0);
    }
    write_json(&root.join("recovery-report.json"), &report)?;
    Ok(notice)
}

pub(crate) fn read_slot_notices(root: &Path) -> Result<Vec<SlotRecoveryNotice>> {
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => check_root(root)?,
    }
    let path = root.join("slot-recovery-report.json");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let notices: Vec<SlotRecoveryNotice> = read_json(&path)?;
    ensure!(notices.len() <= 64, "slot recovery report budget exceeded");
    for n in &notices {
        crate::profile_group::validate_operation_id(&n.operation)?;
        n.group.validate_slot(n.slot)?;
        ensure!(n.reason.len() <= 4096, "slot recovery reason too long");
    }
    Ok(notices)
}
fn append_target_notice(root: &Path, binding: &Target) -> Result<StartupNotice> {
    match binding {
        Target::FullGroup(b) => Ok(StartupNotice::FullGroup(append_notice(root, b)?)),
        Target::NamedSlot(b) => {
            let notice=SlotRecoveryNotice {operation:b.operation.clone(),group:b.group,slot:b.slot,reason:"Slot import commit is unconfirmed or its receipt cannot identify the source archive; files retained without changing slots or applying hardware".into()};
            let mut report = read_slot_notices(root)?;
            report.retain(|n| n.operation != b.operation);
            report.push(notice.clone());
            if report.len() > 64 {
                report.remove(0);
            }
            write_json(&root.join("slot-recovery-report.json"), &report)?;
            Ok(StartupNotice::NamedSlot(notice))
        }
    }
}
/// Caller must run before exposing catalogs or starting device replay. Preparing
/// scopes never admitted owner commit; armed scopes with missing receipts are
/// retained, because receipt eviction cannot establish a negative outcome.
pub(crate) fn recover(store: &Arc<PersistenceStore>) -> Result<Vec<StartupNotice>> {
    let root = store.publication_root();
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => check_root(&root)?,
    }
    let _base = DirectoryLock::open(&root, false)?;
    let mut entries = Vec::new();
    let mut temporaries = Vec::new();
    for (i, e) in fs::read_dir(&root)?.enumerate() {
        ensure!(i < MAX_SCOPES * 3 + 16, "publication scope budget exceeded");
        let e = e?;
        let name = e
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("invalid publication name"))?;
        if matches!(
            name.as_str(),
            "recovery-report.json" | "slot-recovery-report.json"
        ) {
            ensure!(e.file_type()?.is_file(), "invalid recovery report");
            continue;
        }
        if atomic_temporary(&name) {
            ensure!(e.file_type()?.is_file(), "invalid publication temporary");
            temporaries.push(e.path());
            continue;
        }
        let nonce = name.strip_prefix("done-").unwrap_or(&name);
        crate::profile_group::validate_operation_id(nonce)?;
        ensure!(e.file_type()?.is_dir(), "invalid publication directory");
        entries.push((name, e.path()));
    }
    // Diagnostic histories are metadata, but malformed/symlink histories must
    // still fail closed before reconciliation starts.
    read_notices(&root)?;
    read_slot_notices(&root)?;
    for temp in temporaries {
        fs::remove_file(temp)?;
        File::open(&root)?.sync_all()?;
    }
    let mut notices = Vec::new();
    for (name, directory) in entries {
        let lifetime = match DirectoryLock::open(&directory, true) {
            Ok(l) => l,
            Err(e)
                if e.downcast_ref::<rustix::io::Errno>()
                    == Some(&rustix::io::Errno::WOULDBLOCK) =>
            {
                anyhow::bail!("publication scope still active; startup recovery deferred")
            }
            Err(e) => return Err(e),
        };
        check_contents(&directory)?;
        if name.starts_with("done-") {
            retire(&root, &directory)?;
            continue;
        }
        let manifest_path = directory.join("manifest.json");
        if !manifest_path.try_exists()? {
            // create() has not returned, so no writer can have published a final
            // asset. Only an initial atomic manifest temporary is permissible.
            ensure!(
                fs::read_dir(&directory)?
                    .all(|e| e.is_ok_and(|e| atomic_temporary(&e.file_name().to_string_lossy()))),
                "publication manifest missing with owned payloads"
            );
            retire(&root, &directory)?;
            continue;
        }
        let manifest: Manifest = read_json(&manifest_path)?;
        manifest.validate(&name)?;
        let journal = Journal {
            root: root.clone(),
            directory: directory.clone(),
            _lifetime: lifetime,
            manifest: Mutex::new(manifest.clone()),
        };
        let confirmation = manifest.binding.confirmation(store)?;
        let accepted = confirmation == Confirmation::Accepted;
        if accepted
            || confirmation == Confirmation::Unverifiable
            || matches!(manifest.phase, Phase::Keep | Phase::Armed)
        {
            if !accepted {
                let notice = append_target_notice(&root, &manifest.binding)?;
                notices.push(notice);
            }
            journal.keep()?;
        } else {
            // Existing rollback retains all durable/live references and rejects
            // foreign path identities. Durable links keep these inodes alive.
            let token = rand::random();
            let mut records = Vec::new();
            for (i, e) in manifest.entries.iter().enumerate() {
                let path = directory.join(format!("{i}.owned"));
                let m = fs::symlink_metadata(&path)?;
                ensure!(
                    m.is_file()
                        && !m.file_type().is_symlink()
                        && m.dev() == e.device
                        && m.ino() == e.inode,
                    "invalid ownership link"
                );
                let file = File::open(path)?;
                records.push(Created {
                    kind: e.kind,
                    key: e.key.clone(),
                    identity: Some(FileIdentity::capture(&file)?),
                });
                match e.kind {
                    CreatedKind::Image | CreatedKind::ImageName => {
                        store.theme_assets().pending.hide(&e.key, token)
                    }
                    CreatedKind::Font => store.theme_fonts().pending.hide(&e.key, token),
                    CreatedKind::Media => store.media_assets().pending.hide(&e.key, token),
                }
            }
            let cleanup_owner = Some(store.admission().enter()?);
            journal.discard()?;
            let publication = Publication {
                cleanup_owner,
                store: store.clone(),
                token,
                operation: manifest.binding.operation().to_owned(),
                records: Mutex::new(records),
                resolved: std::sync::atomic::AtomicBool::new(false),
                journal: None,
            };
            publication.rollback()?;
        }
        // Recovery already owns the base lock; live finish() acquires it.
        retire(&root, &directory)?;
    }
    Ok(notices)
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
