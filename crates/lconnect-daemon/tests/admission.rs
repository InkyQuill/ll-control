//! Shared production IPC/manager admission with private stores and MockBackend.
use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, DaemonRpc, DeviceId, IpcError,
};
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn sealed_admission_rejects_wire_commands_storage_jobs_and_discovery_but_shutdown_joins() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let mock = MockBackend::new();
    let stats = Arc::new(Stats::new());
    let mut config = Config::default();
    config.poll.hotplug_ms = 5;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = Arc::new(IpcServerImpl::new(
        manager.clone(),
        stats,
        config,
        store.clone(),
    ));
    let gate = store.admission();
    tokio::time::timeout(Duration::from_secs(2), gate.close().wait())
        .await
        .unwrap();
    let (client_end, server_end) = mem_pair();
    let server = IpcServer::new(rpc.clone());
    let serving = tokio::spawn(async move { server.serve_stream(server_end).await });
    let client = IpcClient::from_stream(client_end).await.unwrap();
    assert!(matches!(
        client.call("list_devices", serde_json::json!({})).await,
        Err(IpcError::DeviceBusy(_))
    ));
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), serving)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    // Direct trait paths must have the same guard, including read-only and
    // delegating methods; storage workers cannot bypass it either.
    assert!(matches!(
        rpc.list_devices().await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        rpc.list_profiles().await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        rpc.get_system_spec(false).await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        rpc.get_firmware(DeviceId::new("missing")).await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        rpc.import_theme_asset(vec![]).await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        rpc.get_display_settings().await,
        Err(IpcError::DeviceBusy(_))
    ));
    assert!(matches!(
        store.display_settings().save(Default::default()).await,
        Err(IpcError::DeviceBusy(_))
    ));
    let target = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("sealed".into()),
        "/mock/sealed".into(),
    );
    mock.add_device(target.clone());
    tokio::time::sleep(Duration::from_millis(35)).await;
    assert!(manager.list().await.is_empty());
    assert!(mock.write_history(&target).is_empty());
    assert!(manager.prepare_for_sleep(true).await.is_err());
    tokio::time::timeout(Duration::from_secs(2), manager.shutdown())
        .await
        .unwrap();
    assert!(gate.is_closed());
    assert_eq!(gate.active(), 0);
    assert!(!root.path().join("display-settings.json").exists());
}

#[tokio::test]
async fn scoped_maintenance_clears_real_staging_and_joins_anonymous_file_disposal() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        store.clone(),
        Config::default(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(
        manager.clone(),
        Arc::new(Stats::new()),
        Config::default(),
        store.clone(),
    );
    let before = store.media_assets().list().unwrap();
    rpc.font_upload(lconnect_ipc::FontUploadRequest::Begin { size: 16 })
        .await
        .unwrap();
    rpc.media_upload(lconnect_ipc::MediaUploadRequest::Begin {
        id: "11111111111111111111111111111111".into(),
        name: "Unfinished".into(),
        frames: 1,
        encoded_bytes: 5,
    })
    .await
    .unwrap();
    rpc.profile_archive(lconnect_ipc::ProfileArchiveRequest::BeginImport {
        id: "22222222222222222222222222222222".into(),
        size: 100,
        expected_revision: 0,
        group: lconnect_ipc::ProfileGroup::Fan,
        slot: 1,
    })
    .await
    .unwrap();
    let anonymous_fds = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|e| std::fs::read_link(e.ok()?.path()).ok())
            .filter(|p| {
                p.to_string_lossy()
                    .contains(&root.path().to_string_lossy().to_string())
                    && p.to_string_lossy().contains("(deleted)")
            })
            .count()
    };
    assert!(anonymous_fds() >= 1);
    let foreign = lconnect_daemon::admission::Admission::default()
        .begin_maintenance()
        .unwrap();
    assert!(rpc.close_staging(&foreign).await.is_err());
    foreign.wait_checked().await.unwrap();
    assert!(!store.admission().is_closed());
    let gate = store.admission();
    let maintenance = gate.begin_maintenance().unwrap();
    rpc.close_staging(&maintenance).await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), maintenance.wait_checked())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(gate.active(), 0);
    assert_eq!(anonymous_fds(), 0);
    assert_eq!(store.media_assets().list().unwrap(), before);
    assert!(matches!(
        rpc.list_profiles().await,
        Err(IpcError::DeviceBusy(_))
    ));
    manager.shutdown().await;
}
