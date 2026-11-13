//! Source-verified GIF/MP4 preparation with joined cancellation and private owners.
use crate::{screen::ScreenInspection, screen_assets::read_verified, Error};
use llcontrol_media::media_prepare::{prepare_file_budgeted, Fit, PreparedFile};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Serialize)]
pub struct PreparedMediaItem {
    pub source_entry: String,
    pub source_sha256: String,
    pub encoded_sha256: String,
    pub fps: u8,
    pub frames: usize,
    pub encoded_bytes: u64,
    #[serde(skip)]
    pub file: PreparedFile,
}
#[derive(Serialize)]
pub struct PreparedMedia {
    pub items: Vec<PreparedMediaItem>,
    pub deferred_entries: Vec<String>,
    pub assets_published: bool,
    pub complete_screen_group_supported: bool,
}
pub(crate) struct Cancellable<'a, R> {
    pub(crate) reader: R,
    pub(crate) cancel: &'a AtomicBool,
}
impl<R> Cancellable<'_, R> {
    fn check(&self) -> std::io::Result<()> {
        if self.cancel.load(Ordering::Acquire) {
            Err(std::io::Error::other("Screen media preparation cancelled"))
        } else {
            Ok(())
        }
    }
}
impl<R: Read> Read for Cancellable<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.check()?;
        self.reader.read(bytes)
    }
}
impl<R: Seek> Seek for Cancellable<'_, R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.check()?;
        self.reader.seek(position)
    }
}
/// Blocking API. Cancellation remains owned until every media-tool child is reaped.
pub fn inspect_and_prepare<R: Read + Seek>(
    reader: R,
    record: usize,
    cancel: &AtomicBool,
) -> Result<(ScreenInspection, PreparedMedia), Error> {
    let mut reader = Cancellable { reader, cancel };
    let source = crate::screen::inspect_windows_screen(&mut reader, record)?;
    let prepared = prepare_media(reader, &source, cancel)?;
    Ok((source, prepared))
}
pub fn prepare_media<R: Read + Seek>(
    reader: R,
    source: &ScreenInspection,
    cancel: &AtomicBool,
) -> Result<PreparedMedia, Error> {
    let mut entries = BTreeSet::new();
    let mut deferred_entries = Vec::new();
    for payload in &source.payloads {
        let ext = payload
            .entry
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(ext.as_str(), "gif" | "mp4") {
            entries.insert(payload.entry.clone());
        } else {
            deferred_entries.push(payload.entry.clone());
        }
    }
    if entries.len() > 64 {
        return Err(Error::Invalid("Screen media batch exceeds64 entries"));
    }
    let mut total = 0u64;
    let mut frames = 0usize;
    let items = read_verified(
        Cancellable { reader, cancel },
        source,
        &entries,
        256 * 1024 * 1024,
        |payload, bytes| {
            if cancel.load(Ordering::Acquire) {
                return Err(Error::Invalid("Screen media preparation cancelled"));
            }
            let directory = tempfile::tempdir()?;
            let extension = if payload.entry.to_ascii_lowercase().ends_with(".gif") {
                "gif"
            } else {
                "mp4"
            };
            let path = directory.path().join(format!("source.{extension}"));
            std::fs::write(&path, bytes)?;
            // Windows Image runtime stretches the source into the full square canvas.
            let remaining = llcontrol_ipc::MAX_MEDIA_UPLOAD_FRAMES.saturating_sub(frames);
            let remaining_bytes = (512 * 1024 * 1024u64)
                .saturating_sub(total)
                .min(llcontrol_ipc::MAX_MEDIA_UPLOAD_BYTES);
            let mut file = prepare_file_budgeted(
                &path,
                Fit::Stretch,
                remaining,
                remaining_bytes,
                cancel,
                |_| {},
            )?;
            frames = frames
                .checked_add(file.frames())
                .filter(|count| *count <= llcontrol_ipc::MAX_MEDIA_UPLOAD_FRAMES)
                .ok_or(Error::Invalid(
                    "Screen media batch exceeds one-hour frame budget",
                ))?;
            total = total
                .checked_add(file.encoded_bytes())
                .filter(|size| *size <= 512 * 1024 * 1024)
                .ok_or(Error::Invalid(
                    "prepared Screen media exceeds512MiB staging budget",
                ))?;
            let mut digest = Sha256::new();
            for index in 0..file.frames() {
                if cancel.load(Ordering::Acquire) {
                    return Err(Error::Invalid("Screen media preparation cancelled"));
                }
                digest.update(file.read_unit(index)?.bytes);
            }
            Ok(PreparedMediaItem {
                source_entry: payload.entry.clone(),
                source_sha256: payload.sha256.clone(),
                encoded_sha256: format!("{:x}", digest.finalize()),
                fps: 24,
                frames: file.frames(),
                encoded_bytes: file.encoded_bytes(),
                file,
            })
        },
    )?;
    Ok(PreparedMedia {
        items,
        deferred_entries,
        assets_published: false,
        complete_screen_group_supported: false,
    })
}
