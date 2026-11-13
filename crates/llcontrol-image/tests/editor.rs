use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use llcontrol_image::editor::{prepare_geometry, SquareCrop, Transform, MAX_TRANSFORMS};
use std::io::Cursor;

fn png(image: RgbaImage) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

#[test]
fn off_center_crop_includes_source_edges_and_preserves_alpha() {
    let source = RgbaImage::from_fn(960, 480, |x, y| {
        if x < 480 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([(x - 480) as u8, y as u8, 70, 128])
        }
    });
    let encoded = png(source.clone());
    let output = prepare_geometry(
        &encoded,
        Some(SquareCrop {
            x: 480,
            y: 0,
            side: 480,
        }),
        &[],
    )
    .unwrap();
    let decoded = image::load_from_memory(&output).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (480, 480));
    for (x, y, pixel) in decoded.enumerate_pixels() {
        assert_eq!(pixel, source.get_pixel(x + 480, y));
    }
    assert_eq!(encoded, png(source));
}

#[test]
fn rotate_and_flip_follow_button_order() {
    // Unique corner colours make accidental transform reordering visible.
    let source = png(RgbaImage::from_fn(480, 480, |x, y| {
        Rgba([
            if x < 240 { 10 } else { 20 },
            if y < 240 { 30 } else { 40 },
            0,
            255,
        ])
    }));
    let corners = |actions: &[Transform]| {
        let output = prepare_geometry(&source, None, actions).unwrap();
        let frame = image::load_from_memory(&output).unwrap().into_rgba8();
        [(0, 0), (479, 0), (0, 479), (479, 479)].map(|(x, y)| *frame.get_pixel(x, y))
    };
    let a = Rgba([10, 30, 0, 255]);
    let b = Rgba([20, 30, 0, 255]);
    let c = Rgba([10, 40, 0, 255]);
    let d = Rgba([20, 40, 0, 255]);
    assert_eq!(
        corners(&[Transform::RotateRight, Transform::FlipHorizontal]),
        [a, c, b, d]
    );
    assert_eq!(
        corners(&[Transform::FlipHorizontal, Transform::RotateRight]),
        [d, b, c, a]
    );
    assert_eq!(corners(&[Transform::RotateLeft]), [b, d, a, c]);
    assert_eq!(corners(&[Transform::FlipVertical]), [c, d, a, b]);
    assert_eq!(corners(&[Transform::RotateRight; 4]), [a, b, c, d]);
}

#[test]
fn invalid_selections_are_rejected_without_clamping_or_overflow() {
    let source = png(RgbaImage::new(10, 20));
    for crop in [
        SquareCrop {
            x: 0,
            y: 0,
            side: 0,
        },
        SquareCrop {
            x: 10,
            y: 0,
            side: 1,
        },
        SquareCrop {
            x: 0,
            y: 20,
            side: 1,
        },
        SquareCrop {
            x: 9,
            y: 19,
            side: 2,
        },
        SquareCrop {
            x: u32::MAX,
            y: 0,
            side: 2,
        },
        SquareCrop {
            x: 1,
            y: 1,
            side: u32::MAX,
        },
    ] {
        assert!(matches!(
            prepare_geometry(&source, Some(crop), &[]),
            Err(llcontrol_image::Error::InvalidEdit(_))
        ));
    }
    assert!(matches!(
        prepare_geometry(&source, None, &[Transform::RotateLeft; MAX_TRANSFORMS + 1]),
        Err(llcontrol_image::Error::InvalidEdit(_))
    ));
    // A one-pixel selection at the bottom-right corner is valid.
    let output = prepare_geometry(
        &source,
        Some(SquareCrop {
            x: 9,
            y: 19,
            side: 1,
        }),
        &[],
    )
    .unwrap();
    assert_eq!(image::load_from_memory(&output).unwrap().width(), 480);
    assert!(prepare_geometry(b"not an image", None, &[]).is_err());
}

#[test]
fn edit_only_resizes_full_portrait_and_retains_transparency() {
    let source = png(RgbaImage::from_pixel(40, 80, Rgba([20, 80, 150, 64])));
    let output = prepare_geometry(&source, None, &[Transform::RotateLeft]).unwrap();
    let frame = image::load_from_memory(&output).unwrap().into_rgba8();
    assert_eq!(frame.dimensions(), (480, 480));
    assert!(frame.pixels().all(|p| *p == Rgba([20, 80, 150, 64])));
    assert!(output.len() <= 1024 * 1024);
}

#[test]
fn edit_only_keeps_both_ends_of_a_portrait_instead_of_center_cropping() {
    let source = png(RgbaImage::from_fn(40, 120, |_, y| {
        if y < 30 {
            Rgba([255, 0, 0, 255])
        } else if y >= 90 {
            Rgba([0, 0, 255, 255])
        } else {
            Rgba([0, 255, 0, 255])
        }
    }));
    let output = prepare_geometry(&source, None, &[]).unwrap();
    let frame = image::load_from_memory(&output).unwrap().into_rgba8();
    assert_eq!(*frame.get_pixel(240, 10), Rgba([255, 0, 0, 255]));
    assert_eq!(*frame.get_pixel(240, 240), Rgba([0, 255, 0, 255]));
    assert_eq!(*frame.get_pixel(240, 470), Rgba([0, 0, 255, 255]));
}
