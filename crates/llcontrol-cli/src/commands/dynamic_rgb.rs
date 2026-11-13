use crate::{
    connection::{connect, resolve_device},
    ThermalSubcommand,
};
use llcontrol_ipc::{DynamicRgbConfig, DynamicRgbState};
use serde_json::json;
use std::io::Read;
pub async fn run(command: ThermalSubcommand, as_json: bool) -> anyhow::Result<()> {
    let (device, change) = match command {
        ThermalSubcommand::State { device } => (device, None),
        ThermalSubcommand::Disable { device } => (device, Some(None)),
        ThermalSubcommand::Set { config, device } => {
            let mut bytes = Vec::new();
            std::fs::File::open(config)?
                .take(65537)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 65536, "dynamic RGB config exceeds 64 KiB");
            let config: DynamicRgbConfig = serde_json::from_slice(&bytes)?;
            config.validate()?;
            (device, Some(Some(config)))
        }
    };
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let result = match &change {
        Some(config) => {
            client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":config}))
                .await?
        }
        None => client.call("get_dynamic_rgb", json!({"dev":dev})).await?,
    };
    let state: DynamicRgbState = serde_json::from_value(result)?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&state)?);
    } else {
        println!(
            "Mode: {}",
            state.base_mode.as_deref().unwrap_or("unavailable")
        );
        println!(
            "Dynamic RGB: {}",
            if state.requested.is_some() {
                "enabled"
            } else {
                "disabled"
            }
        );
        if let Some(config) = &state.requested {
            println!(
                "Sensor: {} ({:?}), {}..{}",
                config.sensor_id, config.unit, config.low_value, config.high_value
            );
        }
        if state.suppressed_by_thermal {
            println!("Thermal alert overrides the dynamic output.");
        }
        if state.degraded {
            println!("Sensor unavailable; ordinary lighting base is retained.");
        }
        if let Some(sample) = &state.applied {
            println!("Applied sample: {}", sample.value);
        }
        if let Some(error) = &state.last_error {
            println!("Application error: {error}");
        }
    }
    if change.is_some() && state.last_error.is_some() {
        anyhow::bail!("dynamic RGB configuration retained; lighting application failed");
    }
    Ok(())
}
