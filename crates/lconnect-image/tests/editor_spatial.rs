use image::{Rgba, RgbaImage};
use lconnect_image::editor_filters::ColorFilters;

#[test]
fn spatial_filters_match_independent_shader_grid() {
    let source = [
        [30, 80, 180, 64],
        [60, 80, 160, 89],
        [90, 80, 140, 114],
        [120, 80, 120, 139],
        [150, 80, 100, 164],
        [37, 100, 180, 74],
        [67, 100, 160, 99],
        [97, 100, 140, 124],
        [127, 100, 120, 149],
        [157, 100, 100, 174],
        [44, 120, 180, 84],
        [74, 120, 160, 109],
        [104, 120, 140, 134],
        [134, 120, 120, 159],
        [164, 120, 100, 184],
    ];
    let cases = [
        (
            5.0,
            0.0,
            [
                [0, 4, 112, 64],
                [0, 0, 95, 89],
                [0, 13, 110, 114],
                [0, 22, 110, 139],
                [84, 42, 95, 164],
                [0, 0, 100, 74],
                [0, 0, 85, 99],
                [0, 4, 101, 124],
                [0, 12, 104, 149],
                [79, 37, 89, 174],
                [0, 55, 131, 84],
                [0, 40, 108, 109],
                [0, 51, 116, 134],
                [21, 59, 114, 159],
                [103, 80, 97, 184],
            ],
        ),
        (
            0.0,
            5.0,
            [
                [69, 95, 158, 89],
                [89, 94, 146, 103],
                [108, 94, 130, 120],
                [125, 93, 119, 137],
                [137, 93, 111, 151],
                [71, 101, 156, 93],
                [91, 100, 145, 107],
                [109, 101, 132, 124],
                [127, 101, 119, 141],
                [140, 100, 110, 155],
                [74, 110, 158, 97],
                [92, 108, 145, 111],
                [112, 108, 131, 128],
                [130, 107, 120, 145],
                [143, 109, 111, 159],
            ],
        ),
        (
            4.0,
            2.0,
            [
                [0, 17, 119, 73],
                [0, 22, 112, 93],
                [11, 28, 113, 117],
                [38, 38, 109, 140],
                [82, 48, 98, 159],
                [0, 26, 121, 80],
                [3, 28, 110, 100],
                [14, 35, 111, 124],
                [41, 43, 107, 148],
                [86, 55, 98, 167],
                [0, 56, 129, 87],
                [5, 52, 117, 107],
                [21, 58, 117, 131],
                [53, 66, 110, 155],
                [97, 76, 100, 174],
            ],
        ),
        (
            0.0,
            2.5,
            [
                [48, 88, 170, 75],
                [74, 88, 154, 93],
                [98, 87, 137, 117],
                [127, 87, 118, 141],
                [143, 87, 104, 159],
                [53, 103, 171, 82],
                [77, 102, 153, 100],
                [103, 101, 136, 124],
                [131, 100, 117, 148],
                [149, 101, 104, 166],
                [54, 115, 172, 89],
                [79, 114, 155, 107],
                [107, 113, 136, 131],
                [135, 114, 117, 155],
                [153, 113, 105, 173],
            ],
        ),
    ];
    for (sharpen, blur, expected) in cases {
        let mut frame = RgbaImage::from_fn(5, 3, |x, y| Rgba(source[(y * 5 + x) as usize]));
        ColorFilters {
            sharpen,
            blur,
            ..Default::default()
        }
        .apply(&mut frame)
        .unwrap();
        for (index, (pixel, expected)) in frame.pixels().zip(expected).enumerate() {
            for (actual, wanted) in pixel.0.into_iter().zip(expected) {
                assert!(
                    actual.abs_diff(wanted) <= 1,
                    "sharpen={sharpen} blur={blur} pixel={index}: {pixel:?} expected {expected:?}"
                );
            }
        }
    }
}

#[test]
fn blur_spreads_alpha_without_hidden_color_and_clamps_edges() {
    let mut frame = RgbaImage::from_pixel(5, 5, Rgba([0, 255, 0, 0]));
    frame.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
    ColorFilters {
        blur: 5.,
        ..Default::default()
    }
    .apply(&mut frame)
    .unwrap();
    assert!(frame.get_pixel(0, 0)[3] > frame.get_pixel(1, 0)[3]);
    assert!(frame.get_pixel(1, 0)[3] > 0);
    for p in frame.pixels() {
        assert_eq!(p[1], 0);
        assert_eq!(p[2], 0);
        if p[3] > 0 {
            assert_eq!(p[0], 255);
        }
    }
}

#[test]
fn spatial_controls_reject_invalid_values_without_modifying_source() {
    for (sharpen, blur) in [(-1., 0.), (0., 11.), (f32::NAN, 0.), (0., f32::INFINITY)] {
        let mut frame = RgbaImage::from_pixel(1, 1, Rgba([20, 30, 40, 255]));
        let before = frame.clone();
        assert!(ColorFilters {
            sharpen,
            blur,
            ..Default::default()
        }
        .apply(&mut frame)
        .is_err());
        assert_eq!(frame, before);
    }
}

#[test]
fn neutral_contrast_clears_sharpen_rgb_under_zero_alpha_before_blur() {
    let source = [
        [255, 0, 0, 255],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 255, 255],
    ];
    let expected = [
        [226, 0, 0, 104],
        [185, 0, 0, 84],
        [128, 0, 0, 68],
        [185, 0, 0, 84],
        [128, 0, 0, 82],
        [70, 0, 0, 84],
        [128, 0, 0, 68],
        [70, 0, 0, 84],
        [32, 0, 0, 104],
    ];
    let mut pixels = RgbaImage::from_fn(3, 3, |x, y| Rgba(source[(y * 3 + x) as usize]));
    ColorFilters {
        sharpen: 5.,
        blur: 5.,
        ..Default::default()
    }
    .apply(&mut pixels)
    .unwrap();
    for (pixel, expected) in pixels.pixels().zip(expected) {
        for (actual, wanted) in pixel.0.into_iter().zip(expected) {
            assert!(actual.abs_diff(wanted) <= 1, "{pixel:?} != {expected:?}");
        }
    }
}
