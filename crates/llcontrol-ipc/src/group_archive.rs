//! Full-group backup transport, separate from named-slot LCPROF01 archives.
use crate::{CoolingConfig, DeviceId, DynamicRgbConfig, LcdConfig, ProfileGroup, PwmSource};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FullProfileGroup {
    Screen,
    FanAndLighting,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupArchiveRequest {
    Snapshot,
    BeginExport {
        id: String,
        expected: String,
    },
    Status {
        id: String,
    },
    Read {
        id: String,
        offset: u64,
        length: usize,
    },
    Cancel {
        id: String,
    },
    Release {
        id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupArchiveSlot {
    pub group: ProfileGroup,
    pub slot: u8,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupManualPump {
    pub percent: u8,
    pub source: PwmSource,
}
/// Public policies/names only; historical Windows metadata stays in prepared intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupCoolingModes {
    pub fan: Vec<crate::CoolingModeProfile>,
    pub pump: Vec<crate::CoolingModeProfile>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupArchiveSummary {
    pub version: u32,
    pub slots: Vec<GroupArchiveSlot>,
    pub lcd: Option<LcdConfig>,
    pub theme_name: Option<String>,
    pub media_key: Option<String>,
    pub fan_manual_percent: Option<u8>,
    pub pump_manual: Option<GroupManualPump>,
    pub cooling: Option<CoolingConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooling_modes: Option<GroupCoolingModes>,
    pub fan_led_mode: Option<String>,
    pub pump_led_mode: Option<String>,
    pub dynamic_rgb: Option<DynamicRgbConfig>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupArchiveSnapshot {
    pub dev: DeviceId,
    pub group: FullProfileGroup,
    pub expected: String,
    pub summary: GroupArchiveSummary,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupArchivePhase {
    Preparing,
    Ready,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupArchiveState {
    pub id: String,
    pub snapshot: GroupArchiveSnapshot,
    pub phase: GroupArchivePhase,
    pub size: Option<u64>,
    pub produced: u64,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupArchiveReply {
    Snapshot {
        snapshot: GroupArchiveSnapshot,
    },
    Transfer {
        state: GroupArchiveState,
        offset: Option<u64>,
        bytes: Vec<u8>,
        eof: bool,
    },
}

/// Read-only target-bound Windows control import proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsControlPreview {
    pub target: GroupArchiveSnapshot,
    pub proposed: GroupArchiveSummary,
    pub lighting: WindowsControlLightingReview,
}

/// Executable Linux values for review; never historical Windows JSON/Metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsControlLightingReview {
    pub fan_topology: crate::FanLightingTopology,
    pub fan_modes: Vec<crate::LightingConfig>,
    pub pump_modes: Vec<crate::LightingConfig>,
    pub dynamic_rgb_profiles: crate::DynamicRgbProfiles,
}
