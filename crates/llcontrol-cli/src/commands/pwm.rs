//! Fan and pump PWM subcommands.
//!
//! Both subcommands validate the percent value locally before talking
//! to the daemon, then resolve the target device (auto-pick the only
//! attached device when `--device` is omitted) and issue the matching
//! IPC call.

use crate::connection::{connect, resolve_device};
use serde_json::json;

/// Local range check for a PWM percent value.
fn validate_percent(percent: u8) -> anyhow::Result<()> {
    if percent > 100 {
        anyhow::bail!("PWM out of range: {percent} (allowed 0..=100)");
    }
    Ok(())
}

/// `llcontrol fan <PERCENT> [--device ID]`
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

/// `llcontrol pump <PERCENT> [--source mcu|application] [--device ID]`
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
