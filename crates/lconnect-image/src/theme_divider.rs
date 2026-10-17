//! Bounded area coverage for editable axis-aligned LCD dividers.
use image::{GrayImage, Luma, RgbImage};
use lconnect_ipc::TextShadow;

pub(super) fn draw(
    canvas: &mut RgbImage,
    bounds: [f32; 4],
    color: [u8; 3],
    opacity: u8,
    shadow: Option<&TextShadow>,
) {
    if opacity == 0 {
        return;
    }
    // Theme validation bounds all dimensions before allocation (at most576x576).
    let [x, y, width, height] = bounds;
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let pad = shadow.map_or(0, |s| u32::from(s.blur) * 3);
    let mut mask = GrayImage::new(
        (x + width).ceil() as u32 - left as u32 + 2 * pad,
        (y + height).ceil() as u32 - top as u32 + 2 * pad,
    );
    for py in 0..mask.height() - 2 * pad {
        for px in 0..mask.width() - 2 * pad {
            let bx = left as f32 + px as f32;
            let by = top as f32 + py as f32;
            let coverage = ((bx + 1.0).min(x + width) - bx.max(x)).max(0.0)
                * ((by + 1.0).min(y + height) - by.max(y)).max(0.0);
            mask.put_pixel(px + pad, py + pad, Luma([(coverage * 255.0).round() as u8]));
        }
    }
    let alpha = f32::from(opacity) / 100.0;
    if let Some(s) = shadow.filter(|s| s.opacity > 0) {
        let blurred = (s.blur > 0).then(|| image::imageops::blur(&mask, f32::from(s.blur)));
        for (px, py, a) in blurred.as_ref().unwrap_or(&mask).enumerate_pixels() {
            super::blend(
                canvas,
                left + px as i32 - pad as i32 + i32::from(s.x),
                top + py as i32 - pad as i32 + i32::from(s.y),
                s.color,
                f32::from(a[0]) / 255.0 * alpha * f32::from(s.opacity) / 100.0,
            );
        }
    }
    for (px, py, a) in mask.enumerate_pixels() {
        super::blend(
            canvas,
            left + px as i32 - pad as i32,
            top + py as i32 - pad as i32,
            color,
            f32::from(a[0]) / 255.0 * alpha,
        );
    }
}
