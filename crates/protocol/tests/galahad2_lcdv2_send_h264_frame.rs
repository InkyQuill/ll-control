//! Tests for SendH264Frame (cmd 13) codec. Wire shape is C#-source-verified
//! (see `Galahad2VisionDevice.cs:255-258`) — the call site is
//! `writeBCommand(13, frameBytes)` directly. See the module-level rustdoc
//! on `crate::galahad2_lcdv2::h264` for live-verification status.

use llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_request;

#[test]
fn build_send_h264_frame_short_payload_fits_in_one_packet() {
    // 4 bytes: H.264 NAL unit start code (0x00000001).
    let payload = [0x00, 0x00, 0x00, 0x01];
    let pkts = build_send_h264_frame_request(&payload).expect("valid size");
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[1], 13, "Command = SendH264Frame");
    assert_eq!(&bytes[2..6], &[0, 0, 0, 4], "DataSize = 4");
    assert_eq!(&bytes[9..11], &[0, 4], "Length = 4");
    assert_eq!(&bytes[11..15], &payload);
    // Trailing bytes zero-padded.
    assert!(bytes[15..].iter().all(|&b| b == 0));
}

#[test]
fn build_send_h264_frame_at_payload_max_boundary() {
    // Exactly 1013 bytes — the single-packet ceiling. NO chunking.
    let payload = vec![0xAA_u8; 1013];
    let pkts = build_send_h264_frame_request(&payload).expect("valid size");
    assert_eq!(pkts.len(), 1, "exactly 1013 fits in one packet");
    let bytes = pkts[0];
    assert_eq!(&bytes[2..6], &[0, 0, 0x03, 0xF5], "DataSize = 1013");
    assert_eq!(&bytes[9..11], &[0x03, 0xF5], "Length = 1013");
    // At PAYLOAD_MAX, the entire 1013-byte payload occupies bytes [11..1024]
    // of the wire packet. Catches an off-by-one in the chunker's slice logic.
    assert!(
        bytes[11..].iter().all(|&b| b == 0xAA),
        "full 1013-byte payload embedded; no zero pad expected at exact boundary"
    );
}

#[test]
fn build_send_h264_frame_above_payload_max_chunks_correctly() {
    // 1018 bytes = 1013 + 5 → 2 packets, with PN incrementing.
    let mut payload = vec![0xAA_u8; 1013];
    payload.extend_from_slice(&[1, 2, 3, 4, 5]);
    let pkts = build_send_h264_frame_request(&payload).expect("valid size");
    assert_eq!(pkts.len(), 2);

    // Packet 0: full 1013-byte chunk, PN=0
    let p0 = pkts[0];
    assert_eq!(p0[1], 13);
    assert_eq!(
        &p0[2..6],
        &[0, 0, 0x03, 0xFA],
        "DataSize = 1018 (replicated)"
    );
    assert_eq!(&p0[6..9], &[0, 0, 0], "PN = 0");
    assert_eq!(&p0[9..11], &[0x03, 0xF5], "Length = 1013");

    // Packet 1: 5-byte tail, PN=1
    let p1 = pkts[1];
    assert_eq!(
        &p1[2..6],
        &[0, 0, 0x03, 0xFA],
        "DataSize replicated on packet 1"
    );
    assert_eq!(&p1[6..9], &[0, 0, 1], "PN = 1");
    assert_eq!(&p1[9..11], &[0, 5], "Length = 5");
    assert_eq!(&p1[11..16], &[1, 2, 3, 4, 5]);
}

#[test]
fn build_send_h264_frame_rejects_empty() {
    let err = build_send_h264_frame_request(&[]).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::InvalidH264FrameSize {
            size: 0,
            max: llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES
        }
    ));
}

#[test]
fn build_send_h264_frame_rejects_over_max() {
    let too_big = vec![0xFF_u8; llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES + 1];
    let err = build_send_h264_frame_request(&too_big).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::InvalidH264FrameSize { .. }
    ));
}

#[test]
fn build_send_h264_frame_accepts_at_max() {
    // MAX_H264_FRAME_BYTES (256 KiB = 262144 bytes) → ceil(262144 / 1013) = 259 packets.
    let at_max = vec![0xAA_u8; llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES];
    let pkts = build_send_h264_frame_request(&at_max).expect("at-max is valid");
    assert_eq!(
        pkts.len(),
        259,
        "262144 / 1013 = 258.something → 259 packets"
    );
    // Spot-check first and last:
    let first = pkts[0];
    assert_eq!(first[1], 13, "Command = SendH264Frame");
    assert_eq!(
        u32::from_be_bytes([first[2], first[3], first[4], first[5]]) as usize,
        llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES,
        "DataSize = MAX_H264_FRAME_BYTES on every packet"
    );
    let last = pkts[258];
    assert_eq!(
        u32::from_be_bytes([last[2], last[3], last[4], last[5]]) as usize,
        llcontrol_protocol::galahad2_lcdv2::h264::MAX_H264_FRAME_BYTES,
        "DataSize replicated on the last packet"
    );
    assert_eq!(&last[6..9], &[0, 1, 2], "PN=258 = 0x010002 (24-bit BE)");
    // Last packet length = 262144 - 258*1013 = 262144 - 261354 = 790.
    assert_eq!(
        u16::from_be_bytes([last[9], last[10]]) as usize,
        262144 - 258 * 1013,
        "tail length = 262144 mod 1013"
    );
}

#[test]
fn send_h264_frame_and_send_jpeg_differ_only_by_cmd_byte() {
    use llcontrol_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request;
    // 4-byte H.264 NAL unit start code.
    // Use &payload[..] (explicit slice borrow) rather than &payload —
    // both forms work due to deref coercion, but the slice form makes
    // the borrow type identical to what build_*_request takes, removing
    // any ambiguity for future readers about whether the test exercises
    // a deref path that the codecs also exercise.
    let payload = [0x00, 0x00, 0x00, 0x01];
    let h264 = build_send_h264_frame_request(&payload[..]).expect("valid");
    let jpeg = build_send_jpeg_request(&payload[..]).expect("valid");
    assert_eq!(h264.len(), jpeg.len(), "same chunk count");
    for (h, j) in h264.iter().zip(jpeg.iter()) {
        // Compare every byte EXCEPT [1] (the cmd byte).
        assert_eq!(h[0], j[0], "ReportID matches");
        assert_eq!(h[1], 13, "send_h264_frame cmd = 13");
        assert_eq!(j[1], 14, "send_jpeg cmd = 14");
        assert_eq!(&h[2..], &j[2..], "all post-cmd bytes match");
    }
}

#[test]
fn parse_send_h264_frame_response_accepts_minimal_512_byte_input() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 13;
    let parsed = llcontrol_protocol::galahad2_lcdv2::h264::parse_send_h264_frame_response(&buf)
        .expect("valid empty response");
    assert_eq!(parsed.command(), 13);
    assert_eq!(parsed.length(), 0);
    assert_eq!(parsed.data_size(), 0);
}

#[test]
fn parse_send_h264_frame_response_rejects_wrong_command_echo() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 99; // not 13
    let err =
        llcontrol_protocol::galahad2_lcdv2::h264::parse_send_h264_frame_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::UnexpectedCommand {
            expected: 13,
            got: 99
        }
    ));
}

#[test]
fn parse_send_h264_frame_response_rejects_too_short() {
    let buf = [0x02_u8, 13];
    let err =
        llcontrol_protocol::galahad2_lcdv2::h264::parse_send_h264_frame_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::BufferTooShort { .. }
    ));
}

// ---------------------------------------------------------------------------
// SendH264Frame512 (cmd 13, C-class) — sibling of the B-class codec above.
// Same cmd byte; HID-layer discrimination via ReportID (0x02 vs 0x03).
// Wire shape verified against `Galahad2VisionDevice.cs:262, 353` per the
// Phase 0e static survey. Codec-only — live verification deferred until an
// H.264 frame pipeline exists (Phase 4+).
// ---------------------------------------------------------------------------

use llcontrol_protocol::galahad2_lcdv2::h264::{
    build_send_h264_frame_512_request, MAX_H264_FRAME_BYTES,
};

#[test]
fn send_h264_frame_512_empty_rejects() {
    let r = build_send_h264_frame_512_request(&[]);
    assert!(r.is_err(), "empty frame must error: {r:?}");
    assert!(matches!(
        r.unwrap_err(),
        llcontrol_protocol::Error::InvalidH264FrameSize {
            size: 0,
            max: MAX_H264_FRAME_BYTES
        }
    ));
}

#[test]
fn send_h264_frame_512_oversize_rejects() {
    let huge = vec![0u8; MAX_H264_FRAME_BYTES + 1];
    let r = build_send_h264_frame_512_request(&huge);
    assert!(r.is_err(), "oversize frame must error: {r:?}");
    assert!(matches!(
        r.unwrap_err(),
        llcontrol_protocol::Error::InvalidH264FrameSize { .. }
    ));
}

#[test]
fn send_h264_frame_512_small_one_packet() {
    let frame = vec![0xAB; 100];
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][0], 0x03, "ReportID for C-class");
    assert_eq!(packets[0][1], 0x0D, "cmd byte (same as B-class)");
    assert_eq!(packets[0].len(), 512, "C-class output is 512 bytes");
    // DataSize = 100, Length = 100, payload starts at offset 11.
    assert_eq!(&packets[0][2..6], &[0, 0, 0, 100], "DataSize = 100");
    assert_eq!(&packets[0][9..11], &[0, 100], "Length = 100");
    assert!(
        packets[0][11..111].iter().all(|&b| b == 0xAB),
        "payload region populated"
    );
}

#[test]
fn send_h264_frame_512_at_payload_max_one_packet() {
    use llcontrol_protocol::galahad2_lcdv2::cpacket::PAYLOAD_MAX;
    let frame = vec![0xAB; PAYLOAD_MAX]; // 501 bytes
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_eq!(packets.len(), 1, "exactly PAYLOAD_MAX fits in one packet");
    let bytes = packets[0];
    assert_eq!(&bytes[2..6], &[0, 0, 0x01, 0xF5], "DataSize = 501");
    assert_eq!(&bytes[9..11], &[0x01, 0xF5], "Length = 501");
    // At PAYLOAD_MAX, the payload occupies bytes [11..512] of the wire packet.
    assert!(
        bytes[11..].iter().all(|&b| b == 0xAB),
        "full 501-byte payload embedded; no zero pad expected at exact boundary"
    );
}

#[test]
fn send_h264_frame_512_multipacket() {
    use llcontrol_protocol::galahad2_lcdv2::cpacket::PAYLOAD_MAX;
    let frame = vec![0xAB; PAYLOAD_MAX * 2 + 50];
    let packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_eq!(packets.len(), 3);
    // PN increments across the chunks.
    assert_eq!(&packets[0][6..9], &[0, 0, 0], "PN = 0");
    assert_eq!(&packets[1][6..9], &[0, 0, 1], "PN = 1");
    assert_eq!(&packets[2][6..9], &[0, 0, 2], "PN = 2");
    // Tail length = 50.
    assert_eq!(&packets[2][9..11], &[0, 50], "tail Length = 50");
}

#[test]
fn send_h264_frame_b_and_c_class_diverge() {
    use llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_request;
    // Same payload, different framing classes -> different ReportID + chunk
    // sizes (B-class is 1024 bytes per packet, C-class is 512).
    let frame = vec![0xAB; 100];
    let b_packets = build_send_h264_frame_request(&frame).unwrap();
    let c_packets = build_send_h264_frame_512_request(&frame).unwrap();
    assert_ne!(
        b_packets[0][0], c_packets[0][0],
        "ReportID must differ between B-class (0x02) and C-class (0x03)"
    );
    assert_eq!(b_packets[0][0], 0x02);
    assert_eq!(c_packets[0][0], 0x03);
    // The cmd byte (0x0D) is identical.
    assert_eq!(b_packets[0][1], c_packets[0][1]);
    // Packet sizes differ.
    assert_eq!(b_packets[0].len(), 1024);
    assert_eq!(c_packets[0].len(), 512);
}

#[test]
fn send_h264_frame_512_at_max_size() {
    let frame = vec![0xAB; MAX_H264_FRAME_BYTES];
    let r = build_send_h264_frame_512_request(&frame);
    assert!(r.is_ok(), "at-max should succeed: {r:?}");
}
