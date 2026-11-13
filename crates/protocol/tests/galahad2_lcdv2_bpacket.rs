//! Tests for BCommandPacket framing (1024 out / 512 in, 11-byte header).
//!
//! Source: Products/Galahad2Vision/BCommandPacket.cs:1-189.

use llcontrol_protocol::galahad2_lcdv2::bpacket::{
    BCommandPacket, HEADER_LEN, INPUT_LEN, OUTPUT_LEN, PAYLOAD_MAX, REPORT_ID,
};

#[test]
fn constants_match_csharp() {
    assert_eq!(REPORT_ID, 0x02);
    assert_eq!(OUTPUT_LEN, 1024);
    assert_eq!(INPUT_LEN, 512);
    assert_eq!(HEADER_LEN, 11);
    assert_eq!(PAYLOAD_MAX, 1013);
}

#[test]
fn new_request_single_packet_emits_full_1024_bytes_with_header() {
    let payload = [0xAA, 0xBB, 0xCC, 0xDD];
    let pkt = BCommandPacket::new_request(0x14, 0, &payload, payload.len() as u32);
    let bytes = pkt.as_bytes();

    assert_eq!(bytes.len(), OUTPUT_LEN);
    assert_eq!(bytes[0], REPORT_ID, "ReportID");
    assert_eq!(bytes[1], 0x14, "Command");
    // DataSize = 4 (32-bit BE)
    assert_eq!(&bytes[2..6], &[0, 0, 0, 4]);
    // PacketNumber = 0 (24-bit BE)
    assert_eq!(&bytes[6..9], &[0, 0, 0]);
    // Length = 4 (16-bit BE)
    assert_eq!(&bytes[9..11], &[0, 4]);
    // Payload at offset 11
    assert_eq!(&bytes[11..15], &payload);
    // Trailing bytes are zero
    assert!(bytes[15..].iter().all(|&b| b == 0));
}

#[test]
fn new_request_packet_number_packs_24_bit_be() {
    let pkt = BCommandPacket::new_request(0x14, 0x10_20_30, &[], 0);
    let bytes = pkt.as_bytes();
    assert_eq!(&bytes[6..9], &[0x10, 0x20, 0x30]);
}

#[test]
fn new_request_data_size_packs_32_bit_be() {
    let pkt = BCommandPacket::new_request(0x14, 0, &[], 0xDE_AD_BE_EF);
    let bytes = pkt.as_bytes();
    assert_eq!(&bytes[2..6], &[0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn from_response_accepts_512_byte_input() {
    // Synth: ReportID=2, cmd=23, DataSize=1, PacketNumber=0, Length=1, payload=[1].
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 23;
    buf[5] = 1; // DataSize low byte
    buf[10] = 1; // Length low byte
    buf[11] = 1; // payload[0]

    let parsed = BCommandPacket::from_response(&buf, 23).expect("parses");
    assert_eq!(parsed.command(), 23);
    assert_eq!(parsed.data_size(), 1);
    assert_eq!(parsed.packet_number(), 0);
    assert_eq!(parsed.length(), 1);
    assert_eq!(parsed.data(), &[1]);
}

#[test]
fn from_response_too_short_rejected() {
    let err = BCommandPacket::from_response(&[0x02, 23][..], 23).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::BufferTooShort { need: 11, .. }
    ));
}

#[test]
fn from_response_wrong_report_id_rejected() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = 0x01; // Should be 0x02 for B-packets
    buf[1] = 23;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::UnexpectedReportId {
            expected: 0x02,
            got: 0x01
        }
    ));
}

#[test]
fn from_response_wrong_command_echo_rejected() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 99;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::UnexpectedCommand {
            expected: 23,
            got: 99
        }
    ));
}

#[test]
fn from_response_length_exceeding_payload_max_rejected() {
    let mut buf = [0u8; INPUT_LEN];
    buf[0] = REPORT_ID;
    buf[1] = 23;
    // Length = 1014 (one over the 1013 max).
    buf[9] = 0x03;
    buf[10] = 0xF6;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::InvalidLength {
            length: 1014,
            max: 1013
        }
    ));
}

#[test]
fn new_request_payload_at_max_fills_payload_region() {
    // Exactly PAYLOAD_MAX bytes should fit without truncation.
    let payload = vec![0xAB_u8; PAYLOAD_MAX];
    let pkt = BCommandPacket::new_request(0x14, 0, &payload, payload.len() as u32);
    let bytes = pkt.as_bytes();
    assert_eq!(&bytes[9..11], &(PAYLOAD_MAX as u16).to_be_bytes());
    assert_eq!(
        &bytes[HEADER_LEN..HEADER_LEN + PAYLOAD_MAX],
        payload.as_slice()
    );
    assert_eq!(
        bytes[OUTPUT_LEN - 1],
        0xAB,
        "last byte of buffer is the last payload byte"
    );
}

#[test]
fn new_request_payload_over_max_truncates_silently() {
    // PAYLOAD_MAX + 1 bytes — the extra byte should be dropped.
    let payload = vec![0xCD_u8; PAYLOAD_MAX + 1];
    let pkt = BCommandPacket::new_request(0x14, 0, &payload, PAYLOAD_MAX as u32);
    let bytes = pkt.as_bytes();
    // Length field encodes PAYLOAD_MAX, not PAYLOAD_MAX + 1.
    assert_eq!(&bytes[9..11], &(PAYLOAD_MAX as u16).to_be_bytes());
    // Last byte of the wire buffer is the last in-range payload byte.
    assert_eq!(bytes[OUTPUT_LEN - 1], 0xCD);
}

#[test]
fn chunk_empty_payload_emits_single_zero_packet() {
    let pkts: Vec<_> = BCommandPacket::build_packets_for_write(0x0E, &[]).collect();
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0].as_bytes();
    assert_eq!(bytes[1], 0x0E);
    assert_eq!(&bytes[2..6], &[0, 0, 0, 0], "DataSize = 0");
    assert_eq!(&bytes[9..11], &[0, 0], "Length = 0");
}

#[test]
fn chunk_single_packet_when_payload_fits_in_one() {
    let payload = [0xAB; 1013];
    let pkts: Vec<_> = BCommandPacket::build_packets_for_write(0x0E, &payload).collect();
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0].as_bytes();
    assert_eq!(&bytes[2..6], &[0, 0, 0x03, 0xF5], "DataSize = 1013");
    assert_eq!(&bytes[6..9], &[0, 0, 0], "PacketNumber = 0");
    assert_eq!(&bytes[9..11], &[0x03, 0xF5], "Length = 1013");
}

#[test]
fn chunk_two_packets_for_payload_just_over_one_chunk() {
    // 1013 + 5 = 1018 bytes → 2 packets.
    let mut payload = vec![0xAA; 1013];
    payload.extend_from_slice(&[1, 2, 3, 4, 5]);
    let pkts: Vec<_> = BCommandPacket::build_packets_for_write(0x0F, &payload).collect();
    assert_eq!(pkts.len(), 2);

    // Packet 0: full payload, PN=0
    let p0 = pkts[0].as_bytes();
    assert_eq!(
        &p0[2..6],
        &[0, 0, 0x03, 0xFA],
        "DataSize = 1018 (replicated)"
    );
    assert_eq!(&p0[6..9], &[0, 0, 0], "PN = 0");
    assert_eq!(&p0[9..11], &[0x03, 0xF5], "Length = 1013");

    // Packet 1: 5 bytes, PN=1
    let p1 = pkts[1].as_bytes();
    assert_eq!(
        &p1[2..6],
        &[0, 0, 0x03, 0xFA],
        "DataSize replicated on packet 1"
    );
    assert_eq!(&p1[6..9], &[0, 0, 1], "PN = 1");
    assert_eq!(&p1[9..11], &[0, 5], "Length = 5");
    assert_eq!(&p1[11..16], &[1, 2, 3, 4, 5]);
    // Bytes after the 5-byte payload must be zeroed (matches the C# Array.Clear).
    assert!(p1[16..].iter().all(|&b| b == 0));
}

#[test]
fn chunk_packet_number_increments_for_large_payload() {
    // 4 full chunks + 1 partial = 5 packets total.
    let payload = vec![0xCC; 1013 * 4 + 17];
    let pkts: Vec<_> = BCommandPacket::build_packets_for_write(0x14, &payload).collect();
    assert_eq!(pkts.len(), 5);
    for (i, pkt) in pkts.iter().enumerate() {
        let bytes = pkt.as_bytes();
        let pn = u32::from_be_bytes([0, bytes[6], bytes[7], bytes[8]]);
        assert_eq!(pn as usize, i, "PacketNumber = packet index");
    }
}

#[test]
fn chunk_exact_multiple_of_payload_max_emits_no_phantom_packet() {
    // 2 * PAYLOAD_MAX bytes → exactly 2 packets, never 3.
    // Catches a regression where an "off-by-one phantom empty tail"
    // could slip in.
    let payload = vec![0xBB_u8; PAYLOAD_MAX * 2];
    let pkts: Vec<_> = BCommandPacket::build_packets_for_write(0x0E, &payload).collect();
    assert_eq!(
        pkts.len(),
        2,
        "no phantom 3rd packet at the multiple boundary"
    );

    let p0 = pkts[0].as_bytes();
    assert_eq!(&p0[6..9], &[0, 0, 0], "PN = 0");
    assert_eq!(
        &p0[9..11],
        &(PAYLOAD_MAX as u16).to_be_bytes(),
        "Length = PAYLOAD_MAX"
    );

    let p1 = pkts[1].as_bytes();
    assert_eq!(&p1[6..9], &[0, 0, 1], "PN = 1");
    assert_eq!(
        &p1[9..11],
        &(PAYLOAD_MAX as u16).to_be_bytes(),
        "Length = PAYLOAD_MAX"
    );
}
