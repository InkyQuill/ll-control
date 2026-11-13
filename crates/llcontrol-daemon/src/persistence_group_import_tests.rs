use super::*;
use crate::{
    actor::command_dispatch::{spawn_updates_observed, SettingsUpdate},
    profile_archive::ProfileArchives,
    profile_group::{GroupKind, GroupProfile},
};
use llcontrol_ipc::*;
use std::{io::Cursor, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};

#[tokio::test(flavor = "current_thread")]
async fn abandoned_contended_group_commit_keeps_pins_and_executor_until_publication() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_dir.path().into());
    let image = source
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    let video = source
        .media_assets()
        .import(
            "Pending".into(),
            H264Clip {
                fps: 24,
                units: vec![H264AccessUnit {
                    bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                    keyframe: true,
                }],
            },
        )
        .unwrap();
    let ProfileContent::Screen(mut theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        unreachable!()
    };
    theme.background_image =
        Some(serde_json::from_value(serde_json::json!({"asset":image,"fit":"crop"})).unwrap());
    let source_settings = DeviceSettings {
        theme: Some(theme),
        screen_collections: Some(ScreenCollections::defaults(1)),
        ..Default::default()
    };
    let source_library = ProfileLibrary {
        profiles: vec![NamedProfile {
            slot: 7,
            name: "Video".into(),
            content: ProfileContent::ScreenMedia(video.key),
        }],
        ..Default::default()
    };
    let profile = GroupProfile::capture(&source_settings, &source_library, GroupKind::Screen)
        .unwrap()
        .profile;
    let mut bytes = Vec::new();
    ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    )
    .export_group(&profile, &mut bytes)
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let prepared = Arc::new(
        ProfileArchives::new(
            store.theme_assets(),
            store.theme_fonts(),
            store.media_assets(),
        )
        .prepare_group(&mut Cursor::new(bytes))
        .unwrap(),
    );
    let image = prepared.uploaded_images().first().unwrap().clone();
    let video = prepared.uploaded_media().first().unwrap().clone();
    let revision = store
        .theme_assets()
        .list_named()
        .unwrap()
        .into_iter()
        .find(|a| a.key == image)
        .unwrap()
        .revision;
    let initial = DeviceSettings::default();
    let expected = store
        .snapshot_profile_group("device", &initial, GroupKind::Screen)
        .unwrap()
        .revision;
    let locked = store.clone();
    let (ready, ready_rx) = oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        let _guard = locked.state.access().unwrap();
        ready.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).is_ok()
    });
    ready_rx.await.unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (_stop, stop) = watch::channel(false);
    let (task, _status) = spawn_updates_observed(store.clone(), "device".into(), initial, rx, stop);
    let (reply, lost) = oneshot::channel();
    drop(lost);
    tx.send(SettingsUpdate::CommitProfileGroup {
        operation_id: "a".repeat(32),
        expected,
        prepared,
        reply,
    })
    .await
    .unwrap();
    // Preparation is now owned only by the queued command/blocking worker.
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(store
        .theme_assets()
        .delete_unreferenced(&image, revision, &[], || Ok(false))
        .is_err());
    assert!(store
        .media_assets()
        .delete_unreferenced(&video, || Ok(false))
        .is_err());
    release.send(()).unwrap();
    assert!(
        blocker.await.unwrap(),
        "a blocked settings gate must not block the async timer"
    );
    let (reply, response) = oneshot::channel();
    tx.send(SettingsUpdate::ReadGroupImport {
        operation_id: "a".repeat(32),
        reply,
    })
    .await
    .unwrap();
    let readback = response.await.unwrap().unwrap().unwrap();
    assert!(readback.selected_content_current);
    assert_eq!(readback.profiles.revision, 1);
    drop(tx);
    task.await.unwrap();
    assert_eq!(store.read("device").unwrap().group_imports.len(), 1);
}
