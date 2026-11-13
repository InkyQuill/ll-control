//! Read-only desktop-shell feed. Uses daemon caches, never device polling or GUI.
use anyhow::{Context, Result};
use clap::Subcommand;
use lconnect_ipc::{client::IpcClient, DisplaySettings, SensorDefaultsState, SensorReading};
use serde::Serialize;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;

#[derive(Subcommand)]
pub enum Command {
    /// Emit one versioned JSON snapshot; unavailable data is explicit.
    Snapshot,
    /// Emit NDJSON once per second, reconnecting read-only after daemon loss.
    Watch,
}
#[derive(Serialize)]
struct Data {
    sensors: Vec<SensorReading>,
    defaults: SensorDefaultsState,
    display: DisplaySettings,
}
#[derive(Serialize)]
struct Snapshot {
    version: u32,
    observed_unix_ms: u64,
    available: bool,
    data: Option<Data>,
    error: Option<String>,
}
async fn read(client: &IpcClient) -> Result<Data> {
    let (sensors, defaults, display) = tokio::try_join!(
        client.call(lconnect_ipc::methods::LIST_SENSORS_NAME, json!({})),
        client.call("get_sensor_defaults", json!({})),
        client.call(lconnect_ipc::methods::GET_DISPLAY_SETTINGS_NAME, json!({}))
    )?;
    let data = Data {
        sensors: serde_json::from_value(sensors)?,
        defaults: serde_json::from_value(defaults)?,
        display: serde_json::from_value(display)?,
    };
    data.display.validate()?;
    data.defaults.config.validate()?;
    data.defaults.recommended.validate()?;
    Ok(data)
}
async fn snapshot(client: &mut Option<IpcClient>) -> Snapshot {
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        if client.is_none() {
            *client = Some(crate::connection::connect().await?);
        }
        read(client.as_ref().context("Mini connection unavailable")?).await
    })
    .await
    .context("Mini cache read timed out")
    .and_then(|r| r);
    let observed_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    match result {
        Ok(data) => Snapshot {
            version: 1,
            observed_unix_ms,
            available: true,
            data: Some(data),
            error: None,
        },
        Err(e) => {
            // Dropping a failed read connection is safe: no mutation can be replayed.
            *client = None;
            Snapshot {
                version: 1,
                observed_unix_ms,
                available: false,
                data: None,
                error: Some(e.to_string()),
            }
        }
    }
}
pub async fn run(command: Command) -> Result<()> {
    let mut client = None;
    let mut output = tokio::io::stdout();
    let watch = matches!(command, Command::Watch);
    let work = async {
        loop {
            let value = snapshot(&mut client).await;
            let mut bytes = serde_json::to_vec(&value)?;
            anyhow::ensure!(
                bytes.len() <= 1024 * 1024,
                "Mini snapshot exceeds output budget"
            );
            bytes.push(b'\n');
            output.write_all(&bytes).await?;
            output.flush().await?;
            if !watch {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };
    tokio::select! {
        result=work=>result,
        signal=tokio::signal::ctrl_c()=>{signal?;Ok(())}
    }
}
