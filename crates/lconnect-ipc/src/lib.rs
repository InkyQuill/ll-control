#![forbid(unsafe_code)]

pub mod client;
pub mod clip;
pub mod cooling;
pub mod cooling_banks;
pub mod effects;
pub mod fireworks;
pub mod meteor;
pub use fireworks::*;
pub mod snow3d;
pub use meteor::*;
pub use snow3d::*;
pub mod error;
pub mod framing;
pub mod jsonrpc;
pub mod sensors;
pub mod server;
pub mod socket;
pub mod theme;
pub use effects::*;
pub mod thermal;
pub mod trait_def;
pub mod transport_memory;
pub mod types;
pub use theme::*;
pub use thermal::*;

pub use clip::*;
pub use cooling::*;
pub use cooling_banks::*;
pub use error::{IpcError, Result};
pub use sensors::*;
pub use trait_def::DaemonRpc;
pub use types::*;

pub const IPC_VERSION_MAJOR: u32 = 1;
pub const IPC_VERSION_MINOR: u32 = 0;

pub mod profiles;
pub use profiles::*;

pub mod assets;
pub use assets::*;

pub mod dynamic_rgb;
pub use dynamic_rgb::*;

pub mod sensor_defaults;
pub use sensor_defaults::*;

mod font_upload;
pub use font_upload::*;
mod media_upload;
pub use media_upload::*;

pub mod screen_collections;
pub use screen_collections::*;

pub mod screen_reset;
pub use screen_reset::*;

mod media_export;
pub use media_export::*;

pub mod lighting;
pub use lighting::*;

mod group_archive;
pub use group_archive::*;
mod profile_archive;
pub use profile_archive::*;

pub mod group_upload;
pub use group_upload::*;

pub mod group_import;
pub use group_import::*;

pub mod system_spec;
pub use system_spec::*;

pub mod display_settings;
pub use display_settings::*;

pub mod firmware_recovery;
pub use firmware_recovery::*;

pub mod fps_source;
pub use fps_source::*;
