//! Durable full-group acknowledgement and independent runtime observation.
use crate::{
    CoolingState, DeviceId, DynamicRgbState, FullProfileGroup, GroupArchiveSummary, LcdState,
    ThermalState,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullGroupRuntime {
    /// Opaque atomic actor intent revision; echo unchanged for explicit reapply.
    #[serde(default)]
    pub revision: Option<String>,
    pub lcd: Option<LcdState>,
    pub cooling: Option<CoolingState>,
    pub thermal: Option<ThermalState>,
    pub dynamic: Option<DynamicRgbState>,
    pub recovery_required: Option<bool>,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullGroupReadback {
    pub dev: DeviceId,
    pub group: FullProfileGroup,
    pub operation_id: String,
    /// Opaque durable receipt; clients must not derive revision numbers from it.
    pub receipt: String,
    pub selected_content_current: bool,
    pub summary: GroupArchiveSummary,
    pub runtime: FullGroupRuntime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullGroupCommit {
    pub readback: FullGroupReadback,
    pub repeated: bool,
}

/// Read-only versioned defaults and the exact target token to confirm.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlResetPreview {
    pub target: crate::GroupArchiveSnapshot,
    pub defaults: GroupArchiveSummary,
}
