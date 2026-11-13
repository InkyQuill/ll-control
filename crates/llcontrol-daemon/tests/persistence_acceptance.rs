//! User-visible persistence semantics through JSON-RPC and joined manager teardown.
//! All writes use a temporary settings root and MockBackend; no physical device.
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, DeviceId, LcdState,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};

struct Session {
    manager: ManagerHandle,
    client: IpcClient,
    server: tokio::task::JoinHandle<()>,
    id: DeviceId,
}
impl Session {
    async fn start(store: Arc<PersistenceStore>, path: &str) -> Self {
        let mock = MockBackend::new();
        mock.add_device(info(path));
        let manager = DeviceManager::new(
            Arc::new(mock),
            store.clone(),
            Config::default(),
            Arc::new(Stats::new()),
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
        let rpc = Arc::new(IpcServerImpl::new(
            manager.clone(),
            Arc::new(Stats::new()),
            Config::default(),
            store,
        ));
        let (a, b) = mem_pair();
        let server = tokio::spawn(async move {
            let _ = IpcServer::new(rpc).serve_stream(b).await;
        });
        let client = IpcClient::from_stream(a).await.unwrap();
        let session = Self {
            manager,
            client,
            server,
            id,
        };
        // Require successful replay of this valid fixture before issuing mutations.
        session.until(|s| s.applied.is_some()).await;
        session
    }
    async fn state(&self) -> LcdState {
        serde_json::from_value(
            self.client
                .call("get_lcd_state", json!({"dev":self.id}))
                .await
                .unwrap(),
        )
        .unwrap()
    }
    async fn patch(&self, patch: serde_json::Value) -> LcdState {
        serde_json::from_value(
            self.client
                .call("patch_lcd_config", json!({"dev":self.id,"patch":patch}))
                .await
                .unwrap(),
        )
        .unwrap()
    }
    async fn until(&self, predicate: impl Fn(&LcdState) -> bool) -> LcdState {
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let s = self.state().await;
                if predicate(&s) {
                    break s;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn stop(self) {
        tokio::time::timeout(Duration::from_secs(4), self.manager.shutdown())
            .await
            .expect("joined shutdown must complete");
        drop(self.client);
        self.server.abort();
        let _ = self.server.await;
    }
}
fn info(path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("PERSIST-ACCEPTANCE".into()),
        PathBuf::from(path),
    )
}

#[tokio::test]
async fn ipc_disk_error_retries_then_joined_shutdown_replays_latest_settings() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let key = settings_key(&info("/mock/first"));
    let mut initial = DeviceSettings::default();
    initial.lcd.brightness = 30;
    initial.lcd.rotation = "0".into();
    initial.lcd.control_mode = "LocalUI".into();
    initial.fan.pwm_percent = 61;
    store.write(&key, &initial).unwrap();
    let path = dir.path().join("devices").join(format!("{key}.toml"));
    let original = std::fs::read(&path).unwrap();
    // Fails even as root, while keeping the previous durable file readable.
    let blocker = path.with_extension("toml.tmp");
    std::fs::create_dir(&blocker).unwrap();
    let session = Session::start(store.clone(), "/mock/first").await;
    let applied = session.patch(json!({"brightness_percent":71})).await;
    assert_eq!(applied.applied.unwrap().brightness_percent, 71);
    assert!(applied.last_error.is_none());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "Apply is not a durable-write acknowledgement"
    );
    let failed = session
        .until(|s| {
            s.persistence
                .as_ref()
                .is_some_and(|p| p.last_error.is_some())
        })
        .await;
    let status = failed.persistence.unwrap();
    assert!(status.pending);
    assert!(!status.saved);
    assert!(
        failed.last_error.is_none(),
        "disk error must not become an LCD transport error"
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir(&blocker).unwrap();
    let saved = session
        .until(|s| {
            s.persistence
                .as_ref()
                .is_some_and(|p| p.saved && !p.pending && p.last_error.is_none())
        })
        .await;
    assert_eq!(saved.applied.unwrap().brightness_percent, 71);
    assert_eq!(store.read(&key).unwrap().lcd.brightness, 71);
    // A second, independent field is accepted before immediate joined shutdown.
    session.patch(json!({"rotation_degrees":90})).await;
    session.stop().await;
    let reloaded = PersistenceStore::new_with_root(dir.path().into())
        .read(&key)
        .unwrap();
    assert_eq!(reloaded.lcd.brightness, 71);
    assert_eq!(reloaded.lcd.rotation, "90");
    assert_eq!(reloaded.fan.pwm_percent, 61);
    let restarted = Session::start(store, "/mock/renumbered").await;
    let replayed = restarted.state().await;
    let applied = replayed.applied.unwrap();
    assert_eq!(applied.brightness_percent, 71);
    assert_eq!(applied.rotation_degrees, 90);
    assert_eq!(replayed.requested.rotation_degrees, 90);
    assert!(replayed.last_error.is_none());
    restarted.stop().await;
}

#[tokio::test]
async fn failed_final_flush_retains_error_and_previous_durable_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let key = settings_key(&info("/mock/a"));
    let mut initial = DeviceSettings::default();
    initial.lcd.brightness = 35;
    initial.lcd.rotation = "0".into();
    initial.lcd.control_mode = "LocalUI".into();
    store.write(&key, &initial).unwrap();
    let session = Session::start(store.clone(), "/mock/a").await;
    let status = session
        .manager
        .with_device(&session.id, |d| d.handles.save_state.clone())
        .await
        .unwrap();
    let path = dir.path().join("devices").join(format!("{key}.toml"));
    let original = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("toml.tmp")).unwrap();
    session.patch(json!({"brightness_percent":82})).await;
    session.stop().await;
    let final_status = status.borrow().clone();
    assert!(final_status.pending);
    assert!(!final_status.saved);
    assert!(final_status.last_error.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(store.read(&key).unwrap().lcd.brightness, 35);
    let restarted = Session::start(store, "/mock/b").await;
    assert_eq!(
        restarted.state().await.applied.unwrap().brightness_percent,
        35
    );
    restarted.stop().await;
}
