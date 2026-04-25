//! Tests for GetFanLightingInfo (cmd 132) + GetPumpLightingInfo (cmd 130).
//!
//! Source: Galahad2VisionDevice.cs:73-86, :122-135.

use lconnect_protocol::galahad2_lcdv2::get_lighting::{
    build_get_fan_lighting_info_request, build_get_pump_lighting_info_request,
    parse_fan_lighting_info_response, parse_pump_lighting_info_response,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{FanLightingMode, PumpLightingMode};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

#[test]
fn build_get_fan_request_emits_cmd_132_no_payload() {
    let bytes = build_get_fan_lighting_info_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 132);
    assert_eq!(bytes[5], 0, "Length = 0");
}

#[test]
fn build_get_pump_request_emits_cmd_130_no_payload() {
    let bytes = build_get_pump_lighting_info_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 130);
    assert_eq!(bytes[5], 0, "Length = 0");
}

#[test]
fn parse_fan_response_full_round_trip() {
    // Synth response: cmd 132 echo, length 7, payload =
    //   [Rainbow=1, StaticColor=3, Meteor=6, A5, 5A, MaxSpeed=4, MaxBri=10]
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 132;
    buf[5] = 7;
    buf[6..13].copy_from_slice(&[0x01, 0x03, 0x06, 0xA5, 0x5A, 0x04, 0x0A]);

    let info = parse_fan_lighting_info_response(&buf).expect("should parse");
    assert_eq!(
        info.modes,
        vec![
            FanLightingMode::Rainbow,
            FanLightingMode::StaticColor,
            FanLightingMode::Meteor,
        ]
    );
    assert_eq!(info.max_speed, 4);
    assert_eq!(info.max_brightness, 10);
}

#[test]
fn parse_pump_response_full_round_trip() {
    // Synth response: cmd 130 echo, length 5, payload =
    //   [Rainbow=1, Bounce=16, A5, 5A, MaxSpeed=5, MaxBri=100]
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 130;
    buf[5] = 6;
    buf[6..12].copy_from_slice(&[0x01, 0x10, 0xA5, 0x5A, 0x05, 0x64]);

    let info = parse_pump_lighting_info_response(&buf).expect("should parse");
    assert_eq!(
        info.modes,
        vec![PumpLightingMode::Rainbow, PumpLightingMode::Bounce,]
    );
    assert_eq!(info.max_speed, 5);
    assert_eq!(info.max_brightness, 100);
}

#[test]
fn parse_fan_response_with_wrong_command_echo_rejected() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 0x86; // 134 — not 132
    buf[5] = 0;
    let err = parse_fan_lighting_info_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand {
            expected: 132,
            got: 0x86
        }
    ));
}
