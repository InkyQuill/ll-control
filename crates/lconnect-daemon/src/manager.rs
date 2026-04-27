//! Device manager: hot-plug poll + per-device actor tree lifecycle.
//!
//! Owns a periodic enumeration loop (cadence `config.poll.hotplug_ms`,
//! default 500 ms) that diffs the latest `backend.enumerate()` result
//! against the live device map. New devices get a per-family stable
//! `DeviceId` (`{family}:{counter:04}`) and the full actor tree spawned
//! by [`spawn_device_actor_tree`]. Disappeared devices are evicted from
//! the map; dropping their `ManagedDevice` runs the per-device
//! `watch::Sender<bool>` `Drop` impl, flipping the shutdown watch and
//! tearing every sibling actor down.
//!
//! Counter allocation is per-family and **per-process** -- across a
//! daemon restart the counter resets. We deliberately do not persist
//! the counter: opaque IDs are only required to be stable within one
//! daemon lifetime.
//!
//! # Phase 1 scope
//!
//! - Single supported family: `galahad2-lcdv2`. `enumerate()` returns
//!   the full list and the manager filters to known families before
//!   diffing.
//! - Replay scope (P1D-15): on every successful actor-tree spawn we
//!   re-issue the persisted fan PWM (cmd 139), pump PWM + source
//!   (cmd 138), and LCD config (cmd 12) with 50 ms spacing, so the
//!   device comes back to its last-known user state after a daemon
//!   restart or hot-replug. Lighting was already replayed via the
//!   keeper's `initial_fan` / `initial_pump` arguments and is *not*
//!   re-issued here. The LCD image (JPEG bytes) is deliberately
//!   **not** replayed -- Hybrid C persistence: image bytes do not
//!   persist across daemon restarts (see `persistence.rs` header).
//!
//! [`spawn_device_actor_tree`]: crate::actor::spawn_device_actor_tree

use crate::actor::{spawn_device_actor_tree, DeviceHandles, HidCommand};
use crate::config::Config;
use crate::persistence::{DeviceSettings, PersistenceStore};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend};
use lconnect_ipc::{DeviceDescriptor, DeviceId};
use lconnect_protocol::galahad2_lcdv2::fan_pwm::build_request as build_fan_pwm_request;
use lconnect_protocol::galahad2_lcdv2::lcd::{
    build_lcd_control_request, Brightness, LCDControlMode, LCDControlSetting, ScreenRotation,
};
use lconnect_protocol::galahad2_lcdv2::pump_pwm::{
    build_request as build_pump_pwm_request, PumpPwmSource,
};
use lconnect_protocol::galahad2_lcdv2::pwm::Pwm as ProtoPwm;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, watch, RwLock};
use tracing::{debug, info, warn};

/// Spacing between successive replay writes. The C# service issues
/// settings commands one-at-a-time and the firmware briefly stops
/// servicing new requests immediately after a write; 50 ms matches
/// the inter-command gap observed in the legacy capture set.
const REPLAY_GAP: Duration = Duration::from_millis(50);

/// Phase 1 supported families. Phase 3+ adds entries here as more
/// `DeviceFamily` impls land in `lconnect-hid::family_registry`.
const SUPPORTED_FAMILIES: &[&str] = &["galahad2-lcdv2"];

/// Marketed display name lookup keyed by family slug.
fn display_name_for(family: &str) -> &'static str {
    match family {
        "galahad2-lcdv2" => "Galahad II Vision (LCDV2)",
        _ => "Unknown Lian Li device",
    }
}

/// Tuple identifying a single physical attachment. The `path` component
/// is what flips on hot-plug (the hidraw node number can differ across
/// re-plug); `serial` differentiates two otherwise-identical devices.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DeviceKey {
    family: String,
    vid: u16,
    pid: u16,
    serial: Option<String>,
    path: PathBuf,
}

impl DeviceKey {
    fn from_info(info: &DeviceInfo) -> Self {
        Self {
            family: info.family.as_str().to_string(),
            vid: info.vid,
            pid: info.pid,
            serial: info.serial.clone(),
            path: info.path.clone(),
        }
    }
}

/// One running per-device actor tree.
///
/// Owns the device's `watch::Sender<bool>` shutdown channel so that the
/// `Drop` impl is the unconditional teardown trigger -- removing this
/// struct from the device map (e.g., on detach) flips the watch and
/// every sibling actor exits its `select!` loop.
pub struct ManagedDevice {
    pub descriptor: DeviceDescriptor,
    pub handles: DeviceHandles,
    /// Held so its `Drop` flips the shutdown watch. Never moved out.
    _shutdown_tx: watch::Sender<bool>,
}

/// Read-side handle to the live device map.
///
/// Cheaply cloneable (`Arc`-backed). Phase 1 IPC server (P1D-11) holds
/// one of these and uses [`Self::list`] / [`Self::with_device`] to
/// service `list_devices` / per-device JSON-RPC methods.
#[derive(Clone)]
pub struct ManagerHandle {
    devices: Arc<RwLock<HashMap<DeviceId, ManagedDevice>>>,
}

impl ManagerHandle {
    /// Snapshot the current device descriptors. Order is unspecified;
    /// IPC callers should sort by `id` if they need a stable view.
    pub async fn list(&self) -> Vec<DeviceDescriptor> {
        self.devices
            .read()
            .await
            .values()
            .map(|d| d.descriptor.clone())
            .collect()
    }

    /// Run `f` against the `ManagedDevice` for `id`, returning its
    /// result. Returns `None` if the device is not currently attached.
    pub async fn with_device<F, R>(&self, id: &DeviceId, f: F) -> Option<R>
    where
        F: FnOnce(&ManagedDevice) -> R,
    {
        self.devices.read().await.get(id).map(f)
    }
}

/// Hot-plug poller + per-device-tree lifecycle.
pub struct DeviceManager {
    backend: Arc<dyn HidBackend>,
    persistence: Arc<PersistenceStore>,
    config: Config,
    /// Per-family attach-order counters. Reset on each daemon process.
    counters: HashMap<String, u32>,
    devices: Arc<RwLock<HashMap<DeviceId, ManagedDevice>>>,
    /// Reverse-lookup: physical key -> opaque DeviceId, so the diff loop
    /// can map a vanished `DeviceInfo` back to the entry to evict.
    by_key: HashMap<DeviceKey, DeviceId>,
}

impl DeviceManager {
    #[must_use]
    pub fn new(
        backend: Arc<dyn HidBackend>,
        persistence: Arc<PersistenceStore>,
        config: Config,
    ) -> Self {
        Self {
            backend,
            persistence,
            config,
            counters: HashMap::new(),
            devices: Arc::new(RwLock::new(HashMap::new())),
            by_key: HashMap::new(),
        }
    }

    /// Spawn the polling task, returning a `ManagerHandle` that shares
    /// the live device map.
    ///
    /// The polling task runs forever (until process exit). On every
    /// `config.poll.hotplug_ms` interval it enumerates, diffs, and
    /// reconciles by spawning new actor trees / dropping vanished ones.
    pub async fn spawn(self) -> ManagerHandle {
        let handle = ManagerHandle {
            devices: self.devices.clone(),
        };
        let mut state = self;
        let poll_ms = state.config.poll.hotplug_ms.max(1);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(poll_ms));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                if let Err(e) = state.poll_once().await {
                    warn!("device_manager poll_once failed: {e:#}");
                }
            }
        });
        handle
    }

    /// One enumerate-diff-reconcile pass. Pulled out of `spawn` so
    /// tests / future signal-driven re-enumeration can call it
    /// directly without restructuring the loop.
    async fn poll_once(&mut self) -> anyhow::Result<()> {
        let enumerated = self.backend.enumerate()?;
        let visible: Vec<DeviceInfo> = enumerated
            .into_iter()
            .filter(|i| SUPPORTED_FAMILIES.contains(&i.family.as_str()))
            .collect();

        let visible_keys: HashMap<DeviceKey, DeviceInfo> = visible
            .into_iter()
            .map(|i| (DeviceKey::from_info(&i), i))
            .collect();

        // Detach: anything in by_key not in visible_keys.
        let to_evict: Vec<DeviceKey> = self
            .by_key
            .keys()
            .filter(|k| !visible_keys.contains_key(*k))
            .cloned()
            .collect();
        for key in to_evict {
            if let Some(id) = self.by_key.remove(&key) {
                // Drop the ManagedDevice -> drops its _shutdown_tx ->
                // the watch flips -> every actor's select! arm fires.
                let evicted = self.devices.write().await.remove(&id);
                if evicted.is_some() {
                    info!("device detached: id={} key={:?}", id.as_str(), key);
                }
            }
        }

        // Attach: anything in visible_keys not in by_key.
        for (key, info) in visible_keys {
            if self.by_key.contains_key(&key) {
                continue;
            }
            match self.attach(info, &key).await {
                Ok(id) => {
                    info!("device attached: id={} key={:?}", id.as_str(), key);
                    self.by_key.insert(key, id);
                }
                Err(e) => {
                    // Don't poison `by_key` -- next poll iteration will
                    // retry the attach (e.g., a transient open() race).
                    warn!("device_manager attach failed for {:?}: {:#}", key, e);
                }
            }
        }

        Ok(())
    }

    async fn attach(&mut self, info: DeviceInfo, key: &DeviceKey) -> anyhow::Result<DeviceId> {
        let family = info.family.clone();
        let id = self.allocate_id(&family);

        // The shutdown watch is owned by the ManagedDevice; the actors
        // hold receivers cloned through spawn_device_actor_tree. When
        // the ManagedDevice is dropped (manager evicts it from the
        // map), the sender's Drop flips the watch.
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Re-read persisted settings so the replay helper sees exactly
        // what the keepers were seeded with. spawn_device_actor_tree
        // already reads them once internally; we accept the second
        // read (one disk hit) to avoid restructuring the actor-tree
        // entry point just to plumb the parsed struct out.
        let settings = self.persistence.read(id.as_str())?;

        let handles = spawn_device_actor_tree(
            self.backend.clone(),
            info.clone(),
            &self.config,
            self.persistence.clone(),
            id.as_str().to_string(),
            shutdown_rx,
        )?;

        // Replay persisted device-level settings (fan PWM, pump PWM,
        // LCD config). Run BEFORE inserting into the live device map
        // so an IPC client cannot race a `set_*` between the actor-
        // tree spawn and the replay -- if the IPC `set_*` lands first,
        // the replay would clobber it.
        replay_persisted_settings(&handles, &settings).await;

        let descriptor = build_descriptor(&id, &info)?;

        let managed = ManagedDevice {
            descriptor,
            handles,
            _shutdown_tx: shutdown_tx,
        };
        self.devices.write().await.insert(id.clone(), managed);
        let _ = key; // key is logged by the caller; kept for symmetry.
        Ok(id)
    }

    fn allocate_id(&mut self, family: &DeviceFamilyId) -> DeviceId {
        let slug = family.as_str();
        let counter = self.counters.entry(slug.to_string()).or_insert(0);
        *counter += 1;
        DeviceId::new(format!("{slug}:{:04}", *counter))
    }
}

/// Replay persisted device-level settings to a freshly-spawned actor
/// tree. Issues, in order with [`REPLAY_GAP`] spacing:
///
/// 1. Fan PWM (cmd 139) -- skipped if `fan.pwm_percent == 0`.
/// 2. Pump PWM (cmd 138) -- skipped if `pump.pwm_percent == 0`.
/// 3. LCD config (cmd 12) -- skipped if the LCD section is at its
///    default (brightness 0 + empty rotation + empty control_mode).
///
/// Lighting is replayed automatically by `lighting_keeper`'s
/// `initial_fan` / `initial_pump` arguments and is **not** re-issued
/// here. The LCD image is deliberately not replayed -- per the
/// Hybrid C persistence model the JPEG bytes are not stored on disk.
///
/// # Why skip on default values
///
/// `DeviceSettings::default()` is the on-attach state for any device
/// the user has never configured (no TOML file on disk, or a fresh
/// device the daemon has not seen before). Sending the all-zero
/// defaults would issue PWM 0 to the fan -- silencing it on every
/// reconnect -- and force the LCD into Mode=LocalUI / Brightness=0,
/// either of which would be a regression vs. the C# service
/// behaviour. We treat "everything at default" as "nothing to
/// replay".
///
/// Failures are logged but never propagated: the replay is a best-
/// effort warm-up. If a write fails, the next IPC `set_*` from a
/// client (or the lighting_keeper's interval re-issue) will retry.
async fn replay_persisted_settings(handles: &DeviceHandles, settings: &DeviceSettings) {
    // Brief settle: actors finish their start-up (hid_io opens the
    // handle, lighting_keeper enters its select! loop) before we
    // start enqueueing writes. 50 ms is generous on a healthy host
    // but tight enough that hot-replug feels responsive.
    tokio::time::sleep(REPLAY_GAP).await;

    if settings.fan.pwm_percent != 0 {
        match ProtoPwm::new(settings.fan.pwm_percent) {
            Ok(pwm) => {
                let bytes = build_fan_pwm_request(pwm).to_vec();
                replay_write(handles, bytes, "fan PWM").await;
                tokio::time::sleep(REPLAY_GAP).await;
            }
            Err(e) => warn!(
                "replay: skipping fan PWM ({}% rejected by Pwm::new: {})",
                settings.fan.pwm_percent, e
            ),
        }
    }

    if settings.pump.pwm_percent != 0 {
        match ProtoPwm::new(settings.pump.pwm_percent) {
            Ok(pwm) => {
                let source = parse_pump_source(&settings.pump.source);
                let bytes = build_pump_pwm_request(source, pwm).to_vec();
                replay_write(handles, bytes, "pump PWM").await;
                tokio::time::sleep(REPLAY_GAP).await;
            }
            Err(e) => warn!(
                "replay: skipping pump PWM ({}% rejected by Pwm::new: {})",
                settings.pump.pwm_percent, e
            ),
        }
    }

    let lcd = &settings.lcd;
    if lcd.brightness != 0 || !lcd.rotation.is_empty() || !lcd.control_mode.is_empty() {
        match Brightness::new(lcd.brightness) {
            Ok(brightness) => {
                // LCDControlSetting is `#[non_exhaustive]`; we cannot
                // use struct-literal syntax from outside the protocol
                // crate (mirrors the pattern already used in
                // `ipc_server::set_lcd_config`). Mutate after default-
                // construction instead.
                let mut setting = LCDControlSetting::default();
                setting.mode = parse_lcd_mode(&lcd.control_mode);
                setting.brightness = brightness;
                setting.rotation = parse_rotation(&lcd.rotation);
                let bytes = build_lcd_control_request(setting).to_vec();
                replay_write(handles, bytes, "LCD config").await;
            }
            Err(e) => warn!(
                "replay: skipping LCD config (brightness {} rejected: {})",
                lcd.brightness, e
            ),
        }
    }
}

/// Issue one `HidCommand::Write` for the replay path. Wait for the
/// reply so a slow first write does not stack up behind the next
/// `REPLAY_GAP` sleep, but log + ignore any failure -- replay is
/// best-effort.
async fn replay_write(handles: &DeviceHandles, bytes: Vec<u8>, label: &'static str) {
    let (reply_tx, reply_rx) = oneshot::channel();
    if let Err(e) = handles
        .hid_io_tx
        .send(HidCommand::Write {
            bytes,
            reply: reply_tx,
        })
        .await
    {
        warn!("replay: {label} send failed (channel closed): {e}");
        return;
    }
    match reply_rx.await {
        Ok(Ok(())) => debug!("replay: {label} re-issued"),
        Ok(Err(e)) => warn!("replay: {label} write failed: {e}"),
        Err(e) => warn!("replay: {label} reply dropped: {e}"),
    }
}

/// Map the persistence string into a `PumpPwmSource`. Empty / unknown
/// values fall back to `Mcu`, mirroring the IPC-server default.
fn parse_pump_source(s: &str) -> PumpPwmSource {
    match s {
        "application" | "Application" => PumpPwmSource::Motherboard,
        // Includes "" (never-set default) and "mcu" / "Mcu".
        _ => PumpPwmSource::Mcu,
    }
}

/// Map the persistence rotation string (a numeric `to_string()` of the
/// degrees) back into the wire-format enum. Empty / unparseable -> 0deg.
fn parse_rotation(s: &str) -> ScreenRotation {
    match s.parse::<u16>().unwrap_or(0) {
        90 => ScreenRotation::Rotate90,
        180 => ScreenRotation::Rotate180,
        270 => ScreenRotation::Rotate270,
        _ => ScreenRotation::Rotate0,
    }
}

/// Map the persistence LCD-mode string back into the wire-format enum.
/// Empty / unknown -> LocalUI. Mirrors `ipc_server::lcd_mode_from_str`.
///
/// LCDTest and LCDSetting are intentionally rejected here as defense in
/// depth: a malformed (e.g. hand-edited) persistence file, or a TOML
/// written before the IPC-side guard landed, must not be able to re-issue
/// a panel-locking mode on every hot-plug. Both parsers must agree.
fn parse_lcd_mode(s: &str) -> LCDControlMode {
    match s {
        "LocalUI" | "local_ui" => LCDControlMode::LocalUI,
        "Application" | "application" => LCDControlMode::Application,
        "LocalH264" | "local_h264" => LCDControlMode::LocalH264,
        "LocalAVI" | "local_avi" => LCDControlMode::LocalAVI,
        _ => {
            warn!("ignoring unknown/unsafe persisted LCD mode '{s}'; falling back to LocalUI");
            LCDControlMode::LocalUI
        }
    }
}

/// Build a `DeviceDescriptor` from the daemon's view of a `DeviceInfo`.
///
/// `DeviceDescriptor` is `#[non_exhaustive]`, so external crates cannot
/// use struct-literal syntax. We round-trip through `serde_json` -- same
/// shape, no future-compat hazard. Mirrors the pattern in
/// `crates/lconnect-ipc/tests/client_server_roundtrip.rs`.
fn build_descriptor(id: &DeviceId, info: &DeviceInfo) -> anyhow::Result<DeviceDescriptor> {
    let family = info.family.as_str();
    let descriptor: DeviceDescriptor = serde_json::from_value(json!({
        "id": id.as_str(),
        "family": family,
        "display_name": display_name_for(family),
        "vid": info.vid,
        "pid": info.pid,
        "serial": info.serial,
        "hid_path": info.path,
    }))?;
    debug!("built descriptor for {}", id.as_str());
    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replay-side parser must accept every mode the IPC layer is
    /// allowed to persist. Mirrors `ipc_server::tests::
    /// lcd_mode_from_str_accepts_safe_modes` so the two parsers can't
    /// drift apart.
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

    /// Defense in depth: even if a malformed TOML lands on disk with
    /// `lcd.control_mode = "LCDTest"`, replay must NOT re-issue the
    /// panel-locking mode on hot-plug. Both parsers must reject the
    /// same set of values.
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
                "unsafe/unknown persisted mode {unsafe_mode:?} must fall back to LocalUI"
            );
        }
    }
}
