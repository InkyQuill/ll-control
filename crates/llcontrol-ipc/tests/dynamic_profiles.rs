use llcontrol_ipc::*;
use serde_json::json;
fn cfg(id: &str, unit: SensorUnit) -> DynamicRgbConfig {
    serde_json::from_value(json!({"sensor_id":id,"unit":unit,"low_value":30,"high_value":60,"low_colors":[{"r":1,"g":2,"b":3}],"high_colors":[{"r":4,"g":5,"b":6}],"low_speed_percent":null,"high_speed_percent":null,"brightness_percent":75,"direction":"forward"})).unwrap()
}
#[test]
fn profiles_keep_mode_source_unit_identity_and_last_selection_without_duplicates() {
    let mut profiles = DynamicRgbProfiles::default();
    profiles
        .remember("static_color", cfg("same", SensorUnit::Celsius))
        .unwrap();
    profiles
        .remember("static", cfg("same", SensorUnit::Percent))
        .unwrap();
    let mut changed = cfg("same", SensorUnit::Celsius);
    changed.low_value = 45;
    profiles.remember("static", changed).unwrap();
    assert_eq!(profiles.modes.len(), 1);
    assert_eq!(profiles.modes[0].mode, "static");
    assert_eq!(profiles.modes[0].bindings.len(), 2);
    assert_eq!(profiles.modes[0].bindings[0].unit, SensorUnit::Percent);
    assert_eq!(profiles.modes[0].bindings[1].low_value, 45);
    let raw = serde_json::to_vec(&profiles).unwrap();
    assert_eq!(
        serde_json::from_slice::<DynamicRgbProfiles>(&raw).unwrap(),
        profiles
    );
    let old = profiles.clone();
    let mut invalid = cfg("bad", SensorUnit::Celsius);
    invalid.direction = Some("reverse".into());
    assert!(profiles.remember("static", invalid).is_err());
    assert_eq!(profiles, old);
    assert!(profiles
        .remember("meteor", cfg("bad", SensorUnit::Celsius))
        .is_err());
    assert_eq!(profiles, old);
}
#[test]
fn profiles_enforce_bounds_and_reject_duplicate_or_mismatched_configuration() {
    let mut p = DynamicRgbProfiles::default();
    for n in 0..64 {
        p.remember("static", cfg(&format!("id:{n}"), SensorUnit::Celsius))
            .unwrap();
    }
    let old = p.clone();
    assert!(p
        .remember("static", cfg("overflow", SensorUnit::Celsius))
        .is_err());
    assert_eq!(p, old);
    let duplicate = p.modes[0].bindings[0].clone();
    p.modes[0].bindings.push(duplicate);
    assert!(p.validate().is_err());
    let mut c = cfg("x", SensorUnit::Celsius);
    c.brightness_percent = Some(101);
    assert!(c.validate().is_err());
    c.brightness_percent = None;
    c.direction = Some("left".into());
    assert!(c.validate().is_err());
}

#[test]
fn disabling_a_mode_retains_bindings_and_other_modes_choice() {
    let mut profiles = DynamicRgbProfiles::default();
    profiles
        .remember("static", cfg("cpu", SensorUnit::Celsius))
        .unwrap();
    let mut breathing = cfg("gpu", SensorUnit::Percent);
    breathing.low_speed_percent = Some(25);
    breathing.high_speed_percent = Some(75);
    profiles.remember("breathing", breathing).unwrap();
    profiles.disable("static_color");
    assert!(!profiles.selected("static").unwrap().enabled);
    assert_eq!(
        profiles.selected("static").unwrap().bindings[0].sensor_id,
        "cpu"
    );
    assert!(profiles.selected("breathing").unwrap().enabled);
    let legacy = json!({"modes":[{"mode":"static","bindings":[cfg("old",SensorUnit::Celsius)]}]});
    let restored: DynamicRgbProfiles = serde_json::from_value(legacy).unwrap();
    assert!(!restored.selected("static").unwrap().enabled);
    profiles
        .remember("static", cfg("cpu", SensorUnit::Celsius))
        .unwrap();
    assert!(profiles.selected("static").unwrap().enabled);
}
