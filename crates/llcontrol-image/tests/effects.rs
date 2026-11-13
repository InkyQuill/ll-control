use image::{Rgb, RgbImage};
use llcontrol_image::effects::*;
use llcontrol_ipc::*;
fn config(kind: WeatherKind, intensity: u8) -> WeatherEffect {
    WeatherEffect {
        retained_sensors: Vec::new(),
        presets: Vec::new(),
        kind,
        intensity,
        sensor: None,
        dynamic: false,
        seed: 42,
    }
}
fn canvas() -> RgbImage {
    RgbImage::from_fn(480, 480, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 17]))
}
#[test]
fn weather_frames_are_reproducible_move_with_session_time_and_preserve_background() {
    for kind in [WeatherKind::Rain, WeatherKind::Snow] {
        let effect = config(kind, 1);
        let background = canvas();
        let mut first = background.clone();
        draw_weather(&mut first, &effect, &[], 0).unwrap();
        let mut same = background.clone();
        draw_weather(&mut same, &effect, &[], 0).unwrap();
        assert_eq!(first, same);
        let mut later = background.clone();
        draw_weather(&mut later, &effect, &[], 42).unwrap();
        assert_ne!(first, later);
        let unchanged = first
            .pixels()
            .zip(background.pixels())
            .filter(|(a, b)| a == b)
            .count();
        assert!(
            unchanged > 480 * 480 / 2,
            "Weather must retain visible background"
        );
        assert!(unchanged < 480 * 480, "Weather must be visible");
        let mut final_time = background.clone();
        draw_weather(&mut final_time, &effect, &[], u64::MAX).unwrap();
    }
}
#[test]
fn rain_intensity_and_dynamic_source_change_visible_drop_density() {
    let count = |effect: &WeatherEffect, readings: &[SensorReading]| {
        let mut image = RgbImage::from_pixel(480, 480, Rgb([0, 0, 0]));
        draw_weather(&mut image, effect, readings, 500).unwrap();
        image.pixels().filter(|p| p.0 != [0, 0, 0]).count()
    };
    let low = config(WeatherKind::Rain, 1);
    let high = config(WeatherKind::Rain, 3);
    assert!(count(&high, &[]) > count(&low, &[]));
    let mut dynamic = low.clone();
    dynamic.dynamic = true;
    dynamic.sensor = Some(EffectSensor {
        sensor_id: "cpu/load".into(),
        unit: SensorUnit::Percent,
        low: 20,
        high: 80,
    });
    let reading = SensorReading {
        id: "cpu/load".into(),
        label: "CPU".into(),
        unit: SensorUnit::Percent,
        value: Some(90.0),
        status: SensorStatus::Valid,
        age_ms: Some(0),
        observed_unix_ms: Some(1),
    };
    assert!(count(&dynamic, &[reading]) > count(&high, &[]));
    assert_eq!(count(&dynamic, &[]), count(&low, &[]));
}
#[test]
fn invalid_configuration_and_wrong_canvas_do_not_mutate_the_frame() {
    let mut image = canvas();
    let original = image.clone();
    assert!(draw_weather(&mut image, &config(WeatherKind::Rain, 0), &[], 0).is_err());
    assert_eq!(image, original);
    let mut wrong = RgbImage::new(2, 2);
    assert!(matches!(
        draw_weather(&mut wrong, &config(WeatherKind::Snow, 1), &[], 0),
        Err(EffectError::FrameSize)
    ));
}

#[test]
fn sensor_speed_transition_preserves_position_and_changes_only_future_motion() {
    let mut effect = config(WeatherKind::Snow, 1);
    effect.dynamic = true;
    effect.sensor = Some(EffectSensor {
        sensor_id: "cpu/load".into(),
        unit: SensorUnit::Percent,
        low: 20,
        high: 80,
    });
    let value = |load| SensorReading {
        id: "cpu/load".into(),
        label: "CPU".into(),
        unit: SensorUnit::Percent,
        value: Some(load),
        status: SensorStatus::Valid,
        age_ms: Some(0),
        observed_unix_ms: Some(1),
    };
    let mut scene = WeatherScene::new(effect.clone()).unwrap();
    let mut fixed = WeatherScene::new(effect).unwrap();
    let mut first = canvas();
    scene.render(&mut first, &[value(20.0)], 2500).unwrap();
    let mut reference = canvas();
    fixed.render(&mut reference, &[value(20.0)], 2500).unwrap();
    assert_eq!(first, reference);
    let mut changed = canvas();
    assert_eq!(
        scene
            .render(&mut changed, &[value(44.0)], 2500)
            .unwrap()
            .amount,
        20
    );
    assert_eq!(
        first, changed,
        "New speed must not teleport existing particles"
    );
    let mut moving = canvas();
    scene.render(&mut moving, &[value(44.0)], 3500).unwrap();
    let mut slower = canvas();
    fixed.render(&mut slower, &[value(20.0)], 3500).unwrap();
    assert_ne!(moving, slower, "New speed must affect later frames");
    let mut rewind = canvas();
    let original = rewind.clone();
    assert!(matches!(
        scene.render(&mut rewind, &[value(44.0)], 3499),
        Err(EffectError::TimeReversed)
    ));
    assert_eq!(rewind, original);
    let mut retry = canvas();
    scene.render(&mut retry, &[value(44.0)], 3500).unwrap();
    assert_eq!(retry, moving, "Rejected rewind cannot mutate motion state");
}

#[test]
fn snow_equal_threshold_has_finite_motion_and_recovers_across_the_boundary() {
    let mut effect = config(WeatherKind::Snow, 1);
    effect.dynamic = true;
    effect.sensor = Some(EffectSensor {
        sensor_id: "cpu/load".into(),
        unit: SensorUnit::Percent,
        low: 40,
        high: 40,
    });
    let reading = |value| SensorReading {
        id: "cpu/load".into(),
        label: "CPU".into(),
        unit: SensorUnit::Percent,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    };
    let mut scene = WeatherScene::new(effect).unwrap();
    let mut first = canvas();
    assert_eq!(
        scene
            .render(&mut first, &[reading(40.0)], 0)
            .unwrap()
            .amount,
        10
    );
    let mut next = canvas();
    scene.render(&mut next, &[reading(40.0)], 42).unwrap();
    assert_ne!(first, next);
    for (time, value, expected) in [
        (84, 41.0, 50),
        (126, 40.0, 10),
        (168, 39.0, 10),
        (210, 40.0, 10),
    ] {
        let mut frame = canvas();
        assert_eq!(
            scene
                .render(&mut frame, &[reading(value)], time)
                .unwrap()
                .amount,
            expected
        );
        assert_ne!(
            frame,
            canvas(),
            "snow must remain visible after equality/boundary transitions"
        );
    }
}
