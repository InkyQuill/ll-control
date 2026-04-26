//! IPC error type + JSON-RPC error code mapping.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IpcError {
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("device busy / IO error: {0}")]
    DeviceBusy(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("confirmation token required")]
    ConfirmationRequired,
    #[error("protocol mismatch: {0}")]
    ProtocolMismatch(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl IpcError {
    #[must_use]
    pub fn jsonrpc_code(&self) -> i32 {
        match self {
            IpcError::DeviceNotFound(_) => -32000,
            IpcError::DeviceBusy(_) => -32001,
            IpcError::InvalidArgument(_) => -32002,
            IpcError::NotImplemented(_) => -32003,
            IpcError::ConfirmationRequired => -32004,
            IpcError::ProtocolMismatch(_) => -32005,
            IpcError::Transport(_) | IpcError::Serialization(_) => -32603,
        }
    }
}

pub type Result<T> = std::result::Result<T, IpcError>;
