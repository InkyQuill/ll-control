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
//! - Replay scope: keepers receive `initial_fan` / `initial_pump`
//!   from persistence and start re-issuing immediately, so lighting
//!   replay happens automatically on attach. Explicit fan/pump PWM
//!   and LCD-config replay (which are not auto-driven by keepers)
//!   land in P1D-15.
//!
//! [`spawn_device_actor_tree`]: crate::actor::spawn_device_actor_tree

use crate::actor::{spawn_device_actor_tree, DeviceHandles};
use crate::config::Config;
use crate::persistence::PersistenceStore;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend};
use lconnect_ipc::{DeviceDescriptor, DeviceId};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tracing::{debug, info, warn};

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

        let handles = spawn_device_actor_tree(
            self.backend.clone(),
            info.clone(),
            &self.config,
            self.persistence.clone(),
            id.as_str().to_string(),
            shutdown_rx,
        )?;

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
