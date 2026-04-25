//! `SendH264Frame` (cmd 13) — H.264 frame upload to the LCDV2's pump-cap
//! LCD.
//!
//! Like cmd 14 (`SendJpeg`), this is a transient single-frame command: the
//! firmware decodes and displays the frame, with auto-revert behaviour
//! similar to cmd 14 (cmd 13's hold-time is not yet live-verified). Use
//! the `lconnect-image` workspace crate or an external H.264 encoder to
//! produce frame bytes; this codec is a faithful pass-through.
//!
//! ## SendH264Frame vs SendH264Frame512
//!
//! The C# device class exposes two H.264 paths:
//! - `SendH264Frame` (cmd 13, this codec) — uses BCommandPacket framing
//!   (1024-byte output / 512-byte input, 1013-byte payload chunks).
//! - `SendH264Frame512` (cmd 13, NOT implemented here) — uses
//!   CCommandPacket framing, which is a third framing class with a
//!   different packet length and header layout we have not yet
//!   reverse-engineered. Out of scope for Phase 0d; deferred to a
//!   future phase that needs CCommandPacket.
//!
//! Source: `Galahad2Vision/Galahad2VisionDevice.cs:255-258` (BCommandPacket
//! variant, this codec) and `:262-267` (CCommandPacket variant, deferred).
//! `Galahad2Vision/Commands.cs:10` for the cmd byte.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;

/// Maximum H.264 frame size accepted by [`build_send_h264_frame_request`].
///
/// Set to 256 KiB, matching [`super::jpeg::MAX_JPEG_BYTES`]. H.264 frame
/// sizes vary widely with content (I-frames are larger than P-frames; a
/// 480×480 24fps stream typically produces 1-50 KiB per frame). The cap
/// is conservative and can be lifted if a future capture surfaces a
/// higher legitimate limit.
pub const MAX_H264_FRAME_BYTES: usize = 256 * 1024;

/// Build the BCommandPacket sequence for `SendH264Frame` (cmd 13).
///
/// Wraps `frame_bytes` in B-class chunks via
/// [`BCommandPacket::build_packets_for_write`].
///
/// # Caveats
///
/// **The wire shape for cmd 13 is verified against the C# source** —
/// `Galahad2VisionDevice.cs:255-258` performs `writeBCommand(13,
/// frameBytes)` directly, with no flag byte, no zero pad, and no
/// per-frame header. The codec is a faithful pass-through.
///
/// However, **the codec is not yet live-verified on FW 1.6.** The C#
/// source confirms the wire shape; live behaviour (hold time, decoder
/// readiness, auto-revert semantics) is unknown.
///
/// **The C# also reads a 512-byte BCommandPacket response** with a
/// 20ms timeout (`readCommand(out numArray, 512, 20)`). This codec
/// does not perform that read — it builds the request packets only.
/// Callers (e.g. a future `verify-galahad send-h264` subcommand) must
/// perform the response read themselves and decide what to do with
/// the contents (the C# discards the buffer; we may want to parse
/// it for diagnostic purposes once we have a live capture).
///
/// **Sibling `SendH264Frame512` (CCommandPacket framing) is NOT
/// implemented in this codec.** That sibling uses `writeCCommand(13,
/// frameBytes)` (`Galahad2VisionDevice.cs:262-267`), a third framing
/// class we have not yet reverse-engineered. If a future phase
/// implements CCommandPacket, the parallel `build_send_h264_frame512_request`
/// belongs in this module alongside `build_send_h264_frame_request`.
///
/// **Partial-write recovery is undefined.** Same caveat as on
/// [`super::jpeg::build_send_jpeg_request`].
///
/// # Errors
///
/// Returns [`crate::Error::InvalidH264FrameSize`] when `frame_bytes` is
/// empty or exceeds [`MAX_H264_FRAME_BYTES`].
///
/// # Examples
///
/// ```
/// use lconnect_protocol::galahad2_lcdv2::h264::build_send_h264_frame_request;
///
/// // 4-byte H.264 NAL unit start code (real callers supply a full frame).
/// let payload = [0x00, 0x00, 0x00, 0x01];
/// let packets = build_send_h264_frame_request(&payload).expect("valid size");
/// assert_eq!(packets.len(), 1);
/// assert_eq!(packets[0][1], 13, "Command = SendH264Frame");
/// ```
///
/// # See also
///
/// - [`super::jpeg::build_send_jpeg_request`] — same plumbing with cmd
///   14, for static images. Cmd 14 has been live-verified;
///   cmd 13's behaviour is presumed similar but not measured.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_send_h264_frame_request(frame_bytes: &[u8]) -> crate::Result<Vec<[u8; OUTPUT_LEN]>> {
    if frame_bytes.is_empty() || frame_bytes.len() > MAX_H264_FRAME_BYTES {
        return Err(crate::Error::InvalidH264FrameSize {
            size: frame_bytes.len(),
            max: MAX_H264_FRAME_BYTES,
        });
    }
    Ok(
        BCommandPacket::build_packets_for_write(Command::SendH264Frame as u8, frame_bytes)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}
