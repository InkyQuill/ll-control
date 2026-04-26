//! LCD subcommands: availability probe, mode/brightness/rotation knobs,
//! JPEG image upload, and clear.
//!
//! ## Wholesale config caveat
//!
//! The Phase 1 IPC surface (`set_lcd_config`) only accepts a full
//! [`lconnect_ipc::LcdConfig`] (mode + brightness + rotation). There is
//! no companion `get_lcd_config` to read the current state, so the
//! `brightness`, `rotation`, and `mode` subcommands all overwrite the
//! two siblings they don't take an argument for. We pick reasonable
//! defaults (mode=Application, brightness=100, rotation=0) and surface
//! a one-line warning on stderr so the user isn't surprised.
//!
//! A future `get_lcd_config` IPC method would let each knob be edited
//! independently. Track that under the daemon-IPC plan.

use crate::connection::connect;
use crate::{LcdArgs, LcdSubcommand};
use serde_json::json;

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

/// Parse a `#RRGGBB` (or `RRGGBB`) hex string into an `[r, g, b]` triple.
fn parse_hex_rgb(s: &str) -> anyhow::Result<[u8; 3]> {
    let stripped = s.strip_prefix('#').unwrap_or(s);
    if stripped.len() != 6 {
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

/// Build a JSON `LcdConfig` matching `lconnect_ipc::LcdConfig`.
fn lcd_config_value(
    mode: &str,
    brightness_percent: u8,
    rotation_degrees: u16,
) -> serde_json::Value {
    json!({
        "mode": mode,
        "brightness_percent": brightness_percent,
        "rotation_degrees": rotation_degrees,
    })
}

/// Top-level dispatch for `lconnectctl lcd ...`.
pub async fn run(args: LcdArgs) -> anyhow::Result<()> {
    match args.command {
        LcdSubcommand::Available { device } => available(device).await,
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
    eprintln!(
        "warning: `lcd brightness` overwrites mode (-> Application) and \
         rotation (-> 0); a future `get_lcd_config` IPC will let knobs be \
         edited independently"
    );
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let cfg = lcd_config_value("Application", percent, 0);
    client
        .call("set_lcd_config", json!({ "dev": dev, "cfg": cfg }))
        .await?;
    println!("Set LCD brightness to {percent}%");
    Ok(())
}

async fn rotation(degrees: u16, device: Option<String>) -> anyhow::Result<()> {
    if !matches!(degrees, 0 | 90 | 180 | 270) {
        anyhow::bail!("lcd rotation invalid: {degrees} (allowed: 0, 90, 180, 270)");
    }
    eprintln!(
        "warning: `lcd rotation` overwrites mode (-> Application) and \
         brightness (-> 100); a future `get_lcd_config` IPC will let knobs \
         be edited independently"
    );
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let cfg = lcd_config_value("Application", 100, degrees);
    client
        .call("set_lcd_config", json!({ "dev": dev, "cfg": cfg }))
        .await?;
    println!("Set LCD rotation to {degrees} degrees");
    Ok(())
}

async fn mode_cmd(mode: String, device: Option<String>) -> anyhow::Result<()> {
    let canon = validate_mode(&mode)?;
    eprintln!(
        "warning: `lcd mode` overwrites brightness (-> 100) and rotation \
         (-> 0); a future `get_lcd_config` IPC will let knobs be edited \
         independently"
    );
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let cfg = lcd_config_value(canon, 100, 0);
    client
        .call("set_lcd_config", json!({ "dev": dev, "cfg": cfg }))
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
    println!("Cleared LCD (firmware reverts to LocalUI ~20s after)");
    Ok(())
}
