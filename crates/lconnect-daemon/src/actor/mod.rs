//! Per-device actor tree.
//!
//! One `DeviceActorSet` per connected device, spawned by `manager.rs`.
//! Each actor lives on its own tokio task; channels stitch them together.

#![allow(dead_code)]

pub mod command_dispatch;
pub mod hid_io;
pub mod lcd_keeper;
pub mod lighting_keeper;
pub mod sensor_poll;

use crate::persistence::DeviceSettings;
use lconnect_hid::{DeviceInfo, HidBackend};
use lconnect_ipc::SensorSnapshot;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

/// Owned per-device shutdown signal. Drop the sender to terminate all actors.
pub type Shutdown = watch::Receiver<bool>;

/// Commands sent through `hid_io_tx` (single-consumer).
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
    /// Write then read one response (B-class).
    WriteRead512 {
        bytes: Vec<u8>,
        expected_cmd: u8,
        reply: oneshot::Sender<lconnect_hid::Result<[u8; 512]>>,
    },
}

/// Per-device handles exposed to the IPC layer.
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
#[allow(clippy::unimplemented)]
pub fn spawn_device_actor_tree(
    _backend: Arc<dyn HidBackend>,
    _info: DeviceInfo,
    _initial_settings: DeviceSettings,
    _shutdown: Shutdown,
) -> anyhow::Result<DeviceHandles> {
    // Implementation lands incrementally in P1D-5..P1D-12.
    unimplemented!("see P1D-5..P1D-12")
}
