//! File-backed profile archive transfer sessions. Independent of device/HID
//! actors. Public calls are async; blocking work and permits survive cancellation.
use crate::{profile_archive::ProfileArchives, profiles::ProfileStore};
use anyhow::{ensure, Context};
use llcontrol_ipc::{
    ProfileArchiveDirection as Direction, ProfileArchivePhase as Phase,
    ProfileArchiveReply as Reply, ProfileArchiveRequest as Request, ProfileArchiveState as State,
    MAX_PROFILE_ARCHIVE_BYTES as MAX_BYTES, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const MAX_SESSIONS: usize = 4;
const MAX_RESERVED: u64 = 2 * MAX_BYTES;
const IDLE: Duration = Duration::from_secs(300);
struct Session {
    begin: Request,
    state: State,
    file: Option<File>,
    reserved: u64,
    touched: Instant,
    cleanup_owner: Option<crate::admission::Lease>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let file = self.file.take();
        if let Some(owner) = self.cleanup_owner.take() {
            owner.cleanup(move || {
                drop(file);
                Ok(())
            });
        }
    }
}
#[derive(Default)]
struct Registry {
    sessions: BTreeMap<String, Session>,
}
impl Registry {
    fn expire(&mut self, now: Instant) {
        self.sessions
            .retain(|_, s| now.saturating_duration_since(s.touched) < IDLE);
    }
    fn reserved(&self) -> u64 {
        self.sessions.values().map(|s| s.reserved).sum()
    }
}
#[derive(Clone)]
pub struct ArchiveTransfers {
    archives: ProfileArchives,
    profiles: ProfileStore,
    persistence: Arc<crate::persistence::PersistenceStore>,
    registry: Arc<Mutex<Registry>>,
    jobs: crate::admission::WorkSlots,
    reaper_started: Arc<AtomicBool>,
}
impl ArchiveTransfers {
    pub(crate) fn close_staging(
        &self,
        maintenance: &crate::admission::Maintenance,
    ) -> anyhow::Result<()> {
        let lease = maintenance.child(&self.persistence.admission())?;
        let registry = self.registry.clone();
        lease.cleanup(move || {
            let sessions = std::mem::take(
                &mut registry
                    .lock()
                    .map_err(|_| anyhow::anyhow!("archive registry poisoned"))?
                    .sessions,
            );
            drop(sessions);
            Ok(())
        });
        Ok(())
    }
    pub(crate) fn with_admission(mut self, admission: crate::admission::Admission) -> Self {
        self.jobs = self.jobs.with_admission(admission.clone());
        self
    }
    pub fn new(
        archives: ProfileArchives,
        persistence: Arc<crate::persistence::PersistenceStore>,
    ) -> Self {
        let profiles = persistence.profiles();
        let admission = persistence.admission();
        Self {
            archives,
            profiles,
            persistence,
            registry: Arc::default(),
            jobs: crate::admission::WorkSlots::new(2, admission),
            reaper_started: Arc::new(AtomicBool::new(false)),
        }
    }
    /// Admission is fail-fast; the permit moves into the blocking closure so a
    /// dropped RPC future never hides an ongoing transaction or disk work.
    pub async fn handle(&self, request: Request) -> llcontrol_ipc::Result<Reply> {
        self.start_reaper();
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| llcontrol_ipc::IpcError::DeviceBusy("archive workers busy".into()))?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            this.handle_at(request, Instant::now())
        })
        .await
        .map_err(|e| llcontrol_ipc::IpcError::DeviceBusy(e.to_string()))?
    }
    fn start_reaper(&self) {
        if self.reaper_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let registry = Arc::downgrade(&self.registry);
        let admission = self.jobs.admission();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                let Some(registry) = registry.upgrade() else {
                    break;
                };
                // A busy Finish/BeginExport owns its resources through completion;
                // do not expire a job while its blocking worker is using its file.
                let Ok(lease) = admission.enter() else {
                    break;
                };
                let _ = tokio::task::spawn_blocking(move || {
                    let _lease = lease;
                    if let Ok(mut map) = registry.try_lock() {
                        map.expire(Instant::now());
                    }
                })
                .await;
            }
        });
    }
    fn handle_at(&self, request: Request, now: Instant) -> llcontrol_ipc::Result<Reply> {
        let mut map = self.registry.try_lock().map_err(|_| {
            llcontrol_ipc::IpcError::DeviceBusy(
                "archive transaction busy; retry status/readback".into(),
            )
        })?;
        map.expire(now);
        self.apply(&mut map, request, now)
            .map_err(|e| llcontrol_ipc::IpcError::InvalidArgument(format!("profile archive: {e}")))
    }
    fn apply(&self, map: &mut Registry, request: Request, now: Instant) -> anyhow::Result<Reply> {
        let id = match &request {
            Request::BeginExport { id, .. }
            | Request::BeginImport { id, .. }
            | Request::Chunk { id, .. }
            | Request::Read { id, .. }
            | Request::Inspect { id }
            | Request::Finish { id }
            | Request::Status { id }
            | Request::Cancel { id }
            | Request::Release { id } => id.clone(),
        };
        ensure!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "invalid archive transfer id"
        );
        if !matches!(
            &request,
            Request::BeginExport { .. }
                | Request::Chunk { .. }
                | Request::Read { .. }
                | Request::Inspect { .. }
                | Request::Release { .. }
        ) {
            let recover = map
                .sessions
                .get(&id)
                .is_none_or(|s| s.state.phase == Phase::Failed);
            if recover {
                if self.profiles.recover_import(&id)?.is_none()
                    && self
                        .persistence
                        .slot_import_recovery_notices()?
                        .iter()
                        .any(|n| n.operation == id)
                {
                    anyhow::bail!("Slot import commit is unconfirmed; its files were retained. Read the profile library and use a new operation ID for a new import. No slot or hardware Apply was replayed.");
                }
                if let Some((receipt, library)) = self.profiles.recover_import(&id)? {
                    let begin = Request::BeginImport {
                        id: id.clone(),
                        size: receipt.size,
                        expected_revision: receipt.expected_revision,
                        group: receipt.group,
                        slot: receipt.slot,
                    };
                    if matches!(&request, Request::BeginImport { .. }) {
                        ensure!(
                            request == begin,
                            "archive operation id already has different parameters"
                        );
                    }
                    if let Some(old) = map.sessions.get(&id) {
                        ensure!(
                            old.begin == begin,
                            "archive operation id differs from saved transfer"
                        );
                    }
                    let profile = if library.revision == receipt.committed_revision {
                        library
                            .profiles
                            .iter()
                            .find(|p| p.content.group() == receipt.group && p.slot == receipt.slot)
                            .cloned()
                    } else {
                        None
                    };
                    let state = State {
                        id: id.clone(),
                        direction: Direction::Import,
                        group: receipt.group,
                        slot: receipt.slot,
                        expected_revision: receipt.expected_revision,
                        phase: Phase::Imported,
                        size: receipt.size,
                        received: receipt.size,
                        profile,
                        library: Some(library),
                        last_error: None,
                        committed_revision: Some(receipt.committed_revision),
                    };
                    if !map.sessions.contains_key(&id) && map.sessions.len() >= MAX_SESSIONS {
                        // Durable readback needs no additional staging/capacity.
                        return Ok(reply(&state));
                    }
                    map.sessions.insert(
                        id.clone(),
                        Session {
                            cleanup_owner: Some(self.persistence.admission().enter()?),
                            begin,
                            state,
                            file: None,
                            reserved: 0,
                            touched: now,
                        },
                    );
                }
            }
        }
        if matches!(
            &request,
            Request::Status { .. }
                | Request::Finish { .. }
                | Request::Cancel { .. }
                | Request::BeginImport { .. }
        ) {
            if let Some(s) = map
                .sessions
                .get_mut(&id)
                .filter(|s| s.state.phase == Phase::Imported)
            {
                let library = self.profiles.list()?;
                if s.state.library.as_ref().map(|l| l.revision) != Some(library.revision) {
                    // A receipt confirms a past commit, not ownership of a slot
                    // after later edits. Do not offer obsolete asset keys as current.
                    s.state.profile = None;
                    s.state.library = Some(library);
                }
            }
        }
        if matches!(
            &request,
            Request::BeginExport { .. } | Request::BeginImport { .. }
        ) {
            if let Some(old) = map.sessions.get_mut(&id) {
                ensure!(
                    old.begin == request,
                    "transfer id already has different parameters"
                );
                old.touched = now;
                return Ok(reply(&old.state));
            }
            ensure!(
                map.sessions.len() < MAX_SESSIONS,
                "archive session capacity exhausted; release acknowledged sessions"
            );
            let (size, revision, group, slot, direction) = match &request {
                Request::BeginImport {
                    size,
                    expected_revision,
                    group,
                    slot,
                    ..
                } => {
                    ensure!(
                        (1..=MAX_BYTES).contains(size),
                        "archive size exceeds transfer budget"
                    );
                    (*size, *expected_revision, *group, *slot, Direction::Import)
                }
                Request::BeginExport {
                    expected_revision,
                    group,
                    slot,
                    ..
                } => (
                    MAX_BYTES,
                    *expected_revision,
                    *group,
                    *slot,
                    Direction::Export,
                ),
                _ => unreachable!(),
            };
            group.validate_slot(slot)?;
            ensure!(
                map.reserved()
                    .checked_add(size)
                    .is_some_and(|n| n <= MAX_RESERVED),
                "archive transfer staging quota exhausted"
            );
            let library = self.profiles.list()?;
            ensure!(
                !library.archive_imports.iter().any(|r| r.id == id),
                "transfer id belongs to a committed import"
            );
            ensure!(
                library.revision == revision,
                "profile library changed; reload before starting archive transfer"
            );
            let mut file = tempfile::tempfile()?;
            let mut state = State {
                id: id.clone(),
                direction,
                group,
                slot,
                expected_revision: revision,
                phase: Phase::Uploading,
                size,
                received: 0,
                profile: None,
                library: None,
                last_error: None,
                committed_revision: None,
            };
            if direction == Direction::Export {
                let profile = library
                    .profiles
                    .iter()
                    .find(|p| p.slot == slot && p.content.group() == group)
                    .context("profile slot is empty")?;
                self.archives.export(profile, &mut file)?;
                let bytes = file.metadata()?.len();
                ensure!(bytes <= MAX_BYTES, "archive exceeds transfer budget");
                file.seek(SeekFrom::Start(0))?;
                state.size = bytes;
                state.phase = Phase::Ready;
                state.profile = Some(profile.clone());
            }
            let reserved = state.size;
            let response = reply(&state);
            map.sessions.insert(
                id,
                Session {
                    cleanup_owner: Some(self.persistence.admission().enter()?),
                    begin: request,
                    state,
                    file: Some(file),
                    reserved,
                    touched: now,
                },
            );
            return Ok(response);
        }
        if matches!(&request, Request::Release { .. }) {
            ensure!(
                map.sessions
                    .get(&id)
                    .is_none_or(|s| s.state.phase != Phase::Uploading),
                "cancel an unfinished upload before releasing it"
            );
            let s = map.sessions.remove(&id).context(
                "unknown or expired archive transfer; read back library before retrying an import",
            )?;
            return Ok(reply(&s.state));
        }
        let s = map.sessions.get_mut(&id).context(
            "unknown or expired archive transfer; read back library before retrying an import",
        )?;
        s.touched = now;
        match request {
            Request::BeginExport { .. } | Request::BeginImport { .. } | Request::Release { .. } => {
                unreachable!()
            }
            Request::Status { .. } => Ok(reply(&s.state)),
            Request::Cancel { .. } => {
                // Imported means committed: cancellation can never undo it.
                if s.state.phase != Phase::Imported {
                    s.state.phase = Phase::Cancelled;
                }
                s.file = None;
                s.reserved = 0;
                Ok(reply(&s.state))
            }
            Request::Chunk { offset, bytes, .. } => {
                ensure!(
                    s.state.direction == Direction::Import && s.state.phase == Phase::Uploading,
                    "archive is not accepting chunks"
                );
                ensure!(
                    !bytes.is_empty() && bytes.len() <= CHUNK,
                    "archive chunk must be1..=64KiB"
                );
                let end = offset
                    .checked_add(bytes.len() as u64)
                    .context("chunk offset overflow")?;
                ensure!(end <= s.state.size, "chunk exceeds declared archive length");
                let file = s.file.as_mut().context("archive staging closed")?;
                file.seek(SeekFrom::Start(offset))?;
                if end <= s.state.received {
                    let mut previous = vec![0; bytes.len()];
                    file.read_exact(&mut previous)?;
                    ensure!(
                        previous == bytes,
                        "chunk retry differs from acknowledged bytes"
                    );
                } else {
                    ensure!(
                        offset == s.state.received,
                        "archive chunks must be contiguous"
                    );
                    file.write_all(&bytes)?;
                    s.state.received = end;
                }
                Ok(reply(&s.state))
            }
            Request::Read { offset, length, .. } => {
                ensure!(
                    s.state.direction == Direction::Export && s.state.phase == Phase::Ready,
                    "archive is not ready for download"
                );
                ensure!(
                    (1..=CHUNK).contains(&length) && offset <= s.state.size,
                    "invalid archive read range"
                );
                let count = (s.state.size - offset).min(length as u64) as usize;
                let mut bytes = vec![0; count];
                let file = s.file.as_mut().context("archive staging closed")?;
                file.seek(SeekFrom::Start(offset))?;
                file.read_exact(&mut bytes)?;
                Ok(Reply {
                    state: s.state.clone(),
                    offset: Some(offset),
                    bytes,
                    eof: offset + count as u64 == s.state.size,
                })
            }
            Request::Inspect { .. } => {
                ensure!(
                    s.state.direction == Direction::Import && s.state.phase == Phase::Uploading,
                    "archive is not available for inspection"
                );
                let file = s.file.as_mut().context("archive staging closed")?;
                file.seek(SeekFrom::Start(0))?;
                let profile = self.archives.inspect(file)?;
                let Request::BeginImport { group, .. } = &s.begin else {
                    unreachable!()
                };
                ensure!(
                    profile.content.group() == *group,
                    "archive group differs from selected target group"
                );
                s.state.profile = Some(profile);
                Ok(reply(&s.state))
            }
            Request::Finish { .. } => {
                ensure!(
                    s.state.direction == Direction::Import,
                    "Finish is import-only"
                );
                if matches!(s.state.phase, Phase::Imported | Phase::Failed) {
                    return Ok(reply(&s.state));
                }
                ensure!(
                    s.state.phase == Phase::Uploading && s.state.received == s.state.size,
                    "archive upload is incomplete or cancelled"
                );
                let Request::BeginImport {
                    expected_revision,
                    group,
                    slot,
                    ..
                } = &s.begin
                else {
                    unreachable!()
                };
                let result = (|| -> anyhow::Result<_> {
                    let file = s.file.as_mut().context("archive staging closed")?;
                    ensure!(
                        file.metadata()?.len() == s.state.size,
                        "archive staging length changed"
                    );
                    file.seek(SeekFrom::Start(0))?;
                    let profile = self.archives.inspect(file)?;
                    ensure!(
                        profile.content.group() == *group,
                        "archive group differs from selected target group"
                    );
                    file.seek(SeekFrom::Start(0))?;
                    let mut digest = [0u8; 32];
                    file.seek(SeekFrom::Start(12))?;
                    file.read_exact(&mut digest)?;
                    file.seek(SeekFrom::Start(0))?;
                    let source_sha256 = digest
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>();
                    let receipt = llcontrol_ipc::ArchiveImportReceipt {
                        source_sha256: Some(source_sha256.clone()),
                        id: id.clone(),
                        size: s.state.size,
                        expected_revision: *expected_revision,
                        committed_revision: expected_revision
                            .checked_add(1)
                            .context("profile revision exhausted")?,
                        group: *group,
                        slot: *slot,
                    };
                    let binding = crate::asset_publication::journal::SlotBinding {
                        operation: id.clone(),
                        group: *group,
                        slot: *slot,
                        expected_revision: *expected_revision,
                        size: s.state.size,
                        source_sha256,
                    };
                    let scope = crate::asset_publication::Publication::begin_slot(
                        self.persistence.clone(),
                        binding,
                    )?;
                    let outcome = self.archives.import_transfer_scoped(
                        file,
                        &self.profiles,
                        receipt.clone(),
                        scope.clone(),
                    );
                    let library = match outcome {
                        Ok(library) => {
                            scope.retain()?;
                            library
                        }
                        Err(original) => {
                            match self.profiles.recover_import(&id) {
                                Ok(None) => {
                                    scope.rollback().with_context(|| {
                                        format!("{original}; slot publication cleanup failed")
                                    })?;
                                }
                                Ok(Some((found, _))) if found == receipt => scope.retain()?,
                                Ok(Some(_)) | Err(_) => scope.retain_unknown()?,
                            }
                            return Err(original);
                        }
                    };
                    let imported = library
                        .profiles
                        .iter()
                        .find(|p| p.content.group() == *group && p.slot == *slot)
                        .context("committed archive slot missing")?
                        .clone();
                    Ok((imported, library))
                })();
                s.file = None;
                s.reserved = 0;
                match result {
                    Ok((mut profile, library)) => {
                        profile.slot = *slot;
                        s.state.profile = Some(profile);
                        s.state.committed_revision = Some(library.revision);
                        s.state.library = Some(library);
                        s.state.phase = Phase::Imported;
                    }
                    Err(e) => {
                        s.state.phase = Phase::Failed;
                        s.state.last_error = Some(format!(
                            "{e}; read back profile library before another import"
                        ));
                    }
                }
                // Retain the exact receipt for Finish/Status after a lost reply.
                s.touched = Instant::now();
                Ok(reply(&s.state))
            }
        }
    }
}
fn reply(state: &State) -> Reply {
    Reply {
        state: state.clone(),
        offset: None,
        bytes: Vec::new(),
        eof: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        media_assets::MediaAssetStore, theme_assets::ThemeAssetStore, theme_fonts::ThemeFontStore,
    };
    use llcontrol_ipc::{NamedProfile, ProfileGroup};
    fn fixture() -> (tempfile::TempDir, ArchiveTransfers, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let archive = ProfileArchives::new(
            ThemeAssetStore::new(root.join("images")),
            ThemeFontStore::new(root.join("fonts")),
            MediaAssetStore::new(root.join("media")),
        );
        let mut bytes = Vec::new();
        archive
            .export(
                &NamedProfile::defaults(ProfileGroup::Fan, 0, Some("cpu")).unwrap(),
                &mut bytes,
            )
            .unwrap();
        let store = Arc::new(crate::persistence::PersistenceStore::new_with_root(
            root.join("state"),
        ));
        let archive = ProfileArchives::new(
            store.theme_assets(),
            store.theme_fonts(),
            store.media_assets(),
        );
        let service = ArchiveTransfers::new(archive, store);
        (dir, service, bytes)
    }
    fn id(n: usize) -> String {
        format!("{n:032x}")
    }
    fn begin(n: usize, size: u64) -> Request {
        Request::BeginImport {
            id: id(n),
            size,
            expected_revision: 0,
            group: llcontrol_ipc::ProfileGroup::Fan,
            slot: 2,
        }
    }
    #[test]
    fn exact_begin_chunk_and_finish_retries_keep_one_commit_and_release_can_restore_receipt() {
        let (_dir, s, bytes) = fixture();
        let now = Instant::now();
        let b = begin(1, bytes.len() as u64);
        let first = s.handle_at(b.clone(), now).unwrap();
        assert_eq!(first, s.handle_at(b, now).unwrap());
        assert!(s.handle_at(begin(1, bytes.len() as u64 + 1), now).is_err());
        assert!(s.handle_at(Request::Finish { id: id(1) }, now).is_err());
        let chunk = Request::Chunk {
            id: id(1),
            offset: 0,
            bytes: bytes.clone(),
        };
        let r = s.handle_at(chunk.clone(), now).unwrap();
        assert_eq!(r, s.handle_at(chunk, now).unwrap());
        let mut changed = bytes.clone();
        changed[0] ^= 1;
        assert!(s
            .handle_at(
                Request::Chunk {
                    id: id(1),
                    offset: 0,
                    bytes: changed
                },
                now
            )
            .is_err());
        assert!(s.handle_at(Request::Release { id: id(1) }, now).is_err());
        let inspected = s.handle_at(Request::Inspect { id: id(1) }, now).unwrap();
        assert_eq!(inspected.state.profile.unwrap().slot, 0);
        let done = s.handle_at(Request::Finish { id: id(1) }, now).unwrap();
        assert_eq!(done.state.phase, Phase::Imported);
        assert_eq!(done.state.committed_revision, Some(1));
        assert_eq!(done.state.profile.as_ref().unwrap().slot, 2);
        assert_eq!(
            done,
            s.handle_at(Request::Finish { id: id(1) }, now).unwrap()
        );
        assert_eq!(
            done,
            s.handle_at(Request::Cancel { id: id(1) }, now).unwrap()
        );
        s.handle_at(Request::Release { id: id(1) }, now).unwrap();
        assert_eq!(
            done,
            s.handle_at(Request::Finish { id: id(1) }, now).unwrap()
        );
        assert_eq!(s.profiles.list().unwrap().revision, 1);
        assert_eq!(s.profiles.list().unwrap().archive_imports.len(), 1);
    }
    #[test]
    fn cancelled_and_expired_staging_releases_quota_without_committing() {
        let (_dir, s, _) = fixture();
        let now = Instant::now();
        s.handle_at(begin(1, MAX_BYTES), now).unwrap();
        s.handle_at(begin(2, MAX_BYTES), now).unwrap();
        assert!(s.handle_at(begin(3, 1), now).is_err());
        s.handle_at(Request::Cancel { id: id(1) }, now).unwrap();
        s.handle_at(begin(3, 1), now).unwrap();
        s.handle_at(begin(4, 1), now).unwrap();
        assert!(s.handle_at(begin(5, 1), now).is_err());
        s.handle_at(Request::Release { id: id(1) }, now).unwrap();
        s.handle_at(begin(5, 1), now).unwrap();
        assert!(s
            .handle_at(Request::Status { id: id(2) }, now + IDLE)
            .is_err());
        assert!(s.registry.lock().unwrap().sessions.is_empty());
        assert!(s.profiles.list().unwrap().profiles.is_empty());
    }
    #[test]
    fn invalid_chunks_and_wrong_group_never_modify_library_and_failure_receipt_is_stable() {
        let (_dir, s, bytes) = fixture();
        let now = Instant::now();
        let begin = Request::BeginImport {
            id: id(1),
            size: bytes.len() as u64,
            expected_revision: 0,
            group: ProfileGroup::Pump,
            slot: 0,
        };
        s.handle_at(begin, now).unwrap();
        for (offset, bytes) in [
            (1, vec![1]),
            (u64::MAX, vec![1]),
            (0, vec![]),
            (0, vec![1; CHUNK + 1]),
        ] {
            assert!(s
                .handle_at(
                    Request::Chunk {
                        id: id(1),
                        offset,
                        bytes
                    },
                    now
                )
                .is_err());
        }
        s.handle_at(
            Request::Chunk {
                id: id(1),
                offset: 0,
                bytes,
            },
            now,
        )
        .unwrap();
        assert!(s.handle_at(Request::Inspect { id: id(1) }, now).is_err());
        let failed = s.handle_at(Request::Finish { id: id(1) }, now).unwrap();
        assert_eq!(failed.state.phase, Phase::Failed);
        assert!(failed.state.last_error.as_ref().unwrap().contains("group"));
        assert_eq!(
            failed,
            s.handle_at(Request::Finish { id: id(1) }, now).unwrap()
        );
        assert_eq!(s.registry.lock().unwrap().reserved(), 0);
        assert!(s.profiles.list().unwrap().profiles.is_empty());
    }
    #[tokio::test]
    async fn worker_admission_and_busy_transaction_fail_fast_without_allocating_session() {
        let (_dir, s, _) = fixture();
        let a = s.jobs.clone().acquire_owned().await.unwrap();
        let b = s.jobs.clone().acquire_owned().await.unwrap();
        assert!(matches!(
            s.handle(begin(1, 1)).await,
            Err(llcontrol_ipc::IpcError::DeviceBusy(_))
        ));
        drop(a);
        drop(b);
        let guard = s.registry.lock().unwrap();
        assert!(matches!(
            s.handle_at(begin(1, 1), Instant::now()),
            Err(llcontrol_ipc::IpcError::DeviceBusy(_))
        ));
        drop(guard);
        assert!(s.registry.lock().unwrap().sessions.is_empty());
    }
    #[test]
    fn durable_readback_remains_available_when_all_transfer_slots_are_occupied() {
        let (_dir, s, bytes) = fixture();
        let now = Instant::now();
        s.handle_at(begin(1, bytes.len() as u64), now).unwrap();
        s.handle_at(
            Request::Chunk {
                id: id(1),
                offset: 0,
                bytes,
            },
            now,
        )
        .unwrap();
        let done = s.handle_at(Request::Finish { id: id(1) }, now).unwrap();
        s.handle_at(Request::Release { id: id(1) }, now).unwrap();
        for n in 2..6 {
            s.handle_at(
                Request::BeginImport {
                    id: id(n),
                    size: 1,
                    expected_revision: 1,
                    group: llcontrol_ipc::ProfileGroup::Fan,
                    slot: 0,
                },
                now,
            )
            .unwrap();
        }
        assert_eq!(s.registry.lock().unwrap().sessions.len(), 4);
        assert_eq!(
            done,
            s.handle_at(Request::Status { id: id(1) }, now).unwrap()
        );
        assert_eq!(s.registry.lock().unwrap().sessions.len(), 4);
    }
    fn staged_screen() -> (tempfile::TempDir, ArchiveTransfers) {
        let (dir, service, _) = fixture();
        let source = ThemeAssetStore::new(dir.path().join("source-images"));
        let key = source
            .import_named(
                llcontrol_image::defaults::IMAGES[0].png,
                Some("Transferred image"),
            )
            .unwrap();
        let theme = serde_json::from_value(serde_json::json!({
            "version":1,"name":"Transferred","background":[0,0,0],
            "background_image":{"asset":key,"fit":"crop"},"layers":[]
        }))
        .unwrap();
        let archive = ProfileArchives::new(
            source,
            ThemeFontStore::new(dir.path().join("source-fonts")),
            MediaAssetStore::new(dir.path().join("source-media")),
        );
        let mut bytes = Vec::new();
        archive
            .export(
                &NamedProfile {
                    slot: 0,
                    name: "Transferred".into(),
                    content: llcontrol_ipc::ProfileContent::Screen(theme),
                },
                &mut bytes,
            )
            .unwrap();
        let now = Instant::now();
        service
            .handle_at(
                Request::BeginImport {
                    id: id(1),
                    size: bytes.len() as u64,
                    expected_revision: 0,
                    group: ProfileGroup::Screen,
                    slot: 7,
                },
                now,
            )
            .unwrap();
        for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
            service
                .handle_at(
                    Request::Chunk {
                        id: id(1),
                        offset: (index * CHUNK) as u64,
                        bytes: chunk.to_vec(),
                    },
                    now,
                )
                .unwrap();
        }
        (dir, service)
    }
    fn hold_catalog(
        service: &ArchiveTransfers,
    ) -> (std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>) {
        let assets = service.persistence.theme_assets();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            assets
                .with_catalog(|_| {
                    held_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(20)).unwrap();
                    Ok(())
                })
                .unwrap();
        });
        held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        (release_tx, thread)
    }
    async fn wait_for_decoded_payload(root: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let decoded = std::fs::read_dir(root.join("state/import-publications"))
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .any(|e| e.path().join("0").is_file());
            if decoded {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "import did not reach decoded payload staging"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    #[tokio::test]
    async fn stale_slot_commit_rolls_back_only_its_assets_after_publication() {
        let (dir, service) = staged_screen();
        let ordinary = service
            .persistence
            .theme_assets()
            .import_named(
                llcontrol_image::defaults::IMAGES[1].png,
                Some("Ordinary upload"),
            )
            .unwrap();
        let before = service.persistence.theme_assets().list().unwrap();
        let (release, held) = hold_catalog(&service);
        let worker = {
            let s = service.clone();
            tokio::spawn(async move { s.handle(Request::Finish { id: id(1) }).await })
        };
        wait_for_decoded_payload(dir.path()).await;
        let other = NamedProfile::defaults(ProfileGroup::Fan, 0, Some("cpu")).unwrap();
        let expected = service
            .profiles
            .edit(0, llcontrol_ipc::ProfileEdit::Save { profile: other })
            .unwrap();
        release.send(()).unwrap();
        held.join().unwrap();
        let result = worker.await.unwrap().unwrap();
        assert_eq!(result.state.phase, Phase::Failed);
        assert_eq!(service.profiles.list().unwrap(), expected);
        assert_eq!(service.persistence.theme_assets().list().unwrap(), before);
        assert!(before.contains(&ordinary));
        assert!(service
            .persistence
            .recover_import_publications()
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn caller_loss_does_not_release_worker_or_duplicate_a_slot_commit() {
        let (dir, service) = staged_screen();
        let (release, held) = hold_catalog(&service);
        let caller = {
            let s = service.clone();
            tokio::spawn(async move { s.handle(Request::Finish { id: id(1) }).await })
        };
        wait_for_decoded_payload(dir.path()).await;
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        assert_eq!(service.jobs.available_permits(), 1);
        release.send(()).unwrap();
        held.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let reply = loop {
            match service.handle(Request::Status { id: id(1) }).await {
                Ok(reply) => break reply,
                Err(llcontrol_ipc::IpcError::DeviceBusy(_)) => {}
                Err(error) => panic!("{error}"),
            }
            assert!(Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert_eq!(reply.state.phase, Phase::Imported);
        assert_eq!(reply.state.committed_revision, Some(1));
        assert_eq!(
            service.handle(Request::Finish { id: id(1) }).await.unwrap(),
            reply
        );
        let library = service.profiles.list().unwrap();
        assert_eq!(library.revision, 1);
        assert_eq!(library.archive_imports.len(), 1);
        assert_eq!(
            library.archive_imports[0]
                .source_sha256
                .as_ref()
                .unwrap()
                .len(),
            64
        );
        assert!(service
            .persistence
            .recover_import_publications()
            .unwrap()
            .is_empty());
    }
}
