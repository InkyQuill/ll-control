//! lconnect-daemon library surface.
//!
//! The crate ships both a binary (`lconnectd`, see `src/main.rs`) and a
//! library so integration tests can exercise internal modules directly.
//!
//! Modules are pre-declared here so that follow-up tasks can fill them in
//! parallel without contention on this file.

#![forbid(unsafe_code)]

pub mod actor;
pub mod admission;
pub mod builtin_media;
mod codec_str;
pub mod config;
pub mod firmware_operation;
pub mod ipc_server;
pub mod manager;
pub mod persistence;
pub mod power;
pub mod stats;

pub mod sensors;

pub mod theme_assets;

pub mod theme_runtime;

pub mod profiles;

pub mod sensor_defaults;

pub mod sensor_selection;

pub mod theme_fonts;

pub mod font_upload;

pub mod media_assets;

pub mod media_file;

pub mod media_reader;
pub mod media_upload;

pub mod media_composition;

mod state_transaction;

pub mod screen_collections;

pub mod media_export;

pub mod profile_archive;

pub mod archive_transfer;
pub mod group_exports;

pub mod profile_group;

pub mod group_uploads;

pub(crate) mod asset_publication;

mod control_reset;

pub mod system_spec;

pub mod display_settings;

pub mod fps_source;

/// Windows control preparation into target-preserving Linux groups.
pub mod windows_control;

pub mod screen_editor;

#[cfg(test)]
#[path = "../../lconnect-profile/tests/support/screen_plan.rs"]
pub(crate) mod windows_screen_fixture;

mod editor_cache;

mod editor_video;
