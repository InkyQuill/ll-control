//! sensor_poll actor — periodic GetHandshakeInfo + watch fanout.
//!
//! Spawns a periodic timer (default cadence 1 Hz, configurable via
//! `poll_period_ms`); each tick sends [`Command::GetHandshakeInfo`]
//! (cmd 129, A-class) through the per-device `hid_io_tx`, parses the
//! response with [`lconnect_protocol::galahad2_lcdv2::handshake`], and
//! republishes the result as a [`SensorSnapshot`] on a
//! `tokio::sync::watch` channel.
//!
//! Subscribers (IPC `subscribe_snapshots`, future GUI) borrow the
//! latest value from the watch; the watch coalesces — slow consumers
//! never delay the poll loop, they just see the most recent reading.
//!
//! Failures (HID I/O error, response decode error, channel send fail)
//! are logged at `debug` level and otherwise ignored: the next tick
//! will retry. The watch retains the last successful reading.
//!
//! [`Command::GetHandshakeInfo`]: lconnect_protocol::galahad2_lcdv2::commands::Command::GetHandshakeInfo

use super::{HidCommand, Shutdown};
use lconnect_ipc::SensorSnapshot;
use lconnect_protocol::galahad2_lcdv2::handshake::HandshakeInfo;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::debug;

/// Spawn the per-device sensor-poll actor.
///
/// Returns a `watch::Receiver` that yields the most recent
/// `SensorSnapshot`. The initial value is `SensorSnapshot::default()`
/// (all-zero, `temp_valid = false`, `last_seen_unix_ms = 0`); callers
/// can detect "no reading yet" by checking `last_seen_unix_ms == 0`.
///
/// The actor runs until either `shutdown` fires or `hid_io_tx` is
/// dropped.
pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    poll_period_ms: u64,
    shutdown: Shutdown,
) -> watch::Receiver<SensorSnapshot> {
    spawn_with_cache(
        hid_io_tx,
        poll_period_ms,
        shutdown,
        Default::default(),
        String::new(),
    )
}

pub fn spawn_with_cache(
    hid_io_tx: mpsc::Sender<HidCommand>,
    poll_period_ms: u64,
    shutdown: Shutdown,
    cache: crate::sensors::SensorCache,
    key: String,
) -> watch::Receiver<SensorSnapshot> {
    spawn_with_cache_tracked(hid_io_tx, poll_period_ms, shutdown, cache, key).0
}

/// Return the owned task as well as the snapshot feed for teardown barriers.
pub fn spawn_with_cache_tracked(
    hid_io_tx: mpsc::Sender<HidCommand>,
    poll_period_ms: u64,
    mut shutdown: Shutdown,
    cache: crate::sensors::SensorCache,
    key: String,
) -> (watch::Receiver<SensorSnapshot>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = watch::channel(SensorSnapshot::default());
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(poll_period_ms));
        // Default `Burst` policy would compensate for any missed ticks;
        // `Delay` is friendlier to the device when poll work runs slow.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if *shutdown.borrow() || shutdown.has_changed().is_err() {
                break;
            }
            tokio::select! {
                _ = shutdown.changed() => {
                    debug!("sensor_poll shutting down");
                    break;
                }
                _ = interval.tick() => {
                    match poll_once(&hid_io_tx).await {
                        Ok(snapshot) => {
                            cache.publish(std::time::Instant::now(), crate::sensors::device_readings(&key, &snapshot));
                            // `send` errors only when there are no receivers;
                            // that's a normal teardown signal.
                            if tx.send(snapshot).is_err() {
                                debug!("sensor_poll: no receivers, exiting");
                                break;
                            }
                        }
                        Err(err) => {
                            debug!(error = ?err, "sensor_poll: poll_once failed");
                        }
                    }
                }
            }
        }
    });
    (rx, task)
}

/// Issue one GetHandshakeInfo round-trip and decode the response into
/// a `SensorSnapshot`.
async fn poll_once(tx: &mpsc::Sender<HidCommand>) -> anyhow::Result<SensorSnapshot> {
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(HidCommand::PollSensors { reply: reply_tx })
        .await
        .map_err(|e| anyhow::anyhow!("hid_io channel closed: {e}"))?;
    let resp = reply_rx
        .await
        .map_err(|e| anyhow::anyhow!("hid_io reply dropped: {e}"))??;
    let parsed = HandshakeInfo::from_response(&resp)?;
    Ok(handshake_to_snapshot(parsed, now_unix_ms()))
}

/// Convert a decoded `HandshakeInfo` into a wire-format `SensorSnapshot`.
///
/// Tenths-of-degree are computed the same way the C# UI does:
/// integer part * 10 + (raw_fraction % 10).
fn handshake_to_snapshot(info: HandshakeInfo, now_ms: u64) -> SensorSnapshot {
    let tenths = i16::from(info.temp_integer)
        .saturating_mul(10)
        .saturating_add(i16::from(info.temp_fraction % 10));
    let mut snap = SensorSnapshot::default();
    snap.coolant_c_x10 = tenths;
    snap.fan_rpm = info.fan_rpm;
    snap.pump_rpm = info.pump_rpm;
    snap.temp_valid = info.temp_valid;
    snap.last_seen_unix_ms = now_ms;
    snap
}

/// Wall-clock milliseconds since the Unix epoch. Returns 0 if the
/// system clock is set before 1970 (shouldn't happen in practice; we
/// degrade gracefully rather than panic).
fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
