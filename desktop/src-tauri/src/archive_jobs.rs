//! Native file ownership for archive transfers. The webview gets only progress,
//! names and metadata; archive payloads never become a JavaScript Blob.
use crate::archive_messages::ArchiveNotice;
use anyhow::{ensure, Context, Result};
use llcontrol_ipc::{
    client::IpcClient, ProfileArchivePhase as RemotePhase, ProfileArchiveReply as Reply,
    ProfileArchiveRequest as Request, ProfileGroup, MAX_PROFILE_ARCHIVE_BYTES as MAX_BYTES,
    PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    future::Future,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Import,
    Export,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Choosing,
    Preparing,
    Packing,
    Uploading,
    Downloading,
    Committing,
    Recovering,
    Cancelling,
    Cancelled,
    Complete,
    Failed,
    Uncertain,
}
impl Phase {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Complete | Self::Failed | Self::Uncertain
        )
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub mode: Mode,
    pub group: ProfileGroup,
    pub slot: u8,
    pub phase: Phase,
    pub transferred: u64,
    pub total: u64,
    pub file_name: Option<String>,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_code: Option<ArchiveNotice>,
    pub result: Option<Reply>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub id: String,
    pub mode: Mode,
    pub group: ProfileGroup,
    pub slot: u8,
    pub expected_revision: u64,
    pub suggested_name: String,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recovery {
    Status,
    Finish,
    Cancel,
}
struct Job {
    status: Status,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<bool>,
}
#[derive(Default)]
struct State {
    closed: bool,
    job: Option<Job>,
}
#[derive(Clone, Default)]
pub struct Archives(Arc<Mutex<State>>);
impl Archives {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Archive jobs unavailable"))
    }
    pub fn status(&self) -> Result<Option<Status>> {
        Ok(self.lock()?.job.as_ref().map(|j| j.status.clone()))
    }
    pub fn start(
        &self,
        socket: PathBuf,
        request: Start,
        language: llcontrol_ipc::InterfaceLanguage,
    ) -> Result<Status> {
        let choice = request.clone();
        self.start_with(socket, request, move |cancel| {
            crate::archive_chooser::choose(
                choice.mode == Mode::Export,
                choice.suggested_name,
                cancel,
                language,
            )
        })
    }
    fn start_with<F, Fut>(&self, socket: PathBuf, request: Start, picker: F) -> Result<Status>
    where
        F: FnOnce(watch::Receiver<bool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<PathBuf>>> + Send + 'static,
    {
        let runtime =
            tokio::runtime::Handle::try_current().context("Archive runtime unavailable")?;
        request.group.validate_slot(request.slot)?;
        valid_id(&request.id)?;
        ensure!(
            request.suggested_name.len() <= 512,
            "Archive suggested name too long"
        );
        let status = Status {
            id: request.id.clone(),
            mode: request.mode,
            group: request.group,
            slot: request.slot,
            phase: Phase::Choosing,
            transferred: 0,
            total: 0,
            file_name: None,
            message: None,
            message_code: None,
            result: None,
        };
        let (status, signal, done) = self.admit(status)?;
        let owner = self.clone();
        let id = request.id.clone();
        runtime.spawn(async move {
            let worker_owner = owner.clone();
            let result = tokio::spawn(async move {
                let path = picker(signal.clone()).await?;
                let Some(path) = path else {
                    return Ok(Outcome::Cancelled);
                };
                worker_owner.run(socket, request, path, signal).await
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|r| r);
            let _ = done.send(true);
            let _ = owner.finished(&id, result);
        });
        Ok(status)
    }
    fn admit(
        &self,
        status: Status,
    ) -> Result<(Status, watch::Receiver<bool>, watch::Sender<bool>)> {
        let mut state = self.lock()?;
        ensure!(!state.closed, "Archive jobs are closing");
        if let Some(old) = &state.job {
            ensure!(
                old.status.phase.terminal() && *old.done.borrow(),
                "An archive job is still running"
            );
            ensure!(
                old.status.phase != Phase::Uncertain,
                "Read back or dismiss the uncertain archive before starting another"
            );
        }
        let (cancel, signal) = watch::channel(false);
        let (done, completed) = watch::channel(false);
        state.job = Some(Job {
            status: status.clone(),
            cancel,
            done: completed,
        });
        Ok((status, signal, done))
    }
    fn update(&self, id: &str, update: impl FnOnce(&mut Status)) -> Result<()> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("Archive job missing")?;
        ensure!(job.status.id == id, "Stale archive job");
        update(&mut job.status);
        Ok(())
    }
    fn begin_commit(&self, id: &str) -> Result<()> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("Archive job missing")?;
        ensure!(
            job.status.id == id && !*job.cancel.borrow() && job.status.phase != Phase::Cancelling,
            "Archive cancelled before commit"
        );
        job.status.phase = Phase::Committing;
        Ok(())
    }
    fn finished(&self, id: &str, result: Result<Outcome>) -> Result<()> {
        self.update(id, |s| match result {
            Ok(Outcome::Cancelled) => {
                s.phase = Phase::Cancelled;
                s.message = Some(ArchiveNotice::Cancelled.to_string());
                s.message_code = Some(ArchiveNotice::Cancelled);
            }
            Ok(Outcome::Done(reply)) => {
                let reply = *reply;
                s.phase = if reply.state.phase == RemotePhase::Imported
                    || s.mode == Mode::Export && reply.state.phase == RemotePhase::Ready
                {
                    Phase::Complete
                } else if reply.state.phase == RemotePhase::Cancelled {
                    Phase::Cancelled
                } else if reply.state.phase == RemotePhase::Failed {
                    Phase::Failed
                } else {
                    Phase::Uncertain
                };
                s.message_code = None;
                s.message = reply.state.last_error.clone();
                s.result = Some(reply);
            }
            Err(e) => {
                s.phase = if matches!(s.phase, Phase::Choosing | Phase::Preparing) {
                    Phase::Failed
                } else {
                    Phase::Uncertain
                };
                s.message_code = ArchiveNotice::from_error(&e);
                s.message = Some(e.to_string());
            }
        })
    }
    pub fn cancel(&self, id: &str) -> Result<Status> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("No archive job")?;
        ensure!(job.status.id == id, "Stale archive cancellation");
        ensure!(
            !matches!(job.status.phase, Phase::Committing | Phase::Recovering),
            "Commit/readback is in progress; await its result"
        );
        if !job.status.phase.terminal() {
            let _ = job.cancel.send(true);
            job.status.phase = Phase::Cancelling;
        }
        Ok(job.status.clone())
    }
    pub fn dismiss(&self, id: &str) -> Result<()> {
        let mut state = self.lock()?;
        if let Some(job) = &state.job {
            ensure!(
                job.status.id == id && job.status.phase.terminal() && *job.done.borrow(),
                "Archive worker has not finished"
            );
        }
        state.job = None;
        Ok(())
    }
    pub fn recover(
        &self,
        socket: PathBuf,
        id: String,
        action: Recovery,
        group: ProfileGroup,
        slot: u8,
    ) -> Result<Status> {
        let runtime =
            tokio::runtime::Handle::try_current().context("Archive runtime unavailable")?;
        valid_id(&id)?;
        group.validate_slot(slot)?;
        // Explicit recovery may replace an acknowledged terminal/uncertain job.
        {
            let mut state = self.lock()?;
            ensure!(!state.closed, "Archive jobs are closing");
            if let Some(job) = &state.job {
                ensure!(
                    job.status.phase.terminal() && *job.done.borrow(),
                    "Archive worker is still running"
                );
            }
            state.job = None;
        }
        let status = Status {
            id: id.clone(),
            mode: Mode::Import,
            group,
            slot,
            phase: Phase::Recovering,
            transferred: 0,
            total: 0,
            file_name: None,
            message: None,
            message_code: None,
            result: None,
        };
        let (status, _, done) = self.admit(status)?;
        let owner = self.clone();
        runtime.spawn(async move {
            let result = async {
                let client = connect(&socket).await?;
                let request = match action {
                    Recovery::Status => Request::Status { id: id.clone() },
                    Recovery::Finish => Request::Finish { id: id.clone() },
                    Recovery::Cancel => Request::Cancel { id: id.clone() },
                };
                let reply = rpc(&client, request).await?;
                ensure!(reply.state.id == id, "Unexpected archive recovery ID");
                owner.update(&id, |s| {
                    s.group = reply.state.group;
                    s.slot = reply.state.slot;
                    s.total = reply.state.size;
                    s.transferred = reply.state.received;
                    s.mode = if reply.state.direction
                        == llcontrol_ipc::ProfileArchiveDirection::Import
                    {
                        Mode::Import
                    } else {
                        Mode::Export
                    };
                })?;
                // Export Ready after losing its local file is not a saved output.
                if reply.state.phase == RemotePhase::Ready {
                    anyhow::bail!(ArchiveNotice::ProfileExportUnconfirmed);
                }
                Ok(Outcome::Done(Box::new(reply)))
            }
            .await;
            let _ = done.send(true);
            let _ = owner.finished(&id, result);
        });
        Ok(status)
    }
    pub async fn shutdown(&self) -> Result<()> {
        let mut done = {
            let mut state = self.lock()?;
            state.closed = true;
            state.job.as_mut().map(|j| {
                if !matches!(j.status.phase, Phase::Committing | Phase::Recovering) {
                    let _ = j.cancel.send(true);
                }
                j.done.clone()
            })
        };
        if let Some(done) = &mut done {
            if !*done.borrow() {
                done.wait_for(|v| *v)
                    .await
                    .context("Archive completion unavailable")?;
            }
        }
        Ok(())
    }
    async fn run(
        &self,
        socket: PathBuf,
        request: Start,
        path: PathBuf,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<Outcome> {
        let id = request.id.clone();
        self.update(&id, |s| {
            s.phase = Phase::Preparing;
            s.file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        })?;
        if *cancel.borrow() {
            return Ok(Outcome::Cancelled);
        }
        let client = connect(&socket).await?;
        let result = match request.mode {
            Mode::Import => self.upload(&client, &request, path, &mut cancel).await,
            Mode::Export => self.download(&client, &request, path, &mut cancel).await,
        };
        let committing = self
            .status()?
            .is_some_and(|s| matches!(s.phase, Phase::Committing | Phase::Recovering));
        if result.is_err() && !committing {
            // A disconnected/cancelled packing call can still be running in the
            // daemon. Report cancellation only after the server acknowledges it.
            let cleanup = rpc(&client, Request::Cancel { id: id.clone() }).await;
            if *cancel.borrow() {
                if let Ok(reply) = cleanup {
                    if reply.state.phase == RemotePhase::Cancelled {
                        let _ = rpc(&client, Request::Release { id }).await;
                        return Ok(Outcome::Cancelled);
                    }
                    if reply.state.phase == RemotePhase::Imported {
                        return Ok(Outcome::Done(Box::new(reply)));
                    }
                }
                anyhow::bail!(ArchiveNotice::CancelUnconfirmed);
            }
            if cleanup.is_ok() {
                let _ = rpc(&client, Request::Release { id }).await;
            }
        }
        result
    }
    async fn upload(
        &self,
        client: &IpcClient,
        request: &Start,
        path: PathBuf,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<Outcome> {
        let (mut file, size) = tokio::task::spawn_blocking(move || -> Result<_> {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(path)?;
            let metadata = file.metadata()?;
            ensure!(
                metadata.is_file() && (44..=MAX_BYTES).contains(&metadata.len()),
                ArchiveNotice::InvalidArchiveFile
            );
            let mut prefix = [0; 12];
            file.read_exact(&mut prefix)?;
            ensure!(
                &prefix[..8] == b"LCPROF01",
                ArchiveNotice::InvalidProfileArchive
            );
            let length = u32::from_le_bytes(prefix[8..].try_into()?);
            ensure!(
                (1..=1024 * 1024).contains(&length) && 44 + u64::from(length) <= metadata.len(),
                ArchiveNotice::InvalidManifestLength
            );
            file.seek(SeekFrom::Start(0))?;
            Ok((file, metadata.len()))
        })
        .await??;
        let id = request.id.clone();
        let admitted = rpc_cancel(
            client,
            Request::BeginImport {
                id: id.clone(),
                size,
                expected_revision: request.expected_revision,
                group: request.group,
                slot: request.slot,
            },
            cancel,
        )
        .await?;
        ensure!(
            admitted.state.id == id
                && admitted.state.group == request.group
                && admitted.state.slot == request.slot
                && admitted.state.expected_revision == request.expected_revision
                && admitted.state.size == size
                && admitted.state.phase == RemotePhase::Uploading,
            "Unexpected archive import session"
        );
        self.update(&id, |s| {
            s.phase = Phase::Uploading;
            s.total = size;
        })?;
        let mut offset = 0;
        while offset < size {
            ensure!(!*cancel.borrow(), "Archive upload cancelled");
            let count = (size - offset).min(CHUNK as u64) as usize;
            let (returned, bytes) = tokio::task::spawn_blocking(move || -> Result<_> {
                let mut bytes = vec![0; count];
                file.read_exact(&mut bytes)?;
                Ok((file, bytes))
            })
            .await??;
            file = returned;
            let reply = rpc_cancel(
                client,
                Request::Chunk {
                    id: id.clone(),
                    offset,
                    bytes,
                },
                cancel,
            )
            .await?;
            offset += count as u64;
            ensure!(
                reply.state.id == id
                    && reply.state.size == size
                    && reply.state.received == offset
                    && reply.state.phase == RemotePhase::Uploading,
                "Archive upload acknowledgement mismatch"
            );
            self.update(&id, |s| s.transferred = offset)?;
        }
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut tail = [0];
            ensure!(
                file.read(&mut tail)? == 0 && file.metadata()?.len() == size,
                "Archive changed during upload"
            );
            Ok(())
        })
        .await??;
        rpc_cancel(client, Request::Inspect { id: id.clone() }, cancel).await?;
        ensure!(!*cancel.borrow(), "Archive upload cancelled");
        self.begin_commit(&id)?;
        // Finish is deliberately not selected against cancel: its durable
        // receipt is the only authority after publication starts.
        let done = rpc(client, Request::Finish { id: id.clone() }).await?;
        ensure!(done.state.id == id, "Unexpected archive commit ID");
        if done.state.phase == RemotePhase::Imported {
            let _ = rpc(client, Request::Release { id }).await;
        }
        Ok(Outcome::Done(Box::new(done)))
    }
    async fn download(
        &self,
        client: &IpcClient,
        request: &Start,
        path: PathBuf,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<Outcome> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .context("Archive destination needs a directory")?
            .to_owned();
        let destination = path.clone();
        let mut output = tokio::task::spawn_blocking(move || -> Result<_> {
            ensure!(
                std::fs::symlink_metadata(&destination)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                ArchiveNotice::DestinationExists
            );
            Ok(tempfile::NamedTempFile::new_in(parent)?)
        })
        .await??;
        let id = request.id.clone();
        self.update(&id, |s| s.phase = Phase::Packing)?;
        let state = rpc_cancel(
            client,
            Request::BeginExport {
                id: id.clone(),
                expected_revision: request.expected_revision,
                group: request.group,
                slot: request.slot,
            },
            cancel,
        )
        .await?;
        let size = state.state.size;
        ensure!(
            state.state.id == id && state.state.phase == RemotePhase::Ready && size <= MAX_BYTES,
            "Unexpected archive export session"
        );
        self.update(&id, |s| {
            s.phase = Phase::Downloading;
            s.total = size;
        })?;
        let mut offset = 0;
        loop {
            let part = rpc_cancel(
                client,
                Request::Read {
                    id: id.clone(),
                    offset,
                    length: CHUNK,
                },
                cancel,
            )
            .await?;
            ensure!(
                part.state.id == id
                    && part.state.size == size
                    && part.offset == Some(offset)
                    && part.bytes.len() <= CHUNK
                    && part.state.phase == RemotePhase::Ready,
                "Archive download metadata mismatch"
            );
            let end = offset
                .checked_add(part.bytes.len() as u64)
                .context("Archive offset overflow")?;
            ensure!(
                end <= size && part.eof == (end == size) && (!part.bytes.is_empty() || part.eof),
                "Incomplete archive download"
            );
            output = tokio::task::spawn_blocking(move || -> Result<_> {
                output.write_all(&part.bytes)?;
                Ok(output)
            })
            .await??;
            offset = end;
            self.update(&id, |s| s.transferred = offset)?;
            if offset == size {
                break;
            }
        }
        ensure!(!*cancel.borrow(), "Archive export cancelled");
        self.begin_commit(&id)?;
        tokio::task::spawn_blocking(move || -> Result<()> {
            output.as_file().sync_all()?;
            let parent = path
                .parent()
                .context("Missing archive directory")?
                .to_owned();
            output.persist_noclobber(path).map_err(|e| e.error)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        })
        .await??;
        let _ = rpc(client, Request::Cancel { id: id.clone() }).await;
        let _ = rpc(client, Request::Release { id }).await;
        Ok(Outcome::Done(Box::new(state)))
    }
}
enum Outcome {
    Cancelled,
    Done(Box<Reply>),
}
fn valid_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 32
            && id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "Invalid archive operation ID"
    );
    Ok(())
}
async fn connect(socket: &std::path::Path) -> Result<IpcClient> {
    Ok(
        tokio::time::timeout(Duration::from_secs(3), IpcClient::connect(socket))
            .await
            .context("Archive connection timed out")??,
    )
}
async fn rpc(client: &IpcClient, request: Request) -> Result<Reply> {
    let id = match &request {
        Request::BeginImport { id, .. }
        | Request::BeginExport { id, .. }
        | Request::Chunk { id, .. }
        | Request::Read { id, .. }
        | Request::Inspect { id }
        | Request::Status { id }
        | Request::Finish { id }
        | Request::Cancel { id }
        | Request::Release { id } => id.clone(),
    };
    let reply: Reply = serde_json::from_value(
        tokio::time::timeout(
            Duration::from_secs(60),
            client.call("profile_archive", serde_json::json!({"request":request})),
        )
        .await
        .context("Archive request timed out; read back operation ID")??,
    )?;
    ensure!(reply.state.id == id, "Unexpected archive response ID");
    reply.state.group.validate_slot(reply.state.slot)?;
    ensure!(
        (1..=MAX_BYTES).contains(&reply.state.size) && reply.state.received <= reply.state.size,
        "Invalid archive response size"
    );
    if let Some(library) = &reply.state.library {
        library.validate()?;
    }
    if let Some(profile) = &reply.state.profile {
        profile.validate()?;
    }
    Ok(reply)
}
async fn rpc_cancel(
    client: &IpcClient,
    request: Request,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Reply> {
    tokio::select! {biased;_=cancel.wait_for(|v|*v)=>anyhow::bail!("Archive transfer cancelled"),result=rpc(client,request)=>result}
}

#[cfg(test)]
#[path = "archive_jobs_tests.rs"]
mod tests;
