//! External pinned image + MockBackend only. No live backend, daemon or UI.
use llcontrol_daemon::{
    actor::firmware_update::{CoolingHold, ValidatedImage},
    config::Config,
    ipc_server::{FirmwareJobStage, FirmwareReconnectStage, IpcServerImpl},
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone)]
struct Script {
    mock: MockBackend,
    opens: Arc<AtomicUsize>,
    closes: Arc<AtomicUsize>,
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
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Handle {
            inner: self.mock.open(info)?,
            info: info.clone(),
            script: self.clone(),
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 129]) {
            self.script.mock.queue_response(
                &self.info,
                ACommandPacket::new_request(129, &[0, 1, 0, 2, 1, 35, 2])
                    .as_bytes()
                    .to_vec(),
            );
        }
        if bytes.starts_with(&[1, 134]) {
            self.script.mock.queue_response(
                &self.info,
                ACommandPacket::new_request(134, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6")
                    .as_bytes()
                    .to_vec(),
            );
        }
        if bytes.starts_with(&[2, 11]) || bytes.starts_with(&[2, 10]) {
            let mut reply = bytes[..512].to_vec();
            if bytes[1] == 10 {
                reply[12] = 1;
            }
            self.script.mock.queue_response(&self.info, reply);
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}
#[tokio::test]
#[ignore = "requires external pinned firmware; never uses real HID"]
async fn owned_coordinator_handles_success_observer_loss_and_pretransfer_cancel_without_replay() {
    let bytes = std::fs::read(
        std::env::var("LLCONTROL_TEST_FIRMWARE_IMAGE").expect("external fixture path"),
    )
    .unwrap();
    for mode in 0..4 {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let target = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("COORD".into()),
            "/mock/coordinator".into(),
        );
        let mut saved = DeviceSettings::default();
        saved.fan.pwm_percent = 65;
        store.write(&settings_key(&target), &saved).unwrap();
        let script = Script {
            mock: MockBackend::new(),
            opens: Arc::default(),
            closes: Arc::default(),
        };
        script.mock.add_device(target.clone());
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
        let foreign_root = tempfile::tempdir().unwrap();
        let foreign_store = Arc::new(PersistenceStore::new_with_root(foreign_root.path().into()));
        let foreign_server = Arc::new(IpcServerImpl::new(
            manager.clone(),
            Arc::new(Stats::new()),
            Config::default(),
            foreign_store.clone(),
        ));
        assert!(foreign_server
            .begin_firmware_update(
                id.clone(),
                ValidatedImage::recognize(bytes.clone()).unwrap(),
                CoolingHold::PumpOnly
            )
            .is_err());
        assert!(!foreign_store.admission().is_closed());
        assert!(!store.admission().is_closed());
        if mode == 2 {
            let stopped = Arc::new(IpcServerImpl::new(
                manager.clone(),
                Arc::new(Stats::new()),
                Config::default(),
                store.clone(),
            ));
            assert!(stopped.shutdown_firmware().await.unwrap().is_none());
            assert!(stopped
                .begin_firmware_update(
                    id.clone(),
                    ValidatedImage::recognize(bytes.clone()).unwrap(),
                    CoolingHold::PumpOnly,
                )
                .is_err());
            assert!(!store.admission().is_closed());
        }
        let job = server
            .begin_firmware_update(
                id,
                ValidatedImage::recognize(bytes.clone()).unwrap(),
                CoolingHold::PumpOnly,
            )
            .unwrap();
        if mode == 2 {
            drop(job);
            let report = tokio::time::timeout(Duration::from_secs(5), server.shutdown_firmware())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(report.status.stage, FirmwareJobStage::NotStarted);
            assert!(store.firmware_operations().load().unwrap().is_none());
        } else if mode == 3 {
            let mut progress = job.progress.clone();
            tokio::time::timeout(Duration::from_secs(5), async {
                while !progress.borrow().pump_hold_written {
                    progress.changed().await.unwrap();
                }
            })
            .await
            .unwrap();
            drop(job);
            let report = tokio::time::timeout(Duration::from_secs(5), server.shutdown_firmware())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(report.status.stage, FirmwareJobStage::Unknown);
            assert_eq!(report.outcome.unwrap().progress.chunk_writes, 0);
        } else if mode == 1 {
            drop(job);
            let reattached = server.firmware_job().expect("retained observer");
            let report = tokio::time::timeout(Duration::from_secs(20), reattached.wait())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(report.status.stage, FirmwareJobStage::AwaitingReconnect);
            assert_eq!(report.outcome.unwrap().progress.chunk_echoes, 619);
        } else {
            let report = tokio::time::timeout(Duration::from_secs(20), job.wait())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                report.status.stage,
                FirmwareJobStage::AwaitingReconnect,
                "{report:?}"
            );
            assert_eq!(report.outcome.unwrap().progress.chunk_echoes, 619);
        }
        let retained = server.firmware_job().unwrap().wait().await.unwrap();
        if mode == 0 {
            assert_eq!(script.closes.load(Ordering::SeqCst), 1);
            let journal = root.path().join("firmware-operation.json");
            let before = std::fs::read(&journal).unwrap();
            script.mock.remove_device(&target);
            let probe = server.begin_firmware_reconnect().unwrap();
            let mut status = probe.status.clone();
            tokio::time::timeout(Duration::from_secs(3), async {
                while status.borrow().stage != FirmwareReconnectStage::WaitingForReconnect {
                    status.changed().await.unwrap();
                }
            })
            .await
            .unwrap();
            let mut fresh = target.clone();
            fresh.path = "/mock/fresh-coordinator".into();
            script.mock.add_device(fresh.clone());
            let observed = tokio::time::timeout(Duration::from_secs(3), probe.wait())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                observed.status.stage,
                FirmwareReconnectStage::FreshIdentityObserved
            );
            assert_eq!(observed.evidence.unwrap().target, fresh);
            let writes = script.mock.write_history(&fresh);
            assert_eq!(writes.len(), 1);
            assert!(writes[0].starts_with(&[1, 134]));
            assert_eq!(std::fs::read(journal).unwrap(), before);
            assert!(store
                .firmware_operations()
                .ensure_replay_allowed(&fresh)
                .is_err());
        }
        let joined = server.shutdown_firmware().await.unwrap().unwrap();
        assert_eq!(retained.status.stage, joined.status.stage);
        assert_eq!(retained.outcome, joined.outcome);
        assert_eq!(
            script.opens.load(Ordering::SeqCst),
            if mode == 0 { 2 } else { 1 }
        );
        assert_eq!(
            script.closes.load(Ordering::SeqCst),
            if mode == 0 { 2 } else { 1 }
        );
        assert!(manager.list().await.is_empty());
        assert!(store.admission().is_closed());
        let history = script.mock.write_history(&target);
        if mode == 2 {
            assert!(history
                .iter()
                .all(|p| !p.starts_with(&[2, 11]) && !p.starts_with(&[1, 138])));
        } else {
            let record = store.firmware_operations().load().unwrap().unwrap();
            assert_eq!(
                record.stage,
                if mode == 3 {
                    llcontrol_daemon::firmware_operation::Stage::Unknown
                } else {
                    llcontrol_daemon::firmware_operation::Stage::AwaitingReconnect
                }
            );
            assert_eq!(record.restore_settings.fan.pwm_percent, 65);
            assert_eq!(
                history.iter().filter(|p| p.starts_with(&[2, 11])).count(),
                if mode == 3 { 0 } else { 619 }
            );
            assert!(store
                .read(&settings_key(&target))
                .unwrap()
                .firmware_update_guard
                .is_some());
        }
        manager.shutdown().await;
    }
}
