//! Tests for ResetDevice (cmd 142).
//!
//! Source: Galahad2VisionDevice.cs:233-248. Success requires both
//! Length == 1 AND Data[0] == 1.

use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;
use lconnect_protocol::galahad2_lcdv2::reset::{build_request, parse_response};

#[test]
fn build_request_emits_command_142_with_no_payload() {
    let bytes = build_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01);
    assert_eq!(bytes[1], 142);
    assert_eq!(bytes[5], 0, "Length = 0");
    assert!(bytes[6..].iter().all(|&b| b == 0));
}

#[test]
fn parse_response_with_length_1_and_data_0x01_is_success() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 142;
    buf[5] = 1;
    buf[6] = 0x01;
    parse_response(&buf).expect("0x01 => success");
}

#[test]
fn parse_response_with_data_byte_not_0x01_is_data_invalid() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 142;
    buf[5] = 1;
    buf[6] = 0x00; // device returned 0 for some reason
    let err = parse_response(&buf).unwrap_err();
    assert!(matches!(err, lconnect_protocol::Error::DataNotValid));
}

#[test]
fn parse_response_with_wrong_length_is_invalid_length() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 142;
    buf[5] = 2; // unexpected; must be exactly 1
    buf[6] = 0x01;
    buf[7] = 0xAA;
    let err = parse_response(&buf).unwrap_err();
    // We surface this as InvalidLength (with our own max=1), not DataNotValid,
    // because the C# rejects it by length-mismatch *before* checking Data[0].
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidLength { length: 2, max: 1 }
    ));
}
