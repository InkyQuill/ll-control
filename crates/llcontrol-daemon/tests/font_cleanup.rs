use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{DeviceSettings, PersistenceStore},
    profiles::ProfileStore,
    stats::Stats,
};
use llcontrol_hid::MockBackend;
use llcontrol_ipc::{DaemonRpc, LcdTheme, NamedProfile, ProfileContent, ProfileEdit};
use std::sync::Arc;
fn theme(key: &str) -> LcdTheme {
    serde_json::from_value(serde_json::json!({"version":1,"name":"Fonts","background":[0,0,0],"layers":[{
        "kind":"text","text":"A","style":{"font_asset":key,"x":1,"y":1,"size":20,"color":[255,255,255],"align":"left"}
    }]})).unwrap()
}
#[tokio::test]
async fn cleanup_keeps_offline_device_and_profile_assets_and_refuses_corrupt_roots() {
    let directory = tempfile::tempdir().unwrap();
    let old = PersistenceStore::new_with_root(directory.path().into());
    let png = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");
    let screen = old.theme_fonts().import(png).unwrap();
    let device = old.theme_fonts().import(png).unwrap();
    let orphan = old.theme_fonts().import(png).unwrap();
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
    let scan = rpc.cleanup_theme_fonts(false, vec![]).await.unwrap();
    assert_eq!(scan.candidates.as_slice(), std::slice::from_ref(&orphan));
    assert_eq!(scan.retained_referenced, 2);
    let profile_path = directory.path().join("profiles.json");
    let saved = std::fs::read(&profile_path).unwrap();
    std::fs::write(&profile_path, b"broken").unwrap();
    assert!(rpc.cleanup_theme_fonts(true, vec![]).await.is_err());
    assert!(store
        .theme_fonts_root()
        .join(format!("{orphan}.font"))
        .exists());
    std::fs::write(&profile_path, saved).unwrap();
    let device_path = directory.path().join("devices/offline.toml");
    let device_bytes = std::fs::read(&device_path).unwrap();
    std::fs::write(&device_path, b"not valid settings").unwrap();
    assert!(rpc.cleanup_theme_fonts(true, vec![]).await.is_err());
    assert!(store
        .theme_fonts_root()
        .join(format!("{orphan}.font"))
        .exists());
    std::fs::write(&device_path, device_bytes).unwrap();
    assert_eq!(
        rpc.cleanup_theme_fonts(true, vec![]).await.unwrap().deleted,
        [orphan]
    );
    assert!(store.theme_fonts().load_theme(&theme(&screen)).is_ok());
    assert!(store.theme_fonts().load_theme(&theme(&device)).is_ok());
    manager.shutdown().await;
}

#[test]
fn font_cleanup_pins_shared_imports_and_loads_and_keeps_drafts_unknown_files() {
    use std::collections::BTreeSet;
    let temp = tempfile::tempdir().unwrap();
    let old = PersistenceStore::new_with_root(temp.path().into());
    let font = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");
    let draft = old.theme_fonts().import(font).unwrap();
    let loaded = old.theme_fonts().import(font).unwrap();
    let orphan = old.theme_fonts().import(font).unwrap();
    let store = PersistenceStore::new_with_root(temp.path().into());
    let imported = store.theme_fonts().import(font).unwrap();
    store.theme_fonts().load_theme(&theme(&loaded)).unwrap();
    let root = store.theme_fonts_root();
    std::fs::write(root.join("unknown.ttf"), b"keep").unwrap();
    std::fs::create_dir(root.join(format!("{}.font", "a".repeat(32)))).unwrap();
    std::os::unix::fs::symlink(
        root.join(format!("{orphan}.font")),
        root.join(format!("{}.font", "b".repeat(32))),
    )
    .unwrap();
    let scan = store
        .theme_fonts()
        .cleanup(&BTreeSet::new(), std::slice::from_ref(&draft), false)
        .unwrap();
    assert_eq!(scan.candidates.as_slice(), std::slice::from_ref(&orphan));
    assert_eq!(scan.retained_session, 2);
    assert_eq!(scan.retained_draft, 1);
    assert_eq!(scan.skipped, 3);
    assert!(store
        .theme_fonts()
        .cleanup(&BTreeSet::new(), &["../escape".into()], true)
        .is_err());
    assert!(root.join(format!("{orphan}.font")).exists());
    let removed = store
        .theme_fonts()
        .cleanup(&BTreeSet::new(), std::slice::from_ref(&draft), true)
        .unwrap();
    assert_eq!(removed.deleted, [orphan]);
    for key in [draft, loaded, imported] {
        assert!(root.join(format!("{key}.font")).exists());
    }
}
