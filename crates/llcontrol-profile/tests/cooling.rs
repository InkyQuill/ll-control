use base64::{engine::general_purpose::STANDARD, Engine};
use llcontrol_profile::cooling::{inspect_windows_cooling, Control, SpeedUnit};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const FIXTURE: &[u8] = include_bytes!("fixtures/Cooling.zip");
fn envelope() -> Value {
    let mut archive = ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("backup")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn profile() -> Value {
    serde_json::from_slice(
        &STANDARD
            .decode(envelope()["Datas"][1]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn archive(mut envelope: Value, profile: &Value) -> Vec<u8> {
    envelope["Datas"][1]["Data"] = json!(STANDARD.encode(serde_json::to_vec(profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&envelope).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
fn rejects(profile: &Value) {
    assert!(inspect_windows_cooling(Cursor::new(archive(envelope(), profile)), 1).is_err());
}

#[test]
fn dotnet_symbolic_dictionary_keys_and_all_inactive_modes_survive_normalization() {
    let report = inspect_windows_cooling(Cursor::new(FIXTURE), 1).unwrap();
    assert_eq!(report.fan.modes.len(), 8);
    assert_eq!(report.pump.modes.len(), 3);
    assert_eq!(report.fan.speed_unit, SpeedUnit::Percent);
    assert_eq!(report.pump.speed_unit, SpeedUnit::Rpm);
    assert_eq!(report.fan.selected_mode, "UserDefine2");
    assert_eq!(report.pump.selected_mode, "PWMQuiet");
    let source = profile();
    assert_eq!(report.fan.source_settings, source["Fan"]);
    assert_eq!(report.pump.source_settings, source["Pump"]);
    for (id, mode) in report.fan.modes.iter().enumerate() {
        assert_eq!(mode.control, Control::Curve);
        assert_eq!(mode.reference, "cpu");
        assert_eq!(mode.phases.len(), 7);
        assert_eq!((mode.phases[0].celsius, mode.phases[0].speed), (0, 10));
        assert_eq!((mode.phases[6].celsius, mode.phases[6].speed), (100, 100));
        assert_eq!(
            mode.phases[2].speed,
            40 + id as u16,
            "do not replace edited built-ins with templates"
        );
    }
    assert_eq!(
        report.fan.modes[0].name, "Quiet",
        "null name uses the source mode key, not host locale"
    );
    assert_eq!(report.pump.modes[0].control, Control::Fixed);
    assert_eq!(report.pump.modes[0].phases.len(), 1);
    assert_eq!(report.pump.modes[0].phases[0].speed, 2300);
    assert_eq!(report.pump.modes[1].phases[0].speed, 2000);
    assert_eq!(report.pump.modes[1].phases.last().unwrap().speed, 3000);
    assert_eq!(
        report.unconverted_controller_fields,
        ["FanLEDMode", "ScreenBrightness"]
    );
    assert!(!report.group_import_supported);
    assert!(!report.archive.asset_payloads_validated);
    assert!(!serde_json::to_string(&report)
        .unwrap()
        .contains("OWNED-SYNTHETIC-DEVICE-PATH"));
}

#[test]
fn record_and_archive_group_are_explicit_and_never_fall_back_to_another_record() {
    for record in [0, 2, usize::MAX] {
        assert!(inspect_windows_cooling(Cursor::new(FIXTURE), record).is_err());
    }
    let mut input = envelope();
    input["Type"] = json!(1);
    assert!(inspect_windows_cooling(Cursor::new(archive(input, &profile())), 1).is_err());
}

#[test]
fn invalid_inactive_modes_units_and_capabilities_block_the_whole_preparation() {
    let source = profile();
    for (key, value) in [
        ("SupportedStartStop", json!(true)),
        ("SpeedUnit", json!(0)),
        ("SupportedMotherboardSync", json!(true)),
        ("SupportedFanSize", json!([120])),
        ("SupportedRPMReferenceSource", json!([1])),
        ("Mode", json!(9)),
    ] {
        let mut p = source.clone();
        p["Fan"][key] = value;
        rejects(&p);
    }
    let mut p = source.clone();
    p["Fan"]["Profiles"]
        .as_object_mut()
        .unwrap()
        .remove("UserDefine4");
    rejects(&p);
    let mut p = source.clone();
    p["Pump"]["Profiles"]["PWM"] = p["Pump"]["Profiles"]["PWMQuiet"].clone();
    rejects(&p);
    for (key, value) in [
        ("Mode", json!(6)),
        ("IsStartStop", json!(true)),
        ("RPMReferenceSource", json!(1)),
        ("ControlType", json!(0)),
        ("FanSize", json!(140)),
        ("Name", json!("\n")),
        ("Unknown", json!(1)),
    ] {
        let mut p = source.clone();
        p["Fan"]["Profiles"]["UserDefine4"][key] = value;
        rejects(&p);
    }
}

#[test]
fn unsupported_phase_keys_and_unrepresentable_curves_are_reported_without_lossy_repair() {
    let source = profile();
    for key in [
        "0|0|1",
        "None|GPU|Curve",
        "None|CPU|Fixed",
        "None|CPU|Curve|Extra",
    ] {
        let mut p = source.clone();
        let infos = p["Fan"]["Profiles"]["UserDefine3"]["PhaseInfos"]
            .as_object_mut()
            .unwrap();
        let info = infos.remove("None|CPU|Curve").unwrap();
        infos.insert(key.into(), info);
        rejects(&p);
    }
    let ptr = "/Fan/Profiles/UserDefine3/PhaseInfos/None|CPU|Curve";
    for (key, value) in [
        ("MinSpeed", json!(0)),
        ("MaxSpeed", json!(101)),
        ("Phases", json!([])),
        (
            "Phases",
            json!([{"Temperature":0,"Speed":20},{"Temperature":70,"Speed":100}]),
        ),
        (
            "Phases",
            json!([{"Temperature":30,"Speed":20},{"Temperature":100,"Speed":100}]),
        ),
        (
            "Phases",
            json!([{"Temperature":30,"Speed":40},{"Temperature":60,"Speed":30}]),
        ),
        (
            "Phases",
            json!([{"Temperature":30,"Speed":20},{"Temperature":30,"Speed":50}]),
        ),
        (
            "Phases",
            json!([{"Temperature":30.5,"Speed":20},{"Temperature":70,"Speed":100}]),
        ),
    ] {
        let mut p = source.clone();
        p.pointer_mut(ptr).unwrap()[key] = value;
        rejects(&p);
    }
    let mut p = source.clone();
    p["Pump"]["Profiles"]["FixRPM"]["PhaseInfos"]["None|CPU|Fixed"]["Phases"][0]["Speed"] =
        json!(799);
    rejects(&p);
}

#[test]
fn presentation_metadata_is_retained_and_does_not_change_runtime_phases() {
    let mut source = profile();
    let info = &mut source["Fan"]["Profiles"]["UserDefine1"]["PhaseInfos"]["None|CPU|Curve"];
    info["RPMChartXAxisConfigs"] = json!({"Celsius":{"OwnedPresentation":42}});
    info["ChartSpeedLabels"] = json!(["Своя подпись"]);
    let report = inspect_windows_cooling(Cursor::new(archive(envelope(), &source)), 1).unwrap();
    assert_eq!(report.fan.source_settings, source["Fan"]);
    assert_eq!(report.fan.modes[4].phases[2].speed, 44);
}

#[test]
fn preparation_point_budget_includes_both_implicit_endpoints() {
    let mut source = profile();
    let ptr = "/Fan/Profiles/UserDefine1/PhaseInfos/None|CPU|Curve/Phases";
    let phases: Vec<_> = (1..=14)
        .map(|t| json!({"Temperature": t,"Speed":20+t}))
        .collect();
    *source.pointer_mut(ptr).unwrap() = json!(phases);
    let report = inspect_windows_cooling(Cursor::new(archive(envelope(), &source)), 1).unwrap();
    assert_eq!(report.fan.modes[4].phases.len(), 16);
    source
        .pointer_mut(ptr)
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!({"Temperature":15,"Speed":35}));
    rejects(&source);
    let mut source = profile();
    source["Fan"]["Profiles"]["UserDefine4"]["Name"] = json!("x".repeat(129));
    rejects(&source);
    let mut source = profile();
    let phases =
        &mut source["Pump"]["Profiles"]["FixRPM"]["PhaseInfos"]["None|CPU|Fixed"]["Phases"];
    phases
        .as_array_mut()
        .unwrap()
        .push(json!({"Temperature":70,"Speed":3000}));
    rejects(&source);
}

#[test]
fn empty_source_names_keep_factory_origin_while_explicit_factory_text_stays_literal() {
    let mut source = profile();
    source["Fan"]["Profiles"]["StandardSpeed"]["Name"] = json!("");
    source["Pump"]["Profiles"]["FixRPM"]["Name"] = json!("Fixed RPM");
    let inspected = inspect_windows_cooling(Cursor::new(archive(envelope(), &source)), 1).unwrap();
    let mapped =
        llcontrol_profile::linux_cooling::prepare(&inspected, "cpu:chosen", 0.0, 100).unwrap();
    let standard = mapped
        .cooling_banks
        .fan
        .iter()
        .find(|m| m.id == "StandardSpeed")
        .unwrap();
    assert_eq!(standard.name, "Standard");
    assert!(standard.use_default_name);
    let quiet = mapped
        .cooling_banks
        .fan
        .iter()
        .find(|m| m.id == "Quiet")
        .unwrap();
    assert!(quiet.use_default_name);
    assert_eq!(quiet.name, "Quiet");
    let fixed = mapped
        .cooling_banks
        .pump
        .iter()
        .find(|m| m.id == "FixRPM")
        .unwrap();
    assert!(!fixed.use_default_name);
    assert_eq!(fixed.name, "Fixed RPM");
    assert_eq!(
        mapped.cooling_config.fan_mode.as_deref(),
        Some("UserDefine2")
    );
    assert_eq!(mapped.cooling_config.pump_mode.as_deref(), Some("PWMQuiet"));
}
