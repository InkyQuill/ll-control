use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use lconnect_image::assets::*;
use std::io::Cursor;
fn png(image: DynamicImage) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png).unwrap();
    buffer.into_inner()
}
#[test]
fn normalization_preserves_alpha_and_bounds_dimensions() {
    let source =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(1920, 480, Rgba([70, 20, 80, 100])));
    let output = normalize_asset(&png(source)).unwrap();
    let decoded = decode_asset(&output).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (960, 240));
    assert_eq!(decoded.get_pixel(10, 10).0, [70, 20, 80, 100]);
}
#[test]
fn malformed_large_and_overdimension_images_fail() {
    assert!(decode_asset(&[]).is_err());
    assert!(decode_asset(b"not an image").is_err());
    assert!(decode_asset(&vec![0; MAX_ASSET_BYTES + 1]).is_err());
    let wide = png(DynamicImage::new_rgba8(MAX_ASSET_DIMENSION + 1, 1));
    assert!(decode_asset(&wide).is_err());
    assert!(decode_asset(&wide[..wide.len() / 2]).is_err());
}

#[test]
fn incompressible_rgba_upload_fits_wire_budget_after_normalization() {
    let mut seed = 7u32;
    let source = RgbaImage::from_fn(960, 960, |_, _| {
        let mut pixel = [0; 4];
        for byte in &mut pixel {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
        Rgba(pixel)
    });
    let input = png(DynamicImage::ImageRgba8(source));
    assert!(input.len() > 1024 * 1024);
    let output = normalize_asset(&input).unwrap();
    assert!(output.len() <= 1024 * 1024);
    let image = decode_asset(&output).unwrap();
    assert_eq!((image.width(), image.height()), (480, 480));
}
