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
#[non_exhaustive]
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

    /// A firmware version string did not have the expected 7
    /// comma-separated fields.
    #[error("malformed firmware version string: {got_fields} fields, need 7")]
    MalformedFirmwareVersion {
        /// Number of comma-separated fields actually present.
        got_fields: usize,
    },

    /// A PWM percent value was outside the supported 0..=100 range.
    #[error("invalid PWM percent: {0} (must be 0..=100)")]
    InvalidPwmPercent(u8),

    /// A brightness percent value was outside the supported 0..=100 range.
    #[error("invalid brightness percent: {0} (must be 0..=100)")]
    InvalidBrightness(u8),

    /// An LED color array was empty or exceeded the single-packet maximum
    /// for its target path.
    #[error("invalid LED count for {kind}: got {count} (must be 1..={max})")]
    InvalidLedCount {
        /// `"fan"` or `"screen"` to disambiguate which LED path was targeted.
        kind: &'static str,
        /// The caller-provided LED count.
        count: usize,
        /// The single-packet maximum for this LED path.
        max: usize,
    },

    /// A JPEG byte stream was empty or exceeded the codec-enforced
    /// maximum size.
    #[error("invalid JPEG size: got {size} bytes (must be 1..={max})")]
    InvalidJpegSize {
        /// The caller-provided JPEG byte count.
        size: usize,
        /// The codec-enforced maximum.
        max: usize,
    },

    /// An H.264 frame byte stream was empty or exceeded the codec-enforced
    /// maximum size.
    #[error("invalid H.264 frame size: got {size} bytes (must be 1..={max})")]
    InvalidH264FrameSize {
        /// The caller-provided H.264 frame byte count.
        size: usize,
        /// The codec-enforced maximum.
        max: usize,
    },
}

/// Convenience alias for `Result<T, protocol::Error>`.
pub type Result<T> = core::result::Result<T, Error>;
