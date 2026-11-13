use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::Command,
};
#[test]
fn dynamic_rgb_cli_set_state_disable_and_partial_write_error() {
    for action in ["set", "state", "disable", "failed"] {
        let tmp = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(tmp.path().join("llcontrold.sock")).unwrap();
        let config = json!({"sensor_id":"cpu:test","unit":"celsius","low_value":30,"high_value":60,"low_colors":[{"r":0,"g":0,"b":255}],"high_colors":[{"r":255,"g":0,"b":0}],"low_speed_percent":null,"high_speed_percent":null});
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
                    "get_dynamic_rgb"
                } else {
                    "set_dynamic_rgb"
                }
            );
            assert_eq!(req["params"]["dev"], "test");
            let mut state = llcontrol_ipc::DynamicRgbState::default();
            if action == "set" || action == "failed" {
                assert_eq!(
                    serde_json::from_value::<llcontrol_ipc::DynamicRgbConfig>(
                        req["params"]["config"].clone()
                    )
                    .unwrap(),
                    serde_json::from_value::<llcontrol_ipc::DynamicRgbConfig>(config.clone())
                        .unwrap()
                );
                state.requested = Some(serde_json::from_value(config).unwrap());
            }
            if action == "disable" {
                assert!(req["params"].as_object().unwrap().contains_key("config"));
                assert!(req["params"]["config"].is_null());
            }
            if action == "failed" {
                state.last_error = Some("injected".into());
            }
            writeln!(
                s,
                "{}",
                json!({"jsonrpc":"2.0","id":req["id"],"result":state})
            )
            .unwrap();
        });
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_llcontrol"));
        cmd.args([
            "--json",
            "dynamic-rgb",
            if action == "failed" { "set" } else { action },
        ]);
        if action == "set" || action == "failed" {
            cmd.arg("--config").arg(&path);
        }
        let out = cmd
            .args(["--device", "test"])
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LLCONTROL_SOCKET")
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            action != "failed",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let _: llcontrol_ipc::DynamicRgbState = serde_json::from_slice(&out.stdout).unwrap();
        server.join().unwrap();
    }
}
#[test]
fn dynamic_rgb_cli_rejects_invalid_and_oversized_files_before_connecting() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("bad.json");
    for (text, expected) in [
        (
            r#"{"sensor_id":"cpu:test","unit":"celsius","low_value":80,"high_value":30,"low_colors":[],"high_colors":[]}"#
                .to_string(),
            "range",
        ),
        (" ".repeat(65537), "64 KiB"),
    ] {
        std::fs::write(&path, text).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .args(["dynamic-rgb", "set", "--config"])
            .arg(&path)
            .env("XDG_RUNTIME_DIR", tmp.path())
            .env_remove("LLCONTROL_SOCKET")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains(expected));
    }
}
