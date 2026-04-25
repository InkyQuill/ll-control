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
