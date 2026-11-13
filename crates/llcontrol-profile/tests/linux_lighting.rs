use llcontrol_profile::{
    lighting::inspect_windows_lighting,
    linux_lighting::{parse_bindings, prepare, SensorBindings},
};
use serde_json::{json, Value};
use std::io::Cursor;
fn source() -> llcontrol_profile::lighting::LightingInspection {
    inspect_windows_lighting(Cursor::new(include_bytes!("fixtures/Lighting.zip")), 0).unwrap()
}
fn bindings() -> SensorBindings {
    parse_bindings(br#"{"CPULoad":{"sensor_id":"cpu:load","unit":"percent"},"CPUTemperature":{"sensor_id":"cpu:temp","unit":"celsius"},"GPULoad":{"sensor_id":"gpu:load","unit":"percent"},"GPUTemperature":{"sensor_id":"gpu:temp","unit":"celsius"},"PumpRPM":{"sensor_id":"pump:rpm","unit":"rpm"},"CoolantTemp":{"sensor_id":"coolant:temp","unit":"celsius"}}"#).unwrap()
}
#[test]
fn all_modes_and_roles_map_with_selected_source_last_and_high_brightness_direction() {
    let source = source();
    let mapped = prepare(&source, &bindings()).unwrap();
    assert_eq!(
        (
            mapped.fan_modes.len(),
            mapped.pump_modes.len(),
            mapped.dynamic_rgb_profiles.modes.len()
        ),
        (6, 16, 16)
    );
    assert_eq!(mapped.selected_fan_mode, "breathing");
    assert_eq!(mapped.selected_pump_mode, "big_bang");
    assert_eq!(mapped.fan_topology.effective_led_count(), 37);
    assert_eq!(mapped.pump_modes["static"].speed_percent, 0);
    assert_eq!(
        mapped.pump_modes["big_bang"].brightness_percent, 73,
        "manual base remains separate"
    );
    let active = mapped.dynamic_rgb.unwrap();
    assert_eq!(active.sensor_id, "cpu:load");
    assert_eq!(active.brightness_percent, Some(90));
    assert_eq!(active.direction.as_deref(), Some("forward"));
    for (windows, profile) in &source.pump {
        let name = match windows.as_str() {
            "StaticColor" => "static".into(),
            other => {
                let mut out = String::new();
                for (i, c) in other.chars().enumerate() {
                    if i > 0 && c.is_uppercase() {
                        out.push('_');
                    }
                    out.extend(c.to_lowercase());
                }
                out
            }
        };
        let entry = mapped.dynamic_rgb_profiles.selected(&name).unwrap();
        assert_eq!(entry.bindings.len(), 6);
        assert_eq!(entry.enabled, profile.is_dynamic_mode);
        let role = [
            "CPULoad",
            "CPUTemperature",
            "GPULoad",
            "GPUTemperature",
            "PumpRPM",
            "CoolantTemp",
        ][usize::from(profile.sensor_type) - 1];
        assert_eq!(
            entry.bindings.last().unwrap().sensor_id,
            bindings()[role].sensor_id
        );
        for config in &entry.bindings {
            config.validate_mode(&name).unwrap();
        }
    }
}
#[test]
fn bindings_never_guess_roles_units_or_coalesce_distinct_source_settings() {
    let source = source();
    let mut map = bindings();
    map.remove("CoolantTemp");
    assert!(prepare(&source, &map).is_err());
    let mut map = bindings();
    map.get_mut("CPULoad").unwrap().unit = llcontrol_ipc::SensorUnit::Rpm;
    assert!(prepare(&source, &map).is_err());
    let mut map = bindings();
    map.get_mut("CoolantTemp").unwrap().sensor_id = "cpu:temp".into();
    assert!(prepare(&source, &map).is_err());
    for id in ["".to_owned(), "bad\nsource".into(), "x".repeat(1025)] {
        let mut map = bindings();
        map.get_mut("CPULoad").unwrap().sensor_id = id;
        assert!(prepare(&source, &map).is_err());
    }
    assert!(parse_bindings(br#"{"CPULoad":{},"CPULoad":{}}"#).is_err());
    assert!(parse_bindings(&vec![b' '; 16385]).is_err());
    let mut value: Value = serde_json::from_slice(
        br#"{"CPULoad":{"sensor_id":"cpu:load","unit":"percent","extra":1}}"#,
    )
    .unwrap();
    assert!(parse_bindings(&serde_json::to_vec(&value).unwrap()).is_err());
    value["CPULoad"]["unit"] = json!("fahrenheit");
    assert!(parse_bindings(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn inactive_unrepresentable_values_reject_instead_of_silently_clamping_or_dropping() {
    let mut s = source();
    s.fan_loop_number = 51;
    assert!(prepare(&s, &bindings()).is_err());
    let mut s = source();
    s.pump
        .get_mut("Bounce")
        .unwrap()
        .dynamic_settings
        .get_mut("CoolantTemp")
        .unwrap()
        .high
        .direction = 2;
    assert!(prepare(&s, &bindings()).is_err());
    let mut s = source();
    s.pump
        .get_mut("Bounce")
        .unwrap()
        .dynamic_settings
        .get_mut("CoolantTemp")
        .unwrap()
        .high
        .speed = i32::MIN;
    assert!(prepare(&s, &bindings()).is_err());
    let mut s = source();
    s.pump
        .get_mut("Bounce")
        .unwrap()
        .dynamic_settings
        .get_mut("CPULoad")
        .unwrap()
        .high_value = 101;
    assert!(prepare(&s, &bindings()).is_err());
    let mut s = source();
    s.pump
        .get_mut("Bounce")
        .unwrap()
        .dynamic_settings
        .get_mut("CPULoad")
        .unwrap()
        .high_value = 30;
    assert!(
        prepare(&s, &bindings()).is_ok(),
        "equal thresholds are representable as a step"
    );
}

#[test]
fn active_static_selection_keeps_other_dynamic_modes_and_selected_sources_without_activation() {
    let mut source = source();
    source.selected_pump_mode = "StaticColor".into();
    source.pump.get_mut("StaticColor").unwrap().is_dynamic_mode = false;
    let mapped = prepare(&source, &bindings()).unwrap();
    assert!(mapped.dynamic_rgb.is_none());
    assert_eq!(mapped.pump_modes["static"].dynamic_mode, Some(false));
    assert!(
        mapped
            .dynamic_rgb_profiles
            .selected("big_bang")
            .unwrap()
            .enabled
    );
    let disabled = mapped.dynamic_rgb_profiles.selected("static").unwrap();
    assert!(!disabled.enabled);
    assert_eq!(disabled.bindings.len(), 6);
    assert_eq!(disabled.bindings.last().unwrap().sensor_id, "gpu:load");
}
