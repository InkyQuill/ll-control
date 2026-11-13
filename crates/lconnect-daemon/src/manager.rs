//! Device discovery, stable settings association, and joined transport teardown.
//!
//! Runtime DeviceIds are attachment counters. Persistence uses VID/PID and
//! hex-encoded serial, or an explicitly restricted serial-less singleton key.
//! Detach waits for the old display/transport/settings tasks before reattach.
//! Diagnostic sources are not persisted and restart in Idle.

use crate::actor::cooling::CoolingCommand;
use crate::actor::{spawn_device_actor_tree_with_sensors, DeviceHandles, LcdCommand};
use crate::config::Config;
use crate::persistence::{settings_key, DeviceSettings, PersistenceStore};
use crate::stats::Stats;
use lconnect_hid::{DeviceFamily, DeviceFamilyId, DeviceInfo, Galahad2LcdV2Family, HidBackend};
use lconnect_ipc::{CoolingTarget, DeviceDescriptor, DeviceId, LcdConfigPatch};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch, RwLock};
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
        "galahad2-lcdv2" => Galahad2LcdV2Family.display_name(),
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
    info: DeviceInfo,
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
    shutdown_tx: watch::Sender<bool>,
    done_rx: watch::Receiver<bool>,
    power_tx: mpsc::Sender<PowerTransition>,
    firmware_tx: mpsc::Sender<FirmwareRequest>,
    admission: crate::admission::Admission,
}

struct PowerTransition {
    sleeping: bool,
    reply: oneshot::Sender<anyhow::Result<()>>,
}

struct FirmwareRequest {
    id: DeviceId,
    reply: oneshot::Sender<anyhow::Result<FirmwareOwner>>,
    _admission: crate::admission::Lease,
}
#[derive(Clone)]
struct HandoffStop {
    stop: watch::Sender<bool>,
    done: watch::Receiver<bool>,
}
impl HandoffStop {
    fn from_device(device: &ManagedDevice) -> Self {
        Self {
            stop: device._shutdown_tx.clone(),
            done: device.handles.teardown_rx.clone(),
        }
    }
    fn request_stop(&self) {
        self.stop.send_replace(true);
    }
    async fn wait(mut self) -> anyhow::Result<()> {
        while !*self.done.borrow_and_update() {
            self.done.changed().await?;
        }
        Ok(())
    }
}
/// Exclusive manager handoff. No Clone/recovery constructor: ordinary map no
/// longer owns these attachments. Holds the original target HID actor; this is
/// neither a firmware arm nor permission to flash. Drop requests root shutdown,
/// while the manager retains join receipts for daemon exit.
pub struct FirmwareOwner {
    backend: Arc<dyn HidBackend>,
    admission: crate::admission::Admission,
    target: ManagedDevice,
    others: Vec<ManagedDevice>,
}
/// Created only after checked old-root teardown. Cannot be cloned, recovered
/// from a journal or constructed by an IPC caller. Owns no open HID handle.
pub(crate) struct FirmwareReconnectOwner {
    backend: Arc<dyn HidBackend>,
    admission: crate::admission::Admission,
    original: DeviceInfo,
    operation_id: String,
}
impl FirmwareReconnectOwner {
    pub(crate) fn probe(
        &self,
        store: &PersistenceStore,
    ) -> anyhow::Result<crate::actor::firmware_probe::ReconnectProbe> {
        anyhow::ensure!(
            self.admission.same_scope(&store.admission()) && self.admission.is_closed(),
            "reconnect owner/store maintenance scope mismatch"
        );
        let probe = crate::actor::firmware_probe::ReconnectProbe::new(store.firmware_operations())?;
        probe.bind_closed_target(&self.original, &self.operation_id)?;
        Ok(probe)
    }
    pub(crate) fn observe(
        &self,
        probe: &mut crate::actor::firmware_probe::ReconnectProbe,
    ) -> anyhow::Result<crate::actor::firmware_probe::Observation> {
        probe.observe(&*self.backend)
    }
}
impl FirmwareOwner {
    pub(crate) fn identities(&self) -> Vec<DeviceInfo> {
        self.devices().map(|d| d.info.clone()).collect()
    }
    pub fn info(&self) -> &DeviceInfo {
        &self.target.info
    }
    pub fn descriptor(&self) -> &DeviceDescriptor {
        &self.target.descriptor
    }
    pub fn handles(&self) -> &DeviceHandles {
        &self.target.handles
    }
    fn devices(&self) -> impl Iterator<Item = &ManagedDevice> {
        std::iter::once(&self.target).chain(self.others.iter())
    }
    /// Stop all admitted device producers concurrently; join every one even if
    /// a peer reports cleanup/save failure. Retains the target's original HID.
    pub async fn quiesce_checked(&self) -> anyhow::Result<()> {
        let results = futures::future::join_all(self.devices().map(|device| async move {
            (
                device.descriptor.id.clone(),
                device.handles.quiesce_checked().await,
            )
        }))
        .await;
        let errors: Vec<_> = results
            .into_iter()
            .filter_map(|(id, result)| result.err().map(|e| format!("{}: {e}", id.as_str())))
            .take(16)
            .collect();
        anyhow::ensure!(
            errors.is_empty(),
            "firmware keeper preflight failed: {}",
            errors.join("; ")
        );
        Ok(())
    }
    pub(crate) async fn close_for_reconnect(
        self,
        operation_id: String,
    ) -> anyhow::Result<FirmwareReconnectOwner> {
        let owner = FirmwareReconnectOwner {
            backend: self.backend.clone(),
            admission: self.admission.clone(),
            original: self.info().clone(),
            operation_id,
        };
        self.close().await?;
        Ok(owner)
    }
    /// Root shutdown plus actual HID/settings/keeper teardown before reconnect
    /// probing. Cancelling this waiter still triggers Drop and manager join.
    pub async fn close(self) -> anyhow::Result<()> {
        let stops: Vec<_> = self.devices().map(HandoffStop::from_device).collect();
        for stop in &stops {
            stop.request_stop();
        }
        let results = futures::future::join_all(stops.into_iter().map(HandoffStop::wait)).await;
        for result in results {
            result?;
        }
        Ok(())
    }
}
impl Drop for FirmwareOwner {
    fn drop(&mut self) {
        for device in self.devices() {
            device._shutdown_tx.send_replace(true);
        }
    }
}

impl ManagerHandle {
    pub(crate) fn shares_admission(&self, gate: &crate::admission::Admission) -> bool {
        self.admission.same_scope(gate)
    }
    /// Serialized after in-flight manager attach/replay. Once queued, request
    /// belongs to the manager: reply loss closes the handed-off roots, never
    /// reattaches/replays. Use only from the owned maintenance coordinator.
    pub async fn take_for_firmware(
        &self,
        id: DeviceId,
        maintenance: &crate::admission::Maintenance,
    ) -> anyhow::Result<FirmwareOwner> {
        let lease = maintenance.child(&self.admission)?;
        let (reply, response) = oneshot::channel();
        self.while_running(async {
            self.firmware_tx
                .send(FirmwareRequest {
                    id,
                    reply,
                    _admission: lease,
                })
                .await
                .map_err(|_| anyhow::anyhow!("device manager stopped before firmware handoff"))?;
            response
                .await
                .map_err(|_| anyhow::anyhow!("device manager stopped during firmware handoff"))?
        })
        .await
    }

    /// Pause discovery and join all device actors before acknowledging sleep.
    /// On wake, reopen devices and replay durable settings and themes. Runtime
    /// attachment IDs change; diagnostic sources are deliberately not restored.
    /// Duplicate transitions are harmless. A failed wake enumeration is retried
    /// by the ordinary discovery loop and also returned to the caller.
    pub async fn prepare_for_sleep(&self, sleeping: bool) -> anyhow::Result<()> {
        let (reply, response) = oneshot::channel();
        self.while_running(async {
            self.power_tx
                .send(PowerTransition { sleeping, reply })
                .await
                .map_err(|_| anyhow::anyhow!("device manager stopped"))?;
            response
                .await
                .map_err(|_| anyhow::anyhow!("device manager stopped during power transition"))?
        })
        .await
    }

    // A send can reserve capacity just before receiver teardown. Its queued
    // reply sender may then live until the last channel sender is dropped.
    // Observe manager completion across both queue admission and reply waiting.
    async fn while_running<T>(
        &self,
        request: impl std::future::Future<Output = anyhow::Result<T>>,
    ) -> anyhow::Result<T> {
        let mut done = self.done_rx.clone();
        tokio::select! {
            biased;
            _ = async {
                while !*done.borrow_and_update() {
                    if done.changed().await.is_err() {
                        break;
                    }
                }
            } => Err(anyhow::anyhow!("device manager stopped")),
            result = request => result,
        }
    }

    /// Stop discovery and wait for all device transports and settings flushes.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let mut done = self.done_rx.clone();
        while !*done.borrow_and_update() {
            if done.changed().await.is_err() {
                break;
            }
        }
    }

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
    host_sensors: crate::sensors::SensorCache,
    backend: Arc<dyn HidBackend>,
    persistence: Arc<PersistenceStore>,
    config: Config,
    /// Process-wide counter set; cloned into each spawned actor tree so
    /// the hid_io actor can bump `hid_write_count` directly. The IPC
    /// server holds its own clone for `daemon_stats` and IPC-side
    /// counters.
    stats: Arc<Stats>,
    /// Per-family attach-order counters. Reset on each daemon process.
    counters: HashMap<String, u32>,
    devices: Arc<RwLock<HashMap<DeviceId, ManagedDevice>>>,
    /// Reverse-lookup: physical key -> opaque DeviceId, so the diff loop
    /// can map a vanished `DeviceInfo` back to the entry to evict.
    by_key: HashMap<DeviceKey, DeviceId>,
    handoffs: Vec<HandoffStop>,
    firmware_taken: bool,
}

impl DeviceManager {
    pub fn with_sensors(mut self, cache: crate::sensors::SensorCache) -> Self {
        self.host_sensors = cache;
        self
    }

    #[must_use]
    pub fn new(
        backend: Arc<dyn HidBackend>,
        persistence: Arc<PersistenceStore>,
        config: Config,
        stats: Arc<Stats>,
    ) -> Self {
        Self {
            host_sensors: Default::default(),
            backend,
            persistence,
            config,
            stats,
            counters: HashMap::new(),
            devices: Arc::new(RwLock::new(HashMap::new())),
            by_key: HashMap::new(),
            handoffs: Vec::new(),
            firmware_taken: false,
        }
    }

    /// Spawn the polling task, returning a `ManagerHandle` that shares
    /// the live device map.
    ///
    /// The polling task runs forever (until process exit). On every
    /// `config.poll.hotplug_ms` interval it enumerates, diffs, and
    /// reconciles by spawning new actor trees / dropping vanished ones.
    pub async fn spawn(self) -> ManagerHandle {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let (done_tx, done_rx) = watch::channel(false);
        let (power_tx, mut power_rx) = mpsc::channel::<PowerTransition>(8);
        let (firmware_tx, mut firmware_rx) = mpsc::channel::<FirmwareRequest>(1);
        let handle = ManagerHandle {
            devices: self.devices.clone(),
            shutdown_tx,
            done_rx,
            power_tx,
            firmware_tx,
            admission: self.persistence.admission(),
        };
        let persistence = self.persistence.clone();
        match tokio::task::spawn_blocking(move || persistence.recover_import_publications()).await {
            Ok(Ok(_)) => {}
            failure => {
                tracing::error!(?failure, "Import recovery failed; device replay disabled");
                let _ = done_tx.send(true);
                return handle;
            }
        }
        if let Err(error) = self.persistence.display_settings().get().await {
            tracing::warn!(%error, "Display settings unavailable; using last known presentation unit");
        }
        let mut state = self;
        let poll_ms = state.config.poll.hotplug_ms.max(1);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(poll_ms));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut sleeping = false;
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown_rx.changed() => break,
                    Some(request) = firmware_rx.recv() => {
                        let result = state.handoff_for_firmware(&request.id).await;
                        // Err(payload) drops FirmwareOwner and requests root stop.
                        let _ = request.reply.send(result);
                    }
                    Some(transition) = power_rx.recv() => {
                        let admission = state.persistence.admission().enter();
                        let result = if let Err(error) = &admission {
                            Err(anyhow::anyhow!(error.to_string()))
                        } else if transition.sleeping == sleeping {
                            Ok(())
                        } else {
                            sleeping = transition.sleeping;
                            state.host_sensors.invalidate();
                            if sleeping {
                                state.detach_all().await;
                                Ok(())
                            } else {
                                state.poll_once().await
                            }
                        };
                        // Cancellation of the caller must not interrupt teardown.
                        let _ = transition.reply.send(result);
                    }
                    _ = interval.tick(), if !sleeping && !state.persistence.admission().is_closed() => {
                        if let Err(e) = state.poll_once().await {
                            warn!("device_manager poll_once failed: {e:#}");
                        }
                    }
                }
            }
            state.detach_all().await;
            let _ = done_tx.send(true);
        });
        handle
    }

    async fn handoff_for_firmware(&mut self, id: &DeviceId) -> anyhow::Result<FirmwareOwner> {
        anyhow::ensure!(
            self.persistence.admission().is_closed(),
            "firmware handoff requires sealed maintenance"
        );
        anyhow::ensure!(!self.firmware_taken, "firmware owner already handed off");
        let mut devices = self.devices.write().await;
        let candidate = devices
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("firmware target attachment unavailable"))?;
        anyhow::ensure!(
            candidate.info.vid == 0x0416 && candidate.info.pid == 0x7395,
            "unsupported firmware target"
        );
        let target = devices
            .remove(id)
            .ok_or_else(|| anyhow::anyhow!("firmware target disappeared"))?;
        let others: Vec<_> = devices.drain().map(|(_, device)| device).collect();
        drop(devices);
        self.by_key.clear();
        self.firmware_taken = true;
        for device in std::iter::once(&target).chain(others.iter()) {
            self.handoffs.push(HandoffStop::from_device(device));
        }
        Ok(FirmwareOwner {
            target,
            others,
            backend: self.backend.clone(),
            admission: self.persistence.admission(),
        })
    }

    async fn detach_all(&mut self) {
        for stop in &self.handoffs {
            stop.request_stop();
        }
        // Remove the whole map before joining: IPC cannot obtain a new producer
        // while any old device is draining. Clear keys even if USB stays present.
        self.by_key.clear();
        let devices: Vec<_> = self
            .devices
            .write()
            .await
            .drain()
            .map(|(_, device)| device)
            .collect();
        for device in devices {
            teardown(device).await;
        }
        let handoffs = std::mem::take(&mut self.handoffs);
        for handoff in handoffs {
            if let Err(error) = handoff.wait().await {
                tracing::warn!(%error, "firmware handoff join receipt lost");
            }
        }
    }

    /// One enumerate-diff-reconcile pass. Pulled out of `spawn` so
    /// tests / future signal-driven re-enumeration can call it
    /// directly without restructuring the loop.
    async fn poll_once(&mut self) -> anyhow::Result<()> {
        let _admission = self.persistence.admission().enter()?;
        let enumerated = self.backend.enumerate()?;
        let visible: Vec<DeviceInfo> = enumerated
            .into_iter()
            .filter(|i| SUPPORTED_FAMILIES.contains(&i.family.as_str()))
            .collect();

        let visible_keys: HashMap<DeviceKey, DeviceInfo> = visible
            .into_iter()
            .map(|i| (DeviceKey::from_info(&i), i))
            .collect();

        let mut identity_counts = HashMap::new();
        for info in visible_keys.values() {
            *identity_counts.entry(settings_key(info)).or_insert(0usize) += 1;
        }
        // Detach disappeared or now-ambiguous identities before opening any new handle.
        let to_evict: Vec<DeviceKey> = self
            .by_key
            .keys()
            .filter(|k| {
                visible_keys
                    .get(*k)
                    .is_none_or(|info| identity_counts[&settings_key(info)] > 1)
            })
            .cloned()
            .collect();
        for key in to_evict {
            if let Some(id) = self.by_key.remove(&key) {
                // Drop the ManagedDevice -> drops its _shutdown_tx ->
                // the watch flips -> every actor's select! arm fires.
                let evicted = self.devices.write().await.remove(&id);
                if let Some(device) = evicted {
                    info!("device detached: id={}", id.as_str());
                    teardown(device).await;
                }
            }
        }

        // Attach: anything in visible_keys not in by_key.
        for (key, info) in visible_keys {
            if identity_counts[&settings_key(&info)] > 1 {
                warn!(
                    "ambiguous settings identity for {:04x}:{:04x}; refusing attachment",
                    info.vid, info.pid
                );
                continue;
            }
            if self.by_key.contains_key(&key) {
                continue;
            }
            match self.attach(info, &key).await {
                Ok(id) => {
                    info!("device attached: id={}", id.as_str());
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
        // Guard BEFORE actor spawn/open/replay (including keeper startup writes).
        // Existing live attachments are controlled by the future update owner;
        // journal publication must not make discovery interrupt its transfer.
        let journal = self.persistence.firmware_operations();
        let candidate = info.clone();
        tokio::task::spawn_blocking(move || journal.ensure_replay_allowed(&candidate)).await??;
        let family = info.family.clone();
        let id = self.allocate_id(&family);

        // The shutdown watch is owned by the ManagedDevice; the actors
        // hold receivers cloned through spawn_device_actor_tree. When
        // the ManagedDevice is dropped (manager evicts it from the
        // map), the sender's Drop flips the watch.
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // invariant: same settings instance feeds keepers and replay
        // -- TOCTOU window closed. spawn_device_actor_tree no longer
        // reads persistence internally; it takes the parsed struct as
        // a parameter so a sibling write between two reads cannot
        // leave the keepers seeded with state-A while replay sends
        // state-B for fan/pump.
        let persistence_key = settings_key(&info);
        let persistence = self.persistence.clone();
        let read_key = persistence_key.clone();
        let settings = tokio::task::spawn_blocking(move || persistence.read(&read_key)).await??;

        let handles = spawn_device_actor_tree_with_sensors(
            self.backend.clone(),
            info.clone(),
            &self.config,
            self.persistence.clone(),
            persistence_key,
            settings.clone(),
            self.stats.clone(),
            shutdown_rx,
            self.host_sensors.clone(),
        )?;

        // Replay persisted device-level settings (fan PWM, pump PWM,
        // LCD config). Run BEFORE inserting into the live device map
        // so an IPC client cannot race a `set_*` between the actor-
        // tree spawn and the replay -- if the IPC `set_*` lands first,
        // the replay would clobber it.
        replay_persisted_settings(&handles, &settings).await;
        if let Some(theme) = settings.theme.clone() {
            let store = self.persistence.theme_assets();
            let fonts = self.persistence.theme_fonts();
            let saved = theme.clone();
            let media = self.persistence.media_assets();
            let loaded = tokio::task::spawn_blocking(move || {
                Ok::<_, anyhow::Error>((
                    store.load(&saved)?,
                    fonts.load_theme(&saved)?,
                    media.pin_theme(&saved)?,
                ))
            })
            .await;
            let runtime = match loaded {
                Ok(Ok((assets, fonts, pin))) => crate::theme_runtime::ThemeRuntime::with_fonts(
                    theme.clone(),
                    assets,
                    fonts,
                    vec![self.host_sensors.clone(), handles.sensor_cache.clone()],
                )
                .map_err(|e| e.to_string())
                .and_then(|runtime| {
                    let degrees = if theme.requires_streaming() && !settings.lcd.rotation.is_empty()
                    {
                        settings.lcd.rotation.parse().unwrap_or(u16::MAX)
                    } else {
                        0
                    };
                    lconnect_image::FrameRotation::try_from(degrees)
                        .map(|rotation| {
                            Arc::new(
                                runtime
                                    .with_display_unit(
                                        self.persistence.display_settings().live_unit(),
                                    )
                                    .with_media_pin(pin)
                                    .with_output_rotation(rotation),
                            )
                        })
                        .map_err(|e| e.to_string())
                }),
                Ok(Err(e)) => Err(e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            let (reply, rx) = tokio::sync::oneshot::channel();
            let command = match runtime {
                Ok(runtime) if runtime.theme().requires_streaming() => {
                    crate::media_composition::PreparedComposition::prepare(
                        self.persistence.media_assets(),
                        runtime,
                    )
                    .await
                    .map(|prepared| crate::actor::LcdCommand::SetComposition {
                        prepared,
                        preparation: None,
                        reply,
                    })
                    .map_err(|e| e.to_string())
                }
                Ok(runtime) => runtime
                    .prepare()
                    .await
                    .map(|prepared| crate::actor::LcdCommand::SetTheme {
                        prepared,
                        preparation: None,
                        reply,
                    })
                    .map_err(|e| e.to_string()),
                Err(error) => Err(error),
            };
            let restored = match command {
                Ok(command) => {
                    if handles.lcd_tx.send(command).await.is_ok() {
                        match rx.await {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.to_string()),
                            Err(e) => Err(e.to_string()),
                        }
                    } else {
                        Err("display actor closed".into())
                    }
                }
                Err(error) => Err(error),
            };
            if let Err(error) = restored {
                let (reply, rx) = tokio::sync::oneshot::channel();
                if handles
                    .lcd_tx
                    .send(crate::actor::LcdCommand::RestoreThemeError {
                        theme,
                        error,
                        reply,
                    })
                    .await
                    .is_ok()
                {
                    let _ = rx.await;
                }
            }
        }

        if let Some(key) = settings.media_clip.clone() {
            let prepared = crate::actor::screen_group::PreparedScreen::prepare(
                &settings,
                self.persistence.clone(),
                vec![self.host_sensors.clone(), handles.sensor_cache.clone()],
            )
            .await;
            let restored = match prepared {
                Ok(prepared) => {
                    let (reply, response) = tokio::sync::oneshot::channel();
                    let command = match prepared.source {
                        crate::actor::screen_group::PreparedSource::Media { source, .. } => {
                            Some(crate::actor::LcdCommand::SetMediaClip {
                                key: key.clone(),
                                source,
                                preparation: None,
                                reply,
                            })
                        }
                        crate::actor::screen_group::PreparedSource::Composition {
                            prepared,
                            ..
                        } => Some(crate::actor::LcdCommand::SetMediaComposition {
                            key: key.clone(),
                            prepared,
                            preparation: None,
                            reply,
                        }),
                        _ => None,
                    };
                    if let Some(command) = command {
                        match handles.lcd_tx.send(command).await {
                            Ok(()) => response
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|state| state.map(|_| ()).map_err(|e| e.to_string())),
                            Err(error) => Err(error.to_string()),
                        }
                    } else {
                        Err("display media replay preparation failed".into())
                    }
                }
                Err(error) => Err(error.to_string()),
            };
            if let Err(error) = restored {
                let (reply, rx) = tokio::sync::oneshot::channel();
                if handles
                    .lcd_tx
                    .send(crate::actor::LcdCommand::RestoreMediaError { key, error, reply })
                    .await
                    .is_ok()
                {
                    let _ = rx.await;
                }
            }
        }

        let descriptor = build_descriptor(&id, &info)?;

        let managed = ManagedDevice {
            info,
            descriptor,
            handles,
            _shutdown_tx: shutdown_tx,
        };
        self.devices.write().await.insert(id.clone(), managed);
        let _ = key;
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
/// 1. Fan PWM (cmd139) -- skip legacy unconfigured0, replay explicit manual0.
/// 2. Pump PWM (cmd138) -- explicit source distinguishes manual0 from unconfigured0.
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

    if settings.cooling.fan.is_none() && settings.fan.has_manual_setting() {
        replay_manual_cooling(handles, CoolingTarget::Fan, settings.fan.pwm_percent, false).await;
        tokio::time::sleep(REPLAY_GAP).await;
    }

    if settings.cooling.pump.is_none() && settings.pump.has_manual_setting() {
        let motherboard = matches!(settings.pump.source.as_str(), "application" | "Application");
        replay_manual_cooling(
            handles,
            CoolingTarget::Pump,
            settings.pump.pwm_percent,
            motherboard,
        )
        .await;
        tokio::time::sleep(REPLAY_GAP).await;
    }

    let lcd = &settings.lcd;
    if lcd.brightness != 0 || !lcd.rotation.is_empty() || !lcd.control_mode.is_empty() {
        let rotation = match lcd.rotation.parse::<u16>() {
            Ok(value) => value,
            Err(error) => {
                warn!("invalid persisted LCD rotation: {error}");
                return;
            }
        };
        let (reply, response) = oneshot::channel();
        let patch = LcdConfigPatch {
            mode: Some(lcd.control_mode.clone()),
            brightness_percent: Some(lcd.brightness),
            rotation_degrees: Some(rotation),
        };
        if handles
            .lcd_tx
            .send(LcdCommand::Patch { patch, reply })
            .await
            .is_err()
        {
            warn!("LCD replay actor closed");
        } else if !matches!(response.await, Ok(Ok(_))) {
            warn!("LCD replay failed; inspect LCD state for details");
        }
    }
}

/// Replay through the cooling owner so successful writes also publish applied
/// duty/source. A direct HID write would leave IPC state empty after restart.
async fn replay_manual_cooling(
    handles: &DeviceHandles,
    target: CoolingTarget,
    percent: u8,
    motherboard: bool,
) {
    let (reply, reply_rx) = oneshot::channel();
    if let Err(e) = handles
        .cooling_tx
        .send(CoolingCommand::Manual {
            target,
            percent,
            motherboard,
            reply,
        })
        .await
    {
        warn!("replay: {target:?} send failed (channel closed): {e}");
        return;
    }
    match reply_rx.await {
        Ok(Ok(())) => debug!("replay: {target:?} PWM re-issued"),
        Ok(Err(e)) => warn!("replay: {target:?} PWM failed: {e}"),
        Err(e) => warn!("replay: {target:?} reply dropped: {e}"),
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

async fn teardown(device: ManagedDevice) {
    let mut done = device.handles.teardown_rx.clone();
    // Drop shutdown sender and externally reachable producer handles first.
    drop(device);
    while !*done.borrow_and_update() {
        if done.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codec_str::parse_lcd_mode;
    use lconnect_protocol::galahad2_lcdv2::lcd::LCDControlMode;

    #[test]
    fn lcdv2_product_label_matches_hid_family() {
        use lconnect_hid::DeviceFamily;
        let family = lconnect_hid::Galahad2LcdV2Family;
        assert_eq!(
            super::display_name_for(family.id().as_str()),
            "Galahad II LCDV2"
        );
        assert_eq!(
            super::display_name_for(family.id().as_str()),
            family.display_name()
        );
    }

    #[tokio::test]
    async fn completed_manager_rejects_queued_and_new_power_requests() {
        use super::*;
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        let (done_tx, done_rx) = watch::channel(false);
        let (power_tx, mut power_rx) = mpsc::channel(1);
        let (firmware_tx, _firmware_rx) = mpsc::channel(1);
        let manager = ManagerHandle {
            devices: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx,
            done_rx,
            power_tx,
            firmware_tx,
            admission: crate::admission::Admission::default(),
        };
        let caller = manager.clone();
        let request = tokio::spawn(async move { caller.prepare_for_sleep(false).await });
        // Keep the reply sender alive to reproduce the shutdown/channel race,
        // rather than relying on scheduler timing or channel-drop notification.
        let retained_request = power_rx.recv().await.unwrap();
        done_tx.send_replace(true);
        let result = tokio::time::timeout(Duration::from_secs(1), request)
            .await
            .expect("queued request must observe manager completion")
            .unwrap();
        assert!(result.is_err());
        assert!(
            tokio::time::timeout(Duration::from_secs(1), manager.prepare_for_sleep(false))
                .await
                .expect("post-shutdown request must not wait for a reply")
                .is_err()
        );
        assert!(power_rx.try_recv().is_err());
        drop(retained_request);
    }

    /// Replay-side parser must accept every mode the IPC layer is
    /// allowed to persist. Mirrors
    /// `ipc_server::tests::parse_lcd_mode_accepts_safe_modes` so the two
    /// call sites cannot drift apart -- both now call into
    /// `crate::codec_str::parse_lcd_mode`, but the behavioural pin
    /// stays here as the replay-path-specific safety net.
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
