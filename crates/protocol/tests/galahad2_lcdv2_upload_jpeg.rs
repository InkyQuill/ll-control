//! Tests for UploadJpeg (cmd 15) codec. Wire shape is PRESUMED — see the
//! module-level rustdoc on `crate::galahad2_lcdv2::jpeg`.

use lconnect_protocol::galahad2_lcdv2::jpeg::build_upload_jpeg_request;

#[test]
fn build_upload_jpeg_rejects_empty() {
    let err = build_upload_jpeg_request(&[]).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidJpegSize {
            size: 0,
            max: lconnect_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES
        }
    ));
}

#[test]
fn build_upload_jpeg_short_payload_fits_in_one_packet() {
    // 4 bytes: classic JPEG SOI marker (0xFFD8) + a few bytes.
    let payload = [0xFF, 0xD8, 0xFF, 0xE0];
    let pkts = build_upload_jpeg_request(&payload).expect("valid size");
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[1], 15, "Command = UploadJpeg");
    assert_eq!(&bytes[2..6], &[0, 0, 0, 4], "DataSize = 4");
    assert_eq!(&bytes[9..11], &[0, 4], "Length = 4");
    assert_eq!(&bytes[11..15], &payload);
    // Trailing bytes zero-padded.
    assert!(bytes[15..].iter().all(|&b| b == 0));
}

#[test]
fn build_upload_jpeg_at_payload_max_boundary() {
    // Exactly 1013 bytes — the single-packet ceiling. NO chunking.
    let payload = vec![0xAA_u8; 1013];
    let pkts = build_upload_jpeg_request(&payload).expect("valid size");
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
fn build_upload_jpeg_above_payload_max_chunks_correctly() {
    // 1018 bytes = 1013 + 5 → 2 packets, with PN incrementing.
    let mut payload = vec![0xAA_u8; 1013];
    payload.extend_from_slice(&[1, 2, 3, 4, 5]);
    let pkts = build_upload_jpeg_request(&payload).expect("valid size");
    assert_eq!(pkts.len(), 2);

    // Packet 0: full 1013-byte chunk, PN=0
    let p0 = pkts[0];
    assert_eq!(p0[1], 15);
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
fn build_upload_jpeg_rejects_over_max() {
    let too_big = vec![0xFF_u8; lconnect_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES + 1];
    let err = build_upload_jpeg_request(&too_big).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidJpegSize { .. }
    ));
}

#[test]
fn build_upload_jpeg_accepts_at_max() {
    // MAX_JPEG_BYTES (256 KiB = 262144 bytes) → ceil(262144 / 1013) = 259 packets.
    let at_max = vec![0xAA_u8; lconnect_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES];
    let pkts = build_upload_jpeg_request(&at_max).expect("at-max is valid");
    assert_eq!(
        pkts.len(),
        259,
        "262144 / 1013 = 258.something → 259 packets"
    );
    // Spot-check first and last:
    let first = pkts[0];
    assert_eq!(first[1], 15, "Command = UploadJpeg");
    assert_eq!(
        u32::from_be_bytes([first[2], first[3], first[4], first[5]]) as usize,
        lconnect_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES,
        "DataSize = MAX_JPEG_BYTES on every packet"
    );
    let last = pkts[258];
    assert_eq!(
        u32::from_be_bytes([last[2], last[3], last[4], last[5]]) as usize,
        lconnect_protocol::galahad2_lcdv2::jpeg::MAX_JPEG_BYTES,
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
fn upload_jpeg_and_send_jpeg_differ_only_by_cmd_byte() {
    use lconnect_protocol::galahad2_lcdv2::jpeg::{
        build_send_jpeg_request, build_upload_jpeg_request,
    };
    let payload = [0xFF, 0xD8, 0xFF, 0xE0];
    let send = build_send_jpeg_request(&payload).expect("valid");
    let upload = build_upload_jpeg_request(&payload).expect("valid");
    assert_eq!(send.len(), upload.len(), "same chunk count");
    for (s, u) in send.iter().zip(upload.iter()) {
        // Compare every byte EXCEPT [1] (the cmd byte).
        assert_eq!(s[0], u[0], "ReportID matches");
        assert_eq!(s[1], 14, "send_jpeg cmd = 14");
        assert_eq!(u[1], 15, "upload_jpeg cmd = 15");
        assert_eq!(&s[2..], &u[2..], "all post-cmd bytes match");
    }
}

#[test]
fn upload_jpeg_and_send_jpeg_differ_only_by_cmd_byte_across_chunks() {
    use lconnect_protocol::galahad2_lcdv2::jpeg::{
        build_send_jpeg_request, build_upload_jpeg_request,
    };
    // 1018 bytes → 2 packets via the chunker. Catches a future divergence
    // that only manifests on multi-packet payloads (e.g., an off-by-one
    // in the PacketNumber field for some edge case).
    let mut payload = vec![0xAA_u8; 1013];
    payload.extend_from_slice(&[1, 2, 3, 4, 5]);
    let send = build_send_jpeg_request(&payload).expect("valid");
    let upload = build_upload_jpeg_request(&payload).expect("valid");
    assert_eq!(send.len(), 2, "2 packets");
    assert_eq!(upload.len(), 2);
    for (i, (s, u)) in send.iter().zip(upload.iter()).enumerate() {
        assert_eq!(s[0], u[0], "packet {i}: ReportID matches");
        assert_eq!(s[1], 14, "packet {i}: send cmd = 14");
        assert_eq!(u[1], 15, "packet {i}: upload cmd = 15");
        assert_eq!(&s[2..], &u[2..], "packet {i}: all post-cmd bytes match");
    }
}
