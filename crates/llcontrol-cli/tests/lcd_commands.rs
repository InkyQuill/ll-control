use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::process::Command;

fn run_with_server(args: &[&str]) -> Value {
    let runtime = tempfile::tempdir().unwrap();
    let socket = runtime.path().join("llcontrold.sock");
    let listener = UnixListener::bind(socket).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "{}", json!({"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}})).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let req: Value = serde_json::from_str(&line).unwrap();
        writeln!(
            stream,
            "{}",
            json!({"jsonrpc":"2.0","id":req["id"],"result":{}})
        )
        .unwrap();
        req
    });
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args(args)
        .args(["--device", "galahad2-lcdv2:0001"])
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env_remove("LLCONTROL_SOCKET")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap()
}

#[test]
fn individual_knobs_send_only_their_patch_without_socket_override() {
    for (command, value, patch) in [
        ("brightness", "40", json!({"brightness_percent":40})),
        ("rotation", "90", json!({"rotation_degrees":90})),
        ("mode", "LocalUI", json!({"mode":"LocalUI"})),
    ] {
        let req = run_with_server(&["lcd", command, value]);
        assert_eq!(req["method"], "patch_lcd_config");
        assert_eq!(req["params"]["patch"], patch);
    }
}

#[test]
fn state_and_stats_use_observable_rpc() {
    for (command, method) in [
        ("state", "get_lcd_state"),
        ("playback-stats", "get_lcd_playback_stats"),
    ] {
        assert_eq!(run_with_server(&["lcd", command])["method"], method);
    }
}

#[test]
fn rejects_oversized_clip_before_connecting() {
    let file = tempfile::NamedTempFile::new().unwrap();
    file.as_file().set_len(8 * 1024 * 1024 + 1).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args(["lcd", "play-clip"])
        .arg(file.path())
        .env("LLCONTROL_SOCKET", "/nonexistent/llcontrol-test.sock")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("clip JSON exceeds 8 MiB"));
}

#[test]
fn play_clip_sends_exact_manifest_device_and_nondefault_loops() {
    let file = tempfile::NamedTempFile::new().unwrap();
    serde_json::to_writer(
        file.as_file(),
        &json!({
            "fps": 24,
            "units": [{
                "bytes": [0,0,0,1,103,66,0,30,0,0,1,104,206,0,0,1,101,136],
                "keyframe": true
            }]
        }),
    )
    .unwrap();

    let path = file.path().to_str().unwrap();
    let req = run_with_server(&["lcd", "play-clip", path, "--loops", "7"]);
    assert_eq!(req["method"], "play_lcd_clip");
    assert_eq!(req["params"]["dev"], "galahad2-lcdv2:0001");
    assert_eq!(req["params"]["loops"], 7);
    assert_eq!(req["params"]["clip"]["fps"], 24);
    assert_eq!(req["params"]["clip"]["units"][0]["keyframe"], true);
    assert_eq!(
        req["params"]["clip"]["units"][0]["bytes"],
        json!([0, 0, 0, 1, 103, 66, 0, 30, 0, 0, 1, 104, 206, 0, 0, 1, 101, 136])
    );
}

#[test]
fn rejects_malformed_clip_json_before_connecting() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"{not valid JSON").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args(["lcd", "play-clip"])
        .arg(file.path())
        .env("LLCONTROL_SOCKET", "/nonexistent/llcontrol-test.sock")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("key must be a string"));
    assert!(!stderr.contains("connect"));
}

#[test]
fn non_ascii_letterbox_colour_is_an_error_instead_of_a_panic() {
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .args([
            "lcd",
            "image",
            "/unused-image.png",
            "--fit",
            "letterbox",
            "--letterbox-bg",
            "€abc",
        ])
        .env("LLCONTROL_SOCKET", "/nonexistent/llcontrol-test.sock")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("expected #RRGGBB"), "{error}");
    assert!(!error.contains("panicked"), "{error}");
}
