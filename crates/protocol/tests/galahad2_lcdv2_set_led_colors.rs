//! Tests for SetLEDColors (cmd 20). Source: Galahad2VisionDevice.cs:283-308.

use lconnect_protocol::galahad2_lcdv2::lighting::Rgb;
use lconnect_protocol::galahad2_lcdv2::set_led_colors::{
    build_set_fan_led_colors_request, build_set_screen_led_colors_request, fan_led_payload,
    screen_led_payload,
};

#[test]
fn fan_payload_one_red_led() {
    let payload = fan_led_payload(&[Rgb::RED]);
    // [0x01 (isFanLed=true)] [0xFF, 0x00, 0x00 (R, G, B)]
    assert_eq!(payload, vec![0x01, 0xFF, 0x00, 0x00]);
}

#[test]
fn fan_payload_three_colors() {
    let payload = fan_led_payload(&[Rgb::RED, Rgb::GREEN, Rgb::BLUE]);
    assert_eq!(payload[0], 0x01);
    assert_eq!(&payload[1..4], &[0xFF, 0x00, 0x00]);
    assert_eq!(&payload[4..7], &[0x00, 0xFF, 0x00]);
    assert_eq!(&payload[7..10], &[0x00, 0x00, 0xFF]);
    assert_eq!(payload.len(), 1 + 3 * 3);
}

#[test]
fn screen_payload_one_blue_led_has_24_byte_pad_and_then_rgb() {
    let payload = screen_led_payload(&[Rgb::BLUE]);
    // size = (1 + 8) * 3 + 1 = 28 bytes
    assert_eq!(payload.len(), 28);
    assert_eq!(payload[0], 0x00, "isFanLed=false");
    assert!(payload[1..25].iter().all(|&b| b == 0), "24-byte zero pad");
    assert_eq!(&payload[25..28], &[0x00, 0x00, 0xFF], "RGB at offset 25");
}

#[test]
fn screen_payload_uniform_white_8_leds_size_is_correct() {
    let colors = vec![Rgb::WHITE; 8];
    let payload = screen_led_payload(&colors);
    // (8 + 8) * 3 + 1 = 49 bytes
    assert_eq!(payload.len(), 49);
    assert_eq!(payload[0], 0x00);
    // First 24 bytes after index 0 are zero
    assert!(payload[1..25].iter().all(|&b| b == 0));
    // Then 8 RGB triples of FF FF FF
    assert!(payload[25..49].iter().all(|&b| b == 0xFF));
}

#[test]
fn build_set_screen_led_request_emits_cmd_20_chunked_correctly() {
    let colors = vec![Rgb::RED; 16]; // 16 LEDs, screen path
    let pkts = build_set_screen_led_colors_request(&colors);
    let expected_len = (16 + 8) * 3 + 1; // 73 bytes — one packet
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[1], 20, "Command = SetLEDColors");
    assert_eq!(
        u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
        expected_len as u32
    );
    assert_eq!(
        u16::from_be_bytes([bytes[9], bytes[10]]) as usize,
        expected_len
    );
    assert_eq!(bytes[11], 0x00, "isFanLed=false");
    // RGB[0] starts at offset 11 + 25 = 36
    assert_eq!(&bytes[36..39], &[0xFF, 0, 0]);
}

#[test]
fn build_set_fan_led_request_emits_cmd_20_packets() {
    let colors = vec![Rgb::GREEN; 24];
    let pkts = build_set_fan_led_colors_request(&colors);
    let expected_len = 24 * 3 + 1; // 73 bytes
    assert_eq!(pkts.len(), 1);
    let bytes = pkts[0];
    assert_eq!(bytes[1], 20);
    assert_eq!(
        u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
        expected_len as u32
    );
    assert_eq!(
        u16::from_be_bytes([bytes[9], bytes[10]]) as usize,
        expected_len
    );
    assert_eq!(bytes[11], 0x01, "isFanLed=true");
    assert_eq!(&bytes[12..15], &[0x00, 0xFF, 0x00]);
}
