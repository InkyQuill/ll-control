//! Tests for BCommandPacket framing (1024 out / 512 in, 11-byte header).
//!
//! Source: Products/Galahad2Vision/BCommandPacket.cs:1-189.

use lconnect_protocol::galahad2_lcdv2::bpacket::{
    BCommandPacket, B_HEADER_LEN, B_INPUT_LEN, B_OUTPUT_LEN, B_PAYLOAD_MAX, B_REPORT_ID,
};

#[test]
fn constants_match_csharp() {
    assert_eq!(B_REPORT_ID, 0x02);
    assert_eq!(B_OUTPUT_LEN, 1024);
    assert_eq!(B_INPUT_LEN, 512);
    assert_eq!(B_HEADER_LEN, 11);
    assert_eq!(B_PAYLOAD_MAX, 1013);
}

#[test]
fn new_request_single_packet_emits_full_1024_bytes_with_header() {
    let payload = [0xAA, 0xBB, 0xCC, 0xDD];
    let pkt = BCommandPacket::new_request(0x14, 0, &payload, payload.len() as u32);
    let bytes = pkt.as_bytes();

    assert_eq!(bytes.len(), B_OUTPUT_LEN);
    assert_eq!(bytes[0], B_REPORT_ID, "ReportID");
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
    let mut buf = [0u8; B_INPUT_LEN];
    buf[0] = B_REPORT_ID;
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
        lconnect_protocol::Error::BufferTooShort { need: 11, .. }
    ));
}

#[test]
fn from_response_wrong_report_id_rejected() {
    let mut buf = [0u8; B_INPUT_LEN];
    buf[0] = 0x01; // Should be 0x02 for B-packets
    buf[1] = 23;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedReportId {
            expected: 0x02,
            got: 0x01
        }
    ));
}

#[test]
fn from_response_wrong_command_echo_rejected() {
    let mut buf = [0u8; B_INPUT_LEN];
    buf[0] = B_REPORT_ID;
    buf[1] = 99;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand {
            expected: 23,
            got: 99
        }
    ));
}

#[test]
fn from_response_length_exceeding_payload_max_rejected() {
    let mut buf = [0u8; B_INPUT_LEN];
    buf[0] = B_REPORT_ID;
    buf[1] = 23;
    // Length = 1014 (one over the 1013 max).
    buf[9] = 0x03;
    buf[10] = 0xF6;
    let err = BCommandPacket::from_response(&buf, 23).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidLength {
            length: 1014,
            max: 1013
        }
    ));
}
