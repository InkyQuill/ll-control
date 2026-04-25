//! Tests for SetPumpPWM (cmd 138).
//!
//! Source: Galahad2VisionDevice.cs:316-322
//! (`writeACommand(138, new byte[] { source, pwm })`).

use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;
use lconnect_protocol::galahad2_lcdv2::pump_pwm::{
    build_request, PumpPwmSource,
};
use lconnect_protocol::galahad2_lcdv2::pwm::Pwm;

#[test]
fn pump_pwm_source_discriminants_match_decompiled_enum() {
    // From PumpPWMSource.cs.
    assert_eq!(PumpPwmSource::Mcu as u8, 0);
    assert_eq!(PumpPwmSource::Motherboard as u8, 1);
}

#[test]
fn build_request_with_mcu_source_at_75_percent() {
    let bytes = build_request(PumpPwmSource::Mcu, Pwm::new(75).unwrap());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[0], 0x01);
    assert_eq!(bytes[1], 138);
    assert_eq!(bytes[5], 0x02, "Length = 2");
    assert_eq!(bytes[6], 0x00, "source = MCU = 0");
    assert_eq!(bytes[7], 75, "pwm = 75");
}

#[test]
fn build_request_with_motherboard_source_at_100_percent() {
    let bytes = build_request(PumpPwmSource::Motherboard, Pwm::max());
    assert_eq!(bytes[5], 0x02);
    assert_eq!(bytes[6], 0x01, "source = Motherboard = 1");
    assert_eq!(bytes[7], 100);
}
