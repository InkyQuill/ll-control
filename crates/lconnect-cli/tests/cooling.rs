use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};

#[test]
fn cooling_policy_file_reaches_rpc_and_initial_write_error_is_not_cli_success() {
    for failed in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("lconnectd.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let policy = json!({"mode":"fixed","percent":80});
        let path = tmp.path().join("policy.json");
        std::fs::write(&path, policy.to_string()).unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            writeln!(s,"{}",json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
            let mut reader = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let req: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(req["method"], "set_cooling_policy");
            assert_eq!(
                req["params"],
                json!({"dev":"test","target":"pump","policy":policy})
            );
            let mut state = lconnect_ipc::CoolingState::default();
            state.pump.requested = Some(lconnect_ipc::CoolingPolicy::Fixed { percent: 80 });
            if failed {
                state.pump.last_error = Some("injected write error".into());
            } else {
                state.pump.applied_percent = Some(80);
            }
            writeln!(
                s,
                "{}",
                json!({"jsonrpc":"2.0","id":req["id"],"result":state})
            )
            .unwrap();
        });
        let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args(["cooling", "set", "pump", "--policy"])
            .arg(path)
            .args(["--device", "test"])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            !failed,
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let state: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(state["pump"]["requested"]["percent"], 80);
        server.join().unwrap();
    }
}
#[test]
fn unsafe_policy_is_rejected_before_connecting() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.json");
    std::fs::write(&path, r#"{"mode":"fixed","percent":0}"#).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(["cooling", "set", "pump", "--policy"])
        .arg(path)
        .env("XDG_RUNTIME_DIR", tmp.path())
        .env_remove("LCONNECTCTL_SOCKET")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("20..=100"));
}

#[test]
fn pump_preset_generates_valid_editable_policy_without_a_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["quiet", "performance"] {
        let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args(["cooling", "preset", name, "--sensor", "cpu:test"])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let policy: lconnect_ipc::CoolingPolicy = serde_json::from_slice(&out.stdout).unwrap();
        policy.validate(lconnect_ipc::CoolingTarget::Pump).unwrap();
        let lconnect_ipc::CoolingPolicy::CurveRpm {
            points, sensor_id, ..
        } = policy
        else {
            panic!("RPM curve")
        };
        assert_eq!(sensor_id, "cpu:test");
        assert_eq!(points.len(), 6);
        assert_eq!(
            points.last().unwrap().rpm,
            if name == "quiet" { 3000 } else { 3600 }
        );
    }
}

#[test]
fn fan_presets_are_generated_without_ipc_and_target_mismatches_fail() {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["quiet", "standard", "high", "full"] {
        let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args([
                "cooling", "preset", name, "--target", "fan", "--sensor", "cpu:test",
            ])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let policy: lconnect_ipc::CoolingPolicy = serde_json::from_slice(&out.stdout).unwrap();
        policy.validate(lconnect_ipc::CoolingTarget::Fan).unwrap();
        assert!(matches!(
            policy,
            lconnect_ipc::CoolingPolicy::FanCurve { .. }
        ));
    }
    for (name, target) in [("performance", "fan"), ("standard", "pump")] {
        let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args([
                "cooling", "preset", name, "--target", target, "--sensor", "cpu:test",
            ])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LCONNECTCTL_SOCKET")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("presets:"));
    }
}
