//! Galahad II LCDV2 (`0x0416:0x7395`) protocol.
//!
//! Decompiled-source references are in
//! `docs/protocol/decompiled-source-map.md`.
//!
//! USB / HID identity:
//! - Vendor ID:  `0x0416`  (Winbond / Lian Li)
//! - Product ID: `0x7395`  (Galahad II LCDV2)
//! - Firmware:   `1.6` (only verified revision in Phase 0a)
//!
//! Submodules are added incrementally as commands are verified.

pub mod packets;

pub mod commands;

pub mod handshake;

pub mod firmware;

pub mod pwm;

pub mod fan_pwm;

pub mod pump_pwm;

pub mod reset;

pub mod lighting_enums;

pub mod lighting;

pub mod get_lighting;

pub mod set_lighting;

pub mod bpacket;

pub mod cpacket;

pub mod lcd;

pub mod set_led_colors;

pub mod jpeg;

pub mod h264;

pub use lighting::Rgb;
