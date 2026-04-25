//! Tests for LightingInfo<Mode> parser (escape-byte protocol).
//!
//! Source: Products/Galahad2Vision/FanLightingInfo.cs:21-67 and
//! PumpLightingInfo.cs:21-67. The C# parses the data slice with these
//! rules: bytes are mode discriminants until an `0xA5` escape marker is
//! seen; the next byte after `0xA5` is `0x5A` to terminate, or anything
//! else (silently dropped) to clear the escape; after the terminator,
//! two trailer bytes give MaxSpeed and MaxBrightness.

use lconnect_protocol::galahad2_lcdv2::lighting::{
    parse_fan_lighting_info, parse_pump_lighting_info,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{FanLightingMode, PumpLightingMode};

#[test]
fn parse_fan_info_simple_modes_list() {
    // Modes [Rainbow=1, StaticColor=3, Meteor=6], terminator a5 5a,
    // trailer [MaxSpeed=4, MaxBrightness=10].
    let data = [0x01, 0x03, 0x06, 0xA5, 0x5A, 0x04, 0x0A];
    let info = parse_fan_lighting_info(&data).expect("should parse");
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
fn parse_pump_info_full_mode_list() {
    // All 16 known pump modes (1..=16), then terminator + trailer.
    let mut data = vec![];
    for m in 1u8..=16 {
        data.push(m);
    }
    data.extend_from_slice(&[0xA5, 0x5A, 5, 100]);
    let info = parse_pump_lighting_info(&data).expect("should parse");
    assert_eq!(info.modes.len(), 16);
    assert_eq!(info.modes[0], PumpLightingMode::Rainbow);
    assert_eq!(info.modes[15], PumpLightingMode::Bounce);
    assert_eq!(info.max_speed, 5);
    assert_eq!(info.max_brightness, 100);
}

#[test]
fn parse_fan_info_unknown_byte_becomes_unknown() {
    // 0xFF is not a known FanLightingMode discriminant — must map to Unknown.
    let data = [0x01, 0xFF, 0x06, 0xA5, 0x5A, 1, 1];
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(
        info.modes,
        vec![
            FanLightingMode::Rainbow,
            FanLightingMode::Unknown,
            FanLightingMode::Meteor,
        ]
    );
}

#[test]
fn parse_fan_info_a5_then_non_5a_silently_drops_one_byte() {
    // C# bug-compat: `0xA5 X` where X != 0x5A drops X without pushing.
    // After the dropped byte, parsing continues normally.
    let data = [0x01, 0xA5, 0x77, 0x03, 0xA5, 0x5A, 1, 1];
    //          ^Rainbow    ^dropped ^Static  ^terminate
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(
        info.modes,
        vec![FanLightingMode::Rainbow, FanLightingMode::StaticColor,]
    );
}

#[test]
fn parse_fan_info_empty_input_rejected() {
    let err = parse_fan_lighting_info(&[]).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { .. }
    ));
}

#[test]
fn parse_fan_info_no_trailer_rejected() {
    // Mode list and terminator but only ONE trailer byte instead of TWO.
    let data = [0x01, 0xA5, 0x5A, 0x04];
    let err = parse_fan_lighting_info(&data).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { need: 2, got: 1 }
    ));
}

#[test]
fn parse_fan_info_no_terminator_rejected() {
    // Modes but never an A5 terminator at all.
    let data = [0x01, 0x02, 0x03, 0x04];
    let err = parse_fan_lighting_info(&data).unwrap_err();
    // Reaches end of buffer with no terminator — surfaces as too-short
    // because we never enter the "read trailer" stage. `got` must equal
    // the actual buffer length so the diagnostic isn't misleading.
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { need: 2, got: 4 }
    ));
}

#[test]
fn parse_fan_info_terminator_only_yields_zero_modes() {
    // Edge case: the device advertises zero supported modes by sending
    // the terminator immediately, followed by a valid trailer. This is a
    // legitimate parse — the empty-modes Vec is the correct answer.
    let data = [0xA5, 0x5A, 7, 50];
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert!(info.modes.is_empty());
    assert_eq!(info.max_speed, 7);
    assert_eq!(info.max_brightness, 50);
}

#[test]
fn parse_fan_info_a5_then_a5_drops_second_escape() {
    // Escape-of-escape: `0xA5 0xA5` is the bug-compat path applied to the
    // escape byte itself. The first 0xA5 sets escape; the second is "X != 0x5A"
    // and so the escape state clears with no mode pushed. Parsing then needs
    // a *new* 0xA5 0x5A to terminate. Without one the response is malformed.
    //
    // Verify the no-double-terminator case rejects:
    let data = [0x01, 0xA5, 0xA5, 0x5A, 5, 5];
    //          ^Rainbow ^esc1 ^esc-of-esc dropped, then 0x5A is treated as a
    //          mode discriminant (not a terminator) → never terminates.
    let err = parse_fan_lighting_info(&data).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { .. }
    ));

    // And verify that a proper terminator after the escape-of-escape works:
    let data = [0x01, 0xA5, 0xA5, 0x03, 0xA5, 0x5A, 5, 5];
    //          ^R    ^esc1 ^drop  ^Static ^esc2 ^term
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(
        info.modes,
        vec![FanLightingMode::Rainbow, FanLightingMode::StaticColor]
    );
    assert_eq!(info.max_speed, 5);
    assert_eq!(info.max_brightness, 5);
}
