#[path = "../../llcontrol-profile/tests/support/screen_images.rs"]
mod fixture;
use serde_json::Value;
use std::{fs, os::unix::net::UnixListener, process::Command};
#[test]
fn actual_cli_decodes_verified_static_payloads_offline_and_never_publishes_partial_results() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.zip");
    let png = include_bytes!("../../llcontrol-profile/tests/fixtures/ScreenImage.png");
    let bytes = fixture::archive(png, "wmsq/motion.gif", b"GIF89a");
    fs::write(&source, &bytes).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .env("LLCONTROL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-screen"])
            .arg(&source)
            .args(["--record", "0", "--prepare-images"])
            .output()
            .unwrap()
    };
    let out = run();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let prepared = &report["prepared_images"];
    assert_eq!(prepared["images"].as_array().unwrap().len(), 1);
    assert_eq!(prepared["images"][0]["width"], 2);
    assert_eq!(prepared["images"][0]["height"], 1);
    assert!(prepared["images"][0].get("png").is_none());
    assert_eq!(prepared["images_decoded"], true);
    assert_eq!(prepared["assets_published"], false);
    assert_eq!(prepared["deferred_entries"][0], "wmsq/motion.gif");
    assert_eq!(report["apply_supported"], false);
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert!(!root.path().join("config").exists());
    fs::write(
        &source,
        fixture::archive(png, "wmsq/z-later.png", b"not PNG"),
    )
    .unwrap();
    let out = run();
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "a later failed image must not expose a partial proposal"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!root.path().join("config").exists());
}
