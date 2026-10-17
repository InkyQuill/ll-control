//! Actual executable/offline contract; no daemon or host filesystem references.
use serde_json::Value;
use std::{fs, os::unix::net::UnixListener, process::Command};
#[test]
fn screen_cli_preserves_banks_and_hashes_unreferenced_uploads_without_connecting_or_writing() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("source.zip");
    let source = include_bytes!("../../lconnect-profile/tests/fixtures/ScreenBanks.zip");
    fs::write(&archive, source).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    for json_mode in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lconnectctl"));
        command
            .env("LCONNECTCTL_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"));
        if json_mode {
            command.arg("--json");
        }
        let output = command
            .args(["profiles", "windows-screen"])
            .arg(&archive)
            .args(["--record", "0"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["slots"].as_array().unwrap().len(), 8);
        assert_eq!(report["payloads"].as_array().unwrap().len(), 5);
        assert_eq!(report["rotation_degrees"], 270);
        assert_eq!(report["images"][1]["resolution"], "external_default");
        assert_eq!(report["images"][2]["resolution"], "missing_payload");
        assert_eq!(report["group_import_supported"], false);
        assert_eq!(report["apply_supported"], false);
        assert_eq!(report["payload_streams_verified"], true);
        assert_eq!(report["media_decoded"], false);
        assert_eq!(report["theme_semantics_validated"], false);
        assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-LEAK-SOURCE-IDENTITY"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(fs::read(&archive).unwrap(), source);
        assert!(!root.path().join("config").exists());
    }
    for args in [
        vec!["--record", "99"],
        vec!["--record", "0", "--apply"],
        vec![],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .env("LCONNECTCTL_SOCKET", &socket)
            .args(["profiles", "windows-screen"])
            .arg(&archive)
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    fs::write(&archive, b"not a ZIP").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", &socket)
        .args(["profiles", "windows-screen"])
        .arg(&archive)
        .args(["--record", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
