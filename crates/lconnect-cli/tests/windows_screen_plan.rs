#[path = "../../lconnect-profile/tests/support/screen_plan.rs"]
mod fixture;
use serde_json::{json, Value};
use std::{fs, os::unix::net::UnixListener, process::Command};
#[test]
fn actual_cli_prepares_one_coherent_remap_without_connecting_or_mutating_source_and_rejects_incomplete_maps(
) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.zip");
    let bindings = root.path().join("bindings.json");
    let map = root.path().join("resources.json");
    let bytes = fixture::archive(|_| {}, &[]);
    fs::write(&source, &bytes).unwrap();
    fs::write(&bindings, fixture::bindings().to_string()).unwrap();
    fs::write(&map, fixture::mapping().to_string()).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = |with_bindings: bool, extra: Option<&str>| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lconnectctl"));
        cmd.env("LCONNECTCTL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-screen"])
            .arg(&source)
            .args(["--record", "0", "--resource-map"])
            .arg(&map);
        if with_bindings {
            cmd.arg("--bindings").arg(&bindings);
        }
        if let Some(flag) = extra {
            cmd.arg(flag);
        }
        cmd.output().unwrap()
    };
    let out = run(true, None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let plan = &report["screen_plan"];
    assert_eq!(plan["editor_banks"]["overlay_count"], 14);
    let native: lconnect_ipc::LcdTheme =
        serde_json::from_value(plan["current_editor_theme"].clone()).unwrap();
    native.validate().unwrap();
    assert_eq!(native.basic_editor.as_ref().unwrap().banks.len(), 2);
    assert_eq!(plan["current_editor_deferred"], false);
    assert_eq!(
        plan["inactive_editor_indices"],
        json!([1, 2, 3, 4, 5, 6, 7])
    );
    assert!(plan.get("native_editor_themes").is_none());
    assert_eq!(
        plan["resolved_references"][fixture::PATH],
        plan["resolved_references"][fixture::URI]
    );
    assert_eq!(plan["runtime_deferred"], false);
    assert_eq!(plan["provisional_keys"], true);
    assert_eq!(plan["assets_published"], false);
    assert_eq!(report["apply_supported"], false);
    assert_eq!(report["group_import_supported"], false);
    assert!(plan.get("images").is_none() && plan.get("media").is_none());
    for extra in [
        Some("--prepare-images"),
        Some("--prepare-media"),
        Some("--editor-banks"),
    ] {
        let out = run(true, extra);
        assert!(!out.status.success() && out.stdout.is_empty());
    }
    let out = run(false, None);
    assert!(!out.status.success() && out.stdout.is_empty());
    let mut missing = fixture::mapping();
    missing.as_object_mut().unwrap().remove(fixture::URI);
    fs::write(&map, missing.to_string()).unwrap();
    let out = run(true, None);
    assert!(!out.status.success() && out.stdout.is_empty());
    fs::write(
        &map,
        json!({"x":{"kind":"archive","entry":"wmsq/missing.png"}}).to_string(),
    )
    .unwrap();
    let out = run(true, None);
    assert!(!out.status.success() && out.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!root.path().join("config").exists());
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[test]
fn actual_cli_handles_constructor_shaped_source_and_keeps_current_editor_separate_from_runtime() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("defaults.zip");
    let bindings = root.path().join("bindings.json");
    let map = root.path().join("resources.json");
    let bytes = fixture::archive(
        |profile| {
            profile["ScreenProfiles"] = json!([{}, {}, {}, {}, {}, {}, {}, {}]);
            profile["ScreenEffectType"] = json!([1, 1, 1, 1, 1, 1, 1, 1]);
            profile["ScreenProfileName"] =
                json!(["Full Screen", null, null, null, null, null, null, null]);
            profile["ScreenConfigJson"] = json!("");
            for key in [
                "ScreenImageFileInfos",
                "ScreenImageVideoFileInfos",
                "ScreenWatermarkFileInfos",
            ] {
                profile[key] = json!([]);
            }
        },
        &[],
    );
    fs::write(&source, &bytes).unwrap();
    fs::write(&bindings, fixture::bindings().to_string()).unwrap();
    fs::write(&map, "{}").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("XDG_CONFIG_HOME", root.path().join("config"))
        .env("LCONNECTCTL_SOCKET", root.path().join("absent.sock"))
        .args(["--json", "profiles", "windows-screen"])
        .arg(&source)
        .args(["--record", "0", "--bindings"])
        .arg(bindings)
        .arg("--resource-map")
        .arg(map)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let plan = &report["screen_plan"];
    assert_eq!(plan["current_editor_deferred"], false);
    assert_eq!(plan["runtime_deferred"], true);
    assert!(plan["runtime"].is_null());
    assert_eq!(plan["editor_banks"]["basic_bank_count"], 1);
    assert_eq!(
        plan["editor_banks"]["slots"][0]["basic"]["ImageVideo"]["source_absent_materialized"],
        true
    );
    assert_eq!(
        plan["inactive_editor_indices"],
        json!([1, 2, 3, 4, 5, 6, 7])
    );
    assert!(plan.get("native_editor_themes").is_none());
    let theme: lconnect_ipc::LcdTheme =
        serde_json::from_value(plan["current_editor_theme"].clone()).unwrap();
    theme.validate().unwrap();
    assert_eq!(fs::read(source).unwrap(), bytes);
    assert!(!root.path().join("config").exists());
}
