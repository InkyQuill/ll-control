//! Golden-byte tests for ACommandPacket.

use lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket;

#[test]
fn empty_request_is_64_bytes_of_header_only() {
    let packet = ACommandPacket::new_request(0x81, &[]);
    let bytes = packet.as_bytes();

    assert_eq!(bytes.len(), 64);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 0x81, "Command echo (129 = GetHandshakeInfo)");
    assert_eq!(bytes[2], 0x00, "reserved");
    assert_eq!(bytes[3], 0x00, "PacketNumber high");
    assert_eq!(bytes[4], 0x00, "PacketNumber low");
    assert_eq!(bytes[5], 0x00, "Length = 0 for empty request");
    assert!(bytes[6..].iter().all(|&b| b == 0), "payload zero-filled");
}

#[test]
fn request_with_two_data_bytes_sets_length_and_payload() {
    // Mirrors `writeACommand(139, new byte[]{0, pwm})` from
    // Galahad2VisionDevice.cs:280 (SetFanPWM).
    let packet = ACommandPacket::new_request(0x8B /* 139 */, &[0x00, 0x42]);
    let bytes = packet.as_bytes();

    assert_eq!(bytes.len(), 64);
    assert_eq!(bytes[1], 0x8B);
    assert_eq!(bytes[5], 0x02, "Length = 2");
    assert_eq!(bytes[6], 0x00, "data[0] = group");
    assert_eq!(bytes[7], 0x42, "data[1] = pwm = 0x42");
    assert!(bytes[8..].iter().all(|&b| b == 0));
}

#[test]
fn request_clamps_data_to_58_bytes() {
    let huge = [0xAAu8; 100];
    let packet = ACommandPacket::new_request(0x10, &huge);
    let bytes = packet.as_bytes();

    assert_eq!(bytes[5], 58, "Length saturates at MAX_PAYLOAD");
    assert!(bytes[6..64].iter().all(|&b| b == 0xAA));
}

#[test]
fn parse_response_extracts_header_and_data_slice() {
    // Synthetic GetHandshakeInfo response.
    // Header: ReportID=1, Cmd=129, reserved=0, PacketNumber=0, Length=7.
    // Data:   FanRpm=1234 (BE 0x04 0xD2), PumpRpm=2980 (BE 0x0B 0xA4),
    //         TempValid=1, TempInteger=42, TempFraction=7.
    let mut buf = [0u8; 64];
    buf[0] = 0x01; // ReportID
    buf[1] = 0x81; // Command echo (129)
    buf[5] = 0x07; // Length
    buf[6] = 0x04;
    buf[7] = 0xD2;
    buf[8] = 0x0B;
    buf[9] = 0xA4;
    buf[10] = 0x01;
    buf[11] = 0x2A;
    buf[12] = 0x07;

    let parsed = ACommandPacket::parse_response(&buf, 0x81)
        .expect("valid response should parse");
    assert_eq!(parsed.command(), 0x81);
    assert_eq!(parsed.length(), 7);
    assert_eq!(parsed.data(), &[0x04, 0xD2, 0x0B, 0xA4, 0x01, 0x2A, 0x07]);
}

#[test]
fn parse_response_rejects_wrong_report_id() {
    let mut buf = [0u8; 64];
    buf[0] = 0xFF;   // wrong
    buf[1] = 0x81;
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedReportId { expected: 0x01, got: 0xFF }
    ));
}

#[test]
fn parse_response_rejects_wrong_command_echo() {
    let mut buf = [0u8; 64];
    buf[0] = 0x01;
    buf[1] = 0x86; // 134, not what we asked for
    buf[5] = 0;
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand { expected: 0x81, got: 0x86 }
    ));
}

#[test]
fn parse_response_rejects_short_buffer() {
    let buf = [0u8; 5];
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(err, lconnect_protocol::Error::BufferTooShort { .. }));
}

#[test]
fn parse_response_rejects_oversize_length_field() {
    let mut buf = [0u8; 64];
    buf[0] = 0x01;
    buf[1] = 0x81;
    buf[5] = 0xFF; // claims 255 bytes; max is 58
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::InvalidLength { length: 0xFF, max: 58 }
    ));
}

#[test]
fn parse_response_rejects_truncated_data_section() {
    // Buffer is exactly HEADER_LEN bytes (6) but advertises length=5,
    // so the data section [6..11] would be out-of-bounds.
    // Validates the second BufferTooShort branch (the post-length-field
    // bounds check).
    let buf = [0x01u8, 0x81, 0x00, 0x00, 0x00, 0x05];
    let err = ACommandPacket::parse_response(&buf, 0x81).unwrap_err();
    assert!(
        matches!(
            err,
            lconnect_protocol::Error::BufferTooShort { need: 11, got: 6 }
        ),
        "got {err:?}"
    );
}
