//! One desktop recording job. Admission/status survive lost invoke responses.
//! The blocking owner retains recorder/session/files until joined cancellation.
use crate::{
    screen_record::{self, RecordingFile},
    screen_record_process::Recorder,
    screencast,
};
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    future::Future,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tokio::sync::{oneshot, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Reserved,
    Selecting,
    Setup,
    Recording,
    Paused,
    LimitReached,
    Finalizing,
    Ready,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub id: String,
    pub phase: Phase,
    pub recorded_millis: u64,
    pub frames: u64,
    pub message: Option<String>,
    pub metadata: Option<crate::media_prepare::SourceMetadata>,
    pub region: Option<crate::media_prepare::VideoCrop>,
}
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Pause,
    Resume,
    Finish,
    SetRegion(crate::media_prepare::VideoCrop),
}
enum Command {
    Control {
        action: Action,
        reply: oneshot::Sender<Result<()>>,
    },
    Snapshot {
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
}
struct Job {
    status: Status,
    cancel: watch::Sender<bool>,
    cancel_file: Arc<AtomicBool>,
    done: watch::Receiver<bool>,
    completion: Option<watch::Sender<bool>>,
    commands: mpsc::SyncSender<Command>,
    receiver: Option<mpsc::Receiver<Command>>,
    file: Option<Arc<RecordingFile>>,
}
#[derive(Default)]
struct State {
    closed: bool,
    job: Option<Job>,
}
#[derive(Clone, Default)]
pub struct Recordings(Arc<Mutex<State>>);
struct Capture {
    recorder: Option<Recorder>,
    session: Option<screencast::Session>,
}
impl Capture {
    fn close(mut self, runtime: &tokio::runtime::Handle) -> Result<()> {
        drop(self.recorder.take());
        if let Some(mut session) = self.session.take() {
            close_session(&mut session, runtime)?;
        }
        Ok(())
    }
}
// Local FD/monitor ownership is released even if the remote Close reply fails.
// Record and log that failure; application exit waits for local release rather
// than keeping a dead portal session alive indefinitely.
fn close_session(
    session: &mut screencast::Session,
    runtime: &tokio::runtime::Handle,
) -> Result<()> {
    report_close(runtime.block_on(session.close()))
}
fn report_close(result: Result<()>) -> Result<()> {
    if let Err(error) = &result {
        eprintln!("ScreenCast portal Close was not confirmed after local release: {error:#}");
    }
    result
}
impl Recordings {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Recording state unavailable"))
    }
    pub fn status(&self) -> Result<Option<Status>> {
        Ok(self.lock()?.job.as_ref().map(|j| j.status.clone()))
    }
    pub(crate) fn lease(&self, id: &str) -> Result<Arc<RecordingFile>> {
        let state = self.lock()?;
        ensure!(!state.closed, "Recordings are shutting down");
        let job = state
            .job
            .as_ref()
            .filter(|j| j.status.id == id && j.status.phase == Phase::Ready)
            .context("Recording source unavailable")?;
        job.file.clone().context("Recording source unavailable")
    }
    /// Opaque-ID, bounded readers. Execute file reads on a blocking worker.
    pub fn preview_length(&self, id: &str) -> Result<u64> {
        Ok(self.lease(id)?.length())
    }
    pub fn preview_range(&self, id: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        ensure!(
            length > 0 && length <= crate::media_import::MAX_CHUNK,
            "Invalid recording read size"
        );
        let source = self.lease(id)?;
        ensure!(offset < source.length(), "Invalid recording offset");
        let size = (source.length() - offset).min(length as u64) as usize;
        let mut file = std::fs::File::open(source.path())?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; size];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    /// Reserve a caller-known UUID before opening any consent dialog.
    pub fn begin(&self, id: String) -> Result<Status> {
        ensure!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "Invalid recording ID"
        );
        let mut state = self.lock()?;
        ensure!(!state.closed, "Recordings are shutting down");
        if let Some(job) = &state.job {
            if job.status.id == id {
                return Ok(job.status.clone());
            }
            ensure!(
                matches!(job.status.phase, Phase::Cancelled | Phase::Failed),
                "Close the current recording first"
            );
            ensure!(*job.done.borrow(), "Recording cleanup is still running");
        }
        let (cancel, _) = watch::channel(false);
        let (completion, done) = watch::channel(false);
        let (commands, receiver) = mpsc::sync_channel(4);
        let status = Status {
            region: None,
            id: id.clone(),
            phase: Phase::Reserved,
            frames: 0,
            recorded_millis: 0,
            message: None,
            metadata: None,
        };
        state.job = Some(Job {
            status: status.clone(),
            cancel,
            cancel_file: Arc::new(AtomicBool::new(false)),
            done,
            completion: Some(completion),
            commands,
            receiver: Some(receiver),
            file: None,
        });
        let owner = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            if let Ok(mut state) = owner.lock() {
                if let Some(job) = state
                    .job
                    .as_mut()
                    .filter(|j| j.status.id == id && j.status.phase == Phase::Reserved)
                {
                    Self::cancel_job(job);
                }
            }
        });
        Ok(status)
    }
    /// Synthetic source for the isolated native acceptance binary only.
    #[cfg(feature = "native-smoke")]
    pub fn start_synthetic(&self, id: &str) -> Result<()> {
        self.start_with(id, |cancel| async move {
            Ok(Some(Capture {
                recorder: Some(Recorder::synthetic_prepare_with_cancel(cancel)?),
                session: None,
            }))
        })
    }
    pub fn start(&self, id: &str) -> Result<()> {
        self.start_with(id, |mut cancel| async move {
            let connection = tokio::select! {
                biased;
                _ = cancel.wait_for(|v| *v) => return Ok(None),
                connection = tokio::time::timeout(Duration::from_secs(10), zbus::Connection::session()) => connection??,
            };
            // This future runs on the blocking owner via Handle::block_on;
            // Recorder::prepare must not run on an async executor thread.
            let Some(mut session) = screencast::open(connection, cancel.clone()).await? else { return Ok(None) };
            match Recorder::prepare(&session, cancel.clone()) {
                Ok(recorder) => Ok(Some(Capture { recorder: Some(recorder), session: Some(session) })),
                Err(error) => { report_close(session.close().await)?; Err(error) }
            }
        })
    }
    fn start_with<F, Fut>(&self, id: &str, open: F) -> Result<()>
    where
        F: FnOnce(watch::Receiver<bool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<Capture>>> + Send + 'static,
    {
        let mut state = self.lock()?;
        ensure!(!state.closed, "Recordings are shutting down");
        let job = state
            .job
            .as_mut()
            .filter(|j| j.status.id == id)
            .context("Stale recording ID")?;
        if job.status.phase != Phase::Reserved {
            ensure!(
                !matches!(
                    job.status.phase,
                    Phase::Cancelled | Phase::Failed | Phase::Cancelling
                ),
                "Recording has ended"
            );
            return Ok(());
        }
        let receiver = job.receiver.take().context("Recording already started")?;
        let completion = job.completion.take().context("Missing completion owner")?;
        let cancel = job.cancel.subscribe();
        job.status.phase = Phase::Selecting;
        let owner = self.clone();
        let id = id.to_owned();
        let runtime = tokio::runtime::Handle::current();
        tokio::spawn(async move {
            let worker_owner = owner.clone();
            let worker_id = id.clone();
            let result = tokio::task::spawn_blocking(move || {
                let Some(mut capture) = runtime.block_on(open(cancel.clone()))? else {
                    return Ok(false);
                };
                let outcome =
                    worker_owner.run(&worker_id, &mut capture, &runtime, receiver, cancel.clone());
                // Join recorder before closing the portal FD/session on every path.
                // A cancelled metadata probe is cancellation, but remote cleanup
                // failures remain observable even when cancellation was requested.
                capture.close(&runtime)?;
                if *cancel.borrow() {
                    Ok(false)
                } else {
                    outcome
                }
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|r| r);
            let _ = owner.complete(&id, result);
            let _ = completion.send(true);
        });
        Ok(())
    }
    fn run(
        &self,
        id: &str,
        capture: &mut Capture,
        runtime: &tokio::runtime::Handle,
        commands: mpsc::Receiver<Command>,
        cancel: watch::Receiver<bool>,
    ) -> Result<bool> {
        loop {
            if *cancel.borrow() {
                return Ok(false);
            }
            if let Some(recorder) = &capture.recorder {
                self.progress(id, recorder.status())?;
            }
            match commands.recv_timeout(Duration::from_millis(20)) {
                Ok(Command::Snapshot { reply }) => {
                    let outcome = capture
                        .recorder
                        .as_ref()
                        .context("Recording is finalized")
                        .and_then(Recorder::snapshot);
                    let _ = reply.send(if *cancel.borrow() {
                        Err(anyhow::anyhow!("Recording cancelled"))
                    } else {
                        outcome
                    });
                }
                Ok(Command::Control { action, reply }) => {
                    let outcome = match action {
                        Action::SetRegion(region) => capture
                            .recorder
                            .as_ref()
                            .context("Recorder unavailable")
                            .and_then(|recorder| recorder.set_region(region)),
                        Action::Pause => capture
                            .recorder
                            .as_ref()
                            .context("Recording is already finalized")
                            .and_then(Recorder::pause),
                        Action::Resume => capture
                            .recorder
                            .as_ref()
                            .context("Recording is already finalized")
                            .and_then(Recorder::resume),
                        Action::Finish
                            if capture.recorder.as_ref().is_some_and(|r| {
                                r.status().phase == screen_record::Phase::Setup
                            }) =>
                        {
                            Err(anyhow::anyhow!("Start recording before finishing"))
                        }
                        Action::Finish => {
                            if let Some(recorder) = capture.recorder.take() {
                                self.phase(id, Phase::Finalizing)?;
                                let file = recorder.finish()?;
                                // No live portal session survives a ready recording.
                                if let Some(mut session) = capture.session.take() {
                                    close_session(&mut session, runtime)?;
                                }
                                let cancel_file = self
                                    .lock()?
                                    .job
                                    .as_ref()
                                    .filter(|j| j.status.id == id)
                                    .context("Stale recording ID")?
                                    .cancel_file
                                    .clone();
                                let metadata =
                                    crate::media_prepare::inspect_file(&file.path(), &cancel_file)?;
                                let mut state = self.lock()?;
                                let job = state
                                    .job
                                    .as_mut()
                                    .filter(|j| j.status.id == id)
                                    .context("Stale recording ID")?;
                                if *cancel.borrow() {
                                    drop(file);
                                    return Ok(false);
                                }
                                job.status.metadata = Some(metadata);
                                job.status.frames = file.frames;
                                job.file = Some(Arc::new(file));
                                job.status.phase = Phase::Ready;
                            }
                            Ok(())
                        }
                    };
                    if let Some(recorder) = &capture.recorder {
                        self.progress(id, recorder.status())?;
                    }
                    let _ = reply.send(if *cancel.borrow() {
                        Err(anyhow::anyhow!("Recording cancelled"))
                    } else {
                        outcome
                    });
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(false),
            }
        }
    }
    fn phase(&self, id: &str, phase: Phase) -> Result<()> {
        let mut state = self.lock()?;
        let job = state
            .job
            .as_mut()
            .filter(|j| j.status.id == id)
            .context("Stale recording ID")?;
        if !*job.cancel.borrow() {
            job.status.phase = phase;
        }
        Ok(())
    }
    fn progress(&self, id: &str, progress: screen_record::Status) -> Result<()> {
        let mut state = self.lock()?;
        let job = state
            .job
            .as_mut()
            .filter(|j| j.status.id == id)
            .context("Stale recording ID")?;
        if *job.cancel.borrow() {
            return Ok(());
        }
        job.status.region = progress.region;
        job.status.frames = progress.frames;
        job.status.recorded_millis = progress.recorded_millis();
        job.status.phase = match progress.phase {
            screen_record::Phase::Setup => Phase::Setup,
            screen_record::Phase::Recording => Phase::Recording,
            screen_record::Phase::Paused => Phase::Paused,
            screen_record::Phase::LimitReached => Phase::LimitReached,
            screen_record::Phase::Failed(error) => bail!("{error}"),
            _ => bail!("Recording engine stopped unexpectedly"),
        };
        Ok(())
    }
    fn complete(&self, id: &str, result: Result<bool>) -> Result<()> {
        let mut state = self.lock()?;
        let job = state
            .job
            .as_mut()
            .filter(|j| j.status.id == id)
            .context("Stale recording ID")?;
        // Worker has been joined before any terminal state releases admission.
        job.file = None;
        job.status.metadata = None;
        job.status.phase = match result {
            Ok(_) => Phase::Cancelled,
            Err(error) => {
                job.status.message = Some(format!("{error:#}"));
                Phase::Failed
            }
        };
        Ok(())
    }
    /// Bounded JPEG of the last full-source frame; only available on pause.
    pub async fn snapshot(&self, id: &str) -> Result<Vec<u8>> {
        let (reply, response) = oneshot::channel();
        {
            let state = self.lock()?;
            ensure!(!state.closed, "Recordings are shutting down");
            let job = state
                .job
                .as_ref()
                .filter(|job| job.status.id == id)
                .context("Stale recording ID")?;
            ensure!(
                matches!(job.status.phase, Phase::Setup | Phase::Paused),
                "Pause recording before source preview"
            );
            job.commands
                .try_send(Command::Snapshot { reply })
                .context("Recording queue is busy or closed")?;
        }
        response
            .await
            .context("Recording ended before source preview")?
    }
    pub async fn command(&self, id: &str, action: Action) -> Result<()> {
        let (reply, response) = oneshot::channel();
        {
            let state = self.lock()?;
            ensure!(!state.closed, "Recordings are shutting down");
            let job = state
                .job
                .as_ref()
                .filter(|j| j.status.id == id)
                .context("Stale recording ID")?;
            ensure!(
                matches!(
                    job.status.phase,
                    Phase::Setup
                        | Phase::Recording
                        | Phase::Paused
                        | Phase::LimitReached
                        | Phase::Ready
                ),
                "Recording is not ready for controls"
            );
            job.commands
                .try_send(Command::Control { action, reply })
                .context("Recording control queue is busy or closed")?;
        }
        response
            .await
            .context("Recording ended before control acknowledgement; read status")?
    }
    fn cancel_job(job: &mut Job) {
        job.cancel_file.store(true, Ordering::Release);
        job.status.metadata = None;
        let _ = job.cancel.send_replace(true);
        if job.status.phase == Phase::Reserved {
            job.status.phase = Phase::Cancelled;
            job.receiver = None;
            if let Some(completion) = job.completion.take() {
                let _ = completion.send(true);
            }
        } else if !matches!(job.status.phase, Phase::Cancelled | Phase::Failed) {
            job.status.phase = Phase::Cancelling;
        }
    }
    /// Wait for local resource release. Remote Close errors remain in Failed
    /// status and stderr; they do not prevent exit after the FD/worker is gone.
    pub async fn cancel(&self, id: &str) -> Result<()> {
        let mut done = {
            let mut state = self.lock()?;
            let job = state
                .job
                .as_mut()
                .filter(|j| j.status.id == id)
                .context("Stale recording ID")?;
            Self::cancel_job(job);
            job.done.clone()
        };
        done.wait_for(|v| *v)
            .await
            .context("Recording completion owner lost")?;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let id = {
            let mut state = self.lock()?;
            state.closed = true;
            state.job.as_ref().map(|j| j.status.id.clone())
        };
        if let Some(id) = id {
            self.cancel(&id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
