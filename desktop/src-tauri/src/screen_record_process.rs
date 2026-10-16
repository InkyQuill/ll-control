//! Killable GStreamer boundary. The parent owns portal permissions and staging.
//! The worker inherits only its private control socket and selected PipeWire FD.
use crate::{
    media_prepare::VideoCrop,
    screen_record::{self, Phase, RecordingFile, Status},
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::sync::watch;

const WORKER_FLAG: &str = "--internal-screen-record-worker";
const MAX_MESSAGE: usize = 9 * 1024 * 1024; // bounded 2MiB JPEG as JSON bytes
const IO_TICK: Duration = Duration::from_millis(25);
const COMMAND_LIMIT: Duration = Duration::from_secs(5);
const START_LIMIT: Duration = Duration::from_secs(15);
const FINISH_LIMIT: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Init {
    directory: PathBuf,
    source: Source,
    setup: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Source {
    Portal {
        node: u32,
        serial: Option<u64>,
    },
    #[cfg(any(test, feature = "native-smoke"))]
    Synthetic,
    #[cfg(test)]
    Fault(String),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Request {
    Status,
    Pause,
    Resume,
    Region(VideoCrop),
    Snapshot,
    Finish,
    Cancel,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Reply {
    Status(Status),
    Ack,
    Snapshot(Vec<u8>),
    Finished { frames: u64, length: u64 },
    Error(String),
}

struct Worker {
    child: Child,
    socket: UnixStream,
    stopped: bool,
    failure: Option<String>,
}
impl Worker {
    fn reap(&mut self) -> Result<()> {
        if !self.stopped {
            if self.child.try_wait()?.is_none() {
                self.child.kill()?;
            }
            self.child.wait()?;
            self.stopped = true;
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(error) = self.reap() {
            eprintln!("Recording worker reap failed: {error:#}");
        }
    }
}

pub struct Recorder {
    worker: Mutex<Worker>,
    directory: Option<TempDir>,
    cancel: Option<watch::Receiver<bool>>,
    closed: Option<watch::Receiver<bool>>,
}
impl Recorder {
    pub fn prepare(
        session: &crate::screencast::Session,
        cancel: watch::Receiver<bool>,
    ) -> Result<Self> {
        ensure!(!*session.closed().borrow(), "ScreenCast session is closed");
        let fd = session
            .remote_fd()
            .context("Missing PipeWire descriptor")?
            .try_clone_to_owned()?;
        Self::open(
            Source::Portal {
                node: session.node_id,
                serial: session.serial,
            },
            Some(fd),
            true,
            Some(cancel),
            Some(session.closed()),
        )
    }
    #[cfg(test)]
    pub(crate) fn synthetic_prepare() -> Result<Self> {
        Self::open(Source::Synthetic, None, true, None, None)
    }
    #[cfg(any(test, feature = "native-smoke"))]
    pub(crate) fn synthetic_prepare_with_cancel(cancel: watch::Receiver<bool>) -> Result<Self> {
        Self::open(Source::Synthetic, None, true, Some(cancel), None)
    }
    #[cfg(test)]
    pub(crate) fn synthetic_with_cancel(cancel: watch::Receiver<bool>) -> Result<Self> {
        Self::open(Source::Synthetic, None, false, Some(cancel), None)
    }
    #[cfg(test)]
    pub(crate) fn stalled_for_test(cancel: watch::Receiver<bool>) -> Result<Self> {
        Self::open(
            Source::Fault("command".into()),
            None,
            true,
            Some(cancel),
            None,
        )
    }
    #[cfg(test)]
    pub(crate) fn worker_for_test(&self) -> (u32, PathBuf) {
        (
            self.worker.lock().unwrap().child.id(),
            self.directory.as_ref().unwrap().path().to_owned(),
        )
    }
    #[cfg(test)]
    pub(crate) fn synthetic() -> Result<Self> {
        Self::open(Source::Synthetic, None, false, None, None)
    }

    fn open(
        source: Source,
        fd: Option<OwnedFd>,
        setup: bool,
        cancel: Option<watch::Receiver<bool>>,
        closed: Option<watch::Receiver<bool>>,
    ) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let (socket, child_socket) = UnixStream::pair()?;
        socket.set_read_timeout(Some(IO_TICK))?;
        socket.set_write_timeout(Some(IO_TICK))?;
        // Move inherited sources above the fixed destinations before pre_exec,
        // preventing dup2(3)/dup2(4) from overwriting another source descriptor.
        let control_fd = rustix::io::fcntl_dupfd_cloexec(&child_socket, 10)?;
        let portal_fd = fd
            .as_ref()
            .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 10))
            .transpose()?;
        let control_raw = control_fd.as_raw_fd();
        let portal_raw = portal_fd.as_ref().map(AsRawFd::as_raw_fd);
        let parent = std::process::id() as libc::pid_t;
        let mut command = Command::new(std::env::current_exe()?);
        #[cfg(not(test))]
        command.arg(WORKER_FLAG);
        #[cfg(test)]
        command
            .args([
                "--exact",
                "screen_record_process::tests::worker_entry",
                "--nocapture",
            ])
            .env("LCONNECT_TEST_RECORD_WORKER", "1");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        // SAFETY: only async-signal-safe syscalls run between fork and exec.
        // Owned CLOEXEC descriptors remain alive until spawn has completed.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(control_raw, 3) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if let Some(fd) = portal_raw {
                    if libc::dup2(fd, 4) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                } else {
                    libc::close(4);
                }
                // CLOEXEC preserves std's exec-error pipe until exec while
                // excluding descriptors opened by native GUI libraries.
                if libc::close_range(5, u32::MAX, libc::CLOSE_RANGE_CLOEXEC as i32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                }
                Ok(())
            });
        }
        let child = command.spawn().context("Start isolated recording worker")?;
        drop((child_socket, control_fd, portal_fd, fd));
        let recorder = Self {
            worker: Mutex::new(Worker {
                child,
                socket,
                stopped: false,
                failure: None,
            }),
            directory: Some(directory),
            cancel,
            closed,
        };
        let init = Init {
            directory: recorder
                .directory
                .as_ref()
                .context("Missing recording staging")?
                .path()
                .to_owned(),
            source,
            setup,
        };
        let reply = recorder.checked_exchange(&init, START_LIMIT)?;
        ensure!(
            matches!(reply, Reply::Status(_)),
            "Invalid recording startup acknowledgement"
        );
        Ok(recorder)
    }
    fn interrupted(&self) -> Result<()> {
        ensure!(
            !self.cancel.as_ref().is_some_and(|v| *v.borrow()),
            "Recording cancelled"
        );
        ensure!(
            !self.closed.as_ref().is_some_and(|v| *v.borrow()),
            "ScreenCast session closed while recording"
        );
        Ok(())
    }
    fn exchange(&self, message: &impl Serialize, limit: Duration) -> Result<Reply> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow::anyhow!("Recording worker state unavailable"))?;
        if let Some(error) = &worker.failure {
            bail!("{error}");
        }
        let deadline = Instant::now() + limit;
        let result = (|| {
            let check = || {
                self.interrupted()?;
                ensure!(
                    Instant::now() < deadline,
                    "Recording worker response timed out"
                );
                Ok(())
            };
            write_message(&mut worker.socket, message, &check)?;
            let reply: Reply = read_message(&mut worker.socket, &check)?;
            Ok(reply)
        })();
        if let Err(error) = &result {
            worker.failure = Some(format!("{error:#}"));
            worker.reap().context("Join failed recording worker")?;
        }
        result
    }
    fn checked_exchange(&self, message: &impl Serialize, limit: Duration) -> Result<Reply> {
        match self.exchange(message, limit)? {
            Reply::Error(error) => Err(anyhow::anyhow!(error)),
            reply => Ok(reply),
        }
    }
    pub fn status(&self) -> Status {
        match self.checked_exchange(&Request::Status, COMMAND_LIMIT) {
            Ok(Reply::Status(status)) => status,
            result => Status {
                phase: Phase::Failed(format!(
                    "{}",
                    result
                        .err()
                        .unwrap_or_else(|| anyhow::anyhow!("Invalid recording status reply"))
                )),
                frames: 0,
                active_millis: 0,
                region: None,
            },
        }
    }
    fn control(&self, request: Request) -> Result<()> {
        ensure!(
            matches!(self.checked_exchange(&request, COMMAND_LIMIT)?, Reply::Ack),
            "Invalid recording command acknowledgement"
        );
        Ok(())
    }
    pub fn pause(&self) -> Result<()> {
        self.control(Request::Pause)
    }
    pub fn resume(&self) -> Result<()> {
        self.control(Request::Resume)
    }
    pub fn set_region(&self, region: VideoCrop) -> Result<()> {
        region.validate()?;
        self.control(Request::Region(region))
    }
    pub fn snapshot(&self) -> Result<Vec<u8>> {
        match self.checked_exchange(&Request::Snapshot, COMMAND_LIMIT)? {
            Reply::Snapshot(bytes) => {
                ensure!(
                    bytes.len() <= 2 * 1024 * 1024,
                    "Recording snapshot exceeds limit"
                );
                Ok(bytes)
            }
            _ => bail!("Invalid recording snapshot reply"),
        }
    }
    pub fn finish(mut self) -> Result<RecordingFile> {
        let Reply::Finished { frames, length } =
            self.checked_exchange(&Request::Finish, FINISH_LIMIT)?
        else {
            bail!("Invalid recording finish reply");
        };
        {
            let mut worker = self
                .worker
                .lock()
                .map_err(|_| anyhow::anyhow!("Recording worker state unavailable"))?;
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                self.interrupted()?;
                if let Some(status) = worker.child.try_wait()? {
                    worker.stopped = true;
                    ensure!(
                        status.success(),
                        "Recording worker failed during finalization"
                    );
                    break;
                }
                ensure!(Instant::now() < deadline, "Recording worker exit timed out");
                std::thread::sleep(IO_TICK);
            }
        }
        let directory = self
            .directory
            .take()
            .context("Recording staging unavailable")?;
        let metadata = std::fs::symlink_metadata(directory.path().join("recording.mp4"))?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() == length
                && length > 0
                && length <= 256 * 1024 * 1024
                && frames > 0
                && frames <= 4320,
            "Invalid finalized recording"
        );
        Ok(RecordingFile::from_directory(directory, frames, length))
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        // SIGKILL does not wait for any plugin callback or GStreamer teardown.
        // Always reap before releasing staging or the parent's portal session.
        match self.worker.get_mut() {
            Ok(worker) => {
                if let Err(error) = worker.reap() {
                    eprintln!("Recording cleanup failed: {error:#}");
                }
            }
            Err(poisoned) => {
                let _ = poisoned.into_inner().reap();
            }
        }
    }
}
fn transfer(
    stream: &mut UnixStream,
    bytes: &mut [u8],
    writing: bool,
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        check()?;
        let result = if writing {
            stream.write(&bytes[offset..])
        } else {
            stream.read(&mut bytes[offset..])
        };
        match result {
            Ok(0) => bail!("Recording worker connection closed"),
            Ok(n) => offset += n,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
fn write_message(
    stream: &mut UnixStream,
    message: &impl Serialize,
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(message)?;
    ensure!(
        bytes.len() <= MAX_MESSAGE,
        "Recording worker message exceeds limit"
    );
    transfer(stream, &mut (bytes.len() as u32).to_be_bytes(), true, check)?;
    transfer(stream, &mut bytes, true, check)
}
fn read_message<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
    check: &impl Fn() -> Result<()>,
) -> Result<T> {
    let mut header = [0; 4];
    transfer(stream, &mut header, false, check)?;
    let length = u32::from_be_bytes(header) as usize;
    ensure!(
        length > 0 && length <= MAX_MESSAGE,
        "Invalid recording worker message size"
    );
    let mut bytes = vec![0; length];
    transfer(stream, &mut bytes, false, check)?;
    serde_json::from_slice(&bytes).context("Invalid recording worker message")
}

/// Run before any GUI, single-instance plugin or runtime initialization.
pub fn dispatch_worker() -> bool {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new(WORKER_FLAG)) {
        let result = worker_main();
        if let Err(error) = &result {
            eprintln!("Recording worker failed: {error:#}");
        }
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    false
}
fn validate_socket(fd: libc::c_int) -> Result<()> {
    for (option, expected) in [
        (libc::SO_DOMAIN, libc::AF_UNIX),
        (libc::SO_TYPE, libc::SOCK_STREAM),
    ] {
        let mut value: libc::c_int = 0;
        let mut length = std::mem::size_of_val(&value) as libc::socklen_t;
        // SAFETY: writable stack storage of the given size. getsockopt safely
        // rejects invalid descriptors before any OwnedFd is constructed.
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                (&mut value as *mut libc::c_int).cast(),
                &mut length,
            )
        };
        ensure!(
            result == 0 && length as usize == std::mem::size_of_val(&value) && value == expected,
            "Missing private recording control socket"
        );
    }
    Ok(())
}
fn worker_main() -> Result<()> {
    validate_socket(3)?;
    // SAFETY: this mode is entered once, before GUI startup. Descriptor3 was
    // installed by the parent pre_exec; ownership is transferred to this scope.
    let mut socket = unsafe { UnixStream::from_raw_fd(3) };
    let init: Init = read_message(&mut socket, &|| Ok(()))?;
    #[cfg(test)]
    if let Source::Fault(ref mode) = init.source {
        return tests::fault_worker(&mut socket, &init, mode);
    }
    let (fd, node, serial, synthetic) = match init.source {
        Source::Portal { node, serial } => {
            // Check even manual internal-mode invocations before taking ownership.
            ensure!(
                unsafe { libc::fcntl(4, libc::F_GETFD) } >= 0,
                "Missing inherited PipeWire descriptor"
            );
            // SAFETY: FD4 is open and this process owns its inherited copy.
            (
                Some(unsafe { OwnedFd::from_raw_fd(4) }),
                node,
                serial,
                false,
            )
        }
        #[cfg(any(test, feature = "native-smoke"))]
        Source::Synthetic => (None, 0, None, true),
        #[cfg(test)]
        Source::Fault(_) => unreachable!(),
    };
    let result = screen_record::Recorder::prepare_worker(
        fd,
        node,
        serial,
        init.directory.clone(),
        synthetic,
        init.setup,
    );
    let recorder = match result {
        Ok(recorder) => recorder,
        Err(error) => {
            write_message(&mut socket, &Reply::Error(format!("{error:#}")), &|| Ok(()))?;
            return Err(error);
        }
    };
    write_message(&mut socket, &Reply::Status(recorder.status()), &|| Ok(()))?;
    loop {
        let request: Request = read_message(&mut socket, &|| Ok(()))?;
        let outcome = match request {
            Request::Status => Ok(Reply::Status(recorder.status())),
            Request::Pause => recorder.pause().map(|_| Reply::Ack),
            Request::Resume => recorder.resume().map(|_| Reply::Ack),
            Request::Region(region) => recorder.set_region(region).map(|_| Reply::Ack),
            Request::Snapshot => recorder.snapshot().map(Reply::Snapshot),
            Request::Cancel => {
                drop(recorder);
                write_message(&mut socket, &Reply::Ack, &|| Ok(()))?;
                return Ok(());
            }
            Request::Finish => {
                let result = recorder.finish().and_then(|file| {
                    std::fs::rename(file.path(), init.directory.join("recording.mp4"))?;
                    Ok(Reply::Finished {
                        frames: file.frames,
                        length: file.length(),
                    })
                });
                let reply = result.unwrap_or_else(|error| {
                    Reply::Error(format!("{error:#}").chars().take(2048).collect())
                });
                write_message(&mut socket, &reply, &|| Ok(()))?;
                return Ok(());
            }
        };
        let reply = outcome.unwrap_or_else(|error| {
            Reply::Error(format!("{error:#}").chars().take(2048).collect())
        });
        write_message(&mut socket, &reply, &|| Ok(()))?;
    }
}

#[cfg(test)]
mod tests;
