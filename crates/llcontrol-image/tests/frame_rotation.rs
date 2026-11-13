use image::{Rgb, RgbImage};
use llcontrol_image::FrameRotation;

#[test]
fn whole_frame_rotation_preserves_every_pixel_and_clockwise_orientation() {
    let frame = RgbImage::from_fn(3, 2, |x, y| Rgb([(y * 3 + x + 1) as u8, 0, 0]));
    for (degrees, dimensions, expected) in [
        (0, (3, 2), vec![1, 2, 3, 4, 5, 6]),
        (90, (2, 3), vec![4, 1, 5, 2, 6, 3]),
        (180, (3, 2), vec![6, 5, 4, 3, 2, 1]),
        (270, (2, 3), vec![3, 6, 2, 5, 1, 4]),
    ] {
        let rotated = FrameRotation::try_from(degrees)
            .unwrap()
            .apply(frame.clone());
        assert_eq!(rotated.dimensions(), dimensions);
        assert_eq!(
            rotated.pixels().map(|p| p.0[0]).collect::<Vec<_>>(),
            expected
        );
    }
    assert!(FrameRotation::try_from(45).is_err());
}
