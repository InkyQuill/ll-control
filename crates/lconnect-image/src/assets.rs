//! Bounded image ingestion for persistent theme assets.
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::{BufRead, Cursor, Seek};

/// Maximum encoded upload size (16 MiB).
pub const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
/// Maximum source width or height; decoded allocations are limited separately.
pub const MAX_ASSET_DIMENSION: u32 = 8192;

/// Decode an uploaded image without accepting unbounded dimensions or allocations.
/// Animated inputs import their first frame; video playback is a separate pipeline.
pub fn decode_asset(bytes: &[u8]) -> crate::Result<DynamicImage> {
    if bytes.is_empty() || bytes.len() > MAX_ASSET_BYTES {
        return Err(crate::Error::AssetSize);
    }
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ASSET_DIMENSION);
    limits.max_image_height = Some(MAX_ASSET_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let image = decode_oriented(reader, limits)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(crate::Error::AssetSize);
    }
    Ok(image)
}

/// Apply embedded orientation before fitting or metadata-stripping normalization.
pub(crate) fn decode_oriented<R: BufRead + Seek>(
    mut reader: ImageReader<R>,
    mut limits: Limits,
) -> crate::Result<DynamicImage> {
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    // Keep ImageReader::decode's output-buffer budget as well as decoder limits.
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// Normalize imports to PNG, preserving aspect ratio and alpha. 960px retains
/// oversampling for a 480px LCD while bounding persistent assets and render memory.
pub fn normalize_asset(bytes: &[u8]) -> crate::Result<Vec<u8>> {
    let source = DynamicImage::ImageRgba8(decode_asset(bytes)?.into_rgba8());
    let image = if source.width() > 960 || source.height() > 960 {
        source.resize(960, 960, image::imageops::FilterType::Lanczos3)
    } else {
        source
    };
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png)?;
    let encoded = output.into_inner();
    if encoded.len() <= 1024 * 1024 {
        return Ok(encoded);
    }
    // Noisy RGBA images may exceed the byte-array IPC budget even at 960px.
    // At native 480px, even incompressible RGBA PNG stays below 1 MiB.
    let native = image.resize(480, 480, image::imageops::FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    native.write_to(&mut output, ImageFormat::Png)?;
    let encoded = output.into_inner();
    if encoded.len() > 1024 * 1024 {
        return Err(crate::Error::AssetSize);
    }
    Ok(encoded)
}

/// Bounded gallery preview preserving aspect ratio and alpha.
pub fn thumbnail_asset(bytes: &[u8]) -> crate::Result<Vec<u8>> {
    let image = decode_asset(bytes)?.thumbnail(144, 144);
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}
