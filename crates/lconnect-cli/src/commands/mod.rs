//! Subcommand implementations.
//!
//! Each submodule is a stub until its corresponding P1E-* task lands.
//! Stubs expose the function signatures `main.rs` dispatches to so the
//! binary builds today; bodies return `anyhow::bail!` until replaced.

pub mod devices;
pub mod lcd;
pub mod lighting;
pub mod maintenance;
pub mod pwm;
pub mod watch;

pub mod cooling;

pub mod thermal;

pub mod theme;

pub mod profiles;
mod windows_control;
mod windows_cooling;

pub mod dynamic_rgb;

pub mod group_archives;

pub mod system_spec;

pub mod display_settings;

pub mod mini;

pub mod fps;

pub mod windows_screen_media;
