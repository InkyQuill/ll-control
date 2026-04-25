//! GetFanLightingInfo (cmd 132) + GetPumpLightingInfo (cmd 130).
//!
//! Both commands are bare-cmd-number requests with an `ACommandPacket`
//! response. The response's data slice is parsed via the lighting-info
//! escape-byte parser in `lighting.rs`.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::{
    parse_fan_lighting_info, parse_pump_lighting_info, FanLightingInfo, PumpLightingInfo,
};
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID request for `GetFanLightingInfo` (cmd 132).
pub fn build_get_fan_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetFanLightingInfo as u8, &[]).as_bytes()
}

/// Build the 64-byte HID request for `GetPumpLightingInfo` (cmd 130).
pub fn build_get_pump_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetPumpLightingInfo as u8, &[]).as_bytes()
}

/// Parse a `GetFanLightingInfo` response (must echo cmd 132).
pub fn parse_fan_lighting_info_response(buf: &[u8]) -> crate::Result<FanLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetFanLightingInfo as u8)?;
    parse_fan_lighting_info(parsed.data())
}

/// Parse a `GetPumpLightingInfo` response (must echo cmd 130).
pub fn parse_pump_lighting_info_response(buf: &[u8]) -> crate::Result<PumpLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetPumpLightingInfo as u8)?;
    parse_pump_lighting_info(parsed.data())
}
