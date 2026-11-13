use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};

#[test]
fn profile_apply_uses_selected_snapshot_and_propagates_device_failure() {
    for fail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("llcontrold.sock")).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            writeln!(stream,"{}",json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            for method in ["list_profiles", "set_cooling_policy"] {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["method"], method);
                let result = if method == "list_profiles" {
                    json!({"version":1,"revision":5,"profiles":[{"slot":2,"name":"My fan","content":{"kind":"fan","value":{"mode":"fixed","percent":40}}}]})
                } else {
                    assert_eq!(
                        request["params"],
                        json!({"dev":"test","target":"fan","policy":{"mode":"fixed","percent":40}})
                    );
                    let mut state = llcontrol_ipc::CoolingState::default();
                    if fail {
                        state.fan.last_error = Some("injected transport error".into());
                    }
                    serde_json::to_value(state).unwrap()
                };
                writeln!(
                    stream,
                    "{}",
                    json!({"jsonrpc":"2.0","id":request["id"],"result":result})
                )
                .unwrap();
            }
        });
        let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .args(["profiles", "apply", "fan", "2", "--device", "test"])
            .env("XDG_RUNTIME_DIR", dir.path())
            .env_remove("LLCONTROL_SOCKET")
            .output()
            .unwrap();
        server.join().unwrap();
        assert_eq!(
            output.status.success(),
            !fail,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if fail {
            assert!(String::from_utf8_lossy(&output.stderr).contains("injected transport error"));
        } else {
            assert!(String::from_utf8_lossy(&output.stdout).contains("My fan"));
        }
    }
}

#[test]
fn invalid_slot_is_rejected_before_connecting() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args(["profiles", "apply", "screen", "8"])
        .env("XDG_RUNTIME_DIR", dir.path())
        .env_remove("LLCONTROL_SOCKET")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("slot out of range"));
}

#[test]
fn reset_group_sends_one_revision_checked_edit_without_device_apply() {
    let dir = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(dir.path().join("llcontrold.sock")).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{}", json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        for method in ["list_profiles", "edit_profile"] {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["method"], method);
            if method == "edit_profile" {
                assert_eq!(
                    request["params"],
                    json!({"expected_revision":5,"edit":{"action":"reset_group","group":"screen","sensor_id":null}})
                );
            }
            let revision = if method == "list_profiles" { 5 } else { 6 };
            writeln!(stream, "{}", json!({"jsonrpc":"2.0","id":request["id"],"result":{"version":1,"revision":revision,"profiles":[]}})).unwrap();
        }
        let mut extra = String::new();
        assert_eq!(
            reader.read_line(&mut extra).unwrap(),
            0,
            "unexpected device request"
        );
    });
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args(["profiles", "reset-group", "screen"])
        .env("XDG_RUNTIME_DIR", dir.path())
        .env_remove("LLCONTROL_SOCKET")
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("revision 6"));
}
