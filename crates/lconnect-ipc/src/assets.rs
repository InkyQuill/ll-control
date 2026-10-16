//! Conservative orphan cleanup: retained keys are never removed.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetCleanupReport {
    pub candidates: Vec<String>,
    pub deleted: Vec<String>,
    pub retained_referenced: usize,
    pub retained_session: usize,
    pub retained_draft: usize,
    pub skipped: usize,
    pub failures: Vec<AssetCleanupFailure>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCleanupFailure {
    pub key: String,
    pub error: String,
}

/// A stable image key with optional user-facing metadata (legacy images have none).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeAssetInfo {
    #[serde(default = "all_image_contexts")]
    pub contexts: Vec<ImageGalleryContext>,
    /// Embedded immutable image; old servers/records omit this field.
    #[serde(default)]
    pub is_default: bool,
    pub key: String,
    pub name: Option<String>,
    pub revision: u64,
}

/// Selection purpose; library storage may be shared across these contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageGalleryContext {
    Image,
    Background,
    Watermark,
}

pub fn all_image_contexts() -> Vec<ImageGalleryContext> {
    vec![
        ImageGalleryContext::Image,
        ImageGalleryContext::Background,
        ImageGalleryContext::Watermark,
    ]
}
