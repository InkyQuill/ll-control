//! Atomic storage for prepared clips. Blocking calls belong on blocking workers.
use crate::builtin_media;
use crate::media_file::{MediaFile, MediaFileWriter};
use anyhow::{ensure, Context};
use llcontrol_ipc::{validate_h264_clip, H264Clip, PreparedClipInfo};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};

const MAX_RECORD_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CLIPS: usize = 64;
const MAX_DIRECTORY_ENTRIES: usize = 1024;

#[derive(Debug, Clone)]
pub struct MediaAssetStore {
    pub(crate) pending: crate::asset_publication::PendingFiles,
    root: PathBuf,
    gate: Arc<Mutex<BTreeMap<String, Weak<()>>>>,
}
/// Protects a saved clip while playback preparation, an actor or a pending
/// settings write owns it. Clones share one pin; the final drop releases it.
/// Persisted and offline references must also be checked before deletion.
#[derive(Debug, Clone)]
pub struct MediaPin {
    _leases: Vec<Arc<()>>,
}

impl MediaPin {
    pub(crate) fn retain(&mut self, other: Self) {
        self._leases.extend(other._leases);
    }
}

/// A file reader cannot outlive its deletion guard. The worker owns this object;
/// additional pins may be cloned for profile/pending-settings commits.
pub struct PinnedMedia {
    name: String,
    source: MediaSource,
    pin: MediaPin,
}
impl std::fmt::Debug for PinnedMedia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedMedia")
            .field("frames", &self.frames())
            .field("fps", &self.fps())
            .finish_non_exhaustive()
    }
}
enum MediaSource {
    Builtin,
    Legacy(std::sync::Arc<H264Clip>),
    Indexed(MediaFile),
}
impl PinnedMedia {
    /// A retired export will never read again: release deletion protection now,
    /// then dispose its descriptor on an owned cleanup worker before drain ends.
    pub(crate) fn retire_export(self, permit: crate::admission::WorkPermit) {
        let Self { source, pin, .. } = self;
        drop(pin);
        let lease = permit.lease();
        lease.cleanup(move || {
            let _permit = permit;
            drop(source);
            Ok(())
        });
    }

    pub(crate) fn try_clone(&self) -> anyhow::Result<Self> {
        let source = match &self.source {
            MediaSource::Builtin => MediaSource::Builtin,
            MediaSource::Legacy(clip) => MediaSource::Legacy(clip.clone()),
            MediaSource::Indexed(file) => MediaSource::Indexed(file.try_clone()?),
        };
        Ok(Self {
            name: self.name.clone(),
            source,
            pin: self.pin.clone(),
        })
    }
    pub(crate) fn keyframe_before(&self, frame: usize) -> anyhow::Result<usize> {
        match &self.source {
            MediaSource::Builtin => builtin_media::keyframe_before(frame),
            MediaSource::Legacy(clip) => clip
                .units
                .get(..=frame)
                .context("media frame outside index")?
                .iter()
                .rposition(|unit| unit.keyframe)
                .context("media has no preceding keyframe"),
            MediaSource::Indexed(file) => file.keyframe_before(frame),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn pin(&self) -> MediaPin {
        self.pin.clone()
    }
    pub fn frames(&self) -> usize {
        match &self.source {
            MediaSource::Builtin => builtin_media::frames(),
            MediaSource::Legacy(clip) => clip.units.len(),
            MediaSource::Indexed(file) => file.frames(),
        }
    }
    pub fn fps(&self) -> u8 {
        match &self.source {
            MediaSource::Builtin => builtin_media::FPS,
            MediaSource::Legacy(clip) => clip.fps,
            MediaSource::Indexed(file) => file.fps(),
        }
    }
    /// Encoded byte count from validated metadata, without reading all frames.
    pub fn encoded_bytes(&self) -> u64 {
        match &self.source {
            MediaSource::Builtin => builtin_media::info().encoded_bytes,
            MediaSource::Legacy(clip) => {
                clip.units.iter().map(|unit| unit.bytes.len() as u64).sum()
            }
            MediaSource::Indexed(file) => file.encoded_bytes(),
        }
    }
    /// Blocking for indexed media. Call only on the reader worker.
    pub fn read_unit(&mut self, frame: usize) -> anyhow::Result<llcontrol_ipc::H264AccessUnit> {
        match &mut self.source {
            MediaSource::Builtin => builtin_media::read_unit(frame),
            MediaSource::Legacy(clip) => clip
                .units
                .get(frame)
                .cloned()
                .context("media frame outside index"),
            MediaSource::Indexed(file) => file.read_unit(frame),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    name: String,
    clip: H264Clip,
}
impl MediaAssetStore {
    /// Construct without accessing disk or devices. Clones share admission.
    pub fn new(root: PathBuf) -> Self {
        Self {
            pending: Default::default(),
            root,
            gate: Arc::default(),
        }
    }

    /// Keep every active/inactive page video alive through preparation and commit.
    pub fn pin_theme(&self, theme: &llcontrol_ipc::LcdTheme) -> anyhow::Result<Option<MediaPin>> {
        theme.validate()?;
        let mut pins: Option<MediaPin> = None;
        for key in theme.video_assets() {
            let pin = self.acquire_source(key)?.pin();
            if let Some(group) = &mut pins {
                group.retain(pin);
            } else {
                pins = Some(pin);
            }
        }
        Ok(pins)
    }
    /// Validate completely, then atomically publish without replacing any asset.
    pub fn import(&self, name: String, clip: H264Clip) -> anyhow::Result<PreparedClipInfo> {
        validate_h264_clip(&clip, 1)?;
        validate_name(&name)?;
        let record = Record {
            version: 1,
            name,
            clip,
        };
        let bytes = serde_json::to_vec(&record)?;
        ensure!(
            bytes.len() as u64 <= MAX_RECORD_BYTES,
            "prepared clip record too large"
        );
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        fs::create_dir_all(&self.root)?;
        ensure!(
            self.keys()?.len() < MAX_CLIPS,
            "prepared clip library is full (64 clips)"
        );
        let key = format!("{:032x}", rand::random::<u128>());
        ensure!(key != builtin_media::KEY, "reserved builtin media key");
        let path = self.path(&key)?;
        ensure!(
            fs::symlink_metadata(path.with_extension("lcm"))
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "media key collision"
        );
        let mut staged = tempfile::NamedTempFile::new_in(&self.root)?;
        staged.write_all(&bytes)?;
        staged.as_file().sync_all()?;
        staged.persist_noclobber(path).map_err(|e| e.error)?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(info(key, &record))
    }

    /// Begin unpublished staging. The writer is owned by the caller until commit
    /// or cancellation/drop; quota is rechecked at publish under the shared gate.
    pub fn create_file(&self, name: String) -> anyhow::Result<MediaFileWriter> {
        validate_name(&name)?;
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        fs::create_dir_all(&self.root)?;
        ensure!(
            self.keys()?.len() < MAX_CLIPS,
            "prepared clip library is full (64 clips)"
        );
        MediaFileWriter::new(&self.root, name)
    }

    pub fn publish_file(&self, writer: MediaFileWriter) -> anyhow::Result<PreparedClipInfo> {
        self.publish_file_at(&format!("{:032x}", rand::random::<u128>()), writer)
    }

    #[cfg(test)]
    pub(crate) fn publish_file_observed(
        &self,
        writer: MediaFileWriter,
        created: impl FnOnce(&str, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<PreparedClipInfo> {
        self.publish_file_observed_in(writer, None, created)
    }
    pub(crate) fn publish_file_observed_in(
        &self,
        writer: MediaFileWriter,
        stage: Option<&Path>,
        created: impl FnOnce(&str, &fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<PreparedClipInfo> {
        let key = format!("{:032x}", rand::random::<u128>());
        self.publish_file_at_observed(&key, writer, stage, |file| created(&key, file))
    }
    /// Stable upload key makes a lost publication reply recoverable without
    /// producing another asset. Existing files are never replaced.
    pub(crate) fn publish_file_at(
        &self,
        key: &str,
        writer: MediaFileWriter,
    ) -> anyhow::Result<PreparedClipInfo> {
        self.publish_file_at_observed(key, writer, None, |_| Ok(()))
    }
    fn publish_file_at_observed(
        &self,
        key: &str,
        writer: MediaFileWriter,
        stage: Option<&Path>,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<PreparedClipInfo> {
        ensure!(key != builtin_media::KEY, "reserved builtin media key");
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        ensure!(
            self.keys()?.len() < MAX_CLIPS,
            "prepared clip library is full (64 clips)"
        );
        let path = self.path(key)?.with_extension("lcm");
        // Refuse a namespace collision with either record format.
        ensure!(
            fs::symlink_metadata(self.path(key)?)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "media key collision"
        );
        writer.finish_observed_in(&path, stage, created)?;
        Ok(file_info(key.to_owned(), &MediaFile::open(&path)?))
    }

    /// Recover a commit receipt, including after restart or an ambiguous fsync
    /// error. Confirm durability before reporting publication success again.
    pub(crate) fn published_file(&self, key: &str) -> anyhow::Result<Option<PreparedClipInfo>> {
        ensure!(key != builtin_media::KEY, "reserved builtin media key");
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        let legacy = self.path(key)?;
        match fs::symlink_metadata(&legacy) {
            Ok(_) => anyhow::bail!("media key collision"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let path = legacy.with_extension("lcm");
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(_) => {}
        }
        let file = MediaFile::open(&path)?;
        fs::File::open(&path)?.sync_all()?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(Some(file_info(key.to_owned(), &file)))
    }

    /// Open either format and pin its lifetime atomically with respect to delete.
    /// Metadata/first-frame validation is immediate; indexed later frames are
    /// revalidated on read. The file descriptor never escapes its pin owner.
    pub fn acquire_source(&self, key: &str) -> anyhow::Result<PinnedMedia> {
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        let (source, name) = if key == builtin_media::KEY {
            builtin_media::read_unit(0)?;
            (MediaSource::Builtin, builtin_media::info().name)
        } else {
            let (path, indexed) = self.resolve(key)?;
            if indexed {
                let file = MediaFile::open(&path)?;
                let name = file.name().to_owned();
                (MediaSource::Indexed(file), name)
            } else {
                let record = self.read(key)?;
                (
                    MediaSource::Legacy(std::sync::Arc::new(record.clip)),
                    record.name,
                )
            }
        };
        let lease = gate.get(key).and_then(Weak::upgrade).unwrap_or_else(|| {
            let lease = Arc::new(());
            gate.insert(key.to_owned(), Arc::downgrade(&lease));
            lease
        });
        Ok(PinnedMedia {
            name,
            source,
            pin: MediaPin {
                _leases: vec![lease],
            },
        })
    }

    /// List validated metadata, without silently hiding corrupt saved records.
    pub fn list(&self) -> anyhow::Result<Vec<PreparedClipInfo>> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        let mut entries = vec![builtin_media::info()];
        let user_entries: anyhow::Result<Vec<_>> = self
            .keys()?
            .into_iter()
            .filter(|key| !self.pending.hidden(key))
            .map(|key| {
                let (path, indexed) = self.resolve(&key)?;
                if indexed {
                    Ok(file_info(
                        key.clone(),
                        &MediaFile::open(&path).with_context(|| format!("prepared clip {key}"))?,
                    ))
                } else {
                    Ok(info(
                        key.clone(),
                        &self
                            .read(&key)
                            .with_context(|| format!("prepared clip {key}"))?,
                    ))
                }
            })
            .collect();
        entries.extend(user_entries?);
        Ok(entries)
    }

    pub(crate) fn with_catalog<T>(
        &self,
        action: impl FnOnce(Vec<String>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        let mut keys = self.keys()?;
        keys.retain(|key| !self.pending.hidden(key));
        keys.push(builtin_media::KEY.to_owned());
        action(keys)
    }

    /// Revalidate disk contents on every load; no paths from clients are accepted.
    pub fn load(&self, key: &str) -> anyhow::Result<H264Clip> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        Ok(self.read(key)?.clip)
    }

    /// Load and pin atomically with respect to deletion. Keep the returned pin
    /// until all playback and pending persistence work using this key has ended.
    /// Plain `load` is for transient reads that never publish a saved reference.
    pub fn acquire(&self, key: &str) -> anyhow::Result<(H264Clip, MediaPin)> {
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        let clip = self.read(key)?.clip;
        let lease = gate.get(key).and_then(Weak::upgrade).unwrap_or_else(|| {
            let lease = Arc::new(());
            gate.insert(key.to_owned(), Arc::downgrade(&lease));
            lease
        });
        Ok((
            clip,
            MediaPin {
                _leases: vec![lease],
            },
        ))
    }

    /// Delete only after the last live pin and every durable reference are gone.
    /// `is_referenced` runs under the store gate and must scan current persisted
    /// references strictly: return an error when they cannot be read. It must not
    /// re-enter this store. Every writer of a new reference must hold a pin from
    /// `acquire` through its durable commit. Stores for one root must be clones
    /// of the same instance; independent instances do not share lifetime guards.
    ///
    /// An fsync failure after unlink is an ambiguous result; read back the list
    /// before retrying. No other record is removed, even if this one is corrupt.
    pub fn delete_unreferenced(
        &self,
        key: &str,
        is_referenced: impl FnOnce() -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        ensure!(
            key != builtin_media::KEY,
            "builtin videos cannot be deleted"
        );
        self.path(key)?;
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        ensure!(
            gate.get(key).and_then(Weak::upgrade).is_none(),
            "clip is still in use"
        );
        ensure!(!is_referenced()?, "clip is referenced by saved settings");
        let (path, _) = self.resolve(key)?;
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "clip must be a regular file"
        );
        fs::remove_file(path)?;
        gate.remove(key);
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    pub(crate) fn rollback_pending(
        &self,
        key: &str,
        token: u128,
        identity: crate::asset_publication::FileIdentity,
        referenced: impl FnOnce() -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("media lock poisoned"))?;
        if !self.pending.owns(key, token) {
            return Ok(());
        }
        crate::asset_publication::check_root(&self.root)?;
        if gate.get(key).and_then(Weak::upgrade).is_some() || referenced()? {
            return Ok(());
        }
        let path = self.path(key)?.with_extension("lcm");
        // Our publisher only creates indexed .lcm files; never touch .json.
        match identity.matches(&path)? {
            Some(false) => return Ok(()),
            Some(true) => fs::remove_file(path)?,
            None => {}
        }
        fs::File::open(&self.root)?.sync_all()?;
        gate.remove(key);
        Ok(())
    }

    fn path(&self, key: &str) -> anyhow::Result<PathBuf> {
        ensure!(
            key.len() == 32
                && key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid prepared clip key"
        );
        Ok(self.root.join(format!("{key}.json")))
    }
    fn resolve(&self, key: &str) -> anyhow::Result<(PathBuf, bool)> {
        let legacy = self.path(key)?;
        let indexed = legacy.with_extension("lcm");
        let exists = |path: &PathBuf| -> anyhow::Result<bool> {
            match fs::symlink_metadata(path) {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e.into()),
            }
        };
        match (exists(&legacy)?, exists(&indexed)?) {
            (true, false) => Ok((legacy, false)),
            (false, true) => Ok((indexed, true)),
            (true, true) => anyhow::bail!("ambiguous media key: both v1 and v2 records exist"),
            (false, false) => anyhow::bail!("prepared clip not found"),
        }
    }
    fn keys(&self) -> anyhow::Result<Vec<String>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut keys = Vec::new();
        for (index, entry) in entries.enumerate() {
            ensure!(
                index < MAX_DIRECTORY_ENTRIES,
                "media directory entry limit exceeded"
            );
            let name = entry?.file_name();
            let Some(name) = name.to_str() else { continue };
            if let Some(key) = name
                .strip_suffix(".json")
                .or_else(|| name.strip_suffix(".lcm"))
            {
                if key != builtin_media::KEY && self.path(key).is_ok() {
                    keys.push(key.to_owned());
                    ensure!(
                        keys.len() <= MAX_CLIPS,
                        "prepared clip library exceeds limit"
                    );
                }
            }
        }
        keys.sort();
        ensure!(
            !keys.windows(2).any(|pair| pair[0] == pair[1]),
            "ambiguous media key: both v1 and v2 records exist"
        );
        Ok(keys)
    }
    fn read(&self, key: &str) -> anyhow::Result<Record> {
        ensure!(
            key != builtin_media::KEY,
            "builtin media requires a streaming reader"
        );
        let (path, indexed) = self.resolve(key)?;
        ensure!(!indexed, "indexed media requires a streaming reader");
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "clip must be a regular file"
        );
        ensure!(
            metadata.len() <= MAX_RECORD_BYTES,
            "clip record exceeds size limit"
        );
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_RECORD_BYTES,
            "clip record exceeds size limit"
        );
        let record: Record = serde_json::from_slice(&bytes)?;
        ensure!(record.version == 1, "unsupported prepared clip version");
        validate_name(&record.name)?;
        validate_h264_clip(&record.clip, 1)?;
        Ok(record)
    }
}
fn validate_name(name: &str) -> anyhow::Result<()> {
    ensure!(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "clip name must be nonblank, at most256 UTF-8 bytes and contain no control characters"
    );
    Ok(())
}
fn info(key: String, record: &Record) -> PreparedClipInfo {
    PreparedClipInfo {
        is_default: false,
        key,
        name: record.name.clone(),
        fps: record.clip.fps,
        frames: record.clip.units.len() as u32,
        encoded_bytes: record.clip.units.iter().map(|u| u.bytes.len() as u64).sum(),
    }
}

fn file_info(key: String, file: &MediaFile) -> PreparedClipInfo {
    PreparedClipInfo {
        is_default: false,
        key,
        name: file.name().to_owned(),
        fps: file.fps(),
        frames: file.frames() as u32,
        encoded_bytes: file.encoded_bytes(),
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::*;

    #[test]
    fn publication_cannot_overwrite_or_recover_the_reserved_builtin_key() {
        let temp = tempfile::tempdir().unwrap();
        let store = MediaAssetStore::new(temp.path().into());
        let mut writer = store.create_file("Replacement".into()).unwrap();
        writer
            .append(&builtin_media::read_unit(0).unwrap())
            .unwrap();
        assert!(store.publish_file_at(builtin_media::KEY, writer).is_err());
        assert!(store.published_file(builtin_media::KEY).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        assert_eq!(store.list().unwrap(), [builtin_media::info()]);
    }
}
