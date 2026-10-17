//! Per-user, daemon-managed assets. Call these blocking operations off Tokio workers.
use lconnect_image::{
    assets::{decode_asset, normalize_asset, MAX_ASSET_BYTES},
    defaults,
    theme::ThemeAssets,
};
use lconnect_ipc::LcdTheme;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetName {
    version: u32,
    revision: u64,
    name: String,
}

type LiveImages =
    std::collections::BTreeMap<String, Vec<std::sync::Weak<lconnect_image::theme::DynamicImage>>>;

/// Lightweight protection for collection membership while its settings commit is pending.
/// The decoded pixels are not retained by this lease.
#[derive(Debug, Clone)]
pub struct ImagePin {
    _lease: std::sync::Arc<()>,
}

type ImagePins = std::collections::BTreeMap<String, std::sync::Weak<()>>;

/// Persistent PNG storage. Keys cannot be paths, and imports never overwrite files.
#[derive(Debug, Clone)]
pub struct ThemeAssetStore {
    pub(crate) pending: crate::asset_publication::PendingFiles,
    root: PathBuf,
    live: std::sync::Arc<std::sync::Mutex<LiveImages>>,
    pins: std::sync::Arc<std::sync::Mutex<ImagePins>>,
    session: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
}
impl ThemeAssetStore {
    /// Construct without filesystem or device access.
    pub fn new(root: PathBuf) -> Self {
        Self {
            pending: Default::default(),
            root,
            session: Default::default(),
            live: Default::default(),
            pins: Default::default(),
        }
    }

    /// Decode and persist an image, returning its portable managed key.
    pub fn import(&self, bytes: &[u8]) -> anyhow::Result<String> {
        self.import_named(bytes, None)
    }

    pub fn import_named(&self, bytes: &[u8], name: Option<&str>) -> anyhow::Result<String> {
        self.import_named_observed(bytes, name, |_, _, _| Ok(()))
    }
    pub(crate) fn import_named_observed(
        &self,
        bytes: &[u8],
        name: Option<&str>,
        created: impl FnMut(&str, bool, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        self.import_named_in(bytes, name, None, created)
    }
    pub(crate) fn import_named_in(
        &self,
        bytes: &[u8],
        name: Option<&str>,
        stage: Option<&std::path::Path>,
        mut created: impl FnMut(&str, bool, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        if let Some(name) = name {
            Self::validate_name(name)?;
        }
        let normalized = normalize_asset(bytes)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        fs::create_dir_all(&self.root)?;
        let key = format!("{:032x}", rand::random::<u128>());
        let path = self.path(&key)?;
        // The observer may persist ownership before the final namespace entry.
        // Its failure must prevent publication, including ordinary imports.
        let mut temporary = tempfile::NamedTempFile::new_in(stage.unwrap_or(&self.root))?;
        temporary.write_all(&normalized)?;
        temporary.as_file().sync_all()?;
        let identity = crate::asset_publication::FileIdentity::capture(temporary.as_file())?;
        created(&key, false, temporary.as_file())?;
        temporary.persist_noclobber(&path).map_err(|e| e.error)?;
        if let Some(name) = name {
            if let Err(error) = self.write_name_observed(
                &key,
                &AssetName {
                    version: 1,
                    revision: 1,
                    name: name.to_owned(),
                },
                stage,
                |file| created(&key, true, file),
            ) {
                if identity.matches(&path)? == Some(true) {
                    let _ = fs::remove_file(&path);
                }
                return Err(error);
            }
        }
        fs::File::open(&self.root)?.sync_all()?;
        session.insert(key.clone());
        Ok(key)
    }

    fn validate_name(name: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
            "image name must contain 1..256 UTF-8 bytes without control characters"
        );
        Ok(())
    }

    fn read_name(&self, key: &str) -> anyhow::Result<lconnect_ipc::ThemeAssetInfo> {
        if let Some(asset) = defaults::image(key) {
            return Ok(lconnect_ipc::ThemeAssetInfo {
                key: key.to_owned(),
                name: Some(asset.name.to_owned()),
                revision: 0,
                is_default: true,
                contexts: asset.contexts.to_vec(),
            });
        }
        let path = self.path(key)?.with_extension("name.json");
        let mut info = lconnect_ipc::ThemeAssetInfo {
            key: key.to_owned(),
            name: None,
            revision: 0,
            is_default: false,
            contexts: lconnect_ipc::all_image_contexts(),
        };
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(info),
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= 4096,
            "invalid image metadata file"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?.take(4097).read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() <= 4096, "image metadata too large");
        let record: AssetName = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            record.version == 1 && record.revision > 0,
            "unsupported image metadata version/revision"
        );
        Self::validate_name(&record.name)?;
        info.name = Some(record.name);
        info.revision = record.revision;
        Ok(info)
    }

    fn write_name(&self, key: &str, record: &AssetName) -> anyhow::Result<()> {
        self.write_name_inner(key, record, true, None, |_| Ok(()))
    }
    fn write_name_observed(
        &self,
        key: &str,
        record: &AssetName,
        stage: Option<&std::path::Path>,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.write_name_inner(key, record, false, stage, created)
    }
    fn write_name_inner(
        &self,
        key: &str,
        record: &AssetName,
        replace: bool,
        stage: Option<&std::path::Path>,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let path = self.path(key)?.with_extension("name.json");
        let mut temporary = tempfile::Builder::new()
            .prefix(".name-")
            .suffix(".tmp")
            .tempfile_in(stage.unwrap_or(&self.root))?;
        temporary.write_all(&serde_json::to_vec(record)?)?;
        temporary.as_file().sync_all()?;
        created(temporary.as_file())?;
        if replace {
            temporary.persist(path)
        } else {
            temporary.persist_noclobber(path)
        }
        .map_err(|e| e.error)?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    pub fn list_named(&self) -> anyhow::Result<Vec<lconnect_ipc::ThemeAssetInfo>> {
        let keys = self.list()?;
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let mut entries = Vec::new();
        for key in keys {
            if defaults::image(&key).is_some() {
                entries.push(self.read_name(&key)?);
                continue;
            }
            let metadata = match fs::symlink_metadata(self.path(&key)?) {
                Ok(value) => value,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                entries.push(self.read_name(&key)?);
            }
        }
        Ok(entries)
    }

    /// Serialize metadata edits with cleanup/import; image keys and bytes never change.
    pub fn rename(
        &self,
        key: &str,
        name: &str,
        expected_revision: u64,
    ) -> anyhow::Result<lconnect_ipc::ThemeAssetInfo> {
        anyhow::ensure!(
            defaults::image(key).is_none(),
            "builtin images cannot be renamed"
        );
        Self::validate_name(name)?;
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let metadata = fs::symlink_metadata(self.path(key)?)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "asset must be a regular file"
        );
        let current = self.read_name(key)?;
        anyhow::ensure!(
            current.revision == expected_revision,
            "image name changed; refresh before retrying"
        );
        let revision = current
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("image revision overflow"))?;
        self.write_name(
            key,
            &AssetName {
                version: 1,
                revision,
                name: name.to_owned(),
            },
        )?;
        self.read_name(key)
    }

    /// Enumerate managed images without decoding or pinning them. Unknown files
    /// and symlinks are excluded; the whole scan is bounded before returning.
    pub fn list(&self) -> anyhow::Result<Vec<String>> {
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        self.list_unlocked()
    }

    pub(crate) fn with_catalog<T>(
        &self,
        action: impl FnOnce(Vec<String>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _guard = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        action(self.list_unlocked()?)
    }

    fn list_unlocked(&self) -> anyhow::Result<Vec<String>> {
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(defaults::IMAGES.iter().map(|a| a.key.to_owned()).collect())
            }
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "asset root must be a regular directory"
        );
        let mut keys = Vec::new();
        for (index, entry) in fs::read_dir(&self.root)?.enumerate() {
            anyhow::ensure!(index < 10000, "asset gallery scan exceeds 10000 entries");
            let entry = entry?;
            let filename = entry.file_name();
            let Some(key) = filename.to_str().and_then(|s| s.strip_suffix(".png")) else {
                continue;
            };
            if key.len() == 32
                && key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && entry.file_type()?.is_file()
                && !self.pending.hidden(key)
            {
                keys.push(key.to_owned());
            }
        }
        keys.sort();
        let mut all: Vec<String> = defaults::IMAGES.iter().map(|a| a.key.to_owned()).collect();
        all.extend(keys);
        Ok(all)
    }

    /// Read an immutable user asset for editing, without changing references or pins.
    /// Builtins are not editor inputs. Returned bytes use the bounded upload budget.
    pub fn read_for_edit(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::ensure!(
            defaults::image(key).is_none(),
            "builtin images cannot be edited"
        );
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let path = self.path(key)?;
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "asset must be a regular file"
        );
        anyhow::ensure!(metadata.len() <= 1024 * 1024, "editor source exceeds 1 MiB");
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() <= 1024 * 1024, "editor source exceeds 1 MiB");
        decode_asset(&bytes)?;
        Ok(bytes)
    }

    /// Read a complete normalized source for a portable profile archive.
    /// Unlike the editor's smaller transfer, this accepts the full asset budget.
    pub(crate) fn read_for_archive(&self, key: &str) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        if let Some(builtin) = defaults::image(key) {
            return Ok((builtin.png.to_vec(), Some(builtin.name.to_owned())));
        }
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let path = self.path(key)?;
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "asset must be a regular file"
        );
        anyhow::ensure!(
            metadata.len() <= MAX_ASSET_BYTES as u64,
            "asset exceeds size limit"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_ASSET_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() <= MAX_ASSET_BYTES, "asset exceeds size limit");
        let image = decode_asset(&bytes)?;
        anyhow::ensure!(
            image.width() <= 960 && image.height() <= 960,
            "stored asset is not normalized"
        );
        Ok((bytes, self.read_name(key)?.name))
    }

    /// Preview does not pin: merely browsing must not change cleanup eligibility.
    pub fn thumbnail(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        if let Some(asset) = defaults::image(key) {
            return Ok(lconnect_image::assets::thumbnail_asset(asset.png)?);
        }
        let path = self.path(key)?;
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "asset must be a regular file"
        );
        anyhow::ensure!(
            metadata.len() <= MAX_ASSET_BYTES as u64,
            "asset exceeds size limit"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_ASSET_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        Ok(lconnect_image::assets::thumbnail_asset(&bytes)?)
    }

    /// Load only referenced assets, once each. Missing assets fail before theme apply.
    pub fn load(&self, theme: &LcdTheme) -> anyhow::Result<ThemeAssets> {
        theme.validate()?;
        self.load_keys(theme.image_assets())
    }

    pub(crate) fn load_keys<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a str>,
    ) -> anyhow::Result<ThemeAssets> {
        self.load_keys_budgeted(keys, 256 * 1024 * 1024)
    }

    fn load_keys_budgeted<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a str>,
        decoded_budget: u64,
    ) -> anyhow::Result<ThemeAssets> {
        let mut decoded_bytes = 0u64;
        let mut reserve = |width: u32, height: u32| -> anyhow::Result<()> {
            decoded_bytes = decoded_bytes
                .checked_add(u64::from(width) * u64::from(height) * 4)
                .ok_or_else(|| anyhow::anyhow!("theme decoded image budget overflow"))?;
            anyhow::ensure!(
                decoded_bytes <= decoded_budget,
                "theme decoded images exceed aggregate budget"
            );
            Ok(())
        };
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let mut assets = ThemeAssets::new();
        for asset in keys {
            if assets.contains_key(asset) {
                continue;
            }
            if let Some(builtin) = defaults::image(asset) {
                let image = decode_asset(builtin.png)?;
                reserve(image.width(), image.height())?;
                assets.insert(asset.to_owned(), std::sync::Arc::new(image));
                continue;
            }
            let path = self.path(asset)?;
            let metadata = fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "asset must be a regular file"
            );
            anyhow::ensure!(
                metadata.len() <= MAX_ASSET_BYTES as u64,
                "asset exceeds size limit"
            );
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(MAX_ASSET_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            let image = decode_asset(&bytes)?;
            // Recheck externally mutable files, then bound all unique active and
            // inactive images before retaining their RGBA allocations.
            anyhow::ensure!(
                image.width() <= 960 && image.height() <= 960,
                "stored asset is not normalized"
            );
            reserve(image.width(), image.height())?;
            assets.insert(
                asset.to_owned(),
                std::sync::Arc::new(image.into_rgba8().into()),
            );
        }
        let mut live = self
            .live
            .lock()
            .map_err(|_| anyhow::anyhow!("asset live lock poisoned"))?;
        for (key, image) in &assets {
            let leases = live.entry(key.clone()).or_default();
            leases.retain(|lease| lease.strong_count() > 0);
            leases.push(std::sync::Arc::downgrade(image));
        }
        session.extend(assets.keys().cloned());
        Ok(assets)
    }

    /// Validate one image and retain only a lightweight deletion lease. Decoding
    /// one key at a time bounds memory when preparing a whole collection.
    pub fn acquire_reference(&self, key: &str) -> anyhow::Result<ImagePin> {
        let decoded = self.load_keys(std::iter::once(key))?;
        // The decoded lease closes the gap before the lightweight pin is
        // installed. Delete takes session first, then checks both lease kinds.
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let mut pins = self
            .pins
            .lock()
            .map_err(|_| anyhow::anyhow!("asset pin lock poisoned"))?;
        pins.retain(|_, lease| lease.strong_count() > 0);
        let lease = pins
            .get(key)
            .and_then(std::sync::Weak::upgrade)
            .unwrap_or_else(|| {
                let lease = std::sync::Arc::new(());
                pins.insert(key.to_owned(), std::sync::Arc::downgrade(&lease));
                lease
            });
        drop(decoded);
        Ok(ImagePin { _lease: lease })
    }

    /// Explicit deletion may remove an unused import from this session. Unlike
    /// bulk cleanup, it checks live decoded images and durable references under
    /// the same lock used by load/import, rather than retaining every session key.
    pub fn delete_unreferenced(
        &self,
        key: &str,
        expected_revision: u64,
        keep: &[String],
        referenced: impl FnOnce() -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            defaults::image(key).is_none(),
            "builtin images cannot be deleted"
        );
        anyhow::ensure!(keep.len() <= 1024, "too many draft asset keys");
        for draft in keep {
            self.path(draft)?;
        }
        let path = self.path(key)?;
        let root = fs::symlink_metadata(&self.root)?;
        anyhow::ensure!(
            root.is_dir() && !root.file_type().is_symlink(),
            "asset root must be a regular directory"
        );
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let mut live = self
            .live
            .lock()
            .map_err(|_| anyhow::anyhow!("asset live lock poisoned"))?;
        anyhow::ensure!(
            !keep.iter().any(|draft| draft == key),
            "image is used by the draft"
        );
        anyhow::ensure!(
            !live
                .get(key)
                .is_some_and(|leases| leases.iter().any(|lease| lease.strong_count() > 0)),
            "image is used by an active or pending theme"
        );
        anyhow::ensure!(
            self.pins
                .lock()
                .map_err(|_| anyhow::anyhow!("asset pin lock poisoned"))?
                .get(key)
                .is_none_or(|lease| lease.strong_count() == 0),
            "image is used by a pending collection"
        );
        anyhow::ensure!(
            !referenced()?,
            "image is referenced by saved settings or a profile"
        );
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "asset must be a regular file"
        );
        anyhow::ensure!(
            self.read_name(key)?.revision == expected_revision,
            "image changed; refresh before deleting"
        );
        fs::remove_file(&path)?;
        if let Err(error) = fs::remove_file(path.with_extension("name.json")) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error.into());
            }
        }
        fs::File::open(&self.root)?.sync_all()?;
        session.remove(key);
        live.remove(key);
        Ok(())
    }

    pub(crate) fn rollback_pending(
        &self,
        key: &str,
        token: u128,
        image: crate::asset_publication::FileIdentity,
        name: Option<crate::asset_publication::FileIdentity>,
        referenced: impl FnOnce() -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        if !self.pending.owns(key, token) {
            return Ok(());
        }
        crate::asset_publication::check_root(&self.root)?;
        let mut live = self
            .live
            .lock()
            .map_err(|_| anyhow::anyhow!("asset live lock poisoned"))?;
        let pins = self
            .pins
            .lock()
            .map_err(|_| anyhow::anyhow!("asset pin lock poisoned"))?;
        if live
            .get(key)
            .is_some_and(|v| v.iter().any(|p| p.strong_count() > 0))
            || pins.get(key).is_some_and(|p| p.strong_count() > 0)
            || referenced()?
        {
            return Ok(());
        }
        let path = self.path(key)?;
        let metadata = path.with_extension("name.json");
        let image_owned = image.matches(&path)?;
        let name_owned = match name {
            Some(id) => id.matches(&metadata)?,
            None => match fs::symlink_metadata(&metadata) {
                Ok(_) => Some(false),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            },
        };
        if image_owned == Some(false) || name_owned == Some(false) {
            return Ok(());
        }
        if image_owned == Some(true) {
            fs::remove_file(path)?;
        }
        if name_owned == Some(true) {
            fs::remove_file(metadata)?;
        }
        fs::File::open(&self.root)?.sync_all()?;
        session.remove(key);
        live.remove(key);
        Ok(())
    }

    /// Only imported 32-hex PNG names are eligible. Unknown files are left alone.
    /// Session pins protect drafts, accepted jobs and debounced settings until restart.
    pub fn cleanup(
        &self,
        referenced: &std::collections::BTreeSet<String>,
        keep: &[String],
        delete: bool,
    ) -> anyhow::Result<lconnect_ipc::AssetCleanupReport> {
        anyhow::ensure!(keep.len() <= 1024, "too many draft asset keys");
        for key in keep {
            self.path(key)?;
        }
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("asset lock poisoned"))?;
        let mut report = lconnect_ipc::AssetCleanupReport::default();
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "asset root must be a regular directory"
        );
        for (index, entry) in fs::read_dir(&self.root)?.enumerate() {
            anyhow::ensure!(index < 10000, "asset cleanup scan exceeds 10000 entries");
            let entry = entry?;
            let filename = entry.file_name();
            let Some(key) = filename.to_str().and_then(|s| s.strip_suffix(".png")) else {
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
                match fs::remove_file(self.path(key)?) {
                    Ok(()) => {
                        report.deleted.push(key.clone());
                        if let Err(e) = fs::remove_file(self.path(key)?.with_extension("name.json"))
                        {
                            if e.kind() != std::io::ErrorKind::NotFound {
                                report.failures.push(lconnect_ipc::AssetCleanupFailure {
                                    key: key.clone(),
                                    error: format!("image deleted; metadata cleanup failed: {e}"),
                                });
                            }
                        }
                    }
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
            !key.is_empty()
                && key.len() <= 128
                && key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid asset key"
        );
        Ok(self.root.join(format!("{key}.png")))
    }
}

#[cfg(test)]
mod decoded_budget_tests {
    use super::*;
    #[test]
    fn aggregate_image_budget_counts_unique_assets_and_rejects_without_retaining_partial_pins() {
        let directory = tempfile::tempdir().unwrap();
        let store = ThemeAssetStore::new(directory.path().into());
        let first = defaults::IMAGES[0].key;
        let second = defaults::IMAGES[1].key;
        let image = decode_asset(defaults::IMAGES[0].png).unwrap();
        let budget = u64::from(image.width()) * u64::from(image.height()) * 4;
        assert_eq!(
            store
                .load_keys_budgeted([first, first], budget)
                .unwrap()
                .len(),
            1
        );
        let isolated = ThemeAssetStore::new(directory.path().join("isolated"));
        assert!(isolated
            .load_keys_budgeted([first, second], budget)
            .unwrap_err()
            .to_string()
            .contains("aggregate budget"));
        assert!(isolated.session.lock().unwrap().is_empty());
        assert!(isolated.live.lock().unwrap().is_empty());
    }
}
