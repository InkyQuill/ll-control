//! Indexed persistent media container. All operations are blocking: callers must
//! own the reader/writer on a blocking worker, never a Tokio or HID actor thread.
//! Publication is private staging -> validated container -> atomic no-clobber.
use anyhow::{ensure, Context};
use lconnect_ipc::{validate_h264_access_unit, H264AccessUnit};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::FileExt,
    path::Path,
};

const MAGIC: &[u8; 8] = b"LCMEDIA2";
const MAX_INDEX_BYTES: usize = 8 * 1024 * 1024;
const MAX_UNIT_BYTES: u32 = 128 * 1024;
/// Resource budgets for persistent media, not claims about Windows limits.
pub const MAX_MEDIA_BYTES: u64 = lconnect_ipc::MAX_MEDIA_UPLOAD_BYTES;
pub const MAX_MEDIA_FRAMES: usize = lconnect_ipc::MAX_MEDIA_UPLOAD_FRAMES;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    offset: u64,
    length: u32,
    keyframe: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u8,
    name: String,
    fps: u8,
    encoded_bytes: u64,
    units: Vec<Entry>,
}
impl Index {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 2, "unsupported media file version");
        ensure!(self.fps == 24, "persistent media requires 24 fps");
        ensure!(
            !self.name.trim().is_empty()
                && self.name.len() <= 256
                && !self.name.chars().any(char::is_control),
            "invalid media name"
        );
        ensure!(
            !self.units.is_empty() && self.units.len() <= MAX_MEDIA_FRAMES,
            "media frame count outside resource budget"
        );
        ensure!(
            self.encoded_bytes <= MAX_MEDIA_BYTES,
            "media payload exceeds resource budget"
        );
        ensure!(self.units[0].keyframe, "media must begin with a keyframe");
        let mut offset = 0u64;
        for unit in &self.units {
            ensure!(
                (1..=MAX_UNIT_BYTES).contains(&unit.length),
                "media access unit outside byte budget"
            );
            ensure!(
                unit.offset == offset,
                "media index contains a gap or overlap"
            );
            offset = offset
                .checked_add(u64::from(unit.length))
                .context("media index overflow")?;
            ensure!(offset <= self.encoded_bytes, "media index exceeds payload");
        }
        ensure!(
            offset == self.encoded_bytes,
            "media index does not cover payload"
        );
        Ok(())
    }
}

/// At most one AU plus the bounded index lives in memory. Dropping the writer
/// discards its staging file; failed writes poison it so partial data cannot publish.
pub struct MediaFileWriter {
    payload: fs::File,
    index: Index,
    failed: bool,
}
impl MediaFileWriter {
    pub fn new(directory: &Path, name: String) -> anyhow::Result<Self> {
        ensure!(
            !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
            "invalid media name"
        );
        // Anonymous payloads vanish even on process death. Recover abandoned
        // publication copies while holding the same lock used by live finish.
        let _guard = publication_guard(directory)?;
        Ok(Self {
            payload: tempfile::tempfile_in(directory)?,
            index: Index {
                version: 2,
                name,
                fps: 24,
                encoded_bytes: 0,
                units: Vec::new(),
            },
            failed: false,
        })
    }
    pub fn append(&mut self, unit: &H264AccessUnit) -> anyhow::Result<()> {
        ensure!(!self.failed, "media writer failed");
        ensure!(
            self.index.units.len() < MAX_MEDIA_FRAMES,
            "media frame count exceeds resource budget"
        );
        validate_h264_access_unit(unit, self.index.units.is_empty())?;
        let length = u32::try_from(unit.bytes.len())?;
        let total = self
            .index
            .encoded_bytes
            .checked_add(u64::from(length))
            .context("media size overflow")?;
        ensure!(
            total <= MAX_MEDIA_BYTES,
            "media payload exceeds resource budget"
        );
        self.failed = true;
        self.payload.write_all(&unit.bytes)?;
        self.index.units.push(Entry {
            offset: self.index.encoded_bytes,
            length,
            keyframe: unit.keyframe,
        });
        self.index.encoded_bytes = total;
        self.failed = false;
        Ok(())
    }
    /// Consume on commit; never replace an existing asset. The caller owns its
    /// cancellation barrier through this operation and must read back an ambiguous
    /// fsync result rather than blindly re-publishing.
    pub fn finish(self, destination: &Path) -> anyhow::Result<()> {
        self.finish_observed(destination, |_| Ok(()))
    }
    pub(crate) fn finish_observed(
        self,
        destination: &Path,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.finish_observed_in(destination, None, created)
    }
    pub(crate) fn finish_observed_in(
        mut self,
        destination: &Path,
        stage: Option<&Path>,
        created: impl FnOnce(&fs::File) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        ensure!(!self.failed, "media writer failed");
        self.index.validate()?;
        let header = serde_json::to_vec(&self.index)?;
        ensure!(header.len() <= MAX_INDEX_BYTES, "media index too large");
        let directory = destination
            .parent()
            .context("media destination has no parent")?;
        let _guard = publication_guard(directory)?;
        let mut output = tempfile::Builder::new()
            .prefix(".lconnect-media-publish-")
            .suffix(".tmp")
            .tempfile_in(stage.unwrap_or(directory))?;
        output.write_all(MAGIC)?;
        output.write_all(&u32::try_from(header.len())?.to_le_bytes())?;
        output.write_all(&header)?;
        self.payload.seek(SeekFrom::Start(0))?;
        let copied = std::io::copy(
            &mut Read::by_ref(&mut self.payload).take(self.index.encoded_bytes + 1),
            &mut output,
        )?;
        ensure!(
            copied == self.index.encoded_bytes,
            "staged media length changed"
        );
        output.as_file().sync_all()?;
        created(output.as_file())?;
        output.persist_noclobber(destination).map_err(|e| e.error)?;
        fs::File::open(directory)?.sync_all()?;
        Ok(())
    }
}

/// A directory advisory lock protects publication staging across processes.
/// Never remove the locked directory or a live writer's named temporary file.
struct PublicationGuard(fs::File);
impl Drop for PublicationGuard {
    fn drop(&mut self) {
        // flock belongs to the open file description, which a concurrent fork
        // can briefly inherit before CLOEXEC closes it. Closing only our fd
        // would leave the lock alive in that unrelated child until exec.
        let _ = rustix::fs::flock(&self.0, rustix::fs::FlockOperation::Unlock);
    }
}
fn publication_guard(directory: &Path) -> anyhow::Result<PublicationGuard> {
    let guard = fs::File::open(directory)?;
    rustix::fs::flock(&guard, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .context("another media publication is active")?;
    let guard = PublicationGuard(guard);
    for (index, entry) in fs::read_dir(directory)?.take(1025).enumerate() {
        ensure!(index < 1024, "media staging scan exceeds directory budget");
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(token) = name
            .strip_prefix(".lconnect-media-publish-")
            .and_then(|s| s.strip_suffix(".tmp"))
        else {
            continue;
        };
        if token.len() == 6
            && token.bytes().all(|b| b.is_ascii_alphanumeric())
            && entry.file_type()?.is_file()
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(guard)
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    #[test]
    fn completed_guard_unlocks_even_when_another_descriptor_survives() {
        let root = tempfile::tempdir().unwrap();
        let guard = publication_guard(root.path()).unwrap();
        let inherited = guard.0.try_clone().unwrap();
        assert!(publication_guard(root.path()).is_err());
        drop(guard);
        let next = publication_guard(root.path()).unwrap();
        drop(next);
        drop(inherited);
    }
}

/// Owns one open descriptor and bounded index, never the whole compressed stream.
/// Each read revalidates AU framing; external corruption fails before that AU is
/// handed to the caller. This is not full H.264 decoder conformance validation.
#[derive(Debug)]
pub struct MediaFile {
    file: fs::File,
    index: Index,
    payload_start: u64,
}
impl MediaFile {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "media must be a regular file"
        );
        let mut file = fs::File::open(path)?;
        let mut prefix = [0; 12];
        file.read_exact(&mut prefix)?;
        ensure!(&prefix[..8] == MAGIC, "invalid media magic");
        let length = u32::from_le_bytes(prefix[8..12].try_into()?) as usize;
        ensure!(
            (1..=MAX_INDEX_BYTES).contains(&length),
            "media index size outside resource budget"
        );
        let payload_start = 12 + length as u64;
        ensure!(
            file.metadata()?.len() >= payload_start,
            "truncated media index"
        );
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)?;
        let index: Index = serde_json::from_slice(&bytes)?;
        index.validate()?;
        ensure!(
            file.metadata()?.len() == payload_start + index.encoded_bytes,
            "media payload length mismatch"
        );
        let mut media = Self {
            file,
            index,
            payload_start,
        };
        media.read_unit(0)?;
        Ok(media)
    }
    pub(crate) fn try_clone(&self) -> anyhow::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            index: self.index.clone(),
            payload_start: self.payload_start,
        })
    }
    pub(crate) fn keyframe_before(&self, frame: usize) -> anyhow::Result<usize> {
        self.index
            .units
            .get(..=frame)
            .context("media frame outside index")?
            .iter()
            .rposition(|unit| unit.keyframe)
            .context("media has no preceding keyframe")
    }
    pub fn name(&self) -> &str {
        &self.index.name
    }
    pub fn fps(&self) -> u8 {
        self.index.fps
    }
    pub fn frames(&self) -> usize {
        self.index.units.len()
    }
    pub fn encoded_bytes(&self) -> u64 {
        self.index.encoded_bytes
    }
    pub fn read_unit(&mut self, frame: usize) -> anyhow::Result<H264AccessUnit> {
        let entry = self
            .index
            .units
            .get(frame)
            .context("media frame outside index")?;
        let mut unit = H264AccessUnit {
            bytes: vec![0; entry.length as usize],
            keyframe: entry.keyframe,
        };
        self.file
            .read_exact_at(&mut unit.bytes, self.payload_start + entry.offset)?;
        validate_h264_access_unit(&unit, frame == 0)?;
        Ok(unit)
    }
}
