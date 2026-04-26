//! Process-wide atomic counters surfaced via DaemonRpc::daemon_stats.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Stats {
    pub started_at: Instant,
    pub hid_write_count: AtomicU64,
    pub hid_read_errors: AtomicU64,
    pub hid_decode_errors: AtomicU64,
    pub ipc_request_count: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            hid_write_count: AtomicU64::new(0),
            hid_read_errors: AtomicU64::new(0),
            hid_decode_errors: AtomicU64::new(0),
            ipc_request_count: AtomicU64::new(0),
        }
    }

    pub fn inc_hid_write(&self) {
        self.hid_write_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_hid_read_error(&self) {
        self.hid_read_errors.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_hid_decode_error(&self) {
        self.hid_decode_errors.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_ipc_request(&self) {
        self.ipc_request_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot the counters into the IPC-facing DaemonStats type.
    pub fn snapshot(&self, devices_known: u32) -> lconnect_ipc::DaemonStats {
        use lconnect_ipc::DaemonStats;
        // DaemonStats is #[non_exhaustive] in lconnect-ipc; build via serde_json::from_value
        // to avoid struct-literal restriction across crate boundary.
        let value = serde_json::json!({
            "uptime_seconds": self.started_at.elapsed().as_secs(),
            "devices_known": devices_known,
            "hid_write_count": self.hid_write_count.load(Ordering::Relaxed),
            "hid_read_errors": self.hid_read_errors.load(Ordering::Relaxed),
            "hid_decode_errors": self.hid_decode_errors.load(Ordering::Relaxed),
            "ipc_request_count": self.ipc_request_count.load(Ordering::Relaxed),
        });
        serde_json::from_value::<DaemonStats>(value)
            .expect("DaemonStats schema mismatch -- bug in stats::snapshot")
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}
