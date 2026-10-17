#[path = "../../lconnect-profile/tests/support/screen_images.rs"]
mod archive;
#[path = "../../lconnect-profile/tests/support/screen_media.rs"]
mod motion;
use serde_json::Value;
use std::{fs, os::unix::net::UnixListener, process::Command};
#[test]
fn actual_offline_cli_prepares_animation_without_publishing_and_late_decode_failure_has_no_output()
{
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.zip");
    let png = include_bytes!("../../lconnect-profile/tests/fixtures/ScreenImage.png");
    let gif = motion::motion("gif");
    let original = archive::archive(png, "wmsq/motion.gif", &gif);
    fs::write(&file, &original).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .env("LCONNECTCTL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-screen"])
            .arg(&file)
            .args(["--record", "0", "--prepare-media"])
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let media = &report["prepared_media"];
    assert_eq!(media["items"].as_array().unwrap().len(), 1);
    assert!(media["items"][0]["frames"].as_u64().unwrap() >= 6);
    assert_eq!(media["items"][0]["fps"], 24);
    assert!(media["items"][0].get("file").is_none());
    assert_eq!(media["assets_published"], false);
    assert_eq!(report["apply_supported"], false);
    assert_eq!(fs::read(&file).unwrap(), original);
    fs::write(&file, archive::archive(png, "wmsq/bad.mp4", b"not MP4")).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!root.path().join("config").exists());
}
