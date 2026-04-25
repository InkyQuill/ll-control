//! Tests for the lighting enums.
//!
//! Wire-format discriminants from FanLightingMode.cs, PumpLightingMode.cs,
//! LightingDirection.cs, ARGBSignalSource.cs, PumpLightingScope.cs.

use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection,
    PumpLightingMode, PumpLightingScope,
};

#[test]
fn fan_lighting_mode_discriminants() {
    assert_eq!(FanLightingMode::Unknown as u8, 0);
    assert_eq!(FanLightingMode::Rainbow as u8, 1);
    assert_eq!(FanLightingMode::RainbowMorph as u8, 2);
    assert_eq!(FanLightingMode::StaticColor as u8, 3);
    assert_eq!(FanLightingMode::Breathing as u8, 4);
    assert_eq!(FanLightingMode::Runway as u8, 5);
    assert_eq!(FanLightingMode::Meteor as u8, 6);
}

#[test]
fn pump_lighting_mode_discriminants() {
    assert_eq!(PumpLightingMode::Unknown as u8, 0);
    assert_eq!(PumpLightingMode::Rainbow as u8, 1);
    assert_eq!(PumpLightingMode::RainbowMorph as u8, 2);
    assert_eq!(PumpLightingMode::StaticColor as u8, 3);
    assert_eq!(PumpLightingMode::Breathing as u8, 4);
    assert_eq!(PumpLightingMode::Runway as u8, 5);
    assert_eq!(PumpLightingMode::Meteor as u8, 6);
    assert_eq!(PumpLightingMode::TickerTape as u8, 7);
    assert_eq!(PumpLightingMode::Fluctuation as u8, 8);
    assert_eq!(PumpLightingMode::Transmit as u8, 9);
    assert_eq!(PumpLightingMode::ColorfulStarryNight as u8, 10);
    assert_eq!(PumpLightingMode::StaticStarryNight as u8, 11);
    assert_eq!(PumpLightingMode::Voice as u8, 12);
    assert_eq!(PumpLightingMode::BigBang as u8, 13);
    assert_eq!(PumpLightingMode::Burst as u8, 14);
    assert_eq!(PumpLightingMode::ColorsMorph as u8, 15);
    assert_eq!(PumpLightingMode::Bounce as u8, 16);
}

#[test]
fn lighting_direction_discriminants() {
    assert_eq!(LightingDirection::RightOrClockwise as u8, 0);
    assert_eq!(LightingDirection::LeftOrCounterClockwise as u8, 1);
    assert_eq!(LightingDirection::Up as u8, 2);
    assert_eq!(LightingDirection::Down as u8, 3);
    assert_eq!(LightingDirection::Spreaded as u8, 4);
    assert_eq!(LightingDirection::Gathered as u8, 5);
}

#[test]
fn argb_signal_source_discriminants() {
    assert_eq!(ArgbSignalSource::Mcu as u8, 0);
    assert_eq!(ArgbSignalSource::Motherboard as u8, 1);
    assert_eq!(ArgbSignalSource::Application as u8, 2);
}

#[test]
fn pump_lighting_scope_discriminants() {
    assert_eq!(PumpLightingScope::Inner as u8, 0);
    assert_eq!(PumpLightingScope::Outer as u8, 1);
    assert_eq!(PumpLightingScope::All as u8, 2);
}

#[test]
fn fan_lighting_mode_from_u8() {
    assert_eq!(FanLightingMode::from_u8(1), FanLightingMode::Rainbow);
    assert_eq!(FanLightingMode::from_u8(0), FanLightingMode::Unknown);
    assert_eq!(FanLightingMode::from_u8(99), FanLightingMode::Unknown);
}

#[test]
fn pump_lighting_mode_from_u8() {
    assert_eq!(PumpLightingMode::from_u8(16), PumpLightingMode::Bounce);
    assert_eq!(PumpLightingMode::from_u8(0), PumpLightingMode::Unknown);
    assert_eq!(PumpLightingMode::from_u8(99), PumpLightingMode::Unknown);
}
