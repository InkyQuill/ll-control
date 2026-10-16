//! Native full-group transfers. Only summaries and progress cross the webview boundary.
use crate::archive_messages::ArchiveNotice;
use anyhow::{ensure, Context, Result};
use lconnect_ipc::{
    client::IpcClient, DeviceId, FullGroupReadback, FullProfileGroup as Group,
    GroupArchivePhase as EP, GroupArchiveReply as ER, GroupArchiveRequest as E,
    GroupArchiveSnapshot, GroupArchiveSummary, GroupUploadPhase as UP, GroupUploadRequest as U,
    GroupUploadState, MAX_PROFILE_ARCHIVE_BYTES as MAX, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::File,
    future::Future,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Import,
    Export,
    Reset,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Choosing,
    Uploading,
    Inspecting,
    Prepared,
    Packing,
    Downloading,
    Applying,
    Recovering,
    Cancelling,
    Cancelled,
    Complete,
    Failed,
    Uncertain,
}
impl Phase {
    pub fn idle(self) -> bool {
        matches!(
            self,
            Self::Prepared | Self::Cancelled | Self::Complete | Self::Failed | Self::Uncertain
        )
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub dev: DeviceId,
    pub group: Group,
    pub mode: Mode,
    pub phase: Phase,
    pub transferred: u64,
    pub total: u64,
    pub file_name: Option<String>,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_code: Option<ArchiveNotice>,
    pub snapshot: Option<GroupArchiveSnapshot>,
    pub summary: Option<GroupArchiveSummary>,
    pub readback: Option<FullGroupReadback>,
    pub sensor_id: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub id: String,
    pub dev: DeviceId,
    pub group: Group,
    pub mode: Mode,
    #[serde(default)]
    pub sensor_id: Option<String>,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Finish,
    Cancel,
    Recover,
    Reapply,
    Dismiss,
}
struct Job {
    status: Status,
    socket: PathBuf,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<bool>,
}
#[derive(Default)]
struct State {
    closed: bool,
    job: Option<Job>,
}
#[derive(Clone, Default)]
pub struct Groups(Arc<Mutex<State>>);
impl Groups {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("Group transfer unavailable"))
    }
    pub fn status(&self) -> Result<Option<Status>> {
        Ok(self.lock()?.job.as_ref().map(|j| j.status.clone()))
    }
    fn update(&self, id: &str, f: impl FnOnce(&mut Status)) -> Result<()> {
        let mut state = self.lock()?;
        let job = state.job.as_mut().context("Missing group job")?;
        ensure!(job.status.id == id, "Stale group operation");
        f(&mut job.status);
        Ok(())
    }
    fn launch<F, Fut>(
        &self,
        socket: PathBuf,
        status: Status,
        replace: bool,
        worker: F,
    ) -> Result<Status>
    where
        F: FnOnce(Self, watch::Receiver<bool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let runtime = tokio::runtime::Handle::try_current()?;
        let (cancel, signal) = watch::channel(false);
        let (done, finished) = watch::channel(false);
        {
            let mut state = self.lock()?;
            ensure!(!state.closed, "Group transfers are closing");
            if let Some(old) = &state.job {
                ensure!(
                    *old.done.borrow() && old.status.phase.idle(),
                    "A group worker is still running"
                );
                if replace {
                    ensure!(
                        old.status.id == status.id
                            && old.status.dev == status.dev
                            && old.status.group == status.group
                            && old.status.mode == status.mode,
                        "Stale group target"
                    );
                } else {
                    ensure!(
                        !matches!(old.status.phase, Phase::Prepared | Phase::Uncertain),
                        "Resolve the previous group operation first"
                    );
                }
            }
            state.job = Some(Job {
                status: status.clone(),
                socket,
                cancel,
                done: finished,
            });
        }
        let owner = self.clone();
        let id = status.id.clone();
        runtime.spawn(async move {
            let worker_owner = owner.clone();
            let result = tokio::spawn(async move { worker(worker_owner, signal).await })
                .await
                .map_err(anyhow::Error::from)
                .and_then(|r| r);
            if let Err(e) = result {
                let _ = owner.update(&id, |s| {
                    s.message_code = ArchiveNotice::from_error(&e);
                    s.message = Some(e.to_string());
                    s.phase = if s.phase == Phase::Choosing
                        || (s.mode == Mode::Reset && s.phase == Phase::Inspecting)
                    {
                        Phase::Failed
                    } else {
                        Phase::Uncertain
                    };
                });
            }
            // Terminal state is not a license to replace a live worker.
            let _ = done.send(true);
        });
        Ok(status)
    }
    pub fn start(
        &self,
        socket: PathBuf,
        request: Start,
        language: lconnect_ipc::InterfaceLanguage,
    ) -> Result<Status> {
        let choice = request.clone();
        self.start_with(socket, request, move |cancel| {
            crate::archive_chooser::choose_group(choice.mode == Mode::Export, cancel, language)
        })
    }
    fn start_with<F, Fut>(&self, socket: PathBuf, request: Start, picker: F) -> Result<Status>
    where
        F: FnOnce(watch::Receiver<bool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<PathBuf>>> + Send + 'static,
    {
        valid_id(&request.id)?;
        ensure!(
            !request.dev.0.is_empty() && request.dev.0.len() <= 512,
            "Invalid device"
        );
        ensure!(
            request.mode != Mode::Reset
                || (request.group == Group::FanAndLighting
                    && request
                        .sensor_id
                        .as_ref()
                        .is_some_and(|s| !s.trim().is_empty()
                            && s.len() <= 1024
                            && !s.chars().any(char::is_control))),
            "Reset requires FanAndLighting and an explicit temperature sensor"
        );
        ensure!(
            request.mode == Mode::Reset || request.sensor_id.is_none(),
            "Sensor is only valid for reset"
        );
        let status = Status {
            id: request.id.clone(),
            dev: request.dev.clone(),
            group: request.group,
            mode: request.mode,
            phase: if request.mode == Mode::Reset {
                Phase::Inspecting
            } else {
                Phase::Choosing
            },
            transferred: 0,
            total: 0,
            file_name: None,
            message: None,
            message_code: None,
            snapshot: None,
            summary: None,
            readback: None,
            sensor_id: request.sensor_id.clone(),
        };
        self.launch(
            socket.clone(),
            status,
            false,
            move |owner, mut cancel| async move {
                let client = connect(&socket).await?;
                if request.mode == Mode::Reset {
                    let preview: lconnect_ipc::ControlResetPreview = call(
                        &client,
                        "preview_fan_and_lighting_reset",
                        json!({"dev":request.dev,"sensor_id":request.sensor_id}),
                    )
                    .await?;
                    ensure!(
                        preview.target.dev == request.dev
                            && preview.target.group == Group::FanAndLighting,
                        "Unexpected reset target"
                    );
                    owner.update(&request.id, |s| {
                        s.phase = if *cancel.borrow() {
                            Phase::Cancelled
                        } else {
                            Phase::Prepared
                        };
                        s.snapshot = Some(preview.target);
                        s.summary = Some(preview.defaults);
                    })?;
                    return Ok(());
                }
                let snapshot = match export_rpc(&client, &request, E::Snapshot).await? {
                    ER::Snapshot { snapshot } => snapshot,
                    _ => anyhow::bail!("Expected group snapshot"),
                };
                ensure!(
                    snapshot.dev == request.dev && snapshot.group == request.group,
                    "Unexpected group target"
                );
                owner.update(&request.id, |s| {
                    s.summary = Some(snapshot.summary.clone());
                    s.snapshot = Some(snapshot.clone());
                })?;
                let Some(path) = picker(cancel.clone()).await? else {
                    owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                    return Ok(());
                };
                owner.update(&request.id, |s| {
                    s.file_name = path.file_name().map(|v| v.to_string_lossy().into_owned())
                })?;
                let result = match request.mode {
                    Mode::Export => {
                        owner
                            .download(&client, &request, &snapshot, path, &mut cancel)
                            .await
                    }
                    Mode::Import => {
                        owner
                            .upload(&client, &request, &snapshot, path, &mut cancel)
                            .await
                    }
                    Mode::Reset => unreachable!(),
                };
                if result.is_err() {
                    // Never claim cancellation until the daemon has finished cleanup.
                    let cleaned = cleanup(&client, &request).await;
                    if *cancel.borrow() && cleaned.is_ok() {
                        owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                        return Ok(());
                    }
                }
                result
            },
        )
    }
    pub fn action(
        &self,
        socket: PathBuf,
        request: Start,
        action: Action,
        expected: Option<String>,
    ) -> Result<Option<Status>> {
        valid_id(&request.id)?;
        ensure!(
            request.mode != Mode::Reset || request.group == Group::FanAndLighting,
            "Reset requires FanAndLighting"
        );
        let old = self.status()?;
        if let Some(s) = &old {
            ensure!(
                s.id == request.id
                    && s.dev == request.dev
                    && s.group == request.group
                    && s.mode == request.mode,
                "Stale group target"
            );
        }
        if matches!(action, Action::Dismiss) {
            let mut state = self.lock()?;
            if let Some(j) = &state.job {
                ensure!(
                    j.status.id == request.id
                        && j.status.dev == request.dev
                        && j.status.group == request.group
                        && j.status.mode == request.mode,
                    "Stale group target"
                );
                ensure!(
                    *j.done.borrow() && j.status.phase.idle() && j.status.phase != Phase::Prepared,
                    "Resolve active/prepared transfer first"
                );
            }
            state.job = None;
            return Ok(None);
        }
        if matches!(action, Action::Cancel) && request.mode == Mode::Reset {
            ensure!(
                old.as_ref().is_some_and(|s| matches!(
                    s.phase,
                    Phase::Prepared | Phase::Inspecting | Phase::Cancelled | Phase::Failed
                )),
                "Reset may already be admitted; read its result instead of cancelling"
            );
        }
        if matches!(action, Action::Cancel) {
            let mut state = self.lock()?;
            if let Some(j) = &mut state.job {
                ensure!(
                    j.status.id == request.id
                        && j.status.dev == request.dev
                        && j.status.group == request.group
                        && j.status.mode == request.mode,
                    "Stale group target"
                );
                if !*j.done.borrow() {
                    ensure!(
                        !j.status.phase.idle(),
                        "Group worker is finishing; retry cancellation"
                    );
                    ensure!(
                        !matches!(j.status.phase, Phase::Applying | Phase::Recovering),
                        "Apply/readback already admitted"
                    );
                    let _ = j.cancel.send(true);
                    j.status.phase = Phase::Cancelling;
                    return Ok(Some(j.status.clone()));
                }
            }
        }
        if matches!(action, Action::Finish) {
            ensure!(
                old.as_ref()
                    .is_some_and(|s| (matches!(s.mode, Mode::Import | Mode::Reset)
                        && s.phase == Phase::Prepared)
                        || (s.mode == Mode::Reset
                            && s.phase == Phase::Uncertain
                            && s.snapshot.is_some()
                            && s.sensor_id.is_some())),
                "Inspect the archive before applying it"
            );
        }
        if matches!(action, Action::Reapply) {
            ensure!(
                expected
                    .as_ref()
                    .is_some_and(|v| !v.is_empty() && v.len() <= 4096),
                "Read the runtime revision first"
            );
            ensure!(
                old.as_ref()
                    .and_then(|s| s.readback.as_ref())
                    .is_some_and(|r| r.selected_content_current && r.runtime.revision == expected),
                "Read current result before reapplying"
            );
        }
        let mut status = old.unwrap_or(Status {
            id: request.id.clone(),
            dev: request.dev.clone(),
            group: request.group,
            mode: request.mode,
            phase: Phase::Recovering,
            transferred: 0,
            total: 0,
            file_name: None,
            message: None,
            message_code: None,
            snapshot: None,
            summary: None,
            readback: None,
            sensor_id: None,
        });
        let reset_parameters = if request.mode == Mode::Reset && matches!(action, Action::Finish) {
            Some((
                status
                    .snapshot
                    .as_ref()
                    .context("Reset target snapshot missing")?
                    .expected
                    .clone(),
                status.sensor_id.clone().context("Reset sensor missing")?,
            ))
        } else {
            None
        };
        status.phase = match action {
            Action::Finish | Action::Reapply => Phase::Applying,
            Action::Cancel => Phase::Cancelling,
            _ => Phase::Recovering,
        };
        status.message = None;
        status.message_code = None;
        let started = self.launch(socket.clone(), status, true, move |owner, _| async move {
            let client = connect(&socket).await?;
            match action {
                Action::Finish if request.mode == Mode::Reset => {
                    let (expected, sensor) = reset_parameters.context("Reset preparation missing")?;
                    let result: lconnect_ipc::FullGroupCommit = call(&client,"reset_fan_and_lighting",json!({"dev":request.dev,"operation_id":request.id,"expected":expected,"sensor_id":sensor})).await?;
                    owner.accept_readback(&request,result.readback)?;
                }
                Action::Finish => {
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
                    let mut state = upload_rpc(&client, &request, U::Finish { id: request.id.clone() }).await?;
                    while state.phase == UP::Importing {
                        ensure!(tokio::time::Instant::now() < deadline, "Apply result still pending; read operation ID");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        state = upload_rpc(&client, &request, U::Status { id: request.id.clone() }).await?;
                    }
                    if matches!(state.phase, UP::Imported | UP::Failed | UP::Cancelled) {
                        let _ = upload_rpc(&client, &request, U::Release { id: request.id.clone() }).await;
                    }
                    owner.accept_upload(&request.id, state)?;
                }
                Action::Cancel => {
                    if request.mode != Mode::Reset { cleanup(&client, &request).await?; }
                    owner.update(&request.id, |s| s.phase = Phase::Cancelled)?;
                }
                Action::Recover => {
                    ensure!(request.mode != Mode::Export, ArchiveNotice::GroupExportUnconfirmed);
                    // A durable receipt survives session Release and daemon restart.
                    let receipt = call::<FullGroupReadback>(&client, "read_group_import", json!({"dev":request.dev,"group":request.group,"operation_id":request.id})).await;
                    match receipt {
                        Ok(readback) => owner.accept_readback(&request, readback)?,
                        Err(receipt_error) => {
                            ensure!(request.mode != Mode::Reset, "Reset receipt unavailable: {receipt_error}. No reset was replayed; inspect current settings before a new reset.");
                            let state = upload_rpc(&client, &request, U::Status { id: request.id.clone() }).await.with_context(|| format!("Receipt: {receipt_error}"))?;
                            owner.accept_upload(&request.id, state)?;
                        }
                    }
                }
                Action::Reapply => {
                    let readback = call::<FullGroupReadback>(&client, "reapply_group_import", json!({"dev":request.dev,"group":request.group,"operation_id":request.id,"expected":expected})).await?;
                    owner.accept_readback(&request, readback)?;
                }
                Action::Dismiss => unreachable!(),
            }
            Ok(())
        })?;
        Ok(Some(started))
    }
    fn accept_readback(&self, request: &Start, r: FullGroupReadback) -> Result<()> {
        ensure!(
            r.operation_id == request.id && r.dev == request.dev && r.group == request.group,
            "Unexpected group receipt"
        );
        self.update(&request.id, |s| {
            s.phase = Phase::Complete;
            s.summary = Some(r.summary.clone());
            s.message_code = None;
            s.message = r.runtime.last_error.clone();
            s.readback = Some(r);
        })
    }
    fn accept_upload(&self, id: &str, s: GroupUploadState) -> Result<()> {
        if s.phase == UP::Prepared {
            ensure!(
                s.summary.is_some() && s.source_sha256.is_some(),
                "Missing inspected group summary"
            );
        }
        if s.phase == UP::Imported {
            ensure!(
                s.committed.is_some(),
                "Imported group has no acknowledgement"
            );
        }
        if let Some(c) = &s.committed {
            ensure!(
                c.readback.operation_id == id
                    && c.readback.dev == s.target.dev
                    && c.readback.group == s.target.group,
                "Unexpected committed group receipt"
            );
        }
        self.update(id, |status| {
            status.phase = match s.phase {
                UP::Prepared => Phase::Prepared,
                UP::Imported => Phase::Complete,
                UP::Failed => Phase::Failed,
                UP::Cancelled => Phase::Cancelled,
                _ => Phase::Uncertain,
            };
            status.transferred = s.received;
            status.total = s.size;
            status.summary = s.summary;
            status.message_code = None;
            status.message = s.last_error;
            status.readback = s.committed.map(|c| c.readback);
            if status.message.is_none() {
                status.message = status
                    .readback
                    .as_ref()
                    .and_then(|r| r.runtime.last_error.clone());
            }
        })
    }
    fn begin_publish(&self, id: &str) -> Result<()> {
        let mut state = self.lock()?;
        let j = state.job.as_mut().context("Missing group job")?;
        ensure!(
            j.status.id == id && !*j.cancel.borrow(),
            "Group transfer cancelled before publication"
        );
        j.status.phase = Phase::Applying;
        Ok(())
    }
    async fn download(
        &self,
        client: &IpcClient,
        request: &Start,
        snapshot: &GroupArchiveSnapshot,
        path: PathBuf,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        self.update(&request.id, |s| s.phase = Phase::Packing)?;
        let parent = path
            .parent()
            .context("Destination directory missing")?
            .to_owned();
        let destination = path.clone();
        let mut output = tokio::task::spawn_blocking(move || -> Result<_> {
            ensure!(
                std::fs::symlink_metadata(&destination)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                "Choose a new file name; existing files are not replaced"
            );
            Ok(tempfile::NamedTempFile::new_in(parent)?)
        })
        .await??;
        let mut reply = export_rpc(
            client,
            request,
            E::BeginExport {
                id: request.id.clone(),
                expected: snapshot.expected.clone(),
            },
        )
        .await?;
        let size = loop {
            ensure!(!*cancel.borrow(), "Group export cancelled");
            let ER::Transfer { state, .. } = &reply else {
                anyhow::bail!("Expected export state")
            };
            ensure!(
                state.snapshot == *snapshot && state.id == request.id,
                "Export target changed"
            );
            if state.phase == EP::Ready {
                break state.size.context("Missing export size")?;
            }
            ensure!(
                state.phase == EP::Preparing,
                "{}",
                state.last_error.as_deref().unwrap_or("Export failed")
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
            reply = export_rpc(
                client,
                request,
                E::Status {
                    id: request.id.clone(),
                },
            )
            .await?;
        };
        ensure!((44..=MAX).contains(&size), "Invalid group archive size");
        self.update(&request.id, |s| {
            s.phase = Phase::Downloading;
            s.total = size;
        })?;
        let mut offset = 0;
        let mut magic: Vec<u8> = Vec::with_capacity(8);
        while offset < size {
            ensure!(!*cancel.borrow(), "Group export cancelled");
            let ER::Transfer {
                state,
                offset: start,
                bytes,
                eof,
            } = export_rpc(
                client,
                request,
                E::Read {
                    id: request.id.clone(),
                    offset,
                    length: CHUNK,
                },
            )
            .await?
            else {
                anyhow::bail!("Expected export data")
            };
            let end = offset
                .checked_add(bytes.len() as u64)
                .context("Offset overflow")?;
            ensure!(
                state.id == request.id
                    && state.snapshot == *snapshot
                    && state.phase == EP::Ready
                    && state.size == Some(size)
                    && start == Some(offset)
                    && !bytes.is_empty()
                    && bytes.len() <= CHUNK
                    && end <= size
                    && eof == (end == size),
                "Invalid group download response"
            );
            magic.extend(bytes.iter().take(8 - magic.len()).copied());
            output = tokio::task::spawn_blocking(move || -> Result<_> {
                output.write_all(&bytes)?;
                Ok(output)
            })
            .await??;
            offset = end;
            self.update(&request.id, |s| s.transferred = offset)?;
        }
        ensure!(magic == b"LCGROUP1", "Unexpected full-group archive format");
        self.begin_publish(&request.id)?;
        tokio::task::spawn_blocking(move || -> Result<()> {
            output.as_file().sync_all()?;
            let parent = path
                .parent()
                .context("Destination directory missing")?
                .to_owned();
            output.persist_noclobber(path).map_err(|e| e.error)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        })
        .await??;
        // The output is already durable; remote cleanup errors cannot undo that fact.
        let _ = cleanup(client, request).await;
        self.update(&request.id, |s| s.phase = Phase::Complete)
    }
    async fn upload(
        &self,
        client: &IpcClient,
        request: &Start,
        snapshot: &GroupArchiveSnapshot,
        path: PathBuf,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<()> {
        self.update(&request.id, |s| s.phase = Phase::Uploading)?;
        let (mut input, size) = tokio::task::spawn_blocking(move || -> Result<_> {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            let m = file.metadata()?;
            ensure!(
                m.is_file() && (44..=MAX).contains(&m.len()),
                "Choose a regular full-group archive file"
            );
            let mut magic = [0; 8];
            file.read_exact(&mut magic)?;
            ensure!(&magic == b"LCGROUP1", "Choose an LCGROUP1 archive (.lcg)");
            use std::io::Seek;
            file.rewind()?;
            Ok((file, m.len()))
        })
        .await??;
        self.update(&request.id, |s| {
            s.total = size;
            s.summary = None;
        })?;
        let begun = upload_rpc(
            client,
            request,
            U::Begin {
                id: request.id.clone(),
                size,
                expected: snapshot.expected.clone(),
            },
        )
        .await?;
        ensure!(
            begun.received == 0 && begun.size == size,
            "Unexpected upload start"
        );
        let mut offset = 0;
        while offset < size {
            ensure!(!*cancel.borrow(), "Group upload cancelled");
            let count = (size - offset).min(CHUNK as u64) as usize;
            let (file, bytes) = tokio::task::spawn_blocking(move || -> Result<_> {
                let mut bytes = vec![0; count];
                input.read_exact(&mut bytes)?;
                Ok((input, bytes))
            })
            .await??;
            input = file;
            let state = upload_rpc(
                client,
                request,
                U::Chunk {
                    id: request.id.clone(),
                    offset,
                    bytes,
                },
            )
            .await?;
            offset += count as u64;
            ensure!(
                state.received == offset && state.size == size && state.phase == UP::Uploading,
                "Upload offset mismatch"
            );
            self.update(&request.id, |s| s.transferred = offset)?;
        }
        // Close the granted file off the async runtime before reporting Prepared.
        tokio::task::spawn_blocking(move || drop(input)).await?;
        ensure!(!*cancel.borrow(), "Group upload cancelled");
        self.update(&request.id, |s| s.phase = Phase::Inspecting)?;
        let mut state = upload_rpc(
            client,
            request,
            U::Inspect {
                id: request.id.clone(),
            },
        )
        .await?;
        while state.phase == UP::Inspecting {
            ensure!(!*cancel.borrow(), "Inspection cancelled");
            tokio::time::sleep(Duration::from_millis(100)).await;
            state = upload_rpc(
                client,
                request,
                U::Status {
                    id: request.id.clone(),
                },
            )
            .await?;
        }
        if matches!(state.phase, UP::Failed | UP::Cancelled) {
            let _ = upload_rpc(
                client,
                request,
                U::Release {
                    id: request.id.clone(),
                },
            )
            .await;
        }
        self.accept_upload(&request.id, state)
    }
    pub async fn shutdown(&self) -> Result<()> {
        let job = {
            let mut state = self.lock()?;
            state.closed = true;
            state.job.as_ref().map(|j| {
                if !matches!(j.status.phase, Phase::Applying | Phase::Recovering) {
                    let _ = j.cancel.send(true);
                }
                (j.done.clone(), j.socket.clone(), j.status.clone())
            })
        };
        if let Some((mut done, socket, status)) = job {
            if !*done.borrow() {
                done.wait_for(|v| *v)
                    .await
                    .context("Group worker completion unavailable")?;
            }
            if self
                .status()?
                .is_some_and(|s| s.phase == Phase::Prepared && s.mode != Mode::Reset)
            {
                let client = connect(&socket).await?;
                cleanup(
                    &client,
                    &Start {
                        id: status.id,
                        dev: status.dev,
                        group: status.group,
                        mode: status.mode,
                        sensor_id: None,
                    },
                )
                .await?;
            }
        }
        Ok(())
    }
}
fn valid_id(id: &str) -> Result<()> {
    ensure!(
        id.len() == 32
            && id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "Invalid group operation ID"
    );
    Ok(())
}
async fn connect(socket: &Path) -> Result<IpcClient> {
    Ok(tokio::time::timeout(Duration::from_secs(3), IpcClient::connect(socket)).await??)
}
async fn call<T: serde::de::DeserializeOwned>(
    client: &IpcClient,
    method: &str,
    params: serde_json::Value,
) -> Result<T> {
    Ok(serde_json::from_value(
        tokio::time::timeout(Duration::from_secs(60), client.call(method, params))
            .await
            .context("Group request timed out; read operation ID")??,
    )?)
}
async fn export_rpc(client: &IpcClient, r: &Start, request: E) -> Result<ER> {
    call(
        client,
        "group_archive",
        json!({"dev":r.dev,"group":r.group,"request":request}),
    )
    .await
}
async fn upload_rpc(client: &IpcClient, r: &Start, request: U) -> Result<GroupUploadState> {
    let s: GroupUploadState = call(
        client,
        "group_upload",
        json!({"dev":r.dev,"group":r.group,"request":request}),
    )
    .await?;
    ensure!(
        s.id == r.id
            && s.target.dev == r.dev
            && s.target.group == r.group
            && s.received <= s.size
            && (44..=MAX).contains(&s.size),
        "Unexpected group upload target"
    );
    Ok(s)
}
async fn cleanup(client: &IpcClient, r: &Start) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(60), cleanup_inner(client, r))
        .await
        .context("Group cleanup still pending; read operation ID")?
}
async fn cleanup_inner(client: &IpcClient, r: &Start) -> Result<()> {
    match r.mode {
        Mode::Reset => anyhow::bail!("Reset has no transfer to cancel; read its result"),
        Mode::Import => {
            loop {
                let state = upload_rpc(client, r, U::Cancel { id: r.id.clone() }).await?;
                ensure!(
                    !matches!(
                        state.phase,
                        UP::Importing | UP::Imported | UP::RecoveryRequired
                    ),
                    "Apply cannot be cancelled; read the result"
                );
                if matches!(state.phase, UP::Cancelled | UP::Failed) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let _ = upload_rpc(client, r, U::Release { id: r.id.clone() }).await?;
        }
        Mode::Export => {
            loop {
                let ER::Transfer { state, .. } =
                    export_rpc(client, r, E::Cancel { id: r.id.clone() }).await?
                else {
                    anyhow::bail!("Expected export cancellation")
                };
                ensure!(
                    state.id == r.id
                        && state.snapshot.dev == r.dev
                        && state.snapshot.group == r.group,
                    "Unexpected cancellation target"
                );
                if matches!(state.phase, EP::Cancelled | EP::Failed) {
                    break;
                }
                ensure!(state.phase == EP::Cancelling, "Export cancellation failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let _ = export_rpc(client, r, E::Release { id: r.id.clone() }).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "group_jobs_tests.rs"]
mod tests;
