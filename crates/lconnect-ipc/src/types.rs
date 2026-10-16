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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
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
    /// Unparsed build datetime from firmware PN1; absent when the device did not report it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_datetime: Option<String>,
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
#[serde(deny_unknown_fields)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LightingConfig {
    /// Pump-only explicit selection: true restores the last saved source for this
    /// mode. False/omitted applies the manual base and disables this mode's policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_mode: Option<bool>,
    /// Fan-only global topology; omitted updates preserve the owner's current value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fan_topology: Option<crate::FanLightingTopology>,
    pub mode: String, // serialised as the enum variant name
    pub color: Rgb,
    /// Remaining mode palette entries; omitted by legacy single-color clients.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_colors: Vec<Rgb>,
    pub brightness_percent: u8,
    pub speed_percent: u8,
    pub direction: String,     // "forward" | "reverse"
    pub scope: Option<String>, // pump-only ("inner" | "outer" | "all")
    pub disabled: bool,
}

impl LightingConfig {
    /// Construct a manual setting using the CLI percentage/direction defaults.
    pub fn new(mode: impl Into<String>, color: Rgb) -> Self {
        Self {
            dynamic_mode: None,
            fan_topology: None,
            mode: mode.into(),
            color,
            additional_colors: Vec::new(),
            brightness_percent: 100,
            speed_percent: 50,
            direction: "forward".into(),
            scope: None,
            disabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingInfo {
    /// Supports explicit restoration of per-mode pump dynamic profiles.
    #[serde(default)]
    pub dynamic_mode_supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fan_topology: Option<crate::FanLightingTopology>,
    #[serde(default)]
    pub catalog: Vec<crate::LightingModeControls>,
    #[serde(default)]
    pub profiles: Vec<LightingConfig>,
    /// Last committed manual mode base from the RGB owner, not physical readback.
    /// None for unset/direct-color output. Old daemons omit this field.
    #[serde(default)]
    pub base: Option<LightingConfig>,
    pub max_speed: u8,
    pub max_brightness: u8,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LcdConfig {
    pub mode: String, // "LocalUI" | "Application" | ...
    pub brightness_percent: u8,
    pub rotation_degrees: u16, // 0 / 90 / 180 / 270
}

impl LcdConfig {
    #[must_use]
    pub fn new(mode: impl Into<String>, brightness_percent: u8, rotation_degrees: u16) -> Self {
        Self {
            mode: mode.into(),
            brightness_percent,
            rotation_degrees,
        }
    }
}

/// Partial LCD configuration update. Omitted fields retain their current value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdConfigPatch {
    pub mode: Option<String>,
    pub brightness_percent: Option<u8>,
    pub rotation_degrees: Option<u16>,
}

/// Software source currently driving the LCD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LcdSourceKind {
    Idle,
    Jpeg,
    DiagnosticClip,
    Theme,
}

/// Observable LCD configuration and source state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdState {
    /// Latest dispatcher observation for all device settings, not LCD transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<SettingsSaveState>,
    pub requested: LcdConfig,
    pub applied: Option<LcdConfig>,
    pub source: LcdSourceKind,
    pub generation: u64,
    pub last_error: Option<String>,
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

/// In-memory status of this device's settings writer during the current session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSaveState {
    /// Changes have reached the dispatcher but have not been durably written.
    pub pending: bool,
    /// At least one successful file + parent-directory sync in this session.
    pub saved: bool,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod firmware_info_tests {
    use super::FirmwareInfo;
    #[test]
    fn legacy_firmware_response_does_not_invent_a_build_date() {
        let old = serde_json::json!({"version":"1.6","raw_token":"legacy token"});
        let info: FirmwareInfo = serde_json::from_value(old.clone()).unwrap();
        assert!(info.build_datetime.is_none());
        assert_eq!(serde_json::to_value(info).unwrap(), old);
    }
}
