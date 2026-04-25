//! Tests for LCDControl (cmd 12) and LCDControlSetting.
//!
//! Source: Galahad2VisionDevice.cs:188-193 + LCDControlSetting.cs:7-73 +
//! LCDControlMode.cs + ScreenRotation.cs.

use lconnect_protocol::galahad2_lcdv2::lcd::{
    build_lcd_control_request, LCDControlMode, LCDControlSetting, ScreenRotation,
};
use lconnect_protocol::galahad2_lcdv2::lighting::Rgb;

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
    assert_eq!(s.brightness, 50);
    assert_eq!(s.rotation, ScreenRotation::Rotate0);
    assert!(!s.enable_test);
    assert_eq!(s.test_color, Rgb::BLACK);
    assert_eq!(s.video_fps, 30);
}

#[test]
fn lcd_control_setting_for_brightness_change_matches_service() {
    // Service controller pattern: Mode=LCDSetting (4), VideoFPS=24.
    // Default rotation should pass through via `..Self::default()`.
    let s = LCDControlSetting::for_brightness_change(75);
    assert_eq!(s.mode, LCDControlMode::LCDSetting);
    assert_eq!(s.brightness, 75);
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
    let s = LCDControlSetting {
        mode: LCDControlMode::LCDTest,
        brightness: 100,
        rotation: ScreenRotation::Rotate180,
        enable_test: true,
        test_color: Rgb::RED,
        video_fps: 60,
    };
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
