//! SetFanLighting (cmd 133) + SetPumpLighting (cmd 131).
//!
//! Both commands wrap the setting struct's `to_bytes()` payload in an
//! `ACommandPacket`. No response.
//!
//! Source: `Galahad2VisionDevice.cs:269-272` (fan) and `:310-313` (pump).

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting};
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID request for `SetFanLighting` (cmd 133, 20-byte payload).
///
/// Source: `Galahad2VisionDevice.cs:269` — wraps a `FanLightingSetting.ToBytes()`
/// payload in an `ACommandPacket`. No response.
#[must_use = "the request bytes must be written to hid_device.write(&buf)"]
pub fn build_set_fan_lighting_request(setting: &FanLightingSetting) -> [u8; PACKET_LEN] {
    let payload = setting.to_bytes();
    *ACommandPacket::new_request(Command::SetFanLighting as u8, &payload).as_bytes()
}

/// Build the 64-byte HID request for `SetPumpLighting` (cmd 131, 19-byte payload).
///
/// Source: `Galahad2VisionDevice.cs:310` — wraps a `PumpLightingSetting.ToBytes()`
/// payload in an `ACommandPacket`. No response.
#[must_use = "the request bytes must be written to hid_device.write(&buf)"]
pub fn build_set_pump_lighting_request(setting: &PumpLightingSetting) -> [u8; PACKET_LEN] {
    let payload = setting.to_bytes();
    *ACommandPacket::new_request(Command::SetPumpLighting as u8, &payload).as_bytes()
}
