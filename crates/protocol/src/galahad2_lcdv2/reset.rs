//! ResetDevice (Command 142) — soft-reset the AIO.
//!
//! Source: `Galahad2VisionDevice.cs:233-248`. The C# returns `bool`:
//! `true` if **both** the response `Length == 1` **and** `Data[0] == 1`.
//! We model the same predicate but as `Result<()>`, with explicit error
//! variants distinguishing length-mismatch from data-not-valid.
//!
//! **Operationally risky:** the device drops off USB for 1–3 seconds
//! during a reset, which can stop the pump and cause coolant temperature
//! to spike. Use only when the user explicitly opts in.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID packet to soft-reset the device.
pub fn build_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::ResetDevice as u8, &[]).as_bytes()
}

/// Parse a `ResetDevice` response. Success requires `Length == 1` and
/// `Data[0] == 0x01`; everything else is an error.
pub fn parse_response(buf: &[u8]) -> crate::Result<()> {
    let parsed = ACommandPacket::parse_response(
        buf,
        Command::ResetDevice as u8,
    )?;
    if parsed.length() != 1 {
        return Err(crate::Error::InvalidLength {
            length: parsed.length(),
            max: 1,
        });
    }
    if parsed.data()[0] != 0x01 {
        return Err(crate::Error::DataNotValid);
    }
    Ok(())
}
