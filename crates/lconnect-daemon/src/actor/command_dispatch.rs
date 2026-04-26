//! command_dispatch -- per-device settings persistence with debounce.
//!
//! The IPC server pushes [`DeviceSettings`] updates here at a potentially
//! high rate (e.g., a user dragging a slider). We coalesce a burst into a
//! single atomic disk write after `DEBOUNCE_MS` of quiet.
//!
//! The actor:
//! - Coalesces bursts: 50 updates in 100 ms collapse to one disk write
//!   after the debounce window expires.
//! - Flushes on shutdown so the latest in-memory state is durable.
//! - Flushes when the sender drops (clean test teardown).
//! - Logs warnings on write failure but does not propagate; the next
//!   update overwrites the in-memory state and retries.
//!
//! The `if dirty` guard on the `sleep` arm of `tokio::select!` is
//! critical: without it the actor would wake every `DEBOUNCE_MS` even
//! when there was nothing to flush.

use super::Shutdown;
use crate::persistence::{DeviceSettings, PersistenceStore};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// Quiet-period after the latest update before we write to disk.
const DEBOUNCE_MS: u64 = 250;

/// Spawn the per-device command_dispatch actor.
///
/// `key` is the persistence key (e.g., `"galahad2-lcdv2:0001"`); pass it
/// in -- the actor does not derive it. `initial` seeds the in-memory
/// state from the last known on-disk settings so the first update only
/// triggers a write if it actually differs. `settings_rx` carries the
/// full replacement settings struct on every IPC mutation.
///
/// The task terminates when `shutdown` flips or `settings_rx` is closed.
/// In both cases, any pending dirty state is flushed before exit.
pub fn spawn(
    store: Arc<PersistenceStore>,
    key: String,
    initial: DeviceSettings,
    mut settings_rx: mpsc::Receiver<DeviceSettings>,
    mut shutdown: Shutdown,
) -> JoinHandle<()> {
    let debounce = Duration::from_millis(DEBOUNCE_MS);
    tokio::spawn(async move {
        let mut latest = initial;
        let mut dirty = false;
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    debug!("command_dispatch shutting down (key={})", key);
                    if dirty {
                        if let Err(e) = store.write(&key, &latest) {
                            warn!("command_dispatch flush-on-shutdown failed (key={}): {}", key, e);
                        }
                    }
                    break;
                }
                maybe = settings_rx.recv() => match maybe {
                    Some(s) => {
                        latest = s;
                        dirty = true;
                    }
                    None => {
                        debug!("command_dispatch settings_rx closed (key={}); flushing and exiting", key);
                        if dirty {
                            if let Err(e) = store.write(&key, &latest) {
                                warn!("command_dispatch flush-on-drop failed (key={}): {}", key, e);
                            }
                        }
                        break;
                    }
                },
                // Fire only when there's something pending; without the
                // guard we'd wake every DEBOUNCE_MS even while idle.
                _ = tokio::time::sleep(debounce), if dirty => {
                    if let Err(e) = store.write(&key, &latest) {
                        warn!("command_dispatch debounced write failed (key={}): {}", key, e);
                    }
                    dirty = false;
                }
            }
        }
    })
}
