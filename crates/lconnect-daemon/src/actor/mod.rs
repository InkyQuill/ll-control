//! Per-device actor tree.
//!
//! One `DeviceActorSet` per connected device, spawned by `manager.rs`.
//! Each actor lives on its own tokio task; channels stitch them together.

pub mod command_dispatch;
pub mod hid_io;
pub mod lcd_keeper;
pub mod lighting_keeper;
pub mod sensor_poll;

use crate::config::Config;
use crate::persistence::{DeviceSettings, PersistenceStore};
use lconnect_hid::{DeviceInfo, HidBackend};
use lconnect_ipc::SensorSnapshot;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

/// Owned per-device shutdown signal. Drop the sender to terminate all actors.
pub type Shutdown = watch::Receiver<bool>;

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
    /// Write bytes to the device, no response read.
    Write {
        bytes: Vec<u8>,
        reply: oneshot::Sender<lconnect_hid::Result<()>>,
    },
    /// Write then read one response (A-class).
    WriteRead64 {
        bytes: Vec<u8>,
        expected_cmd: u8,
        reply: oneshot::Sender<lconnect_hid::Result<[u8; 64]>>,
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
        reply: oneshot::Sender<lconnect_hid::Result<[u8; 512]>>,
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
    pub hid_io_tx: mpsc::Sender<HidCommand>,
    pub snapshot_rx: watch::Receiver<SensorSnapshot>,
    pub settings_tx: mpsc::Sender<DeviceSettings>,
    pub lcd_frame_tx: mpsc::Sender<LcdFrameMsg>,
    pub lighting_state_tx: mpsc::Sender<LightingStateMsg>,
}

/// Frame-source message for `lcd_keeper`.
#[derive(Debug)]
pub enum LcdFrameMsg {
    Set(bytes::Bytes),
    Clear,
}

/// Lighting-keeper state replacement.
#[derive(Debug)]
pub enum LightingStateMsg {
    SetFan(crate::persistence::LightingState),
    SetPump(crate::persistence::PumpLightingState),
    Clear(lconnect_ipc::LightingKind),
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
pub fn spawn_device_actor_tree(
    backend: Arc<dyn HidBackend>,
    info: DeviceInfo,
    config: &Config,
    persistence: Arc<PersistenceStore>,
    persistence_key: String,
    initial_settings: DeviceSettings,
    shutdown: Shutdown,
) -> anyhow::Result<DeviceHandles> {
    let hid_io_tx = hid_io::spawn(backend, info, shutdown.clone())?;

    // sensor_poll: poll period derived from sensor_hz (clamped >=1 to
    // avoid div-by-zero if config arrives with sensor_hz = 0).
    let sensor_hz = config.poll.sensor_hz.max(1);
    let sensor_period_ms = 1000u64 / u64::from(sensor_hz);
    let snapshot_rx = sensor_poll::spawn(hid_io_tx.clone(), sensor_period_ms, shutdown.clone());

    let (frame_tx, frame_rx) = mpsc::channel::<LcdFrameMsg>(8);
    // _lcd_handle / _light_handle / _disp_handle are JoinHandles; the
    // actors run for the lifetime of the device, anchored to the
    // shared shutdown watch. We deliberately do not await them here;
    // teardown happens when the manager drops the watch::Sender.
    let _lcd_handle = lcd_keeper::spawn(
        hid_io_tx.clone(),
        config.lcd.refresh_ms,
        frame_rx,
        shutdown.clone(),
    );

    let (lighting_state_tx, lighting_state_rx) = mpsc::channel::<LightingStateMsg>(16);
    let _light_handle = lighting_keeper::spawn(
        hid_io_tx.clone(),
        config.lighting.reissue_ms,
        initial_settings.lighting.fan.clone(),
        initial_settings.lighting.pump.clone(),
        lighting_state_rx,
        shutdown.clone(),
    );

    let (settings_tx, settings_rx) = mpsc::channel::<DeviceSettings>(16);
    let _disp_handle = command_dispatch::spawn(
        persistence,
        persistence_key,
        initial_settings,
        settings_rx,
        shutdown,
    );

    Ok(DeviceHandles {
        hid_io_tx,
        snapshot_rx,
        settings_tx,
        lcd_frame_tx: frame_tx,
        lighting_state_tx,
    })
}
