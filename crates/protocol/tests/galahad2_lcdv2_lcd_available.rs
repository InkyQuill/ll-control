//! Tests for IsLCDAvailable (cmd 23). Source: Galahad2VisionDevice.cs:175-186.

use llcontrol_protocol::galahad2_lcdv2::bpacket::OUTPUT_LEN;
use llcontrol_protocol::galahad2_lcdv2::lcd::{
    build_is_lcd_available_request, parse_is_lcd_available_response,
};

#[test]
fn build_is_lcd_available_emits_cmd_23_no_payload() {
    let bytes = build_is_lcd_available_request();
    assert_eq!(bytes.len(), OUTPUT_LEN);
    assert_eq!(bytes[0], 0x02, "ReportID");
    assert_eq!(bytes[1], 23, "Command");
    assert_eq!(&bytes[2..6], &[0, 0, 0, 0], "DataSize = 0");
    assert_eq!(&bytes[9..11], &[0, 0], "Length = 0");
}

#[test]
fn parse_response_zero_byte_means_available() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 23;
    buf[10] = 1; // Length = 1
    buf[11] = 0x00; // payload[0] = 0 → "available"
    let avail = parse_is_lcd_available_response(&buf).expect("parses");
    assert!(avail, "0x00 means LCD is available");
}

#[test]
fn parse_response_nonzero_byte_means_unavailable() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 23;
    buf[10] = 1;
    buf[11] = 0x01;
    let avail = parse_is_lcd_available_response(&buf).expect("parses");
    assert!(!avail, "0x01 means LCD is not available");
}

#[test]
fn parse_response_zero_length_means_unavailable() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 23;
    buf[10] = 0; // Length = 0 — explicit (no data byte will be read)
    let avail = parse_is_lcd_available_response(&buf).expect("parses");
    assert!(
        !avail,
        "missing data byte → treat as unavailable (matches C#)"
    );
}

#[test]
fn parse_response_too_short_rejected() {
    // Buffer shorter than HEADER_LEN (11) — must reject before any field decode.
    let buf = [0x02_u8, 23];
    let err = parse_is_lcd_available_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::BufferTooShort { .. }
    ));
}

#[test]
fn parse_response_wrong_command_echo_rejected() {
    let mut buf = [0u8; 512];
    buf[0] = 0x02;
    buf[1] = 99;
    let err = parse_is_lcd_available_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::UnexpectedCommand {
            expected: 23,
            got: 99
        }
    ));
}
