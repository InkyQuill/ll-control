use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{fs, io::Write, os::unix::net::UnixListener, process::Command};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

#[test]
fn windows_inspection_is_offline_read_only_and_hides_device_paths() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("windows.zip");
    let input = json!({"Version":"2.1.28.0","CreatedAt":null,"Type":0,"Datas":[{"MainType":0,"SubType":16846849,"Metadata":"DO-NOT-LEAK-WINDOWS-SERIAL","Data":STANDARD.encode(br#"{"Fan":{"Mode":0},"Pump":{"Mode":0}}"#)}]});
    let mut zip = ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file(
        "backup",
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(&serde_json::to_vec(&input).unwrap()).unwrap();
    zip.finish().unwrap();
    let original = fs::read(&archive).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    for json_mode in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llcontrol"));
        command
            .env("LLCONTROL_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"));
        if json_mode {
            command.arg("--json");
        }
        let output = command
            .args(["profiles", "windows-inspect"])
            .arg(&archive)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["group"], "fan_and_lighting");
        assert_eq!(report["inspection_only"], true);
        assert_eq!(report["import_supported"], false);
        assert_eq!(report["asset_payloads_validated"], false);
        assert_eq!(report["controllers"][0]["record_index"], 0);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-LEAK"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(fs::read(&archive).unwrap(), original);
        assert!(!root.path().join("config").exists());
    }
    // Failed format validation is local too: no fallback to uploading/applying.
    fs::write(&archive, b"not a ZIP").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llcontrol"))
        .env("LLCONTROL_SOCKET", &socket)
        .args(["profiles", "windows-inspect"])
        .arg(&archive)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(fs::read(&archive).unwrap(), b"not a ZIP");
}
