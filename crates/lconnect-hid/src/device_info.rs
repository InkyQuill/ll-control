//! Device identity types.
//!
//! `DeviceFamilyId` is a stable string slug per device family
//! ("galahad2-lcdv2", future "hydroshift-ii-lcd", etc.). `DeviceInfo`
//! is what the daemon hands out via its IPC `DeviceDescriptor`.
//!
//! # See also
//!
//! - [`crate::family_registry`] — VID/PID → `DeviceFamilyId` lookup

use std::path::PathBuf;

/// Stable family slug. Used in opaque `DeviceId`s and persistence keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceFamilyId(String);

impl DeviceFamilyId {
    /// Construct a slug from any string-convertible source.
    #[must_use]
    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }

    /// Return the slug as a `&str` for serialisation / logging.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Per-device info populated from `hidapi` enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeviceInfo {
    /// Family slug ("galahad2-lcdv2", etc.).
    pub family: DeviceFamilyId,
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// Serial number reported by hidapi (may be `None`).
    pub serial: Option<String>,
    /// Linux hidraw path (`/dev/hidraw5`) or platform equivalent.
    pub path: PathBuf,
}

impl DeviceInfo {
    /// Construct a `DeviceInfo`. Required because the struct is
    /// `#[non_exhaustive]`, so external crates (notably integration
    /// tests and the daemon) cannot use struct-literal syntax.
    ///
    /// # Examples
    ///
    /// ```
    /// use lconnect_hid::{DeviceFamilyId, DeviceInfo};
    /// use std::path::PathBuf;
    ///
    /// let info = DeviceInfo::new(
    ///     DeviceFamilyId::new("galahad2-lcdv2"),
    ///     0x0416,
    ///     0x7395,
    ///     None,
    ///     PathBuf::from("/dev/hidraw0"),
    /// );
    /// assert_eq!(info.vid, 0x0416);
    /// ```
    #[must_use]
    pub fn new(
        family: DeviceFamilyId,
        vid: u16,
        pid: u16,
        serial: Option<String>,
        path: PathBuf,
    ) -> Self {
        Self {
            family,
            vid,
            pid,
            serial,
            path,
        }
    }
}
