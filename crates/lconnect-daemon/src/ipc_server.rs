//! `IpcServerImpl` -- concrete `DaemonRpc` backed by `ManagerHandle` + `Stats`.
//!
//! Each method:
//!
//! 1. Bumps `Stats::ipc_request_count`.
//! 2. Looks up the live `ManagedDevice` via `manager.with_device(id, ...)`,
//!    cloning out the `DeviceHandles` so the read-lock guard does not span
//!    any await points.
//! 3. Builds the wire bytes via the `lconnect-protocol` codecs and ships
//!    them through the appropriate per-device channel (`hid_io_tx` for one-
//!    shot writes, `lcd_frame_tx` for LCD frames, `lighting_state_tx` for
//!    keeper-state replacement, `settings_tx` for debounced disk writes).
//!
//! ## Reset-token registry
//!
//! `request_reset_token` mints a 32-hex-char token, stores `(token, expires_at)`
//! in a `Mutex<HashMap<DeviceId, ...>>`, and returns it. `reset_device`
//! looks the entry up, verifies the token matches and is unexpired, and
//! removes the entry **before** issuing the reset (single-use, even on
//! failure -- the token is consumed by the attempt). Expiry is 30s.
//!
//! ## raw_send gating
//!
//! `raw_send` is gated by `config.daemon.allow_raw_send`. The default is
//! `false`; opting in requires editing `daemon.toml`. When disabled, the
//! method returns `IpcError::NotImplemented`. Cmd 15 (`UploadJpeg`) is
//! deliberately not exposed via `set_lcd_image` -- only cmd 14 (`SendJpeg`)
//! drives the lcd_keeper. See the Phase 0e finding in
//! `docs/protocol/captures/2026-04-25-jpeg-upload-live.txt`.
//!
//! ## In-memory settings cache
//!
//! `DeviceSettings` is cached per-`DeviceId` so `set_*` methods can
//! mutate-and-resend the full struct through `settings_tx` (the
//! command_dispatch actor takes the whole struct, not a delta). The cache
//! is seeded lazily on first access by reading from the on-disk
//! `PersistenceStore`; the actor tree was already spawned with the same
//! settings, so the two views start in sync.

use crate::actor::{DeviceHandles, HidCommand, LcdFrameMsg, LightingStateMsg};
use crate::codec_str::{
    parse_direction, parse_fan_mode, parse_lcd_mode, parse_pump_mode, parse_pump_scope,
    percent_to_wire, rgb_to_hex, rotation_from_degrees,
};
use crate::config::Config;
use crate::manager::ManagerHandle;
use crate::persistence::{
    DeviceSettings, FanSection, LcdSection, LightingSection, LightingState, PersistenceStore,
    PumpLightingState, PumpSection,
};
use crate::stats::Stats;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use lconnect_ipc::{
    DaemonRpc, DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, IpcError,
    LcdConfig, LedTarget, LightingConfig, LightingInfo, LightingKind, Pwm, PwmSource, ResetToken,
    Result, Rgb, SensorSnapshot,
};
use lconnect_protocol::galahad2_lcdv2::commands::Command;
use lconnect_protocol::galahad2_lcdv2::fan_pwm::build_request as build_fan_pwm_request;
use lconnect_protocol::galahad2_lcdv2::firmware::{
    build_request as build_firmware_request, FirmwareVersion,
};
use lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request;
use lconnect_protocol::galahad2_lcdv2::lcd::{
    build_lcd_control_request, Brightness, LCDControlMode, LCDControlSetting,
};
use lconnect_protocol::galahad2_lcdv2::lighting::{
    FanLightingSetting, PumpLightingSetting, Rgb as ProtoRgb,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::ArgbSignalSource;
use lconnect_protocol::galahad2_lcdv2::pump_pwm::{
    build_request as build_pump_pwm_request, PumpPwmSource,
};
use lconnect_protocol::galahad2_lcdv2::pwm::Pwm as ProtoPwm;
use lconnect_protocol::galahad2_lcdv2::reset::{
    build_request as build_reset_request, parse_response as parse_reset_response,
};
use lconnect_protocol::galahad2_lcdv2::set_led_colors::{
    build_set_fan_led_colors_request, build_set_screen_led_colors_request,
};
use lconnect_protocol::galahad2_lcdv2::set_lighting::{
    build_set_fan_lighting_request, build_set_pump_lighting_request,
};
use rand::RngCore;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};
use tokio_stream::wrappers::WatchStream;
use tracing::warn;

/// Single-use reset-token expiry.
const RESET_TOKEN_TTL: Duration = Duration::from_secs(30);

/// Static lighting-mode catalog for the only Phase 1 family.
///
/// `LightingKind::Fan` uses [`FanLightingMode`] (7 variants); `Pump` uses
/// [`PumpLightingMode`] (17 variants). Mirrors the wire enums; if a
/// future firmware adds modes the codec exposes them via `Unknown` and
/// callers can add them to this list without touching the codec.
const FAN_MODES: &[&str] = &[
    "rainbow",
    "rainbow_morph",
    "static",
    "breathing",
    "runway",
    "meteor",
];
const PUMP_MODES: &[&str] = &[
    "rainbow",
    "rainbow_morph",
    "static",
    "breathing",
    "runway",
    "meteor",
    "ticker_tape",
    "fluctuation",
    "transmit",
    "colorful_starry_night",
    "static_starry_night",
    "voice",
    "big_bang",
    "burst",
    "colors_morph",
    "bounce",
];

/// Concrete `DaemonRpc` implementation for the production daemon.
pub struct IpcServerImpl {
    manager: ManagerHandle,
    stats: Arc<Stats>,
    config: Config,
    persistence: Arc<PersistenceStore>,
    /// device_id -> (token, expires_at). One-shot 30s TTL.
    reset_tokens: Mutex<HashMap<DeviceId, (ResetToken, Instant)>>,
    /// device_id -> last-known full settings, mutated in place by set_*
    /// methods and shipped through `settings_tx` for debounced persistence.
    settings_cache: Mutex<HashMap<DeviceId, DeviceSettings>>,
}

impl IpcServerImpl {
    /// Construct a new server. The daemon's `main.rs` (P1D-14) will wire
    /// this up to a `Arc<dyn DaemonRpc>` and feed it to `IpcServer::new`.
    #[must_use]
    pub fn new(
        manager: ManagerHandle,
        stats: Arc<Stats>,
        config: Config,
        persistence: Arc<PersistenceStore>,
    ) -> Self {
        Self {
            manager,
            stats,
            config,
            persistence,
            reset_tokens: Mutex::default(),
            settings_cache: Mutex::default(),
        }
    }

    /// Look up the `DeviceHandles` for `id`, returning
    /// `IpcError::DeviceNotFound` if the device is not currently attached.
    ///
    /// Cloning the handles inside the `with_device` closure releases the
    /// manager's read lock before we suspend on any HID I/O — the
    /// channels themselves are `Arc`-backed so cloning is cheap.
    async fn handles(&self, id: &DeviceId) -> Result<DeviceHandles> {
        self.manager
            .with_device(id, |dev| dev.handles.clone())
            .await
            .ok_or_else(|| IpcError::DeviceNotFound(id.as_str().to_string()))
    }

    /// Read the cached settings for `id`, lazily seeding from disk on
    /// first access. Returns the cloned settings; mutations belong with
    /// [`Self::commit_settings`].
    async fn current_settings(&self, id: &DeviceId) -> DeviceSettings {
        let mut cache = self.settings_cache.lock().await;
        if let Some(s) = cache.get(id) {
            return s.clone();
        }
        let loaded = match self.persistence.read(id.as_str()) {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "ipc_server: persistence read failed for {} ({:#}); using defaults",
                    id.as_str(),
                    e
                );
                DeviceSettings::default()
            }
        };
        cache.insert(id.clone(), loaded.clone());
        loaded
    }

    /// Replace the cached settings and ship a copy to `settings_tx` so
    /// command_dispatch can debounce-write it to disk. A closed channel
    /// (actor tree torn down concurrently with this call) is logged but
    /// not propagated -- the cache update still stands.
    async fn commit_settings(
        &self,
        id: &DeviceId,
        handles: &DeviceHandles,
        settings: DeviceSettings,
    ) {
        self.settings_cache
            .lock()
            .await
            .insert(id.clone(), settings.clone());
        if let Err(e) = handles.settings_tx.send(settings).await {
            warn!(
                "ipc_server: settings_tx send failed for {} ({}); cache updated but disk lag possible",
                id.as_str(),
                e
            );
        }
    }
}

/// Issue an `HidCommand::Write` and await its reply, mapping any error
/// into a generic `IpcError::DeviceBusy` (the only IPC variant for HID
/// I/O failures).
///
/// `Stats::hid_write_count` is bumped inside the hid_io actor on the
/// success path, so this helper does not touch the counter directly --
/// keeper-driven writes (which never enter this helper) need to count
/// too, and centralising the increment at the actor avoids the
/// double-counting hazard the IPC-side increment used to introduce.
async fn write_and_wait(handles: &DeviceHandles, bytes: Vec<u8>) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::Write {
            bytes,
            reply: reply_tx,
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid_io channel closed: {e}")))?;
    match reply_rx.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(IpcError::DeviceBusy(format!("hid write failed: {e}"))),
        Err(e) => Err(IpcError::DeviceBusy(format!("hid reply dropped: {e}"))),
    }
}

/// Issue an `HidCommand::WriteRead64`, decoding any I/O failure to
/// `IpcError::DeviceBusy`. The hid_io actor bumps
/// `Stats::hid_write_count` on the success path; see [`write_and_wait`].
async fn write_read_64(
    handles: &DeviceHandles,
    bytes: Vec<u8>,
    expected_cmd: u8,
) -> Result<[u8; 64]> {
    let (reply_tx, reply_rx) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::WriteRead64 {
            bytes,
            expected_cmd,
            reply: reply_tx,
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid_io channel closed: {e}")))?;
    reply_rx
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid reply dropped: {e}")))?
        .map_err(|e| IpcError::DeviceBusy(format!("hid round-trip failed: {e}")))
}

/// Issue an `HidCommand::WriteRead512`, decoding any I/O failure to
/// `IpcError::DeviceBusy`. The hid_io actor bumps
/// `Stats::hid_write_count` on the success path; see [`write_and_wait`].
///
/// `expected_report_id` selects between B-class (`0x02`) and C-class
/// (`0x03`) framings — the drain loop in `hid_io` matches against this
/// byte to identify the real response among any stale ones still in
/// the device's USB read buffer.
async fn write_read_512(
    handles: &DeviceHandles,
    bytes: Vec<u8>,
    expected_report_id: u8,
    expected_cmd: u8,
) -> Result<[u8; 512]> {
    let (reply_tx, reply_rx) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::WriteRead512 {
            bytes,
            expected_report_id,
            expected_cmd,
            reply: reply_tx,
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid_io channel closed: {e}")))?;
    reply_rx
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid reply dropped: {e}")))?
        .map_err(|e| IpcError::DeviceBusy(format!("hid round-trip failed: {e}")))
}

/// Generate a 16-byte cryptographically-random hex token.
fn mint_reset_token() -> ResetToken {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let s = bytes.iter().fold(String::with_capacity(32), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    ResetToken(s)
}

/// Convert an IPC `Rgb` to the protocol-crate `Rgb` (same shape, different module).
fn rgb_to_proto(c: Rgb) -> ProtoRgb {
    ProtoRgb {
        r: c.r,
        g: c.g,
        b: c.b,
    }
}

#[async_trait]
impl DaemonRpc for IpcServerImpl {
    // -------------------------------------------------------------------
    // Discovery
    // -------------------------------------------------------------------

    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        self.stats.inc_ipc_request();
        Ok(self.manager.list().await)
    }

    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // GetFirmwareVersion (cmd 134) replies with two A-class packets
        // back-to-back. The hid_io actor's drain_for_echo only matches
        // one frame at a time, so we issue two WriteRead64 round-trips
        // sharing the same expected cmd.
        let cmd = Command::GetFirmwareVersion as u8;
        let req = build_firmware_request().to_vec();
        let buf1 = write_read_64(&handles, req, cmd).await?;
        // Second packet: send an empty A-frame? The C# reads two frames
        // for a single write. The drain helper consumes the first echo
        // and stops; we do not yet have a "read-only" path. Mirror the
        // verify-galahad behaviour: re-issue the request to make the
        // device re-emit the pair. We already consumed packet 1, so this
        // round-trip yields packet 2-equivalent (and a leftover packet 1
        // that the next caller will see -- the drain_for_echo loop tolerates
        // up to 4 stale frames).
        let buf2 = write_read_64(&handles, build_firmware_request().to_vec(), cmd).await?;
        let parsed = FirmwareVersion::from_two_responses(&buf1, &buf2)
            .map_err(|e| IpcError::ProtocolMismatch(format!("firmware decode: {e}")))?;
        let version = parsed.user_version.clone();
        let raw_token = format!(
            "{},{},{},{},{},{},{}|{}",
            parsed.chip_code,
            parsed.boot_code,
            parsed.kind,
            parsed.model_a,
            parsed.model_b,
            parsed.software_version,
            parsed.user_version,
            parsed.raw_datetime,
        );
        let value = json!({
            "version": version,
            "raw_token": raw_token,
        });
        serde_json::from_value::<FirmwareInfo>(value)
            .map_err(|e| IpcError::Serialization(e.to_string()))
    }

    // -------------------------------------------------------------------
    // Sensors
    // -------------------------------------------------------------------

    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // `borrow()` is a `Ref`; clone the inner `Copy` value out before
        // dropping the borrow (Ref is not Send).
        let snap = *handles.snapshot_rx.borrow();
        Ok(snap)
    }

    async fn subscribe_snapshots(
        &self,
        dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // `WatchStream` yields the current value once on first poll,
        // then every subsequent change. The IPC server wraps the items
        // we yield here into `"snapshot"` notifications on the same
        // connection (see `lconnect_ipc::server::spawn_snapshot_pump`).
        let stream = WatchStream::new(handles.snapshot_rx);
        Ok(Box::pin(stream))
    }

    // -------------------------------------------------------------------
    // PWM
    // -------------------------------------------------------------------

    async fn set_fan_pwm(&self, dev: DeviceId, percent: Pwm) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let proto_pwm = ProtoPwm::new(percent.percent())
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let bytes = build_fan_pwm_request(proto_pwm).to_vec();
        write_and_wait(&handles, bytes).await?;

        let mut settings = self.current_settings(&dev).await;
        settings.fan = FanSection {
            pwm_percent: percent.percent(),
        };
        self.commit_settings(&dev, &handles, settings).await;
        Ok(())
    }

    async fn set_pump_pwm(&self, dev: DeviceId, percent: Pwm, source: PwmSource) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let proto_pwm = ProtoPwm::new(percent.percent())
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let proto_source = match source {
            PwmSource::Mcu => PumpPwmSource::Mcu,
            PwmSource::Application => PumpPwmSource::Motherboard,
            // PwmSource is #[non_exhaustive]; if the IPC enum gains a
            // variant the daemon hasn't been taught yet, surface it as
            // an explicit caller error rather than silently degrading.
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported PwmSource: {other:?}"
                )))
            }
        };
        let bytes = build_pump_pwm_request(proto_source, proto_pwm).to_vec();
        write_and_wait(&handles, bytes).await?;

        let mut settings = self.current_settings(&dev).await;
        settings.pump = PumpSection {
            pwm_percent: percent.percent(),
            source: match source {
                PwmSource::Mcu => "mcu".into(),
                PwmSource::Application => "application".into(),
                _ => "mcu".into(),
            },
        };
        self.commit_settings(&dev, &handles, settings).await;
        Ok(())
    }

    // -------------------------------------------------------------------
    // Lighting
    // -------------------------------------------------------------------

    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind) -> Result<LightingInfo> {
        self.stats.inc_ipc_request();
        // Lookup is enough to validate the device exists; we do not
        // currently round-trip the get-lighting-info HID command (the
        // catalog is static for galahad2-lcdv2).
        let _ = self.handles(&dev).await?;
        let modes: Vec<String> = match kind {
            LightingKind::Fan => FAN_MODES.iter().map(|s| (*s).to_string()).collect(),
            LightingKind::Pump => PUMP_MODES.iter().map(|s| (*s).to_string()).collect(),
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported LightingKind: {other:?}"
                )))
            }
        };
        Ok(LightingInfo {
            max_speed: 100,
            max_brightness: 100,
            modes,
        })
    }

    async fn set_lighting(
        &self,
        dev: DeviceId,
        kind: LightingKind,
        cfg: LightingConfig,
    ) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let proto_color = rgb_to_proto(cfg.color);

        match kind {
            LightingKind::Fan => {
                let setting = FanLightingSetting {
                    mode: parse_fan_mode(&cfg.mode),
                    // Wire range is 0..=4 (5 levels); user gives 0..=100 percent.
                    // Phase 0c found the device silently ignores out-of-range bytes.
                    brightness: percent_to_wire(cfg.brightness_percent),
                    speed: percent_to_wire(cfg.speed_percent),
                    color1: proto_color,
                    direction: parse_direction(&cfg.direction),
                    disabled: cfg.disabled,
                    source: ArgbSignalSource::Mcu,
                    ..FanLightingSetting::default()
                };
                let bytes = build_set_fan_lighting_request(setting).to_vec();
                write_and_wait(&handles, bytes).await?;

                let state = LightingState {
                    mode: cfg.mode.clone(),
                    brightness: cfg.brightness_percent,
                    speed: cfg.speed_percent,
                    direction: cfg.direction.clone(),
                    color: rgb_to_hex(cfg.color),
                    disabled: cfg.disabled,
                };
                let _ = handles
                    .lighting_state_tx
                    .send(LightingStateMsg::SetFan(state.clone()))
                    .await;

                let mut settings = self.current_settings(&dev).await;
                settings.lighting = LightingSection {
                    fan: Some(state),
                    pump: settings.lighting.pump,
                };
                self.commit_settings(&dev, &handles, settings).await;
            }
            LightingKind::Pump => {
                let scope_str = cfg.scope.clone().unwrap_or_else(|| "inner".to_string());
                let setting = PumpLightingSetting {
                    scope: parse_pump_scope(&scope_str),
                    mode: parse_pump_mode(&cfg.mode),
                    // Wire range is 0..=4; user gives 0..=100 percent. See Fan arm.
                    brightness: percent_to_wire(cfg.brightness_percent),
                    speed: percent_to_wire(cfg.speed_percent),
                    color1: proto_color,
                    direction: parse_direction(&cfg.direction),
                    disabled: cfg.disabled,
                    source: ArgbSignalSource::Mcu,
                    ..PumpLightingSetting::default()
                };
                let bytes = build_set_pump_lighting_request(setting).to_vec();
                write_and_wait(&handles, bytes).await?;

                let state = PumpLightingState {
                    scope: scope_str,
                    mode: cfg.mode.clone(),
                    brightness: cfg.brightness_percent,
                    speed: cfg.speed_percent,
                    direction: cfg.direction.clone(),
                    color: rgb_to_hex(cfg.color),
                    disabled: cfg.disabled,
                };
                let _ = handles
                    .lighting_state_tx
                    .send(LightingStateMsg::SetPump(state.clone()))
                    .await;

                let mut settings = self.current_settings(&dev).await;
                settings.lighting = LightingSection {
                    fan: settings.lighting.fan,
                    pump: Some(state),
                };
                self.commit_settings(&dev, &handles, settings).await;
            }
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported LightingKind: {other:?}"
                )))
            }
        }
        Ok(())
    }

    async fn set_led_colors(
        &self,
        dev: DeviceId,
        target: LedTarget,
        colors: Vec<Rgb>,
    ) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let proto_colors: Vec<ProtoRgb> = colors.into_iter().map(rgb_to_proto).collect();
        let packets = match target {
            LedTarget::Fan => build_set_fan_led_colors_request(&proto_colors)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?,
            LedTarget::Screen => build_set_screen_led_colors_request(&proto_colors)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?,
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported LedTarget: {other:?}"
                )))
            }
        };
        for pkt in packets {
            write_and_wait(&handles, pkt.to_vec()).await?;
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // LCD
    // -------------------------------------------------------------------

    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool> {
        self.stats.inc_ipc_request();
        // Lookup validates the device is attached. galahad2-lcdv2
        // always has an LCD; future families will read this from a
        // family-registry capability bit.
        let _ = self.handles(&dev).await?;
        Ok(true)
    }

    async fn set_lcd_config(&self, dev: DeviceId, cfg: LcdConfig) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let brightness = Brightness::new(cfg.brightness_percent)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        // LCDControlSetting is #[non_exhaustive], so a struct-literal
        // (even with `..Default::default()`) is rejected from outside
        // the protocol crate. Mutate after construction instead.
        let mut setting = LCDControlSetting::default();
        setting.mode = parse_lcd_mode(&cfg.mode);
        setting.brightness = brightness;
        setting.rotation = rotation_from_degrees(cfg.rotation_degrees);
        let bytes = build_lcd_control_request(setting).to_vec();
        write_and_wait(&handles, bytes).await?;

        let mut settings = self.current_settings(&dev).await;
        settings.lcd = LcdSection {
            brightness: cfg.brightness_percent,
            rotation: cfg.rotation_degrees.to_string(),
            control_mode: cfg.mode.clone(),
        };
        self.commit_settings(&dev, &handles, settings).await;
        Ok(())
    }

    async fn set_lcd_image(&self, dev: DeviceId, jpeg: Vec<u8>) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // Validate eagerly so the caller sees InvalidArgument up front
        // rather than the lcd_keeper logging a warn! and silently
        // dropping the frame. Cmd 14 (SendJpeg) only -- cmd 15
        // (UploadJpeg) is IPC-sealed per the Phase 0e finding (no
        // observable LCD effect on FW 1.6).
        let _packets =
            build_send_jpeg_request(&jpeg).map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        handles
            .lcd_frame_tx
            .send(LcdFrameMsg::Set(Bytes::from(jpeg)))
            .await
            .map_err(|e| IpcError::DeviceBusy(format!("lcd_frame_tx closed: {e}")))?;
        Ok(())
    }

    async fn clear_lcd(&self, dev: DeviceId) -> Result<()> {
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;

        // Step 1: stop the lcd_keeper's refresh loop so the next
        // interval tick does not race the LocalUI mode change with
        // another cmd-14 frame push.
        handles
            .lcd_frame_tx
            .send(LcdFrameMsg::Clear)
            .await
            .map_err(|e| IpcError::DeviceBusy(format!("lcd_frame_tx closed: {e}")))?;

        // Step 2: explicitly revert the panel to LocalUI. P1H-1 live
        // smoke discovered the LCDV2 firmware does NOT auto-revert
        // after the keeper stops refreshing -- the panel held the last
        // uploaded JPEG indefinitely. Sending an LCDControl with
        // mode=LocalUI does cause the panel to clear, which is what
        // a "clear" IPC call has to mean for callers.
        //
        // LCDControlSetting is `#[non_exhaustive]`; mutate after default-
        // construction (mirrors set_lcd_config above).
        let mut setting = LCDControlSetting::default();
        setting.mode = LCDControlMode::LocalUI;
        let bytes = build_lcd_control_request(setting).to_vec();
        write_and_wait(&handles, bytes).await?;

        // Step 3: persist the new control_mode so a daemon restart /
        // hot-replug does not silently re-issue the previous (e.g.
        // Application) mode and bring the stale frame back. Brightness
        // and rotation are preserved -- they are user-facing display
        // attributes, not part of "is anything currently shown?".
        let mut settings = self.current_settings(&dev).await;
        settings.lcd = LcdSection {
            brightness: settings.lcd.brightness,
            rotation: settings.lcd.rotation,
            control_mode: "LocalUI".to_string(),
        };
        self.commit_settings(&dev, &handles, settings).await;
        Ok(())
    }

    // -------------------------------------------------------------------
    // Maintenance
    // -------------------------------------------------------------------

    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken> {
        self.stats.inc_ipc_request();
        // Validate the device exists -- minting a token for an absent
        // device is meaningless and would just expire unused.
        let _ = self.handles(&dev).await?;
        let token = mint_reset_token();
        let expires_at = Instant::now() + RESET_TOKEN_TTL;
        self.reset_tokens
            .lock()
            .await
            .insert(dev, (token.clone(), expires_at));
        Ok(token)
    }

    async fn reset_device(&self, dev: DeviceId, confirm: ResetToken) -> Result<()> {
        self.stats.inc_ipc_request();

        // Pop the entry first so the token is single-use even on failure
        // -- a botched confirm should not leave the slot live for a
        // retry attempt with a different token guess.
        let entry = self.reset_tokens.lock().await.remove(&dev);
        let (expected, expires_at) = entry.ok_or(IpcError::ConfirmationRequired)?;
        if expected.0 != confirm.0 || Instant::now() >= expires_at {
            return Err(IpcError::ConfirmationRequired);
        }

        let handles = self.handles(&dev).await?;
        // Reset replies with one A-class packet; use WriteRead64 so we
        // observe the firmware's success ack before returning.
        let cmd = Command::ResetDevice as u8;
        let req = build_reset_request().to_vec();
        let buf = write_read_64(&handles, req, cmd).await?;
        parse_reset_response(&buf)
            .map_err(|e| IpcError::ProtocolMismatch(format!("reset ack: {e}")))?;
        Ok(())
    }

    async fn raw_send(
        &self,
        dev: DeviceId,
        framing: FramingKind,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        self.stats.inc_ipc_request();
        if !self.config.daemon.allow_raw_send {
            return Err(IpcError::NotImplemented(
                "raw_send disabled; set daemon.allow_raw_send=true in daemon.toml to enable".into(),
            ));
        }
        if bytes.len() < 2 {
            return Err(IpcError::InvalidArgument(
                "raw_send: bytes must include at least [report_id, cmd]".into(),
            ));
        }
        let expected_cmd = bytes[1];
        let handles = self.handles(&dev).await?;
        // FramingKind::A => 64-byte read with report_id 0x01.
        // FramingKind::B => 512-byte read with report_id 0x02.
        // FramingKind::C => 512-byte read with report_id 0x03.
        // The drain loop in hid_io matches the report_id to discriminate
        // the real response from stale frames; passing the wrong id
        // poisons the USB buffer for subsequent reads.
        let buf: Vec<u8> = match framing {
            FramingKind::A => write_read_64(&handles, bytes, expected_cmd).await?.to_vec(),
            FramingKind::B => write_read_512(&handles, bytes, 0x02, expected_cmd)
                .await?
                .to_vec(),
            FramingKind::C => write_read_512(&handles, bytes, 0x03, expected_cmd)
                .await?
                .to_vec(),
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported FramingKind: {other:?}"
                )))
            }
        };
        Ok(buf)
    }

    async fn daemon_stats(&self) -> Result<DaemonStats> {
        self.stats.inc_ipc_request();
        let n = u32::try_from(self.manager.list().await.len()).unwrap_or(u32::MAX);
        Ok(self.stats.snapshot(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lconnect_protocol::galahad2_lcdv2::lcd::LCDControlMode;

    /// Sanity check that the public LCD modes still round-trip through the
    /// IPC parser. If LocalUI/Application/LocalH264/LocalAVI ever stop
    /// matching, this test fails before we ship a daemon that can't
    /// honour valid IPC requests.
    #[test]
    fn parse_lcd_mode_accepts_safe_modes() {
        assert!(matches!(parse_lcd_mode("LocalUI"), LCDControlMode::LocalUI));
        assert!(matches!(
            parse_lcd_mode("Application"),
            LCDControlMode::Application
        ));
        assert!(matches!(
            parse_lcd_mode("LocalH264"),
            LCDControlMode::LocalH264
        ));
        assert!(matches!(
            parse_lcd_mode("LocalAVI"),
            LCDControlMode::LocalAVI
        ));
    }

    /// LCDTest and LCDSetting are firmware-internal modes that lock the
    /// panel until explicitly cleared; they MUST NOT be reachable through
    /// the IPC parser, regardless of casing/snake-casing. If this test
    /// ever flips, a direct IPC caller can persist a mode that re-bricks
    /// the panel on every hot-plug.
    #[test]
    fn parse_lcd_mode_rejects_unsafe_modes() {
        for unsafe_mode in [
            "LCDTest",
            "lcd_test",
            "LCDSetting",
            "lcd_setting",
            "garbage",
            "",
        ] {
            assert!(
                matches!(parse_lcd_mode(unsafe_mode), LCDControlMode::LocalUI),
                "unsafe/unknown mode {unsafe_mode:?} must fall back to LocalUI"
            );
        }
    }
}
