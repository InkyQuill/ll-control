//! HID transport abstraction for the lconnect-linux project.
//!
//! Owns the `HidBackend` trait, `Framing` enum, device-info types, and
//! the per-family registry. Two backends ship: `HidApiBackend` (real
//! hidapi-rs) and `MockBackend` (in-process fake for tests).
//!
//! Module bodies land in subsequent P1B-* tasks; this scaffold just
//! establishes the shape.
//!
//! # See also
//!
//! - `docs/superpowers/specs/2026-04-26-phase-1-daemon-design.md` — Phase 1 spec
//! - `docs/protocol/cross-device-survey.md` section 6.1 — framing extension points

#![forbid(unsafe_code)]

pub mod backend;
pub mod device_info;
pub mod family_registry;
pub mod framing;
pub mod hidapi_backend;
pub mod mock_backend;

pub use backend::{matches_echo, HidBackend, OpenHandle};
pub use device_info::{DeviceFamilyId, DeviceInfo};
pub use family_registry::{DeviceFamily, FamilyRegistry, Galahad2LcdV2Family};
pub use framing::Framing;
pub use hidapi_backend::HidApiBackend;
pub use mock_backend::MockBackend;

/// Errors returned by all `lconnect-hid` I/O and framing operations.
///
/// `Error::HidApi` wraps the underlying `hidapi::HidError` directly via
/// `#[from]`. All other variants report transport-level failures detected
/// by this crate (short writes, read timeouts, response framing
/// violations, echo-drain exhaustion).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("hidapi error: {0}")]
    HidApi(#[from] hidapi::HidError),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("short write: {written}/{expected} bytes")]
    ShortWrite { written: usize, expected: usize },
    #[error("read timeout after {ms} ms")]
    ReadTimeout { ms: u64 },
    #[error("response too short: got {got} bytes, expected at least {expected}")]
    ShortResponse { got: usize, expected: usize },
    #[error("response report ID mismatch: got 0x{got:02x}, expected 0x{expected:02x}")]
    ReportIdMismatch { got: u8, expected: u8 },
    #[error("response cmd echo mismatch: got 0x{got:02x}, expected 0x{expected:02x}")]
    CommandEchoMismatch { got: u8, expected: u8 },
    #[error("drained {stale} stale + {timeout} timeout responses without echo of cmd 0x{expected_cmd:02x}")]
    EchoExhausted {
        stale: u32,
        timeout: u32,
        expected_cmd: u8,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
