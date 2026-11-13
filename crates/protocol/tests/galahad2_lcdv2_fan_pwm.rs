//! Tests for SetFanPWM (cmd 139).
//!
//! Source: Galahad2VisionDevice.cs:271-281
//! (`writeACommand(139, new byte[] { 0, pwm })`).

use llcontrol_protocol::galahad2_lcdv2::fan_pwm::build_request;
use llcontrol_protocol::galahad2_lcdv2::packets::PACKET_LEN;
use llcontrol_protocol::galahad2_lcdv2::pwm::Pwm;

#[test]
fn build_request_at_50_percent_emits_correct_packet() {
    let bytes = build_request(Pwm::new(50).unwrap());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01, "ReportID");
    assert_eq!(bytes[1], 139, "Command");
    assert_eq!(bytes[5], 0x02, "Length = 2");
    assert_eq!(bytes[6], 0x00, "data[0] = group (always 0)");
    assert_eq!(bytes[7], 0x32, "data[1] = pwm = 50 = 0x32");
    assert!(bytes[8..].iter().all(|&b| b == 0), "rest zero-padded");
}

#[test]
fn build_request_at_zero_percent_emits_zero_pwm_byte() {
    let bytes = build_request(Pwm::new(0).unwrap());
    assert_eq!(bytes[5], 0x02);
    assert_eq!(bytes[6], 0x00);
    assert_eq!(bytes[7], 0x00);
}

#[test]
fn build_request_at_max_pwm_emits_100_byte() {
    let bytes = build_request(Pwm::max());
    assert_eq!(bytes[5], 0x02);
    assert_eq!(bytes[6], 0x00);
    assert_eq!(bytes[7], 100);
}
