//! Fan and pump PWM subcommands.
//!
//! Both subcommands validate the percent value locally before talking
//! to the daemon, then resolve the target device (auto-pick the only
//! attached device when `--device` is omitted) and issue the matching
//! IPC call.

use crate::connection::connect;
use serde_json::json;

/// Local range check for a PWM percent value.
fn validate_percent(percent: u8) -> anyhow::Result<()> {
    if percent > 100 {
        anyhow::bail!("PWM out of range: {percent} (allowed 0..=100)");
    }
    Ok(())
}

/// Resolve a `DeviceId`: honour `--device` if present, otherwise call
/// `list_devices` and auto-pick when exactly one device is attached.
async fn resolve_device(
    client: &lconnect_ipc::client::IpcClient,
    override_id: Option<String>,
) -> anyhow::Result<lconnect_ipc::DeviceId> {
    use lconnect_ipc::DeviceId;
    if let Some(s) = override_id {
        return Ok(DeviceId(s));
    }
    let v = client.call("list_devices", serde_json::json!({})).await?;
    let descs: Vec<lconnect_ipc::DeviceDescriptor> = serde_json::from_value(v)?;
    match descs.len() {
        0 => anyhow::bail!("no devices attached"),
        1 => Ok(descs.into_iter().next().unwrap().id),
        _ => {
            let ids: Vec<String> = descs.into_iter().map(|d| d.id.0).collect();
            anyhow::bail!(
                "ambiguous device; specify --device. candidates: {}",
                ids.join(", ")
            )
        }
    }
}

/// `lconnectctl fan <PERCENT> [--device ID]`
pub async fn fan(percent: u8, device: Option<String>) -> anyhow::Result<()> {
    if let Err(e) = validate_percent(percent) {
        anyhow::bail!("fan {e}");
    }
    let client = connect().await?;
    let device_id = resolve_device(&client, device).await?;
    client
        .call(
            "set_fan_pwm",
            json!({ "dev": device_id, "percent": percent }),
        )
        .await?;
    println!("Set fan PWM to {percent}%");
    Ok(())
}

/// `lconnectctl pump <PERCENT> [--source mcu|application] [--device ID]`
pub async fn pump(
    percent: u8,
    source: Option<String>,
    device: Option<String>,
) -> anyhow::Result<()> {
    if let Err(e) = validate_percent(percent) {
        anyhow::bail!("pump {e}");
    }
    let source_str = match source.as_deref() {
        Some("mcu") | None => "mcu",
        Some("application") => "application",
        Some(s) => anyhow::bail!("unknown pump source: {s} (allowed: mcu, application)"),
    };
    let client = connect().await?;
    let device_id = resolve_device(&client, device).await?;
    client
        .call(
            "set_pump_pwm",
            json!({
                "dev": device_id,
                "percent": percent,
                "source": source_str,
            }),
        )
        .await?;
    println!("Set pump PWM to {percent}% (source: {source_str})");
    Ok(())
}
