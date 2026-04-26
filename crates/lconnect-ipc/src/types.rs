//! IPC types shared between daemon and clients.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable opaque device ID, e.g. "galahad2-lcdv2:0001".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl DeviceId {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Per-device descriptor returned by `list_devices` / `device_attached`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DeviceDescriptor {
    pub id: DeviceId,
    pub family: String,
    pub display_name: String,
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
    pub hid_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SensorSnapshot {
    pub coolant_c_x10: i16, // tenths of C; serialised as integer to keep wire compact
    pub fan_rpm: u16,
    pub pump_rpm: u16,
    pub temp_valid: bool,
    pub last_seen_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FirmwareInfo {
    pub version: String,
    pub raw_token: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pwm(pub u8);

impl Pwm {
    /// # Errors
    /// Returns Err if `percent > 100`.
    pub fn new(percent: u8) -> Result<Self, String> {
        if percent > 100 {
            return Err(format!("PWM out of range: {percent}"));
        }
        Ok(Self(percent))
    }
    pub fn percent(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PwmSource {
    Mcu,
    Application,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LightingKind {
    Fan,
    Pump,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum LedTarget {
    Fan,
    Screen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LightingConfig {
    pub mode: String, // serialised as the enum variant name
    pub color: Rgb,
    pub brightness_percent: u8,
    pub speed_percent: u8,
    pub direction: String,     // "forward" | "reverse"
    pub scope: Option<String>, // pump-only ("inner" | "outer" | "all")
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingInfo {
    pub max_speed: u8,
    pub max_brightness: u8,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LcdConfig {
    pub mode: String, // "LocalUI" | "Application" | ...
    pub brightness_percent: u8,
    pub rotation_degrees: u16, // 0 / 90 / 180 / 270
}

/// Cryptographically random token returned by `request_reset_token`.
/// Single-use; expires 30s after issuance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetToken(pub String);

/// Wire framing identifier for `raw_send`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum FramingKind {
    A,
    B,
    C,
}

/// Aggregated daemon counters returned by `daemon_stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DaemonStats {
    pub uptime_seconds: u64,
    pub devices_known: u32,
    pub hid_write_count: u64,
    pub hid_read_errors: u64,
    pub hid_decode_errors: u64,
    pub ipc_request_count: u64,
}

/// Brief structured error for `device_error` notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBrief {
    pub category: String,
    pub message: String,
}
