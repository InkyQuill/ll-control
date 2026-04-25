//! `SendJpeg` (cmd 14) and `UploadJpeg` (cmd 15) — JPEG byte stream
//! upload to the LCDV2's pump-cap LCD.
//!
//! The 480x480 LCD accepts JPEG-encoded frames; for smaller-than-LCD
//! image content, see the `lconnect-image` workspace crate for a
//! converter that produces 480x480 JPEG bytes ready to feed in here.
//!
//! # SendJpeg vs UploadJpeg
//!
//! Neither command has a located call site in any of the
//! L-Connect.Core, L-Connect3, or L-Connect-Service codebases — the
//! wire shapes were originally derived by analogy with
//! `SetLEDColors` (cmd 20), the only B-class write whose call site we
//! can read directly.
//!
//! **Cmd 14 (`SendJpeg`) is now live-verified on FW 1.6.** See
//! `docs/protocol/captures/2026-04-25-jpeg-upload-live.txt`: the
//! presumed wire shape — chunked BCommandPacket with raw JPEG bytes
//! starting at offset 11, no frame header — drives the LCD reliably.
//! Empirically the firmware decodes and displays within ~50 ms but
//! auto-reverts after a bounded hold time (see
//! [`build_send_jpeg_request`]'s caveats).
//!
//! **Cmd 15 (`UploadJpeg`) is codec-verified only.** A live 56-packet
//! transfer ACK'd in full but produced no visible LCD change in either
//! default mode or after `LCDControl{mode=Application}` preflight; the
//! activation path is unknown on FW 1.6. See
//! [`build_upload_jpeg_request`]'s caveats.
//!
//! Source: `Galahad2Vision/Commands.cs:11,12` (`SendJpeg = 14`,
//! `UploadJpeg = 15`).
//!
//! ## Future refactor notes
//!
//! - `build_send_jpeg_request`, `build_upload_jpeg_request`, AND
//!   `super::h264::build_send_h264_frame_request` differ only by their
//!   cmd byte and the error variant they return. With three near-clone
//!   sites the DRY threshold has effectively been crossed, but the
//!   extraction is deliberately deferred: live captures may still
//!   surface a divergent field on cmd 14, 15, or 13 (we have C# call-
//!   site evidence only for cmd 13). The cross-check tests in
//!   `tests/galahad2_lcdv2_upload_jpeg.rs` and
//!   `tests/galahad2_lcdv2_send_h264_frame.rs` enforce byte-level
//!   equivalence for now. If a fourth B-class write codec is added
//!   before the live captures land, extract a private helper at that
//!   point unconditionally.
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
/// **Cmd 14 is verified live on FW 1.6** (capture file
/// `docs/protocol/captures/2026-04-25-jpeg-upload-live.txt`). The wire
/// shape — chunked BCommandPacket with raw JPEG bytes starting at
/// offset 11, per-chunk Command and DataSize replicated, PacketNumber
/// incrementing from 0, no `isFanLed` flag and no zero pad — matches
/// firmware expectations across 21-, 46-, and 56-packet transfers
/// without tearing.
///
/// **Cmd 14 is a transient single-frame command, NOT a persistent
/// display.** The firmware decodes and displays the JPEG within
/// ~50ms of the last packet but auto-reverts to LocalUI default
/// after ~20 seconds (mode-independent — multiple test runs confirm
/// [`LCDControlMode::Application`](super::lcd::LCDControlMode::Application)
/// preflight produces no observable change in the hold time). For
/// sustained static-image display, the daemon must send a fresh
/// SendJpeg every ≤15 seconds. The C# UI's
/// `LCDControlSetting.VideoFPS = 24` is consistent with a
/// continuous-frame architecture (24 fps × ~20 s ≈ a 480-frame
/// buffer), though we have no direct evidence on the firmware's
/// internal mechanism.
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
/// **Cmd 15's wire shape is presumed**, derived by analogy from cmd 14
/// (which IS live-verified). The codec writes the same chunked
/// `BCommandPacket` form, only changing the cmd byte; the cross-check
/// tests in `tests/galahad2_lcdv2_upload_jpeg.rs` pin the byte-level
/// equivalence.
///
/// **Cmd 15's visible behaviour is currently unknown on FW 1.6.** Live
/// runs (capture file
/// `docs/protocol/captures/2026-04-25-jpeg-upload-live.txt`) show the
/// device accepts 56-packet transfers and ACKs every 1024-byte write,
/// but no visible LCD change occurs in either default mode or after
/// `LCDControl{mode=Application}` preflight. Plausible (unverified)
/// explanations:
/// - Stored to non-volatile storage for boot / startup display; would
///   only become visible after a device power-cycle.
/// - Requires a separate trigger command outside the documented
///   `Galahad2Vision/Commands.cs` enum.
/// - Used by an internal display mode
///   ([`super::lcd::LCDControlMode::LocalAVI`],
///   [`super::lcd::LCDControlMode::LocalH264`]) we have not activated.
///
/// **Recommendation: do not use this codec in production daemon code
/// until the activation path is empirically discovered.** Cmd 14 +
/// [`super::lcd::LCDControlMode::Application`] + a daemon-level
/// refresh loop is the established working path for static-image
/// display.
///
/// **The semantic difference between cmd 14 (`SendJpeg`) and cmd 15
/// (`UploadJpeg`) remains partially unverified.** The enum naming in
/// `Galahad2Vision/Commands.cs:11-12` suggests `Send*` is for transient
/// single-frame display and `Upload*` is for persisted, paged, or
/// otherwise differently-handled writes (matching the broader
/// `Send*` / `Upload*` pattern across the C# Commands enums —
/// `SendH264Frame` vs
/// `UploadH264`/`UploadH264Info`/`UploadAvi`/`UploadStartupAviMovie`).
/// Cmd 14 is now empirically transient (auto-revert observed). Cmd
/// 15's no-visible-effect live behaviour is *consistent with* — but
/// not proof of — a "stored for later" persistence semantic.
///
/// **Do not interleave cmd 14 and cmd 15 writes on the same device
/// handle.** Same caveat as on [`build_send_jpeg_request`].
///
/// **Partial-write recovery is undefined.** Same caveat as on
/// [`build_send_jpeg_request`].
///
/// **If cmd 15 ultimately turns out to write to non-volatile storage**
/// (consistent with the broader naming pattern; live data does not
/// yet confirm or refute the NV-storage hypothesis), avoid calling it
/// in high-frequency loops. Flash write cycles are finite (~10K-100K
/// typical for embedded controllers). For animation playback, use
/// [`build_send_jpeg_request`] (cmd 14) instead — empirically
/// transient and not flash-write-cycle bound.
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
