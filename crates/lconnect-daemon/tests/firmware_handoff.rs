//! Serialized manager handoff / original HID / private stores and MockBackend.
use lconnect_daemon::{
    actor::{keeper_lifecycle::State, HidCommand},
    config::Config,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use lconnect_ipc::DeviceId;
use lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tokio::sync::oneshot;
fn info(serial: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some(serial.into()),
        PathBuf::from(format!("/mock/{serial}")),
    )
}
#[derive(Clone)]
struct Script {
    mock: MockBackend,
    opens: Arc<AtomicUsize>,
    closes: Arc<AtomicUsize>,
    gate: Arc<Gate>,
}
struct Gate {
    target: Mutex<Option<(PathBuf, u8)>>,
    entered: AtomicBool,
    released: Mutex<bool>,
    wake: Condvar,
}
impl Script {
    fn new() -> Self {
        Self {
            mock: MockBackend::new(),
            opens: Arc::default(),
            closes: Arc::default(),
            gate: Arc::new(Gate {
                target: Mutex::new(None),
                entered: AtomicBool::new(false),
                released: Mutex::new(false),
                wake: Condvar::new(),
            }),
        }
    }
    fn arm(&self, info: &DeviceInfo, cmd: u8) {
        *self.gate.target.lock().unwrap() = Some((info.path.clone(), cmd));
    }
    fn release(&self) {
        *self.gate.released.lock().unwrap() = true;
        self.gate.wake.notify_all();
    }
}
struct Release(Script);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
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
    fn enumerate(&self) -> lconnect_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> lconnect_hid::Result<Box<dyn OpenHandle>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(Handle {
            inner: self.mock.open(info)?,
            info: info.clone(),
            script: self.clone(),
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> lconnect_hid::Result<()> {
        let block = self
            .script
            .gate
            .target
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(path, cmd)| path == &self.info.path && bytes.get(1) == Some(cmd));
        if block && !self.script.gate.entered.swap(true, Ordering::SeqCst) {
            let mut released = self.script.gate.released.lock().unwrap();
            while !*released {
                let (next, timeout) = self
                    .script
                    .gate
                    .wake
                    .wait_timeout(released, Duration::from_secs(10))
                    .unwrap();
                released = next;
                if timeout.timed_out() && !*released {
                    return Err(lconnect_hid::Error::BlockingTaskFailed(
                        "test gate not released".into(),
                    ));
                }
            }
        }
        self.inner.write(bytes)?;
        if bytes.starts_with(&[2, 11]) || bytes.starts_with(&[2, 10]) {
            let mut response = bytes[..512].to_vec();
            if bytes.starts_with(&[2, 10]) {
                response[12] = 1;
            }
            self.script.mock.queue_response(&self.info, response);
        }
        if bytes.starts_with(&[1, 134]) {
            self.script.mock.queue_response(
                &self.info,
                ACommandPacket::new_request(134, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6")
                    .as_bytes()
                    .to_vec(),
            );
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> lconnect_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}
async fn until(mut check: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !check() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
async fn attached(manager: &ManagerHandle, n: usize) -> Vec<lconnect_ipc::DeviceDescriptor> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let list = manager.list().await;
            if list.len() == n {
                return list;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
async fn setup(
    script: &Script,
) -> (
    tempfile::TempDir,
    Arc<PersistenceStore>,
    ManagerHandle,
    DeviceId,
) {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let target = info("A");
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 65;
    store.write(&settings_key(&target), &settings).unwrap();
    script.mock.add_device(target);
    let mut config = Config::default();
    config.poll.hotplug_ms = 5;
    let manager = DeviceManager::new(
        Arc::new(script.clone()),
        store.clone(),
        config,
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    let id = attached(&manager, 1).await[0].id.clone();
    (root, store, manager, id)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handoff_waits_for_inflight_attach_replay_then_quiesces_all_on_original_handles() {
    let script = Script::new();
    let _release = Release(script.clone());
    let (_root, store, manager, id) = setup(&script).await;
    let original = manager
        .with_device(&id, |d| d.handles.clone())
        .await
        .unwrap();
    let peer = info("B");
    let mut saved = DeviceSettings::default();
    saved.fan.pwm_percent = 65;
    store.write(&settings_key(&peer), &saved).unwrap();
    script.arm(&peer, 139);
    script.mock.add_device(peer.clone());
    until(|| script.gate.entered.load(Ordering::SeqCst)).await;
    let maintenance = store.admission().begin_maintenance().unwrap();
    let mut request = Box::pin(manager.take_for_firmware(id, &maintenance));
    assert!(futures::poll!(request.as_mut()).is_pending());
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(futures::poll!(request.as_mut()).is_pending());
    assert_eq!(manager.list().await.len(), 1);
    script.release();
    let owner = request.await.unwrap();
    assert_eq!(owner.info(), &info("A"));
    assert!(manager.list().await.is_empty());
    assert_eq!(script.opens.load(Ordering::SeqCst), 2);
    owner.quiesce_checked().await.unwrap();
    assert_eq!(original.keepers.state(), State::Joined);
    assert!(!*original.teardown_rx.borrow());
    assert!(!original.hid_io_tx.is_closed());
    let (reply, response) = oneshot::channel();
    owner
        .handles()
        .hid_io_tx
        .send(HidCommand::Firmware { reply })
        .await
        .unwrap();
    response.await.unwrap().unwrap();
    assert_eq!(script.opens.load(Ordering::SeqCst), 2);
    assert_eq!(script.closes.load(Ordering::SeqCst), 0);
    owner.close().await.unwrap();
    assert_eq!(script.closes.load(Ordering::SeqCst), 2);
    maintenance.wait_checked().await.unwrap();
    manager.shutdown().await;
    assert!(store.firmware_operations().load().unwrap().is_none());
    assert_eq!(
        store.read(&settings_key(&peer)).unwrap().fan.pwm_percent,
        65
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_handoff_requester_closes_roots_without_replay_and_manager_still_joins() {
    let script = Script::new();
    let _release = Release(script.clone());
    let (_root, store, manager, id) = setup(&script).await;
    let peer = info("B");
    let mut saved = DeviceSettings::default();
    saved.fan.pwm_percent = 65;
    store.write(&settings_key(&peer), &saved).unwrap();
    script.arm(&peer, 139);
    script.mock.add_device(peer);
    until(|| script.gate.entered.load(Ordering::SeqCst)).await;
    let maintenance = store.admission().begin_maintenance().unwrap();
    let mut request = Box::pin(manager.take_for_firmware(id, &maintenance));
    assert!(futures::poll!(request.as_mut()).is_pending());
    drop(request);
    drop(maintenance);
    script.release();
    attached(&manager, 0).await;
    tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
        .await
        .unwrap();
    assert_eq!(script.opens.load(Ordering::SeqCst), 2);
    assert_eq!(script.closes.load(Ordering::SeqCst), 2);
    assert!(store.admission().is_closed());
}
#[tokio::test]
async fn wrong_scope_missing_target_and_duplicate_claim_never_reopen_or_grant_a_second_owner() {
    let script = Script::new();
    let (_root, store, manager, id) = setup(&script).await;
    let foreign = lconnect_daemon::admission::Admission::default()
        .begin_maintenance()
        .unwrap();
    assert!(manager
        .take_for_firmware(id.clone(), &foreign)
        .await
        .is_err());
    foreign.wait_checked().await.unwrap();
    assert!(!store.admission().is_closed());
    let maintenance = store.admission().begin_maintenance().unwrap();
    assert!(manager
        .take_for_firmware(DeviceId::new("missing"), &maintenance)
        .await
        .is_err());
    assert_eq!(manager.list().await.len(), 1);
    let owner = manager
        .take_for_firmware(id.clone(), &maintenance)
        .await
        .unwrap();
    assert!(manager.take_for_firmware(id, &maintenance).await.is_err());
    owner.quiesce_checked().await.unwrap();
    drop(owner);
    manager.shutdown().await;
    maintenance.wait_checked().await.unwrap();
    assert_eq!(script.opens.load(Ordering::SeqCst), 1);
    assert_eq!(script.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manager_shutdown_waits_for_handed_off_blocking_io_even_while_owner_is_alive() {
    let script = Script::new();
    let _release = Release(script.clone());
    let (_root, store, manager, id) = setup(&script).await;
    let maintenance = store.admission().begin_maintenance().unwrap();
    let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
    owner.quiesce_checked().await.unwrap();
    script.arm(owner.info(), 139);
    let (reply, response) = oneshot::channel();
    owner
        .handles()
        .hid_io_tx
        .send(HidCommand::Write {
            bytes: lconnect_protocol::galahad2_lcdv2::fan_pwm::build_request(
                lconnect_protocol::galahad2_lcdv2::pwm::Pwm::max(),
            )
            .to_vec(),
            reply,
        })
        .await
        .unwrap();
    until(|| script.gate.entered.load(Ordering::SeqCst)).await;
    let copy = manager.clone();
    let shutdown = tokio::spawn(async move { copy.shutdown().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!shutdown.is_finished());
    assert_eq!(script.closes.load(Ordering::SeqCst), 0);
    assert!(!*owner.handles().teardown_rx.borrow());
    script.release();
    let _ = response.await;
    tokio::time::timeout(Duration::from_secs(3), shutdown)
        .await
        .unwrap()
        .unwrap();
    assert!(*owner.handles().teardown_rx.borrow());
    assert_eq!(script.closes.load(Ordering::SeqCst), 1);
    owner.close().await.unwrap();
    maintenance.wait_checked().await.unwrap();
}
#[tokio::test]
async fn checked_quiesce_joins_other_devices_even_when_target_saved_settings_fail() {
    let script = Script::new();
    let (_root, store, manager, id) = setup(&script).await;
    let peer = info("B");
    script.mock.add_device(peer.clone());
    let list = attached(&manager, 2).await;
    let peer_id = list.into_iter().find(|d| d.id != id).unwrap().id;
    let peer_handles = manager
        .with_device(&peer_id, |d| d.handles.clone())
        .await
        .unwrap();
    let maintenance = store.admission().begin_maintenance().unwrap();
    let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
    let target_path = _root
        .path()
        .join("devices")
        .join(format!("{}.toml", settings_key(owner.info())));
    std::fs::remove_file(&target_path).unwrap();
    std::fs::create_dir(&target_path).unwrap();
    owner
        .handles()
        .settings_tx
        .send(
            lconnect_daemon::actor::command_dispatch::SettingsUpdate::Fan(
                lconnect_daemon::persistence::FanSection {
                    pwm_percent: 66,
                    ..Default::default()
                },
            ),
        )
        .await
        .unwrap();
    assert!(owner.quiesce_checked().await.is_err());
    assert_eq!(peer_handles.keepers.state(), State::Joined);
    assert_eq!(owner.handles().keepers.state(), State::Joined);
    assert!(owner.handles().save_state.borrow().last_error.is_some());
    assert!(!owner.handles().hid_io_tx.is_closed());
    assert!(script
        .mock
        .write_history(owner.info())
        .iter()
        .all(|p| !p.starts_with(&[2, 11]) && !p.starts_with(&[2, 10])));
    owner.close().await.unwrap();
    manager.shutdown().await;
    maintenance.wait_checked().await.unwrap();
    assert_eq!(script.closes.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_owner_close_does_not_detach_the_manager_hid_join() {
    let script = Script::new();
    let _release = Release(script.clone());
    let (_root, store, manager, id) = setup(&script).await;
    let maintenance = store.admission().begin_maintenance().unwrap();
    let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
    owner.quiesce_checked().await.unwrap();
    let handles = owner.handles().clone();
    script.arm(owner.info(), 139);
    let (reply, response) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::Write {
            bytes: lconnect_protocol::galahad2_lcdv2::fan_pwm::build_request(
                lconnect_protocol::galahad2_lcdv2::pwm::Pwm::max(),
            )
            .to_vec(),
            reply,
        })
        .await
        .unwrap();
    until(|| script.gate.entered.load(Ordering::SeqCst)).await;
    let closing = tokio::spawn(async move { owner.close().await });
    tokio::task::yield_now().await;
    assert!(!closing.is_finished());
    closing.abort();
    let _ = closing.await;
    let copy = manager.clone();
    let shutdown = tokio::spawn(async move { copy.shutdown().await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!shutdown.is_finished());
    assert!(!*handles.teardown_rx.borrow());
    script.release();
    let _ = response.await;
    tokio::time::timeout(Duration::from_secs(3), shutdown)
        .await
        .unwrap()
        .unwrap();
    assert!(*handles.teardown_rx.borrow());
    assert_eq!(script.closes.load(Ordering::SeqCst), 1);
    maintenance.wait_checked().await.unwrap();
}

#[tokio::test]
#[ignore = "requires external pinned vendor fixture; MockBackend only"]
async fn scoped_manager_owner_transfers_pinned_image_on_original_handle_with_durable_outcome() {
    use lconnect_daemon::actor::firmware_update::{CoolingHold, Phase, Progress, ValidatedImage};
    let path =
        std::env::var("LCONNECT_TEST_FIRMWARE_IMAGE").expect("set external pinned fixture path");
    let image = ValidatedImage::recognize(std::fs::read(path).unwrap()).unwrap();
    let script = Script::new();
    let (_root, store, manager, id) = setup(&script).await;
    let maintenance = store.admission().begin_maintenance().unwrap();
    let owner = manager.take_for_firmware(id, &maintenance).await.unwrap();
    owner.quiesce_checked().await.unwrap();
    maintenance.wait_checked().await.unwrap();
    let saved = store.read(&settings_key(owner.info())).unwrap();
    let operation = store
        .firmware_operations()
        .arm(owner.info(), CoolingHold::PumpOnly, saved.clone())
        .unwrap();
    let before = script.mock.write_history(owner.info()).len();
    let (_cancel, cancel) = tokio::sync::watch::channel(false);
    let (progress, _view) = tokio::sync::watch::channel(Progress::default());
    let (reply, response) = oneshot::channel();
    owner
        .handles()
        .hid_io_tx
        .send(HidCommand::FirmwareUpdate {
            operation: Box::new(operation),
            image,
            cooling_hold: CoolingHold::PumpOnly,
            cancel,
            progress,
            reply,
        })
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(20), response)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        outcome.progress.phase,
        Phase::AwaitingReconnect,
        "{outcome:?}"
    );
    assert_eq!(outcome.progress.chunk_echoes, 619);
    let journal = store.firmware_operations().load().unwrap().unwrap();
    assert_eq!(journal.progress.as_ref(), Some(&outcome.progress));
    let current = store.read(&settings_key(owner.info())).unwrap();
    assert_eq!(current.fan.pwm_percent, saved.fan.pwm_percent);
    assert!(current.firmware_update_guard.is_some());
    let suffix = script.mock.write_history(owner.info()).split_off(before);
    assert_eq!(suffix.len(), 622);
    assert_eq!(
        suffix.iter().filter(|p| p.starts_with(&[2, 11])).count(),
        619
    );
    assert_eq!(script.opens.load(Ordering::SeqCst), 1);
    assert_eq!(script.closes.load(Ordering::SeqCst), 0);
    assert!(store
        .firmware_operations()
        .ensure_replay_allowed(owner.info())
        .is_err());
    owner.close().await.unwrap();
    manager.shutdown().await;
    assert_eq!(script.closes.load(Ordering::SeqCst), 1);
}
