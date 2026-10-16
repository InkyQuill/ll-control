//! LCD configuration patches, source control and diagnostic playback.

use crate::connection::{connect, resolve_device};
use crate::{LcdArgs, LcdSubcommand};
use serde_json::json;

/// Parse a `#RRGGBB` (or `RRGGBB`) hex string into an `[r, g, b]` triple.
fn parse_hex_rgb(s: &str) -> anyhow::Result<[u8; 3]> {
    let stripped = s.strip_prefix('#').unwrap_or(s);
    if stripped.len() != 6 || !stripped.is_ascii() {
        anyhow::bail!("expected #RRGGBB, got {s:?}");
    }
    let r = u8::from_str_radix(&stripped[0..2], 16)
        .map_err(|e| anyhow::anyhow!("invalid red component in {s:?}: {e}"))?;
    let g = u8::from_str_radix(&stripped[2..4], 16)
        .map_err(|e| anyhow::anyhow!("invalid green component in {s:?}: {e}"))?;
    let b = u8::from_str_radix(&stripped[4..6], 16)
        .map_err(|e| anyhow::anyhow!("invalid blue component in {s:?}: {e}"))?;
    Ok([r, g, b])
}

/// Validate / canonicalise an LCD control mode string.
///
/// Mirrors `LCDControlMode` in `crates/protocol/src/galahad2_lcdv2/lcd.rs`.
/// `LCDSetting` and `LCDTest` are intentionally rejected: the former is
/// an internal brightness-update path, the latter is a foot-gun (locks
/// the panel to a solid colour until cleared).
fn validate_mode(mode: &str) -> anyhow::Result<&'static str> {
    match mode {
        "LocalUI" => Ok("LocalUI"),
        "Application" => Ok("Application"),
        "LocalH264" => Ok("LocalH264"),
        "LocalAVI" => Ok("LocalAVI"),
        other => anyhow::bail!(
            "unknown lcd mode: {other:?} (allowed: LocalUI, Application, LocalH264, LocalAVI)"
        ),
    }
}

/// Top-level dispatch for `lconnectctl lcd ...`.
pub async fn run(args: LcdArgs) -> anyhow::Result<()> {
    match args.command {
        LcdSubcommand::Available { device } => available(device).await,
        LcdSubcommand::ImportClip { path, name } => import_clip(path, name).await,
        LcdSubcommand::Clips => list_clips().await,
        LcdSubcommand::DeleteClip { key } => {
            let client = connect().await?;
            client
                .call("delete_prepared_clip", json!({"key":key}))
                .await?;
            println!("Deleted {key}");
            Ok(())
        }
        LcdSubcommand::SelectClip { key, device } => select_clip(key, device).await,
        LcdSubcommand::SelectedClip { device } => print_state("get_lcd_media", device).await,
        LcdSubcommand::PlaySavedClip { key, loops, device } => {
            play_saved_clip(key, loops, device).await
        }
        LcdSubcommand::Brightness { percent, device } => brightness(percent, device).await,
        LcdSubcommand::Rotation { degrees, device } => rotation(degrees, device).await,
        LcdSubcommand::Mode { mode, device } => mode_cmd(mode, device).await,
        LcdSubcommand::Image {
            path,
            fit,
            letterbox_bg,
            quality,
            device,
        } => image(path, fit, letterbox_bg, quality, device).await,
        LcdSubcommand::Clear { device } => clear(device).await,
        LcdSubcommand::State { device } => print_state("get_lcd_state", device).await,
        LcdSubcommand::PlaybackStats { device } => {
            print_state("get_lcd_playback_stats", device).await
        }
        LcdSubcommand::PlayClip {
            path,
            loops,
            device,
        } => play_clip(path, loops, device).await,
    }
}

async fn available(device: Option<String>) -> anyhow::Result<()> {
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let v = client
        .call("is_lcd_available", json!({ "dev": dev }))
        .await?;
    let available = v
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("is_lcd_available returned non-boolean payload: {v}"))?;
    println!("{}", if available { "yes" } else { "no" });
    Ok(())
}

async fn brightness(percent: u8, device: Option<String>) -> anyhow::Result<()> {
    if percent > 100 {
        anyhow::bail!("lcd brightness out of range: {percent} (allowed 0..=100)");
    }
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client
        .call(
            "patch_lcd_config",
            json!({ "dev": dev, "patch": { "brightness_percent": percent } }),
        )
        .await?;
    println!("Set LCD brightness to {percent}%");
    Ok(())
}

async fn rotation(degrees: u16, device: Option<String>) -> anyhow::Result<()> {
    if !matches!(degrees, 0 | 90 | 180 | 270) {
        anyhow::bail!("lcd rotation invalid: {degrees} (allowed: 0, 90, 180, 270)");
    }
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client
        .call(
            "patch_lcd_config",
            json!({ "dev": dev, "patch": { "rotation_degrees": degrees } }),
        )
        .await?;
    println!("Set LCD rotation to {degrees} degrees");
    Ok(())
}

async fn mode_cmd(mode: String, device: Option<String>) -> anyhow::Result<()> {
    let canon = validate_mode(&mode)?;
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client
        .call(
            "patch_lcd_config",
            json!({ "dev": dev, "patch": { "mode": canon } }),
        )
        .await?;
    println!("Set LCD mode to {canon}");
    Ok(())
}

async fn image(
    path: std::path::PathBuf,
    fit: String,
    letterbox_bg: String,
    quality: u8,
    device: Option<String>,
) -> anyhow::Result<()> {
    // Validate quality before doing any I/O.
    let q = lconnect_image::Quality::new(quality)
        .map_err(|e| anyhow::anyhow!("invalid --quality: {e}"))?;

    // Resolve --fit (and --letterbox-bg, which is only consulted when fit=letterbox).
    let fit_mode = match fit.as_str() {
        "crop" => lconnect_image::FitMode::Crop,
        "letterbox" => {
            let bg = parse_hex_rgb(&letterbox_bg)?;
            lconnect_image::FitMode::Letterbox { bg }
        }
        "stretch" => lconnect_image::FitMode::Stretch,
        other => anyhow::bail!("unknown --fit: {other:?} (allowed: crop, letterbox, stretch)"),
    };

    // CPU-heavy: decode + resize + JPEG-encode. Run on a blocking task
    // so we don't stall the tokio runtime.
    let path_for_blocking = path.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        lconnect_image::prepare_jpeg_from_path(&path_for_blocking, fit_mode, q)
    })
    .await
    .map_err(|e| anyhow::anyhow!("image prep task panicked: {e}"))?
    .map_err(|e| anyhow::anyhow!("image prep failed for {}: {e}", path.display()))?;

    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client
        .call(
            "set_lcd_image",
            json!({ "dev": dev, "jpeg": prepared.bytes }),
        )
        .await?;
    println!(
        "Uploaded {} byte JPEG ({}x{}) to LCD",
        prepared.bytes.len(),
        prepared.width,
        prepared.height
    );
    Ok(())
}

async fn clear(device: Option<String>) -> anyhow::Result<()> {
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client.call("clear_lcd", json!({ "dev": dev })).await?;
    println!("Stopped LCD source and sent LocalUI");
    Ok(())
}

async fn print_state(method: &str, device: Option<String>) -> anyhow::Result<()> {
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let state = client.call(method, json!({ "dev": dev })).await?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

async fn play_clip(
    path: std::path::PathBuf,
    loops: u16,
    device: Option<String>,
) -> anyhow::Result<()> {
    let clip = read_clip(path, loops).await?;
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let state = client
        .call(
            "play_lcd_clip",
            json!({ "dev": dev, "clip": clip, "loops": loops }),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

async fn read_clip(path: std::path::PathBuf, loops: u16) -> anyhow::Result<lconnect_ipc::H264Clip> {
    tokio::task::spawn_blocking(move || -> anyhow::Result<lconnect_ipc::H264Clip> {
        use std::io::Read;
        const LIMIT: u64 = 8 * 1024 * 1024;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)?
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() as u64 <= LIMIT, "clip JSON exceeds 8 MiB");
        let clip = serde_json::from_slice(&bytes)?;
        lconnect_ipc::validate_h264_clip(&clip, loops)?;
        Ok(clip)
    })
    .await?
}

async fn import_clip(path: std::path::PathBuf, name: String) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
        "clip name must be nonblank, at most256 UTF-8 bytes and contain no control characters"
    );
    let clip = read_clip(path, 1).await?;
    let client = connect().await?;
    let saved: lconnect_ipc::PreparedClipInfo = serde_json::from_value(
        client
            .call("import_prepared_clip", json!({"name":name,"clip":clip}))
            .await?,
    )?;
    println!("{}", serde_json::to_string_pretty(&saved)?);
    Ok(())
}
async fn list_clips() -> anyhow::Result<()> {
    let client = connect().await?;
    let clips: Vec<lconnect_ipc::PreparedClipInfo> =
        serde_json::from_value(client.call("list_prepared_clips", json!({})).await?)?;
    println!("{}", serde_json::to_string_pretty(&clips)?);
    Ok(())
}
async fn play_saved_clip(key: String, loops: u16, device: Option<String>) -> anyhow::Result<()> {
    anyhow::ensure!(
        key.len() == 32
            && key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid prepared clip key"
    );
    let client = connect().await?;
    let clip: lconnect_ipc::H264Clip = serde_json::from_value(
        client
            .call("load_prepared_clip", json!({"key":key}))
            .await?,
    )?;
    lconnect_ipc::validate_h264_clip(&clip, loops)?;
    let dev = resolve_device(&client, device).await?;
    let state = client
        .call(
            "play_lcd_clip",
            json!({"dev":dev,"clip":clip,"loops":loops}),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

async fn select_clip(key: String, device: Option<String>) -> anyhow::Result<()> {
    anyhow::ensure!(
        key.len() == 32
            && key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid prepared clip key"
    );
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let state = client
        .call("set_lcd_media", json!({"dev":dev,"key":key}))
        .await?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}
