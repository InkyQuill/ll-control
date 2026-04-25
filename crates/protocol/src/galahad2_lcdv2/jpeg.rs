//! `SendJpeg` (cmd 14) and `UploadJpeg` (cmd 15) — JPEG byte stream
//! upload to the LCDV2's pump-cap LCD.
//!
//! The 480x480 LCD accepts JPEG-encoded frames; for smaller-than-LCD
//! image content, see the `lconnect-image` workspace crate for a
//! converter that produces 480x480 JPEG bytes ready to feed in here.
//!
//! ## SendJpeg vs UploadJpeg
//!
//! Both commands appear in the C# `Commands` enum but neither has a
//! located call site in any of the L-Connect.Core, L-Connect3, or
//! L-Connect-Service codebases. The wire shapes here are presumed from
//! the BCommandPacket convention used by `SetLEDColors` (cmd 20) — the
//! only B-class write whose call site we can read directly. If a future
//! reverse-engineering session uncovers the actual UI dispatch path,
//! revise the doc-comments accordingly.
//!
//! Source: `Galahad2Vision/Commands.cs:11,12` (`SendJpeg = 14`,
//! `UploadJpeg = 15`).

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;

/// Build the BCommandPacket sequence for `SendJpeg` (cmd 14).
///
/// Wraps `jpeg_bytes` in B-class chunks via
/// [`BCommandPacket::build_packets_for_write`]. Empty input is allowed
/// and produces a single zero-data packet (matches the C# chunker's
/// empty-payload semantics in `BCommandPacket.cs:153-157`).
///
/// ## Wire shape
///
/// **The wire shape for cmd 14 is presumed**, not verified live. It is
/// derived by analogy from cmd 20 (`SetLEDColors`) — the only B-class
/// write command whose call site we have read directly. Both use the
/// same `BCommandPacket::build_packets_for_write` chunking, with
/// per-chunk Command/DataSize replicated and PacketNumber incrementing
/// from 0. There is no `isFanLed` flag byte or zero pad: the payload IS
/// the JPEG bytes, starting at offset 11 of the first packet.
///
/// If a future investigation surfaces a different wire shape (e.g., a
/// header byte distinguishing JPEG mode, a frame index, an end-of-frame
/// sentinel), this codec must be updated and a live capture committed.
///
/// ## See also
///
/// - `build_upload_jpeg_request` (Phase 0d Task 8) — same plumbing
///   with cmd 15.
/// - The `lconnect-image` workspace crate produces 480x480 JPEG bytes
///   suitable for direct use here.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_send_jpeg_request(jpeg_bytes: &[u8]) -> Vec<[u8; OUTPUT_LEN]> {
    BCommandPacket::build_packets_for_write(Command::SendJpeg as u8, jpeg_bytes)
        .map(|p| *p.as_bytes())
        .collect()
}
