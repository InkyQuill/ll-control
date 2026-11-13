//! llcontrol-image — prepare arbitrary images for the Galahad II LCDV2 LCD.
//!
//! The LCD is 480×480 px (`LCD_WIDTH × LCD_HEIGHT`) at a target 24 fps
//! (`LCD_FPS`). This crate handles the resize/crop and JPEG re-encode
//! step before the bytes are wrapped in a `BCommandPacket` and sent
//! via cmd 14 (SendJpeg) or cmd 15 (UploadJpeg).
//!
//! Source for the dimensions: `LConnectCore/Products/Galahad2Vision/Constants.cs:61-65`.

#![forbid(unsafe_code)]

pub mod assets;
pub mod defaults;
pub mod editor;
pub mod editor_filters;
mod editor_spatial;
pub mod effects;
pub mod fireworks;
pub mod fonts;
pub mod meteor;
pub mod rotation;
pub mod snow3d;
pub mod theme;
pub use rotation::FrameRotation;

mod text_layout;

use image::{imageops, DynamicImage, ImageBuffer, Rgb, RgbImage};

/// LCD width on the Galahad II LCDV2 pump cap. Source: `Constants.cs:61`.
pub const LCD_WIDTH: u32 = 480;

/// LCD height on the Galahad II LCDV2 pump cap. Source: `Constants.cs:63`.
pub const LCD_HEIGHT: u32 = 480;

/// Target frame rate on the LCDV2 LCD. Source: `Constants.cs:65`.
pub const LCD_FPS: u32 = 24;

/// How to fit a non-480x480 source image into the target frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FitMode {
    /// Scale uniformly to cover the target, then centre-crop the excess.
    /// Fills the square LCDV2 canvas; source edges may be cropped.
    Crop,
    /// Scale uniformly to fit inside the target, fill remainder with `bg`.
    Letterbox {
        /// Background colour as `[R, G, B]` used to fill the letterbox bars.
        bg: [u8; 3],
    },
    /// Stretch (may distort aspect ratio). Use only for explicit-aspect inputs.
    Stretch,
}

impl Default for FitMode {
    /// Fill the LCDV2 canvas by default; Letterbox preserves the entire source.
    fn default() -> Self {
        FitMode::Crop
    }
}

/// Output of [`prepare_jpeg_from_path`] / [`prepare_jpeg_from_bytes`].
#[derive(Debug)]
pub struct PreparedJpeg {
    /// JPEG-encoded bytes ready to send via cmd 14 / cmd 15.
    pub bytes: Vec<u8>,
    /// Width of the encoded image (always `LCD_WIDTH`).
    pub width: u32,
    /// Height of the encoded image (always `LCD_HEIGHT`).
    pub height: u32,
}

/// JPEG quality in 1..=100. The C# UI doesn't pin a value; 90 is a sane
/// default that keeps file size reasonable while preserving fine detail.
#[derive(Debug, Clone, Copy)]
pub struct Quality(u8);

impl Quality {
    /// Construct a `Quality` from a 1..=100 value.
    pub fn new(q: u8) -> crate::Result<Self> {
        if !(1..=100).contains(&q) {
            return Err(Error::InvalidQuality(q));
        }
        Ok(Self(q))
    }

    /// The underlying quality value in 1..=100.
    pub fn value(self) -> u8 {
        self.0
    }
}

impl Default for Quality {
    fn default() -> Self {
        Self(90)
    }
}

/// Crate-local error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid frame rotation {0}; expected0/90/180/270")]
    InvalidRotation(u16),
    /// Empty or oversized theme image input.
    #[error("image asset exceeds size limits or is empty")]
    AssetSize,
    /// Invalid image editor selection or operation budget.
    #[error("invalid image edit: {0}")]
    InvalidEdit(&'static str),
    /// JPEG quality outside the valid 1..=100 range.
    #[error("JPEG quality {0} out of range (must be 1..=100)")]
    InvalidQuality(u8),
    /// Underlying I/O error (file open / read).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Underlying `image` crate decode/encode error.
    #[error("image decode/encode error: {0}")]
    Image(#[from] image::ImageError),
}

/// Crate-local result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Read an image file from `path`, fit it to 480×480 per `mode`, and
/// re-encode as JPEG at `quality`. Errors on unreadable files or
/// unsupported formats; the `image` crate handles PNG/JPEG/WebP/BMP/GIF.
pub fn prepare_jpeg_from_path<P: AsRef<std::path::Path>>(
    path: P,
    mode: FitMode,
    quality: Quality,
) -> Result<PreparedJpeg> {
    let img = assets::decode_oriented(image::ImageReader::open(path)?, image::Limits::default())?;
    prepare_jpeg_from_image(img, mode, quality)
}

/// Same as [`prepare_jpeg_from_path`] but takes a byte buffer directly.
pub fn prepare_jpeg_from_bytes(
    bytes: &[u8],
    mode: FitMode,
    quality: Quality,
) -> Result<PreparedJpeg> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let img = assets::decode_oriented(reader, image::Limits::default())?;
    prepare_jpeg_from_image(img, mode, quality)
}

/// Internal entry point: takes an already-decoded image and runs the
/// fit + JPEG-encode pipeline. Useful for callers who already have a
/// `DynamicImage` in memory (e.g., a frame from an animation source).
pub fn prepare_jpeg_from_image(
    img: DynamicImage,
    mode: FitMode,
    quality: Quality,
) -> Result<PreparedJpeg> {
    let fitted = fit(img, mode);
    let bytes = encode_jpeg(&fitted, quality)?;
    Ok(PreparedJpeg {
        bytes,
        width: LCD_WIDTH,
        height: LCD_HEIGHT,
    })
}

/// Resize / crop / letterbox `img` into a 480×480 `DynamicImage`.
///
/// Fast path: if the input already matches `LCD_WIDTH × LCD_HEIGHT`, no
/// resize is performed; only an RGB conversion (which is cheap if the
/// input is already RGB8).
fn fit(img: DynamicImage, mode: FitMode) -> DynamicImage {
    let (w, h) = (img.width(), img.height());

    if w == LCD_WIDTH && h == LCD_HEIGHT {
        // Already the right size; just normalise to RGB8 so the JPEG
        // encoder doesn't have to deal with alpha.
        return DynamicImage::ImageRgb8(img.to_rgb8());
    }

    match mode {
        FitMode::Crop => {
            // Scale uniformly so the image *covers* 480×480 (the smaller
            // input dimension maps to 480; the larger one overflows and
            // gets cropped).
            let scale = LCD_WIDTH as f32 / w.min(h) as f32;
            let new_w = ((w as f32) * scale).round() as u32;
            let new_h = ((h as f32) * scale).round() as u32;
            let resized =
                imageops::resize(&img.to_rgb8(), new_w, new_h, imageops::FilterType::Lanczos3);

            // Centre-crop to 480×480.
            let x = (new_w.saturating_sub(LCD_WIDTH)) / 2;
            let y = (new_h.saturating_sub(LCD_HEIGHT)) / 2;
            let cropped = imageops::crop_imm(&resized, x, y, LCD_WIDTH, LCD_HEIGHT).to_image();
            DynamicImage::ImageRgb8(cropped)
        }
        FitMode::Letterbox { bg } => {
            // Scale uniformly so the image *fits inside* 480×480.
            let scale = LCD_WIDTH as f32 / w.max(h) as f32;
            let new_w = ((w as f32) * scale).round() as u32;
            let new_h = ((h as f32) * scale).round() as u32;
            let resized =
                imageops::resize(&img.to_rgb8(), new_w, new_h, imageops::FilterType::Lanczos3);

            let mut canvas: RgbImage = ImageBuffer::from_pixel(LCD_WIDTH, LCD_HEIGHT, Rgb(bg));
            let x = ((LCD_WIDTH.saturating_sub(new_w)) / 2) as i64;
            let y = ((LCD_HEIGHT.saturating_sub(new_h)) / 2) as i64;
            // `overlay` clamps if the layer extends beyond the canvas;
            // with `new_w/new_h <= 480` that won't happen.
            imageops::overlay(&mut canvas, &resized, x, y);
            DynamicImage::ImageRgb8(canvas)
        }
        FitMode::Stretch => {
            let resized = imageops::resize(
                &img.to_rgb8(),
                LCD_WIDTH,
                LCD_HEIGHT,
                imageops::FilterType::Lanczos3,
            );
            DynamicImage::ImageRgb8(resized)
        }
    }
}

/// Encode `img` as JPEG at the given quality.
fn encode_jpeg(img: &DynamicImage, quality: Quality) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let rgb = img.to_rgb8();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.value());
    enc.encode_image(&rgb)?;
    Ok(buf)
}

pub mod editor_surfaces;
