//! Shared Unix socket path resolution and exclusive daemon ownership.
//!
//! The daemon must hold [`DaemonOwnership`] before it opens persistence or
//! HID. The lock lives beside the socket, and the socket file is removed
//! only after a refused connection proves it stale — or, at shutdown, only
//! when its inode is still the one this process bound.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a connected peer may take to send `hello` before it is treated
/// as a live occupant. A refused connection is stale immediately.
const LIVE_SOCKET_PROBE_TIMEOUT: Duration = Duration::from_millis(300);

const LOCK_FILE_NAME: &str = "lconnectd.lock";

/// Return the well-known daemon socket inside an already-resolved runtime directory.
#[must_use]
pub fn socket_path_from_runtime(runtime: &Path) -> PathBuf {
    runtime.join("lconnectd.sock")
}

/// Resolve the daemon socket from `XDG_RUNTIME_DIR`.
///
/// # Errors
///
/// Returns an error when `XDG_RUNTIME_DIR` is unset, is not an absolute path,
/// or does not identify an existing directory.
pub fn default_socket_path() -> io::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR is unset; cannot resolve lconnectd socket",
        )
    })?;
    socket_path_for_runtime(&PathBuf::from(runtime))
}

fn socket_path_for_runtime(runtime: &Path) -> io::Result<PathBuf> {
    if !runtime.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "XDG_RUNTIME_DIR must be an absolute path, got {}",
                runtime.display()
            ),
        ));
    }

    let metadata = std::fs::metadata(runtime).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "XDG_RUNTIME_DIR does not identify an accessible directory ({}): {error}",
                runtime.display()
            ),
        )
    })?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "XDG_RUNTIME_DIR must identify a directory, got {}",
                runtime.display()
            ),
        ));
    }

    Ok(socket_path_from_runtime(runtime))
}

/// Identity of the socket inode this daemon bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketIdentity {
    dev: u64,
    ino: u64,
}

impl SocketIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }

    /// Stat `path` without following a final symlink.
    ///
    /// # Errors
    ///
    /// Returns an error when the path cannot be stat'ed.
    pub fn of_path(path: &Path) -> io::Result<Self> {
        Ok(Self::from_metadata(&std::fs::symlink_metadata(path)?))
    }
}

/// Exclusive right to be the hardware-owning daemon for one runtime directory.
///
/// Dropping this value removes the bound socket when it is still ours, then
/// releases the lock. Transfer the listener with [`Self::into_listener`] and
/// keep the returned [`DaemonGuard`] until hardware has shut down.
#[derive(Debug)]
#[must_use = "dropping DaemonOwnership releases the runtime lock and owned socket"]
pub struct DaemonOwnership {
    lock: Option<File>,
    listener: Option<UnixListener>,
    path: PathBuf,
    identity: SocketIdentity,
    armed: bool,
}

/// Holds the runtime lock after the listening socket has been handed to tokio.
#[derive(Debug)]
#[must_use = "dropping DaemonGuard releases the runtime lock and owned socket"]
pub struct DaemonGuard {
    lock: Option<File>,
    path: PathBuf,
    identity: SocketIdentity,
    released: bool,
}

impl DaemonOwnership {
    /// Lock the runtime directory, refuse a live peer, and bind `socket_path`.
    ///
    /// A second caller fails before this returns. The socket is unlinked only
    /// when connecting to it is refused, which means nothing is listening.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::AlreadyExists`] when another daemon holds the
    /// lock or a live peer answers on the socket. Other I/O failures are
    /// returned unchanged and leave an unexpected non-socket path in place.
    pub fn acquire(socket_path: &Path) -> io::Result<Self> {
        let lock = acquire_lock(&lock_path_for(socket_path)?)?;
        match classify_socket(socket_path)? {
            SocketState::Absent => {}
            SocketState::Stale => remove_stale_socket(socket_path)?,
            SocketState::Occupied(detail) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "a live daemon already owns {}: {detail}; refusing to replace it or open hardware",
                        socket_path.display()
                    ),
                ));
            }
        }

        let listener = UnixListener::bind(socket_path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to bind {} without replacing an existing listener: {error}",
                    socket_path.display()
                ),
            )
        })?;
        let identity = SocketIdentity::of_path(socket_path)?;
        Ok(Self {
            lock: Some(lock),
            listener: Some(listener),
            path: socket_path.to_path_buf(),
            identity,
            armed: true,
        })
    }

    /// Path of the bound socket.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Inode identity captured immediately after a successful bind.
    #[must_use]
    pub fn identity(&self) -> SocketIdentity {
        self.identity
    }

    /// Borrow the std listener. Production code uses [`Self::into_listener`].
    ///
    /// # Errors
    ///
    /// Returns an error when the listener was already transferred.
    pub fn listener(&self) -> io::Result<&UnixListener> {
        self.listener.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "IPC listener was already transferred",
            )
        })
    }

    /// Convert the bound socket into a nonblocking tokio listener.
    ///
    /// The returned guard must be held until device actors have stopped.
    /// Dropping it then removes this process's socket and releases the lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the std listener cannot be put into nonblocking
    /// mode or adopted by tokio.
    pub fn into_listener(mut self) -> io::Result<(tokio::net::UnixListener, DaemonGuard)> {
        let listener = self.listener.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "IPC listener was already transferred",
            )
        })?;
        listener.set_nonblocking(true)?;
        let listener = tokio::net::UnixListener::from_std(listener)?;
        let lock = self.lock.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "daemon lock was already transferred",
            )
        })?;
        let guard = DaemonGuard {
            lock: Some(lock),
            path: self.path.clone(),
            identity: self.identity,
            released: false,
        };
        self.armed = false;
        Ok((listener, guard))
    }
}

impl Drop for DaemonOwnership {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.listener.take();
        release_owned_socket(&self.path, self.identity);
    }
}

impl DaemonGuard {
    /// Path of the socket this guard is allowed to remove.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Inode identity captured when the socket was bound.
    #[must_use]
    pub fn identity(&self) -> SocketIdentity {
        self.identity
    }

    /// Remove the socket if it is still the inode we bound.
    ///
    /// The lock stays held until this guard is dropped, so a cooperating
    /// daemon cannot bind a replacement until hardware shutdown has finished.
    pub fn release_socket(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        release_owned_socket(&self.path, self.identity);
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        self.release_socket();
        // Drop the lock file only after the socket inode check so flock is
        // still held while a cooperating daemon cannot bind a replacement.
        drop(self.lock.take());
    }
}

enum SocketState {
    Absent,
    Stale,
    Occupied(String),
}

fn lock_path_for(socket_path: &Path) -> io::Result<PathBuf> {
    let parent = socket_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    let Some(parent) = parent else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "socket path must be inside a directory so the daemon lock has a runtime directory",
        ));
    };
    Ok(parent.join(LOCK_FILE_NAME))
}

fn acquire_lock(path: &Path) -> io::Result<File> {
    for _ in 0..8 {
        let mut file = open_lock(path)?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {}
            Err(errno) if errno.kind() == io::ErrorKind::WouldBlock => {
                return Err(lock_held(path));
            }
            Err(errno) => {
                return Err(io::Error::new(
                    errno.kind(),
                    format!("failed to lock {}: {errno}", path.display()),
                ));
            }
        }
        if lock_inode_matches(&file, path)? {
            record_holder(&mut file)?;
            return Ok(file);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not acquire a stable daemon lock at {}",
            path.display()
        ),
    ))
}

fn open_lock(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags((rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW).bits() as i32)
        .open(path)
}

fn lock_inode_matches(file: &File, path: &Path) -> io::Result<bool> {
    let fd_meta = file.metadata()?;
    match std::fs::symlink_metadata(path) {
        Ok(path_meta) => Ok(path_meta.is_file()
            && path_meta.dev() == fd_meta.dev()
            && path_meta.ino() == fd_meta.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn record_holder(file: &mut File) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    writeln!(file, "{}", std::process::id())?;
    Ok(())
}

fn lock_held(path: &Path) -> io::Error {
    let holder = std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()));
    let suffix = match holder {
        Some(pid) => format!(" (pid {pid})"),
        None => String::new(),
    };
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "another lconnectd holds the runtime lock at {}{suffix}; refusing to start a second hardware owner",
            path.display()
        ),
    )
}

fn classify_socket(path: &Path) -> io::Result<SocketState> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(SocketState::Absent),
        Err(error) => return Err(error),
        Ok(metadata) if metadata.file_type().is_socket() => {}
        Ok(_) => {
            return Ok(SocketState::Occupied(format!(
                "{} exists and is not a socket; refusing to remove it",
                path.display()
            )));
        }
    }

    match UnixStream::connect(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SocketState::Absent),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => Ok(SocketState::Stale),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("cannot probe {}: {error}", path.display()),
        )),
        Ok(stream) => Ok(SocketState::Occupied(describe_live_peer(&stream))),
    }
}

fn describe_live_peer(stream: &UnixStream) -> String {
    match read_probe_line(stream) {
        Ok(line) => match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(value) => match value.get("method").and_then(|method| method.as_str()) {
                Some(method) => format!("answered {method}"),
                None => "accepted a connection and sent a response".to_owned(),
            },
            Err(_) => "accepted a connection and sent a response".to_owned(),
        },
        Err(error) => format!("accepted a connection but did not answer hello ({error})"),
    }
}

fn read_probe_line(stream: &UnixStream) -> io::Result<String> {
    let mut stream = stream;
    stream.set_read_timeout(Some(LIVE_SOCKET_PROBE_TIMEOUT))?;
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if buf.len() >= 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "probe response exceeds 8192 bytes",
            ));
        }
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => buf.push(byte[0]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    String::from_utf8(buf).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("refusing to remove non-socket {}", path.display()),
        )),
    }
}

fn release_owned_socket(path: &Path, identity: SocketIdentity) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !metadata.file_type().is_socket() {
        return;
    }
    if SocketIdentity::from_metadata(&metadata) != identity {
        tracing::warn!(
            path = %path.display(),
            "leaving IPC socket in place because it is no longer the daemon we bound"
        );
        return;
    }
    if let Err(error) = std::fs::remove_file(path) {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed to remove owned IPC socket"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_runtime_rejects_relative_path() {
        let error = socket_path_for_runtime(Path::new("run/user/1000")).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn checked_runtime_rejects_missing_path() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing");

        let error = socket_path_for_runtime(&missing).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn checked_runtime_accepts_existing_absolute_directory() {
        let temp = tempfile::tempdir().unwrap();

        let socket = socket_path_for_runtime(temp.path()).unwrap();

        assert_eq!(socket, temp.path().join("lconnectd.sock"));
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::{DaemonOwnership, SocketIdentity};
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    const HELLO: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n";

    fn socket_path(dir: &Path) -> std::path::PathBuf {
        dir.join("lconnectd.sock")
    }

    fn wait_for_hello(path: &Path) {
        let started = Instant::now();
        loop {
            if let Ok(mut client) = UnixStream::connect(path) {
                client
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .unwrap();
                let mut buf = [0u8; HELLO.len()];
                if client.read_exact(&mut buf).is_ok() && buf.as_slice() == HELLO {
                    return;
                }
            }
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "peer did not answer hello"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn socket_path_without_a_directory_is_rejected() {
        let error = DaemonOwnership::acquire(Path::new("lconnectd.sock")).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn stale_socket_is_replaced_and_drop_removes_only_that_inode() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        let stale = UnixListener::bind(&path).unwrap();
        let stale_identity = SocketIdentity::of_path(&path).unwrap();
        drop(stale);

        let ownership = DaemonOwnership::acquire(&path).unwrap();

        assert_ne!(ownership.identity(), stale_identity);
        assert_eq!(
            SocketIdentity::of_path(&path).unwrap(),
            ownership.identity()
        );
        drop(ownership);
        assert!(!path.exists());
    }

    #[test]
    fn second_acquire_fails_before_replacing_the_winner_socket() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        let winner = DaemonOwnership::acquire(&path).unwrap();
        let winner_identity = winner.identity();

        let error = DaemonOwnership::acquire(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(
            error
                .to_string()
                .contains("another lconnectd holds the runtime lock"),
            "{error}"
        );
        assert!(error.to_string().contains(&std::process::id().to_string()));
        assert_eq!(SocketIdentity::of_path(&path).unwrap(), winner_identity);
    }

    #[test]
    fn live_socket_without_our_lock_is_not_removed() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        let listener = UnixListener::bind(&path).unwrap();
        let identity = SocketIdentity::of_path(&path).unwrap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            while !server_stop.load(std::sync::atomic::Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream.write_all(HELLO).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            }
        });
        wait_for_hello(&path);

        let error = DaemonOwnership::acquire(&path).unwrap_err();

        assert!(error.to_string().contains("answered hello"), "{error}");
        assert_eq!(SocketIdentity::of_path(&path).unwrap(), identity);
        let mut client = UnixStream::connect(&path).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = [0u8; HELLO.len()];
        client.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, HELLO);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
    }

    #[test]
    fn silent_accepted_socket_is_not_removed() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        let listener = UnixListener::bind(&path).unwrap();
        let identity = SocketIdentity::of_path(&path).unwrap();
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(500));
        });
        let started = Instant::now();

        let error = DaemonOwnership::acquire(&path).unwrap_err();

        assert!(started.elapsed() >= super::LIVE_SOCKET_PROBE_TIMEOUT);
        assert!(
            error.to_string().contains("did not answer hello"),
            "{error}"
        );
        assert_eq!(SocketIdentity::of_path(&path).unwrap(), identity);
        server.join().unwrap();
    }

    #[test]
    fn non_socket_path_is_left_in_place() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        std::fs::write(&path, b"not-a-socket").unwrap();

        let error = DaemonOwnership::acquire(&path).unwrap_err();

        assert!(error.to_string().contains("not a socket"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"not-a-socket");
    }

    #[test]
    fn shutdown_does_not_remove_a_replacement_inode() {
        let temp = tempfile::tempdir().unwrap();
        let path = socket_path(temp.path());
        let ownership = DaemonOwnership::acquire(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        let replacement_identity = SocketIdentity::of_path(&path).unwrap();
        assert_ne!(ownership.identity(), replacement_identity);

        drop(ownership);

        assert_eq!(
            SocketIdentity::of_path(&path).unwrap(),
            replacement_identity
        );
        drop(replacement);
    }
}
