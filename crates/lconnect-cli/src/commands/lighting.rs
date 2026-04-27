//! `lighting` and `led-colors` subcommands.
//!
//! `lighting set <fan|pump>` builds a `LightingConfig` and issues the
//! `set_lighting` IPC method. `lighting clear <kind>` is the same call
//! with `disabled = true` (the daemon does not currently expose a separate
//! `clear_lighting` method — see `lconnect-ipc/src/trait_def.rs`).
//! `lighting info <kind>` calls `get_lighting_info` and prints the
//! supported modes / parameter bounds.
//!
//! `led-colors <fan|screen>` pushes a per-LED colour vector via
//! `set_led_colors`. The colour list is either a single `--color` repeated
//! `--count` times or a `--from-file` containing one `#RRGGBB` per line.

use crate::connection::{connect, resolve_device};
use crate::{LedColorsArgs, LightingArgs, LightingSubcommand};
use anyhow::{bail, Context, Result};
use lconnect_ipc::types::{LightingInfo, Rgb};
use serde_json::json;

/// Validate that `kind` is one of `fan` or `pump`.
fn validate_lighting_kind(kind: &str) -> Result<&'static str> {
    match kind {
        "fan" => Ok("fan"),
        "pump" => Ok("pump"),
        other => bail!("unknown lighting kind: {other} (allowed: fan, pump)"),
    }
}

/// Validate that `target` is one of `fan` or `screen`.
fn validate_led_target(target: &str) -> Result<&'static str> {
    match target {
        "fan" => Ok("fan"),
        "screen" => Ok("screen"),
        other => bail!("unknown LED target: {other} (allowed: fan, screen)"),
    }
}

/// Validate that `direction` is one of `forward` or `reverse`.
fn validate_direction(direction: &str) -> Result<&'static str> {
    match direction {
        "forward" => Ok("forward"),
        "reverse" => Ok("reverse"),
        other => bail!("unknown direction: {other} (allowed: forward, reverse)"),
    }
}

/// Range-check a percent value (0..=100) using `label` for the error.
fn validate_percent(label: &str, percent: u8) -> Result<()> {
    if percent > 100 {
        bail!("{label} out of range: {percent} (allowed 0..=100)");
    }
    Ok(())
}

/// Parse a hex colour string into an [`Rgb`].
///
/// Accepts `#RRGGBB` and `RRGGBB` (case-insensitive). Returns an error
/// for any other shape.
fn parse_hex_color(s: &str) -> Result<Rgb> {
    let trimmed = s.strip_prefix('#').unwrap_or(s);
    if trimmed.len() != 6 {
        bail!("invalid color {s:?}: expected #RRGGBB or RRGGBB");
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16)
        .with_context(|| format!("invalid red channel in color {s:?}"))?;
    let g = u8::from_str_radix(&trimmed[2..4], 16)
        .with_context(|| format!("invalid green channel in color {s:?}"))?;
    let b = u8::from_str_radix(&trimmed[4..6], 16)
        .with_context(|| format!("invalid blue channel in color {s:?}"))?;
    Ok(Rgb { r, g, b })
}

/// Read a colour list from a file: one `#RRGGBB` per line.
///
/// Blank lines and lines whose first non-whitespace char is `#` *and* whose
/// content is not a valid hex colour are treated as comments. (A bare
/// `#RRGGBB` line is parsed normally.)
fn parse_color_file(path: &std::path::Path) -> Result<Vec<Rgb>> {
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read color file {}", path.display()))?;
    let mut out = Vec::new();
    for (lineno, raw) in body.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // A line that starts with `#` is either a comment or a #RRGGBB
        // colour; disambiguate by trying to parse it.
        if line.starts_with('#') && line.len() != 7 {
            // Treat as comment when the length doesn't match `#RRGGBB`.
            continue;
        }
        let rgb = parse_hex_color(line).with_context(|| {
            format!(
                "failed to parse color file {} line {}",
                path.display(),
                lineno + 1
            )
        })?;
        out.push(rgb);
    }
    Ok(out)
}

/// Default LED count for a target when `--count` is not provided.
///
/// Placeholder values pending a daemon-side query; users should pass
/// `--count` explicitly when they want certainty.
fn default_led_count(target: &str) -> usize {
    match target {
        "fan" => 8,
        "screen" => 16,
        _ => 1,
    }
}

/// `lconnectctl lighting <set|clear|info> ...`.
pub async fn run(args: LightingArgs) -> Result<()> {
    match args.command {
        LightingSubcommand::Set {
            kind,
            mode,
            color,
            speed,
            brightness,
            direction,
            scope,
            disable,
            device,
        } => {
            set_lighting(SetLighting {
                kind,
                mode,
                color,
                speed,
                brightness,
                direction,
                scope,
                disable,
                device,
            })
            .await
        }
        LightingSubcommand::Clear { kind, device } => clear_lighting(kind, device).await,
        LightingSubcommand::Info { kind, device } => info_lighting(kind, device).await,
    }
}

/// Internal arg bundle for [`set_lighting`]. Exists only to keep the
/// function under clippy's `too_many_arguments` lint cleanly.
struct SetLighting {
    kind: String,
    mode: String,
    color: Option<String>,
    speed: Option<u8>,
    brightness: Option<u8>,
    direction: Option<String>,
    scope: Option<String>,
    disable: bool,
    device: Option<String>,
}

async fn set_lighting(args: SetLighting) -> Result<()> {
    let kind = validate_lighting_kind(&args.kind)?;

    let color = match args.color.as_deref() {
        Some(s) => parse_hex_color(s)?,
        None => Rgb { r: 0, g: 0, b: 0 },
    };
    let brightness = args.brightness.unwrap_or(100);
    validate_percent("brightness", brightness)?;
    let speed = args.speed.unwrap_or(50);
    validate_percent("speed", speed)?;
    let direction = match args.direction.as_deref() {
        Some(d) => validate_direction(d)?.to_string(),
        None => "forward".to_string(),
    };

    let cfg = json!({
        "mode": args.mode,
        "color": { "r": color.r, "g": color.g, "b": color.b },
        "brightness_percent": brightness,
        "speed_percent": speed,
        "direction": direction,
        "scope": args.scope,
        "disabled": args.disable,
    });

    let client = connect().await?;
    let dev = resolve_device(&client, args.device).await?;
    client
        .call(
            "set_lighting",
            json!({ "dev": dev, "kind": kind, "cfg": cfg }),
        )
        .await
        .context("set_lighting RPC failed")?;
    println!("Lighting updated ({kind}: {})", args.mode);
    Ok(())
}

async fn clear_lighting(kind_arg: String, device: Option<String>) -> Result<()> {
    let kind = validate_lighting_kind(&kind_arg)?;
    let cfg = json!({
        "mode": "Off",
        "color": { "r": 0, "g": 0, "b": 0 },
        "brightness_percent": 0_u8,
        "speed_percent": 0_u8,
        "direction": "forward",
        "scope": serde_json::Value::Null,
        "disabled": true,
    });

    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    client
        .call(
            "set_lighting",
            json!({ "dev": dev, "kind": kind, "cfg": cfg }),
        )
        .await
        .context("set_lighting RPC failed (clear)")?;
    println!("Lighting cleared ({kind})");
    Ok(())
}

async fn info_lighting(kind_arg: String, device: Option<String>) -> Result<()> {
    let kind = validate_lighting_kind(&kind_arg)?;
    let client = connect().await?;
    let dev = resolve_device(&client, device).await?;
    let v = client
        .call("get_lighting_info", json!({ "dev": dev, "kind": kind }))
        .await
        .context("get_lighting_info RPC failed")?;
    let info: LightingInfo = serde_json::from_value(v)
        .context("failed to deserialise get_lighting_info response as LightingInfo")?;
    println!("Lighting info ({kind}):");
    println!("  Max speed:      {}", info.max_speed);
    println!("  Max brightness: {}", info.max_brightness);
    println!("  Modes:          {}", info.modes.join(", "));
    Ok(())
}

/// `lconnectctl led-colors <fan|screen> [--color #RRGGBB --count N | --from-file PATH]`.
pub async fn led_colors(args: LedColorsArgs) -> Result<()> {
    let target = validate_led_target(&args.target)?;

    let colors: Vec<Rgb> = match (args.color.as_deref(), args.from_file.as_deref()) {
        (Some(_), Some(_)) => {
            // clap's `conflicts_with` should already prevent this, but
            // guard anyway for direct call sites / future refactors.
            bail!("--color and --from-file are mutually exclusive");
        }
        (None, None) => bail!("provide either --color or --from-file"),
        (Some(hex), None) => {
            let rgb = parse_hex_color(hex)?;
            let count = args.count.unwrap_or_else(|| default_led_count(target));
            if count == 0 {
                bail!("--count must be > 0");
            }
            vec![rgb; count]
        }
        (None, Some(path)) => {
            let parsed = parse_color_file(path)?;
            if parsed.is_empty() {
                bail!("color file {} contained no colors", path.display());
            }
            parsed
        }
    };

    let client = connect().await?;
    let dev = resolve_device(&client, args.device).await?;
    client
        .call(
            "set_led_colors",
            json!({
                "dev": dev,
                "target": target,
                "colors": colors,
            }),
        )
        .await
        .context("set_led_colors RPC failed")?;
    println!("Pushed {} colors to {target}", colors.len());
    Ok(())
}
