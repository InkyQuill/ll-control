//! `devices`, `firmware`, and `snapshot` subcommands.
//!
//! All three open a single IPC connection via [`crate::connection::connect`]
//! and issue one JSON-RPC call. The shared
//! [`crate::connection::resolve_device`] helper handles the `--device`
//! flag for `firmware` and `snapshot`: when omitted it auto-picks the only
//! attached device, errors with the candidate list when ambiguous, and
//! errors cleanly when nothing is attached.

use anyhow::{Context, Result};
use lconnect_ipc::types::{DeviceDescriptor, FirmwareInfo, SensorSnapshot};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::connection::{self, resolve_device};

/// `lconnectctl devices` — list every attached device.
pub async fn run(json_mode: bool) -> Result<()> {
    let client = connection::connect().await?;
    let value = client
        .call("list_devices", json!({}))
        .await
        .context("list_devices RPC failed")?;
    let descriptors: Vec<DeviceDescriptor> = serde_json::from_value(value)
        .context("failed to deserialise list_devices response as Vec<DeviceDescriptor>")?;

    if json_mode {
        let s = serde_json::to_string(&descriptors)
            .context("failed to serialise device list as JSON")?;
        println!("{s}");
        return Ok(());
    }

    if descriptors.is_empty() {
        println!("no devices attached");
        return Ok(());
    }

    print_device_table(&descriptors);
    Ok(())
}

/// `lconnectctl firmware [--device <id>]` — print firmware info.
pub async fn firmware(device: Option<String>, json_mode: bool) -> Result<()> {
    let client = connection::connect().await?;
    let dev = resolve_device(&client, device).await?;
    let value = client
        .call("get_firmware", json!({ "dev": dev }))
        .await
        .context("get_firmware RPC failed")?;
    let info: FirmwareInfo = serde_json::from_value(value)
        .context("failed to deserialise get_firmware response as FirmwareInfo")?;

    if json_mode {
        let s =
            serde_json::to_string(&info).context("failed to serialise firmware info as JSON")?;
        println!("{s}");
    } else {
        println!("Version:   {}", info.version);
        println!("Raw token: {}", info.raw_token);
    }
    Ok(())
}

/// `lconnectctl snapshot [--device <id>]` — print latest sensor snapshot.
pub async fn snapshot(device: Option<String>, json_mode: bool) -> Result<()> {
    let client = connection::connect().await?;
    let dev = resolve_device(&client, device).await?;
    let value = client
        .call("get_snapshot", json!({ "dev": dev }))
        .await
        .context("get_snapshot RPC failed")?;
    let snap: SensorSnapshot = serde_json::from_value(value)
        .context("failed to deserialise get_snapshot response as SensorSnapshot")?;

    if json_mode {
        let s =
            serde_json::to_string(&snap).context("failed to serialise sensor snapshot as JSON")?;
        println!("{s}");
    } else {
        print_snapshot_human(&snap);
    }
    Ok(())
}

/// Render the human-readable device table used by `lconnectctl devices`.
fn print_device_table(descriptors: &[DeviceDescriptor]) {
    println!("{:<28}{:<18}{:<15}SERIAL", "ID", "FAMILY", "VID:PID");
    for d in descriptors {
        let vidpid = format!("{:04x}:{:04x}", d.vid, d.pid);
        let serial = d.serial.as_deref().unwrap_or("-");
        println!(
            "{:<28}{:<18}{:<15}{}",
            d.id.as_str(),
            d.family,
            vidpid,
            serial
        );
    }
}

/// Render the human-readable sensor block used by `lconnectctl snapshot`.
fn print_snapshot_human(snap: &SensorSnapshot) {
    if snap.temp_valid {
        let coolant_c = f64::from(snap.coolant_c_x10) / 10.0;
        println!("Liquid:  {:.1} C", coolant_c);
    } else {
        println!("Liquid:  (invalid)");
    }
    println!("Fan:     {} rpm", snap.fan_rpm);
    println!("Pump:    {} rpm", snap.pump_rpm);
    println!("Last:    {}", format_age(snap.last_seen_unix_ms));
}

/// Format "milliseconds since unix epoch" as a coarse "X ago" string.
///
/// Returns "never" when the timestamp is zero (the daemon's sentinel for
/// "no snapshot yet"), and falls back gracefully if the system clock sits
/// before the snapshot timestamp (clock-skew or test fixtures).
fn format_age(last_seen_unix_ms: u64) -> String {
    if last_seen_unix_ms == 0 {
        return "never".to_string();
    }
    let now_ms = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis(),
        Err(_) => return "unknown".to_string(),
    };
    let last = u128::from(last_seen_unix_ms);
    if now_ms < last {
        // Clock skew — report the raw "in the future" age as 0.
        return "0ms ago".to_string();
    }
    let delta_ms = now_ms - last;
    if delta_ms < 1_000 {
        format!("{delta_ms}ms ago")
    } else if delta_ms < 60_000 {
        format!("{}s ago", delta_ms / 1_000)
    } else if delta_ms < 3_600_000 {
        format!("{}m ago", delta_ms / 60_000)
    } else {
        format!("{}h ago", delta_ms / 3_600_000)
    }
}
