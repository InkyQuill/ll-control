use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};
#[test]
fn thermal_cli_set_state_disable_and_partial_write_error() {
    for action in ["set", "state", "disable", "failed"] {
        let tmp = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(tmp.path().join("lconnectd.sock")).unwrap();
        let config = json!({"cpu":{"sensor_id":"cpu:test","threshold_c":70,"color":{"r":255,"g":0,"b":0}},"gpu":null});
        let path = tmp.path().join("thermal.json");
        std::fs::write(&path, config.to_string()).unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            writeln!(s,"{}",json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
            let mut reader = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let req: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                req["method"],
                if action == "state" {
                    "get_thermal_state"
                } else {
                    "set_thermal_config"
                }
            );
            assert_eq!(req["params"]["dev"], "test");
            let mut state = lconnect_ipc::ThermalState::default();
            if action == "set" || action == "failed" {
                assert_eq!(
                    serde_json::from_value::<lconnect_ipc::ThermalConfig>(
                        req["params"]["config"].clone()
                    )
                    .unwrap(),
                    serde_json::from_value::<lconnect_ipc::ThermalConfig>(config.clone()).unwrap()
                );
                state.requested = serde_json::from_value(config).unwrap();
            }
            if action == "disable" {
                assert!(req["params"]["config"]["cpu"].is_null());
                assert!(req["params"]["config"]["gpu"].is_null());
            }
            if action == "failed" {
                state.pump.last_error = Some("injected".into());
            }
            writeln!(
                s,
                "{}",
                json!({"jsonrpc":"2.0","id":req["id"],"result":state})
            )
            .unwrap();
        });
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lconnectctl"));
        cmd.args(["thermal", if action == "failed" { "set" } else { action }]);
        if action == "set" || action == "failed" {
            cmd.arg("--config").arg(&path);
        }
        let out = cmd
            .args(["--device", "test"])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            action != "failed",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let _: lconnect_ipc::ThermalState = serde_json::from_slice(&out.stdout).unwrap();
        server.join().unwrap();
    }
}
#[test]
fn thermal_cli_rejects_invalid_and_oversized_files_before_connecting() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.json");
    for (text, expected) in [
        (
            r#"{"cpu":{"sensor_id":"cpu:test","threshold_c":-1,"color":{"r":255,"g":0,"b":0}}}"#
                .to_string(),
            "threshold",
        ),
        (" ".repeat(65537), "64 KiB"),
    ] {
        std::fs::write(&path, text).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args(["thermal", "set", "--config"])
            .arg(&path)
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains(expected));
    }
}
