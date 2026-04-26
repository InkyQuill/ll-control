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
//! The C# device class exposes two H.264 paths, both writing cmd byte 13:
//! - [`build_send_h264_frame_request`] — BCommandPacket framing
//!   (ReportID=0x02, 1024-byte output / 512-byte input, 1013-byte
//!   payload chunks).
//! - [`build_send_h264_frame_512_request`] — CCommandPacket framing
//!   (ReportID=0x03, 512-byte output / 512-byte input, 501-byte
//!   payload chunks).
//!
//! **Discrimination risk.** Both write cmd byte 13. The firmware
//! discriminates via the HID ReportID (B = 0x02, C = 0x03), but if a
//! particular device variant or firmware revision only accepts one
//! framing class for cmd 13, sending the other will be silently accepted
//! at the HID layer and misparsed at the firmware level (different
//! DataSize / PacketNumber / Length offsets). Before using either
//! variant against an unfamiliar firmware revision, confirm via the
//! handshake or firmware-version response which framing class cmd 13
//! supports.
//!
//! Source: `Galahad2Vision/Galahad2VisionDevice.cs:255-258` (BCommandPacket
//! variant) and `:262, 353` (CCommandPacket variant).
//! `Galahad2Vision/Commands.cs:10` for the cmd byte.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::cpacket::{CCommandPacket, OUTPUT_LEN as C_OUTPUT_LEN};

/// Maximum H.264 frame size accepted by [`build_send_h264_frame_request`].
///
/// Set to 256 KiB, matching [`super::jpeg::MAX_JPEG_BYTES`]. H.264 frame
/// sizes vary widely with content (I-frames are larger than P-frames; a
/// 480×480 24fps stream typically produces 1-50 KiB per frame). The cap
/// is conservative and can be lifted if a future capture surfaces a
/// higher legitimate limit.
///
/// # TODO(Phase 1)
///
/// Verify the cap against the firmware's internal buffer limit via a
/// live capture. If the firmware returns an error code in the
/// `SendH264Frame` BCommandPacket response for oversized frames, this
/// cap should match the firmware's limit, not [`super::jpeg::MAX_JPEG_BYTES`].
pub const MAX_H264_FRAME_BYTES: usize = 256 * 1024;

/// Build the BCommandPacket sequence for `SendH264Frame` (cmd 13).
///
/// Wraps `frame_bytes` in B-class chunks via
/// [`BCommandPacket::build_packets_for_write`].
///
/// # Caveats
///
/// **Cmd 13's wire shape is verified against the C# source** —
/// `Galahad2VisionDevice.cs:255-258` performs `writeBCommand(13,
/// frameBytes)` directly, with no flag byte, no zero pad, and no
/// per-frame header. The codec is a faithful pass-through.
///
/// However, **the codec is not yet live-verified on FW 1.6.** The C#
/// source confirms the wire shape; live behaviour (hold time, decoder
/// readiness, auto-revert semantics) is unknown.
///
/// **Partial-write recovery is undefined.** If a per-packet write
/// fails on packet N of M, the firmware's transfer state is unknown.
/// Recovery requires either a fresh complete transfer or a device
/// reset (cmd 142).
///
/// # Required follow-up reads
///
/// The C# performs a 512-byte BCommandPacket response read after every
/// cmd 13 write (`Galahad2VisionDevice.cs:259`). Use
/// [`parse_send_h264_frame_response`] for that read. **Skipping the
/// read leaves the response in the device's USB buffer**, where it
/// will pollute the next unrelated read on the same handle.
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
/// - [`build_send_h264_frame_512_request`] — C-class sibling (same cmd
///   byte, different framing). Both call sites in
///   `Galahad2VisionDevice.cs:255-258` (this codec) and `:262, 353`
///   (sibling) target the same firmware command; HID-layer
///   discrimination is via ReportID.
/// - [`super::jpeg::build_send_jpeg_request`] — same B-class plumbing
///   with cmd 14, for static images. Cmd 14 has been live-verified;
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

/// Parse a 512-byte BCommandPacket response from `SendH264Frame` (cmd 13).
///
/// The C# performs this read after every cmd 13 write
/// (`Galahad2VisionDevice.cs:259`: `readCommand(out numArray, 512, 20)`)
/// and discards the buffer. The Linux port exposes the parser as a
/// typed entry point so a future CLI consumer (or daemon) can:
/// - confirm the device acknowledged the frame (cmd byte echo at offset 1),
/// - extract whatever status / decoder feedback the response carries
///   (the C# discards it; we may want to log or branch on it once we
///   have a live capture to interpret).
///
/// **Calling this read is not optional.** A successful `SendH264Frame`
/// transfer leaves a 512-byte response in the device's USB buffer. If
/// the next read on the same device handle is for a different command
/// (e.g. a heartbeat or a handshake), it will consume this stale
/// response and surface as `UnexpectedCommand`. Always pair every
/// `build_send_h264_frame_request` call with a `parse_send_h264_frame_response`
/// (or at minimum a discard read of 512 bytes) to keep the read queue clean.
///
/// # Errors
///
/// Same as [`BCommandPacket::from_response`] — surfaces
/// `BufferTooShort`, `UnexpectedReportId`, `UnexpectedCommand`, or
/// `InvalidLength`.
///
/// # Examples
///
/// ```
/// use lconnect_protocol::galahad2_lcdv2::h264::parse_send_h264_frame_response;
///
/// // Synthesised 512-byte response: ReportID=2, cmd=13, DataSize=0, Length=0.
/// let mut buf = [0u8; 512];
/// buf[0] = 0x02;
/// buf[1] = 13;
/// let parsed = parse_send_h264_frame_response(&buf).expect("parses");
/// assert_eq!(parsed.command(), 13);
/// assert_eq!(parsed.length(), 0);
/// ```
pub fn parse_send_h264_frame_response(
    buf: &[u8],
) -> crate::Result<crate::galahad2_lcdv2::bpacket::BResponse<'_>> {
    use crate::galahad2_lcdv2::bpacket::BCommandPacket;
    BCommandPacket::from_response(buf, Command::SendH264Frame as u8)
}

/// Build the CCommandPacket sequence for `SendH264Frame512` (cmd 13).
///
/// Sibling of [`build_send_h264_frame_request`]. Same cmd byte (`0x0D`)
/// but C-class framing (ReportID=`0x03`, 512-byte output, 501-byte
/// payload chunks); HID-layer discrimination via ReportID. Wraps
/// `frame_bytes` in C-class chunks via
/// [`CCommandPacket::build_packets_for_write`].
///
/// # Caveats
///
/// **Codec-only — not yet live-verified.** Wire shape is C#-source-
/// verified against `Galahad2VisionDevice.cs:262, 353` per the Phase 0e
/// static survey: `writeCCommand(13, frameBytes)` with no flag byte, no
/// zero pad, and no per-frame header. Live verification is deferred
/// until an H.264 frame pipeline exists (Phase 4+).
///
/// **Discrimination risk shared with the B-class sibling.** Both
/// codecs target cmd byte 13. If a particular firmware revision only
/// accepts one framing class, sending the other will be silently
/// accepted at the HID layer and misparsed at the firmware level. See
/// the module-level rustdoc.
///
/// **Partial-write recovery is undefined.** If a per-packet write
/// fails on packet N of M, the firmware's transfer state is unknown.
/// Recovery requires either a fresh complete transfer or a device
/// reset (cmd 142).
///
/// `MAX_H264_FRAME_BYTES` is shared with the B-class variant; see its
/// docs for the rationale and the Phase 1 follow-up to verify against
/// firmware's internal buffer limit.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidH264FrameSize`] when `frame_bytes` is
/// empty or exceeds [`MAX_H264_FRAME_BYTES`].
///
/// # Examples
///
/// ```
/// use lconnect_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request;
///
/// let payload = [0x00, 0x00, 0x00, 0x01];
/// let packets = build_send_h264_frame_512_request(&payload).expect("valid size");
/// assert_eq!(packets.len(), 1);
/// assert_eq!(packets[0][0], 0x03, "ReportID = C-class");
/// assert_eq!(packets[0][1], 13, "Command = SendH264Frame");
/// assert_eq!(packets[0].len(), 512, "C-class output is 512 bytes");
/// ```
///
/// # See also
///
/// - [`build_send_h264_frame_request`] — B-class sibling (same cmd
///   byte, 1024-byte framing).
/// - [`crate::galahad2_lcdv2::cpacket`] — C-class framing struct.
/// - `Galahad2VisionDevice.cs:262, 353` — C# call site.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_send_h264_frame_512_request(
    frame_bytes: &[u8],
) -> crate::Result<Vec<[u8; C_OUTPUT_LEN]>> {
    if frame_bytes.is_empty() || frame_bytes.len() > MAX_H264_FRAME_BYTES {
        return Err(crate::Error::InvalidH264FrameSize {
            size: frame_bytes.len(),
            max: MAX_H264_FRAME_BYTES,
        });
    }
    Ok(
        CCommandPacket::build_packets_for_write(Command::SendH264Frame as u8, frame_bytes)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}
