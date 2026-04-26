//! Real hidapi-rs backend for production use.
//!
//! Hardware-required: cannot be unit-tested without an attached device.
//! Tests live in [`MockBackend`](crate::mock_backend) (P1B-4) and the
//! daemon's actor smoke tests use the mock; live device runs are P1H-1.
//!
//! # Caveats
//!
//! `hidapi::HidApi::new()` is heavy (it probes the entire USB tree);
//! construct once per process and share via `Arc`. `HidApiBackend`
//! holds the singleton internally; cloning the wrapping `Arc<dyn
//! HidBackend>` is cheap.
//!
//! Phase 1 hardcodes the VID/PID filter to LCDV2 (`0x0416:0x7395`).
//! P1B-5 replaces the hardcode with a `FamilyRegistry::lookup_vid_pid`
//! consultation so Phase 3+ can register additional families without
//! touching this file.
//!
//! # See also
//!
//! - [`crate::backend::HidBackend`] — the trait this implements
//! - [`crate::mock_backend::MockBackend`] — fake for tests

use crate::{DeviceFamilyId, DeviceInfo, Error, HidBackend, OpenHandle, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Production HID backend.
///
/// Wraps a single `hidapi::HidApi` instance behind an `Arc<Mutex<...>>`
/// so the daemon can clone the backend handle freely. Constructing
/// `HidApiBackend` is the trigger for the heavy `HidApi::new()`
/// probe; do it once per process.
#[derive(Clone)]
pub struct HidApiBackend {
    api: Arc<Mutex<hidapi::HidApi>>,
}

impl HidApiBackend {
    /// Create a new backend instance.
    ///
    /// # Errors
    ///
    /// Returns `Error::HidApi` if `hidapi::HidApi::new()` fails (typically
    /// missing udev permissions on Linux — see the project's
    /// `99-lian-li-galahad2.rules` udev rule).
    pub fn new() -> Result<Self> {
        let api = hidapi::HidApi::new()?;
        Ok(Self {
            api: Arc::new(Mutex::new(api)),
        })
    }
}

impl HidBackend for HidApiBackend {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        let mut api = self.api.lock().expect("hidapi mutex poisoned");
        api.refresh_devices()?;
        let mut out = Vec::new();
        for di in api.device_list() {
            // Phase 1: hardcoded LCDV2 filter. P1B-5 swaps in FamilyRegistry.
            if di.vendor_id() == 0x0416 && di.product_id() == 0x7395 {
                out.push(DeviceInfo {
                    family: DeviceFamilyId::new("galahad2-lcdv2"),
                    vid: di.vendor_id(),
                    pid: di.product_id(),
                    serial: di.serial_number().map(str::to_owned),
                    path: PathBuf::from(di.path().to_string_lossy().into_owned()),
                });
            }
        }
        Ok(out)
    }

    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>> {
        let api = self.api.lock().expect("hidapi mutex poisoned");
        // hidapi-rs needs a CString-equivalent path; convert lossily.
        let path_str = info.path.to_string_lossy();
        let path_cstring = std::ffi::CString::new(path_str.as_ref())
            .map_err(|_| Error::DeviceNotFound(format!("{path_str} (NUL in path)")))?;
        let dev = api.open_path(&path_cstring)?;
        Ok(Box::new(HidApiHandle {
            inner: Arc::new(Mutex::new(dev)),
        }))
    }
}

/// Wrapper around `hidapi::HidDevice` that provides `Send + Sync` via
/// `Arc<Mutex<...>>`. The daemon's per-device `hid_io` task is the
/// single consumer; the mutex only contends if a future architecture
/// adds concurrent readers (none planned for Phase 1).
struct HidApiHandle {
    inner: Arc<Mutex<hidapi::HidDevice>>,
}

impl OpenHandle for HidApiHandle {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let dev = self.inner.lock().expect("hidapi handle mutex poisoned");
        let n = dev.write(buf)?;
        if n != buf.len() {
            return Err(Error::ShortWrite {
                written: n,
                expected: buf.len(),
            });
        }
        Ok(n)
    }

    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let dev = self.inner.lock().expect("hidapi handle mutex poisoned");
        Ok(dev.read_timeout(buf, timeout_ms)?)
    }
}
