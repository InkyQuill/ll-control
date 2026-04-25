//! `SendJpeg` (cmd 14) and `UploadJpeg` (cmd 15) — JPEG byte stream
//! upload to the LCDV2's pump-cap LCD.
//!
//! The 480x480 LCD accepts JPEG-encoded frames; for smaller-than-LCD
//! image content, see the `lconnect-image` workspace crate for a
//! converter that produces 480x480 JPEG bytes ready to feed in here.
//!
//! # SendJpeg vs UploadJpeg
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
//!
//! ## Future refactor notes
//!
//! - `build_send_jpeg_request` and `build_upload_jpeg_request` differ
//!   only by their cmd byte today. They are kept as two separate
//!   functions (rather than collapsed to a private helper) until a live
//!   capture confirms whether the wire shapes are truly identical or
//!   one of them gains a divergent field. The cross-check tests in
//!   `tests/galahad2_lcdv2_upload_jpeg.rs` enforce equivalence at the
//!   byte level for both single-packet and multi-packet payloads.
//! - The return type `Vec<[u8; OUTPUT_LEN]>` may migrate to
//!   `impl Iterator<Item = [u8; OUTPUT_LEN]>` in a future symmetry pass
//!   that also touches `set_led_colors::build_set_*_colors_request`.
//!   Deferred to keep the shape change in a single sweep.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;

/// Maximum JPEG byte count accepted by `build_send_jpeg_request` and
/// `build_upload_jpeg_request`. Set to 256 KiB — comfortably above the
/// upper bound of any reasonable 480x480 JPEG (even at quality 100, full
/// of natural-image entropy, the encoded size rarely exceeds 100 KiB) but
/// well below buffer sizes that risk firmware OOM. Conservatively chosen;
/// can be lifted if a future capture surfaces a higher legitimate limit.
pub const MAX_JPEG_BYTES: usize = 256 * 1024;

/// Build the BCommandPacket sequence for `SendJpeg` (cmd 14).
///
/// Wraps `jpeg_bytes` in B-class chunks via
/// [`BCommandPacket::build_packets_for_write`].
///
/// # Caveats
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
/// **Do not interleave cmd 14 (`SendJpeg`) and cmd 15 (`UploadJpeg`)
/// writes on the same device handle.** If the firmware uses different
/// buffering or apply semantics for the two commands, switching mid-
/// transfer may leave the LCD decoder in an undefined state. Complete
/// or abort the current transfer before switching commands.
///
/// **Partial-write recovery is undefined.** If a per-packet write fails
/// on packet `N` of `M`, the firmware's transfer state is unknown.
/// Recovery requires either a fresh complete transfer or a device
/// reset (cmd 142).
///
/// # Errors
///
/// Returns [`crate::Error::InvalidJpegSize`] when `jpeg_bytes.is_empty()`
/// or `jpeg_bytes.len()` exceeds [`MAX_JPEG_BYTES`].
///
/// # Examples
///
/// ```
/// use lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request;
///
/// // 4-byte SOI fragment (real callers should use the lconnect-image
/// // crate to produce a 480x480 JPEG byte stream).
/// let payload = [0xFF, 0xD8, 0xFF, 0xE0];
/// let packets = build_send_jpeg_request(&payload).expect("valid size");
/// assert_eq!(packets.len(), 1);
/// assert_eq!(packets[0][1], 14, "Command = SendJpeg");
/// ```
///
/// # See also
///
/// - [`build_upload_jpeg_request`] — same plumbing with cmd 15.
/// - The `lconnect-image` workspace crate produces 480x480 JPEG bytes
///   suitable for direct use here.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_send_jpeg_request(jpeg_bytes: &[u8]) -> crate::Result<Vec<[u8; OUTPUT_LEN]>> {
    if jpeg_bytes.is_empty() || jpeg_bytes.len() > MAX_JPEG_BYTES {
        return Err(crate::Error::InvalidJpegSize {
            size: jpeg_bytes.len(),
            max: MAX_JPEG_BYTES,
        });
    }
    Ok(
        BCommandPacket::build_packets_for_write(Command::SendJpeg as u8, jpeg_bytes)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}

/// Build the BCommandPacket sequence for `UploadJpeg` (cmd 15).
///
/// Wraps `jpeg_bytes` in B-class chunks via
/// [`BCommandPacket::build_packets_for_write`].
///
/// # Caveats
///
/// **The wire shape for cmd 15 is presumed**, not verified live —
/// derived by analogy from cmd 20 (`SetLEDColors`) for the same
/// reasons documented on [`build_send_jpeg_request`].
///
/// **The semantic difference between cmd 14 (`SendJpeg`) and cmd 15
/// (`UploadJpeg`) is also presumed.** The enum naming in
/// `Galahad2Vision/Commands.cs:11-12` suggests `Send*` is for transient
/// single-frame display and `Upload*` is for persisted, paged, or
/// otherwise differently-handled writes (this matches the broader pattern
/// of `Send*` / `Upload*` pairs in the C# Commands enums — `SendH264Frame`
/// vs `UploadH264`/`UploadH264Info`/`UploadAvi`/`UploadStartupAviMovie`).
/// The exact persistence, paging, or handshake semantics differ between
/// cmd 14 and cmd 15 are unverified — treat both as opaque byte-stream
/// uploads until a live capture differentiates them.
///
/// **Do not interleave cmd 14 and cmd 15 writes on the same device
/// handle.** Same caveat as on [`build_send_jpeg_request`].
///
/// **Partial-write recovery is undefined.** Same caveat as on
/// [`build_send_jpeg_request`].
///
/// **If cmd 15 writes to non-volatile storage** (consistent with the
/// presumed semantic), avoid calling it in high-frequency loops. Flash
/// write cycles are finite (~10K-100K typical for embedded controllers).
/// For animation playback, use [`build_send_jpeg_request`] (cmd 14)
/// instead — assumed to be transient and not flash-write-cycle bound.
///
/// **CLI consumers (e.g. `verify-galahad upload-jpeg` in Phase 0d Task 9)
/// should require explicit user consent before calling this function**,
/// in the spirit of the `--yes-i-really-want-to-reset` gate on
/// [`build_reset_request`](super::reset::build_request). Cmd 15 may have
/// irreversible effects on device state (e.g. overwriting a saved boot
/// image); treat it as a destructive operation at the CLI surface.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidJpegSize`] when `jpeg_bytes.is_empty()`
/// or `jpeg_bytes.len()` exceeds [`MAX_JPEG_BYTES`].
///
/// # Examples
///
/// ```
/// use lconnect_protocol::galahad2_lcdv2::jpeg::build_upload_jpeg_request;
///
/// let payload = [0xFF, 0xD8, 0xFF, 0xE0];
/// let packets = build_upload_jpeg_request(&payload).expect("valid size");
/// assert_eq!(packets.len(), 1);
/// assert_eq!(packets[0][1], 15, "Command = UploadJpeg");
/// ```
///
/// # See also
///
/// - [`build_send_jpeg_request`] — same plumbing with cmd 14.
/// - The `lconnect-image` workspace crate produces 480x480 JPEG bytes
///   suitable for direct use here.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_upload_jpeg_request(jpeg_bytes: &[u8]) -> crate::Result<Vec<[u8; OUTPUT_LEN]>> {
    if jpeg_bytes.is_empty() || jpeg_bytes.len() > MAX_JPEG_BYTES {
        return Err(crate::Error::InvalidJpegSize {
            size: jpeg_bytes.len(),
            max: MAX_JPEG_BYTES,
        });
    }
    Ok(
        BCommandPacket::build_packets_for_write(Command::UploadJpeg as u8, jpeg_bytes)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}
