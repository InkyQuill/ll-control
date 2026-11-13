#[path = "support/lighting.rs"]
mod fixture;
use llcontrol_profile::lighting::inspect_windows_lighting;
use serde_json::{json, Value};
use std::io::Cursor;
#[test]
fn dotnet_enum_dictionary_and_numeric_selection_match_owned_structural_fixture() {
    let dotnet =
        inspect_windows_lighting(Cursor::new(include_bytes!("fixtures/Lighting.zip")), 0).unwrap();
    let structural = inspect(&fixture::profile()).unwrap();
    assert_eq!(dotnet.selected_fan_mode, structural.selected_fan_mode);
    assert_eq!(dotnet.selected_pump_mode, structural.selected_pump_mode);
    assert_eq!(dotnet.fan, structural.fan);
    assert_eq!(dotnet.pump, structural.pump);
    assert_eq!(
        dotnet.effective_fan_led_count,
        structural.effective_fan_led_count
    );
}
fn inspect(
    profile: &Value,
) -> Result<llcontrol_profile::lighting::LightingInspection, llcontrol_profile::Error> {
    inspect_windows_lighting(Cursor::new(fixture::archive(profile, 0)), 0)
}
#[test]
fn all_static_dynamic_inactive_parameters_survive_without_device_identity_or_binding() {
    let source = fixture::profile();
    let result = inspect(&source).unwrap();
    assert_eq!((result.fan.len(), result.pump.len()), (6, 16));
    assert_eq!(result.selected_fan_mode, "Breathing");
    assert_eq!(result.selected_pump_mode, "BigBang");
    assert_eq!(result.effective_fan_led_count, 37);
    assert_eq!(result.fan_size_mm, 280);
    assert_eq!(result.fan["StaticColor"].speed, i32::MIN);
    for (name, pump) in &result.pump {
        assert_eq!(
            serde_json::to_value(pump).unwrap(),
            source["ScreenLEDProfiles"][name]
        );
        assert_eq!(pump.dynamic_settings.len(), 6);
    }
    let mut other = source.clone();
    other["FanLEDSyncToScreenLED"] = json!(true);
    assert_eq!(inspect(&other).unwrap().effective_fan_led_count, 16);
    other["FanLEDSize"] = json!(1);
    assert_eq!(inspect(&other).unwrap().effective_fan_led_count, 24);
    let report = serde_json::to_string(&result).unwrap();
    assert!(!report.contains("PRIVATE-WINDOWS-SERIAL"));
    assert_eq!(result.unconverted_controller_fields, ["ScreenBrightness"]);
    assert!(!result.group_import_supported && !result.sensor_binding_verified);
}
#[test]
fn unknown_or_missing_modes_and_mismatched_selection_are_never_dropped() {
    for path in [
        "/FanLEDProfiles/Meteor",
        "/ScreenLEDProfiles/Bounce",
        "/ScreenLEDProfiles/Rainbow/DynamicSettings/CoolantTemp",
    ] {
        let mut p = fixture::profile();
        let (parent, key) = path.rsplit_once('/').unwrap();
        p.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(inspect(&p).is_err(), "{path}");
    }
    for (path, value) in [
        ("/FanLEDMode", json!(0)),
        ("/ScreenLEDMode", json!(17)),
        ("/FanLEDSize", json!(2)),
        ("/FanLEDLoopNumber", json!(256)),
        ("/FanLEDProfiles/Rainbow/Mode", json!(2)),
        ("/ScreenLEDProfiles/Bounce/Mode", json!(15)),
        ("/ScreenLEDProfiles/Bounce/SensorType", json!(0)),
    ] {
        let mut p = fixture::profile();
        *p.pointer_mut(path).unwrap() = value;
        assert!(inspect(&p).is_err(), "{path}");
    }
    let mut p = fixture::profile();
    p["FanLEDProfiles"]["Unknown"] = p["FanLEDProfiles"]["Rainbow"].clone();
    assert!(inspect(&p).is_err());
    assert!(
        inspect_windows_lighting(Cursor::new(fixture::archive(&fixture::profile(), 1)), 0).is_err()
    );
    assert!(
        inspect_windows_lighting(Cursor::new(fixture::archive(&fixture::profile(), 0)), 1).is_err()
    );
}
#[test]
fn invalid_inactive_low_high_palettes_and_thresholds_reject_complete_preparation() {
    for (path, value) in [
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/High/Colors",
            json!([[1, 2, 3]]),
        ),
        (
            "/ScreenLEDProfiles/Voice/Static/Colors",
            json!([[1, 2, 300]]),
        ),
        (
            "/ScreenLEDProfiles/Voice/Static/Colors",
            json!([[255, 1, 2, 3]]),
        ),
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/Low/Speed",
            json!(-1),
        ),
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/High/Brightness",
            json!(101),
        ),
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/Low/Direction",
            json!(6),
        ),
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/LowValue",
            json!(100),
        ),
        (
            "/ScreenLEDProfiles/Bounce/DynamicSettings/CoolantTemp/UnitText",
            json!("bad\nunit"),
        ),
    ] {
        let mut p = fixture::profile();
        *p.pointer_mut(path).unwrap() = value;
        assert!(inspect(&p).is_err(), "{path}");
    }
    let mut p = fixture::profile();
    p["ScreenLEDProfiles"]["Bounce"]["DynamicSettings"]["CoolantTemp"]["HighValue"] = json!(35);
    assert!(
        inspect(&p).is_ok(),
        "source UI allows equal thresholds; do not invent interpolation semantics here"
    );
}
