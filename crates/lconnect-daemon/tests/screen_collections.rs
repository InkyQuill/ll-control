use lconnect_daemon::{
    actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
    persistence::{DeviceSettings, FanSection, PersistenceStore},
    screen_collections::PreparedCollections,
};
use lconnect_ipc::ScreenCollections;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};
const PNG: &[u8] = include_bytes!("../../../desktop/src-tauri/icons/icon.png");

#[test]
fn lightweight_image_pin_blocks_delete_until_last_clone_drops() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let images = store.theme_assets();
    let key = images.import_named(PNG, Some("Collection image")).unwrap();
    let pin = images.acquire_reference(&key).unwrap();
    let copy = pin.clone();
    drop(pin);
    assert!(images
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .is_err());
    drop(copy);
    images
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .unwrap();
    assert!(images.acquire_reference(&key).is_err());
}

#[tokio::test]
async fn owner_serializes_collections_with_neighbors_and_readback_after_lost_reply() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let images = store.theme_assets();
    let key = images.import_named(PNG, Some("Collection image")).unwrap();
    let mut value = ScreenCollections::defaults(1);
    value.images.push(key.clone());
    let prepared =
        PreparedCollections::prepare(value.clone(), &images, &store.media_assets()).unwrap();
    assert!(images
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .is_err());
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, _) = spawn_updates_observed(
        store.clone(),
        "device".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 61,
        ..Default::default()
    }))
    .await
    .unwrap();
    let (reply, lost) = oneshot::channel();
    drop(lost);
    tx.send(SettingsUpdate::SetCollections {
        expected_revision: 0,
        prepared,
        reply,
    })
    .await
    .unwrap();
    let (reply, read) = oneshot::channel();
    tx.send(SettingsUpdate::ReadCollections { reply })
        .await
        .unwrap();
    assert_eq!(read.await.unwrap().collections, Some(value.clone()));
    let saved = store.read("device").unwrap();
    assert_eq!(saved.fan.pwm_percent, 61);
    assert_eq!(saved.screen_collections, Some(value.clone()));
    let stale = PreparedCollections::prepare(value, &images, &store.media_assets()).unwrap();
    let (reply, rejected) = oneshot::channel();
    tx.send(SettingsUpdate::SetCollections {
        expected_revision: 0,
        prepared: stale,
        reply,
    })
    .await
    .unwrap();
    assert!(rejected.await.unwrap().is_err());
    let next = ScreenCollections::defaults(2);
    let prepared =
        PreparedCollections::prepare(next.clone(), &images, &store.media_assets()).unwrap();
    let (reply, saved) = oneshot::channel();
    tx.send(SettingsUpdate::SetCollections {
        expected_revision: 1,
        prepared,
        reply,
    })
    .await
    .unwrap();
    assert_eq!(saved.await.unwrap().unwrap(), next);
    images
        .delete_unreferenced(&key, 1, &[], || {
            Ok(store.referenced_theme_assets()?.contains(&key))
        })
        .unwrap();
    drop(tx);
    task.await.unwrap();
}

#[test]
fn preparation_rejects_missing_and_corrupt_assets_without_leaking_pins() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let images = store.theme_assets();
    let key = images.import_named(PNG, Some("Collection image")).unwrap();
    let mut value = ScreenCollections::defaults(1);
    value.images.extend([key.clone(), "missing".into()]);
    assert!(PreparedCollections::prepare(value.clone(), &images, &store.media_assets()).is_err());
    images
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .unwrap();
    let corrupt = images.import_named(PNG, Some("Collection image")).unwrap();
    std::fs::write(
        store.theme_assets_root().join(format!("{corrupt}.png")),
        b"not a PNG",
    )
    .unwrap();
    value.images = vec![
        lconnect_ipc::BUILTIN_MARK_KEY.into(),
        lconnect_ipc::BUILTIN_IMAGE_KEY.into(),
        corrupt,
    ];
    assert!(PreparedCollections::prepare(value, &images, &store.media_assets()).is_err());
}

#[tokio::test]
async fn failed_save_retains_collection_intent_and_pins_until_retry_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let images = store.theme_assets();
    let key = images.import_named(PNG, Some("Collection image")).unwrap();
    let mut value = ScreenCollections::defaults(1);
    value.watermarks.push(key.clone());
    let prepared =
        PreparedCollections::prepare(value.clone(), &images, &store.media_assets()).unwrap();
    std::fs::write(dir.path().join("devices"), b"unwritable directory fixture").unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, mut status) = spawn_updates_observed(
        store.clone(),
        "device".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    let (reply, failed) = oneshot::channel();
    tx.send(SettingsUpdate::SetCollections {
        expected_revision: 0,
        prepared,
        reply,
    })
    .await
    .unwrap();
    assert!(failed.await.unwrap().is_err());
    assert!(status.borrow().pending);
    let (reply, read) = oneshot::channel();
    tx.send(SettingsUpdate::ReadCollections { reply })
        .await
        .unwrap();
    let state = read.await.unwrap();
    assert_eq!(state.collections, Some(value.clone()));
    assert!(state.persistence.pending);
    assert!(state.persistence.last_error.is_some());
    assert!(images
        .delete_unreferenced(&key, 1, &[], || Ok(false))
        .is_err());
    std::fs::remove_file(dir.path().join("devices")).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if !status.borrow_and_update().pending {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        store.read("device").unwrap().screen_collections,
        Some(value)
    );
    drop(tx);
    task.await.unwrap();
    // After owner teardown, durable references take over from the temporary pin.
    assert!(images
        .delete_unreferenced(&key, 1, &[], || Ok(store
            .referenced_theme_assets()?
            .contains(&key)))
        .is_err());
}

#[tokio::test]
async fn legacy_migration_rejects_changed_inventory_then_preserves_all_contexts() {
    use lconnect_daemon::screen_collections::PreparedMigration;
    use lconnect_ipc::ScreenBackgroundAsset;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let images = store.theme_assets();
    let first = images.import(PNG).unwrap();
    let migration = PreparedMigration::prepare(&images, &store.media_assets()).unwrap();
    let second = images.import(PNG).unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, _) = spawn_updates_observed(
        store.clone(),
        "legacy".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    let (reply, rejected) = oneshot::channel();
    tx.send(SettingsUpdate::MigrateCollections { migration, reply })
        .await
        .unwrap();
    assert!(rejected
        .await
        .unwrap()
        .unwrap_err()
        .contains("catalog changed"));
    assert!(store.read("legacy").unwrap().screen_collections.is_none());
    let migration = PreparedMigration::prepare(&images, &store.media_assets()).unwrap();
    let (reply, saved) = oneshot::channel();
    tx.send(SettingsUpdate::MigrateCollections { migration, reply })
        .await
        .unwrap();
    let saved = saved.await.unwrap().unwrap();
    for key in [first, second] {
        assert!(saved.images.contains(&key));
        assert!(saved.watermarks.contains(&key));
        assert!(saved
            .backgrounds
            .contains(&ScreenBackgroundAsset::Image(key)));
    }
    assert!(!saved
        .watermarks
        .contains(&lconnect_ipc::BUILTIN_IMAGE_KEY.into()));
    assert!(saved.backgrounds.contains(&ScreenBackgroundAsset::Image(
        lconnect_ipc::BUILTIN_IMAGE_KEY.into()
    )));
    assert_eq!(
        store.read("legacy").unwrap().screen_collections,
        Some(saved)
    );
    drop(tx);
    task.await.unwrap();
}

#[test]
fn oversized_legacy_catalog_is_rejected_without_truncation_or_writes() {
    use lconnect_daemon::screen_collections::PreparedMigration;
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let root = store.theme_assets_root();
    std::fs::create_dir_all(&root).unwrap();
    for index in 0..255 {
        std::fs::write(root.join(format!("{index:032x}.png")), PNG).unwrap();
    }
    let error =
        PreparedMigration::prepare(&store.theme_assets(), &store.media_assets()).unwrap_err();
    assert!(error.to_string().contains("too many"));
    assert_eq!(store.theme_assets().list().unwrap().len(), 257);
    assert!(!dir.path().join("devices").exists());
}

#[tokio::test]
async fn migration_write_failure_retains_intent_for_shutdown_flush() {
    use lconnect_daemon::screen_collections::PreparedMigration;
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let images = store.theme_assets();
    let image = images.import_named(PNG, Some("Migrated")).unwrap();
    let migration = PreparedMigration::prepare(&images, &store.media_assets()).unwrap();
    std::fs::write(dir.path().join("devices"), b"blocked").unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, _) = spawn_updates_observed(
        store.clone(),
        "legacy".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 72,
        ..Default::default()
    }))
    .await
    .unwrap();
    let (reply, result) = oneshot::channel();
    tx.send(SettingsUpdate::MigrateCollections { migration, reply })
        .await
        .unwrap();
    assert!(result.await.unwrap().is_err());
    let (reply, result) = oneshot::channel();
    tx.send(SettingsUpdate::ReadCollections { reply })
        .await
        .unwrap();
    let state = result.await.unwrap();
    assert!(state.collections.unwrap().images.contains(&image));
    assert!(state.persistence.pending);
    assert!(state.persistence.last_error.is_some());
    assert!(images
        .delete_unreferenced(&image, 1, &[], || Ok(false))
        .is_err());
    std::fs::remove_file(dir.path().join("devices")).unwrap();
    drop(tx);
    task.await.unwrap();
    let saved = store.read("legacy").unwrap();
    assert_eq!(saved.fan.pwm_percent, 72);
    assert!(saved.screen_collections.unwrap().images.contains(&image));
}
