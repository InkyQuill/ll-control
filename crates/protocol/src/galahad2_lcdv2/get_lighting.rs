//! GetFanLightingInfo (cmd 132) + GetPumpLightingInfo (cmd 130).
//!
//! Both commands are bare-cmd-number requests with an `ACommandPacket`
//! response. The response's data slice is parsed via the lighting-info
//! escape-byte parser in `lighting.rs`.
//!
//! Source: `Products/Galahad2Vision/Galahad2VisionDevice.cs:73-86` (fan)
//! and `:122-135` (pump).

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::{
    parse_fan_lighting_info, parse_pump_lighting_info, FanLightingInfo, PumpLightingInfo,
};
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID request for `GetFanLightingInfo` (cmd 132).
///
/// Source: `Galahad2VisionDevice.cs:78` — `writeACommand(132)` with no
/// payload bytes.
#[must_use = "the request bytes must be written to hid_device.write(&buf)"]
pub fn build_get_fan_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetFanLightingInfo as u8, &[]).as_bytes()
}

/// Build the 64-byte HID request for `GetPumpLightingInfo` (cmd 130).
///
/// Source: `Galahad2VisionDevice.cs:127` — `writeACommand(130)` with no
/// payload bytes.
#[must_use = "the request bytes must be written to hid_device.write(&buf)"]
pub fn build_get_pump_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetPumpLightingInfo as u8, &[]).as_bytes()
}

/// Parse a `GetFanLightingInfo` response (must echo cmd 132).
///
/// Source: `Galahad2VisionDevice.cs:80-85` — reads an `ACommandPacket`,
/// rejects mismatched echo, then hands the data slice to
/// `FanLightingInfo.TryParse` (mirrored here by [`parse_fan_lighting_info`]).
///
/// # Errors
///
/// - [`crate::Error::BufferTooShort`] if the buffer is shorter than the
///   `ACommandPacket` header.
/// - [`crate::Error::UnexpectedCommand`] if the response's command byte
///   is not 132.
/// - [`crate::Error::InvalidLength`] if the framed `Length` field exceeds
///   the maximum payload size.
/// - Any error returned by [`parse_fan_lighting_info`] (e.g. missing
///   terminator or short trailer).
pub fn parse_fan_lighting_info_response(buf: &[u8]) -> crate::Result<FanLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetFanLightingInfo as u8)?;
    parse_fan_lighting_info(parsed.data())
}

/// Parse a `GetPumpLightingInfo` response (must echo cmd 130).
///
/// Source: `Galahad2VisionDevice.cs:129-134` — reads an `ACommandPacket`,
/// rejects mismatched echo, then hands the data slice to
/// `PumpLightingInfo.TryParse` (mirrored here by [`parse_pump_lighting_info`]).
///
/// # Errors
///
/// - [`crate::Error::BufferTooShort`] if the buffer is shorter than the
///   `ACommandPacket` header.
/// - [`crate::Error::UnexpectedCommand`] if the response's command byte
///   is not 130.
/// - [`crate::Error::InvalidLength`] if the framed `Length` field exceeds
///   the maximum payload size.
/// - Any error returned by [`parse_pump_lighting_info`] (e.g. missing
///   terminator or short trailer).
pub fn parse_pump_lighting_info_response(buf: &[u8]) -> crate::Result<PumpLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetPumpLightingInfo as u8)?;
    parse_pump_lighting_info(parsed.data())
}
