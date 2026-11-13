use image::{Rgb, RgbImage};
use llcontrol_image::snow3d::*;
use llcontrol_ipc::*;
fn canvas() -> RgbImage {
    RgbImage::from_pixel(480, 480, Rgb([12, 18, 30]))
}
#[test]
fn perspective_snow_moves_reproducibly_and_preserves_visible_background_at_all_levels() {
    for intensity in 1..=3 {
        let config = Snow3dEffect {
            intensity,
            seed: 42,
        };
        let mut first = canvas();
        draw_snow3d(&mut first, &config, 0).unwrap();
        let mut same = canvas();
        draw_snow3d(&mut same, &config, 0).unwrap();
        assert_eq!(first, same);
        let unchanged = first.pixels().filter(|p| p.0 == [12, 18, 30]).count();
        assert!(unchanged > 0 && unchanged < 480 * 480);
        let mut later = canvas();
        draw_snow3d(&mut later, &config, 42).unwrap();
        assert_ne!(later, first);
        let mut extreme = canvas();
        draw_snow3d(&mut extreme, &config, u64::MAX).unwrap();
    }
}
#[test]
fn modes_have_different_depth_density_and_rejected_inputs_leave_pixels_untouched() {
    let mut low = canvas();
    let mut high = canvas();
    draw_snow3d(
        &mut low,
        &Snow3dEffect {
            intensity: 1,
            seed: 42,
        },
        1000,
    )
    .unwrap();
    draw_snow3d(
        &mut high,
        &Snow3dEffect {
            intensity: 3,
            seed: 42,
        },
        1000,
    )
    .unwrap();
    assert_ne!(low, high);
    let original = canvas();
    let mut invalid = original.clone();
    assert!(draw_snow3d(
        &mut invalid,
        &Snow3dEffect {
            intensity: 0,
            seed: 42
        },
        0
    )
    .is_err());
    assert_eq!(invalid, original);
    let mut wrong = RgbImage::new(1, 1);
    assert!(matches!(
        draw_snow3d(&mut wrong, &Snow3dEffect::default(), 0),
        Err(Snow3dError::FrameSize)
    ));
}
