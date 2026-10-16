use image::{Rgb, RgbImage};
use lconnect_image::meteor::*;
use lconnect_ipc::*;
fn canvas() -> RgbImage {
    RgbImage::from_pixel(480, 480, Rgb([10, 12, 14]))
}
fn reading(value: f64) -> SensorReading {
    SensorReading {
        id: "load".into(),
        label: "CPU".into(),
        unit: SensorUnit::Percent,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
fn dynamic() -> MeteorEffect {
    MeteorEffect {
        dynamic: true,
        sensor: Some(MeteorSensor {
            binding: EffectSensor {
                sensor_id: "load".into(),
                unit: SensorUnit::Percent,
                low: 30,
                high: 60,
            },
            low_colors: [[255, 0, 0]; 2],
            high_colors: [[0, 0, 255]; 2],
        }),
        ..Default::default()
    }
}
#[test]
fn scene_is_reproducible_moves_and_keeps_background_without_unbounded_history() {
    let effect = MeteorEffect::default();
    let background = canvas();
    let mut first = background.clone();
    draw_meteor(&mut first, &effect, &[], 0).unwrap();
    let mut same = background.clone();
    draw_meteor(&mut same, &effect, &[], 0).unwrap();
    assert_eq!(first, same);
    let unchanged = first
        .pixels()
        .zip(background.pixels())
        .filter(|(a, b)| a == b)
        .count();
    assert!(unchanged > 480 * 480 / 2 && unchanged < 480 * 480);
    let mut later = background.clone();
    draw_meteor(&mut later, &effect, &[], 42).unwrap();
    assert_ne!(first, later);
    let mut extreme = background.clone();
    draw_meteor(&mut extreme, &effect, &[], u64::MAX).unwrap();
}
#[test]
fn dynamic_palettes_change_visible_color_and_speed_change_does_not_teleport_particles() {
    let effect = dynamic();
    let mut low = canvas();
    let mut high = canvas();
    draw_meteor(&mut low, &effect, &[reading(20.0)], 500).unwrap();
    draw_meteor(&mut high, &effect, &[reading(100.0)], 500).unwrap();
    assert_ne!(low, high);
    assert!(low.pixels().any(|p| p[0] > p[2]));
    assert!(high.pixels().any(|p| p[2] > p[0]));
    let mut changing = effect.clone();
    changing.sensor.as_mut().unwrap().high_colors = changing.sensor.as_ref().unwrap().low_colors;
    let mut scene = MeteorScene::new(changing).unwrap();
    let mut frame = canvas();
    scene.render(&mut frame, &[reading(20.0)], 0).unwrap();
    let mut before = canvas();
    scene.render(&mut before, &[reading(20.0)], 500).unwrap();
    let mut transition = canvas();
    scene
        .render(&mut transition, &[reading(100.0)], 500)
        .unwrap();
    assert_eq!(before, transition);
    let mut next = canvas();
    scene.render(&mut next, &[reading(100.0)], 542).unwrap();
    assert_ne!(next, transition);
}
#[test]
fn malformed_canvas_and_rewind_fail_without_advancing_motion_or_mutating_pixels() {
    let mut scene = MeteorScene::new(dynamic()).unwrap();
    let mut first = canvas();
    scene.render(&mut first, &[reading(40.0)], 100).unwrap();
    let mut malformed = RgbImage::new(1, 1);
    assert!(matches!(
        scene.render(&mut malformed, &[], 200),
        Err(MeteorError::FrameSize)
    ));
    let original = canvas();
    let mut rewind = original.clone();
    assert!(matches!(
        scene.render(&mut rewind, &[], 50),
        Err(MeteorError::TimeReversed)
    ));
    assert_eq!(rewind, original);
    let mut current = canvas();
    scene.render(&mut current, &[reading(40.0)], 100).unwrap();
    assert_eq!(current, first);
}
