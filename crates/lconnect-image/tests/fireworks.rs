use image::{Rgb, RgbImage};
use lconnect_image::fireworks::*;
use lconnect_ipc::*;
fn canvas() -> RgbImage {
    RgbImage::from_pixel(480, 480, Rgb([12, 18, 30]))
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
fn dynamic() -> FireworksEffect {
    FireworksEffect {
        dynamic: true,
        sensor: Some(FireworksSensor {
            binding: EffectSensor {
                sensor_id: "load".into(),
                unit: SensorUnit::Percent,
                low: 30,
                high: 60,
            },
            low_colors: [[255, 0, 0]; 6],
            high_colors: [[0, 0, 255]; 6],
        }),
        ..Default::default()
    }
}
#[test]
fn seeded_launch_burst_motion_preserves_background_and_distinguishes_intensities() {
    let mut results = Vec::new();
    for intensity in 1..=3 {
        let effect = FireworksEffect {
            intensity,
            ..Default::default()
        };
        let mut first = canvas();
        draw_fireworks(&mut first, &effect, &[], 3000).unwrap();
        let mut same = canvas();
        draw_fireworks(&mut same, &effect, &[], 3000).unwrap();
        assert_eq!(first, same);
        let unchanged = first.pixels().filter(|p| p.0 == [12, 18, 30]).count();
        assert!(unchanged > 480 * 480 / 2 && unchanged < 480 * 480);
        let mut later = canvas();
        draw_fireworks(&mut later, &effect, &[], 3042).unwrap();
        assert_ne!(first, later);
        results.push(first);
        let mut extreme = canvas();
        draw_fireworks(&mut extreme, &effect, &[], u64::MAX).unwrap();
    }
    assert_ne!(results[0], results[2]);
}
#[test]
fn new_palettes_apply_to_new_launches_and_never_recolor_existing_rockets_or_sparks() {
    let mut scene = FireworksScene::new(dynamic()).unwrap();
    let mut low = canvas();
    scene.render(&mut low, &[reading(20.0)], 2000).unwrap();
    let mut changed = canvas();
    scene.render(&mut changed, &[reading(100.0)], 2000).unwrap();
    assert_eq!(low, changed);
    let mut later = canvas();
    scene.render(&mut later, &[reading(100.0)], 2500).unwrap();
    assert_ne!(later, changed);
    assert!(later
        .pixels()
        .any(|p| u16::from(p[0]) > u16::from(p[2]) + 20));
    assert!(later
        .pixels()
        .any(|p| u16::from(p[2]) > u16::from(p[0]) + 20));
}
#[test]
fn rewind_and_wrong_canvas_do_not_mutate_pixels_or_advance_simulation() {
    let mut scene = FireworksScene::new(FireworksEffect::default()).unwrap();
    let mut first = canvas();
    scene.render(&mut first, &[], 3000).unwrap();
    let mut wrong = RgbImage::new(1, 1);
    assert!(matches!(
        scene.render(&mut wrong, &[], 4000),
        Err(FireworksError::FrameSize)
    ));
    let original = canvas();
    let mut rewind = original.clone();
    assert!(matches!(
        scene.render(&mut rewind, &[], 2999),
        Err(FireworksError::TimeReversed)
    ));
    assert_eq!(rewind, original);
    let mut same = canvas();
    scene.render(&mut same, &[], 3000).unwrap();
    assert_eq!(same, first);
}

#[test]
fn different_render_cadences_produce_the_same_fixed_step_state_within_catchup_window() {
    for intensity in [1, 3] {
        let effect = FireworksEffect {
            intensity,
            ..Default::default()
        };
        let mut incremental = FireworksScene::new(effect.clone()).unwrap();
        for millis in (0..9000).step_by(42) {
            incremental.render(&mut canvas(), &[], millis).unwrap();
        }
        let mut final_frame = canvas();
        incremental.render(&mut final_frame, &[], 9000).unwrap();
        let mut single_jump = canvas();
        draw_fireworks(&mut single_jump, &effect, &[], 9000).unwrap();
        assert_eq!(final_frame, single_jump);
    }
}
