use llcontrol_daemon::persistence::{DeviceSettings, PersistenceStore};
use llcontrol_ipc::{ProfileEdit, ProfileGroup};

#[test]
fn screen_reset_commits_config_theme_and_slots_preserving_cooling_files_and_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut latest = DeviceSettings::default();
    latest.pump.pwm_percent = 81;
    latest.lcd.brightness = 12;
    latest.lcd.rotation = "270".into();
    latest.media_clip = Some("b".repeat(32));
    store.write("device", &latest).unwrap();
    let library = store
        .profiles()
        .edit(
            0,
            ProfileEdit::ResetGroup {
                group: ProfileGroup::Pump,
                sensor_id: Some("cpu:temp".into()),
            },
        )
        .unwrap();
    let user_file = dir.path().join("user.png");
    std::fs::write(&user_file, b"user artwork").unwrap();
    let id = "a".repeat(32);
    let (mut saved, reset) = store
        .commit_screen_reset("device", &latest, library.revision, 0, &id)
        .unwrap();
    assert_eq!(saved.pump.pwm_percent, 81);
    assert_eq!(saved.lcd.brightness, 50);
    assert_eq!(saved.lcd.rotation, "0");
    assert_eq!(saved.lcd.control_mode, "Application");
    assert!(saved.theme.is_some());
    assert!(saved.media_clip.is_none());
    assert_eq!(reset.profiles.len(), 12);
    assert_eq!(reset.profiles[..4], library.profiles);
    assert_eq!(reset.revision, library.revision + 1);
    assert_eq!(std::fs::read(user_file).unwrap(), b"user artwork");
    // A delayed retry must not undo ordinary subsequent settings edits.
    saved.lcd.brightness = 75;
    store.write("device", &saved).unwrap();
    let reopened = PersistenceStore::new_with_root(dir.path().into());
    let current = reopened.read("device").unwrap();
    let (retry, profiles) = reopened
        .commit_screen_reset("device", &current, library.revision, 0, &id)
        .unwrap();
    assert_eq!(retry.lcd.brightness, 75);
    assert_eq!(profiles, reset);
    assert!(reopened
        .commit_screen_reset("device", &retry, library.revision, 1, &"c".repeat(32))
        .is_err());
    assert_eq!(reopened.read("device").unwrap().lcd.brightness, 75);
}

#[tokio::test]
async fn settings_owner_commits_queued_neighbors_and_lost_reply_retry_keeps_later_deltas() {
    use llcontrol_daemon::{
        actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
        persistence::{FanSection, LcdSection},
    };
    use tokio::sync::{mpsc, oneshot, watch};
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, status) = spawn_updates_observed(
        store.clone(),
        "device".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 63,
        ..Default::default()
    }))
    .await
    .unwrap();
    let id = "d".repeat(32);
    let (reply, lost) = oneshot::channel();
    drop(lost);
    tx.send(SettingsUpdate::ResetScreen {
        expected_profile_revision: 0,
        expected_collection_revision: 0,
        operation_id: id.clone(),
        reply,
    })
    .await
    .unwrap();
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 75,
        rotation: "90".into(),
        control_mode: "Application".into(),
    }))
    .await
    .unwrap();
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ResetScreen {
        expected_profile_revision: 0,
        expected_collection_revision: 0,
        operation_id: id,
        reply,
    })
    .await
    .unwrap();
    let committed = response.await.unwrap().unwrap();
    assert!(committed.repeated);
    let current = committed.settings;
    let profiles = committed.profiles;
    assert_eq!(profiles.revision, 1, "retry must not repeat the reset");
    assert_eq!(current.fan.pwm_percent, 63);
    assert_eq!(current.lcd.brightness, 75);
    assert!(
        status.borrow().pending,
        "retry must retain the unflushed newer delta"
    );
    drop(tx);
    task.await.unwrap();
    let saved = store.read("device").unwrap();
    assert_eq!(saved.fan.pwm_percent, 63);
    assert_eq!(saved.lcd.brightness, 75);
    assert_eq!(saved.lcd.rotation, "90");
    assert_eq!(store.profiles().list().unwrap().profiles.len(), 8);
}

#[tokio::test]
async fn rejected_reset_does_not_lose_queued_settings() {
    use llcontrol_daemon::{
        actor::command_dispatch::{spawn_updates, SettingsUpdate},
        persistence::FanSection,
    };
    use tokio::sync::{mpsc, oneshot, watch};
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let task = spawn_updates(
        store.clone(),
        "device".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 47,
        ..Default::default()
    }))
    .await
    .unwrap();
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ResetScreen {
        expected_profile_revision: 99,
        expected_collection_revision: 0,
        operation_id: "e".repeat(32),
        reply,
    })
    .await
    .unwrap();
    assert!(response
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("library changed"));
    drop(tx);
    task.await.unwrap();
    assert_eq!(store.read("device").unwrap().fan.pwm_percent, 47);
    assert!(store.read("device").unwrap().screen_reset_receipt.is_none());
    assert_eq!(store.profiles().list().unwrap().revision, 0);
}

#[tokio::test]
async fn unresolved_journal_stops_writer_without_flushing_an_old_snapshot() {
    use llcontrol_daemon::{
        actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
        persistence::FanSection,
    };
    use tokio::sync::{mpsc, oneshot, watch};
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let initial = DeviceSettings::default();
    store.write("device", &initial).unwrap();
    let path = dir.path().join("devices/device.toml");
    let before = std::fs::read(&path).unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_shutdown, stop) = watch::channel(false);
    let (task, state) = spawn_updates_observed(store, "device".into(), initial, rx, stop);
    std::fs::write(
        dir.path().join("screen-reset-transaction.json"),
        "broken journal",
    )
    .unwrap();
    tx.send(SettingsUpdate::Fan(FanSection {
        pwm_percent: 47,
        ..Default::default()
    }))
    .await
    .unwrap();
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ResetScreen {
        expected_profile_revision: 0,
        expected_collection_revision: 0,
        operation_id: "e".repeat(32),
        reply,
    })
    .await
    .unwrap();
    assert!(response
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("recovery required"));
    task.await.unwrap();
    assert!(tx
        .send(SettingsUpdate::Fan(FanSection {
            pwm_percent: 99,
            ..Default::default()
        }))
        .await
        .is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("screen-reset-transaction.json")).unwrap(),
        "broken journal"
    );
    assert!(state.borrow().pending);
    assert!(state
        .borrow()
        .last_error
        .as_deref()
        .unwrap()
        .contains("recovery required"));
}

#[test]
fn screen_reset_and_profile_editor_share_one_revision_gate() {
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let reset_store = store.clone();
    let reset_barrier = barrier.clone();
    let reset = std::thread::spawn(move || {
        reset_barrier.wait();
        reset_store
            .commit_screen_reset("device", &DeviceSettings::default(), 0, 0, &"a".repeat(32))
            .is_ok()
    });
    let profiles = store.profiles();
    let edit = std::thread::spawn(move || {
        barrier.wait();
        profiles
            .edit(
                0,
                ProfileEdit::Reset {
                    group: ProfileGroup::Screen,
                    slot: 7,
                    sensor_id: None,
                },
            )
            .is_ok()
    });
    let reset_won = reset.join().unwrap();
    assert_ne!(
        reset_won,
        edit.join().unwrap(),
        "exactly one contender commits"
    );
    let profiles = store.profiles().list().unwrap();
    assert_eq!(profiles.revision, 1);
    assert_eq!(profiles.profiles.len(), if reset_won { 8 } else { 1 });
    assert_eq!(
        store.read("device").unwrap().screen_reset_receipt.is_some(),
        reset_won
    );
}

#[test]
fn reset_collections_checks_revision_preserves_files_and_releases_offline_references() {
    use llcontrol_ipc::{ScreenBackgroundAsset, ScreenCollections};
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut collections = ScreenCollections::defaults(7);
    collections.images.push("user-image".into());
    collections.watermarks.push("user-watermark".into());
    let video = "f".repeat(32);
    collections
        .backgrounds
        .push(ScreenBackgroundAsset::Video(video.clone()));
    collections
        .backgrounds
        .push(ScreenBackgroundAsset::Image("user-background".into()));
    let latest = DeviceSettings {
        screen_collections: Some(collections),
        ..Default::default()
    };
    store.write("offline", &latest).unwrap();
    let artwork = dir.path().join("user-artwork.png");
    std::fs::write(&artwork, b"preserve user bytes").unwrap();
    let references = store.referenced_theme_assets().unwrap();
    for key in ["user-image", "user-watermark", "user-background"] {
        assert!(references.contains(key));
    }
    assert!(store.referenced_media_clips().unwrap().contains(&video));
    let before = std::fs::read(dir.path().join("devices/offline.toml")).unwrap();
    assert!(store
        .commit_screen_reset("offline", &latest, 0, 6, &"a".repeat(32))
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("devices/offline.toml")).unwrap(),
        before
    );
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    let (saved, _) = store
        .commit_screen_reset("offline", &latest, 0, 7, &"b".repeat(32))
        .unwrap();
    assert_eq!(
        saved.screen_collections,
        Some(ScreenCollections::defaults(8))
    );
    assert_eq!(saved.screen_reset_receipt.unwrap().collection_revision, 8);
    let reopened = PersistenceStore::new_with_root(dir.path().into());
    assert_eq!(
        reopened.read("offline").unwrap().screen_collections,
        Some(ScreenCollections::defaults(8))
    );
    assert!(!reopened
        .referenced_theme_assets()
        .unwrap()
        .contains("user-image"));
    assert!(!reopened.referenced_media_clips().unwrap().contains(&video));
    assert_eq!(std::fs::read(artwork).unwrap(), b"preserve user bytes");
}

#[test]
fn legacy_collections_are_absent_and_invalid_or_exhausted_collections_fail_closed() {
    use llcontrol_ipc::ScreenCollections;
    let legacy: DeviceSettings = toml::from_str("schema_version = 1\n").unwrap();
    assert!(legacy.screen_collections.is_none());
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let latest = DeviceSettings {
        screen_collections: Some(ScreenCollections::defaults(u64::MAX)),
        ..Default::default()
    };
    // Overflow must fail before a transaction is published, even if this
    // in-memory snapshot has not yet been serialized by the settings owner.
    assert!(store
        .commit_screen_reset("device", &latest, 0, u64::MAX, &"a".repeat(32))
        .is_err());
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    let mut malformed = DeviceSettings {
        screen_collections: Some(ScreenCollections::defaults(1)),
        ..Default::default()
    };
    malformed
        .screen_collections
        .as_mut()
        .unwrap()
        .watermarks
        .clear();
    assert!(store.write("device", &malformed).is_err());
    std::fs::create_dir_all(dir.path().join("devices")).unwrap();
    std::fs::write(
        dir.path().join("devices/device.toml"),
        toml::to_string(&malformed).unwrap(),
    )
    .unwrap();
    assert!(store.read("device").is_err());
    assert!(store.referenced_theme_assets().is_err());
    assert!(store.referenced_media_clips().is_err());
}
