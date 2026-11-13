use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use llcontrol_image::{
    assets::{decode_asset, normalize_asset},
    prepare_jpeg_from_bytes, prepare_jpeg_from_image, FitMode, Quality,
};
use std::io::Cursor;

fn jpeg_with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    // Minimal little-endian TIFF IFD in a JPEG EXIF APP1 segment.
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend(orientation.to_le_bytes());
    exif.extend([0; 6]);
    let mut bytes = jpeg[..2].to_vec();
    bytes.extend([0xff, 0xe1]);
    bytes.extend(((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend(exif);
    bytes.extend(&jpeg[2..]);
    bytes
}

#[test]
fn exif_orientation_is_applied_once_before_normalization_and_lcd_fit() {
    let source = DynamicImage::ImageRgb8(RgbImage::from_fn(32, 16, |x, y| {
        Rgb([(x * 7) as u8, (y * 13) as u8, ((x + y) * 3) as u8])
    }));
    let mut encoded = Cursor::new(Vec::new());
    source.write_to(&mut encoded, ImageFormat::Jpeg).unwrap();
    let jpeg = encoded.into_inner();
    let raw = image::load_from_memory(&jpeg).unwrap().to_rgb8();
    let (w, h) = raw.dimensions();
    for orientation in 1..=8 {
        let (width, height) = if orientation >= 5 { (h, w) } else { (w, h) };
        let expected = RgbImage::from_fn(width, height, |x, y| {
            let (sx, sy) = match orientation {
                1 => (x, y),
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                8 => (w - 1 - y, x),
                _ => unreachable!(),
            };
            *raw.get_pixel(sx, sy)
        });
        let input = jpeg_with_orientation(&jpeg, orientation);
        assert_eq!(
            decode_asset(&input).unwrap().to_rgb8(),
            expected,
            "EXIF {orientation}"
        );
        let normalized = normalize_asset(&input).unwrap();
        assert_eq!(
            decode_asset(&normalized).unwrap().to_rgb8(),
            expected,
            "normalized EXIF {orientation} must not rotate again"
        );
        let actual = prepare_jpeg_from_bytes(
            &input,
            FitMode::Letterbox { bg: [0, 0, 0] },
            Quality::default(),
        )
        .unwrap();
        let expected_lcd = prepare_jpeg_from_image(
            expected.into(),
            FitMode::Letterbox { bg: [0, 0, 0] },
            Quality::default(),
        )
        .unwrap();
        assert_eq!(
            actual.bytes, expected_lcd.bytes,
            "LCD fit must use oriented dimensions for EXIF {orientation}"
        );
    }
}

#[test]
fn all_static_import_formats_normalize_without_changing_decoded_pixels() {
    let source = DynamicImage::ImageRgb8(RgbImage::from_fn(16, 8, |x, y| {
        Rgb([(x * 11) as u8, (y * 23) as u8, 90])
    }));
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::WebP,
        ImageFormat::Bmp,
    ] {
        let mut encoded = Cursor::new(Vec::new());
        source.write_to(&mut encoded, format).unwrap();
        let input = encoded.into_inner();
        let decoded = image::load_from_memory(&input).unwrap().to_rgba8();
        let normalized = normalize_asset(&input).unwrap();
        assert_eq!(image::guess_format(&normalized).unwrap(), ImageFormat::Png);
        assert_eq!(
            decode_asset(&normalized).unwrap().to_rgba8(),
            decoded,
            "{format:?}"
        );
        assert!(
            decode_asset(&input[..12.min(input.len())]).is_err(),
            "truncated {format:?}"
        );
    }
}

#[test]
fn editor_selection_uses_oriented_dimensions_before_crop() {
    use llcontrol_image::editor::{prepare_geometry, SquareCrop};
    let source = DynamicImage::ImageRgb8(RgbImage::from_fn(32, 16, |x, _| {
        if x < 16 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    }));
    let mut encoded = Cursor::new(Vec::new());
    source.write_to(&mut encoded, ImageFormat::Jpeg).unwrap();
    let input = jpeg_with_orientation(&encoded.into_inner(), 6);
    // The lower half only exists after EXIF rotates 32x16 into 16x32.
    let output = prepare_geometry(
        &input,
        Some(SquareCrop {
            x: 0,
            y: 16,
            side: 16,
        }),
        &[],
    )
    .unwrap();
    let frame = image::load_from_memory(&output).unwrap().into_rgb8();
    let pixel = frame.get_pixel(240, 240);
    assert!(pixel[2] > 240 && pixel[0] < 15 && pixel[1] < 15);
    assert_eq!(frame.dimensions(), (480, 480));
}
