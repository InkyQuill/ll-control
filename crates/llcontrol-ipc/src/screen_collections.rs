//! Persisted LCD gallery membership, separate from managed files and live themes.
use crate::{IpcError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const BUILTIN_MARK_KEY: &str = "builtin-orbit-mark-v1";
pub const BUILTIN_IMAGE_KEY: &str = "builtin-midnight-v1";
pub const BUILTIN_VIDEO_KEY: &str = "bcf5a102bf2a426286d34310cad4a001";
const MAX_COLLECTION_ENTRIES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "key",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ScreenBackgroundAsset {
    Image(String),
    Video(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenCollections {
    pub version: u32,
    pub revision: u64,
    pub images: Vec<String>,
    pub backgrounds: Vec<ScreenBackgroundAsset>,
    pub watermarks: Vec<String>,
}
impl ScreenCollections {
    /// Original Linux defaults with the same three collection purposes as LCDV2.
    pub fn defaults(revision: u64) -> Self {
        Self {
            version: 1,
            revision,
            images: vec![BUILTIN_MARK_KEY.into(), BUILTIN_IMAGE_KEY.into()],
            backgrounds: vec![
                ScreenBackgroundAsset::Image(BUILTIN_MARK_KEY.into()),
                ScreenBackgroundAsset::Video(BUILTIN_VIDEO_KEY.into()),
            ],
            watermarks: vec![BUILTIN_MARK_KEY.into()],
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.revision == 0 {
            return Err(invalid(
                "unsupported screen collections version or revision",
            ));
        }
        for keys in [&self.images, &self.watermarks] {
            validate_count(keys.len())?;
            if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
                return Err(invalid("duplicate image in screen collection"));
            }
            for key in keys {
                validate_image_key(key)?;
            }
        }
        validate_count(self.backgrounds.len())?;
        if self.backgrounds.iter().collect::<BTreeSet<_>>().len() != self.backgrounds.len() {
            return Err(invalid("duplicate background in screen collection"));
        }
        for asset in &self.backgrounds {
            match asset {
                ScreenBackgroundAsset::Image(key) => validate_image_key(key)?,
                ScreenBackgroundAsset::Video(key) => {
                    if key.len() != 32
                        || !key
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    {
                        return Err(invalid("invalid screen collection video key"));
                    }
                }
            }
        }
        let defaults = Self::defaults(self.revision);
        if !defaults.images.iter().all(|key| self.images.contains(key))
            || !defaults
                .backgrounds
                .iter()
                .all(|asset| self.backgrounds.contains(asset))
            || !defaults
                .watermarks
                .iter()
                .all(|key| self.watermarks.contains(key))
        {
            return Err(invalid("screen collections must retain protected defaults"));
        }
        Ok(())
    }

    pub fn image_keys(&self) -> BTreeSet<&str> {
        self.images
            .iter()
            .chain(&self.watermarks)
            .map(String::as_str)
            .chain(self.backgrounds.iter().filter_map(|asset| match asset {
                ScreenBackgroundAsset::Image(key) => Some(key.as_str()),
                _ => None,
            }))
            .collect()
    }

    pub fn video_keys(&self) -> BTreeSet<&str> {
        self.backgrounds
            .iter()
            .filter_map(|asset| match asset {
                ScreenBackgroundAsset::Video(key) => Some(key.as_str()),
                _ => None,
            })
            .collect()
    }
}
fn validate_image_key(key: &str) -> Result<()> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(invalid("invalid screen collection image key"));
    }
    Ok(())
}
fn validate_count(count: usize) -> Result<()> {
    if count > MAX_COLLECTION_ENTRIES {
        return Err(invalid("too many screen collection entries"));
    }
    Ok(())
}
fn invalid(message: &str) -> IpcError {
    IpcError::InvalidArgument(message.into())
}

/// Owner-consistent accepted membership and persistence status. `None` means
/// legacy shared catalog; `saved` alone is not sufficient while `pending` is true.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenCollectionsState {
    pub collections: Option<ScreenCollections>,
    pub persistence: crate::SettingsSaveState,
}
