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
use crate::codec_str::{
    parse_direction, parse_fan_mode, parse_hex_color, parse_pump_mode, parse_pump_scope,
    percent_to_wire,
};
use crate::persistence::{LightingState, PumpLightingState};
use lconnect_ipc::LightingKind;
use lconnect_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::ArgbSignalSource;
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
        brightness: percent_to_wire(s.brightness),
        speed: percent_to_wire(s.speed),
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
        brightness: percent_to_wire(s.brightness),
        speed: percent_to_wire(s.speed),
        direction: parse_direction(&s.direction),
        disabled: s.disabled,
        ..PumpLightingSetting::default()
    };
    setting.color1 = parse_hex_color(&s.color);
    setting.source = ArgbSignalSource::Mcu;
    setting
}

#[cfg(test)]
mod tests {
    use super::*;
    use lconnect_protocol::galahad2_lcdv2::lighting_enums::{FanLightingMode, LightingDirection};
    use lconnect_protocol::galahad2_lcdv2::Rgb;

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
