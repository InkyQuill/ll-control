//! Tests for LCDControl (cmd 12) and LCDControlSetting.
//!
//! Source: Galahad2VisionDevice.cs:188-193 + LCDControlSetting.cs:7-73 +
//! LCDControlMode.cs + ScreenRotation.cs.

use llcontrol_protocol::galahad2_lcdv2::lcd::{
    build_lcd_control_request, Brightness, LCDControlMode, LCDControlSetting, ScreenRotation,
};
use llcontrol_protocol::galahad2_lcdv2::lighting::Rgb;

#[test]
fn lcd_control_mode_discriminants() {
    assert_eq!(LCDControlMode::LocalUI as u8, 0);
    assert_eq!(LCDControlMode::Application as u8, 1);
    assert_eq!(LCDControlMode::LocalH264 as u8, 2);
    assert_eq!(LCDControlMode::LocalAVI as u8, 3);
    assert_eq!(LCDControlMode::LCDSetting as u8, 4);
    assert_eq!(LCDControlMode::LCDTest as u8, 5);
}

#[test]
fn screen_rotation_discriminants() {
    assert_eq!(ScreenRotation::Rotate0 as u8, 0);
    assert_eq!(ScreenRotation::Rotate90 as u8, 1);
    assert_eq!(ScreenRotation::Rotate180 as u8, 2);
    assert_eq!(ScreenRotation::Rotate270 as u8, 3);
}

#[test]
fn lcd_control_setting_default_matches_csharp() {
    let s = LCDControlSetting::default();
    assert_eq!(s.mode, LCDControlMode::LocalUI);
    assert_eq!(s.brightness, Brightness::default());
    assert_eq!(s.brightness.percent(), 50);
    assert_eq!(s.rotation, ScreenRotation::Rotate0);
    assert!(!s.enable_test);
    assert_eq!(s.test_color, Rgb::BLACK);
    assert_eq!(s.video_fps, 30);
}

#[test]
fn lcd_control_setting_for_brightness_change_matches_service() {
    // Service controller pattern: Mode=LCDSetting (4), VideoFPS=24.
    // Default rotation should pass through via `..Self::default()`.
    let s = LCDControlSetting::for_brightness_change(Brightness::new(75).unwrap());
    assert_eq!(s.mode, LCDControlMode::LCDSetting);
    assert_eq!(s.brightness, Brightness::new(75).unwrap());
    assert_eq!(s.rotation, ScreenRotation::Rotate0);
    assert_eq!(s.video_fps, 24);
}

#[test]
fn lcd_control_setting_to_bytes_default() {
    let bytes = LCDControlSetting::default().to_bytes();
    assert_eq!(bytes, [0u8, 50, 0, 0, 0, 0, 0, 30]);
}

#[test]
fn lcd_control_setting_to_bytes_test_pattern_red() {
    // `LCDControlSetting` is `#[non_exhaustive]`, so external crates
    // (this integration test) cannot use struct-literal construction;
    // start from `default()` and assign fields.
    let mut s = LCDControlSetting::default();
    s.mode = LCDControlMode::LCDTest;
    s.brightness = Brightness::max();
    s.rotation = ScreenRotation::Rotate180;
    s.enable_test = true;
    s.test_color = Rgb::RED;
    s.video_fps = 60;
    assert_eq!(s.to_bytes(), [5, 100, 2, 1, 0xFF, 0, 0, 60]);
}

#[test]
fn build_lcd_control_request_emits_cmd_12_with_8_byte_payload() {
    let bytes = build_lcd_control_request(LCDControlSetting::default());
    assert_eq!(bytes[0], 0x02, "ReportID");
    assert_eq!(bytes[1], 12, "Command = LCDControl");
    assert_eq!(&bytes[2..6], &[0, 0, 0, 8], "DataSize = 8");
    assert_eq!(&bytes[9..11], &[0, 8], "Length = 8");
    assert_eq!(&bytes[11..19], &[0, 50, 0, 0, 0, 0, 0, 30]);
}

#[test]
fn brightness_new_accepts_0_through_100() {
    assert!(Brightness::new(0).is_ok());
    assert!(Brightness::new(50).is_ok());
    assert!(Brightness::new(100).is_ok());
}

#[test]
fn brightness_new_rejects_over_100() {
    let err = Brightness::new(101).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::InvalidBrightness(101)
    ));
    let err = Brightness::new(255).unwrap_err();
    assert!(matches!(
        err,
        llcontrol_protocol::Error::InvalidBrightness(255)
    ));
}

#[test]
fn brightness_max_is_100() {
    assert_eq!(Brightness::max().percent(), 100);
}

#[test]
fn brightness_default_is_50() {
    assert_eq!(Brightness::default().percent(), 50);
}

#[test]
fn lcd_control_setting_to_bytes_rotate90_at_offset_2() {
    // Rotate90 has wire byte 1, NOT 90. Catches a regression where the
    // discriminant gets confused with the angle.
    let mut s = LCDControlSetting::default();
    s.rotation = ScreenRotation::Rotate90;
    let bytes = s.to_bytes();
    assert_eq!(bytes[2], 1, "Rotate90 -> wire byte 1 (NOT 90)");
}

#[test]
fn lcd_control_setting_to_bytes_rotate270() {
    let mut s = LCDControlSetting::default();
    s.rotation = ScreenRotation::Rotate270;
    let bytes = s.to_bytes();
    assert_eq!(bytes[2], 3, "Rotate270 -> wire byte 3 (NOT 270)");
}
