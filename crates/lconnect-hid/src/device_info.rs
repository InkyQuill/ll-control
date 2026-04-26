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
pub struct DeviceFamilyId(pub String);

impl DeviceFamilyId {
    #[must_use]
    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Per-device info populated from `hidapi` enumeration.
#[derive(Debug, Clone)]
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
