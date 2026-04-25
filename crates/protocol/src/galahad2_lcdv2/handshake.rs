//! GetHandshakeInfo (Command 129) — live sensor read.
//!
//! Sources:
//! - Request: `Galahad2VisionDevice.cs:250-253` (`SendGetHandshakeInfo`).
//! - Response framing: `ACommandPacket.cs`.
//! - Response data layout: `HandshakeInfo.cs:58-76`.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID packet to request a handshake from the device.
///
/// Returns the raw bytes ready for `hid_device.write(&buf)`.
pub fn build_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetHandshakeInfo as u8, &[])
        .as_bytes()
}

/// Decoded GetHandshakeInfo response.
///
/// Mirrors `LConnectCore.Products.Galahad2Vision.HandshakeInfo` field-for-
/// field, with the small simplification that we drop the C# `IsValid`
/// boolean — failure is reported via `Result` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeInfo {
    /// Fan tach reading in RPM.
    pub fan_rpm: u16,
    /// Pump tach reading in RPM.
    pub pump_rpm: u16,
    /// Whether the device considers the temperature reading reliable
    /// (e.g., probe attached).
    pub temp_valid: bool,
    /// Coolant temperature, integer part (°C).
    pub temp_integer: u8,
    /// Coolant temperature, fractional part — *raw* byte. The C# code
    /// applies `% 10` before display; see [`Self::temperature_c`].
    pub temp_fraction: u8,
}

impl HandshakeInfo {
    /// Parse a 64-byte HID response buffer.
    ///
    /// Validates the `ACommandPacket` header (ReportID, command echo,
    /// Length) before decoding. Returns
    /// [`crate::Error::BufferTooShort`] if `Length < 7`.
    pub fn from_response(buf: &[u8]) -> crate::Result<Self> {
        let parsed = ACommandPacket::parse_response(
            buf,
            Command::GetHandshakeInfo as u8,
        )?;
        let data = parsed.data();
        if data.len() < 7 {
            return Err(crate::Error::BufferTooShort {
                need: 7,
                got: data.len(),
            });
        }
        Ok(HandshakeInfo {
            fan_rpm:       u16::from_be_bytes([data[0], data[1]]),
            pump_rpm:      u16::from_be_bytes([data[2], data[3]]),
            temp_valid:    data[4] != 0,
            temp_integer:  data[5],
            temp_fraction: data[6],
        })
    }

    /// Convert to a single floating-point °C value (only meaningful when
    /// `temp_valid == true`). Mirrors the C# `Temperature` getter:
    /// `TemperatureInteger + (TemperatureFraction % 10) / 10`.
    pub fn temperature_c(&self) -> f32 {
        self.temp_integer as f32 + ((self.temp_fraction % 10) as f32) / 10.0
    }
}
