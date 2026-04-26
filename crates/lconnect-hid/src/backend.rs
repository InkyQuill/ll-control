//! `HidBackend` trait.
//!
//! Two impls ship: `HidApiBackend` (production, P1B-3) and `MockBackend`
//! (tests, P1B-4). The daemon takes `Arc<dyn HidBackend>` so swapping
//! backends in tests requires no production-code change.
//!
//! # See also
//!
//! - [`crate::hidapi_backend`] — production impl
//! - [`crate::mock_backend`] — fake for tests

use crate::{DeviceInfo, Framing, Result};

/// Opened device handle. Reads + writes are synchronous (`hidapi-rs` is
/// sync); the daemon wraps each call in `tokio::task::spawn_blocking`.
pub trait OpenHandle: Send + Sync {
    /// Write a HID output report.
    ///
    /// # Errors
    /// Returns `Error::ShortWrite` on partial write or `Error::HidApi`
    /// for backend-level failures.
    fn write(&self, buf: &[u8]) -> Result<usize>;

    /// Read with timeout.
    ///
    /// `timeout_ms` matches `hidapi`'s convention: positive value waits
    /// up to N ms; `0` is non-blocking; `-1` blocks forever.
    ///
    /// # Errors
    /// Returns `Error::HidApi` for backend failures. A 0-byte return is
    /// the timeout signal (not an error).
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
}

/// HID transport backend.
pub trait HidBackend: Send + Sync {
    /// Enumerate devices currently visible to this backend.
    ///
    /// # Errors
    /// Returns `Error::HidApi` on enumeration failures.
    fn enumerate(&self) -> Result<Vec<DeviceInfo>>;

    /// Open a device by its info (typically obtained from `enumerate`).
    ///
    /// # Errors
    /// Returns `Error::DeviceNotFound` if the path no longer resolves,
    /// or `Error::HidApi` for backend-level failures.
    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>>;
}

/// Default echo convention shared by Galahad II Vision: `[ReportID, cmd, ...]`.
///
/// Returns `true` if `buf[0] == framing.report_id()` and `buf[1] == cmd`.
/// This is the convention used by all A/B/C-class responses on LCDV2;
/// other framings (Phase 3+) may need their own helper.
///
/// # Examples
///
/// ```
/// use lconnect_hid::{Framing, backend::matches_echo};
/// let mut buf = [0u8; 64];
/// buf[0] = 0x02;
/// buf[1] = 0x86;
/// assert!(matches_echo(&buf, Framing::B, 0x86));
/// assert!(!matches_echo(&buf, Framing::A, 0x86)); // ReportID mismatch
/// assert!(!matches_echo(&buf, Framing::B, 0x99)); // cmd mismatch
/// ```
#[must_use]
pub fn matches_echo(buf: &[u8], framing: Framing, cmd: u8) -> bool {
    buf.len() >= 2 && buf[0] == framing.report_id() && buf[1] == cmd
}
