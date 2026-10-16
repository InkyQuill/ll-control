use lconnect_ipc::*;
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
fn source() -> FireworksSensor {
    FireworksSensor {
        binding: EffectSensor {
            sensor_id: "cpu/load".into(),
            unit: SensorUnit::Percent,
            low: 30,
            high: 60,
        },
        low_colors: FIREWORKS_COLORS,
        high_colors: FIREWORKS_COLORS,
    }
}
#[test]
fn all_six_native_hsb_palettes_and_launch_rates_match_installed_js_finite_outputs() {
    #[derive(serde::Deserialize)]
    struct Oracle {
        low_colors: [[u8; 3]; 6],
        high_colors: [[u8; 3]; 6],
        rows: Vec<Row>,
    }
    #[derive(serde::Deserialize)]
    struct Row {
        low: i32,
        high: i32,
        value: f64,
        rate: f64,
        hsb: [[Option<f64>; 3]; 6],
    }
    let oracle: Oracle =
        serde_json::from_str(include_str!("fixtures/fireworks-2.1.28-parameters.json")).unwrap();
    let mut compared = 0;
    for row in oracle.rows {
        let mut source = source();
        source.binding.low = row.low;
        source.binding.high = row.high;
        source.low_colors = oracle.low_colors;
        source.high_colors = oracle.high_colors;
        let effect = FireworksEffect {
            dynamic: true,
            sensor: Some(source),
            ..Default::default()
        };
        let actual = effect.parameters(&[reading(row.value)]).unwrap();
        if row.hsb.iter().flatten().any(Option::is_none) {
            assert_eq!(row.low, row.high);
            assert_eq!(row.value, f64::from(row.low));
            assert!(actual.hsb.iter().flatten().all(|v| v.is_finite()));
            assert_eq!(actual.rate, 0.1);
            continue;
        }
        assert!((actual.rate - row.rate).abs() < 1e-9);
        for (actual, expected) in actual.hsb.iter().flatten().zip(row.hsb.iter().flatten()) {
            assert!((actual - expected.unwrap()).abs() < 1e-9);
        }
        compared += 1;
    }
    assert!(compared >= 17);
}
#[test]
fn independent_source_colors_ranges_roundtrip_and_malformed_or_duplicate_settings_fail() {
    let mut config = FireworksEffect {
        dynamic: true,
        sensor: Some(source()),
        ..Default::default()
    };
    let mut other = source();
    other.binding.sensor_id = "cpu/temp".into();
    other.binding.unit = SensorUnit::Celsius;
    other.low_colors = [[255, 255, 255]; 6];
    config.retained_sensors.push(other);
    config.validate().unwrap();
    assert_eq!(
        serde_json::from_slice::<FireworksEffect>(&serde_json::to_vec(&config).unwrap()).unwrap(),
        config
    );
    config.retained_sensors.push(source());
    assert!(config.validate().is_err());
    config.retained_sensors.clear();
    config.intensity = 4;
    assert!(config.validate().is_err());
    config.intensity = 1;
    config.sensor = None;
    assert!(config.validate().is_err());
    assert!(serde_json::from_str::<FireworksEffect>(
        r#"{"intensity":1,"colors":[[0,255,0],[0,0,255]],"dynamic":false,"sensor":null,"seed":42}"#
    )
    .is_err());
}
#[test]
fn static_probability_and_unavailable_fallback_do_not_invent_a_zero_reading() {
    let mut config = FireworksEffect::default();
    for intensity in 1..=3 {
        config.intensity = intensity;
        assert!((config.parameters(&[]).unwrap().rate - f64::from(intensity) * 0.1).abs() < 1e-9);
    }
    config.dynamic = true;
    config.sensor = Some(source());
    for readings in [
        vec![],
        vec![{
            let mut r = reading(40.0);
            r.age_ms = Some(4000);
            r
        }],
        vec![{
            let mut r = reading(40.0);
            r.unit = SensorUnit::Rpm;
            r
        }],
    ] {
        let parameters = config.parameters(&readings).unwrap();
        assert!(parameters.unavailable);
        assert!((parameters.rate - 0.3).abs() < 1e-9);
        assert_eq!(parameters.hsb, FIREWORKS_COLORS.map(rgb_to_hsb));
    }
}

#[test]
fn fireworks_theme_is_single_leading_scene_and_roundtrips_in_an_inactive_profile_page() {
    let mut theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":3,"name":"Fireworks","background":[0,0,0],"layers":[{"kind":"fireworks","effect":{"intensity":1,"colors":FIREWORKS_COLORS,"dynamic":false,"sensor":null,"seed":42}}]})).unwrap();
    theme.validate().unwrap();
    assert!(theme.requires_streaming());
    assert_eq!(theme.page_kind(), LcdPageKind::Fireworks);
    let mut basic: LcdTheme = serde_json::from_value(
        serde_json::json!({"version":1,"name":"Basic","background":[0,0,0],"layers":[]}),
    )
    .unwrap();
    basic.page_cache.push(SavedLcdPage {
        page: LcdPageKind::Fireworks,
        theme: Box::new(theme.clone()),
    });
    basic.validate().unwrap();
    assert!(!basic.requires_streaming());
    let profile = ProfileContent::Screen(basic);
    assert_eq!(
        serde_json::from_slice::<ProfileContent>(&serde_json::to_vec(&profile).unwrap()).unwrap(),
        profile
    );
    theme.layers.push(ThemeLayer::Fireworks {
        effect: FireworksEffect::default(),
    });
    assert!(theme.validate().is_err());
}
