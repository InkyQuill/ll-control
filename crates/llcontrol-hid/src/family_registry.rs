//! Per-device-family registry.
//!
//! A `DeviceFamily` declares VID/PID pairs and identity. Phase 1 ships
//! one impl: [`Galahad2LcdV2Family`] (VID `0x0416`, PID `0x7395`). Phase
//! 3+ adds more families by registering additional impls without
//! touching the daemon core.
//!
//! # See also
//!
//! - [`crate::hidapi_backend::HidApiBackend`] — consults the registry to
//!   filter `enumerate` results by known VID/PID
//! - `docs/development/protocol/cross-device-survey.md` section 4.6 — VID-clustering
//!   matrix that drives Phase 3+ family additions

use crate::DeviceFamilyId;
use std::sync::Arc;

/// One device family's identity and VID/PID claims.
pub trait DeviceFamily: Send + Sync {
    /// Stable family slug, e.g. `"galahad2-lcdv2"`.
    fn id(&self) -> DeviceFamilyId;

    /// VID/PID pairs this family claims.
    fn vid_pids(&self) -> &[(u16, u16)];

    /// Marketed name(s) for human-readable output.
    fn display_name(&self) -> &'static str;
}

/// Registered device families consulted by `HidApiBackend::enumerate`.
///
/// Phase 1 default: LCDV2 only (via [`FamilyRegistry::with_default_lcdv2`]).
/// Phase 3+ adds more by calling [`FamilyRegistry::register`] before
/// constructing `HidApiBackend`.
#[derive(Clone)]
pub struct FamilyRegistry {
    families: Vec<Arc<dyn DeviceFamily>>,
}

impl FamilyRegistry {
    /// Create an empty registry. Use `register` to add families, or
    /// [`Self::with_default_lcdv2`] for the Phase 1 default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            families: Vec::new(),
        }
    }

    /// Register a device family.
    pub fn register(&mut self, family: Arc<dyn DeviceFamily>) {
        self.families.push(family);
    }

    /// Look up a family by VID/PID.
    ///
    /// Returns `None` if no registered family claims the pair.
    #[must_use]
    pub fn lookup_vid_pid(&self, vid: u16, pid: u16) -> Option<Arc<dyn DeviceFamily>> {
        self.families
            .iter()
            .find(|f| f.vid_pids().iter().any(|&(v, p)| v == vid && p == pid))
            .cloned()
    }

    /// Phase 1 default registry: only LCDV2 (`0x0416:0x7395`).
    #[must_use]
    pub fn with_default_lcdv2() -> Self {
        let mut r = Self::new();
        r.register(Arc::new(Galahad2LcdV2Family));
        r
    }
}

impl Default for FamilyRegistry {
    fn default() -> Self {
        Self::with_default_lcdv2()
    }
}

/// Galahad II LCDV2 — the Phase 1 device.
pub struct Galahad2LcdV2Family;

impl DeviceFamily for Galahad2LcdV2Family {
    fn id(&self) -> DeviceFamilyId {
        DeviceFamilyId::new("galahad2-lcdv2")
    }
    fn vid_pids(&self) -> &[(u16, u16)] {
        &[(0x0416, 0x7395)]
    }
    fn display_name(&self) -> &'static str {
        "Galahad II LCDV2"
    }
}
