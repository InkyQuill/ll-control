//! Explicit probe job after acknowledged transfer and checked original close.
//! Identity observation is historical evidence, never quarantine resolution.
use super::{FirmwareJobStage, IpcServerImpl};
use crate::{
    actor::firmware_probe::{Observation, ProbeEvidence},
    manager::FirmwareReconnectOwner,
    persistence::PersistenceStore,
};
use anyhow::{ensure, Context};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const WINDOW: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(200);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareReconnectStage {
    Observing,
    WaitingForDetach,
    WaitingForReconnect,
    FreshIdentityObserved,
    Cancelled,
    TimedOut,
    Failed,
}
#[derive(Debug, Clone)]
pub struct FirmwareReconnectStatus {
    pub stage: FirmwareReconnectStage,
    pub diagnostic: Option<String>,
}
/// A read-only historical observation. No writable or clearing authority.
#[derive(Debug, Clone)]
pub struct FirmwareReconnectIdentity {
    pub operation_id: String,
    pub target: llcontrol_hid::DeviceInfo,
    pub identity: llcontrol_protocol::galahad2_lcdv2::firmware::FirmwareVersion,
}
#[derive(Debug, Clone)]
pub struct FirmwareReconnectReport {
    pub status: FirmwareReconnectStatus,
    pub evidence: Option<FirmwareReconnectIdentity>,
}
#[derive(Clone)]
pub struct FirmwareReconnectJob {
    pub status: watch::Receiver<FirmwareReconnectStatus>,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<Option<FirmwareReconnectReport>>,
}
impl FirmwareReconnectJob {
    pub fn cancel(&self) {
        self.cancel.send_replace(true);
    }
    pub async fn wait(mut self) -> anyhow::Result<FirmwareReconnectReport> {
        loop {
            if let Some(report) = self.done.borrow_and_update().clone() {
                return Ok(report);
            }
            self.done
                .changed()
                .await
                .context("reconnect supervisor lost completion; verification unconfirmed")?;
        }
    }
}
impl IpcServerImpl {
    pub fn firmware_reconnect_job(&self) -> Option<FirmwareReconnectJob> {
        self.firmware_jobs
            .lock()
            .expect("firmware registry poisoned")
            .reconnect_job
            .clone()
    }
    /// Internal explicit action only; no RPC, automatic flash retry or clearing.
    /// Consumes a scope-bound proof of joined original roots, once per process.
    pub fn begin_firmware_reconnect(self: &Arc<Self>) -> anyhow::Result<FirmwareReconnectJob> {
        let runtime = tokio::runtime::Handle::try_current()?;
        let mut jobs = self
            .firmware_jobs
            .lock()
            .expect("firmware registry poisoned");
        ensure!(!jobs.shutdown, "firmware supervisor shutting down");
        ensure!(
            jobs.reconnect_job.is_none(),
            "reconnect job already registered"
        );
        ensure!(
            jobs.job
                .as_ref()
                .and_then(|j| j.done.borrow().clone())
                .is_some_and(|r| r.status.stage == FirmwareJobStage::AwaitingReconnect),
            "reconnect requires acknowledged transfer and joined original close"
        );
        let owner = jobs.reconnect_owner.take().context(
            "joined reconnect owner unavailable; restart recovery requires explicit policy",
        )?;
        let (status, status_rx) = watch::channel(FirmwareReconnectStatus {
            stage: FirmwareReconnectStage::Observing,
            diagnostic: None,
        });
        let (cancel, cancelled) = watch::channel(false);
        let (done, finished) = watch::channel(None);
        let job = FirmwareReconnectJob {
            status: status_rx,
            cancel: cancel.clone(),
            done: finished,
        };
        jobs.reconnect_job = Some(job.clone());
        let store = self.persistence.clone();
        let worker_status = status.clone();
        let worker = runtime.spawn_blocking(move || {
            let _cancel_owner = cancel;
            observe(owner, store, cancelled, worker_status, WINDOW)
        });
        runtime.spawn(async move {
            let report = match worker.await {
                Ok(report) => report,
                Err(e) => report(
                    FirmwareReconnectStage::Failed,
                    Some(format!(
                        "reconnect worker lost; verification unconfirmed: {e}"
                    )),
                    None,
                ),
            };
            status.send_replace(report.status.clone());
            done.send_replace(Some(report));
        });
        Ok(job)
    }
}
fn report(
    stage: FirmwareReconnectStage,
    diagnostic: Option<String>,
    evidence: Option<FirmwareReconnectIdentity>,
) -> FirmwareReconnectReport {
    FirmwareReconnectReport {
        status: FirmwareReconnectStatus {
            stage,
            diagnostic: diagnostic.map(|s| s.chars().take(2048).collect()),
        },
        evidence,
    }
}
fn interrupted(
    cancel: &watch::Receiver<bool>,
    deadline: Instant,
) -> Option<FirmwareReconnectReport> {
    if *cancel.borrow() {
        Some(report(
            FirmwareReconnectStage::Cancelled,
            Some("reconnect cancelled; quarantine preserved".into()),
            None,
        ))
    } else if Instant::now() >= deadline {
        Some(report(FirmwareReconnectStage::TimedOut, Some("fresh detach/reconnect identity not verified within observation window; quarantine preserved".into()), None))
    } else {
        None
    }
}
fn identity(evidence: ProbeEvidence) -> FirmwareReconnectIdentity {
    FirmwareReconnectIdentity {
        operation_id: evidence.operation_id().into(),
        target: evidence.target().clone(),
        identity: evidence.identity().clone(),
    }
}
fn observe(
    owner: FirmwareReconnectOwner,
    store: Arc<PersistenceStore>,
    cancel: watch::Receiver<bool>,
    status: watch::Sender<FirmwareReconnectStatus>,
    window: Duration,
) -> FirmwareReconnectReport {
    let deadline = Instant::now() + window;
    if let Some(stop) = interrupted(&cancel, deadline) {
        return stop;
    }
    let mut probe = match owner.probe(&store) {
        Ok(probe) => probe,
        Err(e) => return report(FirmwareReconnectStage::Failed, Some(e.to_string()), None),
    };
    loop {
        if let Some(stop) = interrupted(&cancel, deadline) {
            return stop;
        }
        // The worker owns the whole synchronous observe and its handle until
        // it returns or unwinds. Shutdown awaits this actual blocking worker.
        let observed = owner.observe(&mut probe);
        if let Some(stop) = interrupted(&cancel, deadline) {
            return stop;
        }
        let stage=match observed {
            Ok(Observation::WaitingForDetach)=>FirmwareReconnectStage::WaitingForDetach,
            Ok(Observation::WaitingForReconnect)=>FirmwareReconnectStage::WaitingForReconnect,
            Ok(Observation::Evidence(e))=>return report(FirmwareReconnectStage::FreshIdentityObserved,
                Some("fresh target identity observed; image/revision qualification and quarantine resolution still required".into()),Some(identity(*e))),
            Err(e)=>return report(FirmwareReconnectStage::Failed,Some(e.to_string()),None),
        };
        status.send_if_modified(|current| {
            if current.stage == stage {
                false
            } else {
                *current = FirmwareReconnectStatus {
                    stage,
                    diagnostic: None,
                };
                true
            }
        });
        let next = Instant::now() + POLL;
        while Instant::now() < next {
            if let Some(stop) = interrupted(&cancel, deadline) {
                return stop;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actor::firmware_update::{CoolingHold, Outcome, Phase, Progress, Step},
        config::Config,
        manager::{DeviceManager, ManagerHandle},
        persistence::{settings_key, DeviceSettings},
        stats::Stats,
    };
    use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
    use llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Condvar, Mutex,
    };

    #[derive(Clone, Default)]
    struct Script {
        mock: MockBackend,
        opens: Arc<AtomicUsize>,
        closes: Arc<AtomicUsize>,
        block: Arc<AtomicBool>,
        panic: Arc<AtomicBool>,
        entered: Arc<AtomicBool>,
        gate: Arc<(Mutex<bool>, Condvar)>,
    }
    struct Handle {
        inner: Box<dyn OpenHandle>,
        info: DeviceInfo,
        script: Script,
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            self.script.closes.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl HidBackend for Script {
        fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
            self.mock.enumerate()
        }
        fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
            let inner = self.mock.open(info)?;
            self.opens.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Handle {
                inner,
                info: info.clone(),
                script: self.clone(),
            }))
        }
    }
    impl OpenHandle for Handle {
        fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
            self.inner.write(bytes)?;
            if bytes.starts_with(&[1, 134]) {
                if self.script.block.load(Ordering::SeqCst) {
                    self.script.entered.store(true, Ordering::SeqCst);
                    let (lock, wake) = &*self.script.gate;
                    let (released, _) = wake
                        .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(5), |r| !*r)
                        .unwrap();
                    assert!(*released, "fixture probe write gate timed out");
                }
                assert!(
                    !self.script.panic.load(Ordering::SeqCst),
                    "injected probe writer panic"
                );
                self.script.mock.queue_response(
                    &self.info,
                    ACommandPacket::new_request(134, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6")
                        .as_bytes()
                        .to_vec(),
                );
            }
            Ok(())
        }
        fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
            self.inner.read_timeout(bytes, ms)
        }
    }
    struct Fixture {
        root: tempfile::TempDir,
        store: Arc<PersistenceStore>,
        manager: ManagerHandle,
        server: Arc<IpcServerImpl>,
        script: Script,
        original: DeviceInfo,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            *self
                .script
                .gate
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.script.gate.1.notify_all();
        }
    }
    impl Fixture {
        fn fresh(&self) -> DeviceInfo {
            let mut d = self.original.clone();
            d.path = "/mock/reconnected".into();
            d
        }
        fn files(&self) -> (Vec<u8>, Vec<u8>) {
            (
                std::fs::read(self.root.path().join("firmware-operation.json")).unwrap(),
                std::fs::read(
                    self.root
                        .path()
                        .join(format!("devices/{}.toml", settings_key(&self.original))),
                )
                .unwrap(),
            )
        }
    }
    async fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let original = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("OWNED-PROBE".into()),
            "/mock/original".into(),
        );
        store
            .write(&settings_key(&original), &DeviceSettings::default())
            .unwrap();
        let script = Script::default();
        script.mock.add_device(original.clone());
        let stats = Arc::new(Stats::new());
        let mut config = Config::default();
        config.poll.hotplug_ms = 5;
        let manager = DeviceManager::new(
            Arc::new(script.clone()),
            store.clone(),
            config.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let id = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(device) = manager.list().await.first() {
                    break device.id.clone();
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
        assert!(
            server.begin_firmware_reconnect().is_err(),
            "no proof/job before handoff"
        );
        let maintenance = store.admission().begin_maintenance().unwrap();
        let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
        owner.quiesce_checked().await.unwrap();
        maintenance.wait_checked().await.unwrap();
        // Synthetic acknowledged activation record; no firmware bytes or flash.
        let mut operation = store
            .firmware_operations()
            .arm(&original, CoolingHold::PumpOnly, DeviceSettings::default())
            .unwrap();
        let operation_id = operation.record().operation_id.clone();
        operation.effects_possible().unwrap();
        let outcome = Outcome {
            progress: Progress {
                phase: Phase::AwaitingReconnect,
                total_chunks: 619,
                cooling_hold: Some(CoolingHold::PumpOnly),
                pump_hold_written: true,
                query_written: true,
                chunk_writes: 619,
                chunk_echoes: 619,
                activation_written: true,
                attempted: Some(Step::Activate),
                ..Progress::default()
            },
            diagnostic: None,
        };
        operation.finish(&outcome).unwrap();
        let closed = owner.close_for_reconnect(operation_id).await.unwrap();
        assert_eq!(script.opens.load(Ordering::SeqCst), 1);
        assert_eq!(script.closes.load(Ordering::SeqCst), 1);
        let terminal =
            super::super::report(FirmwareJobStage::AwaitingReconnect, None, Some(outcome));
        let (_status, status) = watch::channel(terminal.status.clone());
        let (_progress, progress) = watch::channel(Progress::default());
        let (cancel, _) = watch::channel(false);
        let (_done, done) = watch::channel(Some(terminal));
        {
            let mut jobs = server.firmware_jobs.lock().unwrap();
            jobs.job = Some(super::super::FirmwareJob {
                status,
                progress,
                cancel,
                done,
            });
            jobs.reconnect_owner = Some(closed);
        }
        Fixture {
            root,
            store,
            manager,
            server,
            script,
            original,
        }
    }
    async fn wait_stage(job: &FirmwareReconnectJob, expected: FirmwareReconnectStage) {
        let mut status = job.status.clone();
        tokio::time::timeout(Duration::from_secs(3), async {
            while status.borrow().stage != expected {
                status.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn observed_detach_and_fresh_handle_are_bound_to_joined_owner_without_replay() {
        let f = fixture().await;
        let before = f.files();
        f.script.mock.remove_device(&f.original);
        let job = f.server.begin_firmware_reconnect().unwrap();
        assert!(f.server.begin_firmware_reconnect().is_err());
        wait_stage(&job, FirmwareReconnectStage::WaitingForReconnect).await;
        let fresh = f.fresh();
        f.script.mock.add_device(fresh.clone());
        drop(job);
        let report = tokio::time::timeout(
            Duration::from_secs(3),
            f.server.firmware_reconnect_job().unwrap().wait(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            report.status.stage,
            FirmwareReconnectStage::FreshIdentityObserved
        );
        let evidence = report.evidence.unwrap();
        assert_eq!(evidence.target, fresh);
        assert_eq!(
            evidence.operation_id,
            f.store
                .firmware_operations()
                .load()
                .unwrap()
                .unwrap()
                .operation_id
        );
        assert_eq!(evidence.identity.user_version, "1.6");
        assert_eq!(f.script.opens.load(Ordering::SeqCst), 2);
        assert_eq!(f.script.closes.load(Ordering::SeqCst), 2);
        let writes = f.script.mock.write_history(&fresh);
        assert_eq!(writes.len(), 1);
        assert!(writes[0].starts_with(&[1, 134]));
        assert_eq!(f.files(), before);
        assert!(f
            .store
            .firmware_operations()
            .ensure_replay_allowed(&fresh)
            .is_err());
        assert!(f.manager.list().await.is_empty());
        f.server.shutdown_firmware().await.unwrap();
        f.manager.shutdown().await;
    }
    #[tokio::test]
    async fn changed_path_without_observed_absence_times_out_without_opening() {
        let f = fixture().await;
        let before = f.files();
        f.script.mock.remove_device(&f.original);
        f.script.mock.add_device(f.fresh());
        let owner = f
            .server
            .firmware_jobs
            .lock()
            .unwrap()
            .reconnect_owner
            .take()
            .unwrap();
        let (_cancel, rx) = watch::channel(false);
        let (status, _) = watch::channel(FirmwareReconnectStatus {
            stage: FirmwareReconnectStage::Observing,
            diagnostic: None,
        });
        let store = f.store.clone();
        let report = tokio::task::spawn_blocking(move || {
            observe(owner, store, rx, status, Duration::from_millis(80))
        })
        .await
        .unwrap();
        assert_eq!(report.status.stage, FirmwareReconnectStage::TimedOut);
        assert!(report.evidence.is_none());
        assert_eq!(f.script.opens.load(Ordering::SeqCst), 1);
        assert_eq!(f.files(), before);
        f.manager.shutdown().await;
    }
    #[tokio::test]
    async fn closed_owner_refuses_foreign_scope_and_replaced_operation_binding() {
        let f = fixture().await;
        let owner = f
            .server
            .firmware_jobs
            .lock()
            .unwrap()
            .reconnect_owner
            .take()
            .unwrap();
        let foreign = tempfile::tempdir().unwrap();
        assert!(owner
            .probe(&PersistenceStore::new_with_root(foreign.path().into()))
            .is_err());
        let id = f
            .store
            .firmware_operations()
            .load()
            .unwrap()
            .unwrap()
            .operation_id;
        for path in [
            f.root.path().join("firmware-operation.json"),
            f.root
                .path()
                .join(format!("devices/{}.toml", settings_key(&f.original))),
        ] {
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(path, text.replace(&id, "22222222222222222222222222222222")).unwrap();
        }
        let before = f.files();
        assert!(owner
            .probe(&f.store)
            .unwrap_err()
            .to_string()
            .contains("joined original owner"));
        assert_eq!(f.files(), before);
        assert_eq!(f.script.opens.load(Ordering::SeqCst), 1);
        f.manager.shutdown().await;
    }
    #[tokio::test]
    async fn cancelled_shutdown_waiter_does_not_detach_blocked_probe_io() {
        let f = fixture().await;
        let before = f.files();
        f.script.mock.remove_device(&f.original);
        let job = f.server.begin_firmware_reconnect().unwrap();
        wait_stage(&job, FirmwareReconnectStage::WaitingForReconnect).await;
        f.script.block.store(true, Ordering::SeqCst);
        f.script.mock.add_device(f.fresh());
        tokio::time::timeout(Duration::from_secs(3), async {
            while !f.script.entered.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let mut shutdown = Box::pin(f.server.shutdown_firmware());
        assert!(futures::poll!(shutdown.as_mut()).is_pending());
        drop(shutdown);
        drop(job);
        assert_eq!(f.script.closes.load(Ordering::SeqCst), 1);
        let mut shutdown = Box::pin(f.server.shutdown_firmware());
        assert!(futures::poll!(shutdown.as_mut()).is_pending());
        *f.script.gate.0.lock().unwrap() = true;
        f.script.gate.1.notify_all();
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .unwrap()
            .unwrap();
        let report = f
            .server
            .firmware_reconnect_job()
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(report.status.stage, FirmwareReconnectStage::Cancelled);
        assert!(report.evidence.is_none());
        assert_eq!(f.script.closes.load(Ordering::SeqCst), 2);
        assert_eq!(f.files(), before);
        f.manager.shutdown().await;
    }
    #[tokio::test]
    async fn probe_worker_panic_keeps_completion_diagnostic_and_quarantine() {
        let f = fixture().await;
        let before = f.files();
        f.script.mock.remove_device(&f.original);
        let job = f.server.begin_firmware_reconnect().unwrap();
        wait_stage(&job, FirmwareReconnectStage::WaitingForReconnect).await;
        f.script.panic.store(true, Ordering::SeqCst);
        f.script.mock.add_device(f.fresh());
        let report = tokio::time::timeout(Duration::from_secs(3), job.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.status.stage, FirmwareReconnectStage::Failed);
        assert!(report.evidence.is_none());
        assert!(report
            .status
            .diagnostic
            .unwrap()
            .contains("verification unconfirmed"));
        assert_eq!(f.script.closes.load(Ordering::SeqCst), 2);
        assert_eq!(f.files(), before);
        f.server.shutdown_firmware().await.unwrap();
        f.manager.shutdown().await;
    }
}
