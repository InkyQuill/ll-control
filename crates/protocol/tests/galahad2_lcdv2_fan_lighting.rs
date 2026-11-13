//! Tests for Rgb color and FanLightingSetting.
//!
//! Source: Products/Galahad2Vision/FanLightingSetting.cs:117-150
//! (`ToBytes` returns 20 bytes).

use llcontrol_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, Rgb};
use llcontrol_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection,
};

#[test]
fn rgb_constructs_with_named_components() {
    let c = Rgb {
        r: 0x10,
        g: 0x20,
        b: 0x30,
    };
    assert_eq!(c.r, 0x10);
    assert_eq!(c.g, 0x20);
    assert_eq!(c.b, 0x30);
}

#[test]
fn rgb_const_helpers() {
    assert_eq!(Rgb::BLACK, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        Rgb::WHITE,
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF
        }
    );
    assert_eq!(
        Rgb::RED,
        Rgb {
            r: 0xFF,
            g: 0,
            b: 0
        }
    );
    assert_eq!(
        Rgb::GREEN,
        Rgb {
            r: 0,
            g: 0xFF,
            b: 0
        }
    );
    assert_eq!(
        Rgb::BLUE,
        Rgb {
            r: 0,
            g: 0,
            b: 0xFF
        }
    );
}

#[test]
fn fan_lighting_setting_default_matches_csharp() {
    // C# constructor sets Mode=Rainbow, Brightness=2, Speed=2,
    // Colors=BLACK, Direction=RightOrClockwise, Disabled=false,
    // Source=MCU, SyncToPump=false, NumberOfLED=24.
    let s = FanLightingSetting::default();
    assert_eq!(s.mode, FanLightingMode::Rainbow);
    assert_eq!(s.brightness, 2);
    assert_eq!(s.speed, 2);
    assert_eq!(s.color1, Rgb::BLACK);
    assert_eq!(s.color2, Rgb::BLACK);
    assert_eq!(s.color3, Rgb::BLACK);
    assert_eq!(s.color4, Rgb::BLACK);
    assert_eq!(s.direction, LightingDirection::RightOrClockwise);
    assert!(!s.disabled);
    assert_eq!(s.source, ArgbSignalSource::Mcu);
    assert!(!s.sync_to_pump);
    assert_eq!(s.number_of_led, 24);
}

#[test]
fn fan_lighting_setting_to_bytes_default() {
    let bytes = FanLightingSetting::default().to_bytes();
    assert_eq!(bytes.len(), 20);
    assert_eq!(bytes[0], 1, "Mode");
    assert_eq!(bytes[1], 2, "Brightness");
    assert_eq!(bytes[2], 2, "Speed");
    #[expect(
        clippy::needless_range_loop,
        reason = "preserves `{i}` in the per-byte assertion message"
    )]
    for i in 3..15 {
        assert_eq!(bytes[i], 0, "Color RGB byte at offset {i}");
    }
    assert_eq!(bytes[15], 0, "Direction");
    assert_eq!(bytes[16], 0, "Disabled");
    assert_eq!(bytes[17], 0, "Source");
    assert_eq!(bytes[18], 0, "SyncToPump");
    assert_eq!(bytes[19], 24, "NumberOfLED");
}

#[test]
fn fan_lighting_setting_to_bytes_static_red_at_50_percent() {
    let s = FanLightingSetting {
        mode: FanLightingMode::StaticColor,
        brightness: 50,
        speed: 0,
        color1: Rgb::RED,
        color2: Rgb::BLACK,
        color3: Rgb::BLACK,
        color4: Rgb::BLACK,
        direction: LightingDirection::RightOrClockwise,
        disabled: false,
        source: ArgbSignalSource::Mcu,
        sync_to_pump: false,
        number_of_led: 24,
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 3, "Mode = StaticColor");
    assert_eq!(bytes[1], 50, "Brightness");
    assert_eq!(bytes[3], 0xFF, "Color1.R");
    assert_eq!(bytes[4], 0, "Color1.G");
    assert_eq!(bytes[5], 0, "Color1.B");
    assert_eq!(bytes[19], 24, "NumberOfLED");
}

#[test]
fn fan_lighting_setting_to_bytes_disabled_and_app_source() {
    let s = FanLightingSetting {
        disabled: true,
        source: ArgbSignalSource::Application,
        ..FanLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[16], 1, "Disabled = true → 1");
    assert_eq!(bytes[17], 2, "Source = Application → 2");
}
