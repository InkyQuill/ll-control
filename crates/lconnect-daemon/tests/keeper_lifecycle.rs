//! Real actor tree/private persistence/MockBackend; never opens physical HID.
use lconnect_daemon::{
    actor::{self, keeper_lifecycle::State, HidCommand},
    config::Config,
    persistence::PersistenceStore,
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, watch};

#[derive(Clone)]
struct Script {
    mock: MockBackend,
    info: DeviceInfo,
    opens: Arc<AtomicUsize>,
    gate_enabled: bool,
    entered: Arc<AtomicBool>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}
struct Handle {
    inner: Box<dyn OpenHandle>,
    script: Script,
}
impl Script {
    fn new(gated: bool) -> Self {
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            None,
            PathBuf::from("/mock/keepers"),
        );
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        Self {
            mock,
            info,
            opens: Arc::default(),
            gate_enabled: gated,
            entered: Arc::default(),
            gate: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }
    async fn entered(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !self.entered.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    fn history(&self) -> Vec<Vec<u8>> {
        self.mock.write_history(&self.info)
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
            script: self.clone(),
        }))
    }
}
impl OpenHandle for Handle {
    fn write(&self, bytes: &[u8]) -> lconnect_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 129]) {
            self.script.mock.queue_response(
                &self.script.info,
                ACommandPacket::new_request(129, &[0, 1, 0, 2, 1, 35, 2])
                    .as_bytes()
                    .to_vec(),
            );
        } else if bytes.starts_with(&[1, 134]) {
            self.script.mock.queue_response(
                &self.script.info,
                ACommandPacket::new_request(134, b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6")
                    .as_bytes()
                    .to_vec(),
            );
            let mut date = ACommandPacket::new_request(134, b"synthetic")
                .as_bytes()
                .to_vec();
            date[4] = 1;
            self.script.mock.queue_response(&self.script.info, date);
        } else if bytes.starts_with(&[2, 11]) {
            self.script
                .mock
                .queue_response(&self.script.info, bytes[..512].to_vec());
        } else if bytes.starts_with(&[2, 10]) {
            let mut reply = bytes[..512].to_vec();
            reply[12] = 1;
            self.script.mock.queue_response(&self.script.info, reply);
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout: i32) -> lconnect_hid::Result<usize> {
        if self.script.gate_enabled && !self.script.entered.swap(true, Ordering::AcqRel) {
            let guard = self.script.gate.0.lock().unwrap();
            let (guard, elapsed) = self
                .script
                .gate
                .1
                .wait_timeout_while(guard, Duration::from_secs(5), |open| !*open)
                .unwrap();
            assert!(
                *guard && !elapsed.timed_out(),
                "owned read gate not released"
            );
        }
        self.inner.read_timeout(bytes, timeout)
    }
}
struct Harness {
    _dir: tempfile::TempDir,
    script: Script,
    root: watch::Sender<bool>,
    handles: actor::DeviceHandles,
}
fn harness(gated: bool) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let initial = store.read("test").unwrap();
    let (root, shutdown) = watch::channel(false);
    let script = Script::new(gated);
    let mut config = Config::default();
    config.poll.sensor_hz = 100;
    let handles = actor::spawn_device_actor_tree(
        Arc::new(script.clone()),
        script.info.clone(),
        &config,
        store,
        "test".into(),
        initial,
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    Harness {
        _dir: dir,
        script,
        root,
        handles,
    }
}
async fn torn_down(handles: &actor::DeviceHandles) {
    let mut state = handles.teardown_rx.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*state.borrow() {
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn quiesce_waits_for_inflight_sensor_retains_same_hid_and_is_cancellation_safe() {
    let h = harness(true);
    h.script.entered().await;
    h.handles
        .settings_tx
        .send(actor::command_dispatch::SettingsUpdate::Lcd(
            lconnect_daemon::persistence::LcdSection {
                brightness: 47,
                rotation: "90".into(),
                control_mode: "LocalUI".into(),
            },
        ))
        .await
        .unwrap();
    let lifecycle = h.handles.keepers.clone();
    let observing = lifecycle.clone();
    let first = tokio::spawn(async move { lifecycle.quiesce().await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while observing.state() != State::Stopping {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!first.is_finished());
    assert!(!*h.handles.teardown_rx.borrow());
    first.abort();
    let _ = first.await;
    assert_eq!(observing.state(), State::Stopping);
    h.script.release();
    tokio::time::timeout(Duration::from_secs(2), observing.quiesce())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observing.state(), State::Joined);
    assert_eq!(
        PersistenceStore::new_with_root(h._dir.path().into())
            .read("test")
            .unwrap()
            .lcd
            .brightness,
        47
    );
    assert!(!h.handles.save_state.borrow().pending);

    observing.quiesce().await.unwrap();
    h.handles.quiesce_checked().await.unwrap();
    assert!(h.handles.lcd_tx.is_closed());
    assert!(h.handles.cooling_tx.is_closed());
    assert!(h.handles.lighting_state_tx.is_closed());
    assert!(h.handles.control_group_tx.is_closed());
    assert!(h.handles.settings_tx.is_closed());
    assert!(h.handles.snapshot_rx.has_changed().is_err());
    assert!(!h.handles.hid_io_tx.is_closed());
    assert!(!*h.handles.teardown_rx.borrow());
    let (reply, result) = oneshot::channel();
    h.handles
        .hid_io_tx
        .send(HidCommand::Firmware { reply })
        .await
        .unwrap();
    assert_eq!(result.await.unwrap().unwrap().version[1], 134);
    assert_eq!(h.script.opens.load(Ordering::SeqCst), 1);
    let after = h.script.history();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        h.script.history(),
        after,
        "joined producers must not keep polling/reissuing"
    );
    h.root.send(true).unwrap();
    torn_down(&h.handles).await;
}
#[tokio::test]
async fn manager_shutdown_and_sender_drop_still_join_the_whole_tree() {
    for drop_sender in [false, true] {
        let h = harness(false);
        let Harness {
            _dir,
            script,
            root,
            handles,
        } = h;
        if drop_sender {
            drop(root);
        } else {
            root.send(true).unwrap();
        }
        torn_down(&handles).await;
        assert_eq!(handles.keepers.state(), State::Joined);
        assert!(handles.hid_io_tx.is_closed());
        assert_eq!(script.opens.load(Ordering::SeqCst), 1);
        drop(_dir);
    }
}
#[tokio::test]
async fn root_shutdown_during_quiescence_joins_blocking_io_before_teardown() {
    let h = harness(true);
    h.script.entered().await;
    let lifecycle = h.handles.keepers.clone();
    let quiescing = tokio::spawn(async move { lifecycle.quiesce().await });
    h.root.send(true).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!*h.handles.teardown_rx.borrow());
    assert!(!quiescing.is_finished());
    h.script.release();
    quiescing.await.unwrap().unwrap();
    torn_down(&h.handles).await;
    assert!(h.handles.hid_io_tx.is_closed());
    assert_eq!(h.handles.keepers.state(), State::Joined);
}

#[tokio::test]
async fn joined_barrier_does_not_hide_failed_persistence_or_claim_firmware_readiness() {
    let h = harness(false);
    std::fs::create_dir_all(h._dir.path().join("devices/test.toml")).unwrap();
    h.handles
        .settings_tx
        .send(actor::command_dispatch::SettingsUpdate::Lcd(
            lconnect_daemon::persistence::LcdSection {
                brightness: 47,
                rotation: "90".into(),
                control_mode: "LocalUI".into(),
            },
        ))
        .await
        .unwrap();
    h.handles.keepers.quiesce().await.unwrap();
    assert_eq!(h.handles.keepers.state(), State::Joined);
    let state = h.handles.save_state.borrow().clone();
    assert!(state.pending);
    assert!(state.last_error.is_some());
    let writes = h.script.history();
    for _ in 0..2 {
        assert!(matches!(
            h.handles.quiesce_checked().await,
            Err(actor::keeper_lifecycle::QuiesceError::Persistence { pending: true, .. })
        ));
    }
    assert_eq!(h.script.history(), writes);

    h.root.send(true).unwrap();
    torn_down(&h.handles).await;
}

#[tokio::test]
#[ignore = "requires separately acquired vendor image; MockBackend only"]
async fn joined_tree_transfers_pinned_image_on_the_original_hid_handle_without_keeper_replay() {
    use actor::firmware_update::{CoolingHold, Phase, Progress, ValidatedImage};
    let h = harness(false);
    let saved_intent = lconnect_ipc::CoolingConfig {
        pump: Some(lconnect_ipc::CoolingPolicy::Motherboard { percent: 80 }),
        fan: None,
    };
    h.handles
        .settings_tx
        .send(actor::command_dispatch::SettingsUpdate::Cooling(
            saved_intent.clone(),
        ))
        .await
        .unwrap();
    h.handles.quiesce_checked().await.unwrap();
    let before = h.script.history().len();
    let path = std::env::var("LCONNECT_TEST_FIRMWARE_IMAGE").unwrap();
    let image = ValidatedImage::recognize(std::fs::read(path).unwrap()).unwrap();
    let (_cancel, cancel) = watch::channel(false);
    let (progress, _) = watch::channel(Progress::default());
    let (reply, result) = oneshot::channel();
    h.handles
        .hid_io_tx
        .send(HidCommand::FirmwareUpdate {
            operation: Box::new(
                PersistenceStore::new_with_root(h._dir.path().into())
                    .firmware_operations()
                    .arm(
                        &h.script.info,
                        CoolingHold::PumpOnly,
                        PersistenceStore::new_with_root(h._dir.path().into())
                            .read("test")
                            .unwrap(),
                    )
                    .unwrap(),
            ),
            cooling_hold: CoolingHold::PumpOnly,
            image,
            cancel,
            progress,
            reply,
        })
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(30), result)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome.progress.phase, Phase::AwaitingReconnect);
    let durable = PersistenceStore::new_with_root(h._dir.path().into())
        .firmware_operations()
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(
        durable.stage,
        lconnect_daemon::firmware_operation::Stage::AwaitingReconnect
    );
    assert_eq!(durable.progress.as_ref(), Some(&outcome.progress));

    assert_eq!(outcome.progress.chunk_echoes, 619);
    assert_eq!(outcome.progress.chunk_writes, 619);
    let writes = h.script.history();
    let update = &writes[before..];
    assert_eq!(update.len(), 622);
    assert_eq!(update[1][..2], [1, 138]);
    assert_eq!(&update[1][6..8], &[0, 100]);
    assert!(outcome.progress.pump_hold_written);
    assert!(!outcome.progress.fan_hold_written);
    assert_eq!(update[0][..2], [1, 134]);
    assert!(update[2..621].iter().all(|p| p.starts_with(&[2, 11])));
    assert_eq!(update[621][..2], [2, 10]);
    assert_eq!(h.script.opens.load(Ordering::SeqCst), 1);
    assert_eq!(
        PersistenceStore::new_with_root(h._dir.path().into())
            .read("test")
            .unwrap()
            .cooling,
        saved_intent
    );

    let (reply, result) = oneshot::channel();
    h.handles
        .hid_io_tx
        .send(HidCommand::Write {
            bytes: vec![1, 222],
            reply,
        })
        .await
        .unwrap();
    assert!(result.await.unwrap().is_err());
    assert_eq!(h.script.history(), writes);
    h.root.send(true).unwrap();
    torn_down(&h.handles).await;
}
