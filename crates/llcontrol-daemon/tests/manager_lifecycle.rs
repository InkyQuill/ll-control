use bytes::Bytes;
use llcontrol_daemon::{
    actor::LcdCommand,
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use llcontrol_ipc::{DaemonRpc, DeviceId, LcdSourceKind, LcdTheme};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tokio::sync::{oneshot, Notify};

fn info(serial: Option<&str>, path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        serial.map(str::to_owned),
        PathBuf::from(path),
    )
}

#[test]
fn stable_key_is_path_independent_and_serial_is_filename_safe() {
    let a = info(Some("../А/"), "/mock/a");
    let b = info(Some("../А/"), "/mock/b");
    assert_eq!(settings_key(&a), settings_key(&b));
    assert_eq!(settings_key(&a), "0416-7395-serial-2e2e2fd0902f");
    assert_eq!(settings_key(&info(None, "/a")), "0416-7395-singleton");
    assert_eq!(settings_key(&info(Some(""), "/b")), "0416-7395-singleton");
}

async fn wait_device(manager: &ManagerHandle) -> DeviceId {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                return device.id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("device attach")
}

struct Gate {
    first: AtomicBool,
    started: Notify,
    released: Mutex<bool>,
    wake: Condvar,
    opens: AtomicUsize,
}
// An assertion/timeout must release the fake blocking HID call before Tokio
// drops its runtime and waits for blocking workers during panic unwinding.
struct ReleaseGateOnExit(Arc<Gate>);
impl Drop for ReleaseGateOnExit {
    fn drop(&mut self) {
        *self
            .0
            .released
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.0.wake.notify_all();
    }
}
struct GatedBackend {
    mock: MockBackend,
    gate: Arc<Gate>,
}
struct GatedHandle {
    inner: Box<dyn OpenHandle>,
    gate: Arc<Gate>,
}
impl HidBackend for GatedBackend {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        self.gate.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(GatedHandle {
            inner: self.mock.open(info)?,
            gate: self.gate.clone(),
        }))
    }
}
impl OpenHandle for GatedHandle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        if bytes.get(1) == Some(&14) && self.gate.first.swap(false, Ordering::SeqCst) {
            self.gate.started.notify_one();
            let mut released = self.gate.released.lock().unwrap();
            while !*released {
                let (next, timeout) = self
                    .gate
                    .wake
                    .wait_timeout(released, Duration::from_secs(10))
                    .unwrap();
                released = next;
                if timeout.timed_out() && !*released {
                    return Err(llcontrol_hid::Error::BlockingTaskFailed(
                        "test HID gate was not released within10s".into(),
                    ));
                }
            }
        }
        self.inner.write(bytes)
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reattach_waits_for_old_batch_and_restores_stable_settings_with_idle_source() {
    let mock = MockBackend::new();
    let old = info(Some("M1-TEST"), "/mock/old");
    let new = info(Some("M1-TEST"), "/mock/new");
    mock.add_device(old.clone());
    let gate = Arc::new(Gate {
        first: AtomicBool::new(true),
        started: Notify::new(),
        released: Mutex::new(false),
        wake: Condvar::new(),
        opens: AtomicUsize::new(0),
    });
    let _release_on_exit = ReleaseGateOnExit(gate.clone());
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 65;
    settings.lcd.brightness = 70;
    settings.lcd.rotation = "90".into();
    settings.lcd.control_mode = "LocalUI".into();
    store.write(&settings_key(&old), &settings).unwrap();
    let mut config = Config::default();
    config.poll.hotplug_ms = 10;
    let manager = DeviceManager::new(
        Arc::new(GatedBackend {
            mock: mock.clone(),
            gate: gate.clone(),
        }),
        store,
        config,
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    let id = wait_device(&manager).await;
    let handles = manager
        .with_device(&id, |d| d.handles.clone())
        .await
        .unwrap();
    let (reply, response) = oneshot::channel();
    handles
        .lcd_tx
        .send(LcdCommand::SetImage {
            jpeg: Bytes::from_static(&[255, 216, 255, 217]),
            reply,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), gate.started.notified())
        .await
        .unwrap();
    mock.remove_device(&old);
    mock.add_device(new.clone());
    tokio::time::timeout(Duration::from_secs(2), async {
        while !manager.list().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // Manager is waiting on the old physical write. Mock removal deliberately
    // leaves its opened handle functional, so only a real join barrier suffices.
    assert_eq!(gate.opens.load(Ordering::SeqCst), 1);
    assert!(!*handles.teardown_rx.borrow());
    *gate.released.lock().unwrap() = true;
    gate.wake.notify_all();
    let _ = response.await;
    let new_id = wait_device(&manager).await;
    assert_ne!(id, new_id);
    assert_eq!(gate.opens.load(Ordering::SeqCst), 2);
    assert!(handles.hid_io_tx.is_closed());
    assert!(*handles.teardown_rx.borrow());
    let current = manager
        .with_device(&new_id, |d| d.handles.clone())
        .await
        .unwrap();
    let (reply, response) = oneshot::channel();
    current
        .lcd_tx
        .send(LcdCommand::State { reply })
        .await
        .unwrap();
    let state = response.await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert_eq!(state.applied.unwrap().brightness_percent, 70);
    assert!(mock
        .write_history(&new)
        .iter()
        .any(|p| p[1] == 139 && p[7] == 65));
    assert_eq!(
        mock.write_history(&old)
            .iter()
            .filter(|p| p[1] == 14)
            .count(),
        1
    );
    manager.shutdown().await;
}

#[tokio::test]
async fn ambiguous_serialless_units_are_not_assigned_singleton_settings() {
    let mock = MockBackend::new();
    mock.add_device(info(None, "/mock/a"));
    mock.add_device(info(None, "/mock/b"));
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let backend: Arc<dyn HidBackend> = Arc::new(mock.clone());
    let manager = DeviceManager::new(backend, store, Config::default(), Arc::new(Stats::new()))
        .spawn()
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(manager.list().await.is_empty());
    assert!(mock.write_history(&info(None, "/mock/a")).is_empty());
    manager.shutdown().await;
}

#[tokio::test]
async fn legacy_runtime_settings_are_preserved_but_not_automatically_replayed() {
    let mock = MockBackend::new();
    let device = info(Some("new-device"), "/mock/a");
    mock.add_device(device.clone());
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 75;
    store.write("galahad2-lcdv2:0001", &settings).unwrap();
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        Config::default(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    wait_device(&manager).await;
    assert!(!mock.write_history(&device).iter().any(|p| p[1] == 139));
    assert_eq!(
        store.read("galahad2-lcdv2:0001").unwrap().fan.pwm_percent,
        75
    );
    manager.shutdown().await;
}

// A USB path can be reused by another unit. Exercise live detach/replay, not
// just key construction, and compare the complete rendered frame on return.
#[tokio::test]
async fn theme_follows_serial_to_new_path_without_leaking_to_replacement_unit() {
    let mock = MockBackend::new();
    let original = info(Some("theme-owner"), "/mock/reused");
    let replacement = info(Some("other-owner"), "/mock/reused");
    let returned = info(Some("theme-owner"), "/mock/moved");
    mock.add_device(original.clone());
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let stats = Arc::new(Stats::new());
    let mut config = Config::default();
    config.poll.hotplug_ms = 10;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone());
    let original_id = wait_device(&manager).await;
    let theme: LcdTheme = serde_json::from_value(serde_json::json!({
        "version":1,"name":"Original screen","background":[20,30,40],
        "layers":[{"kind":"sensor","sensor_id":"fixture:original-cpu",
            "label":"Original CPU","unit":"celsius","temperature_unit":"fahrenheit",
            "decimals":1,"style":{"x":240,"y":220,"size":32,
                "color":[255,255,255],"align":"center"}}]
    }))
    .unwrap();
    let preview = rpc.preview_lcd_theme(None, theme.clone()).await.unwrap();
    let expected: Vec<Vec<u8>> =
        llcontrol_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(&preview)
            .unwrap()
            .into_iter()
            .map(|p| p.to_vec())
            .collect();
    rpc.set_lcd_theme(original_id.clone(), theme.clone())
        .await
        .unwrap();
    let old_handles = manager
        .with_device(&original_id, |d| d.handles.clone())
        .await
        .unwrap();
    mock.remove_device(&original);
    tokio::time::timeout(Duration::from_secs(3), async {
        while !*old_handles.teardown_rx.borrow() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("original actor must finish before the path is reused");
    // MockBackend keeps history by path, including the disconnected unit's
    // packets. Start the replacement observation only after joined teardown.
    let before_replacement = mock.write_history(&original).len();
    mock.add_device(replacement.clone());
    let replacement_id = wait_replacement(&manager, &original_id).await;
    assert!(rpc
        .get_lcd_theme(replacement_id.clone())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        rpc.get_lcd_state(replacement_id.clone())
            .await
            .unwrap()
            .source,
        LcdSourceKind::Idle
    );
    assert!(
        !mock.write_history(&replacement)[before_replacement..]
            .iter()
            .any(|p| p.get(1) == Some(&14)),
        "replacement must not receive the original owner's image"
    );
    assert_eq!(
        store.read(&settings_key(&original)).unwrap().theme,
        Some(theme.clone()),
        "detach must flush the original theme with its binding"
    );

    let mut other_theme = theme.clone();
    other_theme.name = "Other screen".into();
    other_theme.background = [200, 40, 10];
    rpc.set_lcd_theme(replacement_id.clone(), other_theme.clone())
        .await
        .unwrap();
    mock.remove_device(&replacement);
    mock.add_device(returned.clone());
    let returned_id = wait_replacement(&manager, &replacement_id).await;
    assert_eq!(
        rpc.get_lcd_theme(returned_id.clone()).await.unwrap(),
        Some(theme.clone())
    );
    let state = rpc.get_lcd_state(returned_id).await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Theme);
    assert!(state.last_error.is_none());
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if mock
                .write_history(&returned)
                .windows(expected.len())
                .any(|batch| batch == expected)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("returned serial must send its own complete rendered frame");
    manager.shutdown().await;
    assert_eq!(
        store.read(&settings_key(&returned)).unwrap().theme,
        Some(theme)
    );
    assert_eq!(
        store.read(&settings_key(&replacement)).unwrap().theme,
        Some(other_theme)
    );
}

async fn wait_replacement(manager: &ManagerHandle, previous: &DeviceId) -> DeviceId {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                if &device.id != previous {
                    return device.id.clone();
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("replacement attachment must finish replay")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sleep_ack_waits_for_old_batch_and_wake_reopens_still_present_device() {
    let mock = MockBackend::new();
    let old = info(Some("M1-TEST"), "/mock/old");
    let new = old.clone();
    mock.add_device(old.clone());
    let gate = Arc::new(Gate {
        first: AtomicBool::new(true),
        started: Notify::new(),
        released: Mutex::new(false),
        wake: Condvar::new(),
        opens: AtomicUsize::new(0),
    });
    let _release_on_exit = ReleaseGateOnExit(gate.clone());
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 65;
    settings.lcd.brightness = 70;
    settings.lcd.rotation = "90".into();
    settings.lcd.control_mode = "LocalUI".into();
    store.write(&settings_key(&old), &settings).unwrap();
    let mut config = Config::default();
    config.poll.hotplug_ms = 10;
    let manager = DeviceManager::new(
        Arc::new(GatedBackend {
            mock: mock.clone(),
            gate: gate.clone(),
        }),
        store,
        config,
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    let id = wait_device(&manager).await;
    let handles = manager
        .with_device(&id, |d| d.handles.clone())
        .await
        .unwrap();
    let (reply, response) = oneshot::channel();
    handles
        .lcd_tx
        .send(LcdCommand::SetImage {
            jpeg: Bytes::from_static(&[255, 216, 255, 217]),
            reply,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), gate.started.notified())
        .await
        .unwrap();
    let sleeper = manager.clone();
    let sleep = tokio::spawn(async move { sleeper.prepare_for_sleep(true).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !manager.list().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // A sleep acknowledgement must wait for the physical write, not just
    // removal from the IPC map. The USB device stays enumerated throughout.
    assert!(!sleep.is_finished());
    assert_eq!(gate.opens.load(Ordering::SeqCst), 1);
    assert!(!*handles.teardown_rx.borrow());
    *gate.released.lock().unwrap() = true;
    gate.wake.notify_all();
    let _ = response.await;
    sleep.await.unwrap().unwrap();
    manager.prepare_for_sleep(true).await.unwrap();
    let writes = mock.write_history(&old).len();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(manager.list().await.is_empty());
    assert_eq!(mock.write_history(&old).len(), writes);
    assert_eq!(gate.opens.load(Ordering::SeqCst), 1);
    manager.prepare_for_sleep(false).await.unwrap();
    let new_id = wait_device(&manager).await;
    manager.prepare_for_sleep(false).await.unwrap();
    assert_eq!(wait_device(&manager).await, new_id);
    assert_ne!(id, new_id);
    assert_eq!(gate.opens.load(Ordering::SeqCst), 2);
    assert!(handles.hid_io_tx.is_closed());
    assert!(*handles.teardown_rx.borrow());
    let current = manager
        .with_device(&new_id, |d| d.handles.clone())
        .await
        .unwrap();
    let (reply, response) = oneshot::channel();
    current
        .lcd_tx
        .send(LcdCommand::State { reply })
        .await
        .unwrap();
    let state = response.await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert_eq!(state.applied.unwrap().brightness_percent, 70);
    assert!(mock.write_history(&new)[writes..]
        .iter()
        .any(|p| p[1] == 139 && p[7] == 65));
    assert_eq!(
        mock.write_history(&old)
            .iter()
            .filter(|p| p[1] == 14)
            .count(),
        1
    );
    manager.shutdown().await;
    assert!(
        tokio::time::timeout(Duration::from_secs(2), manager.prepare_for_sleep(false))
            .await
            .expect("power request after shutdown must terminate")
            .is_err()
    );
}

#[tokio::test]
async fn sleep_flushes_theme_and_wake_restores_it_without_daemon_restart() {
    let mock = MockBackend::new();
    let device = info(Some("sleep-theme"), "/mock/sleep-theme");
    mock.add_device(device.clone());
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone());
    let original_id = wait_device(&manager).await;
    let theme: LcdTheme = serde_json::from_value(serde_json::json!({
        "version":1,"name":"Wake theme","background":[40,70,90],"layers":[]
    }))
    .unwrap();
    rpc.set_lcd_theme(original_id.clone(), theme.clone())
        .await
        .unwrap();
    manager.prepare_for_sleep(true).await.unwrap();
    assert!(manager.list().await.is_empty());
    assert!(rpc.get_lcd_state(original_id).await.is_err());
    assert_eq!(
        store.read(&settings_key(&device)).unwrap().theme,
        Some(theme.clone())
    );
    let before = mock.write_history(&device).len();
    manager.prepare_for_sleep(false).await.unwrap();
    let id = wait_device(&manager).await;
    assert_eq!(rpc.get_lcd_theme(id.clone()).await.unwrap(), Some(theme));
    let state = rpc.get_lcd_state(id).await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Theme);
    assert!(state.last_error.is_none());
    assert!(mock.write_history(&device)[before..]
        .iter()
        .any(|p| p.get(1) == Some(&14)));
    // Shutdown while suspended must not wait for a wake event.
    manager.prepare_for_sleep(true).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
        .await
        .unwrap();
}

#[tokio::test]
async fn panic_exit_releases_blocked_mock_hid_worker_before_runtime_shutdown() {
    let mock = MockBackend::new();
    let target = info(Some("PANIC-GATE"), "/mock/panic-gate");
    mock.add_device(target.clone());
    let gate = Arc::new(Gate {
        first: AtomicBool::new(true),
        started: Notify::new(),
        released: Mutex::new(false),
        wake: Condvar::new(),
        opens: AtomicUsize::new(0),
    });
    let handle = GatedHandle {
        inner: mock.open(&target).unwrap(),
        gate: gate.clone(),
    };
    let writer = std::thread::spawn(move || handle.write(&[1, 14]));
    tokio::time::timeout(Duration::from_secs(2), gate.started.notified())
        .await
        .unwrap();
    let exit = std::thread::spawn(move || {
        let _release_on_exit = ReleaseGateOnExit(gate);
        panic!("injected gated test failure");
    });
    assert!(exit.join().is_err());
    writer.join().unwrap().unwrap();
    assert_eq!(mock.write_history(&target), vec![vec![1, 14]]);
}
