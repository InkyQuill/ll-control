//! Saved firmware recovery inspection only. No action authorization or live HID.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareRecoveryState {
    NoSavedOperation,
    JournalOnly,
    Guarded,
    GuardOnly,
    Inconsistent,
    Unreadable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareSavedStage {
    Armed,
    EffectsPossible,
    NotStarted,
    Unknown,
    AwaitingReconnect,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareRecoveryGuard {
    pub settings_key: String,
    pub operation_id: String,
    pub image_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareRecoveryProgress {
    pub total_chunks: u32,
    pub chunk_writes: u32,
    pub chunk_echoes: u32,
    pub pump_hold_written: bool,
    pub fan_hold_written: bool,
    pub activation_written: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareSavedOperation {
    pub operation_id: String,
    pub stage: FirmwareSavedStage,
    pub image_sha256: String,
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub previous_hid_path: String,
    pub settings_key: String,
    /// Last durable progress may lag actual writes after a crash.
    pub progress: Option<FirmwareRecoveryProgress>,
    pub diagnostic: Option<String>,
}
/// Saved artifacts may coexist with a live job. This is not a live status or a
/// proof that actors stopped, no writes happened, or an update succeeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareRecoveryStatus {
    pub state: FirmwareRecoveryState,
    pub operation: Option<FirmwareSavedOperation>,
    pub guards: Vec<FirmwareRecoveryGuard>,
    pub diagnostic: Option<String>,
}
