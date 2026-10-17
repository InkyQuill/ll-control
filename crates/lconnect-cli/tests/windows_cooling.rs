use lconnect_daemon::actor::cooling;
use lconnect_ipc::{CoolingPolicy, CoolingTarget, SensorReading, SensorStatus, SensorUnit};
use serde_json::Value;
use std::{
    fs, io,
    os::unix::net::UnixListener,
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .env("LCONNECTCTL_SOCKET", root.join("daemon.sock"))
        .env("XDG_RUNTIME_DIR", root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .args(args)
        .output()
        .unwrap()
}
fn temperature(value: f64) -> SensorReading {
    SensorReading {
        id: "fixture:selected".into(),
        label: "Owned CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
fn fixture(root: &Path) -> std::path::PathBuf {
    let file = root.join("windows.zip");
    fs::write(
        &file,
        include_bytes!("../../lconnect-profile/tests/fixtures/Cooling.zip"),
    )
    .unwrap();
    file
}

#[test]
fn offline_conversion_preserves_all_modes_and_feeds_the_actual_linux_cooling_evaluator() {
    let root = tempfile::tempdir().unwrap();
    let file = fixture(root.path());
    let original = fs::read(&file).unwrap();
    let listener = UnixListener::bind(root.path().join("daemon.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    for json_mode in [false, true] {
        let mut args = Vec::new();
        if json_mode {
            args.push("--json");
        }
        args.extend([
            "profiles",
            "windows-cooling",
            file.to_str().unwrap(),
            "--record",
            "1",
            "--cpu-sensor",
            "fixture:selected",
        ]);
        let output = run(root.path(), &args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["policy_conversion_only"], true);
        for field in [
            "group_import_supported",
            "apply_supported",
            "sensor_binding_verified",
            "source_presentation_metadata_validated",
        ] {
            assert_eq!(report[field], false);
        }
        assert_eq!(report["fan"]["selected_mode"], "UserDefine2");
        assert_eq!(report["pump"]["selected_mode"], "PWMQuiet");
        let config: lconnect_ipc::CoolingConfig =
            serde_json::from_value(report["cooling_config"].clone()).unwrap();
        let banks: lconnect_ipc::CoolingModeBanks =
            serde_json::from_value(report["cooling_banks"].clone()).unwrap();
        lconnect_ipc::validate_cooling_selection(&config, Some(&banks)).unwrap();
        assert_eq!(config.fan_mode.as_deref(), Some("UserDefine2"));
        assert_eq!(config.pump_mode.as_deref(), Some("PWMQuiet"));
        assert_eq!(
            config.fan.as_ref(),
            Some(
                &banks
                    .fan
                    .iter()
                    .find(|m| m.id == "UserDefine2")
                    .unwrap()
                    .policy
            )
        );
        assert_eq!(
            config.pump.as_ref(),
            Some(
                &banks
                    .pump
                    .iter()
                    .find(|m| m.id == "PWMQuiet")
                    .unwrap()
                    .policy
            )
        );
        assert_eq!(
            report["unconverted_controller_fields"],
            serde_json::json!(["FanLEDMode", "ScreenBrightness"])
        );
        for (key, target, count) in [
            ("fan", CoolingTarget::Fan, 8),
            ("pump", CoolingTarget::Pump, 3),
        ] {
            let modes = report[key]["modes"].as_array().unwrap();
            assert_eq!(modes.len(), count);
            for (index, mode) in modes.iter().enumerate() {
                let policy: CoolingPolicy = serde_json::from_value(mode["policy"].clone()).unwrap();
                policy.validate(target).unwrap();
                if let Some(id) = policy.sensor_id() {
                    assert_eq!(id, "fixture:selected");
                }
                if target == CoolingTarget::Fan {
                    assert_eq!(
                        cooling::evaluate(&policy, &[temperature(10.9)], &mut None),
                        (14, false, false)
                    );
                    assert_eq!(
                        cooling::evaluate(&policy, &[temperature(40.9)], &mut None),
                        (40 + index as u8, false, false)
                    );
                    assert_eq!(
                        cooling::evaluate(&policy, &[temperature(110.0)], &mut None),
                        (100, false, false)
                    );
                    assert_eq!(
                        cooling::evaluate(&policy, &[], &mut None),
                        (100, false, true)
                    );
                } else if mode["windows_mode"] == "FixRPM" {
                    assert_eq!(policy, CoolingPolicy::FixedRpm { rpm: 2300 });
                    assert_eq!(
                        cooling::evaluate(&policy, &[], &mut None),
                        (58, false, false)
                    );
                } else {
                    assert_eq!(
                        cooling::evaluate(&policy, &[temperature(40.9)], &mut None),
                        (55, false, false)
                    );
                    assert_eq!(
                        cooling::evaluate(&policy, &[], &mut None),
                        (100, false, true)
                    );
                }
            }
        }
        assert!(!String::from_utf8_lossy(&output.stdout).contains("OWNED-SYNTHETIC-DEVICE-PATH"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(fs::read(&file).unwrap(), original);
        assert!(!root.path().join("config").exists());
    }
}

#[test]
fn missing_selection_and_bad_local_options_never_connect_or_produce_a_partial_report() {
    let root = tempfile::tempdir().unwrap();
    let file = fixture(root.path());
    let listener = UnixListener::bind(root.path().join("daemon.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let common = ["profiles", "windows-cooling", file.to_str().unwrap()];
    for options in [
        vec!["--cpu-sensor", "fixture:selected"],
        vec!["--record", "1"],
        vec!["--record", "0", "--cpu-sensor", "fixture:selected"],
        vec!["--record", "1", "--cpu-sensor", ""],
        vec![
            "--record",
            "1",
            "--cpu-sensor",
            "fixture:selected",
            "--fallback-percent",
            "19",
        ],
        vec![
            "--record",
            "1",
            "--cpu-sensor",
            "fixture:selected",
            "--hysteresis-c",
            "NaN",
        ],
    ] {
        let mut args = common.to_vec();
        args.extend(options);
        let output = run(root.path(), &args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(!root.path().join("config").exists());
    }
}

#[test]
fn converted_banks_survive_toml_archive_transaction_restart_and_neighboring_screen_import() {
    use lconnect_daemon::{
        persistence::{DeviceSettings, PersistenceStore},
        profile_archive::ProfileArchives,
        profile_group::{GroupKind, GroupProfile},
    };
    use lconnect_ipc::CoolingModeBanks;
    use std::io::Cursor;
    let root = tempfile::tempdir().unwrap();
    let file = fixture(root.path());
    let output = run(
        root.path(),
        &[
            "profiles",
            "windows-cooling",
            file.to_str().unwrap(),
            "--record",
            "1",
            "--cpu-sensor",
            "fixture:selected",
        ],
    );
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let banks: CoolingModeBanks = serde_json::from_value(report["cooling_banks"].clone()).unwrap();
    banks.validate().unwrap();
    assert_eq!((banks.fan.len(), banks.pump.len()), (8, 3));
    let selected: lconnect_ipc::CoolingConfig =
        serde_json::from_value(report["cooling_config"].clone()).unwrap();
    let settings = DeviceSettings {
        cooling: selected.clone(),
        cooling_banks: Some(banks.clone()),
        ..Default::default()
    };
    let source = PersistenceStore::new_with_root(root.path().join("source"));
    source.write("device", &settings).unwrap();
    assert_eq!(
        source.read("device").unwrap().cooling_banks,
        Some(banks.clone())
    );
    let profile = GroupProfile::capture(&settings, &Default::default(), GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let archives = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let mut bytes = Vec::new();
    archives.export_group(&profile, &mut bytes).unwrap();
    let target = PersistenceStore::new_with_root(root.path().join("target"));
    let mut before = DeviceSettings::default();
    before.lcd.brightness = 73;
    target.write("device", &before).unwrap();
    let importer = ProfileArchives::new(
        target.theme_assets(),
        target.theme_fonts(),
        target.media_assets(),
    );
    let prepared = importer.prepare_group(&mut Cursor::new(bytes)).unwrap();
    let expected = target
        .snapshot_profile_group("device", &before, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let commit = target
        .commit_group_import("device", &before, &expected, &"a".repeat(32), &prepared)
        .unwrap();
    assert!(commit.readback.selected_content_current);
    assert_eq!(commit.readback.settings.lcd.brightness, 73);
    assert_eq!(commit.readback.settings.cooling, selected);
    assert_eq!(commit.readback.settings.cooling_banks, Some(banks.clone()));
    let reopened = PersistenceStore::new_with_root(root.path().join("target"));
    let saved = reopened.read("device").unwrap();
    assert_eq!(saved.cooling_banks, Some(banks.clone()));
    assert_eq!(saved.cooling, selected);
    let receipt = reopened
        .read_group_import("device", &saved, &"a".repeat(32))
        .unwrap()
        .unwrap();
    assert!(receipt.selected_content_current);
    let screen = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::Screen,
    )
    .unwrap()
    .profile;
    let expected = reopened
        .snapshot_profile_group("device", &saved, GroupKind::Screen)
        .unwrap()
        .revision;
    let (after, _) = reopened
        .commit_profile_group("device", &saved, &expected, &screen)
        .unwrap();
    assert_eq!(after.cooling_banks, Some(banks.clone()));
    assert_eq!(after.cooling, selected);
    assert_eq!(reopened.read("device").unwrap().cooling_banks, Some(banks));
    let saved = reopened.read("device").unwrap();
    let old = reopened
        .snapshot_profile_group("device", &saved, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let mut changed = saved.clone();
    changed.cooling_banks.as_mut().unwrap().fan[7].policy = CoolingPolicy::Fixed { percent: 49 };
    reopened.write("device", &changed).unwrap();
    assert!(
        !reopened
            .read_group_import("device", &changed, &"a".repeat(32))
            .unwrap()
            .unwrap()
            .selected_content_current
    );
    assert!(
        reopened
            .commit_profile_group("device", &changed, &old, &profile)
            .is_err(),
        "inactive modes participate in optimistic concurrency"
    );
    let bytes = fs::read(root.path().join("target/devices/device.toml")).unwrap();
    let mut invalid = changed.clone();
    invalid.cooling_banks.as_mut().unwrap().pump[0].policy = CoolingPolicy::FixedRpm { rpm: 0 };
    assert!(reopened.write("device", &invalid).is_err());
    assert_eq!(
        fs::read(root.path().join("target/devices/device.toml")).unwrap(),
        bytes
    );
    let legacy = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::FanAndLighting,
    )
    .unwrap()
    .profile;
    let mut json = serde_json::to_value(legacy).unwrap();
    json["content"]["settings"]
        .as_object_mut()
        .unwrap()
        .remove("cooling_banks");
    let legacy: GroupProfile = serde_json::from_value(json).unwrap();
    let expected = reopened
        .snapshot_profile_group("device", &changed, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let (reset, _) = reopened
        .commit_profile_group("device", &changed, &expected, &legacy)
        .unwrap();
    assert!(
        reset.cooling_banks.is_none(),
        "legacy absent banks replace old overrides, not keep stale imported modes"
    );
}
