//! Tests for SendJpeg (cmd 14) codec. Wire shape is PRESUMED — see the
//! module-level rustdoc on `crate::galahad2_lcdv2::jpeg`.

use lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request;

#[test]
fn build_send_jpeg_empty_emits_single_zero_packet() {
    // Matches the C# chunker's empty-payload semantics
    // (BCommandPacket.cs:153-157 yields one packet with DataSize=0).
    let pkts = build_send_jpeg_request(&[]);
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[0], 0x02, "ReportID");
    assert_eq!(bytes[1], 14, "Command = SendJpeg");
    assert_eq!(&bytes[2..6], &[0, 0, 0, 0], "DataSize = 0");
    assert_eq!(&bytes[6..9], &[0, 0, 0], "PacketNumber = 0");
    assert_eq!(&bytes[9..11], &[0, 0], "Length = 0");
}

#[test]
fn build_send_jpeg_short_payload_fits_in_one_packet() {
    // 4 bytes: classic JPEG SOI marker (0xFFD8) + a few bytes.
    let payload = [0xFF, 0xD8, 0xFF, 0xE0];
    let pkts = build_send_jpeg_request(&payload);
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[1], 14);
    assert_eq!(&bytes[2..6], &[0, 0, 0, 4], "DataSize = 4");
    assert_eq!(&bytes[9..11], &[0, 4], "Length = 4");
    assert_eq!(&bytes[11..15], &payload);
    // Trailing bytes zero-padded.
    assert!(bytes[15..].iter().all(|&b| b == 0));
}

#[test]
fn build_send_jpeg_at_payload_max_boundary() {
    // Exactly 1013 bytes — the single-packet ceiling. NO chunking.
    let payload = vec![0xAA_u8; 1013];
    let pkts = build_send_jpeg_request(&payload);
    assert_eq!(pkts.len(), 1, "exactly 1013 fits in one packet");
    let bytes = pkts[0];
    assert_eq!(&bytes[2..6], &[0, 0, 0x03, 0xF5], "DataSize = 1013");
    assert_eq!(&bytes[9..11], &[0x03, 0xF5], "Length = 1013");
}

#[test]
fn build_send_jpeg_above_payload_max_chunks_correctly() {
    // 1018 bytes = 1013 + 5 → 2 packets, with PN incrementing.
    let mut payload = vec![0xAA_u8; 1013];
    payload.extend_from_slice(&[1, 2, 3, 4, 5]);
    let pkts = build_send_jpeg_request(&payload);
    assert_eq!(pkts.len(), 2);

    // Packet 0: full 1013-byte chunk, PN=0
    let p0 = pkts[0];
    assert_eq!(p0[1], 14);
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
