//! Global opt-in FPS source intent. Does not launch/inject or enable game logging.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FpsSourceSettings {
    pub version: u32,
    pub revision: u64,
    pub enabled: bool,
    pub log_path: Option<String>,
}
impl Default for FpsSourceSettings {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            enabled: false,
            log_path: None,
        }
    }
}
impl FpsSourceSettings {
    pub fn validate(&self) -> crate::Result<()> {
        if self.version != 1 || self.revision > 9_007_199_254_740_990 {
            return Err(crate::IpcError::InvalidArgument(
                "unsupported FPS settings version or revision".into(),
            ));
        }
        if self.enabled && self.log_path.is_none() {
            return Err(crate::IpcError::InvalidArgument(
                "enabled FPS source requires a log path".into(),
            ));
        }
        if self.log_path.as_ref().is_some_and(|p| {
            p.is_empty()
                || p.len() > 384
                || !std::path::Path::new(p).is_absolute()
                || p.chars().any(char::is_control)
        }) {
            return Err(crate::IpcError::InvalidArgument(
                "FPS log path must be absolute UTF-8, at most384bytes, without control characters"
                    .into(),
            ));
        }
        Ok(())
    }
}
