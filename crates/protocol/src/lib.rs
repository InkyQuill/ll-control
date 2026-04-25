//! Pure protocol types and codecs for Lian Li L-Connect devices.
//!
//! No I/O, no async runtime, no hidapi. Bytes in, bytes out. Hardware
//! interaction lives in higher-level crates (`hid`, `daemon`, the
//! Phase-0 `verify-galahad` harness).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod common;
pub mod galahad2_lcdv2;

/// Errors produced by the protocol crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A response or input buffer was too short for the operation.
    #[error("buffer too short: need {need} bytes, got {got}")]
    BufferTooShort {
        /// Required byte count.
        need: usize,
        /// Actual byte count.
        got: usize,
    },

    /// The HID Report ID byte was not the expected value.
    #[error("unexpected report id: expected {expected:#04x}, got {got:#04x}")]
    UnexpectedReportId {
        /// Expected report id.
        expected: u8,
        /// Actual report id.
        got: u8,
    },

    /// The command echo byte did not match the request command.
    #[error("unexpected command echo: expected {expected:#04x}, got {got:#04x}")]
    UnexpectedCommand {
        /// Expected command byte.
        expected: u8,
        /// Actual command byte.
        got: u8,
    },

    /// A response payload's `Length` field claimed more bytes than fit.
    #[error("invalid length field: {length} (max {max})")]
    InvalidLength {
        /// The bogus length value.
        length: usize,
        /// The maximum allowed.
        max: usize,
    },

    /// A response indicated its data was not valid (device-side flag).
    #[error("device reported response data invalid")]
    DataNotValid,
}

/// Convenience alias for `Result<T, protocol::Error>`.
pub type Result<T> = core::result::Result<T, Error>;
