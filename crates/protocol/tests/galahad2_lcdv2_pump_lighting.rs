//! Tests for PumpLightingSetting::to_bytes.
//!
//! Source: Products/Galahad2Vision/PumpLightingSetting.cs:107-141
//! (`ToBytes` returns 19 bytes — note Scope byte at offset 0, no
//! SyncToPump or NumberOfLED).

use llcontrol_protocol::galahad2_lcdv2::lighting::{PumpLightingSetting, Rgb};
use llcontrol_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, LightingDirection, PumpLightingMode, PumpLightingScope,
};

#[test]
fn pump_lighting_setting_default_matches_csharp() {
    let s = PumpLightingSetting::default();
    assert_eq!(s.scope, PumpLightingScope::Inner);
    assert_eq!(s.mode, PumpLightingMode::Rainbow);
    assert_eq!(s.brightness, 2);
    assert_eq!(s.speed, 2);
    assert_eq!(s.color1, Rgb::BLACK);
    assert_eq!(s.direction, LightingDirection::RightOrClockwise);
    assert!(!s.disabled);
    assert_eq!(s.source, ArgbSignalSource::Mcu);
}

#[test]
fn pump_lighting_setting_to_bytes_default() {
    // Default -> 19 bytes:
    //   [Scope=0 Inner] [Mode=1 Rainbow] [Bright=2] [Speed=2]
    //   [C1..C4 black] [Dir=0] [Disabled=0] [Source=0 Mcu]
    let bytes = PumpLightingSetting::default().to_bytes();
    assert_eq!(bytes.len(), 19);
    assert_eq!(bytes[0], 0, "Scope = Inner");
    assert_eq!(bytes[1], 1, "Mode = Rainbow");
    assert_eq!(bytes[2], 2, "Brightness");
    assert_eq!(bytes[3], 2, "Speed");
    #[expect(
        clippy::needless_range_loop,
        reason = "preserves `{i}` in the per-byte assertion message"
    )]
    for i in 4..16 {
        assert_eq!(bytes[i], 0, "Color RGB byte at offset {i}");
    }
    assert_eq!(bytes[16], 0, "Direction");
    assert_eq!(bytes[17], 0, "Disabled");
    assert_eq!(bytes[18], 0, "Source");
}

#[test]
fn pump_lighting_setting_to_bytes_static_blue_outer_at_max() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::Outer,
        mode: PumpLightingMode::StaticColor,
        brightness: 100,
        speed: 0,
        color1: Rgb::BLUE,
        ..PumpLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 1, "Scope = Outer");
    assert_eq!(bytes[1], 3, "Mode = StaticColor");
    assert_eq!(bytes[2], 100, "Brightness");
    assert_eq!(bytes[3], 0, "Speed = 0 (overridden from default 2)");
    assert_eq!(bytes[4], 0, "Color1.R");
    assert_eq!(bytes[5], 0, "Color1.G");
    assert_eq!(bytes[6], 0xFF, "Color1.B");
}

#[test]
fn pump_lighting_setting_to_bytes_disabled_and_left_direction() {
    // Covers the two adjacent boolean/byte fields at offsets 16 and 17
    // that all the other test cases leave at zero. Catches a field-swap
    // between `direction` and `disabled` and a wrong `bool -> u8` cast.
    let s = PumpLightingSetting {
        disabled: true,
        direction: LightingDirection::LeftOrCounterClockwise,
        ..PumpLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[16], 1, "Direction = LeftOrCounterClockwise -> 1");
    assert_eq!(bytes[17], 1, "Disabled = true -> 1");
}

#[test]
fn pump_lighting_setting_to_bytes_all_scope_with_motherboard_source() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::All,
        source: ArgbSignalSource::Motherboard,
        ..PumpLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 2, "Scope = All → 2");
    assert_eq!(bytes[18], 1, "Source = Motherboard → 1");
}
