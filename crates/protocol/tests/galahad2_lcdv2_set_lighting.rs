//! Tests for SetFanLighting (cmd 133) + SetPumpLighting (cmd 131).
//!
//! Source: Galahad2VisionDevice.cs:269-272, :310-313.

use lconnect_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting, Rgb};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    FanLightingMode, PumpLightingMode, PumpLightingScope,
};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;
use lconnect_protocol::galahad2_lcdv2::set_lighting::{
    build_set_fan_lighting_request, build_set_pump_lighting_request,
};

#[test]
fn build_set_fan_lighting_default() {
    let bytes = build_set_fan_lighting_request(&FanLightingSetting::default());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 133, "Command = SetFanLighting");
    assert_eq!(
        bytes[5], 20,
        "Length = 20 (FanLightingSetting payload size)"
    );
    // First three data bytes: Mode=Rainbow=1, Brightness=2, Speed=2
    assert_eq!(bytes[6], 1);
    assert_eq!(bytes[7], 2);
    assert_eq!(bytes[8], 2);
    assert_eq!(bytes[6 + 19], 24, "NumberOfLED at last data offset");
}

#[test]
fn build_set_fan_lighting_static_red() {
    let s = FanLightingSetting {
        mode: FanLightingMode::StaticColor,
        brightness: 50,
        color1: Rgb::RED,
        ..FanLightingSetting::default()
    };
    let bytes = build_set_fan_lighting_request(&s);
    assert_eq!(bytes[5], 20, "Length = 20");
    assert_eq!(bytes[6], 3, "Mode = StaticColor = 3");
    assert_eq!(bytes[7], 50, "Brightness");
    assert_eq!(bytes[6 + 3], 0xFF, "Color1.R");
    assert_eq!(bytes[6 + 4], 0, "Color1.G");
    assert_eq!(bytes[6 + 5], 0, "Color1.B");
}

#[test]
fn build_set_pump_lighting_default() {
    let bytes = build_set_pump_lighting_request(&PumpLightingSetting::default());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 131, "Command = SetPumpLighting");
    assert_eq!(
        bytes[5], 19,
        "Length = 19 (PumpLightingSetting payload size)"
    );
    assert_eq!(bytes[6], 0, "Scope = Inner = 0");
    assert_eq!(bytes[7], 1, "Mode = Rainbow = 1");
    assert_eq!(bytes[8], 2, "Brightness");
    assert_eq!(bytes[9], 2, "Speed");
}

#[test]
fn build_set_pump_lighting_static_blue_outer_at_max() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::Outer,
        mode: PumpLightingMode::StaticColor,
        brightness: 100,
        color1: Rgb::BLUE,
        ..PumpLightingSetting::default()
    };
    let bytes = build_set_pump_lighting_request(&s);
    assert_eq!(bytes[5], 19);
    assert_eq!(bytes[6], 1, "Scope = Outer = 1");
    assert_eq!(bytes[7], 3, "Mode = StaticColor = 3");
    assert_eq!(bytes[8], 100, "Brightness");
    assert_eq!(bytes[6 + 4], 0, "Color1.R");
    assert_eq!(bytes[6 + 5], 0, "Color1.G");
    assert_eq!(bytes[6 + 6], 0xFF, "Color1.B");
}
