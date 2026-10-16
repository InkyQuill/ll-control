use lconnect_ipc::*;
fn source() -> MeteorSensor {
    MeteorSensor {
        binding: EffectSensor {
            sensor_id: "cpu/load".into(),
            unit: SensorUnit::Percent,
            low: 30,
            high: 60,
        },
        low_colors: [[0, 255, 0], [0, 0, 255]],
        high_colors: [[255, 0, 0], [255, 255, 0]],
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
fn native_static_export_and_dynamic_ceil_bands_match_installed_js_oracle() {
    let mut config = MeteorEffect::default();
    for speed in 1..=3 {
        config.speed = speed;
        assert_eq!(config.parameters(&[]).unwrap().speed, speed + 1);
    }
    config.dynamic = true;
    config.sensor = Some(source());
    for (value, speed, hsb) in [
        (30.0, 2, [[120.0, 100.0, 100.0], [240.0, 100.0, 100.0]]),
        (46.0, 3, [[72.0, 100.0, 100.0], [168.0, 100.0, 100.0]]),
        (50.0, 3, [[60.0, 100.0, 100.0], [150.0, 100.0, 100.0]]),
        (58.0, 4, [[0.0, 100.0, 100.0], [60.0, 100.0, 100.0]]),
    ] {
        let result = config.parameters(&[reading(value)]).unwrap();
        assert_eq!(result.speed, speed);
        assert_eq!(result.hsb, hsb);
    }
    assert_eq!(rgb_to_hsb([1, 2, 3]), [210.0, 67.0, 2.0]);
}
#[test]
fn each_source_keeps_palette_and_range_and_invalid_or_ambiguous_configs_fail() {
    let mut config = MeteorEffect {
        dynamic: true,
        sensor: Some(source()),
        ..Default::default()
    };
    let mut other = source();
    other.binding.sensor_id = "cpu/temp".into();
    other.binding.unit = SensorUnit::Celsius;
    other.low_colors = [[255, 255, 255]; 2];
    config.retained_sensors.push(other);
    config.validate().unwrap();
    let json = serde_json::to_vec(&config).unwrap();
    assert_eq!(
        serde_json::from_slice::<MeteorEffect>(&json).unwrap(),
        config
    );
    config.retained_sensors.push(source());
    assert!(config.validate().is_err());
    config.retained_sensors.clear();
    config.sensor.as_mut().unwrap().binding.low = 61;
    assert!(config.validate().is_err());
    config.sensor = Some(source());
    config.speed = 0;
    assert!(config.validate().is_err());
    config.speed = 2;
    config.sensor = None;
    assert!(config.validate().is_err());
    assert!(serde_json::from_str::<MeteorEffect>(
        r#"{"speed":2,"colors":[[0,0,0]],"dynamic":false,"sensor":null,"seed":42}"#
    )
    .is_err());
}
#[test]
fn missing_stale_wrong_unit_and_equal_bounds_never_generate_zero_or_nan() {
    let mut config = MeteorEffect {
        dynamic: true,
        sensor: Some(source()),
        ..Default::default()
    };
    for readings in [
        vec![],
        vec![{
            let mut r = reading(50.0);
            r.age_ms = Some(4000);
            r
        }],
        vec![{
            let mut r = reading(50.0);
            r.unit = SensorUnit::Rpm;
            r
        }],
        vec![reading(f64::NAN)],
    ] {
        let resolved = config.parameters(&readings).unwrap();
        assert!(resolved.unavailable);
        assert_eq!(resolved.speed, 3);
        assert_eq!(resolved.hsb, config.colors.map(rgb_to_hsb));
    }
    config.sensor.as_mut().unwrap().binding.low = 40;
    config.sensor.as_mut().unwrap().binding.high = 40;
    for (value, speed) in [(39.0, 2), (40.0, 2), (41.0, 4)] {
        let resolved = config.parameters(&[reading(value)]).unwrap();
        assert_eq!(resolved.speed, speed);
        assert!(resolved.hsb.iter().flatten().all(|v| v.is_finite()));
    }
}

#[test]
fn captured_installed_js_finite_outputs_match_speed_and_both_hsb_palettes() {
    #[derive(serde::Deserialize)]
    struct Oracle {
        low: i32,
        high: i32,
        value: f64,
        speed: u8,
        hsb: [[Option<f64>; 3]; 2],
    }
    let rows: Vec<Oracle> =
        serde_json::from_str(include_str!("fixtures/meteor-2.1.28-parameters.json")).unwrap();
    let mut compared = 0;
    for row in rows {
        let mut binding = source();
        binding.binding.low = row.low;
        binding.binding.high = row.high;
        let effect = MeteorEffect {
            dynamic: true,
            sensor: Some(binding),
            ..Default::default()
        };
        let actual = effect.parameters(&[reading(row.value)]).unwrap();
        if row.hsb.iter().flatten().any(Option::is_none) {
            assert_eq!(row.low, row.high);
            assert_eq!(row.value, f64::from(row.low));
            assert!(actual.hsb.iter().flatten().all(|v| v.is_finite()));
            continue;
        }
        assert_eq!(actual.speed, row.speed);
        for (actual, expected) in actual.hsb.iter().flatten().zip(row.hsb.iter().flatten()) {
            assert!((actual - expected.unwrap()).abs() < 1e-9);
        }
        compared += 1;
    }
    assert!(compared >= 18);
}

#[test]
fn meteor_theme_uses_single_first_scene_version_three_and_bounded_independent_pages() {
    let mut theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Meteor","background":[0,0,0],"layers":[{"kind":"meteor","effect":{"speed":2,"colors":[[0,255,0],[0,0,255]],"dynamic":false,"sensor":null,"seed":42}}]})).unwrap();
    theme.validate().unwrap();
    assert_eq!(theme.page_kind(), LcdPageKind::Meteor);
    assert!(theme.requires_streaming());
    let saved = theme.clone();
    theme.version = 1;
    assert!(theme.validate().is_err());
    theme = saved.clone();
    theme.layers.push(ThemeLayer::Meteor {
        effect: MeteorEffect::default(),
    });
    assert!(theme.validate().is_err());
    theme = saved.clone();
    let basic: LcdTheme = serde_json::from_value(
        serde_json::json!({"version":1,"name":"Basic","background":[0,0,0],"layers":[]}),
    )
    .unwrap();
    let mut root = basic.clone();
    root.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Meteor,
        theme: Box::new(saved.clone()),
    });
    root.validate().unwrap();
    assert!(!root.requires_streaming());
    let profile = ProfileContent::Screen(root);
    profile.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&serde_json::to_vec(&profile).unwrap()).unwrap(),
        profile
    );
    theme.layers.insert(
        0,
        ThemeLayer::Meteor {
            effect: MeteorEffect::default(),
        },
    );
    assert!(theme.validate().is_err());
}
