use lconnect_daemon::{
    actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
    persistence::{DeviceSettings, LcdSection, PersistenceStore},
    profile_archive::{PreparedGroup, ProfileArchives},
    profile_group::{
        GroupImportCommit, GroupImportError, GroupImportReadback, GroupKind, GroupProfile,
        GroupRevision, GroupSnapshot,
    },
};
use lconnect_ipc::*;
use std::{io::Cursor, sync::Arc};
use tokio::sync::{mpsc, oneshot, watch};

fn archives(store: &PersistenceStore) -> ProfileArchives {
    ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    )
}
fn prepare(target: &PersistenceStore, kind: GroupKind, brightness: u8) -> Arc<PreparedGroup> {
    let dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(dir.path().into());
    let mut settings = DeviceSettings::default();
    settings.lcd.brightness = brightness;
    settings.fan.pwm_percent = brightness;
    let mut library = ProfileLibrary::default();
    if kind == GroupKind::Screen {
        settings.screen_collections = Some(ScreenCollections::defaults(1));
        let image = source
            .theme_assets()
            .import(lconnect_image::defaults::IMAGES[0].png)
            .unwrap();
        let ProfileContent::Screen(mut theme) =
            NamedProfile::defaults(ProfileGroup::Screen, 0, None)
                .unwrap()
                .content
        else {
            unreachable!()
        };
        theme.background_image =
            Some(serde_json::from_value(serde_json::json!({"asset":image,"fit":"crop"})).unwrap());
        settings.theme = Some(theme);
        let media = source
            .media_assets()
            .import(
                "Slot media".into(),
                H264Clip {
                    fps: 24,
                    units: vec![H264AccessUnit {
                        bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                        keyframe: true,
                    }],
                },
            )
            .unwrap();
        library.profiles.push(NamedProfile {
            slot: 7,
            name: "Media seventh index".into(),
            content: ProfileContent::ScreenMedia(media.key),
        });
    }
    let group = GroupProfile::capture(&settings, &library, kind)
        .unwrap()
        .profile;
    let mut bytes = Vec::new();
    archives(&source).export_group(&group, &mut bytes).unwrap();
    Arc::new(
        archives(target)
            .prepare_group(&mut Cursor::new(bytes))
            .unwrap(),
    )
}
async fn snapshot(tx: &mpsc::Sender<SettingsUpdate>, kind: GroupKind) -> GroupSnapshot {
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ReadProfileGroup { kind, reply })
        .await
        .unwrap();
    response.await.unwrap().unwrap()
}
async fn commit(
    tx: &mpsc::Sender<SettingsUpdate>,
    id: &str,
    expected: GroupRevision,
    prepared: Arc<PreparedGroup>,
) -> std::result::Result<GroupImportCommit, GroupImportError> {
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::CommitProfileGroup {
        operation_id: id.into(),
        expected,
        prepared,
        reply,
    })
    .await
    .unwrap();
    response.await.unwrap()
}
async fn readback(tx: &mpsc::Sender<SettingsUpdate>, id: &str) -> Option<GroupImportReadback> {
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ReadGroupImport {
        operation_id: id.into(),
        reply,
    })
    .await
    .unwrap();
    response.await.unwrap().unwrap()
}

#[tokio::test]
async fn lost_reply_readback_and_repeated_commit_preserve_later_queued_changes_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let prepared = prepare(&store, GroupKind::Screen, 72);
    let images = store.theme_assets().list().unwrap();
    let media = store.media_assets().list().unwrap().len();
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, status) = spawn_updates_observed(
        store.clone(),
        "target".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    let expected = snapshot(&tx, GroupKind::Screen).await.revision;
    let id = "a".repeat(32);
    let (reply, lost) = oneshot::channel();
    drop(lost);
    tx.send(SettingsUpdate::CommitProfileGroup {
        operation_id: id.clone(),
        expected: expected.clone(),
        prepared: prepared.clone(),
        reply,
    })
    .await
    .unwrap();
    let confirmed = readback(&tx, &id).await.unwrap();
    assert!(confirmed.selected_content_current);
    assert_eq!(confirmed.receipt.profile_revision, 1);
    assert_eq!(confirmed.settings.lcd.brightness, 72);
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 91,
        rotation: "90".into(),
        control_mode: "Application".into(),
    }))
    .await
    .unwrap();
    let repeated = commit(&tx, &id, expected, prepared).await.unwrap();
    assert!(repeated.repeated);
    assert!(!repeated.readback.selected_content_current);
    assert_eq!(repeated.readback.settings.lcd.brightness, 91);
    assert!(
        status.borrow().pending,
        "repeat must not flush or discard newer dirty intent"
    );
    assert_eq!(store.theme_assets().list().unwrap(), images);
    assert_eq!(store.media_assets().list().unwrap().len(), media);
    assert_eq!(store.profiles().list().unwrap().revision, 1);
    drop(tx);
    task.await.unwrap();
    let restarted = PersistenceStore::new_with_root(dir.path().into());
    let latest = restarted.read("target").unwrap();
    let current = restarted
        .read_group_import("target", &latest, &id)
        .unwrap()
        .unwrap();
    assert_eq!(current.settings.lcd.brightness, 91);
    assert!(!current.selected_content_current);
    assert_eq!(current.receipt, confirmed.receipt);
    restarted
        .profiles()
        .edit(
            1,
            ProfileEdit::Delete {
                group: ProfileGroup::Screen,
                slot: 7,
            },
        )
        .unwrap();
    let current = restarted
        .read_group_import("target", &latest, &id)
        .unwrap()
        .unwrap();
    assert_eq!(current.profiles.revision, 2);
    assert!(current.profiles.profiles.is_empty());
    assert!(
        !current.selected_content_current,
        "readback must never resurrect a removed slot"
    );
}

#[tokio::test]
async fn new_import_saves_queued_neighbors_but_stale_rejection_keeps_pending_changes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let prepared = prepare(&store, GroupKind::Screen, 67);
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, status) = spawn_updates_observed(
        store.clone(),
        "target".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    let expected = snapshot(&tx, GroupKind::Screen).await.revision;
    tx.send(SettingsUpdate::Fan(
        lconnect_daemon::persistence::FanSection {
            pwm_percent: 64,
            ..Default::default()
        },
    ))
    .await
    .unwrap();
    let result = commit(&tx, &"b".repeat(32), expected, prepared.clone())
        .await
        .unwrap();
    assert!(!result.repeated);
    assert_eq!(result.readback.settings.fan.pwm_percent, 64);
    assert!(!status.borrow().pending);
    let expected = snapshot(&tx, GroupKind::Screen).await.revision;
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 29,
        rotation: "0".into(),
        control_mode: "LocalUI".into(),
    }))
    .await
    .unwrap();
    assert!(matches!(
        commit(&tx, &"c".repeat(32), expected, prepared).await,
        Err(GroupImportError::Rejected(_))
    ));
    assert!(status.borrow().pending);
    assert!(readback(&tx, &"c".repeat(32)).await.is_none());
    drop(tx);
    task.await.unwrap();
    assert_eq!(store.read("target").unwrap().lcd.brightness, 29);
    assert_eq!(store.profiles().list().unwrap().revision, 1);
}

#[test]
fn same_id_rejects_changed_source_target_or_group_and_receipts_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let prepared = prepare(&store, GroupKind::FanAndLighting, 62);
    let mut latest = DeviceSettings::default();
    let expected = store
        .snapshot_profile_group("target", &latest, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let id = "a".repeat(32);
    let committed = store
        .commit_group_import("target", &latest, &expected, &id, &prepared)
        .unwrap();
    latest = committed.readback.settings;
    assert!(store
        .commit_group_import(
            "target",
            &latest,
            &expected,
            &id,
            &prepare(&store, GroupKind::FanAndLighting, 71)
        )
        .is_err());
    let fresh = store
        .snapshot_profile_group("target", &latest, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    assert!(store
        .commit_group_import("target", &latest, &fresh, &id, &prepared)
        .is_err());
    assert!(store
        .commit_group_import(
            "target",
            &latest,
            &expected,
            &id,
            &prepare(&store, GroupKind::Screen, 62)
        )
        .is_err());
    for i in 1..=64 {
        let expected = store
            .snapshot_profile_group("target", &latest, GroupKind::FanAndLighting)
            .unwrap()
            .revision;
        latest = store
            .commit_group_import(
                "target",
                &latest,
                &expected,
                &format!("{i:032x}"),
                &prepared,
            )
            .unwrap()
            .readback
            .settings;
    }
    assert_eq!(latest.group_imports.len(), 64);
    assert!(!latest.group_imports.iter().any(|r| r.id == id));
    assert!(store
        .read_group_import("target", &latest, &id)
        .unwrap()
        .is_none());
    assert!(
        store
            .commit_group_import("target", &latest, &expected, &id, &prepared)
            .is_err(),
        "evicted old intent still fails original target CAS"
    );
    assert_eq!(store.profiles().list().unwrap().revision, 65);
}

#[tokio::test]
async fn fan_group_keeps_screen_intent_and_pending_media_pin() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let prepared = prepare(&store, GroupKind::FanAndLighting, 56);
    let screen = prepare(&store, GroupKind::Screen, 70);
    let key = screen.uploaded_media().first().unwrap().clone();
    let pin = store.media_assets().acquire_source(&key).unwrap().pin();
    drop(screen);
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, _status) = spawn_updates_observed(
        store.clone(),
        "target".into(),
        DeviceSettings::default(),
        rx,
        stop,
    );
    let expected = snapshot(&tx, GroupKind::FanAndLighting).await.revision;
    tx.send(SettingsUpdate::MediaClip {
        key: key.clone(),
        pin: Some(pin),
    })
    .await
    .unwrap();
    let result = commit(&tx, &"d".repeat(32), expected, prepared)
        .await
        .unwrap();
    assert_eq!(
        result.readback.settings.media_clip.as_deref(),
        Some(key.as_str())
    );
    assert_eq!(result.readback.settings.fan.pwm_percent, 56);
    assert!(
        store
            .media_assets()
            .delete_unreferenced(&key, || Ok(false))
            .is_err(),
        "fan commit must retain the screen owner lease"
    );
    drop(tx);
    task.await.unwrap();
    assert_eq!(
        store.read("target").unwrap().media_clip.as_deref(),
        Some(key.as_str())
    );
}

#[tokio::test]
async fn unresolved_journal_stops_group_writer_without_flushing_old_pending_intent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let prepared = prepare(&store, GroupKind::FanAndLighting, 61);
    let initial = DeviceSettings::default();
    store.write("target", &initial).unwrap();
    let expected = store
        .snapshot_profile_group("target", &initial, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let path = dir.path().join("devices/target.toml");
    let before = std::fs::read(&path).unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, status) = spawn_updates_observed(store.clone(), "target".into(), initial, rx, stop);
    std::fs::write(
        dir.path().join("screen-reset-transaction.json"),
        "broken group journal",
    )
    .unwrap();
    tx.send(SettingsUpdate::Lcd(LcdSection {
        brightness: 90,
        rotation: "0".into(),
        control_mode: "LocalUI".into(),
    }))
    .await
    .unwrap();
    assert!(matches!(
        commit(&tx, &"f".repeat(32), expected, prepared).await,
        Err(GroupImportError::RecoveryRequired(_))
    ));
    task.await.unwrap();
    assert!(tx
        .send(SettingsUpdate::Fan(
            lconnect_daemon::persistence::FanSection {
                pwm_percent: 90,
                ..Default::default()
            }
        ))
        .await
        .is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("screen-reset-transaction.json")).unwrap(),
        "broken group journal"
    );
    assert!(status.borrow().pending);
    assert!(status.borrow().last_error.is_some());
}

#[test]
fn receipt_referring_to_missing_profile_revision_is_preserved_and_blocks_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let initial = DeviceSettings::default();
    let prepared = prepare(&store, GroupKind::FanAndLighting, 59);
    let expected = store
        .snapshot_profile_group("target", &initial, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let committed = store
        .commit_group_import("target", &initial, &expected, &"a".repeat(32), &prepared)
        .unwrap();
    let valid = committed.readback.settings;
    let mut corrupt = valid.clone();
    corrupt.group_imports[0].expected.profile_revision += 1;
    corrupt.group_imports[0].profile_revision += 1;
    let text = toml::to_string_pretty(&corrupt).unwrap();
    let path = dir.path().join("devices/target.toml");
    std::fs::write(&path, &text).unwrap();
    assert!(store
        .read("target")
        .unwrap_err()
        .to_string()
        .contains("missing profile revision"));
    assert!(
        store.write("target", &valid).is_err(),
        "a good caller snapshot cannot silently erase corrupt persisted receipt evidence"
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    assert_eq!(store.profiles().list().unwrap().revision, 1);
}
