use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};

#[test]
fn sensors_uses_global_catalog_without_device_resolution_and_preserves_nulls() {
    let tmp = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(tmp.path().join("lconnectd.sock")).unwrap();
    let expected = json!([{"id":"proc:cpu:utilization","label":"CPU utilization","unit":"percent","value":null,"status":"unavailable","observed_unix_ms":null,"age_ms":null}]);
    let response = expected.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{}", json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["method"], "list_sensors");
        assert_eq!(request["params"], json!({}));
        writeln!(
            stream,
            "{}",
            json!({"jsonrpc":"2.0","id":request["id"],"result":response})
        )
        .unwrap();
    });
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(["--json", "sensors"])
        .env("XDG_RUNTIME_DIR", tmp.path())
        .env_remove("LCONNECTCTL_SOCKET")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        expected
    );
    server.join().unwrap();
}
