//! Image editor geometry for the GA2LCD 480×480 output.
//!
//! This is preparation only: it does not modify a gallery asset or write to HID.
//! Coordinates refer to the decoded, EXIF-oriented source, before transforms.
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use std::io::Cursor;

/// Maximum number of ordered geometry actions accepted in one preparation.
/// A local resource limit, not a Windows editor limit.
pub const MAX_TRANSFORMS: usize = 64;

/// A square selection in oriented source pixels, including the last row/column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquareCrop {
    /// Left edge of the selection.
    pub x: u32,
    /// Top edge of the selection.
    pub y: u32,
    /// Nonzero width and height of the selection.
    pub side: u32,
}

/// Geometry actions are applied in order; rotate and flip do not commute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    /// Rotate 90 degrees counter-clockwise.
    RotateLeft,
    /// Rotate 90 degrees clockwise.
    RotateRight,
    /// Mirror left and right.
    FlipHorizontal,
    /// Mirror top and bottom.
    FlipVertical,
}

/// Prepare a fresh 480×480 RGBA PNG from a bounded encoded source.
///
/// Cropping precedes geometry actions, matching the crop → edit workflow. With
/// no crop, the complete source is resized to the square target (edit-only path).
/// Source bytes are never changed. A caller can preview or publish the result
/// through the existing managed-asset import path after explicit confirmation.
/// CPU-bound: async callers must use their bounded blocking preparation worker.
///
/// Returns an error for invalid input, an empty/out-of-bounds crop, or more than
/// [`MAX_TRANSFORMS`] actions. No silent clamping of a stale selection occurs.
/// PNG alpha is retained. The output geometry matches GA2LCD; Lanczos sampling
/// is not claimed to reproduce WPF/Telerik rasterization pixel for pixel.
pub fn prepare_geometry(
    source: &[u8],
    crop: Option<SquareCrop>,
    transforms: &[Transform],
) -> crate::Result<Vec<u8>> {
    prepare_edited(source, crop, transforms, Default::default())
}

/// Prepare geometry with validated colour controls. Crop output is 480×480
/// before ImageEdit; the edit-only path filters the source before final resize.
pub fn prepare_edited(
    source: &[u8],
    crop: Option<SquareCrop>,
    transforms: &[Transform],
    filters: crate::editor_filters::ColorFilters,
) -> crate::Result<Vec<u8>> {
    filters.validate()?;
    if transforms.len() > MAX_TRANSFORMS {
        return Err(crate::Error::InvalidEdit("too many geometry actions"));
    }
    let decoded = crate::assets::decode_asset(source)?;
    let selected = if let Some(crop) = crop {
        // Subtraction after checking the origin avoids overflow on external input.
        if crop.side == 0
            || crop.x >= decoded.width()
            || crop.y >= decoded.height()
            || crop.side > decoded.width() - crop.x
            || crop.side > decoded.height() - crop.y
        {
            return Err(crate::Error::InvalidEdit(
                "crop is outside the source image",
            ));
        }
        decoded.crop_imm(crop.x, crop.y, crop.side, crop.side)
    } else {
        decoded
    };
    let selected = DynamicImage::ImageRgba8(selected.into_rgba8());
    let frame = if crop.is_some() || filters.is_neutral() {
        selected.resize_exact(crate::LCD_WIDTH, crate::LCD_HEIGHT, FilterType::Lanczos3)
    } else {
        selected
    };
    let input_size = (frame.width(), frame.height());
    let frame = apply_transforms(frame, transforms);
    let mut pixels = frame.into_rgba8();
    filters.apply_with_input_size(&mut pixels, input_size)?;
    let frame = DynamicImage::ImageRgba8(pixels).resize_exact(
        crate::LCD_WIDTH,
        crate::LCD_HEIGHT,
        FilterType::Lanczos3,
    );
    let mut output = Cursor::new(Vec::new());
    frame.write_to(&mut output, ImageFormat::Png)?;
    let bytes = output.into_inner();
    if bytes.len() > 1024 * 1024 {
        return Err(crate::Error::AssetSize);
    }
    Ok(bytes)
}

fn apply_transforms(mut frame: DynamicImage, transforms: &[Transform]) -> DynamicImage {
    // Ordered right-angle rotations/reflections have only eight outcomes.
    // Keep reflection-before-rotation form to avoid copying a full source for
    // each button press. Filters still receive the original input dimensions.
    let (mut rotations, mut reflected) = (0_u8, false);
    for transform in transforms {
        match transform {
            Transform::RotateRight => rotations = (rotations + 1) % 4,
            Transform::RotateLeft => rotations = (rotations + 3) % 4,
            Transform::FlipHorizontal => {
                rotations = (4 - rotations) % 4;
                reflected = !reflected;
            }
            Transform::FlipVertical => {
                rotations = (6 - rotations) % 4;
                reflected = !reflected;
            }
        }
    }
    if reflected {
        frame = frame.fliph();
    }
    match rotations {
        1 => frame.rotate90(),
        2 => frame.rotate180(),
        3 => frame.rotate270(),
        _ => frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn verify_sequence(source: &DynamicImage, actions: &[Transform]) {
        let mut expected = source.clone();
        for action in actions {
            expected = match action {
                Transform::RotateLeft => expected.rotate270(),
                Transform::RotateRight => expected.rotate90(),
                Transform::FlipHorizontal => expected.fliph(),
                Transform::FlipVertical => expected.flipv(),
            };
        }
        assert_eq!(
            apply_transforms(source.clone(), actions).into_rgba8(),
            expected.into_rgba8(),
            "ordered actions: {actions:?}"
        );
    }

    fn verify_short_sequences(source: &DynamicImage, actions: &mut Vec<Transform>) {
        verify_sequence(source, actions);
        if actions.len() == 6 {
            return;
        }
        for action in [
            Transform::RotateLeft,
            Transform::RotateRight,
            Transform::FlipHorizontal,
            Transform::FlipVertical,
        ] {
            actions.push(action);
            verify_short_sequences(source, actions);
            actions.pop();
        }
    }

    #[test]
    fn composed_geometry_preserves_every_pixel_and_action_order() {
        for (width, height) in [(5, 3), (3, 5), (1, 7), (7, 1)] {
            let source = DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
                Rgba([
                    x as u8 * 13,
                    y as u8 * 17,
                    (x + y) as u8 * 19,
                    (x * y) as u8 * 7,
                ])
            }));
            verify_short_sequences(&source, &mut Vec::new());
            let actions: Vec<_> = [
                Transform::RotateRight,
                Transform::FlipHorizontal,
                Transform::RotateLeft,
                Transform::FlipVertical,
            ]
            .into_iter()
            .cycle()
            .take(MAX_TRANSFORMS)
            .collect();
            verify_sequence(&source, &actions);
            verify_sequence(&source, &actions[..MAX_TRANSFORMS - 1]);
        }
    }
}
