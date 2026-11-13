#[path = "../../llcontrol-profile/tests/support/screen_runtime.rs"]
mod fixture;
use base64::{engine::general_purpose::STANDARD, Engine};
use llcontrol_image::theme::{
    render_theme, render_theme_elapsed, ClockTime, RenderError, ThemeAssets,
};
use llcontrol_ipc::{LcdTheme, SensorReading, SensorStatus, SensorUnit, ThemeLayer};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read, Write},
    os::unix::net::UnixListener,
    process::Command,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
fn archive(runtime: Value) -> Vec<u8> {
    let mut source = ZipArchive::new(Cursor::new(include_bytes!(
        "../../llcontrol-profile/tests/fixtures/ScreenBanks.zip"
    )))
    .unwrap();
    let mut bytes = Vec::new();
    source
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
    profile["ScreenConfigJson"] = json!(runtime.to_string());
    backup["Datas"][0]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
fn clock() -> ClockTime {
    ClockTime {
        year: 2026,
        month: 10,
        day: 17,
        hour: 13,
        minute: 47,
        second: 9,
    }
}
fn literal(theme: &LcdTheme, label: &str, value: &str, unit: &str) -> LcdTheme {
    let ThemeLayer::SensorCycle {
        style,
        parts: Some(parts),
        ..
    } = &theme.layers[0]
    else {
        panic!("cycle")
    };
    let mut expected = theme.clone();
    expected.layers = vec![
        ThemeLayer::Text {
            editor: None,
            text: value.into(),
            style: style.clone(),
        },
        ThemeLayer::Text {
            editor: None,
            text: label.into(),
            style: parts.label.clone(),
        },
        ThemeLayer::Text {
            editor: None,
            text: unit.into(),
            style: parts.unit.clone(),
        },
    ];
    expected
}
#[test]
fn actual_offline_cli_proposal_renders_bound_live_caches_and_cycle_without_fake_zero_or_rebinding()
{
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.zip");
    let binding_file = root.path().join("bindings.json");
    let runtime = fixture::source(
        json!({"Type":"SliderSensor","Sensors":["CPUTemperature","GPULoad"],"LoopSecond":7,"Layout":[fixture::layout()]}),
    );
    let original = archive(runtime);
    fs::write(&file, &original).unwrap();
    fs::write(&binding_file, fixture::bindings().to_string()).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .env("LLCONTROL_SOCKET", &socket)
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-screen"])
            .arg(&file)
            .args(["--record", "0", "--bindings"])
            .arg(&binding_file)
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
    assert_eq!(report["runtime"]["layout_review_required"], true);
    assert_eq!(report["runtime"]["editor_banks_converted"], false);
    assert_eq!(report["apply_supported"], false);
    assert_eq!(report["source"]["slots"].as_array().unwrap().len(), 8);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("DO-NOT-LEAK-SOURCE-IDENTITY"));
    let theme: LcdTheme = serde_json::from_value(report["runtime"]["theme"].clone()).unwrap();
    let temp = SensorReading {
        id: "explicit:CPUTemperature".into(),
        label: "untrusted catalog label".into(),
        unit: SensorUnit::Celsius,
        value: Some(20.0),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(50),
    };
    let gpu = SensorReading {
        id: "explicit:GPULoad".into(),
        label: "other catalog label".into(),
        unit: SensorUnit::Percent,
        value: Some(41.0),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(50),
    };
    let assets = ThemeAssets::new();
    for scenario in [
        "valid",
        "updated",
        "missing",
        "stale",
        "invalid",
        "wrong_unit",
        "wrong_id",
    ] {
        let mut temp = temp.clone();
        let expected = match scenario {
            "valid" => "68.0",
            "updated" => {
                temp.value = Some(30.0);
                "86.0"
            }
            "missing" => {
                temp.value = None;
                "—"
            }
            "stale" => {
                temp.age_ms = Some(3001);
                "—"
            }
            "invalid" => {
                temp.status = SensorStatus::Invalid;
                "—"
            }
            "wrong_unit" => {
                temp.unit = SensorUnit::Volts;
                "—"
            }
            _ => {
                temp.id = "different:temp".into();
                "—"
            }
        };
        let readings = vec![temp, gpu.clone()];
        let actual = render_theme_elapsed(&theme, &readings, clock(), &assets, 0).unwrap();
        let expected_theme = literal(&theme, "Owned CPUTemperature", expected, "°F");
        assert_eq!(
            actual,
            render_theme(&expected_theme, &[], clock(), &assets).unwrap(),
            "{scenario}"
        );
        assert_eq!(actual.dimensions(), (480, 480));
        assert_eq!(
            render_theme_elapsed(&theme, &readings, clock(), &assets, 7).unwrap(),
            render_theme(
                &literal(&theme, "Owned GPULoad", "41.0", "%"),
                &[],
                clock(),
                &assets
            )
            .unwrap()
        );
    }
    let mut bad = fixture::bindings();
    bad["sensors"]["CPUTemperature"]["unit"] = json!("volts");
    fs::write(&binding_file, bad.to_string()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(!root.path().join("config").exists());
    assert_eq!(fs::read(&file).unwrap(), original);
}
#[test]
fn mapped_managed_fonts_and_media_are_mandatory_not_silently_replaced_at_render_time() {
    let mut source = fixture::source(json!({"Type":"None"}));
    source["Screens"][0]["Theme"]["Path"] = json!("file:///C:/owned/art.png");
    let bindings =
        llcontrol_profile::linux_screen::parse_bindings(fixture::bindings().to_string().as_bytes())
            .unwrap();
    let theme = llcontrol_profile::linux_screen::prepare_runtime(&source.to_string(), &bindings)
        .unwrap()
        .theme;
    assert!(matches!(
        render_theme(&theme, &[], clock(), &ThemeAssets::new()),
        Err(RenderError::MissingAsset(_))
    ));
    source["Screens"][0]["Theme"]["Path"] = json!("file:///C:/owned/clip.mp4");
    let theme = llcontrol_profile::linux_screen::prepare_runtime(&source.to_string(), &bindings)
        .unwrap()
        .theme;
    assert!(matches!(
        render_theme(&theme, &[], clock(), &ThemeAssets::new()),
        Err(RenderError::MissingVideoFrame)
    ));
    let mut layout = fixture::layout();
    layout["FontColors"] = json!([[230, 240, 250]]);
    let source =
        fixture::source(json!({"Type":"Custom","Text":"Mandatory user font","Layout":[layout]}));
    let mut bindings = fixture::bindings();
    bindings["fonts"]["HyperspaceRace_BoldItalic"]["font_asset"] =
        json!("44444444444444444444444444444444");
    let bindings =
        llcontrol_profile::linux_screen::parse_bindings(bindings.to_string().as_bytes()).unwrap();
    let theme = llcontrol_profile::linux_screen::prepare_runtime(&source.to_string(), &bindings)
        .unwrap()
        .theme;
    assert!(matches!(
        render_theme(&theme, &[], clock(), &ThemeAssets::new()),
        Err(RenderError::MissingFont(_))
    ));
}
