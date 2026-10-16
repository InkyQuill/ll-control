use lconnect_daemon::actor::dynamic_rgb::evaluate;
use lconnect_ipc::{DynamicRgbConfig, Rgb, SensorReading, SensorStatus, SensorUnit};
const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };
const GREEN: Rgb = Rgb { r: 0, g: 255, b: 0 };
fn config() -> DynamicRgbConfig {
    DynamicRgbConfig {
        sensor_id: "cpu".into(),
        unit: SensorUnit::Celsius,
        low_value: 30,
        high_value: 60,
        low_colors: vec![RED],
        high_colors: vec![BLUE],
        low_speed_percent: Some(100),
        high_speed_percent: Some(0),
        brightness_percent: None,
        direction: None,
    }
}
fn row(value: f64) -> SensorReading {
    SensorReading {
        id: "cpu".into(),
        label: "CPU".into(),
        unit: SensorUnit::Celsius,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
#[test]
fn reference_uses_integer_input_hsv_interpolation_and_high_first_equal_thresholds() {
    let mut config = config();
    assert_eq!(evaluate(&config, &[row(0.0)]).unwrap().colors, [RED]);
    assert_eq!(evaluate(&config, &[row(100.0)]).unwrap().colors, [BLUE]);
    let middle = evaluate(&config, &[row(45.9)]).unwrap();
    assert_eq!(middle.value, 45);
    assert_eq!(middle.colors, [GREEN]); // RGB interpolation would incorrectly produce purple.
    assert_eq!(middle.speed_percent, Some(50));
    config.high_value = config.low_value;
    assert_eq!(evaluate(&config, &[row(29.9)]).unwrap().colors, [RED]);
    assert_eq!(evaluate(&config, &[row(30.0)]).unwrap().colors, [BLUE]);
}
#[test]
fn reference_four_color_slots_and_negative_hue_clamp_are_preserved() {
    let mut config = config();
    config.low_colors = vec![
        RED,
        BLUE,
        Rgb {
            r: 255,
            g: 0,
            b: 255,
        },
        Rgb { r: 0, g: 0, b: 0 },
    ];
    config.high_colors = vec![
        BLUE,
        RED,
        BLUE,
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
    ];
    config.low_speed_percent = None;
    config.high_speed_percent = None;
    let output = evaluate(&config, &[row(45.0)]).unwrap();
    assert_eq!(
        output.colors,
        [
            GREEN,
            GREEN,
            GREEN,
            Rgb {
                r: 127,
                g: 127,
                b: 127
            }
        ]
    );
    assert_eq!(output.speed_percent, None);
    // Endpoint colors bypass HSV conversion, including the reference magenta quirk.
    assert_eq!(
        evaluate(&config, &[row(30.0)]).unwrap().colors,
        config.low_colors
    );
}
#[test]
fn unavailable_wrong_unit_stale_nonfinite_and_missing_binding_never_become_zero() {
    let config = config();
    assert!(evaluate(&config, &[]).is_none());
    for status in [
        SensorStatus::Unavailable,
        SensorStatus::Stale,
        SensorStatus::Invalid,
    ] {
        let mut value = row(45.0);
        value.status = status;
        assert!(evaluate(&config, &[value]).is_none());
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(evaluate(&config, &[row(value)]).is_none());
    }
    let mut value = row(45.0);
    value.unit = SensorUnit::Percent;
    assert!(evaluate(&config, &[value]).is_none());
    let mut value = row(45.0);
    value.age_ms = Some(3001);
    assert!(evaluate(&config, &[value]).is_none());
    let mut value = row(45.0);
    value.age_ms = None;
    assert!(evaluate(&config, &[value]).is_none());
}
#[test]
fn config_rejects_invalid_bounds_units_color_counts_and_half_speed_pair() {
    let mut value = config();
    value.high_value = 101;
    assert!(value.validate().is_err());
    value = config();
    value.low_value = 61;
    assert!(value.validate().is_err());
    value = config();
    value.unit = SensorUnit::Bytes;
    assert!(value.validate().is_err());
    value = config();
    value.high_colors.push(RED);
    assert!(value.validate().is_err());
    value = config();
    value.low_speed_percent = None;
    assert!(value.validate().is_err());
    value = config();
    value.unit = SensorUnit::Rpm;
    value.low_value = 800;
    value.high_value = 1200;
    let mut reading = row(5000.0);
    reading.unit = SensorUnit::Rpm;
    assert_eq!(evaluate(&value, &[reading]).unwrap().value, 3600);
}
