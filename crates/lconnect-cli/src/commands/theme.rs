use crate::{
    connection::{connect, resolve_device},
    ThemeSubcommand,
};
use lconnect_ipc::LcdTheme;
use serde_json::json;
use std::{
    io::{Read, Write},
    path::Path,
};
fn bounded(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "input exceeds {limit} bytes");
    Ok(bytes)
}
fn theme(path: &Path) -> anyhow::Result<LcdTheme> {
    let value: LcdTheme = serde_json::from_slice(&bounded(path, 65536)?)?;
    value.validate()?;
    Ok(value)
}
pub async fn run(command: ThemeSubcommand) -> anyhow::Result<()> {
    match command {
        ThemeSubcommand::Cleanup { delete, keep } => {
            let report: lconnect_ipc::AssetCleanupReport = serde_json::from_value(
                connect()
                    .await?
                    .call("cleanup_theme_assets", json!({"delete":delete,"keep":keep}))
                    .await?,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            anyhow::ensure!(
                report.failures.is_empty(),
                "some assets could not be deleted; see failures"
            );
        }

        ThemeSubcommand::Import { file } => {
            let bytes = bounded(&file, lconnect_image::assets::MAX_ASSET_BYTES)?;
            let normalized = tokio::task::spawn_blocking(move || {
                lconnect_image::assets::normalize_asset(&bytes)
            })
            .await??;
            anyhow::ensure!(
                normalized.len() <= 1024 * 1024,
                "normalized image exceeds 1 MiB upload limit"
            );
            let client = connect().await?;
            let key = client
                .call("import_theme_asset", json!({"bytes":normalized}))
                .await?;
            println!(
                "{}",
                key.as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid asset response"))?
            );
        }
        ThemeSubcommand::State { device } => {
            let client = connect().await?;
            let dev = resolve_device(&client, device).await?;
            let value = client.call("get_lcd_theme", json!({"dev":dev})).await?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        ThemeSubcommand::Apply { config, device } => {
            let value = theme(&config)?;
            let client = connect().await?;
            let dev = resolve_device(&client, device).await?;
            let state = client
                .call("set_lcd_theme", json!({"dev":dev,"theme":value}))
                .await?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        ThemeSubcommand::Preview {
            config,
            output,
            device,
        } => {
            let value = theme(&config)?;
            let client = connect().await?;
            let dev = if device.is_some() {
                Some(resolve_device(&client, device).await?)
            } else {
                None
            };
            let response = client
                .call("preview_lcd_theme", json!({"dev":dev,"theme":value}))
                .await?;
            let jpeg: Vec<u8> = serde_json::from_value(response)?;
            // Preserve an existing file; preview output is explicitly JPEG.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            file.write_all(&jpeg)?;
            println!("JPEG preview: {}", output.display());
        }
    }
    Ok(())
}
