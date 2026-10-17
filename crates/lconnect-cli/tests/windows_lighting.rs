#[path = "../../lconnect-profile/tests/support/lighting.rs"]
mod fixture;
use serde_json::Value;
use std::{fs, io, os::unix::net::UnixListener, process::Command};
#[test]
fn complete_lighting_inspection_is_offline_read_only_and_retains_inactive_values() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("lighting.zip");
    let source = fixture::profile();
    let original = include_bytes!("../../lconnect-profile/tests/fixtures/Lighting.zip").to_vec();
    fs::write(&file, &original).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    for json_mode in [false, true] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lconnectctl"));
        cmd.env("LCONNECTCTL_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"));
        if json_mode {
            cmd.arg("--json");
        }
        let out = cmd
            .args(["profiles", "windows-lighting"])
            .arg(&file)
            .args(["--record", "0"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["fan"].as_object().unwrap().len(), 6);
        assert_eq!(report["pump"].as_object().unwrap().len(), 16);
        assert_eq!(report["pump"], source["ScreenLEDProfiles"]);
        assert_eq!(report["effective_fan_led_count"], 37);
        assert_eq!(report["group_import_supported"], false);
        assert_eq!(report["sensor_binding_verified"], false);
        assert!(!String::from_utf8_lossy(&out.stdout).contains("PRIVATE-WINDOWS-SERIAL"));
        assert_eq!(fs::read(&file).unwrap(), original);
        assert!(!root.path().join("config").exists());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", &socket)
        .args(["profiles", "windows-lighting"])
        .arg(&file)
        .args(["--record", "1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    fs::write(&file, fixture::archive(&source, 1)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", &socket)
        .args(["profiles", "windows-lighting"])
        .arg(&file)
        .args(["--record", "0"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("FanAndLighting"));
}

#[test]
fn explicit_binding_preparation_feeds_the_actual_dynamic_evaluator_without_daemon_writes() {
    use lconnect_ipc::{
        DynamicRgbConfig, DynamicRgbProfiles, Rgb, SensorReading, SensorStatus, SensorUnit,
    };
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.zip");
    let bindings = root.path().join("bindings.json");
    let mut source = fixture::profile();
    let sensor = &mut source["ScreenLEDProfiles"]["BigBang"]["DynamicSettings"]["CPULoad"];
    sensor["Low"]["Colors"] =
        serde_json::json!([[255, 0, 0], [255, 0, 0], [255, 0, 0], [255, 0, 0]]);
    sensor["High"]["Colors"] =
        serde_json::json!([[0, 255, 0], [0, 255, 0], [0, 255, 0], [0, 255, 0]]);
    sensor["Low"]["Speed"] = 20.into();
    sensor["High"]["Speed"] = 80.into();
    fs::write(&file, fixture::archive(&source, 0)).unwrap();
    let original = fs::read(&file).unwrap();
    let mut map = serde_json::Map::new();
    for (role, unit) in [
        ("CPULoad", "percent"),
        ("CPUTemperature", "celsius"),
        ("GPULoad", "percent"),
        ("GPUTemperature", "celsius"),
        ("PumpRPM", "rpm"),
        ("CoolantTemp", "celsius"),
    ] {
        map.insert(
            role.into(),
            serde_json::json!({"sensor_id":format!("fixture:{role}"),"unit":unit}),
        );
    }
    fs::write(&bindings, serde_json::to_vec(&map).unwrap()).unwrap();
    let socket = root.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .env("LCONNECTCTL_SOCKET", &socket)
            .env("XDG_RUNTIME_DIR", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .args(["--json", "profiles", "windows-lighting"])
            .arg(&file)
            .args(["--record", "0", "--bindings"])
            .arg(&bindings)
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
    let config: DynamicRgbConfig =
        serde_json::from_value(report["lighting"]["dynamic_rgb"].clone()).unwrap();
    let profiles: DynamicRgbProfiles =
        serde_json::from_value(report["lighting"]["dynamic_rgb_profiles"].clone()).unwrap();
    profiles.validate().unwrap();
    assert_eq!(profiles.modes.len(), 16);
    assert_eq!(config.brightness_percent, Some(90));
    assert_eq!(config.direction.as_deref(), Some("forward"));
    for (value, color, speed) in [
        (0.0, Rgb { r: 255, g: 0, b: 0 }, 20),
        (
            45.9,
            Rgb {
                r: 255,
                g: 255,
                b: 0,
            },
            50,
        ),
        (100.0, Rgb { r: 0, g: 255, b: 0 }, 80),
    ] {
        let row = SensorReading {
            id: config.sensor_id.clone(),
            label: "Owned CPU".into(),
            unit: SensorUnit::Percent,
            value: Some(value),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        };
        let sample = lconnect_daemon::actor::dynamic_rgb::evaluate(&config, &[row]).unwrap();
        assert_eq!(sample.colors, vec![color; 4]);
        assert_eq!(sample.speed_percent, Some(speed));
    }
    assert_eq!(
        report["source"]["pump"], source["ScreenLEDProfiles"],
        "source-only low brightness/direction/ranges remain available to the future group mapper"
    );
    assert_eq!(report["group_import_supported"], false);
    assert_eq!(report["apply_supported"], false);
    assert_eq!(report["sensor_binding_verified"], false);
    map.remove("CoolantTemp");
    fs::write(&bindings, serde_json::to_vec(&map).unwrap()).unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    fs::write(&bindings, b"{\"CPULoad\":{},\"CPULoad\":{}}").unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&file).unwrap(), original);
    assert!(!root.path().join("config").exists());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
