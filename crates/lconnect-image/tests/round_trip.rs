//! Integration tests round-tripping the 5 test images through the converter.

use lconnect_image::{prepare_jpeg_from_path, FitMode, Quality, LCD_HEIGHT, LCD_WIDTH};
use std::path::Path;

const TEST_IMAGES: &[&str] = &[
    "test-images/01-radial-gradient.png",
    "test-images/02-concentric-rings.png",
    "test-images/03-strawberry.png",
    "test-images/04-mountain-sunset.png",
    "test-images/05-clock-face.png",
];

fn fixture(name: &str) -> std::path::PathBuf {
    // tests run from the crate root.
    std::env::var("CARGO_MANIFEST_DIR")
        .map(|d| Path::new(&d).join(name))
        .unwrap_or_else(|_| Path::new(name).to_path_buf())
}

#[test]
fn each_test_image_round_trips_to_480x480_jpeg_crop_mode() {
    for name in TEST_IMAGES {
        let path = fixture(name);
        let out = prepare_jpeg_from_path(&path, FitMode::Crop, Quality::default())
            .unwrap_or_else(|e| panic!("prepare {name}: {e}"));
        assert_eq!(out.width, LCD_WIDTH);
        assert_eq!(out.height, LCD_HEIGHT);
        // Decode the output to confirm it's a valid JPEG of the right size.
        let decoded = image::load_from_memory(&out.bytes)
            .unwrap_or_else(|e| panic!("decode output of {name}: {e}"));
        assert_eq!(decoded.width(), LCD_WIDTH);
        assert_eq!(decoded.height(), LCD_HEIGHT);
    }
}

#[test]
fn each_test_image_round_trips_letterbox_with_black_background() {
    let mode = FitMode::Letterbox { bg: [0, 0, 0] };
    for name in TEST_IMAGES {
        let out = prepare_jpeg_from_path(fixture(name), mode, Quality::default())
            .unwrap_or_else(|e| panic!("prepare {name}: {e}"));
        assert_eq!(out.width, LCD_WIDTH);
        assert_eq!(out.height, LCD_HEIGHT);
        let decoded = image::load_from_memory(&out.bytes).expect("valid JPEG");
        assert_eq!((decoded.width(), decoded.height()), (LCD_WIDTH, LCD_HEIGHT));
    }
}

#[test]
fn stretch_mode_gives_480x480_even_for_non_square_input() {
    // The fixtures happen to be 1:1, so synthesise a non-square in memory.
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        320,
        240,
        image::Rgb([200, 100, 50]),
    ));
    let out = lconnect_image::prepare_jpeg_from_image(img, FitMode::Stretch, Quality::default())
        .expect("encode");
    assert_eq!((out.width, out.height), (LCD_WIDTH, LCD_HEIGHT));
    let decoded = image::load_from_memory(&out.bytes).expect("valid JPEG");
    assert_eq!((decoded.width(), decoded.height()), (LCD_WIDTH, LCD_HEIGHT));
}

#[test]
fn quality_rejects_out_of_range() {
    assert!(Quality::new(0).is_err());
    assert!(Quality::new(101).is_err());
    assert!(Quality::new(1).is_ok());
    assert!(Quality::new(50).is_ok());
    assert!(Quality::new(100).is_ok());
}

#[test]
fn jpeg_quality_affects_output_size() {
    // Higher quality should produce a larger byte buffer for a non-trivial image.
    let path = fixture("test-images/03-strawberry.png");
    let lo = prepare_jpeg_from_path(&path, FitMode::Crop, Quality::new(20).unwrap()).unwrap();
    let hi = prepare_jpeg_from_path(&path, FitMode::Crop, Quality::new(95).unwrap()).unwrap();
    assert!(
        hi.bytes.len() > lo.bytes.len(),
        "q95 ({}) should be > q20 ({})",
        hi.bytes.len(),
        lo.bytes.len()
    );
}
