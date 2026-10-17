//! Authoritative per-layer rasters for local editor transforms.
//! Two opaque mattes recover alpha from the existing RGB renderer, preserving its
//! font shaping and shadow implementation instead of substituting browser metrics.
use crate::theme::{render_theme_measured, ClockTime, RenderError, ThemeAssets, ThemeFonts};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use lconnect_ipc::{LcdTheme, SensorReading, ThemeLayer};

pub fn layer_surface(
    source: &LcdTheme,
    layer: &ThemeLayer,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    fonts: &ThemeFonts,
    elapsed_seconds: u64,
) -> Result<RgbaImage, RenderError> {
    let mut theme = source.clone();
    theme.version = 1;
    theme.background_image = None;
    theme.background_video = None;
    theme.background_fill = None;
    theme.page_cache.clear();
    theme.basic_editor = None;
    theme.layers = vec![layer.clone()];
    let render = |color| {
        render_theme_measured(
            &theme,
            readings,
            clock,
            assets,
            fonts,
            elapsed_seconds,
            RgbImage::from_pixel(480, 480, Rgb([color; 3])),
        )
        .map(|frame| frame.image)
    };
    let black = render(0)?;
    let white = render(255)?;
    let mut rgba = RgbaImage::new(480, 480);
    for ((black, white), out) in black.pixels().zip(white.pixels()).zip(rgba.pixels_mut()) {
        // RGB rounding can differ by one between channels; average transmission.
        let transmission = (0..3)
            .map(|c| u16::from(white[c].saturating_sub(black[c])))
            .sum::<u16>();
        let alpha = 255 - ((transmission + 1) / 3);
        let channel = |c: usize| {
            (u16::from(black[c]) * 255 + alpha / 2)
                .checked_div(alpha)
                .unwrap_or(0)
                .min(255) as u8
        };
        *out = Rgba([channel(0), channel(1), channel(2), alpha as u8]);
    }
    Ok(rgba)
}

pub fn encode_png(pixels: RgbaImage) -> Result<Vec<u8>, image::ImageError> {
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels).write_to(&mut output, image::ImageFormat::Png)?;
    Ok(output.into_inner())
}
