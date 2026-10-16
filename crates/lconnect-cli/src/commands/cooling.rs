use crate::{
    connection::{connect, resolve_device},
    CoolingSubcommand,
};
use lconnect_ipc::{CoolingPolicy, CoolingState, CoolingTarget, FanPreset, PumpPreset};
use serde_json::json;
use std::io::Read;

pub async fn run(command: CoolingSubcommand) -> anyhow::Result<()> {
    // Validate the policy file before opening IPC; no mutation on bad input.
    let (device, mutation) = match command {
        CoolingSubcommand::Preset {
            name,
            target,
            sensor,
            hysteresis_c,
            fallback_percent,
        } => {
            let (target, policy) = if target == "fan" {
                let preset = match name.as_str() {
                    "quiet" => FanPreset::Quiet,
                    "standard" => FanPreset::Standard,
                    "high" => FanPreset::High,
                    "full" => FanPreset::Full,
                    _ => anyhow::bail!("fan presets: quiet, standard, high, full"),
                };
                (
                    CoolingTarget::Fan,
                    preset.policy(sensor, hysteresis_c, fallback_percent),
                )
            } else {
                let preset = match name.as_str() {
                    "quiet" => PumpPreset::Quiet,
                    "performance" => PumpPreset::Performance,
                    _ => anyhow::bail!("pump presets: quiet, performance"),
                };
                (
                    CoolingTarget::Pump,
                    preset.policy(sensor, hysteresis_c, fallback_percent),
                )
            };
            policy.validate(target)?;
            println!("{}", serde_json::to_string_pretty(&policy)?);
            return Ok(());
        }
        CoolingSubcommand::State { device } => (device, None),
        CoolingSubcommand::Set {
            target,
            policy,
            device,
        } => {
            let target = match target.as_str() {
                "fan" => CoolingTarget::Fan,
                "pump" => CoolingTarget::Pump,
                _ => anyhow::bail!("target must be fan or pump"),
            };
            let mut bytes = Vec::new();
            std::fs::File::open(policy)?
                .take(65537)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 65536 {
                anyhow::bail!("policy file exceeds 64 KiB");
            }
            let policy: CoolingPolicy = serde_json::from_slice(&bytes)?;
            policy.validate(target)?;
            (device, Some((target, policy)))
        }
    };
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let result = if let Some((target, policy)) = &mutation {
        client
            .call(
                "set_cooling_policy",
                json!({"dev":dev,"target":target,"policy":policy}),
            )
            .await?
    } else {
        client.call("get_cooling_state", json!({"dev":dev})).await?
    };
    let state: CoolingState = serde_json::from_value(result)?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    if let Some((target, _)) = mutation {
        let channel = match target {
            CoolingTarget::Fan => state.fan,
            CoolingTarget::Pump => state.pump,
        };
        if let Some(error) = channel.last_error {
            anyhow::bail!("policy retained but initial PWM write failed: {error}");
        }
    }
    Ok(())
}
