//! Per-user font storage. Blocking operations must run off the async executor.
use lconnect_image::fonts::{validate_font, MAX_FONT_BYTES};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Debug)]
pub struct ThemeFontStore {
    pub(crate) pending: crate::asset_publication::PendingFiles,
    observed: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
    root: PathBuf,
    session: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
}
impl ThemeFontStore {
    /// Construct without filesystem or device access; use a private settings root.
    pub fn new(root: PathBuf) -> Self {
        Self {
            pending: Default::default(),
            observed: Default::default(),
            root,
            session: Default::default(),
        }
    }

    /// Preload only referenced fonts, once per key. Bound total source bytes per theme.
    pub fn load_theme(
        &self,
        theme: &lconnect_ipc::LcdTheme,
    ) -> anyhow::Result<lconnect_image::theme::ThemeFonts> {
        self.load_theme_exempt(theme, &Default::default())
    }
    pub(crate) fn load_theme_exempt(
        &self,
        theme: &lconnect_ipc::LcdTheme,
        exempt: &std::collections::BTreeSet<String>,
    ) -> anyhow::Result<lconnect_image::theme::ThemeFonts> {
        theme.validate()?;
        let mut fonts = lconnect_image::theme::ThemeFonts::new();
        let mut total = 0usize;
        for key in theme.font_assets() {
            let bytes = self.load_internal(key, !exempt.contains(key))?;
            total += bytes.len();
            anyhow::ensure!(total <= 64 * 1024 * 1024, "theme fonts exceed 64 MiB total");
            fonts.insert(key.to_owned(), lconnect_image::fonts::prepare_font(bytes)?);
        }
        Ok(fonts)
    }

    /// Persist the original font bytes under a generated key. Never overwrite assets.
    pub fn import(&self, bytes: &[u8]) -> anyhow::Result<String> {
        let key = format!("{:032x}", rand::random::<u128>());
        self.import_key(&key, bytes)?;
        Ok(key)
    }

    #[cfg(test)]
    pub(crate) fn import_observed(
        &self,
        bytes: &[u8],
        created: impl FnOnce(&str, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        self.import_observed_in(bytes, None, created)
    }
    pub(crate) fn import_observed_in(
        &self,
        bytes: &[u8],
        stage: Option<&std::path::Path>,
        created: impl FnOnce(&str, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        let key = format!("{:032x}", rand::random::<u128>());
        self.import_key_observed(&key, bytes, stage, |file| created(&key, file))?;
        Ok(key)
    }

    /// Same upload key is reused after ambiguous durability errors.
    pub(crate) fn import_key(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        self.import_key_observed(key, bytes, None, |_| Ok(()))
    }
    fn import_key_observed(
        &self,
        key: &str,
        bytes: &[u8],
        stage: Option<&std::path::Path>,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        validate_font(bytes)?;
        let path = self.path(key)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("font lock poisoned"))?;
        // Keep even ambiguous persistence outcomes until the next daemon session.
        session.insert(key.to_owned());
        let absolute = std::path::absolute(&self.root)?;
        fs::create_dir_all(&absolute)?;
        // Sync ancestors even on retry: directory existence alone does not prove
        // that an earlier interrupted first import persisted its parent entries.
        for directory in absolute.ancestors() {
            fs::File::open(directory)?.sync_all()?;
        }
        if path.try_exists()? {
            anyhow::ensure!(
                self.load_unpinned(key)? == bytes,
                "font key already contains different bytes"
            );
            fs::File::open(&path)?.sync_all()?;
            fs::File::open(&self.root)?.sync_all()?;
            return Ok(());
        }
        let mut temporary = tempfile::NamedTempFile::new_in(stage.unwrap_or(&self.root))?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        created(temporary.as_file())?;
        temporary.persist_noclobber(path)?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    /// Read and validate again: files might have changed since they were imported.
    pub fn load(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        self.load_internal(key, true)
    }
    fn load_internal(&self, key: &str, observed: bool) -> anyhow::Result<Vec<u8>> {
        self.path(key)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("font lock poisoned"))?;
        let bytes = self.load_unpinned(key)?;
        session.insert(key.to_owned());
        if observed {
            self.observed
                .lock()
                .map_err(|_| anyhow::anyhow!("font observation lock poisoned"))?
                .insert(key.to_owned());
        }
        Ok(bytes)
    }

    fn load_unpinned(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.path(key)?;
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "font must be a regular file"
        );
        anyhow::ensure!(
            metadata.len() <= MAX_FONT_BYTES as u64,
            "font exceeds 16 MiB"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_FONT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        validate_font(&bytes)?;
        Ok(bytes)
    }

    pub(crate) fn rollback_pending(
        &self,
        key: &str,
        token: u128,
        identity: crate::asset_publication::FileIdentity,
        referenced: impl FnOnce() -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("font lock poisoned"))?;
        if !self.pending.owns(key, token) {
            return Ok(());
        }
        crate::asset_publication::check_root(&self.root)?;
        let observed = self
            .observed
            .lock()
            .map_err(|_| anyhow::anyhow!("font observation lock poisoned"))?;
        if observed.contains(key) || referenced()? {
            return Ok(());
        }
        let path = self.path(key)?;
        match identity.matches(&path)? {
            Some(false) => return Ok(()),
            Some(true) => fs::remove_file(path)?,
            None => {}
        }
        fs::File::open(&self.root)?.sync_all()?;
        session.remove(key);
        Ok(())
    }

    /// Only imported 32-hex font names are eligible. Unknown files are left alone.
    /// Session pins protect drafts, accepted jobs and debounced settings until restart.
    pub fn cleanup(
        &self,
        referenced: &std::collections::BTreeSet<String>,
        keep: &[String],
        delete: bool,
    ) -> anyhow::Result<lconnect_ipc::AssetCleanupReport> {
        self.cleanup_with_remove(referenced, keep, delete, |path| fs::remove_file(path))
    }

    fn cleanup_with_remove(
        &self,
        referenced: &std::collections::BTreeSet<String>,
        keep: &[String],
        delete: bool,
        mut remove: impl FnMut(&std::path::Path) -> std::io::Result<()>,
    ) -> anyhow::Result<lconnect_ipc::AssetCleanupReport> {
        anyhow::ensure!(keep.len() <= 1024, "too many draft font keys");
        for key in keep {
            self.path(key)?;
        }
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("font lock poisoned"))?;
        let mut report = lconnect_ipc::AssetCleanupReport::default();
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "font root must be a regular directory"
        );
        for (index, entry) in fs::read_dir(&self.root)?.enumerate() {
            anyhow::ensure!(index < 10000, "font cleanup scan exceeds 10000 entries");
            let entry = entry?;
            let filename = entry.file_name();
            let Some(key) = filename.to_str().and_then(|s| s.strip_suffix(".font")) else {
                report.skipped += 1;
                continue;
            };
            if key.len() != 32
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !entry.file_type()?.is_file()
            {
                report.skipped += 1;
                continue;
            }
            if referenced.contains(key) {
                report.retained_referenced += 1;
            } else if session.contains(key) {
                report.retained_session += 1;
            } else if keep.iter().any(|k| k == key) {
                report.retained_draft += 1;
            } else {
                report.candidates.push(key.to_owned());
            }
        }
        report.candidates.sort();
        // Validate the whole scan before any deletion; per-file I/O failures remain visible.
        if delete {
            for key in &report.candidates {
                match remove(&self.path(key)?) {
                    Ok(()) => report.deleted.push(key.clone()),
                    Err(e) => report.failures.push(lconnect_ipc::AssetCleanupFailure {
                        key: key.clone(),
                        error: e.to_string(),
                    }),
                }
            }
        }
        Ok(report)
    }

    fn path(&self, key: &str) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            key.len() == 32
                && key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid font asset key"
        );
        Ok(self.root.join(format!("{key}.font")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_after_rename_uses_same_file_and_rejects_changed_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = ThemeFontStore::new(dir.path().into());
        let key = "00000000000000000000000000000001";
        let original = include_bytes!("../../lconnect-image/assets/DejaVuSans.ttf");
        store.import_key(key, original).unwrap();
        // Models a persisted file whose successful response/session update was lost.
        store.import_key(key, original).unwrap();
        assert!(store
            .import_key(
                key,
                include_bytes!("../../lconnect-image/assets/DejaVuSerif.ttf")
            )
            .is_err());
        assert_eq!(store.load(key).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn partial_unlink_failure_is_reported_and_retry_removes_only_remaining_orphan() {
        let dir = tempfile::tempdir().unwrap();
        let store = ThemeFontStore::new(dir.path().into());
        let failed = "00000000000000000000000000000001";
        let removed = "00000000000000000000000000000002";
        for key in [failed, removed] {
            fs::write(store.path(key).unwrap(), b"prior-session orphan").unwrap();
        }
        let report = store
            .cleanup_with_remove(&Default::default(), &[], true, |path| {
                if path.file_stem().unwrap() == failed {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "fixture denial",
                    ))
                } else {
                    fs::remove_file(path)
                }
            })
            .unwrap();
        assert_eq!(report.deleted, [removed]);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].key, failed);
        assert!(report.failures[0].error.contains("fixture denial"));
        assert!(store.path(failed).unwrap().exists());
        assert!(!store.path(removed).unwrap().exists());
        let retry = store.cleanup(&Default::default(), &[], true).unwrap();
        assert_eq!(retry.deleted, [failed]);
        assert!(retry.failures.is_empty());
    }

    #[test]
    fn concurrent_load_import_and_cleanup_never_remove_a_successfully_prepared_font() {
        use std::sync::{Arc, Barrier};
        let font = include_bytes!("../../lconnect-image/assets/DejaVuSans.ttf");
        // Both load/cleanup orderings are valid: a cleanup winner causes load to fail;
        // a successful load must pin its bytes for the remainder of this session.
        for _ in 0..32 {
            let dir = tempfile::tempdir().unwrap();
            let old = ThemeFontStore::new(dir.path().into());
            let key = old.import(font).unwrap();
            let store = ThemeFontStore::new(dir.path().into());
            let barrier = Arc::new(Barrier::new(3));
            std::thread::scope(|scope| {
                let loaded = scope.spawn(|| {
                    barrier.wait();
                    store.load(&key)
                });
                let imported = scope.spawn(|| {
                    barrier.wait();
                    store.import(font).unwrap()
                });
                barrier.wait();
                let report = store.cleanup(&Default::default(), &[], true).unwrap();
                let result = loaded.join().unwrap();
                let imported_key = imported.join().unwrap();
                assert_eq!(store.load(&imported_key).unwrap(), font);
                match result {
                    Ok(bytes) => {
                        assert_eq!(bytes, font);
                        assert!(!report.deleted.contains(&key));
                        assert_eq!(store.load(&key).unwrap(), font);
                    }
                    Err(_) => assert!(report.deleted.contains(&key)),
                }
                assert!(store
                    .cleanup(&Default::default(), &[], true)
                    .unwrap()
                    .deleted
                    .is_empty());
            });
        }
    }
}
