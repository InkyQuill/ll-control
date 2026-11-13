//! Software lifecycle qualification only: temporary stores and MockBackend.
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use llcontrol_ipc::{
    DaemonRpc, DeviceId, LcdSourceKind, NamedProfile, ProfileContent, ProfileEdit, ProfileGroup,
    ScreenCollections,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

async fn start(
    backend: Arc<dyn HidBackend>,
    store: Arc<PersistenceStore>,
) -> (ManagerHandle, IpcServerImpl) {
    let mut config = Config::default();
    config.poll.hotplug_ms = 20;
    let stats = Arc::new(Stats::default());
    let manager = DeviceManager::new(backend, store.clone(), config.clone(), stats.clone())
        .spawn()
        .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store);
    (manager, rpc)
}

async fn ready(manager: &ManagerHandle, rpc: &IpcServerImpl) -> DeviceId {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                if let Ok(state) = rpc.get_lcd_state(device.id.clone()).await {
                    if state.source == LcdSourceKind::Theme && state.last_error.is_none() {
                        return device.id.clone();
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("theme replay must complete")
}

async fn verify_replay(
    rpc: &IpcServerImpl,
    dev: DeviceId,
    id: &str,
    default: &llcontrol_ipc::LcdTheme,
) {
    let before = rpc.get_screen_reset(dev.clone()).await.unwrap();
    let receipt = before.receipt.as_ref().unwrap();
    assert_eq!(receipt.operation_id, id);
    assert_eq!(receipt.profile_revision, 3);
    assert_eq!(receipt.collection_revision, 2);
    assert!(
        !before.reapply_available,
        "retry capability must not cross attachments"
    );
    assert_eq!(before.lcd.requested.brightness_percent, 50);
    assert_eq!(before.lcd.requested.rotation_degrees, 0);
    assert_eq!(before.lcd.requested.mode, "Application");
    let applied = before
        .lcd
        .applied
        .as_ref()
        .expect("transport config acknowledged");
    assert_eq!(applied.brightness_percent, 50);
    assert_eq!(applied.rotation_degrees, 0);
    assert_eq!(applied.mode, "Application");
    assert_eq!(
        rpc.get_lcd_theme(dev.clone()).await.unwrap().as_ref(),
        Some(default)
    );
    // A caller recovering an old lost reply must not perform the reset again.
    let repeated = rpc
        .reset_screen(dev.clone(), id.into(), 2, 1)
        .await
        .unwrap();
    assert!(repeated.repeated);
    assert_eq!(repeated.lcd.generation, before.lcd.generation);
    assert!(rpc
        .reapply_screen_reset(dev, id.into(), before.lcd.generation)
        .await
        .is_err());
}

#[tokio::test]
async fn screen_reset_replays_across_sleep_reattach_and_fresh_daemon_without_resetting_neighbors() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let mock = MockBackend::new();
    let mut info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("reset-lifecycle".into()),
        "/mock/reset-first".into(),
    );
    let key = settings_key(&info);
    let ProfileContent::Screen(default) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        panic!("screen default")
    };
    let mut custom = default.clone();
    custom.name = "Before reset".into();
    let settings: DeviceSettings = serde_json::from_value(serde_json::json!({
        "theme": custom,
        "fan":{"pwm_percent":67}, "pump":{"pwm_percent":82,"source":"mcu"},
        "lcd":{"brightness":77,"rotation":"90","control_mode":"Application"}
    }))
    .unwrap();
    store.write(&key, &settings).unwrap();
    let mut screen = NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap();
    screen.name = "User screen slot".into();
    store
        .profiles()
        .edit(0, ProfileEdit::Save { profile: screen })
        .unwrap();
    let fan = NamedProfile::defaults(ProfileGroup::Fan, 0, Some("fixture:cpu")).unwrap();
    store
        .profiles()
        .edit(
            1,
            ProfileEdit::Save {
                profile: fan.clone(),
            },
        )
        .unwrap();
    let image = store
        .theme_assets()
        .import(include_bytes!("../../../desktop/src-tauri/icons/icon.png"))
        .unwrap();
    mock.add_device(info.clone());
    let (manager, rpc) = start(Arc::new(mock.clone()), store.clone()).await;
    let first = ready(&manager, &rpc).await;
    rpc.migrate_screen_collections(first.clone()).await.unwrap();
    let id = "a".repeat(32);
    rpc.reset_screen(first.clone(), id.clone(), 2, 1)
        .await
        .unwrap();
    verify_replay(&rpc, first.clone(), &id, &default).await;

    manager.prepare_for_sleep(true).await.unwrap();
    assert!(manager.list().await.is_empty());
    manager.prepare_for_sleep(false).await.unwrap();
    let awake = ready(&manager, &rpc).await;
    assert_ne!(first, awake);
    verify_replay(&rpc, awake.clone(), &id, &default).await;

    mock.remove_device(&info);
    tokio::time::timeout(Duration::from_secs(3), async {
        while !manager.list().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    info.path = "/mock/reset-returned".into();
    mock.add_device(info.clone());
    let returned = ready(&manager, &rpc).await;
    assert_ne!(awake, returned);
    verify_replay(&rpc, returned, &id, &default).await;
    manager.shutdown().await;
    drop(rpc);
    drop(store);

    let fresh = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let (manager, rpc) = start(Arc::new(mock.clone()), fresh.clone()).await;
    let restarted = ready(&manager, &rpc).await;
    verify_replay(&rpc, restarted, &id, &default).await;
    manager.shutdown().await;
    let saved = fresh.read(&key).unwrap();
    assert_eq!(
        saved.screen_collections,
        Some(ScreenCollections::defaults(2))
    );
    let before = serde_json::to_value(settings).unwrap();
    let after = serde_json::to_value(saved).unwrap();
    for field in [
        "fan",
        "pump",
        "cooling",
        "lighting",
        "thermal",
        "dynamic_rgb",
    ] {
        assert_eq!(before.get(field), after.get(field), "preserve {field}");
    }
    let profiles = fresh.profiles().list().unwrap();
    assert_eq!(profiles.revision, 3);
    assert!(profiles.profiles.contains(&fan));
    for slot in 0..8 {
        assert!(profiles
            .profiles
            .contains(&NamedProfile::defaults(ProfileGroup::Screen, slot, None).unwrap()));
    }
    assert!(fresh.theme_assets().list().unwrap().contains(&image));
}

struct FailResetConfig {
    mock: MockBackend,
    fail: Arc<AtomicBool>,
}
struct FailingHandle {
    inner: Box<dyn OpenHandle>,
    fail: Arc<AtomicBool>,
}
impl HidBackend for FailResetConfig {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(FailingHandle {
            inner: self.mock.open(info)?,
            fail: self.fail.clone(),
        }))
    }
}
impl OpenHandle for FailingHandle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        if bytes.get(1) == Some(&12) && self.fail.swap(false, Ordering::SeqCst) {
            return Err(llcontrol_hid::Error::BlockingTaskFailed(
                "reset config fault".into(),
            ));
        }
        self.inner.write(bytes)
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}

#[tokio::test]
async fn failed_reset_replays_after_sleep_without_reusing_old_retry_capability() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("reset-fault".into()),
        "/mock/reset-fault".into(),
    );
    let ProfileContent::Screen(theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        panic!("default")
    };
    store
        .write(
            &settings_key(&info),
            &DeviceSettings {
                theme: Some(theme.clone()),
                ..DeviceSettings::default()
            },
        )
        .unwrap();
    mock.add_device(info);
    let fail = Arc::new(AtomicBool::new(false));
    let (manager, rpc) = start(
        Arc::new(FailResetConfig {
            mock,
            fail: fail.clone(),
        }),
        store.clone(),
    )
    .await;
    let old = ready(&manager, &rpc).await;
    fail.store(true, Ordering::SeqCst);
    let id = "b".repeat(32);
    let reset = rpc
        .reset_screen(old.clone(), id.clone(), 0, 0)
        .await
        .unwrap();
    assert!(reset
        .lcd
        .last_error
        .as_deref()
        .unwrap()
        .contains("reset config fault"));
    assert!(
        rpc.get_screen_reset(old.clone())
            .await
            .unwrap()
            .reapply_available
    );
    manager.prepare_for_sleep(true).await.unwrap();
    manager.prepare_for_sleep(false).await.unwrap();
    let new = ready(&manager, &rpc).await;
    assert_ne!(old, new);
    let read = rpc.get_screen_reset(new.clone()).await.unwrap();
    assert_eq!(read.receipt.unwrap().operation_id, id);
    assert!(!read.reapply_available);
    assert!(read.lcd.last_error.is_none());
    let applied = read.lcd.applied.unwrap();
    assert_eq!(
        (
            applied.mode.as_str(),
            applied.brightness_percent,
            applied.rotation_degrees
        ),
        ("Application", 50, 0)
    );
    assert_eq!(rpc.get_lcd_theme(new.clone()).await.unwrap(), Some(theme));
    assert!(rpc
        .reapply_screen_reset(old, id.clone(), reset.lcd.generation)
        .await
        .is_err());
    assert!(rpc
        .reapply_screen_reset(new, id, read.lcd.generation)
        .await
        .is_err());
    manager.shutdown().await;
    assert_eq!(store.profiles().list().unwrap().revision, 1);
}

struct ResetGate {
    armed: AtomicBool,
    entered: tokio::sync::Notify,
    released: std::sync::Mutex<bool>,
    wake: std::sync::Condvar,
}
impl ResetGate {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct ReleaseOnDrop(Arc<ResetGate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct HeldResetBackend {
    mock: MockBackend,
    gate: Arc<ResetGate>,
}
struct HeldResetHandle {
    inner: Box<dyn OpenHandle>,
    gate: Arc<ResetGate>,
}
impl HidBackend for HeldResetBackend {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        Ok(Box::new(HeldResetHandle {
            inner: self.mock.open(info)?,
            gate: self.gate.clone(),
        }))
    }
}
impl OpenHandle for HeldResetHandle {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        if bytes.get(1) == Some(&12) && self.gate.armed.swap(false, Ordering::SeqCst) {
            self.gate.entered.notify_one();
            let mut released = self.gate.released.lock().unwrap();
            while !*released {
                released = self.gate.wake.wait(released).unwrap();
            }
        }
        self.inner.write(bytes)
    }
    fn read_timeout(&self, bytes: &mut [u8], ms: i32) -> llcontrol_hid::Result<usize> {
        self.inner.read_timeout(bytes, ms)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sleep_joins_inflight_reset_after_caller_loss_and_replays_committed_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("held-reset".into()),
        "/mock/held-reset".into(),
    );
    let key = settings_key(&info);
    let ProfileContent::Screen(theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        panic!("default")
    };
    store
        .write(
            &key,
            &DeviceSettings {
                theme: Some(theme.clone()),
                ..DeviceSettings::default()
            },
        )
        .unwrap();
    mock.add_device(info.clone());
    let gate = Arc::new(ResetGate {
        armed: AtomicBool::new(false),
        entered: tokio::sync::Notify::new(),
        released: std::sync::Mutex::new(false),
        wake: std::sync::Condvar::new(),
    });
    // Even a failed assertion must release the blocking mock HID worker.
    let _release_on_failure = ReleaseOnDrop(gate.clone());
    let (manager, rpc) = start(
        Arc::new(HeldResetBackend {
            mock: mock.clone(),
            gate: gate.clone(),
        }),
        store.clone(),
    )
    .await;
    let rpc = Arc::new(rpc);
    let old = ready(&manager, &rpc).await;
    let handles = manager
        .with_device(&old, |d| d.handles.clone())
        .await
        .unwrap();
    gate.armed.store(true, Ordering::SeqCst);
    let id = "c".repeat(32);
    let task_rpc = rpc.clone();
    let task_id = id.clone();
    let task_dev = old.clone();
    let caller = tokio::spawn(async move { task_rpc.reset_screen(task_dev, task_id, 0, 0).await });
    tokio::time::timeout(Duration::from_secs(3), gate.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        store
            .read(&key)
            .unwrap()
            .screen_reset_receipt
            .unwrap()
            .operation_id,
        id
    );
    caller.abort();
    let _ = caller.await;
    let sleep_manager = manager.clone();
    let mut sleep = tokio::spawn(async move { sleep_manager.prepare_for_sleep(true).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !manager.list().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut sleep)
            .await
            .is_err(),
        "sleep cannot acknowledge while a physical write is held"
    );
    assert!(!*handles.teardown_rx.borrow());
    gate.release();
    tokio::time::timeout(Duration::from_secs(3), sleep)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(*handles.teardown_rx.borrow());
    let writes_after_join = mock.write_history(&info).len();
    // An acknowledged sleep has no live old actor to accept another mutation.
    let (reply, response) = tokio::sync::oneshot::channel();
    assert!(handles
        .lcd_tx
        .send(llcontrol_daemon::actor::LcdCommand::Clear { reply })
        .await
        .is_err());
    assert!(response.await.is_err());
    assert_eq!(mock.write_history(&info).len(), writes_after_join);
    manager.prepare_for_sleep(false).await.unwrap();
    let new = ready(&manager, &rpc).await;
    assert_ne!(old, new);
    let read = rpc.get_screen_reset(new.clone()).await.unwrap();
    assert_eq!(read.receipt.unwrap().operation_id, id);
    assert!(read.lcd.last_error.is_none());
    assert_eq!(read.lcd.applied.unwrap().brightness_percent, 50);
    assert_eq!(rpc.get_lcd_theme(new.clone()).await.unwrap(), Some(theme));
    let repeated = rpc.reset_screen(new, id, 0, 0).await.unwrap();
    assert!(repeated.repeated);
    assert_eq!(repeated.lcd.generation, read.lcd.generation);
    manager.shutdown().await;
    assert_eq!(store.profiles().list().unwrap().revision, 1);
    assert_eq!(
        store.read(&key).unwrap().screen_collections,
        Some(ScreenCollections::defaults(1))
    );
}
