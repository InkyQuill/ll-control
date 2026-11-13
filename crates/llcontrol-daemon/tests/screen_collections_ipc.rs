#[path = "support/display_firmware.rs"]
mod display_firmware;
use display_firmware::FirmwareMock;
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, ScreenCollections,
    ScreenCollectionsState,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn collection_rpc_reports_legacy_then_durable_membership_and_rejects_stale_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("collections".into()),
        "/mock/collections".into(),
    );
    let key = settings_key(&info);
    mock.add_device(info);
    let mut config = Config::default();
    config.poll.hotplug_ms = 20;
    let stats = Arc::new(Stats::default());
    let manager = DeviceManager::new(
        Arc::new(FirmwareMock(mock)),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                break device.id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (a, b) = mem_pair();
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone());
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let state: ScreenCollectionsState = serde_json::from_value(
        client
            .call("get_screen_collections", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        state.collections.is_none(),
        "legacy must not look like an empty catalog"
    );
    let image = store
        .theme_assets()
        .import(include_bytes!("../../../desktop/src-tauri/icons/icon.png"))
        .unwrap();
    let migrated: ScreenCollections = serde_json::from_value(
        client
            .call("migrate_screen_collections", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(migrated.images.contains(&image));
    assert!(migrated.watermarks.contains(&image));
    assert!(client
        .call("migrate_screen_collections", json!({"dev":dev}))
        .await
        .is_err());
    let mut collections = ScreenCollections::defaults(2);
    collections.watermarks.push(image.clone());
    let params = json!({"dev":dev,"expected_revision":1,"collections":collections});
    let saved: ScreenCollections = serde_json::from_value(
        client
            .call("set_screen_collections", params.clone())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved, collections);
    assert!(client.call("set_screen_collections", params).await.is_err());
    let state: ScreenCollectionsState = serde_json::from_value(
        client
            .call("get_screen_collections", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.collections, Some(collections.clone()));
    assert!(!state.persistence.pending);
    assert!(state.persistence.saved);
    assert!(state.persistence.last_error.is_none());
    assert_eq!(
        store.read(&key).unwrap().screen_collections,
        Some(collections.clone())
    );
    let mut invalid = ScreenCollections::defaults(3);
    invalid.images.push("missing-asset".into());
    assert!(client
        .call(
            "set_screen_collections",
            json!({"dev":dev,"expected_revision":2,"collections":invalid})
        )
        .await
        .is_err());
    assert_eq!(
        store.read(&key).unwrap().screen_collections,
        Some(collections)
    );
    assert!(store.referenced_theme_assets().unwrap().contains(&image));
    let before: llcontrol_ipc::ScreenResetReadback = serde_json::from_value(
        client
            .call("get_screen_reset", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(before.receipt.is_none());
    assert!(!before.reapply_available);
    assert!(client.call("reapply_screen_reset", json!({"dev":dev,"operation_id":"a".repeat(32),"expected_generation":before.lcd.generation})).await.is_err());
    assert!(client.call("reset_screen", json!({"dev":dev,"operation_id":"bad","expected_profile_revision":0,"expected_collection_revision":2})).await.is_err());
    assert!(client.call("reset_screen", json!({"dev":dev,"operation_id":"c".repeat(32),"expected_profile_revision":0,"expected_collection_revision":1})).await.is_err());
    let id = "d".repeat(32);
    let params = json!({"dev":dev,"operation_id":id,"expected_profile_revision":0,"expected_collection_revision":2});
    let reset: llcontrol_ipc::ScreenResetResult =
        serde_json::from_value(client.call("reset_screen", params.clone()).await.unwrap()).unwrap();
    assert!(!reset.repeated);
    assert_eq!(reset.receipt.collection_revision, 3);
    assert_eq!(reset.receipt.profile_revision, 1);
    let read: llcontrol_ipc::ScreenResetReadback = serde_json::from_value(
        client
            .call("get_screen_reset", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(read.receipt.unwrap().operation_id, id);
    assert!(!read.persistence.pending);
    assert!(read.persistence.saved);
    assert_eq!(read.lcd.generation, reset.lcd.generation);
    let repeated: llcontrol_ipc::ScreenResetResult =
        serde_json::from_value(client.call("reset_screen", params).await.unwrap()).unwrap();
    assert!(repeated.repeated);
    assert_eq!(repeated.lcd.generation, reset.lcd.generation);
    assert_eq!(
        store.read(&key).unwrap().screen_collections,
        Some(ScreenCollections::defaults(3))
    );
    assert!(store.theme_assets().list().unwrap().contains(&image));
    drop(client);
    manager.shutdown().await;
    server.abort();
    let _ = server.await;
}
