//! command_dispatch -- per-device settings persistence with debounce.
//!
//! The IPC server and display actor push independent [`SettingsUpdate`] deltas here at a potentially
//! high rate (e.g., a user dragging a slider). We coalesce a burst into a
//! single atomic disk write after `DEBOUNCE_MS` of quiet.
//!
//! The actor:
//! - Coalesces bursts: 50 updates in 100 ms collapse to one disk write
//!   after the debounce window expires.
//! - Flushes on shutdown so the latest in-memory state is durable.
//! - Flushes when the sender drops (clean test teardown).
//! - Retains dirty intent on write failure and retries after one second.
//!   New updates merge into the same pending state; shutdown makes a final attempt.
//!
//! The `if dirty` guard on the `sleep` arm of `tokio::select!` is
//! critical: without it the actor would wake every `DEBOUNCE_MS` even
//! when there was nothing to flush.

use super::Shutdown;
use crate::persistence::{
    DeviceSettings, FanSection, LcdSection, LightingState, PersistenceStore, PumpLightingState,
    PumpSection,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// Quiet-period after the latest update before we write to disk.
const DEBOUNCE_MS: u64 = 250;

/// Spawn the per-device command_dispatch actor.
///
/// `key` is the persistence key (e.g., `"galahad2-lcdv2:0001"`); pass it
/// in -- the actor does not derive it. `initial` seeds the in-memory
/// state from the last known on-disk settings so the first update only
/// triggers a write if it actually differs. `settings_rx` carries the
/// independent settings deltas on every accepted mutation.
///
/// The task terminates when `shutdown` flips or `settings_rx` is closed.
/// In both cases, any pending dirty state is flushed before exit.
pub fn spawn_updates(
    store: Arc<PersistenceStore>,
    key: String,
    initial: DeviceSettings,
    settings_rx: mpsc::Receiver<SettingsUpdate>,
    shutdown: Shutdown,
) -> JoinHandle<()> {
    spawn_updates_observed(store, key, initial, settings_rx, shutdown).0
}

/// Same settings writer with a cache-only status stream for IPC observers.
pub fn spawn_updates_observed(
    store: Arc<PersistenceStore>,
    key: String,
    initial: DeviceSettings,
    mut settings_rx: mpsc::Receiver<SettingsUpdate>,
    mut shutdown: Shutdown,
) -> (
    JoinHandle<()>,
    tokio::sync::watch::Receiver<lconnect_ipc::SettingsSaveState>,
) {
    let (status_tx, status_rx) =
        tokio::sync::watch::channel(lconnect_ipc::SettingsSaveState::default());
    let debounce = Duration::from_millis(DEBOUNCE_MS);
    let task = tokio::spawn(async move {
        let mut pending = PendingSettings {
            latest: initial,
            collection_images: Vec::new(),
            collection_media: Vec::new(),
            media_pin: None,
            image_leases: lconnect_image::theme::ThemeAssets::new(),
            dirty: false,
            retrying: false,
        };
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    settings_rx.close();
                    while let Some(update) = settings_rx.recv().await {
                        if !pending.accept(update, &store, &key, &status_tx).await { return; }
                    }
                    debug!("command_dispatch shutting down (key={})", key);
                    if pending.dirty {
                        if let Err(e) = write_observed(&store, &key, &pending.latest, &status_tx).await {
                            warn!("command_dispatch flush-on-shutdown failed (key={}): {}", key, e);
                        }
                    }
                    break;
                }
                maybe = settings_rx.recv() => match maybe {
                    Some(s) => {
                        if !pending.accept(s, &store, &key, &status_tx).await { return; }
                    }
                    None => {
                        debug!("command_dispatch settings_rx closed (key={}); flushing and exiting", key);
                        if pending.dirty {
                            if let Err(e) = write_observed(&store, &key, &pending.latest, &status_tx).await {
                                warn!("command_dispatch flush-on-drop failed (key={}): {}", key, e);
                            }
                        }
                        break;
                    }
                },
                // Fire only when there's something pending; without the
                // guard we'd wake every DEBOUNCE_MS even while idle.
                _ = tokio::time::sleep(if pending.retrying { Duration::from_secs(1) } else { debounce }), if pending.dirty => {
                    match write_observed(&store, &key, &pending.latest, &status_tx).await {
                        Ok(()) => { pending.dirty = false; pending.retrying = false; }
                        Err(e) => {
                            warn!("command_dispatch debounced write failed (key={}): {}", key, e);
                            pending.retrying = true;
                        }
                    }
                }
            }
        }
    });
    (task, status_rx)
}

struct PendingSettings {
    latest: DeviceSettings,
    collection_images: Vec<crate::theme_assets::ImagePin>,
    collection_media: Vec<crate::media_assets::MediaPin>,
    media_pin: Option<crate::media_assets::MediaPin>,
    image_leases: lconnect_image::theme::ThemeAssets,
    dirty: bool,
    retrying: bool,
}
impl PendingSettings {
    async fn persist_cooling_modes(
        &mut self,
        banks: lconnect_ipc::CoolingModeBanks,
        store: &Arc<PersistenceStore>,
        key: &str,
        status: &tokio::sync::watch::Sender<lconnect_ipc::SettingsSaveState>,
    ) -> Result<lconnect_ipc::CoolingModeLibrary, String> {
        let Some(next_revision) = self
            .latest
            .cooling_modes_revision
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
        else {
            return Err("cooling mode revision exhausted".into());
        };
        self.latest.cooling_banks = Some(banks);
        self.latest.cooling_modes_revision = next_revision;
        self.dirty = true;
        status.send_modify(|state| state.pending = true);
        let result = write_observed(store, key, &self.latest, status).await;
        self.retrying = result.is_err();
        if result.is_ok() {
            self.dirty = false;
        }
        result.map_err(|error| error.to_string()).and_then(|()| {
            cooling_library(&self.latest, status.borrow().clone())?
                .ok_or("stored cooling modes disappeared".into())
        })
    }
    /// False means recovery cannot prove the committed state: stop this writer
    /// rather than flushing its old snapshot over a pending reset transaction.
    async fn accept(
        &mut self,
        update: SettingsUpdate,
        store: &Arc<PersistenceStore>,
        key: &str,
        status: &tokio::sync::watch::Sender<lconnect_ipc::SettingsSaveState>,
    ) -> bool {
        let update = match update {
            SettingsUpdate::ReadCoolingModes { reply } => {
                let result = cooling_library(&self.latest, status.borrow().clone());
                let _ = reply.send(result);
                return true;
            }
            SettingsUpdate::InitializeCoolingModes { sensor, reply } => {
                let result = if self.latest.cooling_banks.is_some() {
                    Err("stored cooling modes already exist; read them before editing".into())
                } else {
                    match lconnect_ipc::lcdv2_default_cooling_modes(&sensor) {
                        Ok(banks) => self.persist_cooling_modes(banks, store, key, status).await,
                        Err(error) => Err(error.to_string()),
                    }
                };
                let _ = reply.send(result);
                return true;
            }
            SettingsUpdate::ResolveCoolingMode {
                target,
                expected_revision,
                id,
                reply,
            } => {
                let result = (|| {
                    let library = cooling_library(&self.latest, status.borrow().clone())?
                        .ok_or("no stored cooling modes")?;
                    if library.revision != expected_revision {
                        return Err("cooling modes changed; read them before applying".into());
                    }
                    let modes = match target {
                        lconnect_ipc::CoolingTarget::Fan => library.fan,
                        lconnect_ipc::CoolingTarget::Pump => library.pump,
                    };
                    modes
                        .into_iter()
                        .find(|mode| mode.id == id)
                        .ok_or("stored cooling mode no longer exists".into())
                })();
                let _ = reply.send(result);
                return true;
            }
            SettingsUpdate::SaveCoolingMode {
                target,
                expected_revision,
                profile,
                reply,
            } => {
                let prepared = (|| {
                    if expected_revision.len() != 64
                        || !expected_revision
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    {
                        return Err("invalid cooling mode revision".to_owned());
                    }
                    let current = cooling_library(&self.latest, status.borrow().clone())?
                        .ok_or("no stored cooling modes")?;
                    if current.revision != expected_revision {
                        return Err("cooling modes changed; read them before saving".into());
                    }
                    let mut banks = self
                        .latest
                        .cooling_banks
                        .clone()
                        .ok_or("no stored cooling modes")?;
                    let modes = match target {
                        lconnect_ipc::CoolingTarget::Fan => &mut banks.fan,
                        lconnect_ipc::CoolingTarget::Pump => &mut banks.pump,
                    };
                    let mode = modes
                        .iter_mut()
                        .find(|mode| mode.id == profile.id)
                        .ok_or("stored cooling mode no longer exists")?;
                    *mode = profile;
                    banks.validate().map_err(|error| error.to_string())?;
                    Ok::<_, String>(banks)
                })();
                let banks = match prepared {
                    Ok(banks) => banks,
                    Err(error) => {
                        let _ = reply.send(Err(error));
                        return true;
                    }
                };
                let result = self.persist_cooling_modes(banks, store, key, status).await;
                let _ = reply.send(result);
                return true;
            }
            SettingsUpdate::MigrateCollections { migration, reply } => {
                if self.latest.screen_collections.is_some() {
                    let _ = reply.send(Err("collections already initialized; reload".into()));
                    return true;
                }
                let mut next = self.latest.clone();
                next.screen_collections = Some(migration.prepared.value.clone());
                let store = store.clone();
                let key = key.to_owned();
                let outcome = tokio::task::spawn_blocking(move || {
                    let outcome =
                        migration.commit(&store.theme_assets(), &store.media_assets(), || {
                            store.write(&key, &next)
                        });
                    (migration, next, outcome)
                })
                .await;
                let (migration, next, outcome) = match outcome {
                    Ok(value) => value,
                    Err(error) => {
                        let error =
                            format!("migration outcome unknown; restart for readback: {error}");
                        status.send_modify(|state| {
                            state.pending = true;
                            state.last_error = Some(error.clone());
                        });
                        let _ = reply.send(Err(error));
                        return false;
                    }
                };
                let written = match outcome {
                    Ok(result) => result,
                    Err(error) => {
                        let _ = reply.send(Err(error.to_string()));
                        return true;
                    }
                };
                self.latest = next;
                self.collection_images = migration.prepared.images;
                self.collection_media = migration.prepared.media;
                self.dirty = written.is_err();
                self.retrying = written.is_err();
                status.send_modify(|state| {
                    state.pending = self.dirty;
                    if written.is_ok() {
                        state.saved = true;
                    }
                    state.last_error = written.as_ref().err().map(ToString::to_string);
                });
                let _ = reply.send(
                    written
                        .map(|()| migration.prepared.value)
                        .map_err(|error| error.to_string()),
                );
                return true;
            }
            SettingsUpdate::ReadGroupImport {
                operation_id,
                reply,
            } => {
                if let Err(error) = crate::profile_group::validate_operation_id(&operation_id) {
                    let _ = reply.send(Err(crate::profile_group::GroupImportError::Rejected(
                        error.to_string(),
                    )));
                    return true;
                }
                let store = store.clone();
                let key = key.to_owned();
                let latest = self.latest.clone();
                let result = tokio::task::spawn_blocking(move || {
                    store.read_group_import(&key, &latest, &operation_id)
                })
                .await;
                return match result {
                    Ok(Ok(readback)) => {
                        let _ = reply.send(Ok(readback));
                        true
                    }
                    error => {
                        let error = match error {
                            Ok(Err(error)) => error.to_string(),
                            Err(error) => error.to_string(),
                            _ => unreachable!(),
                        };
                        status.send_modify(|s| {
                            s.pending = true;
                            s.last_error = Some(error.clone());
                        });
                        let _ = reply.send(Err(
                            crate::profile_group::GroupImportError::RecoveryRequired(error),
                        ));
                        false
                    }
                };
            }
            SettingsUpdate::CommitProfileGroup {
                operation_id,
                expected,
                prepared,
                reply,
            } => {
                let kind = prepared.profile().kind();
                let store = store.clone();
                let key = key.to_owned();
                let latest = self.latest.clone();
                // The worker owns all preparation pins through publication and
                // recovery, including if the RPC/oneshot receiver disappears.
                let outcome = tokio::task::spawn_blocking(move || {
                    commit_group_import(&store, &key, latest, &operation_id, &expected, &prepared)
                })
                .await
                .unwrap_or_else(|error| {
                    GroupImportOutcome::RecoveryRequired(format!(
                        "group import worker failed: {error}"
                    ))
                });
                return match outcome {
                    GroupImportOutcome::Committed(commit) => {
                        if !commit.repeated {
                            self.latest = commit.readback.settings.clone();
                            if kind == crate::profile_group::GroupKind::Screen {
                                self.media_pin = None;
                                self.image_leases.clear();
                                self.collection_images.clear();
                                self.collection_media.clear();
                            }
                            self.dirty = false;
                            self.retrying = false;
                            status.send_modify(|s| {
                                s.pending = false;
                                s.saved = true;
                                s.last_error = None;
                            });
                        }
                        let _ = reply.send(Ok(*commit));
                        true
                    }
                    GroupImportOutcome::Rejected(error) => {
                        let _ = reply
                            .send(Err(crate::profile_group::GroupImportError::Rejected(error)));
                        true
                    }
                    GroupImportOutcome::RecoveryRequired(error) => {
                        status.send_modify(|s| {
                            s.pending = true;
                            s.last_error = Some(error.clone());
                        });
                        let _ = reply.send(Err(
                            crate::profile_group::GroupImportError::RecoveryRequired(error),
                        ));
                        false
                    }
                };
            }
            SettingsUpdate::ReadProfileGroup { kind, reply } => {
                let store = store.clone();
                let key = key.to_owned();
                let latest = self.latest.clone();
                let outcome = tokio::task::spawn_blocking(move || {
                    store.snapshot_profile_group(&key, &latest, kind)
                })
                .await;
                let outcome = outcome
                    .map_err(|error| format!("profile group snapshot worker failed: {error}"))
                    .and_then(|value| value.map_err(|error| error.to_string()));
                let _ = reply.send(outcome);
                return true;
            }
            SettingsUpdate::ReadScreenReset { reply } => {
                let _ = reply.send((
                    self.latest.screen_reset_receipt.clone(),
                    status.borrow().clone(),
                ));
                return true;
            }
            SettingsUpdate::ReadCollections { reply } => {
                let _ = reply.send(lconnect_ipc::ScreenCollectionsState {
                    collections: self.latest.screen_collections.clone(),
                    persistence: status.borrow().clone(),
                });
                return true;
            }
            SettingsUpdate::SetCollections {
                expected_revision,
                prepared,
                reply,
            } => {
                let current = self
                    .latest
                    .screen_collections
                    .as_ref()
                    .map_or(0, |value| value.revision);
                if current != expected_revision
                    || current.checked_add(1) != Some(prepared.value.revision)
                {
                    let _ = reply.send(Err(
                        "screen collections changed; reload before editing".into()
                    ));
                    return true;
                }
                self.latest.screen_collections = Some(prepared.value);
                self.collection_images = prepared.images;
                self.collection_media = prepared.media;
                self.dirty = true;
                status.send_modify(|state| state.pending = true);
                let result = write_observed(store, key, &self.latest, status).await;
                self.retrying = result.is_err();
                if result.is_ok() {
                    self.dirty = false;
                }
                // Failed writes retain accepted intent and pins for retry. A
                // caller must read back after an error or a lost response.
                let result = result
                    .map(|()| {
                        self.latest
                            .screen_collections
                            .clone()
                            .expect("accepted collection intent")
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
                return true;
            }
            other => other,
        };
        let SettingsUpdate::ResetScreen {
            expected_profile_revision,
            expected_collection_revision,
            operation_id,
            reply,
        } = update
        else {
            update.apply(
                &mut self.latest,
                &mut self.media_pin,
                &mut self.image_leases,
            );
            self.dirty = true;
            self.retrying = false;
            status.send_modify(|s| s.pending = true);
            return true;
        };
        let repeated = self
            .latest
            .screen_reset_receipt
            .as_ref()
            .is_some_and(|r| r.operation_id == operation_id);
        let latest = self.latest.clone();
        let store = store.clone();
        let key = key.to_owned();
        // Keep this owner sequential and retain all resource leases while disk
        // work waits for the shared state gate on a blocking worker.
        let outcome = tokio::task::spawn_blocking(move || {
            reset_screen(
                &store,
                &key,
                latest,
                expected_profile_revision,
                expected_collection_revision,
                &operation_id,
                repeated,
            )
        })
        .await
        .unwrap_or_else(|error| {
            ResetOutcome::RecoveryRequired(format!("screen reset worker failed: {error}"))
        });
        match outcome {
            ResetOutcome::Committed(result) => {
                let (settings, profiles) = *result;
                if !repeated {
                    self.latest = settings.clone();
                    self.media_pin = None;
                    self.image_leases.clear();
                    self.collection_images.clear();
                    self.collection_media.clear();
                    self.dirty = false;
                    self.retrying = false;
                    status.send_modify(|s| {
                        s.pending = false;
                        s.saved = true;
                        s.last_error = None;
                    });
                }
                // A duplicate only acknowledges prior work: preserve later
                // queued deltas, dirty status and leases until their own flush.
                let _ = reply.send(Ok(ScreenResetCommit {
                    settings,
                    profiles,
                    repeated,
                }));
                true
            }
            ResetOutcome::Rejected(error) => {
                let _ = reply.send(Err(ScreenResetError::Rejected(error)));
                true
            }
            ResetOutcome::RecoveryRequired(error) => {
                status.send_modify(|s| {
                    s.pending = true;
                    s.last_error = Some(error.clone());
                });
                let _ = reply.send(Err(ScreenResetError::RecoveryRequired(error)));
                false
            }
        }
    }
}

enum GroupImportOutcome {
    Committed(Box<crate::profile_group::GroupImportCommit>),
    Rejected(String),
    RecoveryRequired(String),
}

fn commit_group_import(
    store: &PersistenceStore,
    key: &str,
    latest: DeviceSettings,
    operation_id: &str,
    expected: &crate::profile_group::GroupRevision,
    prepared: &crate::profile_archive::PreparedGroup,
) -> GroupImportOutcome {
    use crate::profile_group::{GroupImportCommit, GroupImportIntent};
    let intent = match GroupImportIntent::new(operation_id.into(), expected.clone(), prepared) {
        Ok(intent) => intent,
        Err(error) => return GroupImportOutcome::Rejected(error.to_string()),
    };
    let repeated = latest.group_imports.iter().any(|r| r.id == operation_id);
    match store.commit_group_import(key, &latest, expected, operation_id, prepared) {
        Ok(commit) => GroupImportOutcome::Committed(Box::new(commit)),
        Err(error) => match store.read(key) {
            Ok(recovered) => {
                let current = if repeated { &latest } else { &recovered };
                match store.read_group_import(key, current, operation_id) {
                    Ok(Some(readback)) if readback.receipt.matches(&intent) => {
                        GroupImportOutcome::Committed(Box::new(GroupImportCommit {
                            readback,
                            repeated,
                        }))
                    }
                    Ok(_) => GroupImportOutcome::Rejected(error.to_string()),
                    Err(recovery) => GroupImportOutcome::RecoveryRequired(format!(
                        "group import: {error}; recovery required: {recovery}"
                    )),
                }
            }
            Err(recovery) => GroupImportOutcome::RecoveryRequired(format!(
                "group import: {error}; recovery required: {recovery}"
            )),
        },
    }
}

enum ResetOutcome {
    Committed(Box<(DeviceSettings, lconnect_ipc::ProfileLibrary)>),
    Rejected(String),
    RecoveryRequired(String),
}

fn reset_screen(
    store: &PersistenceStore,
    key: &str,
    latest: DeviceSettings,
    expected_revision: u64,
    expected_collection_revision: u64,
    operation_id: &str,
    repeated: bool,
) -> ResetOutcome {
    if repeated {
        return match store.profiles().list() {
            Ok(profiles) => ResetOutcome::Committed(Box::new((latest, profiles))),
            Err(error) => ResetOutcome::Rejected(error.to_string()),
        };
    }
    match store.commit_screen_reset(
        key,
        &latest,
        expected_revision,
        expected_collection_revision,
        operation_id,
    ) {
        Ok(result) => ResetOutcome::Committed(Box::new(result)),
        Err(error) => match store.read(key) {
            Ok(recovered)
                if recovered
                    .screen_reset_receipt
                    .as_ref()
                    .is_some_and(|r| r.operation_id == operation_id) =>
            {
                match store.profiles().list() {
                    Ok(profiles) => ResetOutcome::Committed(Box::new((recovered, profiles))),
                    Err(recovery) => ResetOutcome::RecoveryRequired(format!(
                        "screen reset: {error}; profile recovery required: {recovery}"
                    )),
                }
            }
            Ok(_) => ResetOutcome::Rejected(error.to_string()),
            Err(recovery) => ResetOutcome::RecoveryRequired(format!(
                "screen reset: {error}; recovery required: {recovery}"
            )),
        },
    }
}

fn cooling_library(
    settings: &DeviceSettings,
    persistence: lconnect_ipc::SettingsSaveState,
) -> Result<Option<lconnect_ipc::CoolingModeLibrary>, String> {
    settings
        .cooling_banks
        .as_ref()
        .map(|banks| {
            banks.validate().map_err(|error| error.to_string())?;
            let bytes = serde_json::to_vec(&(settings.cooling_modes_revision, banks))
                .map_err(|error| error.to_string())?;
            Ok(lconnect_ipc::CoolingModeLibrary {
                version: 1,
                revision: format!("{:x}", Sha256::digest(bytes)),
                fan: banks.fan.clone(),
                pump: banks.pump.clone(),
                persistence,
            })
        })
        .transpose()
}

async fn write_observed(
    store: &Arc<PersistenceStore>,
    key: &str,
    settings: &DeviceSettings,
    status: &tokio::sync::watch::Sender<lconnect_ipc::SettingsSaveState>,
) -> anyhow::Result<()> {
    let store = store.clone();
    let key = key.to_owned();
    let settings = settings.clone();
    let result = tokio::task::spawn_blocking(move || store.write(&key, &settings))
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
    status.send_modify(|state| match &result {
        Ok(()) => {
            state.pending = false;
            state.saved = true;
            state.last_error = None;
        }
        Err(error) => {
            state.pending = true;
            state.last_error = Some(error.to_string());
        }
    });
    result
}

#[derive(Debug)]
pub struct ScreenResetCommit {
    pub settings: DeviceSettings,
    pub profiles: lconnect_ipc::ProfileLibrary,
    pub repeated: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ScreenResetError {
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    RecoveryRequired(String),
}

/// Independent settings deltas prevent unrelated concurrent writes from being lost.
#[derive(Debug)]
pub enum SettingsUpdate {
    /// Read and CAS-resolve while the cooling owner holds ordinary command
    /// ordering; coordinated group import cannot freeze it midway through Apply.
    ResolveCoolingMode {
        target: lconnect_ipc::CoolingTarget,
        expected_revision: String,
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<lconnect_ipc::CoolingModeProfile, String>>,
    },
    InitializeCoolingModes {
        sensor: String,
        reply: tokio::sync::oneshot::Sender<Result<lconnect_ipc::CoolingModeLibrary, String>>,
    },
    ReadCoolingModes {
        reply:
            tokio::sync::oneshot::Sender<Result<Option<lconnect_ipc::CoolingModeLibrary>, String>>,
    },
    SaveCoolingMode {
        target: lconnect_ipc::CoolingTarget,
        expected_revision: String,
        profile: lconnect_ipc::CoolingModeProfile,
        reply: tokio::sync::oneshot::Sender<Result<lconnect_ipc::CoolingModeLibrary, String>>,
    },
    ReadGroupImport {
        operation_id: String,
        reply: tokio::sync::oneshot::Sender<
            Result<
                Option<crate::profile_group::GroupImportReadback>,
                crate::profile_group::GroupImportError,
            >,
        >,
    },
    CommitProfileGroup {
        operation_id: String,
        expected: crate::profile_group::GroupRevision,
        prepared: Arc<crate::profile_archive::PreparedGroup>,
        reply: tokio::sync::oneshot::Sender<
            Result<crate::profile_group::GroupImportCommit, crate::profile_group::GroupImportError>,
        >,
    },
    /// Source and target tokens must reflect accepted, not merely flushed, intent.
    ReadProfileGroup {
        kind: crate::profile_group::GroupKind,
        reply: tokio::sync::oneshot::Sender<Result<crate::profile_group::GroupSnapshot, String>>,
    },
    ReadScreenReset {
        reply: tokio::sync::oneshot::Sender<(
            Option<lconnect_ipc::ScreenResetReceipt>,
            lconnect_ipc::SettingsSaveState,
        )>,
    },
    MigrateCollections {
        migration: crate::screen_collections::PreparedMigration,
        reply: tokio::sync::oneshot::Sender<
            std::result::Result<lconnect_ipc::ScreenCollections, String>,
        >,
    },
    ReadCollections {
        reply: tokio::sync::oneshot::Sender<lconnect_ipc::ScreenCollectionsState>,
    },
    SetCollections {
        expected_revision: u64,
        prepared: crate::screen_collections::PreparedCollections,
        reply: tokio::sync::oneshot::Sender<
            std::result::Result<lconnect_ipc::ScreenCollections, String>,
        >,
    },
    /// Owner-serialized durable reset. Caller must serialize LCD source changes
    /// around this barrier and apply the returned intent only after success.
    ResetScreen {
        expected_profile_revision: u64,
        expected_collection_revision: u64,
        operation_id: String,
        reply:
            tokio::sync::oneshot::Sender<std::result::Result<ScreenResetCommit, ScreenResetError>>,
    },
    Fan(FanSection),
    Cooling(lconnect_ipc::CoolingConfig),
    CoolingManual {
        target: lconnect_ipc::CoolingTarget,
        percent: u8,
        motherboard: bool,
    },
    Pump(PumpSection),
    Thermal(lconnect_ipc::ThermalConfig),
    DynamicRgb(Option<lconnect_ipc::DynamicRgbConfig>),
    DynamicRgbSnapshot {
        config: Option<lconnect_ipc::DynamicRgbConfig>,
        profiles: lconnect_ipc::DynamicRgbProfiles,
    },
    LightingFan(LightingState),
    LightingPump(PumpLightingState),
    /// A pump mode and its active policy must become durable together.
    LightingPumpDynamic {
        state: PumpLightingState,
        config: Option<lconnect_ipc::DynamicRgbConfig>,
        profiles: lconnect_ipc::DynamicRgbProfiles,
    },
    Lcd(LcdSection),
    Theme(Option<lconnect_ipc::LcdTheme>),
    PinnedTheme {
        theme: lconnect_ipc::LcdTheme,
        pin: Option<crate::media_assets::MediaPin>,
        images: lconnect_image::theme::ThemeAssets,
    },
    ComposedTheme {
        theme: lconnect_ipc::LcdTheme,
        pin: crate::media_assets::MediaPin,
        images: lconnect_image::theme::ThemeAssets,
    },
    MediaClip {
        key: String,
        pin: Option<crate::media_assets::MediaPin>,
    },
    Replace(Box<DeviceSettings>),
}
impl SettingsUpdate {
    fn apply(
        self,
        settings: &mut DeviceSettings,
        media_pin: &mut Option<crate::media_assets::MediaPin>,
        image_leases: &mut lconnect_image::theme::ThemeAssets,
    ) {
        match self {
            Self::ReadGroupImport { .. }
            | Self::ReadCoolingModes { .. }
            | Self::ResolveCoolingMode { .. }
            | Self::InitializeCoolingModes { .. }
            | Self::SaveCoolingMode { .. }
            | Self::CommitProfileGroup { .. }
            | Self::ReadProfileGroup { .. }
            | Self::ReadScreenReset { .. }
            | Self::ReadCollections { .. }
            | Self::SetCollections { .. }
            | Self::MigrateCollections { .. } => {
                unreachable!("collection commands require settings owner")
            }
            Self::ResetScreen { reply, .. } => {
                let _ = reply.send(Err(ScreenResetError::Rejected(
                    "screen reset requires settings owner".into(),
                )));
            }
            Self::CoolingManual {
                target,
                percent,
                motherboard,
            } => match target {
                lconnect_ipc::CoolingTarget::Fan => {
                    settings.cooling.fan = None;
                    settings.cooling.fan_mode = None;
                    settings.fan.pwm_percent = percent;
                    settings.fan.manual_zero = percent == 0;
                }
                lconnect_ipc::CoolingTarget::Pump => {
                    settings.cooling.pump = None;
                    settings.cooling.pump_mode = None;
                    settings.pump.pwm_percent = percent;
                    settings.pump.source = if motherboard { "application" } else { "mcu" }.into();
                }
            },
            Self::Cooling(value) => settings.cooling = value,
            Self::Fan(value) => settings.fan = value,
            Self::Pump(value) => settings.pump = value,
            Self::Thermal(value) => settings.thermal = value,
            Self::DynamicRgb(value) => settings.dynamic_rgb = value,
            Self::DynamicRgbSnapshot { config, profiles } => {
                settings.dynamic_rgb = config;
                settings.dynamic_rgb_profiles = profiles;
            }
            Self::LightingFan(value) => {
                if let Some(topology) = &value.fan_topology {
                    settings.lighting.fan_topology = topology.clone();
                }
                let mode = lconnect_ipc::canonical_lighting_mode(&value.mode);
                if lconnect_ipc::lighting_color_count(lconnect_ipc::LightingKind::Fan, mode)
                    .is_some()
                {
                    settings
                        .lighting
                        .fan_modes
                        .insert(mode.into(), value.clone());
                }
                settings.lighting.fan = Some(value);
            }
            Self::LightingPumpDynamic {
                state,
                config,
                profiles,
            } => {
                Self::LightingPump(state).apply(settings, media_pin, image_leases);
                settings.dynamic_rgb = config;
                settings.dynamic_rgb_profiles = profiles;
            }
            Self::LightingPump(value) => {
                let mode = lconnect_ipc::canonical_lighting_mode(&value.mode);
                if lconnect_ipc::lighting_color_count(lconnect_ipc::LightingKind::Pump, mode)
                    .is_some()
                {
                    settings
                        .lighting
                        .pump_modes
                        .insert(mode.into(), value.clone());
                }
                settings.lighting.pump = Some(value);
            }
            Self::Lcd(value) => settings.lcd = value,
            Self::PinnedTheme { theme, images, pin } => {
                settings.theme = Some(theme);
                settings.media_clip = None;
                *media_pin = pin;
                *image_leases = images;
            }
            Self::ComposedTheme { theme, pin, images } => {
                *image_leases = images;
                settings.theme = Some(theme);
                settings.media_clip = None;
                *media_pin = Some(pin);
            }
            Self::Theme(value) => {
                image_leases.clear();
                settings.theme = value;
                settings.media_clip = None;
                *media_pin = None;
            }
            Self::MediaClip { key, pin } => {
                image_leases.clear();
                *media_pin = pin;
                settings.media_clip = Some(key);
                settings.theme = None;
            }
            Self::Replace(value) => {
                if value.theme != settings.theme {
                    image_leases.clear();
                }
                if value.media_clip != settings.media_clip
                    || value.theme.as_ref().map(|theme| theme.video_assets())
                        != settings.theme.as_ref().map(|theme| theme.video_assets())
                {
                    *media_pin = None;
                }
                *settings = *value;
            }
        }
    }
}
/// Compatibility entry point for full-state persistence callers.
pub fn spawn(
    store: Arc<PersistenceStore>,
    key: String,
    initial: DeviceSettings,
    mut settings_rx: mpsc::Receiver<DeviceSettings>,
    shutdown: Shutdown,
) -> JoinHandle<()> {
    let (tx, rx) = mpsc::channel(16);
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let task = spawn_updates(store, key, initial, rx, stop_rx);
    tokio::spawn(async move {
        let mut shutdown = shutdown;
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => { while let Ok(s) = settings_rx.try_recv() { let _ = tx.send(SettingsUpdate::Replace(Box::new(s))).await; } break; }
                s = settings_rx.recv() => match s { Some(s) => { if tx.send(SettingsUpdate::Replace(Box::new(s))).await.is_err() { break; } }, None => break }
            }
        }
        drop(tx);
        let _ = stop_tx.send(true);
        let _ = task.await;
    })
}

#[cfg(test)]
mod composition_tests {
    use super::*;

    #[test]
    fn queued_and_pending_image_theme_retains_images_until_replacement() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::theme_assets::ThemeAssetStore::new(root.path().into());
        let key = store
            .import(include_bytes!(
                "../../../../desktop/src-tauri/icons/icon.png"
            ))
            .unwrap();
        let theme = serde_json::from_value(serde_json::json!({"version":1,"name":"Image","background":[0,0,0],"layers":[{"kind":"image","asset":key,"x":0,"y":0,"width":480,"height":480,"fit":"crop","opacity":255}]})).unwrap();
        let images = store.load(&theme).unwrap();
        let pending = SettingsUpdate::PinnedTheme {
            theme,
            images,
            pin: None,
        };
        assert!(store
            .delete_unreferenced(&key, 0, &[], || Ok(false))
            .is_err());
        let mut settings = DeviceSettings::default();
        let mut media = None;
        let mut retained = lconnect_image::theme::ThemeAssets::new();
        pending.apply(&mut settings, &mut media, &mut retained);
        SettingsUpdate::Replace(Box::new(settings.clone())).apply(
            &mut settings,
            &mut media,
            &mut retained,
        );
        assert!(store
            .delete_unreferenced(&key, 0, &[], || Ok(false))
            .is_err());
        SettingsUpdate::Theme(None).apply(&mut settings, &mut media, &mut retained);
        store
            .delete_unreferenced(&key, 0, &[], || Ok(false))
            .unwrap();
    }

    #[test]
    fn replacing_saved_composition_releases_only_the_obsolete_media_pin() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::media_assets::MediaAssetStore::new(root.path().into());
        let clip = lconnect_ipc::H264Clip {
            fps: 24,
            units: vec![lconnect_ipc::H264AccessUnit {
                keyframe: true,
                bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
            }],
        };
        let key = store.import("Video".into(), clip).unwrap().key;
        let source = store.acquire_source(&key).unwrap();
        let theme = serde_json::from_value(serde_json::json!({
            "version":2,"name":"Video","background":[0,0,0],"background_video":key,"layers":[]
        }))
        .unwrap();
        let mut settings = DeviceSettings::default();
        let mut pin = None;
        let mut images = lconnect_image::theme::ThemeAssets::new();
        SettingsUpdate::ComposedTheme {
            theme,
            pin: source.pin(),
            images: Default::default(),
        }
        .apply(&mut settings, &mut pin, &mut images);
        drop(source);
        SettingsUpdate::Replace(Box::new(settings.clone())).apply(
            &mut settings,
            &mut pin,
            &mut images,
        );
        assert!(store.delete_unreferenced(&key, || Ok(false)).is_err());
        SettingsUpdate::Replace(Box::default()).apply(&mut settings, &mut pin, &mut images);
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
    }
}
