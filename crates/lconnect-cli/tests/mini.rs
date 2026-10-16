//! The shell feed talks only to a private synthetic Unix RPC peer.
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
fn peer(path: &Path, value: f64, calls: Arc<Mutex<Vec<String>>>) -> thread::JoinHandle<()> {
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }
    let listener = UnixListener::bind(path).unwrap();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(
            socket,
            "{}",
            json!({"jsonrpc":"2.0","method":"hello","params":{"version":"test","capabilities":[]}})
        )
        .unwrap();
        socket.flush().unwrap();
        let mut input = BufReader::new(socket.try_clone().unwrap());
        for _ in 0..3 {
            let mut line = String::new();
            assert_ne!(input.read_line(&mut line).unwrap(), 0);
            let req: Value = serde_json::from_str(&line).unwrap();
            let method = req["method"].as_str().unwrap();
            assert_eq!(req["params"], json!({}));
            calls.lock().unwrap().push(method.into());
            let result = match method {
                "list_sensors" => {
                    json!([{"id":"gpu:physical-A:load","label":"Настройки","unit":"percent","value":value,"status":"valid","age_ms":0,"observed_unix_ms":1}])
                }
                "get_sensor_defaults" => {
                    json!({"config":lconnect_ipc::SensorDefaults::default(),"recommended":lconnect_ipc::SensorDefaults::default(),"candidates":[],"values":[]})
                }
                "get_display_settings" => {
                    serde_json::to_value(lconnect_ipc::DisplaySettings::default()).unwrap()
                }
                _ => panic!("Unexpected mutation or unsupported call: {method}"),
            };
            writeln!(
                socket,
                "{}",
                json!({"jsonrpc":"2.0","id":req["id"],"result":result})
            )
            .unwrap();
            socket.flush().unwrap();
        }
    })
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn snapshot_requires_no_gui_or_device_and_preserves_raw_labels_and_cache_units() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("peer.sock");
    let calls = Arc::new(Mutex::new(vec![]));
    let server = peer(&path, 37.0, calls.clone());
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(["--json", "mini", "snapshot"])
        .env("LCONNECTCTL_SOCKET", &path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(data["version"], 1);
    assert_eq!(data["available"], true);
    assert_eq!(data["data"]["display"]["interface_language"], "en-US");
    assert_eq!(data["data"]["sensors"][0]["label"], "Настройки");
    assert_eq!(data["data"]["sensors"][0]["value"], 37.0);
    assert_eq!(data["data"]["sensors"][0]["unit"], "percent");
    server.join().unwrap();
    assert_eq!(calls.lock().unwrap().len(), 3);
}
#[test]
fn watch_reports_offline_reconnects_after_server_loss_and_stops_without_control_writes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("peer.sock");
    let calls = Arc::new(Mutex::new(vec![]));
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args(["mini", "watch"])
            .env("LCONNECTCTL_SOCKET", &path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx
                .send(serde_json::from_str::<Value>(&line).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let next = || {
        rx.recv_timeout(Duration::from_secs(8))
            .expect("bounded shell feed reply")
    };
    let offline = next();
    assert_eq!(offline["available"], false);
    assert!(offline["data"].is_null());
    let first = peer(&path, 10.0, calls.clone());
    let first_value = next();
    assert_eq!(first_value["available"], true);
    assert_eq!(first_value["data"]["sensors"][0]["value"], 10.0);
    first.join().unwrap();
    let offline = next();
    assert_eq!(offline["available"], false);
    assert!(
        offline["data"].is_null(),
        "Old samples must not remain valid after disconnect"
    );
    let second = peer(&path, 20.0, calls.clone());
    let recovered = next();
    assert_eq!(recovered["available"], true);
    assert_eq!(recovered["data"]["sensors"][0]["value"], 20.0);
    second.join().unwrap();
    assert_eq!(calls.lock().unwrap().len(), 6);
    drop(child);
    drop(rx);
    reader.join().unwrap();
}
