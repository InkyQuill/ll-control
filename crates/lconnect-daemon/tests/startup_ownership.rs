//! Exercise the production executable's losing startup path without opening HID.
//! A socket-only owner fixture holds the winning runtime lock.
use lconnect_ipc::socket::{DaemonOwnership, SocketIdentity};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn production_loser_preserves_owner_and_never_initializes_persistence() {
    let isolated = tempfile::tempdir().unwrap();
    let runtime = isolated.path().join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let socket = runtime.join("lconnectd.sock");
    let owner = DaemonOwnership::acquire(&socket).unwrap();
    let identity = owner.identity();
    let listener = owner.listener().unwrap();
    // Establish a client before launching the real daemon, retaining both ends.
    let mut existing = UnixStream::connect(&socket).unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    existing
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let config = isolated.path().join("config");
    let data = isolated.path().join("data");
    let state = isolated.path().join("state");
    let stderr = std::fs::File::create(isolated.path().join("stderr")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lconnectd"))
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_CONFIG_DIRS", &config)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_STATE_HOME", &state)
        .env("RUST_LOG", "info")
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("production losing daemon did not exit promptly");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let error = std::fs::read_to_string(isolated.path().join("stderr")).unwrap();
    assert!(!status.success(), "{error}");
    assert!(
        error.contains("exited before persistence, HID, or settings replay"),
        "{error}"
    );
    assert!(
        error.contains("another lconnectd holds the runtime lock"),
        "{error}"
    );
    assert!(
        !error.contains("exclusive daemon ownership acquired"),
        "{error}"
    );
    assert!(!data.exists(), "loser initialized persistent data");
    assert!(!state.exists(), "loser initialized persistent state");
    assert_eq!(SocketIdentity::of_path(&socket).unwrap(), identity);
    peer.write_all(b"existing client retained").unwrap();
    let mut response = [0; 24];
    existing.read_exact(&mut response).unwrap();
    assert_eq!(&response, b"existing client retained");
    let _new_client = UnixStream::connect(&socket).unwrap();
    listener.accept().unwrap();
    drop(owner);
    assert!(!socket.exists(), "winner could not clean up its own socket");
    assert!(
        DaemonOwnership::acquire(&socket).is_ok(),
        "lock was not released"
    );
}
