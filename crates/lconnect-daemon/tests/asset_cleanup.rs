use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{DeviceSettings, PersistenceStore},
    profiles::ProfileStore,
    stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{
    DaemonRpc, ImageFit, LcdTheme, NamedProfile, ProfileContent, ProfileEdit, ThemeLayer,
};
use std::sync::Arc;
fn theme(key: &str) -> LcdTheme {
    LcdTheme {
        page_cache: Vec::new(),
        version: 1,
        name: "Stored".into(),
        background: [0; 3],
        background_image: None,
        background_video: None,
        background_fill: None,
        layers: vec![ThemeLayer::Image {
            watermark_position: None,
            shadow: None,
            asset: key.into(),
            x: 0,
            y: 0,
            width: 480,
            height: 480,
            fit: ImageFit::Contain,
            opacity: 255,
        }],
    }
}
#[tokio::test]
async fn cleanup_keeps_offline_device_and_profile_assets_and_refuses_corrupt_roots() {
    let directory = tempfile::tempdir().unwrap();
    let old = PersistenceStore::new_with_root(directory.path().into());
    let png = include_bytes!("../../../desktop/src-tauri/icons/icon.png");
    let screen = old.theme_assets().import(png).unwrap();
    let device = old.theme_assets().import(png).unwrap();
    let orphan = old.theme_assets().import(png).unwrap();
    old.write(
        "offline",
        &DeviceSettings {
            theme: Some(theme(&device)),
            ..Default::default()
        },
    )
    .unwrap();
    ProfileStore::new(old.profiles_root())
        .edit(
            0,
            ProfileEdit::Save {
                profile: NamedProfile {
                    slot: 0,
                    name: "Saved".into(),
                    content: ProfileContent::Screen(theme(&screen)),
                },
            },
        )
        .unwrap();
    // Restart lifetime registry; there are deliberately no connected devices.
    drop(old);
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone());
    let scan = rpc.cleanup_theme_assets(false, vec![]).await.unwrap();
    assert_eq!(scan.candidates.as_slice(), std::slice::from_ref(&orphan));
    assert_eq!(scan.retained_referenced, 2);
    let profile_path = directory.path().join("profiles.json");
    let saved = std::fs::read(&profile_path).unwrap();
    std::fs::write(&profile_path, b"broken").unwrap();
    assert!(rpc.cleanup_theme_assets(true, vec![]).await.is_err());
    assert!(store
        .theme_assets_root()
        .join(format!("{orphan}.png"))
        .exists());
    std::fs::write(&profile_path, saved).unwrap();
    assert_eq!(
        rpc.cleanup_theme_assets(true, vec![])
            .await
            .unwrap()
            .deleted,
        [orphan]
    );
    assert!(store.theme_assets().load(&theme(&screen)).is_ok());
    assert!(store.theme_assets().load(&theme(&device)).is_ok());
    manager.shutdown().await;
}
