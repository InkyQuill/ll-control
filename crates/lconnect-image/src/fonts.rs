//! Validation for user-owned font assets. No filesystem paths or fontconfig.
use ab_glyph::{Font, FontRef};

/// Full CJK fonts must fit without subsetting away the user's character coverage.
pub use lconnect_ipc::MAX_FONT_BYTES;

#[derive(Debug, thiserror::Error)]
pub enum FontAssetError {
    #[error("font must be non-empty and at most 16 MiB")]
    Size,
    #[error("expected a single TrueType/OpenType font; collections and web fonts are unsupported")]
    Container,
    #[error("font tables or metrics are invalid")]
    Invalid,
}

/// Parse a single-face TTF/OTF using the same parser as the renderer.
/// Bytes are preserved: importing must not strip copyright, license or glyphs.
/// Variable fonts use their default instance; axis editing is a separate feature.
pub fn validate_font(bytes: &[u8]) -> Result<FontRef<'_>, FontAssetError> {
    if bytes.is_empty() || bytes.len() > MAX_FONT_BYTES {
        return Err(FontAssetError::Size);
    }
    if !matches!(bytes.get(..4), Some(b"\0\x01\0\0" | b"OTTO" | b"true")) {
        return Err(FontAssetError::Container);
    }
    let font = FontRef::try_from_slice(bytes).map_err(|_| FontAssetError::Invalid)?;
    if font.glyph_count() == 0
        || !font.height_unscaled().is_finite()
        || font.height_unscaled() <= 0.0
        || !font.ascent_unscaled().is_finite()
        || !font.descent_unscaled().is_finite()
        || !font.line_gap_unscaled().is_finite()
    {
        return Err(FontAssetError::Invalid);
    }
    Ok(font)
}

/// Validate and own a font once, before starting a render loop.
pub fn prepare_font(bytes: Vec<u8>) -> Result<ab_glyph::FontArc, FontAssetError> {
    validate_font(&bytes)?;
    ab_glyph::FontVec::try_from_vec(bytes)
        .map(ab_glyph::FontArc::new)
        .map_err(|_| FontAssetError::Invalid)
}
