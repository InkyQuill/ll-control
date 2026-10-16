use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{DeviceSettings, PersistenceStore},
    stats::Stats,
    theme_assets::ThemeAssetStore,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair, LcdTheme};
use serde_json::json;
use std::sync::Arc;
const PNG: &[u8] = include_bytes!("../../../desktop/src-tauri/icons/icon.png");
fn theme(key: &str) -> LcdTheme {
    serde_json::from_value(json!({"version":1,"name":"Image","background":[0,0,0],"layers":[{"kind":"image","asset":key,"x":0,"y":0,"width":480,"height":480,"fit":"crop","opacity":255}]})).unwrap()
}
#[test]
fn explicit_delete_checks_live_leases_draft_revision_and_exact_key() {
    let root = tempfile::tempdir().unwrap();
    let store = ThemeAssetStore::new(root.path().into());
    let key = store.import_named(PNG, Some("Delete me")).unwrap();
    let other = store.import(PNG).unwrap();
    let loaded = store.load(&theme(&key)).unwrap();
    let cloned = loaded.clone();
    drop(loaded);
    assert!(store
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .is_err());
    drop(cloned);
    assert!(store
        .delete_unreferenced(&key, 1, std::slice::from_ref(&key), || Ok(false))
        .is_err());
    assert!(store
        .delete_unreferenced(&key, 1, &[], || Ok(true))
        .is_err());
    store.rename(&key, "Renamed", 1).unwrap();
    assert!(store
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .is_err());
    assert!(store
        .delete_unreferenced("../outside", 0, &[], || Ok(false))
        .is_err());
    store
        .delete_unreferenced(&key, 2, &[], || Ok(false))
        .unwrap();
    assert!(!root.path().join(format!("{key}.name.json")).exists());
    assert!(store.list().unwrap().contains(&other));
    assert!(!store.list().unwrap().contains(&key));
    assert!(store.load(&theme(&key)).is_err());
}
#[tokio::test]
async fn ipc_delete_protects_offline_settings_profiles_and_corrupt_references() {
    for background in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let assets = store.theme_assets();
        let key = assets.import_named(PNG, Some("Image")).unwrap();
        let config = Config::default();
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(MockBackend::new()),
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
        let (a, b) = mem_pair();
        let server = tokio::spawn(async move { IpcServer::new(rpc).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        let editor_bytes = client
            .call("read_theme_asset_for_edit", json!({"key":key}))
            .await
            .unwrap();
        let decoded: Vec<u8> = serde_json::from_value(editor_bytes).unwrap();
        assert_eq!(decoded, assets.read_for_edit(&key).unwrap());
        assert!(client
            .call(
                "read_theme_asset_for_edit",
                json!({"key":lconnect_ipc::BUILTIN_IMAGE_KEY})
            )
            .await
            .is_err());
        assert!(client
            .call(
                "read_theme_asset_for_edit",
                json!({"key":key,"path":"/tmp/other"})
            )
            .await
            .is_err());

        let mut saved_theme = theme(&key);
        if background {
            saved_theme.background_image = Some(lconnect_ipc::BackgroundImage {
                asset: key.clone(),
                fit: lconnect_ipc::ImageFit::Crop,
            });
            saved_theme.layers.clear();
        }
        let delete = json!({"key":key,"expected_revision":1,"keep":[]});
        store
            .write(
                "offline",
                &DeviceSettings {
                    theme: Some(saved_theme.clone()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(client
            .call("delete_theme_asset", delete.clone())
            .await
            .is_err());
        store.write("offline", &DeviceSettings::default()).unwrap();
        client.call("edit_profile",json!({"expected_revision":0,"edit":{"action":"save","profile":{"slot":0,"name":"Saved","content":{"kind":"screen","value":saved_theme}}}})).await.unwrap();
        assert!(client
            .call("delete_theme_asset", delete.clone())
            .await
            .is_err());
        client
            .call(
                "edit_profile",
                json!({"expected_revision":1,"edit":{"action":"delete","group":"screen","slot":0}}),
            )
            .await
            .unwrap();
        let profiles = root.path().join("profiles.json");
        let saved = std::fs::read(&profiles).unwrap();
        std::fs::write(&profiles, b"broken").unwrap();
        assert!(client
            .call("delete_theme_asset", delete.clone())
            .await
            .is_err());
        assert!(assets.list().unwrap().contains(&key));
        std::fs::write(&profiles, saved).unwrap();
        client.call("delete_theme_asset", delete).await.unwrap();
        assert!(!assets.list().unwrap().contains(&key));
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
    }
}

#[test]
fn explicit_delete_rejects_symlink_root_and_asset() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    let store = ThemeAssetStore::new(real.clone());
    let key = store.import(PNG).unwrap();
    let alias = root.path().join("alias");
    symlink(&real, &alias).unwrap();
    assert!(ThemeAssetStore::new(alias)
        .delete_unreferenced(&key, 0, &[], || Ok(false))
        .is_err());
    let path = real.join(format!("{key}.png"));
    let original = root.path().join("original.png");
    std::fs::rename(&path, &original).unwrap();
    symlink(&original, &path).unwrap();
    assert!(store
        .delete_unreferenced(&key, 0, &[], || Ok(false))
        .is_err());
    assert!(original.is_file());
}
