//! `watch` subcommand -- stream `SensorSnapshot` notifications from the
//! daemon and either redraw a TTY block in place or emit one NDJSON
//! line per snapshot.
//!
//! Subscribes to the `"snapshot"` notification channel exposed by
//! [`lconnect_ipc::client::IpcClient::subscribe`] (P1D-13) and pumps
//! incoming notifications to stdout until the user hits Ctrl+C.

use anyhow::{anyhow, Context};
use lconnect_ipc::types::{DeviceDescriptor, SensorSnapshot};
use serde_json::json;
use std::io::{IsTerminal, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::connection::connect;
use crate::format::print_ndjson_line;

/// Number of human-readable lines emitted per redraw block. Kept here
/// (rather than inlined as a literal) because the ANSI "move-up-N"
/// escape below has to match exactly.
const TTY_BLOCK_LINES: usize = 4;

/// Stream sensor snapshots until interrupted.
///
/// Behaviour:
/// - `--json` or non-TTY stdout: one NDJSON line per snapshot.
/// - TTY: redraw a 4-line block in place using ANSI cursor controls.
pub async fn run(device: Option<String>, json: bool) -> anyhow::Result<()> {
    let client = connect().await?;

    // Resolve the target device. Mirrors the logic the sibling P1E-2
    // task uses in `commands::devices::resolve_device`; copied locally
    // to avoid a cross-file dependency between parallel tasks.
    let device_id = resolve_device(&client, device).await?;

    // Register the subscription *before* asking the daemon to start
    // pushing snapshots, so the very first notification can't race the
    // reader and be dropped on the floor.
    let mut rx = client.subscribe("snapshot").await;

    // The result body (`{"subscribed": true, "device_id": ...}`) is not
    // load-bearing for the CLI; we just need the call to succeed so the
    // daemon starts its pump task.
    let _ = client
        .call("subscribe_snapshots", json!({ "dev": device_id }))
        .await
        .context("subscribe_snapshots failed")?;

    // If stdout is being piped (e.g. `lconnectctl watch | tee ...`),
    // ANSI cursor controls would corrupt the file, so we coerce into
    // NDJSON mode regardless of the explicit flag.
    let ndjson = json || !std::io::stdout().is_terminal();

    let mut stdout = std::io::stdout().lock();
    let mut first = true;

    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    loop {
        tokio::select! {
            biased;
            _ = &mut ctrl_c => {
                // Move past the redraw region so the shell prompt
                // doesn't overwrite our last frame.
                if !ndjson && !first {
                    let _ = writeln!(stdout);
                }
                let _ = stdout.flush();
                return Ok(());
            }
            maybe_notif = rx.recv() => {
                let Some(notif) = maybe_notif else {
                    // Channel closed -- daemon hung up.
                    if !ndjson && !first {
                        let _ = writeln!(stdout);
                    }
                    let _ = stdout.flush();
                    return Err(anyhow!("daemon closed the snapshot subscription"));
                };
                let snap_val = notif
                    .params
                    .get("snapshot")
                    .ok_or_else(|| anyhow!("snapshot notification missing `snapshot` field"))?
                    .clone();
                let snapshot: SensorSnapshot = serde_json::from_value(snap_val)
                    .context("decoding SensorSnapshot from notification")?;

                if ndjson {
                    print_ndjson_line(&snapshot);
                } else {
                    if first {
                        first = false;
                    } else {
                        // Move cursor up `TTY_BLOCK_LINES` rows and
                        // clear from the cursor to the end of screen,
                        // then reprint the whole block. `\x1b[<n>A`
                        // is CSI Cursor Up; `\x1b[J` is CSI Erase
                        // Display (default 0 = cursor to end).
                        let _ = write!(stdout, "\x1b[{TTY_BLOCK_LINES}A\x1b[J");
                    }
                    write_human_block(&mut stdout, &snapshot)?;
                    let _ = stdout.flush();
                }
            }
        }
    }
}

/// Resolve a device id from the optional `--device` flag.
///
/// If `device` is `Some`, it's used verbatim. Otherwise the CLI calls
/// `list_devices` and picks the only attached device, erroring if zero
/// or more than one are present.
async fn resolve_device(
    client: &lconnect_ipc::client::IpcClient,
    device: Option<String>,
) -> anyhow::Result<String> {
    if let Some(d) = device {
        return Ok(d);
    }
    let value = client
        .call("list_devices", json!({}))
        .await
        .context("list_devices failed")?;
    let devices: Vec<DeviceDescriptor> =
        serde_json::from_value(value).context("decoding list_devices response")?;
    match devices.as_slice() {
        [] => Err(anyhow!(
            "no devices attached; pass --device <id> once one is connected"
        )),
        [only] => Ok(only.id.0.clone()),
        many => {
            let ids: Vec<&str> = many.iter().map(|d| d.id.0.as_str()).collect();
            Err(anyhow!(
                "multiple devices attached ({}); pass --device <id> to choose one",
                ids.join(", ")
            ))
        }
    }
}

/// Write the four-line human-readable snapshot block. Layout is fixed
/// (same number of lines every redraw) so the move-up-N escape stays
/// in sync with what's actually on screen.
fn write_human_block<W: Write>(out: &mut W, snap: &SensorSnapshot) -> anyhow::Result<()> {
    let liquid_c = f64::from(snap.coolant_c_x10) / 10.0;
    let age = snapshot_age(snap.last_seen_unix_ms);
    if snap.temp_valid {
        writeln!(out, "Liquid: {liquid_c:>5.1} C")?;
    } else {
        writeln!(out, "Liquid: --.- C (invalid)")?;
    }
    writeln!(out, "Fan:    {:>5} rpm", snap.fan_rpm)?;
    writeln!(out, "Pump:   {:>5} rpm", snap.pump_rpm)?;
    writeln!(out, "Time:   {age}")?;
    Ok(())
}

/// Render the age of a snapshot relative to wall-clock now. We don't
/// rely on `std::time::Instant` here because the timestamp comes from
/// the daemon as a Unix epoch in milliseconds.
fn snapshot_age(last_seen_unix_ms: u64) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if last_seen_unix_ms == 0 {
        return "n/a".into();
    }
    let delta = now_ms.saturating_sub(last_seen_unix_ms);
    if delta < 1_000 {
        format!("{delta} ms ago")
    } else if delta < 60_000 {
        format!("{:.1} s ago", delta as f64 / 1000.0)
    } else {
        let mins = delta / 60_000;
        let secs = (delta % 60_000) / 1_000;
        format!("{mins}m {secs}s ago")
    }
}
