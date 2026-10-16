//! Private full-group publication ownership. Never infer ownership from a
//! directory diff: only files reported after exclusive creation are eligible.
pub(crate) mod journal;
use crate::persistence::PersistenceStore;
use anyhow::{ensure, Context};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[derive(Debug, Clone, Default)]
pub(crate) struct PendingFiles(Arc<Mutex<BTreeMap<String, u128>>>);
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl PendingFiles {
    pub fn hide(&self, key: &str, token: u128) {
        lock(&self.0).insert(key.into(), token);
    }
    pub fn owns(&self, key: &str, token: u128) -> bool {
        lock(&self.0).get(key) == Some(&token)
    }
    pub fn hidden(&self, key: &str) -> bool {
        lock(&self.0).contains_key(key)
    }
    pub fn reveal(&self, key: &str, token: u128) {
        let mut entries = lock(&self.0);
        if entries.get(key) == Some(&token) {
            entries.remove(key);
        }
    }
}
pub(crate) fn check_root(path: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "asset root must be a regular directory"
    );
    Ok(())
}
#[derive(Debug, Clone)]
pub(crate) struct FileIdentity {
    device: u64,
    inode: u64,
    // Keep the original inode alive: path replacement must not trick rollback
    // through immediate inode reuse. Clones share one bounded descriptor.
    _original: Arc<File>,
}
impl FileIdentity {
    pub fn capture(file: &File) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
            _original: Arc::new(file.try_clone()?),
        })
    }
    /// None = absent, Some(false) = replacement/symlink: never unlink it.
    pub fn matches(&self, path: &Path) -> anyhow::Result<Option<bool>> {
        use std::os::unix::fs::MetadataExt;
        let m = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(
            m.is_file()
                && !m.file_type().is_symlink()
                && m.dev() == self.device
                && m.ino() == self.inode,
        ))
    }
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CreatedKind {
    Image,
    ImageName,
    Font,
    Media,
}
#[derive(Debug, Clone)]
struct Created {
    kind: CreatedKind,
    key: String,
    identity: Option<FileIdentity>,
}
pub(crate) struct Publication {
    store: Arc<PersistenceStore>,
    token: u128,
    operation: String,
    records: Mutex<Vec<Created>>,
    resolved: AtomicBool,
    journal: Option<Arc<journal::Journal>>,
    cleanup_owner: Option<crate::admission::Lease>,
}
impl std::fmt::Debug for Publication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Publication")
            .field("operation", &self.operation)
            .field("created", &lock(&self.records).len())
            .finish_non_exhaustive()
    }
}
impl Publication {
    #[cfg(test)]
    pub fn new(store: Arc<PersistenceStore>, operation: String) -> Arc<Self> {
        let cleanup_owner = Some(store.admission().enter().unwrap());
        Arc::new(Self {
            cleanup_owner,
            store,
            operation,
            token: rand::random(),
            records: Mutex::default(),
            resolved: AtomicBool::new(false),
            journal: None,
        })
    }
    pub fn begin(
        store: Arc<PersistenceStore>,
        binding: journal::Binding,
    ) -> anyhow::Result<Arc<Self>> {
        let cleanup_owner = Some(store.admission().enter()?);
        let journal = journal::Journal::create(
            store.publication_root(),
            journal::Target::FullGroup(binding.clone()),
        )?;
        Ok(Arc::new(Self {
            cleanup_owner,
            store,
            operation: binding.operation,
            token: rand::random(),
            records: Mutex::default(),
            resolved: AtomicBool::new(false),
            journal: Some(journal),
        }))
    }
    pub fn begin_slot(
        store: Arc<PersistenceStore>,
        binding: journal::SlotBinding,
    ) -> anyhow::Result<Arc<Self>> {
        let cleanup_owner = Some(store.admission().enter()?);
        let journal = journal::Journal::create(
            store.publication_root(),
            journal::Target::NamedSlot(binding.clone()),
        )?;
        Ok(Arc::new(Self {
            cleanup_owner,
            store,
            operation: binding.operation,
            token: rand::random(),
            records: Mutex::default(),
            resolved: AtomicBool::new(false),
            journal: Some(journal),
        }))
    }
    pub fn staging_directory(&self) -> Option<&Path> {
        self.journal.as_ref().map(|j| j.directory())
    }
    pub fn arm(&self) -> anyhow::Result<()> {
        if let Some(j) = &self.journal {
            j.arm()?;
        }
        Ok(())
    }
    /// Observe a synced private candidate before its final-name publication.
    /// Returning an error must veto that publication; no owned-path inference.
    pub fn created(&self, kind: CreatedKind, key: &str, file: &File) -> anyhow::Result<()> {
        let identity = FileIdentity::capture(file)?;
        if let Some(j) = &self.journal {
            j.record(kind, key, file)?;
        }
        lock(&self.records).push(Created {
            kind,
            key: key.into(),
            identity: Some(identity),
        });
        match kind {
            CreatedKind::Image | CreatedKind::ImageName => {
                self.store.theme_assets().pending.hide(key, self.token)
            }
            CreatedKind::Font => self.store.theme_fonts().pending.hide(key, self.token),
            CreatedKind::Media => self.store.media_assets().pending.hide(key, self.token),
        }
        Ok(())
    }
    pub fn font_exemptions(&self) -> BTreeSet<String> {
        lock(&self.records)
            .iter()
            .filter(|r| r.kind == CreatedKind::Font)
            .map(|r| r.key.clone())
            .collect()
    }
    pub fn retain(&self) -> anyhow::Result<()> {
        if self.resolved.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(j) = &self.journal {
            j.keep()?;
        }
        self.resolved.store(true, Ordering::Release);
        let records = std::mem::take(&mut *lock(&self.records));
        reveal(&self.store, self.token, &records);
        drop(records);
        if let Some(j) = &self.journal {
            j.finish()?;
        }
        Ok(())
    }
    pub fn retain_unknown(&self) -> anyhow::Result<()> {
        if self.resolved.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(j) = &self.journal {
            j.note_unknown()?;
        }
        self.retain()
    }
    /// Blocking; after the actor result and a serialized negative receipt lookup.
    /// Unknown receipt/storage outcomes must use retain(), never this method.
    pub fn rollback(&self) -> anyhow::Result<()> {
        if self.resolved.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(j) = &self.journal {
            j.discard()?;
        }
        if self.resolved.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let records = std::mem::take(&mut *lock(&self.records));
        let mut errors = Vec::new();
        for record in &records {
            if record.kind == CreatedKind::ImageName {
                continue;
            }
            let result = (|| -> anyhow::Result<()> {
                let identity = record
                    .identity
                    .clone()
                    .context("created file identity unavailable; retained")?;
                let key = &record.key;
                match record.kind {
                    CreatedKind::Image => {
                        let name = records
                            .iter()
                            .find(|r| r.kind == CreatedKind::ImageName && r.key == *key);
                        ensure!(
                            name.is_none_or(|r| r.identity.is_some()),
                            "image metadata identity unavailable; retained"
                        );
                        self.store.theme_assets().rollback_pending(key,self.token,identity,name.and_then(|r|r.identity.clone()),||{
                            let refs=self.store.referenced_theme_assets()?;let library=self.store.profiles().list()?;
                            Ok(refs.contains(key)||library.profiles.iter().any(|p|matches!(&p.content,lconnect_ipc::ProfileContent::Screen(t) if t.image_assets().contains(key.as_str()))))
                        })?;
                    }
                    CreatedKind::Font => {
                        self.store.theme_fonts().rollback_pending(key,self.token,identity,||{
                        let refs=self.store.referenced_theme_fonts()?;let library=self.store.profiles().list()?;
                        Ok(refs.contains(key)||library.profiles.iter().any(|p|matches!(&p.content,lconnect_ipc::ProfileContent::Screen(t) if t.font_assets().contains(key.as_str()))))
                    })?;
                    }
                    CreatedKind::Media => {
                        self.store.media_assets().rollback_pending(
                            key,
                            self.token,
                            identity,
                            || {
                                let refs = self.store.referenced_media_clips()?;
                                let library = self.store.profiles().list()?;
                                Ok(refs.contains(key)
                                    || library.profiles.iter().any(|p| match &p.content {
                                        lconnect_ipc::ProfileContent::Screen(t) => {
                                            t.video_assets().contains(key.as_str())
                                        }
                                        lconnect_ipc::ProfileContent::ScreenMedia(k) => k == key,
                                        _ => false,
                                    }))
                            },
                        )?;
                    }
                    CreatedKind::ImageName => unreachable!(),
                }
                Ok(())
            })();
            if let Err(e) = result {
                errors.push(format!("{:?} {}: {e:#}", record.kind, record.key));
            }
        }
        // Retained/adopted files become ordinary assets. Never leave a hidden
        // object behind after an error, or delete a file whose identity changed.
        reveal(&self.store, self.token, &records);
        drop(records);
        ensure!(
            errors.is_empty(),
            "asset rollback incomplete: {}",
            errors.join("; ")
        );
        if let Some(j) = &self.journal {
            j.finish()?;
        }
        Ok(())
    }
}
fn reveal(store: &PersistenceStore, token: u128, records: &[Created]) {
    for r in records {
        match r.kind {
            CreatedKind::Image | CreatedKind::ImageName => {
                store.theme_assets().pending.reveal(&r.key, token)
            }
            CreatedKind::Font => store.theme_fonts().pending.reveal(&r.key, token),
            CreatedKind::Media => store.media_assets().pending.reveal(&r.key, token),
        }
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        // Panic/unknown outcome: retain, never blindly roll back committed assets.
        let records = std::mem::take(self.records.get_mut().unwrap_or_else(|e| e.into_inner()));
        if records.is_empty() && (self.journal.is_none() || self.resolved.load(Ordering::Acquire)) {
            return;
        }
        let store = self.store.clone();
        let token = self.token;
        let journal = self.journal.clone();
        let cleanup = move || {
            if let Some(j) = &journal {
                j.note_unknown()?;
                j.keep()?;
            }
            reveal(&store, token, &records);
            if let Some(j) = &journal {
                j.finish()?;
            }
            Ok(())
        };
        if let Some(owner) = self.cleanup_owner.take() {
            owner.cleanup(cleanup);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        persistence::DeviceSettings,
        profile_archive::{PreparedGroup, ProfileArchives},
        profile_group::{GroupKind, GroupProfile, GroupSettings},
    };
    #[tokio::test]
    async fn sealed_publication_drop_is_joined_and_failed_unknown_cleanup_blocks_preflight() {
        for fault in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
            let publication = Publication::begin_slot(
                store.clone(),
                journal::SlotBinding {
                    operation: "12345678901234567890123456789012".into(),
                    group: lconnect_ipc::ProfileGroup::Fan,
                    slot: 0,
                    expected_revision: 0,
                    size: 44,
                    source_sha256: "a".repeat(64),
                },
            )
            .unwrap();
            let directory = publication.staging_directory().unwrap().to_path_buf();
            if fault {
                std::fs::create_dir(store.publication_root().join("slot-recovery-report.json"))
                    .unwrap();
            }
            let gate = store.admission();
            let maintenance = gate.begin_maintenance().unwrap();
            drop(publication);
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                maintenance.wait_checked(),
            )
            .await
            .unwrap();
            assert_eq!(result.is_err(), fault);
            assert_eq!(gate.active(), 0);
            assert_eq!(directory.exists(), fault);
            assert!(gate.is_closed());
            if fault {
                assert!(directory.join("manifest.json").is_file());
            }
        }
    }
    const FONT: &[u8] = include_bytes!("../../lconnect-image/assets/DejaVuSans.ttf");
    struct Fixture {
        _source: tempfile::TempDir,
        _target: tempfile::TempDir,
        store: Arc<PersistenceStore>,
        archives: ProfileArchives,
        bytes: Vec<u8>,
    }
    impl Fixture {
        fn new() -> Self {
            let source_root = tempfile::tempdir().unwrap();
            let source = PersistenceStore::new_with_root(source_root.path().into());
            let image = source
                .theme_assets()
                .import_named(
                    lconnect_image::defaults::IMAGES[0].png,
                    Some("Portable image"),
                )
                .unwrap();
            let font = source.theme_fonts().import(FONT).unwrap();
            let mut writer = source
                .media_assets()
                .create_file("Gallery video".into())
                .unwrap();
            writer
                .append(&crate::builtin_media::read_unit(0).unwrap())
                .unwrap();
            source.media_assets().publish_file(writer).unwrap();
            let settings = DeviceSettings {
                theme:Some(serde_json::from_value(serde_json::json!({"version":1,"name":"Owned","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[{"kind":"text","text":"Font","style":{"x":0,"y":0,"size":24,"color":[255,255,255],"align":"left","font_asset":font}}]})).unwrap()),
                ..Default::default()
            };
            let profile = GroupProfile::capture(&settings, &Default::default(), GroupKind::Screen)
                .unwrap()
                .profile;
            let source_archives = ProfileArchives::new(
                source.theme_assets(),
                source.theme_fonts(),
                source.media_assets(),
            );
            let mut bytes = Vec::new();
            source_archives.export_group(&profile, &mut bytes).unwrap();
            let target = tempfile::tempdir().unwrap();
            let store = Arc::new(PersistenceStore::new_with_root(target.path().into()));
            let archives = ProfileArchives::new(
                store.theme_assets(),
                store.theme_fonts(),
                store.media_assets(),
            );
            Self {
                _source: source_root,
                _target: target,
                store,
                archives,
                bytes,
            }
        }
        fn publication(&self) -> Arc<Publication> {
            Publication::new(
                self.store.clone(),
                format!("{:032x}", rand::random::<u128>()),
            )
        }
        fn prepare(&self, scope: &Arc<Publication>) -> PreparedGroup {
            self.archives
                .prepare_group_scoped(&mut std::io::Cursor::new(&self.bytes), Some(scope.clone()))
                .unwrap()
        }
        fn keys(group: &PreparedGroup) -> (String, String, String) {
            let image = group.uploaded_images().first().unwrap().clone();
            let GroupSettings::Screen(g) = &group.profile().content else {
                panic!()
            };
            let font = g
                .theme
                .as_ref()
                .unwrap()
                .font_assets()
                .into_iter()
                .next()
                .unwrap()
                .to_owned();
            let media = group.uploaded_media().first().unwrap().clone();
            (image, font, media)
        }
        fn fonts(&self) -> usize {
            std::fs::read_dir(self.store.theme_fonts_root())
                .unwrap()
                .filter(|e| {
                    e.as_ref()
                        .unwrap()
                        .path()
                        .extension()
                        .is_some_and(|e| e == "font")
                })
                .count()
        }
    }
    #[test]
    fn rollback_hides_pending_catalogs_and_never_sweeps_parallel_publications() {
        let f = Fixture::new();
        let a = f.publication();
        let ga = f.prepare(&a);
        let ka = Fixture::keys(&ga);
        let b = f.publication();
        let gb = f.prepare(&b);
        let kb = Fixture::keys(&gb);
        let foreign = f
            .store
            .theme_assets()
            .import(lconnect_image::defaults::IMAGES[1].png)
            .unwrap();
        assert_eq!(
            f.store.theme_assets().list().unwrap().len(),
            lconnect_image::defaults::IMAGES.len() + 1
        );
        assert_eq!(f.store.media_assets().list().unwrap().len(), 1);
        f.store
            .theme_assets()
            .with_catalog(|keys| {
                assert!(!keys.contains(&ka.0) && !keys.contains(&kb.0));
                Ok(())
            })
            .unwrap();
        f.store
            .media_assets()
            .with_catalog(|keys| {
                assert!(!keys.contains(&ka.2) && !keys.contains(&kb.2));
                Ok(())
            })
            .unwrap();
        drop(ga);
        a.rollback().unwrap();
        assert_eq!(f.fonts(), 1);
        assert!(!f
            .store
            .theme_assets_root()
            .join(format!("{}.png", ka.0))
            .exists());
        assert!(!f
            .store
            .theme_assets_root()
            .join(format!("{}.name.json", ka.0))
            .exists());
        f.store.theme_assets().read_for_edit(&foreign).unwrap();
        b.retain().unwrap();
        assert!(f.store.theme_assets().list().unwrap().contains(&kb.0));
        assert_eq!(f.store.media_assets().list().unwrap().len(), 2);
        f.store.media_assets().acquire_source(&kb.2).unwrap();
        assert_eq!(f.store.theme_fonts().load(&kb.1).unwrap(), FONT);
        drop(gb);
    }
    #[test]
    fn rollback_preserves_external_image_media_pins_and_observed_fonts() {
        let f = Fixture::new();
        let scope = f.publication();
        let group = f.prepare(&scope);
        let (image, font, media) = Fixture::keys(&group);
        let image_pin = f.store.theme_assets().acquire_reference(&image).unwrap();
        let media_pin = f.store.media_assets().acquire_source(&media).unwrap();
        assert_eq!(f.store.theme_fonts().load(&font).unwrap(), FONT);
        drop(group);
        scope.rollback().unwrap();
        assert!(f.store.theme_assets().list().unwrap().contains(&image));
        assert_eq!(f.store.media_assets().list().unwrap().len(), 2);
        assert_eq!(f.fonts(), 1);
        drop((image_pin, media_pin));
    }
    #[test]
    fn rollback_preserves_offline_saved_references_and_unknown_outcomes() {
        let f = Fixture::new();
        let scope = f.publication();
        let group = f.prepare(&scope);
        let keys = Fixture::keys(&group);
        let GroupSettings::Screen(g) = &group.profile().content else {
            panic!()
        };
        let settings = DeviceSettings {
            theme: g.theme.clone(),
            screen_collections: g.collections.clone(),
            ..Default::default()
        };
        f.store.write("offline", &settings).unwrap();
        drop(group);
        scope.rollback().unwrap();
        assert!(f.store.theme_assets().list().unwrap().contains(&keys.0));
        assert_eq!(f.store.media_assets().list().unwrap().len(), 2);
        assert_eq!(f.fonts(), 1);
        // Unknown commit outcome uses retention rather than negative inference.
        let uncertain = f.publication();
        let group = f.prepare(&uncertain);
        let keys = Fixture::keys(&group);
        drop(group);
        uncertain.retain().unwrap();
        assert!(f.store.theme_assets().list().unwrap().contains(&keys.0));
        assert!(f.store.theme_fonts().load(&keys.1).is_ok());
        assert!(f.store.media_assets().acquire_source(&keys.2).is_ok());
    }
    #[test]
    fn partial_publication_failure_removes_its_prefix_and_preserves_foreign_blocker() {
        let f = Fixture::new();
        let blocker = f.store.theme_fonts_root();
        std::fs::write(&blocker, b"foreign file").unwrap();
        let scope = f.publication();
        assert!(f
            .archives
            .prepare_group_scoped(&mut std::io::Cursor::new(&f.bytes), Some(scope.clone()))
            .is_err());
        scope.rollback().unwrap();
        assert_eq!(std::fs::read(blocker).unwrap(), b"foreign file");
        assert_eq!(
            f.store.theme_assets().list().unwrap().len(),
            lconnect_image::defaults::IMAGES.len()
        );
        assert_eq!(
            std::fs::read_dir(f.store.theme_assets_root())
                .unwrap()
                .count(),
            0
        );
    }
    #[test]
    fn rollback_never_unlinks_replaced_paths_or_renamed_metadata() {
        let f = Fixture::new();
        let scope = f.publication();
        let group = f.prepare(&scope);
        let (image, font, _) = Fixture::keys(&group);
        drop(group);
        // Keep original ownership descriptors alive so replacing a path cannot
        // reuse an inode and masquerade as the file created by this operation.
        let path = f.store.theme_fonts_root().join(format!("{font}.font"));
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, FONT).unwrap();
        let metadata = f.store.theme_assets().list_named().unwrap();
        assert!(!metadata.iter().any(|m| m.key == image));
        f.store
            .theme_assets()
            .rename(&image, "Adopted image", 1)
            .unwrap();
        scope.rollback().unwrap();
        assert_eq!(std::fs::read(path).unwrap(), FONT);
        assert!(f
            .store
            .theme_assets()
            .list_named()
            .unwrap()
            .iter()
            .any(|m| m.key == image && m.name.as_deref() == Some("Adopted image")));
    }
    #[test]
    fn creating_image_metadata_never_overwrites_a_conflicting_foreign_sidecar() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let scope = Publication::new(store.clone(), "55555555555555555555555555555555".into());
        let mut claimed = None;
        let result = store.theme_assets().import_named_observed(
            lconnect_image::defaults::IMAGES[0].png,
            Some("New upload"),
            |key, name, file| {
                scope.created(
                    if name {
                        CreatedKind::ImageName
                    } else {
                        CreatedKind::Image
                    },
                    key,
                    file,
                )?;
                if !name {
                    let path = store.theme_assets_root().join(format!("{key}.name.json"));
                    std::fs::write(&path, b"foreign metadata").unwrap();
                    claimed = Some(path);
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        scope.rollback().unwrap();
        let path = claimed.unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"foreign metadata");
        assert_eq!(
            store.theme_assets().list().unwrap().len(),
            lconnect_image::defaults::IMAGES.len()
        );
    }
    #[test]
    fn ownership_observer_precedes_every_final_asset_publication() {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let mut observed = 0;
        store
            .theme_assets()
            .import_named_observed(
                lconnect_image::defaults::IMAGES[0].png,
                Some("Before publish"),
                |key, name, file| {
                    assert!(file.metadata().unwrap().is_file());
                    let suffix = if name { "name.json" } else { "png" };
                    assert!(
                        !store
                            .theme_assets_root()
                            .join(format!("{key}.{suffix}"))
                            .exists(),
                        "Image or sidecar reached final namespace before its ownership callback"
                    );
                    observed += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(observed, 2);
        store
            .theme_fonts()
            .import_observed(FONT, |key, file| {
                assert!(file.metadata().unwrap().is_file());
                assert!(
                    !store
                        .theme_fonts_root()
                        .join(format!("{key}.font"))
                        .exists(),
                    "Font reached final namespace before its ownership callback"
                );
                Ok(())
            })
            .unwrap();
        let mut writer = store
            .media_assets()
            .create_file("Before publish".into())
            .unwrap();
        writer
            .append(&crate::builtin_media::read_unit(0).unwrap())
            .unwrap();
        store
            .media_assets()
            .publish_file_observed(writer, |key, file| {
                assert!(file.metadata().unwrap().is_file());
                assert!(
                    !root
                        .path()
                        .join("prepared-clips")
                        .join(format!("{key}.lcm"))
                        .exists(),
                    "Media reached final namespace before its ownership callback"
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn ownership_failure_vetoes_image_sidecar_font_and_media_publication() {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        for fail_name in [false, true] {
            let mut key = String::new();
            let result = store.theme_assets().import_named_observed(
                lconnect_image::defaults::IMAGES[0].png,
                Some("Unpublished"),
                |candidate, name, file| {
                    key = candidate.into();
                    assert!(file.metadata()?.len() > 0);
                    if name == fail_name {
                        anyhow::bail!("ownership journal fsync failed");
                    }
                    Ok(())
                },
            );
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("ownership journal fsync failed"));
            assert!(!store
                .theme_assets_root()
                .join(format!("{key}.png"))
                .exists());
            assert!(!store
                .theme_assets_root()
                .join(format!("{key}.name.json"))
                .exists());
        }
        let mut font = String::new();
        assert!(store
            .theme_fonts()
            .import_observed(FONT, |key, file| {
                font = key.into();
                assert!(file.metadata()?.len() > 0);
                anyhow::bail!("ownership journal fsync failed")
            })
            .is_err());
        assert!(!store
            .theme_fonts_root()
            .join(format!("{font}.font"))
            .exists());
        let mut writer = store
            .media_assets()
            .create_file("Unpublished".into())
            .unwrap();
        writer
            .append(&crate::builtin_media::read_unit(0).unwrap())
            .unwrap();
        let mut media = String::new();
        assert!(store
            .media_assets()
            .publish_file_observed(writer, |key, file| {
                media = key.into();
                assert!(file.metadata()?.len() > 0);
                anyhow::bail!("ownership journal fsync failed")
            })
            .is_err());
        assert!(!root
            .path()
            .join("prepared-clips")
            .join(format!("{media}.lcm"))
            .exists());
        // Atomic staging must also disappear on these ordinary error paths.
        for folder in [
            store.theme_assets_root(),
            store.theme_fonts_root(),
            root.path().join("prepared-clips"),
        ] {
            assert!(std::fs::read_dir(folder).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
        }
    }
    #[test]
    fn metadata_veto_preserves_a_concurrently_replaced_image() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let scope = Publication::new(store.clone(), "66666666666666666666666666666666".into());
        let mut replaced = None;
        let result = store.theme_assets().import_named_observed(
            lconnect_image::defaults::IMAGES[0].png,
            Some("Original"),
            |key, name, file| {
                scope.created(
                    if name {
                        CreatedKind::ImageName
                    } else {
                        CreatedKind::Image
                    },
                    key,
                    file,
                )?;
                if name {
                    let path = store.theme_assets_root().join(format!("{key}.png"));
                    let foreign = root.path().join("replacement");
                    std::fs::write(&foreign, lconnect_image::defaults::IMAGES[1].png)?;
                    std::fs::rename(foreign, &path)?;
                    replaced = Some(path);
                    anyhow::bail!("metadata ownership journal failure");
                }
                Ok(())
            },
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("metadata ownership journal failure"));
        scope.rollback().unwrap();
        assert_eq!(
            std::fs::read(replaced.unwrap()).unwrap(),
            lconnect_image::defaults::IMAGES[1].png
        );
    }
}
