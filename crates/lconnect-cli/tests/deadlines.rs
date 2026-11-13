#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

fn run_devices(socket: &Path, connect_ms: u64, rpc_ms: u64) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .arg("devices")
        .env("LCONNECTCTL_SOCKET", socket)
        .env("LCONNECTCTL_CONNECT_TIMEOUT_MS", connect_ms.to_string())
        .env("LCONNECTCTL_RPC_TIMEOUT_MS", rpc_ms.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(_status) = child.try_wait().unwrap() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("CLI exceeded its configured deadlines");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn listener() -> (tempfile::TempDir, std::path::PathBuf, UnixListener) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&path).unwrap();
    (root, path, listener)
}

#[test]
fn stalled_hello_fails_connect_without_sending_an_rpc() {
    let (_root, path, listener) = listener();
    let peer = std::thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_millis(250));
    });
    let output = run_devices(&path, 50, 50);
    peer.join().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("daemon connect/hello timed out after 50 ms"),
        "{stderr}"
    );
    assert!(stderr.contains("no RPC request was sent"), "{stderr}");
}

#[test]
fn stalled_response_is_ambiguous_and_not_replayed() {
    let (_root, path, listener) = listener();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
            .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        std::thread::sleep(Duration::from_millis(250));
        let mut replay = String::new();
        let replayed = reader
            .read_line(&mut replay)
            .map(|count| count > 0)
            .unwrap_or(false);
        (request, replayed)
    });
    let output = run_devices(&path, 100, 50);
    let (request, replayed) = peer.join().unwrap();
    assert!(request.contains("list_devices"), "{request}");
    assert!(!replayed, "timed-out request was replayed");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("RPC 'list_devices' timed out after 50 ms"),
        "{stderr}"
    );
    assert!(stderr.contains("outcome is ambiguous"), "{stderr}");
    assert!(stderr.contains("do not retry automatically"), "{stderr}");
}

#[test]
fn committed_but_lost_response_reports_an_ambiguous_mutation() {
    let (_root, path, listener) = listener();
    let committed = Arc::new(AtomicBool::new(false));
    let peer_committed = Arc::clone(&committed);
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"hello\",\"params\":{}}\n")
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        peer_committed.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(250));
        request
    });
    let output = run_devices(&path, 100, 50);
    let request = peer.join().unwrap();
    assert!(request.contains("list_devices"), "{request}");
    assert!(committed.load(Ordering::SeqCst));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("outcome is ambiguous and the request may have been applied"),
        "{stderr}"
    );
    assert!(stderr.contains("do not retry automatically"), "{stderr}");
}

#[test]
fn zero_rpc_deadline_is_rejected_before_connecting() {
    let root = tempfile::tempdir().unwrap();
    let output = run_devices(&root.path().join("unused.sock"), 100, 0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("LCONNECTCTL_RPC_TIMEOUT_MS must be greater than zero"));
}
