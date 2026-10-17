//! Internal owned updater job. No RPC/GUI Apply, automatic retry, restore or
//! quarantine resolution. AwaitingReconnect is deliberately not success.
mod reconnect;
use super::IpcServerImpl;
use crate::{
    actor::{
        firmware_update::{CoolingHold, Outcome, Phase, Progress, ValidatedImage},
        HidCommand,
    },
    admission::Maintenance,
    manager::FirmwareOwner,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
};
use anyhow::{ensure, Context};
use futures::FutureExt;
use lconnect_ipc::{DeviceId, ProfileContent};
pub use reconnect::{
    FirmwareReconnectIdentity, FirmwareReconnectJob, FirmwareReconnectReport,
    FirmwareReconnectStage, FirmwareReconnectStatus,
};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::sync::{oneshot, watch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareJobStage {
    Handoff,
    ClosingStaging,
    Quiescing,
    Draining,
    Snapshot,
    Transferring,
    Closing,
    NotStarted,
    Unknown,
    AwaitingReconnect,
}
#[derive(Debug, Clone)]
pub struct FirmwareJobStatus {
    pub stage: FirmwareJobStage,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Clone)]
pub struct FirmwareJobReport {
    pub status: FirmwareJobStatus,
    pub outcome: Option<Outcome>,
}
/// One job per process: maintenance admission is deliberately one-way.
#[derive(Default)]
pub(super) struct FirmwareJobs {
    shutdown: bool,
    job: Option<FirmwareJob>,
    reconnect_owner: Option<crate::manager::FirmwareReconnectOwner>,
    reconnect_job: Option<FirmwareReconnectJob>,
}
/// Dropping any observer never aborts the owned worker. Cancellation is explicit.
/// A cloned observer retains the same status, progress and completion receipt.
#[derive(Clone)]
pub struct FirmwareJob {
    pub status: watch::Receiver<FirmwareJobStatus>,
    pub progress: watch::Receiver<Progress>,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<Option<FirmwareJobReport>>,
}
impl FirmwareJob {
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    pub async fn wait(mut self) -> anyhow::Result<FirmwareJobReport> {
        loop {
            if let Some(report) = self.done.borrow_and_update().clone() {
                return Ok(report);
            }
            self.done
                .changed()
                .await
                .context("firmware supervisor lost its completion owner; outcome unconfirmed")?;
        }
    }
}
impl IpcServerImpl {
    /// Read-only process-local reattachment, available after admission closes.
    /// Durable restart recovery and a product IPC API remain separate work.
    pub fn firmware_job(&self) -> Option<FirmwareJob> {
        self.firmware_jobs
            .lock()
            .expect("firmware registry poisoned")
            .job
            .clone()
    }
    /// Seal job creation, explicitly cancel and join the retained job. Cancelling
    /// this waiter does not detach the supervisor; a later caller can join again.
    pub async fn shutdown_firmware(&self) -> anyhow::Result<Option<FirmwareJobReport>> {
        let (job, reconnect) = {
            let mut jobs = self
                .firmware_jobs
                .lock()
                .expect("firmware registry poisoned");
            jobs.shutdown = true;
            (jobs.job.clone(), jobs.reconnect_job.clone())
        };
        if let Some(reconnect) = &reconnect {
            reconnect.cancel();
        }
        if let Some(job) = &job {
            job.cancel();
        }
        if let Some(reconnect) = reconnect {
            reconnect.wait().await?;
        }
        match job {
            Some(job) => {
                job.cancel();
                job.wait().await.map(Some)
            }
            None => Ok(None),
        }
    }
    /// Internal executable path for Mock acceptance. Product Inspect/Apply must
    /// still qualify hardware/image and provide durable reconnect/recovery UI.
    pub fn begin_firmware_update(
        self: &Arc<Self>,
        id: DeviceId,
        image: ValidatedImage,
        hold: CoolingHold,
    ) -> anyhow::Result<FirmwareJob> {
        // Check runtime and registry before sealing admission. Serialize begin
        // against shutdown, with no await while the registry lock is held.
        let runtime = tokio::runtime::Handle::try_current()?;
        let mut jobs = self
            .firmware_jobs
            .lock()
            .expect("firmware registry poisoned");
        ensure!(!jobs.shutdown, "firmware supervisor shutting down");
        ensure!(jobs.job.is_none(), "firmware job already registered");
        ensure!(
            self.manager.shares_admission(&self.persistence.admission()),
            "firmware manager/store scope mismatch"
        );
        let maintenance = self.persistence.admission().begin_maintenance()?;
        let recovery = maintenance.recovery();
        let initial = FirmwareJobStatus {
            stage: FirmwareJobStage::Handoff,
            diagnostic: None,
        };
        let (status, status_rx) = watch::channel(initial);
        let (progress, progress_rx) = watch::channel(Progress::default());
        let (cancel, cancelled) = watch::channel(false);
        let (done, finished) = watch::channel(None);
        let job = FirmwareJob {
            status: status_rx,
            progress: progress_rx,
            cancel,
            done: finished,
        };
        jobs.job = Some(job.clone());
        let server = self.clone();
        let worker = runtime.spawn(run(
            self.clone(),
            id,
            maintenance,
            image,
            hold,
            cancelled,
            status.clone(),
            progress,
        ));
        runtime.spawn(async move {
            let report = supervise(server, worker, recovery, &status).await;
            status.send_replace(report.status.clone());
            done.send_replace(Some(report));
        });
        Ok(job)
    }
}
/// A worker panic cannot be mistaken for completion or silently lose status.
/// Recovery authority adds a count only after the worker has joined, so the
/// normal preparation drain never waits on its own supervisor.
async fn supervise(
    server: Arc<IpcServerImpl>,
    worker: tokio::task::JoinHandle<FirmwareJobReport>,
    recovery: crate::admission::MaintenanceRecovery,
    status: &watch::Sender<FirmwareJobStatus>,
) -> FirmwareJobReport {
    match worker.await {
        Ok(report) => report,
        Err(error) => {
            stage(status, FirmwareJobStage::Closing);
            let maintenance = recovery.resume();
            let staging = std::panic::AssertUnwindSafe(server.close_staging(&maintenance))
                .catch_unwind()
                .await;
            // Always join roots, even if closing a staging registry panicked.
            // Manager owns receipts even if FirmwareOwner unwound/dropped.
            server.manager.shutdown().await;
            let cleanup = match staging {
                Ok(staging) => {
                    let drained = maintenance.wait_checked().await;
                    match staging.and(drained) {
                        Ok(()) => {
                            "tracked cleanup drained; durable recovery still required".to_owned()
                        }
                        Err(e) => format!("cleanup unconfirmed: {e}"),
                    }
                }
                Err(_) => {
                    drop(maintenance);
                    "staging cleanup panicked; cleanup unconfirmed".to_owned()
                }
            };
            report(
                FirmwareJobStage::Unknown,
                Some(format!(
                    "firmware worker lost: {error}; outcome unconfirmed; {cleanup}"
                )),
                None,
            )
        }
    }
}
fn stage(status: &watch::Sender<FirmwareJobStatus>, stage: FirmwareJobStage) {
    status.send_replace(FirmwareJobStatus {
        stage,
        diagnostic: None,
    });
}
fn check_cancel(cancel: &watch::Receiver<bool>) -> anyhow::Result<()> {
    ensure!(!*cancel.borrow(), "firmware job cancelled before transfer");
    Ok(())
}
/// Keep resources for every admitted device and saved slot until transport and
/// root teardown end. No encoder/render subprocess or HID is started here.
struct RestoreSnapshot {
    target: DeviceSettings,
    _images: Vec<crate::theme_assets::ImagePin>,
    _media: Vec<crate::media_assets::MediaPin>,
    _fonts: Vec<Vec<u8>>,
}
fn snapshot_from_identities(
    store: &PersistenceStore,
    identities: Vec<lconnect_hid::DeviceInfo>,
    target_info: &lconnect_hid::DeviceInfo,
) -> anyhow::Result<RestoreSnapshot> {
    // Never silently recover imports during firmware preparation. Historical
    // notices are allowed; actual remaining scopes/temporaries are not.
    let root = store.publication_root();
    crate::state_transaction::check_directory(&root)?;
    if root.exists() {
        for (i, entry) in std::fs::read_dir(&root)?.enumerate() {
            ensure!(i < 208, "publication root entry budget exceeded");
            let entry = entry?;
            let name = entry.file_name();
            ensure!(
                matches!(
                    name.to_str(),
                    Some("recovery-report.json" | "slot-recovery-report.json")
                ) && entry.file_type()?.is_file(),
                "unresolved publication scope; recover imports before firmware update"
            );
        }
    }
    store.import_recovery_notices()?;
    store.slot_import_recovery_notices()?;
    ensure!(
        store.firmware_operations().load()?.is_none(),
        "firmware operation already unresolved"
    );
    let mut images = BTreeSet::new();
    let mut media = BTreeSet::new();
    let mut fonts = BTreeSet::new();
    let mut themes = Vec::new();
    let mut target = None;
    for info in identities {
        let saved = store.read(&settings_key(&info))?;
        ensure!(
            saved.firmware_update_guard.is_none(),
            "firmware settings already guarded"
        );
        if &info == target_info {
            target = Some(saved.clone());
        }
        if let Some(editor) = &saved.screen_editor {
            images.extend(editor.image_keys().into_iter().map(str::to_owned));
            media.extend(editor.video_keys().into_iter().map(str::to_owned));
            fonts.extend(editor.font_keys().into_iter().map(str::to_owned));
        }
        if let Some(theme) = saved.theme {
            themes.push(theme);
        }
        if let Some(theme) = saved.screen_editor.and_then(|editor| editor.theme) {
            themes.push(theme);
        }
        if let Some(key) = saved.media_clip {
            media.insert(key);
        }
        if let Some(collections) = saved.screen_collections {
            images.extend(collections.image_keys().into_iter().map(str::to_owned));
            media.extend(collections.video_keys().into_iter().map(str::to_owned));
        }
    }
    for profile in store.profiles().list()?.profiles {
        if let ProfileContent::Screen(theme) = profile.content {
            themes.push(theme);
        }
    }
    for theme in themes {
        theme.validate()?;
        images.extend(theme.image_assets().into_iter().map(str::to_owned));
        media.extend(theme.video_assets().into_iter().map(str::to_owned));
        fonts.extend(theme.font_assets().into_iter().map(str::to_owned));
    }
    ensure!(
        images.len() + media.len() + fonts.len() <= 2048,
        "restore asset budget exceeded"
    );
    let image_store = store.theme_assets();
    let media_store = store.media_assets();
    let image_pins = images
        .iter()
        .map(|k| image_store.acquire_reference(k))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let media_pins = media
        .iter()
        .map(|k| media_store.acquire_source(k).map(|s| s.pin()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let font_store = store.theme_fonts();
    let mut bytes = 0usize;
    let mut loaded = Vec::new();
    for key in fonts {
        let font = font_store.load(&key)?;
        bytes += font.len();
        ensure!(bytes <= 64 * 1024 * 1024, "restore font budget exceeded");
        loaded.push(font);
    }
    Ok(RestoreSnapshot {
        target: target.context("target absent from restore snapshot")?,
        _images: image_pins,
        _media: media_pins,
        _fonts: loaded,
    })
}
async fn prepare(
    server: &IpcServerImpl,
    owner: &FirmwareOwner,
    maintenance: Maintenance,
    cancel: &watch::Receiver<bool>,
    status: &watch::Sender<FirmwareJobStatus>,
) -> anyhow::Result<RestoreSnapshot> {
    stage(status, FirmwareJobStage::ClosingStaging);
    server.close_staging(&maintenance).await?;
    stage(status, FirmwareJobStage::Quiescing);
    owner.quiesce_checked().await?;
    stage(status, FirmwareJobStage::Draining);
    let mut drain = Box::pin(maintenance.wait_checked());
    match tokio::time::timeout(Duration::from_secs(30), drain.as_mut()).await {
        Ok(result) => result?,
        Err(_) => {
            status.send_replace(FirmwareJobStatus {
                stage: FirmwareJobStage::Draining,
                diagnostic: Some(
                    "drain deadline exceeded; joining existing work, transfer refused".into(),
                ),
            });
            let _ = drain.await;
            anyhow::bail!("firmware preparation drain deadline exceeded");
        }
    }
    check_cancel(cancel)?;
    stage(status, FirmwareJobStage::Snapshot);
    let store = server.persistence.clone();
    let identities = owner.identities();
    let target = owner.info().clone();
    // Snapshot helper uses the stable identities, not a live/discovery lookup.
    let result =
        tokio::task::spawn_blocking(move || snapshot_identities(&store, identities, target))
            .await??;
    check_cancel(cancel)?;
    Ok(result)
}
fn snapshot_identities(
    store: &PersistenceStore,
    identities: Vec<lconnect_hid::DeviceInfo>,
    target: lconnect_hid::DeviceInfo,
) -> anyhow::Result<RestoreSnapshot> {
    snapshot_from_identities(store, identities, &target)
}
fn report(
    stage: FirmwareJobStage,
    diagnostic: Option<String>,
    outcome: Option<Outcome>,
) -> FirmwareJobReport {
    FirmwareJobReport {
        status: FirmwareJobStatus {
            stage,
            diagnostic: diagnostic.map(|s| s.chars().take(2048).collect()),
        },
        outcome,
    }
}
#[allow(clippy::too_many_arguments)]
async fn run(
    server: Arc<IpcServerImpl>,
    id: DeviceId,
    maintenance: Maintenance,
    image: ValidatedImage,
    hold: CoolingHold,
    cancel: watch::Receiver<bool>,
    status: watch::Sender<FirmwareJobStatus>,
    progress: watch::Sender<Progress>,
) -> FirmwareJobReport {
    let owner = match server.manager.take_for_firmware(id, &maintenance).await {
        Ok(owner) => owner,
        Err(error) => {
            stage(&status, FirmwareJobStage::Closing);
            let _ = server.close_staging(&maintenance).await;
            server.manager.shutdown().await;
            let cleanup = maintenance
                .wait_checked()
                .await
                .err()
                .map(|e| format!("; cleanup: {e}"))
                .unwrap_or_default();
            return report(
                FirmwareJobStage::NotStarted,
                Some(format!("{error}{cleanup}")),
                None,
            );
        }
    };
    let prepared = prepare(&server, &owner, maintenance, &cancel, &status).await;
    let restore = match prepared {
        Ok(restore) => restore,
        Err(error) => {
            stage(&status, FirmwareJobStage::Closing);
            let close = owner
                .close()
                .await
                .err()
                .map(|e| format!("; teardown: {e}"))
                .unwrap_or_default();
            // Even preparation failures cannot reopen ordinary work. Join any
            // cleanup descendants that outlived the failing stage.
            let cleanup = server
                .persistence
                .admission()
                .close()
                .wait_checked()
                .await
                .err()
                .map(|e| format!("; cleanup: {e}"))
                .unwrap_or_default();
            return report(
                FirmwareJobStage::NotStarted,
                Some(format!("{error}{close}{cleanup}")),
                None,
            );
        }
    };
    let store = server.persistence.clone();
    let info = owner.info().clone();
    let saved = restore.target.clone();
    let armed = tokio::task::spawn_blocking(move || {
        let mut operation = store.firmware_operations().arm(&info, hold, saved)?;
        operation.require_fresh_snapshot()?;
        Ok::<_, anyhow::Error>(operation)
    })
    .await;
    let operation = match armed {
        Ok(Ok(operation)) => operation,
        error => {
            stage(&status, FirmwareJobStage::Closing);
            let close = owner.close().await.err();
            return report(
                FirmwareJobStage::NotStarted,
                Some(format!(
                    "journal arm failed: {error:?}; teardown: {close:?}"
                )),
                None,
            );
        }
    };
    stage(&status, FirmwareJobStage::Transferring);
    let operation_id = operation.record().operation_id.clone();
    let (reply, response) = oneshot::channel();
    let sent = owner
        .handles()
        .hid_io_tx
        .send(HidCommand::FirmwareUpdate {
            operation: Box::new(operation),
            image,
            cooling_hold: hold,
            cancel,
            progress,
            reply,
        })
        .await;
    let result = if sent.is_err() {
        Err("original HID actor closed before transfer".to_owned())
    } else {
        response
            .await
            .map_err(|e| format!("firmware reply lost; outcome unconfirmed: {e}"))
    };
    stage(&status, FirmwareJobStage::Closing);
    let closed = owner.close_for_reconnect(operation_id).await;
    // Keep the fresh asset ownership through all transport and root teardown.
    drop(restore);
    match result {
        Ok(outcome) => {
            let phase = match outcome.progress.phase {
                Phase::NotStarted => FirmwareJobStage::NotStarted,
                Phase::AwaitingReconnect if closed.is_ok() => FirmwareJobStage::AwaitingReconnect,
                _ => FirmwareJobStage::Unknown,
            };
            let diagnostic = match closed {
                Ok(owner) => {
                    if phase == FirmwareJobStage::AwaitingReconnect {
                        server
                            .firmware_jobs
                            .lock()
                            .expect("firmware registry poisoned")
                            .reconnect_owner = Some(owner);
                    }
                    outcome.diagnostic.clone()
                }
                Err(e) => Some(format!("original HID teardown unconfirmed: {e}")),
            };
            report(phase, diagnostic, Some(outcome))
        }
        Err(error) => report(FirmwareJobStage::Unknown, Some(error), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        manager::{DeviceManager, ManagerHandle},
        stats::Stats,
    };
    use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    async fn setup() -> (
        tempfile::TempDir,
        Arc<PersistenceStore>,
        ManagerHandle,
        Arc<IpcServerImpl>,
        DeviceId,
    ) {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let mock = MockBackend::new();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("PREP".into()),
            "/mock/prep".into(),
        );
        let mut saved = DeviceSettings::default();
        saved.fan.pwm_percent = 65;
        store.write(&settings_key(&info), &saved).unwrap();
        mock.add_device(info);
        let stats = Arc::new(Stats::new());
        let mut config = Config::default();
        config.poll.hotplug_ms = 5;
        let manager =
            DeviceManager::new(Arc::new(mock), store.clone(), config.clone(), stats.clone())
                .spawn()
                .await;
        let id = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(d) = manager.list().await.first() {
                    break d.id.clone();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let server = Arc::new(IpcServerImpl::new(
            manager.clone(),
            stats,
            config,
            store.clone(),
        ));
        (root, store, manager, server, id)
    }
    #[tokio::test]
    async fn recovery_authority_does_not_count_until_worker_has_joined() {
        let gate = crate::admission::Admission::default();
        let maintenance = gate.begin_maintenance().unwrap();
        let recovery = maintenance.recovery();
        tokio::time::timeout(Duration::from_secs(1), maintenance.wait_checked())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gate.active(), 0);
        let cleanup = recovery.resume();
        assert_eq!(gate.active(), 1);
        cleanup.wait_checked().await.unwrap();
        assert!(gate.enter().is_err());
    }
    #[tokio::test]
    async fn supervisor_panic_joins_handoff_and_preserves_durable_quarantine() {
        let (_root, store, manager, server, id) = setup().await;
        let maintenance = store.admission().begin_maintenance().unwrap();
        let recovery = maintenance.recovery();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        owner.quiesce_checked().await.unwrap();
        let mut teardown = owner.handles().teardown_rx.clone();
        let key = settings_key(owner.info());
        let mut operation = store
            .firmware_operations()
            .arm(
                owner.info(),
                CoolingHold::PumpOnly,
                store.read(&key).unwrap(),
            )
            .unwrap();
        operation.effects_possible().unwrap();
        let before = std::fs::read(_root.path().join("firmware-operation.json"));
        let worker = tokio::spawn(async move {
            let _maintenance = maintenance;
            let _owner = owner;
            let _operation = operation;
            panic!("injected firmware worker panic");
        });
        let (_, _, status) = channels();
        let report = tokio::time::timeout(
            Duration::from_secs(5),
            supervise(server, worker, recovery, &status),
        )
        .await
        .unwrap();
        assert_eq!(report.status.stage, FirmwareJobStage::Unknown);
        assert!(report.outcome.is_none());
        assert!(report
            .status
            .diagnostic
            .unwrap()
            .contains("outcome unconfirmed"));
        assert!(*teardown.borrow_and_update());
        assert!(manager.list().await.is_empty());
        assert!(store.read(&key).unwrap().firmware_update_guard.is_some());
        assert_eq!(store.admission().active(), 0);
        assert!(store.admission().enter().is_err());
        assert_eq!(
            before.unwrap(),
            std::fs::read(_root.path().join("firmware-operation.json")).unwrap()
        );
    }
    #[tokio::test]
    async fn shutdown_waiter_loss_keeps_cancel_join_and_retained_completion() {
        let (_root, _store, manager, server, _id) = setup().await;
        let (status, status_rx) = watch::channel(FirmwareJobStatus {
            stage: FirmwareJobStage::Closing,
            diagnostic: None,
        });
        let (_progress, progress_rx) = watch::channel(Progress::default());
        let (cancel, mut cancelled) = watch::channel(false);
        let (done, done_rx) = watch::channel(None);
        let job = FirmwareJob {
            status: status_rx,
            progress: progress_rx,
            cancel,
            done: done_rx,
        };
        server.firmware_jobs.lock().unwrap().job = Some(job.clone());
        drop(job);
        let mut shutdown = Box::pin(server.shutdown_firmware());
        assert!(futures::poll!(shutdown.as_mut()).is_pending());
        assert!(*cancelled.borrow_and_update());
        // Drop the actual waiter, not just a borrowed Pin handle.
        drop(shutdown);
        let retained = server.firmware_job().unwrap();
        assert!(server.firmware_jobs.lock().unwrap().shutdown);
        let report = report(
            FirmwareJobStage::NotStarted,
            Some("cancelled and joined".into()),
            None,
        );
        status.send_replace(report.status.clone());
        done.send_replace(Some(report));
        let observed = retained.wait().await.unwrap();
        assert_eq!(observed.status.stage, FirmwareJobStage::NotStarted);
        assert_eq!(
            server
                .shutdown_firmware()
                .await
                .unwrap()
                .unwrap()
                .status
                .stage,
            FirmwareJobStage::NotStarted
        );
        assert_eq!(
            server
                .firmware_job()
                .unwrap()
                .wait()
                .await
                .unwrap()
                .status
                .diagnostic
                .as_deref(),
            Some("cancelled and joined")
        );
        manager.shutdown().await;
    }
    #[tokio::test]
    async fn empty_shutdown_seals_registry_without_closing_ordinary_admission() {
        let (_root, store, manager, server, _id) = setup().await;
        assert!(server.shutdown_firmware().await.unwrap().is_none());
        assert!(server.firmware_job().is_none());
        assert!(server.firmware_jobs.lock().unwrap().shutdown);
        assert!(store.admission().enter().is_ok());
        manager.shutdown().await;
    }
    fn channels() -> (
        watch::Sender<bool>,
        watch::Receiver<bool>,
        watch::Sender<FirmwareJobStatus>,
    ) {
        let (c, rx) = watch::channel(false);
        let (status, _) = watch::channel(FirmwareJobStatus {
            stage: FirmwareJobStage::Handoff,
            diagnostic: None,
        });
        (c, rx, status)
    }
    #[tokio::test]
    async fn coordinator_snapshot_retains_custom_image_and_rejects_missing_asset() {
        let (_root, store, manager, _server, id) = setup().await;
        let key = store
            .theme_assets()
            .import(include_bytes!(
                "../../../../desktop/src-tauri/icons/icon.png"
            ))
            .unwrap();
        let mut theme = lconnect_ipc::LcdTheme {
            version: 1,
            name: "Restore".into(),
            background: [0; 3],
            background_image: None,
            background_video: None,
            background_fill: None,
            layers: vec![],
            page_cache: vec![],
            basic_editor: None,
        };
        theme.background_image = Some(lconnect_ipc::BackgroundImage {
            asset: key.clone(),
            fit: lconnect_ipc::ImageFit::Contain,
        });
        let maintenance = store.admission().begin_maintenance().unwrap();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        owner.quiesce_checked().await.unwrap();
        maintenance.wait_checked().await.unwrap();
        let mut saved = store.read(&settings_key(owner.info())).unwrap();
        saved.theme = Some(theme.clone());
        store.write(&settings_key(owner.info()), &saved).unwrap();
        let restore = snapshot_from_identities(&store, owner.identities(), owner.info()).unwrap();
        assert!(!restore._images.is_empty());
        assert!(store
            .theme_assets()
            .delete_unreferenced(&key, 0, &[], || Ok(false))
            .is_err());
        drop(restore);
        store
            .theme_assets()
            .delete_unreferenced(&key, 0, &[], || Ok(false))
            .unwrap();
        theme.background_image.as_mut().unwrap().asset = "0".repeat(32);
        saved.theme = Some(theme);
        store.write(&settings_key(owner.info()), &saved).unwrap();
        assert!(snapshot_from_identities(&store, owner.identities(), owner.info()).is_err());
        assert!(store.firmware_operations().load().unwrap().is_none());
        owner.close().await.unwrap();
        manager.shutdown().await;
    }
    #[test]
    fn snapshot_bound_operation_rejects_saved_intent_change_before_effects_marker() {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("FRESH".into()),
            "/mock/fresh".into(),
        );
        let mut saved = DeviceSettings::default();
        saved.fan.pwm_percent = 65;
        store.write(&settings_key(&info), &saved).unwrap();
        let mut operation = store
            .firmware_operations()
            .arm(&info, CoolingHold::PumpOnly, saved.clone())
            .unwrap();
        operation.require_fresh_snapshot().unwrap();
        saved.fan.pwm_percent = 66;
        store.write(&settings_key(&info), &saved).unwrap();
        assert!(operation.effects_possible().is_err());
        assert_eq!(
            store.firmware_operations().load().unwrap().unwrap().stage,
            crate::firmware_operation::Stage::Armed
        );
        assert!(store
            .read(&settings_key(&info))
            .unwrap()
            .firmware_update_guard
            .is_none());
    }
    #[tokio::test]
    async fn coordinator_preparation_flushes_accepted_intent_before_snapshot_and_pins_assets() {
        let (_root, store, manager, server, id) = setup().await;
        let handles = manager
            .with_device(&id, |d| d.handles.clone())
            .await
            .unwrap();
        handles
            .settings_tx
            .send(crate::actor::command_dispatch::SettingsUpdate::Fan(
                crate::persistence::FanSection {
                    pwm_percent: 73,
                    ..Default::default()
                },
            ))
            .await
            .unwrap();
        let maintenance = store.admission().begin_maintenance().unwrap();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        let (_cancel, rx, status) = channels();
        let restore = prepare(&server, &owner, maintenance, &rx, &status)
            .await
            .unwrap();
        assert_eq!(restore.target.fan.pwm_percent, 73);
        assert_eq!(status.borrow().stage, FirmwareJobStage::Snapshot);
        assert!(!owner.handles().hid_io_tx.is_closed());
        assert!(store.firmware_operations().load().unwrap().is_none());
        owner.close().await.unwrap();
        manager.shutdown().await;
        assert_eq!(store.admission().active(), 0);
    }
    #[tokio::test]
    async fn coordinator_preparation_refuses_cancel_without_arm_or_hold() {
        let (_root, store, manager, server, id) = setup().await;
        let maintenance = store.admission().begin_maintenance().unwrap();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        let (cancel, rx, status) = channels();
        cancel.send_replace(true);
        assert!(prepare(&server, &owner, maintenance, &rx, &status)
            .await
            .is_err());
        assert!(store.firmware_operations().load().unwrap().is_none());
        owner.close().await.unwrap();
        manager.shutdown().await;
    }
    #[tokio::test]
    async fn coordinator_preparation_rejects_unresolved_import_scope_before_firmware_arm() {
        let (_root, store, manager, server, id) = setup().await;
        let maintenance = store.admission().begin_maintenance().unwrap();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        std::fs::create_dir_all(
            store
                .publication_root()
                .join("11111111111111111111111111111111"),
        )
        .unwrap();
        let (_cancel, rx, status) = channels();
        assert!(prepare(&server, &owner, maintenance, &rx, &status)
            .await
            .is_err());
        assert!(store.firmware_operations().load().unwrap().is_none());
        owner.close().await.unwrap();
        manager.shutdown().await;
    }
}
