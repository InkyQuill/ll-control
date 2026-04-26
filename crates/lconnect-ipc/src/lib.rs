#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod jsonrpc;
pub mod server;
pub mod trait_def;
pub mod transport_memory;
pub mod types;

pub use error::{IpcError, Result};
pub use trait_def::DaemonRpc;
pub use types::*;

pub const IPC_VERSION_MAJOR: u32 = 1;
pub const IPC_VERSION_MINOR: u32 = 0;
