//! Tests for GetHandshakeInfo (cmd 129).
//!
//! Source: Products/Galahad2Vision/HandshakeInfo.cs:58-76.

use lconnect_protocol::galahad2_lcdv2::handshake::{HandshakeInfo, build_request};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

#[test]
fn build_request_emits_a_64_byte_command_129_packet() {
    let bytes = build_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 129, "Command");
    assert_eq!(bytes[5], 0, "Length = 0");
    assert!(bytes[6..].iter().all(|&b| b == 0), "no payload");
}

#[test]
fn parse_known_response_yields_expected_values() {
    // Synthetic well-formed response: fan=1234, pump=2980,
    // temp_valid=1, temp_int=42, temp_frac=7.  Expect 42.7°C.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01; // ReportID
    buf[1] = 129;  // command echo
    buf[5] = 7;    // length
    buf[6] = 0x04;
    buf[7] = 0xD2; // 1234
    buf[8] = 0x0B;
    buf[9] = 0xA4; // 2980
    buf[10] = 1;   // temp valid
    buf[11] = 42;
    buf[12] = 7;

    let info = HandshakeInfo::from_response(&buf).expect("should parse");
    assert_eq!(info.fan_rpm, 1234);
    assert_eq!(info.pump_rpm, 2980);
    assert!(info.temp_valid);
    assert_eq!(info.temp_integer, 42);
    assert_eq!(info.temp_fraction, 7);
    let c = info.temperature_c();
    assert!((c - 42.7).abs() < 1e-6, "got {}", c);
}

#[test]
fn parse_response_with_invalid_temp_flag_still_decodes_rpms() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 7;
    buf[6] = 0x00;
    buf[7] = 0x32; // fan = 50
    buf[8] = 0x00;
    buf[9] = 0xC8; // pump = 200
    buf[10] = 0;   // temp NOT valid
    buf[11] = 99;
    buf[12] = 99;

    let info = HandshakeInfo::from_response(&buf).expect("still parses");
    assert_eq!(info.fan_rpm, 50);
    assert_eq!(info.pump_rpm, 200);
    assert!(!info.temp_valid);
    // We still expose the raw bytes; callers can ignore them.
    assert_eq!(info.temp_integer, 99);
}

#[test]
fn parse_response_rejects_too_short_data() {
    // Length field claims < 7 bytes — handshake needs 7.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 6; // one byte short
    let err = HandshakeInfo::from_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { need: 7, got: 6 }
    ));
}

#[test]
fn temp_fraction_modulo_10() {
    // C# does `TemperatureFraction % 10` to compute the displayed °C.
    // We mirror that in `temperature_c()`.
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 129;
    buf[5] = 7;
    buf[10] = 1;
    buf[11] = 30;
    buf[12] = 17; // 17 % 10 = 7

    let info = HandshakeInfo::from_response(&buf).unwrap();
    assert_eq!(info.temp_fraction, 17, "raw byte preserved");
    assert!(
        (info.temperature_c() - 30.7).abs() < 1e-6,
        "modulo applied in display"
    );
}
