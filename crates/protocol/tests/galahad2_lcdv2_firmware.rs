//! Tests for GetFirmwareVersion (cmd 134).
//!
//! Sources:
//! - Wire flow: Galahad2VisionDevice.cs:88-105 (two ACommandPacket reads).
//! - Version-string parser: FirmwareVersion.cs (comma-split into 7 fields).

use lconnect_protocol::galahad2_lcdv2::firmware::{build_request, FirmwareVersion};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

#[test]
fn parse_canonical_version_string() {
    // Realistic example for the LCDV2 hardware:
    let v = FirmwareVersion::parse(
        "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
        "Apr 12 2024 14:33:21",
    )
    .expect("well-formed input");

    assert_eq!(v.chip_code, "N9");
    assert_eq!(v.boot_code, "01");
    assert_eq!(v.kind, "HS");
    assert_eq!(v.model_a, "SQ");
    assert_eq!(v.model_b, "CA_II-Vision");
    assert_eq!(v.software_version, "V2.01.041");
    assert_eq!(v.user_version, "1.6");
    assert_eq!(v.raw_datetime, "Apr 12 2024 14:33:21");
}

#[test]
fn parse_strips_whitespace_and_nul_bytes_around_inputs() {
    // The C# parser does `versionStr.Trim().Trim('\0')` before splitting.
    // We mirror that.
    let v = FirmwareVersion::parse(
        "  N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6 \0",
        "  Apr 12 2024 14:33:21 \0",
    )
    .expect("trimmed input");
    assert_eq!(v.chip_code, "N9");
    assert_eq!(v.user_version, "1.6");
    assert_eq!(v.raw_datetime, "Apr 12 2024 14:33:21");
}

#[test]
fn parse_rejects_too_few_fields() {
    let err = FirmwareVersion::parse("N9,01,HS", "Apr 12 2024 14:33:21").unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::MalformedFirmwareVersion { got_fields: 3 }
    ));
}

#[test]
fn parse_rejects_empty_version() {
    let err = FirmwareVersion::parse("", "Apr 12 2024 14:33:21").unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::MalformedFirmwareVersion { got_fields: 0 }
    ));
}

#[test]
fn build_request_emits_command_134_with_no_payload() {
    let bytes = build_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 134, "Command");
    assert_eq!(bytes[5], 0, "Length");
    assert!(bytes[6..].iter().all(|&b| b == 0), "no payload");
}

#[test]
fn from_two_responses_decodes_full_round_trip() {
    // Synthesise two packets the device would return.
    let mk = |data: &[u8]| {
        let mut buf = [0u8; PACKET_LEN];
        buf[0] = 0x01;
        buf[1] = 134; // command echo
        buf[5] = data.len() as u8;
        buf[6..6 + data.len()].copy_from_slice(data);
        buf
    };
    let buf1 = mk(b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6");
    let buf2 = mk(b"Apr 12 2024 14:33:21");

    let v = FirmwareVersion::from_two_responses(&buf1, &buf2).expect("valid pair");
    assert_eq!(v.user_version, "1.6");
    assert_eq!(v.software_version, "V2.01.041");
    assert_eq!(v.raw_datetime, "Apr 12 2024 14:33:21");
}

#[test]
fn from_two_responses_rejects_wrong_command_echo_on_first_packet() {
    let mut buf1 = [0u8; PACKET_LEN];
    buf1[0] = 0x01;
    buf1[1] = 0x81; // wrong command (handshake echo, not firmware)
    buf1[5] = 0;

    let mut buf2 = [0u8; PACKET_LEN];
    buf2[0] = 0x01;
    buf2[1] = 134;
    buf2[5] = 0;

    let err = FirmwareVersion::from_two_responses(&buf1, &buf2).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand {
            expected: 134,
            got: 0x81
        }
    ));
}
