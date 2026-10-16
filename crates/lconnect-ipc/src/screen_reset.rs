//! Durable software Screen reset, distinct from firmware/factory reset.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenResetReceipt {
    pub operation_id: String,
    pub profile_revision: u64,
    #[serde(default)]
    pub collection_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenResetResult {
    pub receipt: ScreenResetReceipt,
    pub repeated: bool,
    pub lcd: crate::LcdState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenResetReadback {
    pub receipt: Option<ScreenResetReceipt>,
    pub persistence: crate::SettingsSaveState,
    /// Retry is available only on this attachment before a newer display intent.
    pub reapply_available: bool,
    pub lcd: crate::LcdState,
}
