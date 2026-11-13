#[path = "../../llcontrol-profile/tests/support/screen_editor.rs"]
mod editor_fixture;
#[path = "../../llcontrol-profile/tests/support/screen_bindings.rs"]
mod runtime_fixture;
#[path = "../../llcontrol-profile/tests/support/wpf_color.rs"]
mod wpf_fixture;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read, Write},
    os::unix::net::UnixListener,
    process::Command,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
fn archive() -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(include_bytes!(
        "../../llcontrol-profile/tests/fixtures/ScreenBanks.zip"
    )))
    .unwrap();
    let mut bytes = Vec::new();
    input
        .by_name("backup")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut backup: Value = serde_json::from_slice(&bytes).unwrap();
    let mut profile: Value = serde_json::from_slice(
        &STANDARD
            .decode(backup["Datas"][0]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    profile["ScreenConfigJson"] = json!("");
    let settings = |image, i| {
        let mut value = editor_fixture::editor(image, i);
        wpf_fixture::storage_colors(&mut value);
        value.to_string()
    };
    profile["ScreenProfiles"]=Value::Array((0..8).map(|i|json!({"ImageVideo":[settings(true,i)],"BackgroundColor":[settings(false,i)],"Rain":["{\"Owned\":\"Deferred\"}"]})).collect());
    backup["Datas"][0]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
#[test]
fn actual_cli_prepares_complete_basic_editor_banks_without_reconstructing_runtime_or_applying() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.zip");
    let mapping = root.path().join("bindings.json");
    let original = archive();
    fs::write(&source, &original).unwrap();
    fs::write(&mapping, runtime_fixture::bindings().to_string()).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = |with_binding: bool| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_llcontrol"));
        c.env("LLCONTROL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-screen"])
            .arg(&source)
            .args(["--record", "0", "--editor-banks"]);
        if with_binding {
            c.arg("--bindings").arg(&mapping);
        }
        c.output().unwrap()
    };
    let out = run(true);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["source"]["runtime_config"]["source_json"], "");
    assert_eq!(report["editor_banks"]["basic_bank_count"], 16);
    assert_eq!(report["editor_banks"]["overlay_count"], 112);
    assert_eq!(report["editor_banks"]["apply_supported"], false);
    assert_eq!(report["group_import_supported"], false);
    for i in 0..8 {
        let slot = &report["editor_banks"]["slots"][i];
        assert_eq!(slot["source_name"], report["source"]["slots"][i]["name"]);
        assert_eq!(slot["deferred_effects"], json!(["Rain"]));
        assert_eq!(slot["basic"]["ImageVideo"]["selected_overlay"], "Custom");
        for kind in ["ImageVideo", "BackgroundColor"] {
            for value in slot["basic"][kind]["overlays"]
                .as_object()
                .unwrap()
                .values()
            {
                let theme: llcontrol_ipc::LcdTheme =
                    serde_json::from_value(value["theme"].clone()).unwrap();
                theme.validate().unwrap();
            }
        }
        assert_eq!(
            slot["basic"]["BackgroundColor"]["overlays"]["None"]["theme"]["background_fill"]
                ["radial"]["colors"],
            json!([[15, 16, 17], [18, 19, 20]])
        );
    }
    let mut bad = runtime_fixture::bindings();
    bad["sensors"]
        .as_object_mut()
        .unwrap()
        .remove("GPUTemperature");
    fs::write(&mapping, bad.to_string()).unwrap();
    let out = run(true);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let out = run(false);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!root.path().join("config").exists());
    assert_eq!(fs::read(&source).unwrap(), original);
}
