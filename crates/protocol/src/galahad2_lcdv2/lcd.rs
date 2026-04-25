//! LCD lifecycle commands.
//!
//! - `IsLCDAvailable` (cmd 23) — read-only probe. Source:
//!   Galahad2VisionDevice.cs:175-186.
//! - `LCDControl`     (cmd 12) — set mode/brightness/rotation. Source:
//!   Galahad2VisionDevice.cs:188-193 + LCDControlSetting.cs.
//!
//! ## Latent C# bug (do NOT replicate)
//!
//! The C# service's `IsLCDAvailable` polling timer at
//! `Galahad2VisionController.cs:2410-2411` checks `nullable.HasValue`
//! against a stale local that is no longer in sync with the value
//! actually stored into `this.isAvailable`. The codec parse is correct;
//! the bug is at the call site. The Linux port's contract is: callers
//! branch on the `bool` returned by [`parse_is_lcd_available_response`]
//! directly — never reintroduce a separate "has value" indirection.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;

/// Build the 1024-byte HID request for `IsLCDAvailable` (cmd 23).
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_is_lcd_available_request() -> [u8; OUTPUT_LEN] {
    *BCommandPacket::new_request(Command::IsLCDAvailable as u8, 0, &[], 0).as_bytes()
}

/// Parse an `IsLCDAvailable` response. Returns `true` iff the device
/// reported a single `0x00` byte (the "available" sentinel). Anything
/// else — non-zero byte, missing payload, mismatched length — means
/// not available.
///
/// # Caveats
///
/// The C# service controller has a latent stale-local bug at
/// `Galahad2VisionController.cs:2410-2411`: it checks
/// `nullable.HasValue` against a value that is no longer in sync with
/// `this.isAvailable`. Callers MUST branch on the returned `bool`
/// directly; the Linux port deliberately surfaces only the `bool` to
/// make that anti-pattern unrepresentable.
///
/// # Errors
///
/// Returns:
/// - [`crate::Error::BufferTooShort`] if `buf` is shorter than the
///   B-packet header (11 bytes), OR if the parsed `Length` field
///   announces more payload bytes than `buf` contains.
/// - [`crate::Error::UnexpectedReportId`] if `buf[0]` is not `0x02`.
/// - [`crate::Error::UnexpectedCommand`] if `buf[1]` is not `23`.
/// - [`crate::Error::InvalidLength`] if the header's `Length` field
///   exceeds the available payload bytes in `buf`.
pub fn parse_is_lcd_available_response(buf: &[u8]) -> crate::Result<bool> {
    let parsed = BCommandPacket::from_response(buf, Command::IsLCDAvailable as u8)?;
    Ok(parsed.length() == 1 && parsed.data()[0] == 0x00)
}
