//! lighting_keeper -- re-issues current lighting state at fixed cadence.
//!
//! Receives state replacements via mpsc; emits the latest fan and pump
//! state at `reissue_ms` cadence to `hid_io_tx`. This mirrors the C#
//! service behaviour and mitigates the LCDV2 cmd 131 buffering quirk
//! observed on FW 1.6 in Phase 0c -- a single SetPumpLighting write
//! often "sticks" in the device's buffer until the next refresh.
//!
//! On send/reply error, we log and return -- the next tick retries.

use super::{HidCommand, LightingStateMsg, Shutdown};
use crate::persistence::{LightingState, PumpLightingState};
use lconnect_ipc::LightingKind;
use lconnect_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting, Rgb};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection, PumpLightingMode, PumpLightingScope,
};
use lconnect_protocol::galahad2_lcdv2::set_lighting::{
    build_set_fan_lighting_request, build_set_pump_lighting_request,
};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// Spawn the per-device lighting_keeper actor.
///
/// `reissue_ms` is the re-issue cadence in milliseconds (production uses
/// 1500). `initial_fan` / `initial_pump` seed the latest-known state from
/// persistence; either may be `None` until the first `LightingStateMsg`
/// replacement arrives.
///
/// The actor terminates when the shutdown signal flips or `state_rx` is
/// closed.
pub fn spawn(
    hid_io_tx: mpsc::Sender<HidCommand>,
    reissue_ms: u64,
    initial_fan: Option<LightingState>,
    initial_pump: Option<PumpLightingState>,
    mut state_rx: mpsc::Receiver<LightingStateMsg>,
    mut shutdown: Shutdown,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut fan = initial_fan;
        let mut pump = initial_pump;
        let mut interval = tokio::time::interval(Duration::from_millis(reissue_ms));
        // Skip the immediate first tick so a freshly-spawned keeper does
        // not race the test's setup; mirrors C# service start-up timing.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    debug!("lighting_keeper shutting down");
                    break;
                }
                maybe_msg = state_rx.recv() => match maybe_msg {
                    Some(LightingStateMsg::SetFan(s)) => fan = Some(s),
                    Some(LightingStateMsg::SetPump(s)) => pump = Some(s),
                    Some(LightingStateMsg::Clear(LightingKind::Fan)) => fan = None,
                    Some(LightingStateMsg::Clear(LightingKind::Pump)) => pump = None,
                    Some(LightingStateMsg::Clear(other)) => {
                        warn!("lighting_keeper: unknown LightingKind {:?} in Clear; ignoring", other);
                    }
                    None => {
                        debug!("lighting_keeper state_rx closed; exiting");
                        break;
                    }
                },
                _ = interval.tick() => {
                    if let Some(s) = &fan {
                        reissue_fan(&hid_io_tx, s).await;
                    }
                    if let Some(s) = &pump {
                        reissue_pump(&hid_io_tx, s).await;
                    }
                }
            }
        }
    })
}

async fn reissue_fan(hid_io_tx: &mpsc::Sender<HidCommand>, state: &LightingState) {
    let setting = fan_setting_from_state(state);
    let bytes = build_set_fan_lighting_request(setting).to_vec();
    write_and_log(hid_io_tx, bytes, "fan").await;
}

async fn reissue_pump(hid_io_tx: &mpsc::Sender<HidCommand>, state: &PumpLightingState) {
    let setting = pump_setting_from_state(state);
    let bytes = build_set_pump_lighting_request(setting).to_vec();
    write_and_log(hid_io_tx, bytes, "pump").await;
}

async fn write_and_log(hid_io_tx: &mpsc::Sender<HidCommand>, bytes: Vec<u8>, kind: &'static str) {
    let (reply_tx, reply_rx) = oneshot::channel();
    if let Err(e) = hid_io_tx
        .send(HidCommand::Write {
            bytes,
            reply: reply_tx,
        })
        .await
    {
        warn!("lighting_keeper {} send failed: {}", kind, e);
        return;
    }
    match reply_rx.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => warn!("lighting_keeper {} write returned error: {}", kind, e),
        Err(e) => warn!("lighting_keeper {} reply dropped: {}", kind, e),
    }
}

fn fan_setting_from_state(s: &LightingState) -> FanLightingSetting {
    let mut setting = FanLightingSetting {
        mode: parse_fan_mode(&s.mode),
        brightness: s.brightness,
        speed: s.speed,
        direction: parse_direction(&s.direction),
        disabled: s.disabled,
        ..FanLightingSetting::default()
    };
    setting.color1 = parse_hex_color(&s.color);
    setting.source = ArgbSignalSource::Mcu;
    setting
}

fn pump_setting_from_state(s: &PumpLightingState) -> PumpLightingSetting {
    let mut setting = PumpLightingSetting {
        scope: parse_pump_scope(&s.scope),
        mode: parse_pump_mode(&s.mode),
        brightness: s.brightness,
        speed: s.speed,
        direction: parse_direction(&s.direction),
        disabled: s.disabled,
        ..PumpLightingSetting::default()
    };
    setting.color1 = parse_hex_color(&s.color);
    setting.source = ArgbSignalSource::Mcu;
    setting
}

/// Parse a fan-lighting mode string into the wire enum.
///
/// Unknown values default to `Rainbow` (the codec's own `FanLightingSetting`
/// default), with a warning. Matches the lower-cased external IPC vocabulary.
fn parse_fan_mode(s: &str) -> FanLightingMode {
    match s.to_ascii_lowercase().as_str() {
        "rainbow" => FanLightingMode::Rainbow,
        "rainbow_morph" | "rainbowmorph" => FanLightingMode::RainbowMorph,
        "static" | "static_color" | "staticcolor" => FanLightingMode::StaticColor,
        "breathing" => FanLightingMode::Breathing,
        "runway" => FanLightingMode::Runway,
        "meteor" => FanLightingMode::Meteor,
        "" => FanLightingMode::Rainbow,
        other => {
            warn!(
                "lighting_keeper: unknown fan mode {:?}, defaulting to Rainbow",
                other
            );
            FanLightingMode::Rainbow
        }
    }
}

/// Parse a pump-lighting mode string into the wire enum. Unknown -> Rainbow.
fn parse_pump_mode(s: &str) -> PumpLightingMode {
    match s.to_ascii_lowercase().as_str() {
        "rainbow" => PumpLightingMode::Rainbow,
        "rainbow_morph" | "rainbowmorph" => PumpLightingMode::RainbowMorph,
        "static" | "static_color" | "staticcolor" => PumpLightingMode::StaticColor,
        "breathing" => PumpLightingMode::Breathing,
        "runway" => PumpLightingMode::Runway,
        "meteor" => PumpLightingMode::Meteor,
        "ticker_tape" | "tickertape" => PumpLightingMode::TickerTape,
        "fluctuation" => PumpLightingMode::Fluctuation,
        "transmit" => PumpLightingMode::Transmit,
        "colorful_starry_night" | "colorfulstarrynight" => PumpLightingMode::ColorfulStarryNight,
        "static_starry_night" | "staticstarrynight" => PumpLightingMode::StaticStarryNight,
        "voice" => PumpLightingMode::Voice,
        "big_bang" | "bigbang" => PumpLightingMode::BigBang,
        "burst" => PumpLightingMode::Burst,
        "colors_morph" | "colorsmorph" => PumpLightingMode::ColorsMorph,
        "bounce" => PumpLightingMode::Bounce,
        "" => PumpLightingMode::Rainbow,
        other => {
            warn!(
                "lighting_keeper: unknown pump mode {:?}, defaulting to Rainbow",
                other
            );
            PumpLightingMode::Rainbow
        }
    }
}

/// Parse a direction string into the wire enum. Empty / unknown ->
/// `RightOrClockwise` (the codec's default, matching the C# constructor).
fn parse_direction(s: &str) -> LightingDirection {
    match s.to_ascii_lowercase().as_str() {
        "forward" | "right" | "clockwise" | "right_or_clockwise" => {
            LightingDirection::RightOrClockwise
        }
        "reverse"
        | "left"
        | "counter_clockwise"
        | "counterclockwise"
        | "left_or_counter_clockwise" => LightingDirection::LeftOrCounterClockwise,
        "up" => LightingDirection::Up,
        "down" => LightingDirection::Down,
        "spreaded" | "spread" => LightingDirection::Spreaded,
        "gathered" | "gather" => LightingDirection::Gathered,
        "" => LightingDirection::RightOrClockwise,
        other => {
            warn!(
                "lighting_keeper: unknown direction {:?}, defaulting to RightOrClockwise",
                other
            );
            LightingDirection::RightOrClockwise
        }
    }
}

/// Parse a pump scope string into the wire enum. Unknown -> `Inner`.
fn parse_pump_scope(s: &str) -> PumpLightingScope {
    match s.to_ascii_lowercase().as_str() {
        "inner" => PumpLightingScope::Inner,
        "outer" => PumpLightingScope::Outer,
        "all" => PumpLightingScope::All,
        "" => PumpLightingScope::Inner,
        other => {
            warn!(
                "lighting_keeper: unknown pump scope {:?}, defaulting to Inner",
                other
            );
            PumpLightingScope::Inner
        }
    }
}

/// Parse a `#rrggbb` (or `rrggbb`) hex color into an `Rgb`.
///
/// On parse failure, returns `Rgb::BLACK` and logs a warning. This is the
/// safest no-op default -- a black `color1` paired with `disabled = false`
/// effectively produces no visible artifact for solid-color modes.
fn parse_hex_color(s: &str) -> Rgb {
    let trimmed = s.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        if !s.is_empty() {
            warn!(
                "lighting_keeper: malformed hex color {:?} (expected #rrggbb), defaulting to black",
                s
            );
        }
        return Rgb::BLACK;
    }
    let parse = |start: usize| u8::from_str_radix(&trimmed[start..start + 2], 16);
    match (parse(0), parse(2), parse(4)) {
        (Ok(r), Ok(g), Ok(b)) => Rgb { r, g, b },
        _ => {
            warn!(
                "lighting_keeper: non-hex color {:?}, defaulting to black",
                s
            );
            Rgb::BLACK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_with_hash() {
        assert_eq!(
            parse_hex_color("#c4a8ff"),
            Rgb {
                r: 0xc4,
                g: 0xa8,
                b: 0xff,
            }
        );
    }

    #[test]
    fn parse_hex_color_without_hash() {
        assert_eq!(
            parse_hex_color("ff0000"),
            Rgb {
                r: 0xff,
                g: 0x00,
                b: 0x00,
            }
        );
    }

    #[test]
    fn parse_hex_color_invalid_falls_back_to_black() {
        assert_eq!(parse_hex_color("nope"), Rgb::BLACK);
        assert_eq!(parse_hex_color(""), Rgb::BLACK);
        assert_eq!(parse_hex_color("#zzzzzz"), Rgb::BLACK);
    }

    #[test]
    fn parse_fan_mode_known_and_unknown() {
        assert_eq!(parse_fan_mode("static"), FanLightingMode::StaticColor);
        assert_eq!(parse_fan_mode("Rainbow"), FanLightingMode::Rainbow);
        assert_eq!(parse_fan_mode("nope"), FanLightingMode::Rainbow);
    }

    #[test]
    fn parse_direction_forward_and_reverse() {
        assert_eq!(
            parse_direction("forward"),
            LightingDirection::RightOrClockwise
        );
        assert_eq!(
            parse_direction("reverse"),
            LightingDirection::LeftOrCounterClockwise
        );
    }
}
