//! In-process mock backend for hardware-free tests.
//!
//! Supports:
//! - [`MockBackend::add_device`] — register a device for enumeration
//! - [`MockBackend::queue_response`] — enqueue a canned read response
//! - [`MockBackend::write_history`] — inspect writes for assertions
//!
//! All state is `Arc<Mutex<...>>`-internal; the backend itself is `Clone`
//! so tests can hand it to the daemon's `Arc<dyn HidBackend>` and then
//! still call `add_device` / `queue_response` / `write_history` from the
//! test body.
//!
//! # Examples
//!
//! ```
//! use lconnect_hid::mock_backend::MockBackend;
//! use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend};
//! use std::path::PathBuf;
//!
//! let mock = MockBackend::new();
//! let info = DeviceInfo::new(
//!     DeviceFamilyId::new("galahad2-lcdv2"),
//!     0x0416,
//!     0x7395,
//!     None,
//!     PathBuf::from("/mock/hidraw0"),
//! );
//! mock.add_device(info.clone());
//! mock.queue_response(&info, vec![0x01, 0x86, 0xAA]);
//!
//! let handle = mock.open(&info).unwrap();
//! handle.write(&[0x01, 0x86]).unwrap();
//! let mut buf = [0u8; 64];
//! let n = handle.read_timeout(&mut buf, 100).unwrap();
//! assert_eq!(&buf[..n], &[0x01, 0x86, 0xAA]);
//! ```
//!
//! # See also
//!
//! - [`crate::HidBackend`] — the trait this implements

use crate::{DeviceInfo, Error, HidBackend, OpenHandle, Result};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DeviceState {
    write_history: Vec<Vec<u8>>,
    queued_responses: VecDeque<Vec<u8>>,
}

#[derive(Default)]
struct Inner {
    devices: Vec<DeviceInfo>,
    states: HashMap<DeviceKey, Arc<Mutex<DeviceState>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceKey {
    vid: u16,
    pid: u16,
    path: PathBuf,
}

impl DeviceKey {
    fn from(info: &DeviceInfo) -> Self {
        Self {
            vid: info.vid,
            pid: info.pid,
            path: info.path.clone(),
        }
    }
}

/// In-process mock HID backend.
///
/// See module-level docs for usage. `Clone` is cheap (`Arc`-based).
#[derive(Default, Clone)]
pub struct MockBackend {
    inner: Arc<Mutex<Inner>>,
}

impl MockBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a device for `enumerate` + `open`.
    pub fn add_device(&self, info: DeviceInfo) {
        let mut inner = self.inner.lock().expect("mock mutex poisoned");
        let key = DeviceKey::from(&info);
        inner.devices.push(info);
        inner.states.entry(key).or_default();
    }

    /// Queue a canned response that the next `read_timeout` call on the
    /// device's handle will return.
    ///
    /// # Panics
    ///
    /// Panics if `info` was not previously registered via `add_device`.
    pub fn queue_response(&self, info: &DeviceInfo, bytes: Vec<u8>) {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        let state = inner
            .states
            .get(&DeviceKey::from(info))
            .expect("device not registered; call add_device first")
            .clone();
        drop(inner);
        state
            .lock()
            .expect("state mutex poisoned")
            .queued_responses
            .push_back(bytes);
    }

    /// Inspect the write log for assertions. Returns empty vec for
    /// unregistered devices.
    #[must_use]
    pub fn write_history(&self, info: &DeviceInfo) -> Vec<Vec<u8>> {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        inner
            .states
            .get(&DeviceKey::from(info))
            .map(|s| {
                s.lock()
                    .expect("state mutex poisoned")
                    .write_history
                    .clone()
            })
            .unwrap_or_default()
    }
}

impl HidBackend for MockBackend {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>> {
        Ok(self
            .inner
            .lock()
            .expect("mock mutex poisoned")
            .devices
            .clone())
    }

    fn open(&self, info: &DeviceInfo) -> Result<Box<dyn OpenHandle>> {
        let inner = self.inner.lock().expect("mock mutex poisoned");
        let state = inner
            .states
            .get(&DeviceKey::from(info))
            .ok_or_else(|| Error::DeviceNotFound(format!("{:?}", info.path)))?
            .clone();
        Ok(Box::new(MockHandle { state }))
    }
}

struct MockHandle {
    state: Arc<Mutex<DeviceState>>,
}

impl OpenHandle for MockHandle {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.state
            .lock()
            .expect("state mutex poisoned")
            .write_history
            .push(buf.to_vec());
        Ok(buf.len())
    }

    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let mut state = self.state.lock().expect("state mutex poisoned");
        if let Some(canned) = state.queued_responses.pop_front() {
            let n = canned.len().min(buf.len());
            buf[..n].copy_from_slice(&canned[..n]);
            return Ok(n);
        }
        Err(Error::ReadTimeout {
            ms: timeout_ms.max(0) as u64,
        })
    }
}
