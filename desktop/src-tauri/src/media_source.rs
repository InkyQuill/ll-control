//! One managed gallery source. Owned workers retain admission through I/O/reaping.
use crate::{
    media_download,
    media_prepare::{inspect_file, PreparedFile, PreviewFile, SourceMetadata},
};
use anyhow::{bail, ensure, Context, Result};
use llcontrol_ipc::client::IpcClient;
use serde::Serialize;
use std::{
    future::Future,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Downloading,
    Remuxing,
    Ready,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub key: String,
    pub phase: Phase,
    pub message: Option<String>,
    pub metadata: Option<SourceMetadata>,
}
struct Files {
    _media: PreparedFile,
    preview: PreviewFile,
    metadata: SourceMetadata,
}
impl Files {
    // Retain media/preview through both remux and probe; a cancelled request
    // remains admitted until the blocking worker returns and drops these owners.
    fn prepare(media: PreparedFile, cancel: &AtomicBool) -> Result<Self> {
        let preview = media.preview(cancel)?;
        let metadata = inspect_file(&preview.lease().path(), cancel)?;
        Ok(Self {
            _media: media,
            preview,
            metadata,
        })
    }
}
struct Job {
    status: Status,
    cancel: Arc<AtomicBool>,
    signal: watch::Sender<bool>,
    files: Option<Files>,
}
#[derive(Default)]
struct State {
    closed: bool,
    job: Option<Job>,
}
#[derive(Clone, Default)]
pub struct MediaSources(Arc<Mutex<State>>);
impl MediaSources {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Media source state unavailable"))
    }
    pub fn status(&self) -> Result<Option<Status>> {
        Ok(self.lock()?.job.as_ref().map(|job| job.status.clone()))
    }

    /// Start from a trusted native socket path, never a webview-provided path.
    /// Returns admission immediately; a lost invoke reply is recovered via status.
    pub fn start(&self, socket: PathBuf, id: String, key: String) -> Result<Status> {
        let owner = self.clone();
        let job_id = id.clone();
        let source_key = key.clone();
        self.start_with(id,key,move |mut signal,cancel| async move {
            let client=tokio::select! {
                biased;
                _=signal.wait_for(|v|*v)=>bail!("Media source cancelled"),
                connected=tokio::time::timeout(Duration::from_secs(3),IpcClient::connect(&socket))=>connected.context("Daemon connection timed out")??,
            };
            let media=media_download::download_cancellable(client,source_key,job_id.clone(),signal).await?;
            owner.remuxing(&job_id)?;
            tokio::task::spawn_blocking(move || {
                Files::prepare(media, &cancel)
            }).await?
        })
    }
    fn start_with<F, Fut>(&self, id: String, key: String, work: F) -> Result<Status>
    where
        F: FnOnce(watch::Receiver<bool>, Arc<AtomicBool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Files>> + Send + 'static,
    {
        ensure!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Invalid source id"
        );
        ensure!(!key.is_empty() && key.len() <= 128, "Invalid media key");
        let mut state = self.lock()?;
        ensure!(!state.closed, "Media sources are shutting down");
        if let Some(job) = &state.job {
            ensure!(
                matches!(job.status.phase, Phase::Cancelled | Phase::Failed),
                "Close the current source before opening another"
            );
            ensure!(job.status.id != id, "Use a fresh source id");
        }
        let (signal, receiver) = watch::channel(false);
        let cancel = Arc::new(AtomicBool::new(false));
        let status = Status {
            id: id.clone(),
            key,
            phase: Phase::Downloading,
            message: None,
            metadata: None,
        };
        state.job = Some(Job {
            status: status.clone(),
            cancel: cancel.clone(),
            signal,
            files: None,
        });
        let owner = self.clone();
        // Outer task always records completion, even when an inner worker panics.
        // Cancellation signals the worker; it never aborts a filesystem owner.
        tokio::spawn(async move {
            let result = tokio::spawn(async move { work(receiver, cancel).await }).await;
            let result = result.map_err(anyhow::Error::from).and_then(|value| value);
            let _ = owner.finished(&id, result);
        });
        Ok(status)
    }
    fn remuxing(&self, id: &str) -> Result<()> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("No source")?;
        ensure!(job.status.id == id, "Stale source");
        ensure!(
            !job.cancel.load(Ordering::Acquire),
            "Media source cancelled"
        );
        job.status.phase = Phase::Remuxing;
        Ok(())
    }
    fn finished(&self, id: &str, result: Result<Files>) -> Result<()> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("No source")?;
        ensure!(job.status.id == id, "Stale source");
        if job.cancel.load(Ordering::Acquire) {
            drop(result);
            job.status.phase = Phase::Cancelled;
            job.status.message = None;
        } else {
            match result {
                Ok(files) => {
                    job.status.metadata = Some(files.metadata.clone());
                    job.files = Some(files);
                    job.status.phase = Phase::Ready;
                }
                Err(error) => {
                    job.status.phase = Phase::Failed;
                    job.status.message = Some(error.to_string());
                }
            }
        }
        Ok(())
    }
    /// Run on blocking worker: ready cancellation removes temporary files before
    /// terminal status becomes visible. Active cancellation retains admission.
    pub fn cancel(&self, id: &str) -> Result<Status> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("No source")?;
        ensure!(job.status.id == id, "Stale source");
        Self::cancel_job(job);
        Ok(job.status.clone())
    }
    fn cancel_job(job: &mut Job) {
        job.status.metadata = None;
        job.cancel.store(true, Ordering::Release);
        let _ = job.signal.send(true);
        match job.status.phase {
            Phase::Ready => {
                drop(job.files.take());
                job.status.phase = Phase::Cancelled;
            }
            Phase::Downloading | Phase::Remuxing => job.status.phase = Phase::Cancelling,
            _ => {}
        }
    }
    /// Close admission and request cancellation. False means an owned worker is
    /// still running; caller must wait for terminal status before app exit.
    pub fn shutdown(&self) -> Result<bool> {
        let mut state = self.lock()?;
        state.closed = true;
        if let Some(job) = state.job.as_mut() {
            Self::cancel_job(job);
            Ok(matches!(job.status.phase, Phase::Cancelled | Phase::Failed))
        } else {
            Ok(true)
        }
    }
    pub(crate) fn lease(&self, id: &str) -> Result<crate::media_prepare::PreviewLease> {
        let state = self.lock()?;
        let job = state.job.as_ref().context("No source")?;
        ensure!(
            job.status.id == id && job.status.phase == Phase::Ready,
            "Source unavailable"
        );
        Ok(job
            .files
            .as_ref()
            .context("No source files")?
            .preview
            .lease())
    }
    /// Native range handlers use opaque IDs. Filesystem paths never reach UI.
    pub fn preview_length(&self, id: &str) -> Result<u64> {
        let state = self.lock()?;
        let job = state.job.as_ref().context("No source")?;
        ensure!(
            job.status.id == id && job.status.phase == Phase::Ready,
            "Source unavailable"
        );
        Ok(job
            .files
            .as_ref()
            .context("No source files")?
            .preview
            .length())
    }
    pub fn preview_range(&self, id: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("No source")?;
        ensure!(
            job.status.id == id && job.status.phase == Phase::Ready,
            "Source unavailable"
        );
        job.files
            .as_mut()
            .context("No source files")?
            .preview
            .read_range(offset, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn terminal(owner: &MediaSources) -> Status {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let status = owner.status().unwrap().unwrap();
                if matches!(
                    status.phase,
                    Phase::Cancelled | Phase::Failed | Phase::Ready
                ) {
                    return status;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn cancellation_keeps_admission_until_worker_exits_and_shutdown_closes_it() {
        let owner = MediaSources::default();
        let id = "a".repeat(32);
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        owner
            .start_with(id.clone(), "saved".into(), move |_, cancel| async move {
                tokio::task::spawn_blocking(move || {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                    assert!(cancel.load(Ordering::Acquire));
                    bail!("worker reaped")
                })
                .await?
            })
            .unwrap();
        ready.await.unwrap();
        assert_eq!(owner.cancel(&id).unwrap().phase, Phase::Cancelling);
        assert!(owner
            .start_with("b".repeat(32), "next".into(), |_, _| async {
                bail!("unused")
            })
            .is_err());
        assert!(!owner.shutdown().unwrap());
        release.send(()).unwrap();
        assert_eq!(terminal(&owner).await.phase, Phase::Cancelled);
        assert!(owner.shutdown().unwrap());
        assert!(owner
            .start_with("b".repeat(32), "next".into(), |_, _| async {
                bail!("unused")
            })
            .is_err());
    }
    #[tokio::test]
    async fn worker_panic_recovers_and_stale_cancel_cannot_cancel_new_job() {
        let owner = MediaSources::default();
        let id = "a".repeat(32);
        owner
            .start_with(id.clone(), "saved".into(), |_, _| async {
                panic!("source worker failure")
            })
            .unwrap();
        assert_eq!(terminal(&owner).await.phase, Phase::Failed);
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let next = "b".repeat(32);
        owner
            .start_with(next.clone(), "next".into(), |_, _| async {
                let _ = wait.await;
                bail!("expected failure")
            })
            .unwrap();
        assert!(owner.cancel(&id).is_err());
        assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Downloading);
        release.send(()).unwrap();
        assert_eq!(terminal(&owner).await.phase, Phase::Failed);
    }
    #[tokio::test]
    async fn ready_source_has_bounded_reads_and_cancel_revokes_and_removes_files() {
        let files = tokio::task::spawn_blocking(|| {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("source.mp4");
            assert!(std::process::Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=s=480x480:r=24",
                    "-t",
                    "0.5",
                    "-threads",
                    "1"
                ])
                .arg(&input)
                .status()
                .unwrap()
                .success());
            let media = crate::media_prepare::prepare_file(
                &input,
                crate::media_prepare::Fit::Contain,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            Files::prepare(media, &AtomicBool::new(false)).unwrap()
        })
        .await
        .unwrap();
        let path = files._media.staging_path().to_owned();
        let owner = MediaSources::default();
        let id = "a".repeat(32);
        owner
            .start_with(id.clone(), "saved".into(), |_, _| async { Ok(files) })
            .unwrap();
        let ready = terminal(&owner).await;
        assert_eq!(ready.phase, Phase::Ready);
        let metadata = ready.metadata.as_ref().unwrap();
        assert_eq!((metadata.width, metadata.height), (480, 480));
        assert_eq!(metadata.duration_seconds, Some(0.5));
        assert_eq!(metadata.average_frame_rate.unwrap().numerator, 24);
        assert_eq!(metadata.average_frame_rate.unwrap().denominator, 1);
        assert_eq!(
            serde_json::to_value(&ready).unwrap()["metadata"]["durationSeconds"],
            0.5
        );
        let worker = owner.clone();
        let job = id.clone();
        tokio::task::spawn_blocking(move || {
            assert!(worker.preview_length(&job).unwrap() > 0);
            let request = tauri::http::Request::builder()
                .uri(format!("media-source://localhost/{job}"))
                .header("Range", "bytes=0-255")
                .body(Vec::new())
                .unwrap();
            let response = crate::media_preview::serve(&worker, "main", request);
            assert_eq!(response.status(), tauri::http::StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.body().len(), 256);

            assert!(!worker.preview_range(&job, 0, 256).unwrap().is_empty());
            assert!(worker.preview_range(&job, 0, 256 * 1024 + 1).is_err());
            let cancelled = worker.cancel(&job).unwrap();
            assert_eq!(cancelled.phase, Phase::Cancelled);
            assert!(cancelled.metadata.is_none());
            assert!(worker.status().unwrap().unwrap().metadata.is_none());
            assert!(worker.preview_length(&job).is_err());
            let request = tauri::http::Request::builder()
                .uri(format!("media-source://localhost/{job}"))
                .body(Vec::new())
                .unwrap();
            assert_eq!(
                crate::media_preview::serve(&worker, "main", request).status(),
                tauri::http::StatusCode::NOT_FOUND
            );

            assert!(worker.preview_range(&job, 0, 1).is_err());
            assert!(!path.exists());
        })
        .await
        .unwrap();
        owner
            .start_with("b".repeat(32), "next".into(), |_, _| async {
                bail!("new job")
            })
            .unwrap();
        let failed = terminal(&owner).await;
        assert_eq!(failed.phase, Phase::Failed);
        assert!(failed.metadata.is_none());
    }
    #[tokio::test]
    async fn edit_lease_survives_preview_cancel_and_prepares_selected_frames_for_review() {
        let files = tokio::task::spawn_blocking(|| {
            let root = tempfile::tempdir().unwrap();
            let input = root.path().join("source.mp4");
            assert!(std::process::Command::new("ffmpeg")
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=s=480x480:r=24",
                    "-t",
                    "0.5",
                    "-threads",
                    "1"
                ])
                .arg(&input)
                .status()
                .unwrap()
                .success());
            let media = crate::media_prepare::prepare_file(
                &input,
                crate::media_prepare::Fit::Contain,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap();
            Files::prepare(media, &AtomicBool::new(false)).unwrap()
        })
        .await
        .unwrap();

        let source = MediaSources::default();
        let source_id = "a".repeat(32);
        source
            .start_with(source_id.clone(), "saved".into(), |_, _| async {
                Ok(files)
            })
            .unwrap();
        assert_eq!(terminal(&source).await.phase, Phase::Ready);
        let imports = crate::media_import::MediaImports::default();
        let id = "b".repeat(32);
        let worker = imports.clone();
        let sources = source.clone();
        let job = id.clone();
        let path = tokio::task::spawn_blocking(move || {
            let lease = sources.lease(&source_id).unwrap();
            let path = lease.path();
            drop(lease);
            let cut = crate::media_prepare::VideoCut {
                segments: vec![crate::media_prepare::Segment {
                    start_ms: 250,
                    end_ms: 500,
                    crop: None,
                }],
            };
            let status = worker
                .begin_source_edited(
                    &sources,
                    &source_id,
                    job.clone(),
                    "edited.mp4".into(),
                    crate::media_import::EditOptions {
                        range: None,
                        fit: crate::media_prepare::Fit::Contain,
                        cut: Some(cut),
                        adjustments: crate::video_adjustments::VideoAdjustments {
                            contrast: 32,
                            brightness: 255,
                        },
                    },
                )
                .unwrap();
            assert_eq!(status.received, status.total);
            assert!(worker
                .begin_source(
                    &sources,
                    &source_id,
                    "c".repeat(32),
                    "duplicate.mp4".into(),
                    crate::media_prepare::Fit::Contain,
                    None
                )
                .is_err());
            sources.cancel(&source_id).unwrap();
            assert!(sources.preview_length(&source_id).is_err());
            assert!(
                path.exists(),
                "admitted editor must retain source after preview revocation"
            );
            path
        })
        .await
        .unwrap();
        imports.prepare_review(&id).await.unwrap();
        assert_eq!(
            imports.status().unwrap().unwrap().phase,
            crate::media_import::Phase::ReviewReady
        );
        assert!(!path.exists(), "input lease released after preparation");
        tokio::task::spawn_blocking(move || {
            assert!(imports.preview_length(&id).unwrap() > 0);
            let result = imports.take_review(&id).unwrap();
            assert_eq!(result.file.frames(), 6);
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
            assert_eq!(output.stdout.len(), 6 * 480 * 480 * 3);
            assert!(
                output.stdout.iter().all(|&channel| channel >= 250),
                "leased source must retain admitted effects after revocation"
            );
            assert!(imports.take_review(&id).is_err());
        })
        .await
        .unwrap();
    }
}
