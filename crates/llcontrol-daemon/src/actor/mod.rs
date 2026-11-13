//! Per-device actor tree.
//!
//! One `DeviceActorSet` per connected device, spawned by `manager.rs`.
//! Each actor lives on its own tokio task; channels stitch them together.

pub mod command_dispatch;
pub mod control_group;
pub mod cooling;
pub mod firmware_probe;
pub mod firmware_update;
pub mod group_barrier;
pub mod hid_io;
pub mod keeper_lifecycle;
pub mod lcd_keeper;
pub mod lcd_state;
pub mod lighting_keeper;
pub mod screen_group;
pub mod sensor_poll;
pub mod thermal;

use crate::config::Config;
use crate::persistence::{DeviceSettings, PersistenceStore};
use crate::stats::Stats;
use llcontrol_hid::{DeviceInfo, HidBackend};
use llcontrol_ipc::SensorSnapshot;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

/// Owned per-device shutdown signal. Drop the sender to terminate all actors.
pub type Shutdown = watch::Receiver<bool>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReadPolicy {
    None,
    ObserveOnce512 { timeout_ms: i32 },
}

#[derive(Debug)]
pub struct FrameOutcome {
    pub written_reports: usize,
    pub readback: Vec<u8>,
    pub elapsed: Duration,
}

/// Firmware responses collected from one request, classified by packet number.
#[derive(Debug)]
pub struct FirmwareResponses {
    /// Required PN0 version report.
    pub version: [u8; 64],
    /// Optional PN1 raw build datetime report.
    pub datetime: Option<[u8; 64]>,
}

/// Commands sent through `hid_io_tx` (single-consumer).
///
/// Constructed by `IpcServerImpl` (P1D-11) and the keepers; the manager
/// itself only routes these channels, it does not produce variants.
// `WriteRead64` / `WriteRead512` exist for future IPC consumers
// (firmware probe, raw_send, snapshot subscribers); the keepers
// landed in P1D-5..P1D-9 only need `Write`. The `expected_cmd` /
// `reply` fields are used by `hid_io` itself.
#[allow(dead_code)]
#[derive(Debug)]
pub enum HidCommand {
    /// Internal update transport. Permanently fences this actor's attachment.
    /// Not exposed by IPC until keeper/reconnect coordination is implemented.
    FirmwareUpdate {
        operation: Box<crate::firmware_operation::ArmedOperation>,
        image: firmware_update::ValidatedImage,
        cooling_hold: firmware_update::CoolingHold,
        cancel: Shutdown,
        progress: watch::Sender<firmware_update::Progress>,
        reply: oneshot::Sender<firmware_update::Outcome>,
    },
    /// One firmware query and bounded PN0/PN1 collection, without another writer between reads.
    Firmware {
        reply: oneshot::Sender<llcontrol_hid::Result<FirmwareResponses>>,
    },
    /// Write all reports as one indivisible HID transaction, optionally
    /// retaining one raw diagnostic observation after the final write.
    FrameBatch {
        packets: Vec<Vec<u8>>,
        read_policy: FrameReadPolicy,
        reply: oneshot::Sender<llcontrol_hid::Result<FrameOutcome>>,
    },
    /// Write bytes to the device, no response read.
    Write {
        bytes: Vec<u8>,
        reply: oneshot::Sender<llcontrol_hid::Result<()>>,
    },
    /// Request sensors while permitting frame transactions before the response arrives.
    PollSensors {
        reply: oneshot::Sender<llcontrol_hid::Result<[u8; 64]>>,
    },
    /// Write then read one response (A-class).
    WriteRead64 {
        bytes: Vec<u8>,
        expected_cmd: u8,
        reply: oneshot::Sender<llcontrol_hid::Result<[u8; 64]>>,
    },
    /// Write then read one response (512-byte; B-class or C-class).
    ///
    /// `expected_report_id` distinguishes the two framings: `0x02` for
    /// `FramingKind::B` and `0x03` for `FramingKind::C`. Hardcoding either
    /// here would cause the drain loop to discard real responses on the
    /// other class and leave the device's USB buffer poisoned with
    /// the actual reply for the next unrelated read to consume.
    WriteRead512 {
        bytes: Vec<u8>,
        expected_report_id: u8,
        expected_cmd: u8,
        reply: oneshot::Sender<llcontrol_hid::Result<[u8; 512]>>,
    },
}

/// Per-device handles exposed to the IPC layer.
///
/// `Clone` is cheap (every field is itself an `Arc`-backed channel handle)
/// and lets the IPC server lift a snapshot of the handles out of the
/// `ManagerHandle::with_device` closure so subsequent `.await` calls on
/// `mpsc::Sender::send` / `oneshot::Receiver` do not have to be performed
/// while still holding the manager's `RwLock` read guard.
#[derive(Clone)]
pub struct DeviceHandles {
    /// Stop/join producers while retaining the attachment's original HID actor.
    pub keepers: keeper_lifecycle::KeeperLifecycle,
    pub(crate) persistence_key: String,
    pub control_group_tx: mpsc::Sender<control_group::ControlGroupCommand>,
    pub save_state: watch::Receiver<llcontrol_ipc::SettingsSaveState>,
    pub cooling_tx: mpsc::Sender<cooling::CoolingCommand>,
    pub sensor_cache: crate::sensors::SensorCache,
    pub hid_io_tx: mpsc::Sender<HidCommand>,
    pub snapshot_rx: watch::Receiver<SensorSnapshot>,
    pub settings_tx: mpsc::Sender<command_dispatch::SettingsUpdate>,
    pub lcd_tx: mpsc::Sender<LcdCommand>,
    pub teardown_rx: watch::Receiver<bool>,
    pub lighting_state_tx: mpsc::Sender<LightingStateMsg>,
}

impl DeviceHandles {
    /// Producer/cleanup/save preflight only. Does not authorize flash: global
    /// admission, cooling hold, durable outcome and reconnect gate remain needed.
    pub async fn quiesce_checked(&self) -> Result<(), keeper_lifecycle::QuiesceError> {
        self.keepers
            .quiesce()
            .await
            .map_err(keeper_lifecycle::QuiesceError::Cleanup)?;
        let save = self.save_state.borrow().clone();
        if save.pending || save.last_error.is_some() {
            return Err(keeper_lifecycle::QuiesceError::Persistence {
                pending: save.pending,
                last_error: save.last_error,
            });
        }
        Ok(())
    }
}

/// A durable reset can succeed even when applying it to the display fails.
/// Inspect lcd.last_error; repeated requests never apply old content again.
pub use llcontrol_ipc::ScreenResetResult as ScreenResetState;

/// Admission preserves newer display intent when a completed operation is retried.
#[derive(Debug)]
pub enum ScreenResetAdmission {
    Repeated(ScreenResetState),
    Prepare(u64),
}

/// Serialized display operations. Replies are delivered after the transaction.
#[derive(Debug)]
pub enum LcdCommand {
    SetMediaComposition {
        key: String,
        prepared: crate::media_composition::PreparedComposition,
        preparation: Option<u64>,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    GroupState {
        reply: oneshot::Sender<screen_group::ScreenGroupState>,
    },
    BeginScreenGroupReapply {
        operation_id: String,
        expected: group_barrier::RuntimeRevision,
        reply: oneshot::Sender<llcontrol_ipc::Result<screen_group::ScreenReapplyAdmission>>,
    },
    ReapplyScreenGroupChecked {
        operation_id: String,
        expected: group_barrier::RuntimeRevision,
        expected_generation: u64,
        preparation: u64,
        prepared: screen_group::PreparedScreen,
        reply: oneshot::Sender<llcontrol_ipc::Result<screen_group::ScreenReapplyResult>>,
    },
    CommitScreenGroup {
        operation_id: String,
        expected: crate::profile_group::GroupRevision,
        preparation: Option<u64>,
        prepared: screen_group::PreparedScreenGroup,
        reply: oneshot::Sender<llcontrol_ipc::Result<screen_group::ScreenGroupResult>>,
    },
    ReapplyScreenGroup {
        operation_id: String,
        expected_generation: u64,
        preparation: u64,
        prepared: screen_group::PreparedScreen,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },

    PrepareScreenResetReapply {
        operation_id: String,
        expected_generation: u64,
        reply: oneshot::Sender<
            llcontrol_ipc::Result<(std::sync::Arc<crate::theme_runtime::ThemeRuntime>, u64)>,
        >,
    },
    ReapplyScreenReset {
        operation_id: String,
        expected_generation: u64,
        preparation: u64,
        prepared: crate::theme_runtime::PreparedTheme,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    ReadScreenReset {
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::ScreenResetReadback>>,
    },
    BeginScreenReset {
        operation_id: String,
        reply: oneshot::Sender<llcontrol_ipc::Result<ScreenResetAdmission>>,
    },
    ResetScreen {
        preparation: Option<u64>,
        operation_id: String,
        expected_profile_revision: u64,
        expected_collection_revision: u64,
        prepared: crate::theme_runtime::PreparedTheme,
        reply: oneshot::Sender<llcontrol_ipc::Result<ScreenResetState>>,
    },
    SetComposition {
        prepared: crate::media_composition::PreparedComposition,
        preparation: Option<u64>,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    SetMediaClip {
        key: String,
        source: crate::media_assets::PinnedMedia,
        preparation: Option<u64>,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    RestoreMediaError {
        key: String,
        error: String,
        reply: oneshot::Sender<()>,
    },
    MediaClip {
        reply: oneshot::Sender<Option<String>>,
    },
    RestoreThemeError {
        theme: llcontrol_ipc::LcdTheme,
        error: String,
        reply: oneshot::Sender<()>,
    },
    /// Reserve an intent before loading assets; newer source changes invalidate it.
    BeginThemePreparation { reply: oneshot::Sender<u64> },
    SetTheme {
        preparation: Option<u64>,
        prepared: crate::theme_runtime::PreparedTheme,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    Theme {
        reply: oneshot::Sender<Option<llcontrol_ipc::LcdTheme>>,
    },
    SetImagePrepared {
        preparation: u64,
        prepared: crate::media_composition::PreparedComposition,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    SetImage {
        jpeg: bytes::Bytes,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    Patch {
        patch: llcontrol_ipc::LcdConfigPatch,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    Clear {
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    State {
        reply: oneshot::Sender<llcontrol_ipc::LcdState>,
    },
    PlayClip {
        clip: llcontrol_ipc::H264Clip,
        loops: u16,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::LcdState>>,
    },
    Stats {
        reply: oneshot::Sender<llcontrol_ipc::LcdPlaybackStats>,
    },
}

#[derive(Debug)]
pub struct LightingBase {
    pub fan_topology: Option<llcontrol_ipc::FanLightingTopology>,
    pub current: Option<llcontrol_ipc::LightingConfig>,
    pub profiles: Vec<llcontrol_ipc::LightingConfig>,
}

#[derive(Debug)]
pub struct LightingRuntimeState {
    pub thermal: llcontrol_ipc::ThermalState,
    pub dynamic: llcontrol_ipc::DynamicRgbState,
}

/// Lighting-keeper state replacement.
#[derive(Debug)]
pub enum LightingStateMsg {
    GroupState(oneshot::Sender<(group_barrier::RuntimeRevision, LightingRuntimeState)>),
    PrepareGroup {
        barrier: group_barrier::GroupBarrier,
        group: Arc<crate::profile_group::FanAndLightingGroup>,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    InstallGroup {
        barrier: group_barrier::GroupBarrier,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    ApplyGroup {
        barrier: group_barrier::GroupBarrier,
        reply: oneshot::Sender<llcontrol_ipc::Result<LightingRuntimeState>>,
    },
    FreezeGroup {
        barrier: group_barrier::GroupBarrier,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    Base {
        kind: llcontrol_ipc::LightingKind,
        reply: oneshot::Sender<LightingBase>,
    },
    DynamicRgbState(oneshot::Sender<llcontrol_ipc::DynamicRgbState>),
    SetDynamicRgb {
        config: Option<llcontrol_ipc::DynamicRgbConfig>,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::DynamicRgbState>>,
    },
    ThermalState(oneshot::Sender<llcontrol_ipc::ThermalState>),
    SetThermal {
        config: llcontrol_ipc::ThermalConfig,
        reply: oneshot::Sender<llcontrol_ipc::Result<llcontrol_ipc::ThermalState>>,
    },
    SetFan {
        state: crate::persistence::LightingState,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    SetPumpWithDynamic {
        state: crate::persistence::PumpLightingState,
        restore_dynamic: bool,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    SetPump {
        state: crate::persistence::PumpLightingState,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    Direct {
        target: llcontrol_ipc::LedTarget,
        colors: Vec<llcontrol_ipc::Rgb>,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
    Clear {
        kind: llcontrol_ipc::LightingKind,
        reply: oneshot::Sender<llcontrol_ipc::Result<()>>,
    },
}

/// Spawn the full per-device actor tree.
///
/// Opens the HID handle via `backend.open(&info)`, then spawns the
/// five sibling actors (hid_io, sensor_poll, lighting_keeper,
/// lcd_keeper, command_dispatch) seeded from `initial_settings`. The
/// returned `DeviceHandles` is the daemon's only reference to the
/// running tree -- dropping it (or flipping the shared `shutdown`
/// watch) tears the whole tree down.
///
/// The keepers' "initial state" is taken from `initial_settings`,
/// which IS replay-on-spawn for lighting (the keepers re-issue their
/// initial fan/pump state on their first interval tick). Explicit
/// fan/pump PWM and LCD-config replay -- which are not auto-driven by
/// any keeper -- land in P1D-15.
///
/// # Why `initial_settings` is a parameter (not read here)
///
/// `manager::attach` performs a single `persistence.read(...)` and
/// passes the same `DeviceSettings` instance into both this function
/// (for keeper seeding) and `replay_persisted_settings` (for fan /
/// pump / LCD writes). Reading twice would open a TOCTOU window where
/// a sibling write between the two reads could leave the keepers
/// seeded with state-A while replay sends state-B for fan/pump.
/// `PersistenceStore` is still passed through because
/// `command_dispatch` needs it for its own debounced writes.
///
/// # Errors
///
/// Returns `Err` if `backend.open(&info)` fails (typically
/// `Error::DeviceNotFound` if the hidraw path was racing detach).
// The arity is load-bearing: every parameter is a distinct dependency
// (backend, info, config, persistence, persistence_key, initial_settings,
// stats, shutdown). Bundling them into a struct just to mollify clippy
// would obscure the call site, so we silence the lint here instead.
#[allow(clippy::too_many_arguments)]
pub fn spawn_device_actor_tree(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    config: &Config,
    persistence: Arc<PersistenceStore>,
    persistence_key: String,
    initial_settings: DeviceSettings,
    stats: Arc<Stats>,
    shutdown: Shutdown,
) -> anyhow::Result<DeviceHandles> {
    spawn_device_actor_tree_with_sensors(
        backend,
        info,
        config,
        persistence,
        persistence_key,
        initial_settings,
        stats,
        shutdown,
        Default::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_device_actor_tree_with_sensors(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    config: &Config,
    persistence: Arc<PersistenceStore>,
    persistence_key: String,
    initial_settings: DeviceSettings,
    stats: Arc<Stats>,
    shutdown: Shutdown,
    host: crate::sensors::SensorCache,
) -> anyhow::Result<DeviceHandles> {
    let (hid_io_tx, hid_handle) = hid_io::spawn_tracked_with_journal(
        backend,
        info,
        stats,
        shutdown.clone(),
        persistence.firmware_operations(),
    )?;
    let initially_stopped = *shutdown.borrow() || shutdown.has_changed().is_err();
    let (keepers, keeper_shutdown, keepers_joined) = keeper_lifecycle::channel(initially_stopped);
    let root_forwarder = keeper_lifecycle::forward(shutdown, &keepers);

    // sensor_poll: poll period derived from sensor_hz (clamped >=1 to
    // avoid div-by-zero if config arrives with sensor_hz = 0).
    let sensor_hz = config.poll.sensor_hz.max(1);
    let sensor_period_ms = 1000u64 / u64::from(sensor_hz);
    let sensor_cache = crate::sensors::SensorCache::default();
    let (snapshot_rx, sensor_handle) = sensor_poll::spawn_with_cache_tracked(
        hid_io_tx.clone(),
        sensor_period_ms.max(1),
        keeper_shutdown.clone(),
        sensor_cache.clone(),
        persistence_key.clone(),
    );

    let (settings_tx, settings_rx) = mpsc::channel(16);
    let (cooling_tx, cooling_handle) = cooling::spawn(
        cooling::CoolingInputs {
            hid: hid_io_tx.clone(),
            settings: settings_tx.clone(),
            host: host.clone(),
            device: sensor_cache.clone(),
            initial: initial_settings.cooling.clone(),
        },
        keeper_shutdown.clone(),
    );
    let (lcd_tx, lcd_rx) = mpsc::channel(8);
    let (lcd_handle, lcd_cleanup) = lcd_keeper::spawn_observed(
        hid_io_tx.clone(),
        config.lcd.refresh_ms,
        initial_settings.lcd.clone(),
        settings_tx.clone(),
        lcd_rx,
        keeper_shutdown.clone(),
    );

    let (lighting_state_tx, lighting_state_rx) = mpsc::channel::<LightingStateMsg>(16);
    let light_handle = lighting_keeper::spawn_with_profiles(
        hid_io_tx.clone(),
        config.lighting.reissue_ms,
        initial_settings.lighting.clone(),
        lighting_keeper::ThermalInputs {
            dynamic_profiles: initial_settings.dynamic_rgb_profiles.clone(),
            dynamic: initial_settings.dynamic_rgb.clone(),
            config: initial_settings.thermal.clone(),
            host,
            device: sensor_cache.clone(),
        },
        lighting_state_rx,
        settings_tx.clone(),
        keeper_shutdown.clone(),
    );

    let (control_group_tx, control_group_handle) = control_group::spawn(
        cooling_tx.clone(),
        lighting_state_tx.clone(),
        settings_tx.clone(),
        keeper_shutdown,
    );
    let (dispatch_shutdown_tx, dispatch_shutdown_rx) = watch::channel(false);
    let (disp_handle, save_state) = command_dispatch::spawn_updates_observed(
        persistence,
        persistence_key.clone(),
        initial_settings,
        settings_rx,
        dispatch_shutdown_rx,
    );
    let (teardown_tx, teardown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let mut errors = Vec::new();
        keeper_lifecycle::join("sensor", sensor_handle, &mut errors).await;
        keeper_lifecycle::join("cooling", cooling_handle, &mut errors).await;
        keeper_lifecycle::join("LCD", lcd_handle, &mut errors).await;
        match lcd_cleanup.await {
            Ok(reports) => errors.extend(
                reports
                    .into_iter()
                    .map(|error| format!("LCD cleanup: {error}")),
            ),
            Err(_) => errors.push("LCD cleanup report missing".into()),
        }
        keeper_lifecycle::join("lighting", light_handle, &mut errors).await;
        keeper_lifecycle::join("control group", control_group_handle, &mut errors).await;
        // Producers finished their current transactions and enqueued all accepted deltas.
        let _ = dispatch_shutdown_tx.send(true);
        keeper_lifecycle::join("settings dispatcher", disp_handle, &mut errors).await;
        keeper_lifecycle::complete(keepers_joined, errors);
        let _ = hid_handle.await;
        root_forwarder.abort();
        let _ = root_forwarder.await;
        let _ = teardown_tx.send(true);
    });

    Ok(DeviceHandles {
        keepers,
        persistence_key,
        control_group_tx,
        save_state,
        cooling_tx,
        sensor_cache,
        hid_io_tx,
        snapshot_rx,
        settings_tx,
        lcd_tx,
        lighting_state_tx,
        teardown_rx,
    })
}

pub mod dynamic_rgb;
