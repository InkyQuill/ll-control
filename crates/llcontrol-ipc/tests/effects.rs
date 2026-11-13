use llcontrol_ipc::*;
fn effect(kind: WeatherKind) -> WeatherEffect {
    WeatherEffect {
        retained_sensors: Vec::new(),
        presets: Vec::new(),
        kind,
        intensity: 3,
        seed: 7,
        dynamic: true,
        sensor: Some(EffectSensor {
            sensor_id: "cpu/load".into(),
            unit: SensorUnit::Percent,
            low: 20,
            high: 80,
        }),
    }
}
fn reading(value: f64) -> SensorReading {
    SensorReading {
        id: "cpu/load".into(),
        label: "CPU".into(),
        unit: SensorUnit::Percent,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
#[test]
fn windows_static_selectors_and_strict_rain_thresholds_are_preserved() {
    for (kind, expected) in [
        (WeatherKind::Rain, [1, 2, 4]),
        (WeatherKind::Snow, [10, 30, 60]),
    ] {
        let mut config = effect(kind);
        config.sensor = None;
        config.dynamic = false;
        for intensity in 1..=3 {
            config.intensity = intensity;
            assert_eq!(
                config.parameters(&[]).unwrap().amount,
                expected[usize::from(intensity - 1)]
            );
        }
    }
    for (value, expected) in [(19.9, 1), (20.0, 2), (50.0, 2), (80.0, 2), (80.1, 6)] {
        assert_eq!(
            effect(WeatherKind::Rain)
                .parameters(&[reading(value)])
                .unwrap()
                .amount,
            expected
        );
    }
}
#[test]
fn snow_ceil_bands_and_dynamic_high_differ_from_static_high() {
    let mut config = effect(WeatherKind::Snow);
    config.sensor.as_mut().unwrap().high = 83;
    // ceil((83-20)/5)=13; truncation at each 13-unit boundary.
    for (value, expected) in [
        (0.0, 10),
        (20.0, 10),
        (32.9, 10),
        (33.0, 10),
        (46.0, 20),
        (59.0, 30),
        (72.0, 40),
        (83.0, 40),
        (85.0, 50),
        (100.0, 50),
    ] {
        assert_eq!(
            config.parameters(&[reading(value)]).unwrap().amount,
            expected,
            "value {value}"
        );
    }
}
#[test]
fn unavailable_sources_use_retained_static_intensity_without_fake_zero() {
    let config = effect(WeatherKind::Rain);
    let fallback = WeatherParameters {
        amount: 4,
        unavailable: true,
    };
    assert_eq!(config.parameters(&[]).unwrap(), fallback);
    let mut value = reading(90.0);
    for status in [
        SensorStatus::Invalid,
        SensorStatus::Unavailable,
        SensorStatus::Stale,
    ] {
        value.status = status;
        assert_eq!(config.parameters(&[value.clone()]).unwrap(), fallback);
    }
    value.status = SensorStatus::Valid;
    for age in [None, Some(3001)] {
        value.age_ms = age;
        assert_eq!(config.parameters(&[value.clone()]).unwrap(), fallback);
    }
    value.age_ms = Some(3000);
    assert_eq!(config.parameters(&[value.clone()]).unwrap().amount, 6);
    value.unit = SensorUnit::Rpm;
    assert_eq!(config.parameters(&[value.clone()]).unwrap(), fallback);
    value.unit = SensorUnit::Percent;
    value.id = "other/cpu".into();
    assert_eq!(config.parameters(&[value.clone()]).unwrap(), fallback);
    value.id = "cpu/load".into();
    value.value = Some(f64::NAN);
    assert_eq!(config.parameters(&[value]).unwrap(), fallback);
}
#[test]
fn invalid_weather_settings_fail_before_render_admission() {
    let mut config = effect(WeatherKind::Snow);
    config.intensity = 0;
    assert!(config.validate().is_err());
    config.intensity = 4;
    assert!(config.validate().is_err());
    config.intensity = 1;
    config.sensor.as_mut().unwrap().high = 19;
    assert!(config.validate().is_err());
    config.sensor.as_mut().unwrap().high = 80;
    config.sensor.as_mut().unwrap().sensor_id = "\0".into();
    assert!(config.validate().is_err());
    config.sensor.as_mut().unwrap().sensor_id = "cpu/load".into();
    config.sensor.as_mut().unwrap().unit = SensorUnit::Volts;
    assert!(config.validate().is_err());
    config.sensor = None;
    config.dynamic = false;
    assert!(config.validate().is_ok());
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(
        serde_json::from_value::<WeatherEffect>(json.clone()).unwrap(),
        config
    );
    let mut extra = json;
    extra["vendor_path"] = serde_json::json!("arbitrary");
    assert!(serde_json::from_value::<WeatherEffect>(extra).is_err());
}

#[test]
fn weather_theme_version_order_and_profile_roundtrip_are_explicit() {
    let mut theme = LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 3,
        name: "Rain".into(),
        background: [0; 3],
        background_fill: None,
        background_image: None,
        background_video: None,
        layers: vec![ThemeLayer::Weather {
            editor: None,
            effect: effect(WeatherKind::Rain),
        }],
    };
    theme.validate().unwrap();
    assert!(theme.requires_streaming());
    assert!(theme.image_assets().is_empty());
    assert!(theme.font_assets().is_empty());
    let profile = ProfileContent::Screen(theme.clone());
    let encoded = serde_json::to_vec(&profile).unwrap();
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&encoded).unwrap(),
        profile
    );
    for version in [1, 2, 4] {
        theme.version = version;
        assert!(theme.validate().is_err());
    }
    theme.version = 3;
    theme.background_video = Some("a".repeat(32));
    theme.validate().unwrap();
    theme.layers.push(theme.layers[0].clone());
    assert!(theme.validate().is_err());
    theme.layers.pop();
    theme.layers.clear();
    assert!(theme.validate().is_err());
}

#[test]
fn static_mode_preserves_dynamic_settings_without_using_the_sensor_value() {
    let mut config = effect(WeatherKind::Snow);
    config.dynamic = false;
    assert_eq!(
        config.parameters(&[reading(100.0)]).unwrap(),
        WeatherParameters {
            amount: 60,
            unavailable: false
        }
    );
    assert!(config.sensor.is_some());
    config.dynamic = true;
    assert_eq!(config.parameters(&[reading(100.0)]).unwrap().amount, 50);
    config.sensor = None;
    assert!(config.validate().is_err());
}

#[test]
fn retained_ranges_roundtrip_and_do_not_drive_the_selected_effect() {
    let mut config = effect(WeatherKind::Rain);
    config.retained_sensors.push(EffectSensor {
        sensor_id: "pump/rpm".into(),
        unit: SensorUnit::Rpm,
        low: 800,
        high: 1200,
    });
    // Native unit is part of identity, so a former provider unit remains separate.
    config.retained_sensors.push(EffectSensor {
        sensor_id: "cpu/load".into(),
        unit: SensorUnit::Celsius,
        low: 30,
        high: 60,
    });
    config.validate().unwrap();
    let encoded = serde_json::to_vec(&config).unwrap();
    assert_eq!(
        serde_json::from_slice::<WeatherEffect>(&encoded).unwrap(),
        config
    );
    assert_eq!(config.parameters(&[reading(90.0)]).unwrap().amount, 6);
    let mut retained_reading = reading(1500.0);
    retained_reading.id = "pump/rpm".into();
    retained_reading.unit = SensorUnit::Rpm;
    assert!(config.parameters(&[retained_reading]).unwrap().unavailable);
    // An old v3 document without the new optional field remains readable.
    let mut old = serde_json::to_value(&config).unwrap();
    old.as_object_mut().unwrap().remove("retained_sensors");
    let old: WeatherEffect = serde_json::from_value(old).unwrap();
    assert!(old.retained_sensors.is_empty());
    old.validate().unwrap();
}

#[test]
fn invalid_inactive_ranges_and_duplicate_identities_are_rejected() {
    let mut config = effect(WeatherKind::Snow);
    let active = config.sensor.clone().unwrap();
    config.retained_sensors.push(active.clone());
    assert!(config.validate().is_err());
    config.retained_sensors[0].sensor_id = "other".into();
    config.validate().unwrap();
    config
        .retained_sensors
        .push(config.retained_sensors[0].clone());
    assert!(config.validate().is_err());
    config.retained_sensors.pop();
    for bad in [
        EffectSensor {
            low: 80,
            high: 20,
            ..active.clone()
        },
        EffectSensor {
            sensor_id: "\0".into(),
            ..active.clone()
        },
        EffectSensor {
            unit: SensorUnit::Volts,
            ..active.clone()
        },
    ] {
        config.retained_sensors = vec![bad];
        assert!(config.validate().is_err());
    }
    config.retained_sensors = (0..63)
        .map(|i| EffectSensor {
            sensor_id: format!("source/{i}"),
            ..active.clone()
        })
        .collect();
    config.validate().unwrap();
    config.retained_sensors.push(EffectSensor {
        sensor_id: "overflow".into(),
        ..active
    });
    assert!(config.validate().is_err());
}

#[test]
fn independent_weather_pages_survive_profiles_and_only_active_settings_drive_rendering() {
    let mut config = effect(WeatherKind::Rain);
    let mut other = effect(WeatherKind::Snow);
    other.dynamic = false;
    other.intensity = 2;
    other.seed = 123;
    let other: WeatherPreset =
        serde_json::from_value(serde_json::to_value(other).unwrap()).unwrap();
    config.presets = vec![other];
    config.validate().unwrap();
    assert_eq!(config.parameters(&[reading(90.0)]).unwrap().amount, 6);
    let theme = LcdTheme {
        page_cache: Vec::new(),
        basic_editor: None,
        version: 3,
        name: "Independent pages".into(),
        background: [0; 3],
        background_fill: None,
        background_image: None,
        background_video: None,
        layers: vec![ThemeLayer::Weather {
            editor: None,
            effect: config.clone(),
        }],
    };
    let profile = ProfileContent::Screen(theme);
    profile.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&serde_json::to_vec(&profile).unwrap()).unwrap(),
        profile
    );
    let mut old = serde_json::to_value(&config).unwrap();
    old.as_object_mut().unwrap().remove("presets");
    assert!(serde_json::from_value::<WeatherEffect>(old)
        .unwrap()
        .presets
        .is_empty());
    config.presets[0].kind = WeatherKind::Rain;
    assert!(config.validate().is_err());
    config.presets[0].kind = WeatherKind::Snow;
    config.presets.push(config.presets[0].clone());
    assert!(config.validate().is_err());
    config.presets.pop();
    config.presets[0].sensor.as_mut().unwrap().low = 81;
    assert!(config.validate().is_err());
    config.presets[0].sensor.as_mut().unwrap().low = 20;
    let mut nested = serde_json::to_value(&config).unwrap();
    nested["presets"][0]["presets"] = serde_json::json!([]);
    assert!(serde_json::from_value::<WeatherEffect>(nested).is_err());
}

#[test]
fn equal_thresholds_match_native_finite_branches_and_snow_nan_is_resolved() {
    for kind in [WeatherKind::Rain, WeatherKind::Snow] {
        for threshold in [0, 40, 100] {
            let mut config = effect(kind);
            config.sensor.as_mut().unwrap().low = threshold;
            config.sensor.as_mut().unwrap().high = threshold;
            config.validate().unwrap();
            let expected = if kind == WeatherKind::Rain {
                [1, 2, 6]
            } else {
                [10, 10, 50]
            };
            for (offset, amount) in [-1, 0, 1].into_iter().zip(expected) {
                assert_eq!(
                    config
                        .parameters(&[reading(f64::from(threshold + offset))])
                        .unwrap()
                        .amount,
                    amount
                );
            }
            config.dynamic = false;
            config.validate().unwrap();
            assert_eq!(
                config.parameters(&[]).unwrap().amount,
                if kind == WeatherKind::Rain { 4 } else { 60 }
            );
        }
    }
}
