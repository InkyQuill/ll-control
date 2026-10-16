//! One desktop import at a time, from bounded upload to preparation/publication.
use crate::media_prepare::{self, Fit, Stage, VideoCut};
use crate::video_adjustments::VideoAdjustments;
use anyhow::{bail, Context, Result};
use lconnect_ipc::{MediaUploadPhase, MediaUploadRequest, MediaUploadState};
use serde::Serialize;
use std::{
    fs::File,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

/// Maximum bytes carried by one webview request.
pub const MAX_CHUNK: usize = 256 * 1024;
const MAX_INPUT: u64 = 256 * 1024 * 1024;

/// Current stage; publishing cannot safely be cancelled after dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Uploading,
    Inspecting,
    SourceReady,
    Encoding,
    Validating,
    Remuxing,
    ReviewReady,
    Cancelling,
    Transferring,
    Publishing,
    Complete,
    Cancelled,
    Failed,
}
impl Phase {
    fn terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Cancelled | Self::Failed)
    }
}

/// Metadata only: source bytes and prepared clip never cross back into the UI.
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub id: String,
    pub name: String,
    pub total: u64,
    pub received: u64,
    pub phase: Phase,
    pub key: Option<String>,
    pub message: Option<String>,
    pub metadata: Option<media_prepare::SourceMetadata>,
}
/// Preparation parameters, fixed while a worker owns the input.
pub struct EditOptions {
    pub range: Option<crate::video_range::VideoEditRange>,
    pub fit: Fit,
    pub cut: Option<VideoCut>,
    pub adjustments: VideoAdjustments,
}

impl EditOptions {
    fn validate(&self) -> Result<()> {
        if self.range.is_some() && self.cut.is_some() {
            bail!("VideoEdit range and VideoCut segments are mutually exclusive");
        }
        if let Some(range) = self.range {
            range.validate()?;
        }
        if let Some(cut) = &self.cut {
            cut.validate()?;
        }
        Ok(())
    }
}
fn prepare_options(
    source: &std::path::Path,
    fit: Fit,
    cut: Option<VideoCut>,
    adjustments: VideoAdjustments,
    range: Option<crate::video_range::VideoEditRange>,
    cancel: &AtomicBool,
    progress: impl FnMut(Stage),
) -> Result<media_prepare::PreparedFile> {
    if let Some(range) = range {
        if cut.is_some() {
            bail!("VideoEdit range and VideoCut segments are mutually exclusive");
        }
        media_prepare::prepare_video_edit(source, fit, range, adjustments, cancel, progress)
    } else {
        media_prepare::prepare_file_edited(source, fit, cut, adjustments, cancel, progress)
    }
}

enum SourceLease {
    Gallery(media_prepare::PreviewLease),
    Recording(Arc<crate::screen_record::RecordingFile>),
}
impl SourceLease {
    fn path(&self) -> std::path::PathBuf {
        match self {
            Self::Gallery(source) => source.path(),
            Self::Recording(source) => source.path(),
        }
    }
}

enum InputStorage {
    Uploaded {
        directory: tempfile::TempDir,
        file: Option<File>,
        extension: String,
    },
    Leased(SourceLease),
}
struct Upload {
    storage: InputStorage,
    options: EditOptions,
}
impl Upload {
    fn path(&self) -> std::path::PathBuf {
        match &self.storage {
            InputStorage::Uploaded {
                directory,
                extension,
                ..
            } => directory.path().join(format!("source.{extension}")),
            InputStorage::Leased(source) => source.path(),
        }
    }
    fn close_writer(&mut self) {
        if let InputStorage::Uploaded { file, .. } = &mut self.storage {
            drop(file.take());
        }
    }
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        match &mut self.storage {
            InputStorage::Uploaded { file, .. } => file
                .as_mut()
                .context("Upload writer closed")?
                .write_all(bytes)
                .context("Cannot write upload"),
            InputStorage::Leased(_) => bail!("Leased source is immutable"),
        }
    }
}
struct Job {
    status: Status,
    cancel: Arc<AtomicBool>,
    upload: Option<Upload>,
    review: Option<ReviewMedia>,
}
struct ReviewMedia {
    media: PreparedMedia,
    preview: media_prepare::PreviewFile,
}

/// Cloneable handle. An active job remains admitted until its worker is reaped.
#[derive(Clone, Default)]
pub struct MediaImports(Arc<Mutex<Option<Job>>>, Arc<AtomicBool>);

/// Validated clip awaiting one daemon publication, with the library display name.
pub struct PreparedMedia {
    pub name: String,
    pub file: media_prepare::PreparedFile,
}

impl MediaImports {
    fn lock(&self) -> Result<MutexGuard<'_, Option<Job>>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Media import state is unavailable"))
    }
    /// Read the latest job, including terminal outcome, to recover after UI reload.
    pub fn status(&self) -> Result<Option<Status>> {
        Ok(self.lock()?.as_ref().map(|j| j.status.clone()))
    }
    /// Begin a new upload. Filesystem work belongs on a blocking worker.
    pub fn begin(&self, id: String, name: String, total: u64, fit: Fit) -> Result<Status> {
        self.begin_cut(id, name, total, fit, None)
    }
    /// Begin an upload with an optional ordered VideoCut selection.
    pub fn begin_cut(
        &self,
        id: String,
        name: String,
        total: u64,
        fit: Fit,
        cut: Option<VideoCut>,
    ) -> Result<Status> {
        self.begin_edited(
            id,
            name,
            total,
            EditOptions {
                range: None,
                fit,
                cut,
                adjustments: VideoAdjustments::default(),
            },
        )
    }
    /// Begin a new upload with fixed channel effects and optional intervals.
    pub fn begin_edited(
        &self,
        id: String,
        name: String,
        total: u64,
        options: EditOptions,
    ) -> Result<Status> {
        self.begin_input(id, name, total, options, None)
    }
    /// Re-edit a leased gallery source entirely in native code. Must run on a
    /// blocking worker. Source revocation cannot remove an admitted input.
    pub fn begin_source(
        &self,
        sources: &crate::media_source::MediaSources,
        source_id: &str,
        id: String,
        name: String,
        fit: Fit,
        cut: Option<VideoCut>,
    ) -> Result<Status> {
        self.begin_source_edited(
            sources,
            source_id,
            id,
            name,
            EditOptions {
                range: None,
                fit,
                cut,
                adjustments: VideoAdjustments::default(),
            },
        )
    }
    /// Admit a leased gallery source with the same effects as a fresh upload.
    pub fn begin_source_edited(
        &self,
        sources: &crate::media_source::MediaSources,
        source_id: &str,
        id: String,
        name: String,
        options: EditOptions,
    ) -> Result<Status> {
        let source = sources.lease(source_id)?;
        self.begin_input(
            id,
            name,
            source.length,
            options,
            Some(SourceLease::Gallery(source)),
        )
    }
    /// Synchronously reserve a finalized recording before preparation. No disk
    /// I/O or encoding is performed; cancellation can target this admitted ID.
    /// Capture cancellation cannot revoke an already-admitted input.
    pub fn begin_recording(
        &self,
        recordings: &crate::screen_record_jobs::Recordings,
        recording_id: &str,
        id: String,
        name: String,
        options: EditOptions,
    ) -> Result<Status> {
        if !name.to_ascii_lowercase().ends_with(".mp4") {
            bail!("A recording must use an MP4 name");
        }
        let source = recordings.lease(recording_id)?;
        let total = source.length();
        self.begin_input(
            id,
            name,
            total,
            options,
            Some(SourceLease::Recording(source)),
        )
    }
    fn begin_input(
        &self,
        id: String,
        name: String,
        total: u64,
        options: EditOptions,
        source: Option<SourceLease>,
    ) -> Result<Status> {
        options.validate()?;
        if id.len() != 32
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("Invalid media import ID");
        }
        if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            bail!("Media name must contain 1..256 UTF-8 bytes and no control characters");
        }
        let maximum = if source.is_some() {
            media_prepare::MAX_STAGED_SOURCE
        } else {
            MAX_INPUT
        };
        if !(1..=maximum).contains(&total) {
            bail!("Media file must contain 1 byte..256 MiB");
        }
        let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "gif" | "mp4") {
            bail!("Select a GIF or MP4 file");
        }
        let mut current = self.lock()?;
        if self.1.load(Ordering::Acquire) {
            bail!("Application is closing");
        }
        if let Some(job) = current.as_ref() {
            if job.status.id == id {
                bail!("This import ID has already been used; inspect its status");
            }
            if !job.status.phase.terminal() {
                bail!("Another media import is still active");
            }
        }
        let received = if source.is_some() { total } else { 0 };
        let storage = match source {
            Some(source) => InputStorage::Leased(source),
            None => {
                let directory = tempfile::tempdir()?;
                let file = Some(File::create(
                    directory.path().join(format!("source.{extension}")),
                )?);
                InputStorage::Uploaded {
                    directory,
                    file,
                    extension,
                }
            }
        };
        let status = Status {
            id,
            name,
            total,
            received,
            phase: Phase::Uploading,
            key: None,
            message: None,
            metadata: None,
        };
        *current = Some(Job {
            status: status.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            review: None,
            upload: Some(Upload { storage, options }),
        });
        Ok(status)
    }
    /// Append exactly the next chunk; ambiguous retry cannot silently duplicate bytes.
    pub fn append(&self, id: &str, offset: u64, bytes: &[u8]) -> Result<Status> {
        if bytes.is_empty() || bytes.len() > MAX_CHUNK {
            bail!("Invalid upload chunk size");
        }
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.status.phase != Phase::Uploading || offset != job.status.received {
            bail!("Upload is not at the expected offset; inspect its status");
        }
        let end = offset
            .checked_add(bytes.len() as u64)
            .context("Upload size overflow")?;
        if end > job.status.total {
            bail!("Chunk exceeds declared file size");
        }
        let upload = job.upload.as_mut().context("Upload no longer available")?;
        if let Err(error) = upload.append(bytes) {
            job.status.phase = Phase::Failed;
            job.status.message = Some(error.to_string());
            job.upload = None;
            return Err(error);
        }
        job.status.received = end;
        Ok(job.status.clone())
    }
    /// Cancel upload immediately, or request cancellation and retain preparation admission.
    pub fn cancel(&self, id: &str) -> Result<Status> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.status.phase == Phase::Publishing {
            bail!("Publication has started; inspect its result before retrying");
        }
        if !job.status.phase.terminal() {
            job.cancel.store(true, Ordering::Release);
            job.status.metadata = None;
            if matches!(
                job.status.phase,
                Phase::Uploading | Phase::SourceReady | Phase::ReviewReady
            ) {
                job.upload = None;
                job.review = None;
                job.status.phase = Phase::Cancelled;
            } else {
                job.status.phase = Phase::Cancelling;
            }
        }
        Ok(job.status.clone())
    }
    /// Inspect a complete staged input without encoding or publication. Run in
    /// an owned task; cancellation retains admission until the child is reaped.
    pub async fn inspect(&self, id: &str) -> Result<Status> {
        self.inspect_with(id, media_prepare::inspect_file).await
    }
    async fn inspect_with<F>(&self, id: &str, inspect: F) -> Result<Status>
    where
        F: FnOnce(&std::path::Path, &AtomicBool) -> Result<media_prepare::SourceMetadata>
            + Send
            + 'static,
    {
        let owner = self.clone();
        let worker_id = id.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let (upload, cancel) = {
                let mut current = owner.lock()?;
                let job = find(&mut current, &worker_id)?;
                if job.status.phase == Phase::SourceReady {
                    return Ok(job.status.clone());
                }
                if job.status.phase != Phase::Uploading || job.status.received != job.status.total {
                    bail!("Upload is incomplete or inspection already started");
                }
                let upload = job.upload.take().context("Upload unavailable")?;
                job.status.phase = Phase::Inspecting;
                (upload, job.cancel.clone())
            };
            let result = inspect(&upload.path(), &cancel);
            let mut current = owner.lock()?;
            let job = find(&mut current, &worker_id)?;
            if cancel.load(Ordering::Acquire) {
                drop(upload);
                job.status.phase = Phase::Cancelled;
                job.status.metadata = None;
                return Ok(job.status.clone());
            }
            match result {
                Ok(metadata) => {
                    job.upload = Some(upload);
                    job.status.metadata = Some(metadata);
                    job.status.phase = Phase::SourceReady;
                    Ok(job.status.clone())
                }
                Err(error) => {
                    drop(upload);
                    job.status.phase = Phase::Failed;
                    job.status.message = Some(error.to_string());
                    Err(error)
                }
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(error) => {
                self.fail(id, error.to_string())?;
                Err(error.into())
            }
        }
    }
    /// Prepare, retaining admission if its requesting task is dropped.
    /// The caller must spawn this future in an owned task, not tie it to a window request.
    pub async fn prepare(&self, id: &str) -> Result<Option<PreparedMedia>> {
        self.prepare_with(id, None, prepare_options).await
    }
    async fn prepare_with<F>(
        &self,
        id: &str,
        replacement: Option<EditOptions>,
        prepare: F,
    ) -> Result<Option<PreparedMedia>>
    where
        F: FnOnce(
                &std::path::Path,
                Fit,
                Option<VideoCut>,
                VideoAdjustments,
                Option<crate::video_range::VideoEditRange>,
                &AtomicBool,
                Box<dyn FnMut(Stage) + Send>,
            ) -> Result<media_prepare::PreparedFile>
            + Send
            + 'static,
    {
        if let Some(cut) = replacement
            .as_ref()
            .and_then(|options| options.cut.as_ref())
        {
            cut.validate()?;
        }
        let owner = self.clone();
        let worker_id = id.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let (mut upload, cancel, name) = {
                let mut current = owner.lock()?;
                let job = find(&mut current, &worker_id)?;
                if !matches!(job.status.phase, Phase::Uploading | Phase::SourceReady)
                    || job.status.received != job.status.total
                {
                    bail!("Upload is incomplete or already started");
                }
                if let Some(options) = replacement {
                    if job.status.phase != Phase::SourceReady {
                        bail!("Source is not ready for editing");
                    }
                    if let Some(range) = options.range {
                        range.validate_source(
                            job.status
                                .metadata
                                .as_ref()
                                .context("Source metadata unavailable")?,
                        )?;
                    }
                    job.upload.as_mut().context("Upload unavailable")?.options = options;
                }
                let upload = job.upload.take().context("Upload unavailable")?;
                job.status.phase = Phase::Inspecting;
                job.status.metadata = None;
                (upload, job.cancel.clone(), job.status.name.clone())
            };
            let source = upload.path();
            upload.close_writer();
            let progress_owner = owner.clone();
            let progress_id = worker_id.clone();
            let callback: Box<dyn FnMut(Stage) + Send> = Box::new(move |stage| {
                if let Ok(mut current) = progress_owner.lock() {
                    if let Ok(job) = find(&mut current, &progress_id) {
                        if job.status.phase != Phase::Cancelling {
                            job.status.phase = match stage {
                                Stage::Inspecting => Phase::Inspecting,
                                Stage::Encoding => Phase::Encoding,
                                Stage::Validating => Phase::Validating,
                            };
                        }
                    }
                }
            });
            let result = prepare(
                &source,
                upload.options.fit,
                upload.options.cut,
                upload.options.adjustments,
                upload.options.range,
                &cancel,
                callback,
            );
            drop(upload.storage);
            // Resolve the cancellation/publication boundary under the same lock.
            let mut current = owner.lock()?;
            let job = find(&mut current, &worker_id)?;
            if job.cancel.load(Ordering::Acquire) {
                // A successful preparation owns its own staging directory.
                // Release it before exposing terminal status to begin/shutdown.
                drop(result);
                job.status.phase = Phase::Cancelled;
                return Ok(None);
            }
            match result {
                Ok(file) => {
                    job.status.phase = Phase::Transferring;
                    Ok(Some(PreparedMedia { name, file }))
                }
                Err(error) => {
                    job.status.phase = Phase::Failed;
                    job.status.message = Some(error.to_string());
                    Err(error)
                }
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(error) => {
                let owner = self.clone();
                let id = id.to_owned();
                let message = error.to_string();
                tokio::task::spawn_blocking(move || owner.fail(&id, message)).await??;
                Err(error.into())
            }
        }
    }
    /// Prepare and retain the exact encoded media for explicit review/publication.
    /// Run in an owned task, as with `prepare`; no daemon requests are made here.
    pub async fn prepare_review(&self, id: &str) -> Result<()> {
        let Some(media) = self.prepare(id).await? else {
            return Ok(());
        };
        self.review_prepared(id, media).await
    }
    /// Atomically admit inspected input with the editor's final options. A second
    /// request cannot overwrite options after worker admission, even if its reply is lost.
    pub async fn prepare_review_edited(&self, id: &str, options: EditOptions) -> Result<()> {
        let Some(media) = self
            .prepare_with(id, Some(options), prepare_options)
            .await?
        else {
            return Ok(());
        };
        self.review_prepared(id, media).await
    }
    async fn review_prepared(&self, id: &str, media: PreparedMedia) -> Result<()> {
        let owner = self.clone();
        let worker_id = id.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let cancel = {
                let mut current = owner.lock()?;
                let job = find(&mut current, &worker_id)?;
                if !job.cancel.load(Ordering::Acquire) {
                    job.status.phase = Phase::Remuxing;
                }
                job.cancel.clone()
            };
            let result = media.file.preview(&cancel);
            let mut current = owner.lock()?;
            let job = find(&mut current, &worker_id)?;
            if cancel.load(Ordering::Acquire) {
                drop(result);
                drop(media);
                job.status.phase = Phase::Cancelled;
                return Ok(());
            }
            match result {
                Ok(preview) => {
                    job.review = Some(ReviewMedia { media, preview });
                    job.status.phase = Phase::ReviewReady;
                    Ok(())
                }
                Err(error) => {
                    drop(media);
                    job.status.phase = Phase::Failed;
                    job.status.message = Some(error.to_string());
                    Err(error)
                }
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(error) => {
                self.fail(id, error.to_string())?;
                Err(error.into())
            }
        }
    }

    /// Preview metadata is only available for the currently reviewed job.
    pub fn preview_length(&self, id: &str) -> Result<u64> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.status.phase != Phase::ReviewReady {
            bail!("Preview is not ready");
        }
        Ok(job
            .review
            .as_ref()
            .context("Preview unavailable")?
            .preview
            .length())
    }

    /// Read one bounded range on a blocking worker. No arbitrary filesystem paths.
    pub fn preview_range(&self, id: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.status.phase != Phase::ReviewReady {
            bail!("Preview is not ready");
        }
        job.review
            .as_mut()
            .context("Preview unavailable")?
            .preview
            .read_range(offset, length)
    }

    /// Atomically consume the reviewed media once; the caller must publish it in
    /// an owned task. No re-encoding or source re-read occurs after confirmation.
    pub fn take_review(&self, id: &str) -> Result<PreparedMedia> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if self.1.load(Ordering::Acquire)
            || job.cancel.load(Ordering::Acquire)
            || job.status.phase != Phase::ReviewReady
        {
            bail!("Import is not awaiting review");
        }
        let review = job.review.take().context("Preview unavailable")?;
        drop(review.preview);
        job.status.phase = Phase::Transferring;
        Ok(review.media)
    }

    fn cancelled(&self, id: &str) -> Result<bool> {
        Ok(find(&mut *self.lock()?, id)?.cancel.load(Ordering::Acquire))
    }
    fn begin_commit(&self, id: &str) -> Result<bool> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        if job.status.phase != Phase::Transferring {
            bail!("Import is not transferring");
        }
        job.status.phase = Phase::Publishing;
        Ok(true)
    }

    /// Transfer a prepared file one AU at a time. The caller owns this future
    /// independently of the window request, through cleanup and terminal status.
    pub async fn publish<F, Fut>(
        &self,
        id: &str,
        prepared: PreparedMedia,
        mut call: F,
    ) -> Result<()>
    where
        F: FnMut(MediaUploadRequest) -> Fut,
        Fut: std::future::Future<Output = Result<MediaUploadState>>,
    {
        let frames = prepared.file.frames();
        let bytes = prepared.file.encoded_bytes();
        let mut file = Some(prepared.file);
        let mut dispatched = false;
        let outcome: Result<Option<String>> = async {
            if self.cancelled(id)? {
                return Ok(None);
            }
            dispatched = true;
            let state = upload_request(
                &mut call,
                id,
                MediaUploadRequest::Begin {
                    id: id.into(),
                    name: prepared.name.clone(),
                    frames,
                    encoded_bytes: bytes,
                },
            )
            .await?;
            validate_upload(&state, id, &prepared.name, frames, bytes, 0, 0)?;
            let mut transferred = 0;
            for index in 0..frames {
                if self.cancelled(id)? {
                    return Ok(None);
                }
                let mut reader = file.take().context("Prepared file unavailable")?;
                let (reader, unit) = tokio::task::spawn_blocking(move || {
                    let unit = reader.read_unit(index);
                    (reader, unit)
                })
                .await?;
                file = Some(reader);
                let unit = unit?;
                transferred += unit.bytes.len() as u64;
                let state = upload_request(
                    &mut call,
                    id,
                    MediaUploadRequest::Chunk {
                        id: id.into(),
                        index,
                        unit,
                    },
                )
                .await?;
                validate_upload(
                    &state,
                    id,
                    &prepared.name,
                    frames,
                    bytes,
                    index + 1,
                    transferred,
                )?;
            }
            if !self.begin_commit(id)? {
                return Ok(None);
            }
            let state =
                upload_request(&mut call, id, MediaUploadRequest::Finish { id: id.into() }).await?;
            published_key(&state, id, &prepared.name, frames, bytes).map(Some)
        }
        .await;
        // A failed/ambiguous request may have left staging, or committed despite
        // its lost reply. Cancel never deletes a published asset; inspect receipt.
        let outcome = match outcome {
            Ok(Some(key)) => Ok(Some(key)),
            other if !dispatched => other,
            other => {
                match upload_request(&mut call, id, MediaUploadRequest::Cancel { id: id.into() })
                    .await
                {
                    Ok(state) if state.phase == MediaUploadPhase::Published => {
                        published_key(&state, id, &prepared.name, frames, bytes).map(Some)
                    }
                    Ok(state) if state.id == id && state.phase == MediaUploadPhase::Cancelled => {
                        other
                    }
                    Ok(_) => Err(anyhow::anyhow!("Daemon did not confirm upload cleanup")),
                    Err(error) => Err(error.context("Cannot confirm upload outcome or cleanup")),
                }
            }
        };
        // Do not release admission until the prepared-file owner has been dropped.
        tokio::task::spawn_blocking(move || drop(file)).await?;
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        match outcome {
            Ok(Some(key)) => {
                job.status.phase = Phase::Complete;
                job.status.key = Some(key);
            }
            Ok(None) => job.status.phase = Phase::Cancelled,
            Err(error) => {
                job.status.phase = Phase::Failed;
                job.status.message = Some(format!("{error}. Check library key {id} before importing again: publication may have completed."));
            }
        }
        Ok(())
    }

    /// Resolve one dispatched publication. An error may be an ambiguous daemon outcome.
    pub fn published(&self, id: &str, result: Result<String, String>) -> Result<()> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        if job.status.phase != Phase::Publishing {
            bail!("Import is not awaiting publication");
        }
        match result {
            Ok(key) => {
                job.status.phase = Phase::Complete;
                job.status.key = Some(key);
            }
            Err(message) => {
                job.status.phase = Phase::Failed;
                job.status.message = Some(format!("{message}. Check the library before importing again: publication may have completed."));
            }
        }
        Ok(())
    }
    /// Stop accepting work and wait for upload cleanup, child reap, or publication.
    /// A normal window close must await this before terminating the application.
    pub async fn shutdown(&self) -> Result<()> {
        self.1.store(true, Ordering::Release);
        let owner = self.clone();
        tokio::task::spawn_blocking(move || loop {
            let Some(status) = owner.status()? else {
                return Ok(());
            };
            if status.phase.terminal() {
                return Ok(());
            }
            if status.phase != Phase::Publishing {
                if let Err(error) = owner.cancel(&status.id) {
                    // Preparation may have crossed into publication after status().
                    if !owner
                        .status()?
                        .is_some_and(|s| s.phase == Phase::Publishing || s.phase.terminal())
                    {
                        return Err(error);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        })
        .await?
    }
    fn fail(&self, id: &str, message: String) -> Result<()> {
        let mut current = self.lock()?;
        let job = find(&mut current, id)?;
        job.status.phase = Phase::Failed;
        job.status.message = Some(message);
        Ok(())
    }
}
async fn upload_request<F, Fut>(
    call: &mut F,
    id: &str,
    request: MediaUploadRequest,
) -> Result<MediaUploadState>
where
    F: FnMut(MediaUploadRequest) -> Fut,
    Fut: std::future::Future<Output = Result<MediaUploadState>>,
{
    match call(request).await {
        Ok(state) => Ok(state),
        Err(original) => call(MediaUploadRequest::Status { id: id.into() })
            .await
            .with_context(|| format!("Upload request failed: {original}")),
    }
}
fn validate_upload(
    state: &MediaUploadState,
    id: &str,
    name: &str,
    frames: usize,
    bytes: u64,
    received: usize,
    received_bytes: u64,
) -> Result<()> {
    if state.id != id
        || state.name != name
        || state.frames != frames
        || state.encoded_bytes != bytes
        || state.phase != MediaUploadPhase::Receiving
        || state.received_frames != received
        || state.received_bytes != received_bytes
    {
        bail!(
            "Unexpected upload receipt: {:?}: {:?}",
            state.phase,
            state.error
        );
    }
    Ok(())
}
fn published_key(
    state: &MediaUploadState,
    id: &str,
    name: &str,
    frames: usize,
    bytes: u64,
) -> Result<String> {
    let asset = state
        .asset
        .as_ref()
        .context("Publication has no asset receipt")?;
    if state.id != id
        || state.phase != MediaUploadPhase::Published
        || asset.key != id
        || asset.name != name
        || asset.frames as usize != frames
        || asset.encoded_bytes != bytes
        || asset.fps != 24
    {
        bail!("Unexpected publication receipt");
    }
    Ok(asset.key.clone())
}
fn find<'a>(current: &'a mut Option<Job>, id: &str) -> Result<&'a mut Job> {
    current
        .as_mut()
        .filter(|j| j.status.id == id)
        .context("Unknown media import ID")
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[tokio::test]
    async fn uploaded_adjustments_reach_the_reviewed_video() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.mp4");
        assert!(std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=cyan:s=32x32:r=24:d=0.25",
                "-c:v",
                "libx264",
                "-threads",
                "1",
            ])
            .arg(&source)
            .status()
            .unwrap()
            .success());
        let bytes = std::fs::read(source).unwrap();
        let imports = MediaImports::default();
        imports
            .begin(
                A.into(),
                "effects.mp4".into(),
                bytes.len() as u64,
                Fit::Contain,
            )
            .unwrap();
        assert!(imports.inspect(A).await.is_err()); // No partial upload inspection.
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Uploading);
        imports.append(A, 0, &bytes).unwrap();
        let inspected = imports.inspect(A).await.unwrap();
        assert_eq!(inspected.phase, Phase::SourceReady);
        let metadata = inspected.metadata.unwrap();
        assert_eq!((metadata.width, metadata.height), (32, 32));
        assert_eq!(metadata.duration_seconds, Some(0.25));
        assert_eq!(metadata.average_frame_rate.unwrap().numerator, 24);
        assert_eq!(
            imports
                .inspect_with(A, |_, _| panic!("ready inspection must not rerun"))
                .await
                .unwrap()
                .phase,
            Phase::SourceReady
        );
        assert!(imports.append(A, bytes.len() as u64, &[0]).is_err());
        assert!(imports
            .begin(B.into(), "other.mp4".into(), 1, Fit::Contain)
            .is_err());
        assert!(imports
            .prepare_review_edited(
                A,
                EditOptions {
                    range: Some(crate::video_range::VideoEditRange {
                        start_seconds: 0.0,
                        end_seconds: 0.01
                    }),
                    fit: Fit::Contain,
                    cut: None,
                    adjustments: VideoAdjustments::default(),
                }
            )
            .await
            .is_err());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::SourceReady);
        imports
            .prepare_review_edited(
                A,
                EditOptions {
                    range: Some(crate::video_range::VideoEditRange {
                        start_seconds: 0.0,
                        end_seconds: 0.2,
                    }),
                    fit: Fit::Contain,
                    cut: None,
                    adjustments: serde_json::from_str(r#"{"brightness":255,"contrast":32}"#)
                        .unwrap(),
                },
            )
            .await
            .unwrap();
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::ReviewReady);
        assert!(imports.status().unwrap().unwrap().metadata.is_none());
        assert!(imports
            .prepare_review_edited(
                A,
                EditOptions {
                    range: None,
                    fit: Fit::Cover,
                    cut: None,
                    adjustments: VideoAdjustments::default()
                }
            )
            .await
            .is_err());
        let result = imports.take_review(A).unwrap();
        assert_eq!(result.file.frames(), 5);
        let preview = result.file.preview(&AtomicBool::new(false)).unwrap();
        let output = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-threads", "1", "-i"])
            .arg(preview.lease().path())
            .args([
                "-frames:v",
                "6",
                "-pix_fmt",
                "rgb24",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 5 * 480 * 480 * 3);
        assert!(
            output.stdout.iter().all(|&channel| channel >= 250),
            "brightness255 must turn cyan source white"
        );
    }

    #[test]
    fn video_edit_range_cannot_be_combined_with_video_cut_before_admission() {
        let imports = MediaImports::default();
        let result = imports.begin_edited(
            A.into(),
            "source.mp4".into(),
            1,
            EditOptions {
                range: Some(crate::video_range::VideoEditRange {
                    start_seconds: 0.0,
                    end_seconds: 1.0,
                }),
                fit: Fit::Contain,
                cut: Some(VideoCut {
                    segments: vec![media_prepare::Segment {
                        start_ms: 0,
                        end_ms: 1000,
                        crop: None,
                    }],
                }),
                adjustments: VideoAdjustments::default(),
            },
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("mutually exclusive"));
        assert!(imports.status().unwrap().is_none());
    }

    #[tokio::test]
    async fn inspected_prepare_admits_options_atomically_and_rejects_competing_request() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "source.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let options = || EditOptions {
            range: None,
            fit: Fit::Cover,
            cut: None,
            adjustments: VideoAdjustments {
                contrast: 32,
                brightness: 128,
            },
        };
        assert!(imports.prepare_review_edited(A, options()).await.is_err());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Uploading);
        imports
            .inspect_with(A, |_, _| {
                Ok(media_prepare::SourceMetadata {
                    width: 32,
                    height: 32,
                    duration_seconds: Some(1.0),
                    average_frame_rate: None,
                    nominal_frame_rate: None,
                })
            })
            .await
            .unwrap();
        let invalid = EditOptions {
            range: None,
            fit: Fit::Contain,
            cut: Some(VideoCut { segments: vec![] }),
            adjustments: VideoAdjustments::default(),
        };
        assert!(imports.prepare_review_edited(A, invalid).await.is_err());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::SourceReady);
        assert!(imports.status().unwrap().unwrap().metadata.is_some());
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let owner = imports.clone();
        let task = tokio::spawn(async move {
            owner
                .prepare_with(
                    A,
                    Some(options()),
                    move |_, fit, cut, adjustments, _range, _, _| {
                        assert!(matches!(fit, Fit::Cover));
                        assert!(cut.is_none());
                        assert_eq!((adjustments.contrast, adjustments.brightness), (32, 128));
                        started.send(()).unwrap();
                        wait.recv().unwrap();
                        bail!("worker finished")
                    },
                )
                .await
        });
        wait_started.await.unwrap();
        assert!(imports.status().unwrap().unwrap().metadata.is_none());
        assert!(imports
            .prepare_review_edited(
                A,
                EditOptions {
                    range: None,
                    fit: Fit::Stretch,
                    cut: None,
                    adjustments: VideoAdjustments::default()
                }
            )
            .await
            .is_err());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Inspecting);
        imports.cancel(A).unwrap();
        release.send(()).unwrap();
        assert!(task.await.unwrap().unwrap().is_none());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Cancelled);
    }

    #[tokio::test]
    async fn inspecting_cancellation_keeps_input_until_worker_returns_then_discards_metadata() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "source.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let (started, wait_started) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let owner = imports.clone();
        let task = tokio::spawn(async move {
            owner
                .inspect_with(A, move |path, cancel| {
                    started.send(path.to_owned()).unwrap();
                    wait.recv().unwrap();
                    assert!(cancel.load(Ordering::Acquire));
                    assert!(path.exists());
                    Ok(media_prepare::SourceMetadata {
                        width: 32,
                        height: 32,
                        duration_seconds: Some(1.0),
                        average_frame_rate: None,
                        nominal_frame_rate: None,
                    })
                })
                .await
        });
        let path = wait_started.await.unwrap();
        assert_eq!(imports.cancel(A).unwrap().phase, Phase::Cancelling);
        assert!(path.exists());
        assert!(imports
            .begin(B.into(), "other.mp4".into(), 1, Fit::Contain)
            .is_err());
        assert!(imports.prepare(A).await.is_err());
        release.send(()).unwrap();
        let result = task.await.unwrap().unwrap();
        assert_eq!(result.phase, Phase::Cancelled);
        assert!(result.metadata.is_none());
        assert!(!path.exists());
        imports
            .begin(B.into(), "other.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.cancel(B).unwrap();
    }

    #[tokio::test]
    async fn failed_inspection_and_ready_shutdown_release_staging() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "broken.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let path = imports
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .path();
        assert!(imports.inspect(A).await.is_err());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Failed);
        assert!(imports.status().unwrap().unwrap().metadata.is_none());
        assert!(!path.exists());
        imports
            .begin(B.into(), "next.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(B, 0, &[0]).unwrap();
        imports
            .inspect_with(B, |_, _| {
                Ok(media_prepare::SourceMetadata {
                    width: 32,
                    height: 32,
                    duration_seconds: None,
                    average_frame_rate: None,
                    nominal_frame_rate: None,
                })
            })
            .await
            .unwrap();
        let path = imports
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .path();
        imports.shutdown().await.unwrap();
        let status = imports.status().unwrap().unwrap();
        assert_eq!(status.phase, Phase::Cancelled);
        assert!(status.metadata.is_none());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn reviewed_media_is_owned_until_confirm_cancel_or_shutdown() {
        let fixture = tempfile::tempdir().unwrap();
        let source = fixture.path().join("source.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=cyan:s=64x64:r=24:d=0.5",
                "-c:v",
                "libx264",
                "-threads",
                "1",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let bytes = std::fs::read(source).unwrap();
        for action in ["confirm", "cancel", "shutdown"] {
            let imports = MediaImports::default();
            imports
                .begin(
                    A.into(),
                    "review.mp4".into(),
                    bytes.len() as u64,
                    Fit::Contain,
                )
                .unwrap();
            assert!(imports.preview_length(A).is_err());
            imports.append(A, 0, &bytes).unwrap();
            let owner = imports.clone();
            tokio::spawn(async move { owner.prepare_review(A).await })
                .await
                .unwrap()
                .unwrap();
            assert_eq!(imports.status().unwrap().unwrap().phase, Phase::ReviewReady);
            assert!(imports.preview_length(A).unwrap() > 0);
            let request = |method, range: Option<&str>| {
                let mut builder = tauri::http::Request::builder()
                    .method(method)
                    .uri(format!("media-preview://localhost/{A}"));
                if let Some(range) = range {
                    builder = builder.header("Range", range);
                }
                builder.body(Vec::new()).unwrap()
            };
            let total = imports.preview_length(A).unwrap();
            let head = crate::media_preview::serve(&imports, "main", request("HEAD", None));
            assert_eq!(head.status(), 200);
            assert!(head.body().is_empty());
            assert_eq!(head.headers()["content-length"], total.to_string());
            let partial =
                crate::media_preview::serve(&imports, "main", request("GET", Some("bytes=8-15")));
            assert_eq!(partial.status(), 206);
            assert_eq!(
                partial.headers()["content-range"],
                format!("bytes 8-15/{total}")
            );
            assert_eq!(partial.body(), &imports.preview_range(A, 8, 8).unwrap());
            assert_eq!(partial.headers()["cache-control"], "no-store");
            let invalid = crate::media_preview::serve(
                &imports,
                "main",
                request("GET", Some("bytes=999999999-")),
            );
            assert_eq!(invalid.status(), 416);
            assert_eq!(
                invalid.headers()["content-range"],
                format!("bytes */{total}")
            );
            assert_eq!(
                crate::media_preview::serve(&imports, "main", request("POST", None)).status(),
                405
            );

            assert!(!imports.preview_range(A, 0, 64).unwrap().is_empty());
            assert!(imports.preview_range(B, 0, 64).is_err());
            assert!(imports.preview_range(A, 0, MAX_CHUNK + 1).is_err());
            assert!(imports
                .begin(B.into(), "other.mp4".into(), 1, Fit::Cover)
                .is_err());
            assert!(imports.prepare_review(A).await.is_err());
            let (media_path, first) = {
                let mut current = imports.lock().unwrap();
                let review = current.as_mut().unwrap().review.as_mut().unwrap();
                (
                    review.media.file.staging_path().to_owned(),
                    review.media.file.read_unit(0).unwrap().bytes,
                )
            };
            if action == "confirm" {
                let mut media = imports.take_review(A).unwrap();
                assert_eq!(media.file.read_unit(0).unwrap().bytes, first);
                assert_eq!(media.file.staging_path(), media_path);
                assert!(imports.take_review(A).is_err());
                assert!(imports.preview_range(A, 0, 1).is_err());
                assert_eq!(
                    imports.status().unwrap().unwrap().phase,
                    Phase::Transferring
                );
                imports
                    .publish(A, media, |_| async { bail!("fixture disconnected") })
                    .await
                    .unwrap();
                assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Failed);
            } else if action == "cancel" {
                assert_eq!(imports.cancel(A).unwrap().phase, Phase::Cancelled);
                assert!(imports.take_review(A).is_err());
            } else {
                imports.shutdown().await.unwrap();
                assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Cancelled);
                assert!(imports
                    .begin(B.into(), "other.mp4".into(), 1, Fit::Contain)
                    .is_err());
            }
            assert!(!media_path.exists());
            assert!(imports.preview_length(A).is_err());
            if action != "shutdown" {
                imports
                    .begin(B.into(), "other.mp4".into(), 1, Fit::Contain)
                    .unwrap();
                imports.cancel(B).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn cancelled_successful_preparation_releases_its_file_before_terminal_return() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "test.mp4".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let staging = Arc::new(Mutex::new(None));
        let observation = staging.clone();
        let result = imports
            .prepare_with(
                A,
                None,
                move |source, fit, _cut, _adjustments, _range, cancel, progress| {
                    let output = std::process::Command::new("ffmpeg")
                        .args([
                            "-y",
                            "-v",
                            "error",
                            "-f",
                            "lavfi",
                            "-i",
                            "color=c=cyan:s=64x64:r=24",
                            "-t",
                            "0.5",
                            "-threads",
                            "1",
                        ])
                        .arg(source)
                        .output()?;
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let prepared = media_prepare::prepare_file(source, fit, cancel, progress)?;
                    *observation.lock().unwrap() = Some(prepared.staging_path().to_owned());
                    cancel.store(true, Ordering::Release);
                    Ok(prepared)
                },
            )
            .await
            .unwrap();
        assert!(result.is_none());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Cancelled);
        assert!(!staging.lock().unwrap().as_ref().unwrap().exists());
        imports.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn chunked_publication_recovers_receipts_and_cancels_without_duplicates() {
        for scenario in [
            "normal",
            "lost_begin",
            "lost_chunk",
            "lost_finish",
            "cancel",
            "failed_chunk",
        ] {
            let imports = MediaImports::default();
            imports
                .begin(A.into(), "test.mp4".into(), 1, Fit::Contain)
                .unwrap();
            imports.append(A, 0, &[0]).unwrap();
            let prepared = imports
                .prepare_with(
                    A,
                    None,
                    |source, fit, _cut, _adjustments, _range, cancel, progress| {
                        let output = std::process::Command::new("ffmpeg")
                            .args([
                                "-y",
                                "-v",
                                "error",
                                "-f",
                                "lavfi",
                                "-i",
                                "color=c=cyan:s=64x64:r=24",
                                "-t",
                                "0.5",
                                "-threads",
                                "1",
                            ])
                            .arg(source)
                            .output()?;
                        assert!(
                            output.status.success(),
                            "{}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        media_prepare::prepare_file(source, fit, cancel, progress)
                    },
                )
                .await
                .unwrap()
                .unwrap();
            let receipt = Arc::new(Mutex::new(None::<MediaUploadState>));
            let finish_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let remote = receipt.clone();
            let count = finish_count.clone();
            let canceller = imports.clone();
            imports
                .publish(A, prepared, move |request| {
                    let remote = remote.clone();
                    let count = count.clone();
                    let canceller = canceller.clone();
                    async move {
                        let mut state = remote.lock().unwrap();
                        match request {
                            MediaUploadRequest::Begin {
                                id,
                                name,
                                frames,
                                encoded_bytes,
                            } => {
                                assert!(state.is_none());
                                *state = Some(MediaUploadState {
                                    id,
                                    name,
                                    frames,
                                    encoded_bytes,
                                    received_frames: 0,
                                    received_bytes: 0,
                                    phase: MediaUploadPhase::Receiving,
                                    asset: None,
                                    error: None,
                                });
                                if scenario == "lost_begin" {
                                    bail!("lost Begin reply");
                                }
                            }
                            MediaUploadRequest::Chunk { index, unit, .. } => {
                                if scenario == "failed_chunk" {
                                    bail!("chunk not accepted");
                                }
                                let state = state.as_mut().unwrap();
                                assert_eq!(index, state.received_frames);
                                state.received_frames += 1;
                                state.received_bytes += unit.bytes.len() as u64;
                                if index == 2 && scenario == "cancel" {
                                    canceller.cancel(A).unwrap();
                                }
                                if scenario == "lost_chunk" {
                                    bail!("lost Chunk reply");
                                }
                            }
                            MediaUploadRequest::Finish { .. } => {
                                count.fetch_add(1, Ordering::SeqCst);
                                let state = state.as_mut().unwrap();
                                assert_eq!(state.received_frames, state.frames);
                                assert_eq!(state.received_bytes, state.encoded_bytes);
                                assert_eq!(
                                    canceller.status().unwrap().unwrap().phase,
                                    Phase::Publishing
                                );
                                assert!(canceller.cancel(A).is_err());
                                state.phase = MediaUploadPhase::Published;
                                state.asset = Some(lconnect_ipc::PreparedClipInfo {
                                    is_default: false,
                                    key: A.into(),
                                    name: state.name.clone(),
                                    fps: 24,
                                    frames: state.frames as u32,
                                    encoded_bytes: state.encoded_bytes,
                                });
                                if scenario == "lost_finish" {
                                    bail!("lost Finish reply");
                                }
                            }
                            MediaUploadRequest::Cancel { .. } => {
                                let state = state.as_mut().unwrap();
                                if state.phase != MediaUploadPhase::Published {
                                    state.phase = MediaUploadPhase::Cancelled;
                                }
                            }
                            MediaUploadRequest::Status { .. } => {}
                        }
                        Ok(state.as_ref().unwrap().clone())
                    }
                })
                .await
                .unwrap();
            let status = imports.status().unwrap().unwrap();
            if scenario == "cancel" || scenario == "failed_chunk" {
                assert_eq!(finish_count.load(Ordering::SeqCst), 0);
                assert_eq!(
                    receipt.lock().unwrap().as_ref().unwrap().phase,
                    MediaUploadPhase::Cancelled
                );
                assert_eq!(
                    status.phase,
                    if scenario == "cancel" {
                        Phase::Cancelled
                    } else {
                        Phase::Failed
                    }
                );
            } else {
                assert_eq!(finish_count.load(Ordering::SeqCst), 1);
                assert_eq!(status.phase, Phase::Complete);
                assert_eq!(status.key.as_deref(), Some(A));
            }
        }
    }

    #[test]
    fn bounded_ordered_upload_and_cancel_release_owned_files() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "x.mp4".into(), 4, Fit::Contain)
            .unwrap();
        let path = imports
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .path()
            .parent()
            .unwrap()
            .to_owned();
        assert!(imports
            .begin(B.into(), "b.gif".into(), 4, Fit::Cover)
            .is_err());
        imports.append(A, 0, &[1, 2]).unwrap();
        assert!(imports.append(A, 0, &[1, 2]).is_err());
        assert!(imports.append(A, 2, &[1, 2, 3]).is_err());
        assert!(imports.append(A, 2, &vec![0; MAX_CHUNK + 1]).is_err());
        assert!(imports.cancel(B).is_err());
        assert_eq!(imports.status().unwrap().unwrap().received, 2);
        assert_eq!(imports.cancel(A).unwrap().phase, Phase::Cancelled);
        assert!(!path.exists());
        imports
            .begin(B.into(), "b.gif".into(), 4, Fit::Cover)
            .unwrap();
    }

    #[tokio::test]
    async fn complete_uploaded_mp4_prepares_once_then_publication_is_explicit() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.mp4");
        let result = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=64x64:r=24",
                "-t",
                "0.5",
                "-threads",
                "1",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(result.status.success());
        let bytes = std::fs::read(path).unwrap();
        let imports = MediaImports::default();
        imports
            .begin(
                A.into(),
                "blue.mp4".into(),
                bytes.len() as u64,
                Fit::Contain,
            )
            .unwrap();
        assert!(imports.prepare(A).await.is_err());
        for (index, chunk) in bytes.chunks(256).enumerate() {
            imports.append(A, (index * 256) as u64, chunk).unwrap();
        }
        let path = imports
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .path()
            .parent()
            .unwrap()
            .to_owned();
        let mut prepared = imports.prepare(A).await.unwrap().unwrap();
        assert_eq!(prepared.name, "blue.mp4");
        assert!(prepared.file.frames() > 0);
        prepared.file.read_unit(0).unwrap();
        assert!(imports.begin_commit(A).unwrap());
        assert!(!path.exists());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Publishing);
        assert!(imports.cancel(A).is_err());
        assert!(imports.prepare(A).await.is_err());
        imports.published(A, Ok("saved-key".into())).unwrap();
        assert_eq!(
            imports.status().unwrap().unwrap().key.as_deref(),
            Some("saved-key")
        );
        assert!(imports.published(A, Ok("duplicate".into())).is_err());
    }

    #[tokio::test]
    async fn cancellation_keeps_admission_until_worker_returns_and_never_publishes() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "x.gif".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = imports.clone();
        let task = tokio::spawn(async move {
            worker
                .prepare_with(A, None, move |_, _, _, _, _, _, _| {
                    started_tx.send(()).unwrap();
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    bail!("cancelled fixture")
                })
                .await
        });
        started_rx.await.unwrap();
        assert_eq!(imports.cancel(A).unwrap().phase, Phase::Cancelling);
        assert!(imports
            .begin(B.into(), "b.gif".into(), 1, Fit::Contain)
            .is_err());
        release_tx.send(()).unwrap();
        assert!(task.await.unwrap().unwrap().is_none());
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Cancelled);
        imports
            .begin(B.into(), "b.gif".into(), 1, Fit::Contain)
            .unwrap();
    }

    #[tokio::test]
    async fn shutdown_stops_admission_and_waits_for_preparing_worker() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "x.gif".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = imports.clone();
        let task = tokio::spawn(async move {
            worker
                .prepare_with(A, None, move |_, _, _, _, _, _, _| {
                    started_tx.send(()).unwrap();
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    bail!("cancelled fixture")
                })
                .await
        });
        started_rx.await.unwrap();
        let closer = imports.clone();
        let shutdown = tokio::spawn(async move { closer.shutdown().await });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while imports.status().unwrap().unwrap().phase != Phase::Cancelling {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!shutdown.is_finished());
        release_tx.send(()).unwrap();
        assert!(task.await.unwrap().unwrap().is_none());
        shutdown.await.unwrap().unwrap();
        assert!(imports
            .begin(B.into(), "b.gif".into(), 1, Fit::Contain)
            .is_err());
    }

    #[tokio::test]
    async fn shutdown_waits_for_publication_and_preserves_ambiguous_failure() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "x.gif".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        {
            let mut state = imports.lock().unwrap();
            let job = find(&mut state, A).unwrap();
            job.upload = None;
            job.status.phase = Phase::Publishing;
        }
        // This fixture targets lifecycle after preparation, not codec validation.
        let closer = imports.clone();
        let mut shutdown = tokio::spawn(async move { closer.shutdown().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(60), &mut shutdown)
                .await
                .is_err()
        );
        assert_eq!(imports.status().unwrap().unwrap().phase, Phase::Publishing);
        assert!(imports
            .begin(B.into(), "b.gif".into(), 1, Fit::Contain)
            .is_err());
        imports
            .published(A, Err("Daemon reply lost".into()))
            .unwrap();
        shutdown.await.unwrap().unwrap();
        let status = imports.status().unwrap().unwrap();
        assert_eq!(status.phase, Phase::Failed);
        assert!(status
            .message
            .unwrap()
            .contains("publication may have completed"));
        assert!(imports.published(A, Ok("retry".into())).is_err());
    }

    #[tokio::test]
    async fn failed_preparation_cleans_staging_and_allows_new_job() {
        let imports = MediaImports::default();
        imports
            .begin(A.into(), "bad.gif".into(), 1, Fit::Contain)
            .unwrap();
        imports.append(A, 0, &[0]).unwrap();
        let path = imports
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .upload
            .as_ref()
            .unwrap()
            .path()
            .parent()
            .unwrap()
            .to_owned();
        assert!(imports.prepare(A).await.is_err());
        let status = imports.status().unwrap().unwrap();
        assert_eq!(status.phase, Phase::Failed);
        assert!(status.message.is_some());
        assert!(!path.exists());
        imports
            .begin(B.into(), "b.gif".into(), 1, Fit::Contain)
            .unwrap();
    }
}
