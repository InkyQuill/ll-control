use crate::{
    connection::{connect, resolve_device},
    ThermalSubcommand,
};
use llcontrol_ipc::{ThermalConfig, ThermalState};
use serde_json::json;
use std::io::Read;
pub async fn run(command: ThermalSubcommand) -> anyhow::Result<()> {
    let (device, config) = match command {
        ThermalSubcommand::State { device } => (device, None),
        ThermalSubcommand::Disable { device } => (device, Some(ThermalConfig::default())),
        ThermalSubcommand::Set { config, device } => {
            let mut bytes = Vec::new();
            std::fs::File::open(config)?
                .take(65537)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                anyhow::bail!("thermal config exceeds 64 KiB");
            }
            let config: ThermalConfig = serde_json::from_slice(&bytes)?;
            config.validate()?;
            (device, Some(config))
        }
    };
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let result = match &config {
        Some(config) => {
            client
                .call("set_thermal_config", json!({"dev":dev,"config":config}))
                .await?
        }
        None => client.call("get_thermal_state", json!({"dev":dev})).await?,
    };
    let state: ThermalState = serde_json::from_value(result)?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    if config.is_some()
        && (state.blocked_reason.is_some()
            || state.fan.last_error.is_some()
            || state.pump.last_error.is_some())
    {
        anyhow::bail!("thermal configuration retained; lighting application failed, inspect state");
    }
    Ok(())
}
