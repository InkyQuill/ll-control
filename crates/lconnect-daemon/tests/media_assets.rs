use lconnect_daemon::media_assets::MediaAssetStore;
use lconnect_ipc::{H264AccessUnit, H264Clip};
fn clip() -> H264Clip {
    H264Clip {
        fps: 24,
        units: vec![H264AccessUnit {
            bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
            keyframe: true,
        }],
    }
}
#[test]
fn import_reload_and_corruption_preserve_original_records() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    assert!(store.user_list().unwrap().is_empty());
    let first = store.import("Анимация".into(), clip()).unwrap();
    let second = store.import("Анимация".into(), clip()).unwrap();
    assert_ne!(first.key, second.key);
    let restarted = MediaAssetStore::new(tmp.path().join("media"));
    assert_eq!(restarted.load(&first.key).unwrap(), clip());
    assert_eq!(restarted.user_list().unwrap().len(), 2);
    let path = tmp.path().join("media").join(format!("{}.json", first.key));
    let original = std::fs::read(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    value["version"] = 2.into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(restarted.load(&first.key).is_err());
    assert!(restarted.user_list().is_err());
    assert_eq!(restarted.load(&second.key).unwrap(), clip());
    std::fs::write(&path, original).unwrap();
    assert_eq!(restarted.load(&first.key).unwrap(), clip());
}
#[test]
fn rejected_inputs_never_create_assets_and_nonregular_files_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("media");
    let store = MediaAssetStore::new(root.clone());
    for name in ["".into(), " \t".into(), "a\nb".into(), "x".repeat(257)] {
        assert!(store.import(name, clip()).is_err());
    }
    let mut bad = clip();
    bad.units[0].keyframe = false;
    assert!(store.import("Invalid".into(), bad).is_err());
    assert!(!root.exists());
    assert!(store.load("../../outside").is_err());
    let saved = store.import("Good".into(), clip()).unwrap();
    let path = root.join(format!("{}.json", saved.key));
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("absent"), &path).unwrap();
    assert!(store.load(&saved.key).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(store.load(&saved.key).is_err());
}
#[test]
fn cloned_stores_serialize_quota_and_never_replace_existing_clips() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    for i in 0..63 {
        store.import(i.to_string(), clip()).unwrap();
    }
    let workers: Vec<_> = (0..4)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || store.import(format!("racer{i}"), clip()))
        })
        .collect();
    let successes = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .filter(Result::is_ok)
        .count();
    assert_eq!(successes, 1);
    assert_eq!(store.user_list().unwrap().len(), 64);
    assert_eq!(
        std::fs::read_dir(tmp.path().join("media")).unwrap().count(),
        64
    );
}
#[tokio::test]
async fn ipc_library_survives_restart_without_any_device() {
    use lconnect_daemon::{
        config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
        persistence::PersistenceStore, stats::Stats,
    };
    use lconnect_hid::MockBackend;
    use lconnect_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair};
    use serde_json::json;
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let mut saved = None;
    for restart in [false, true] {
        let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
        let stats = Arc::new(Stats::new());
        let cfg = Config::default();
        let manager = DeviceManager::new(
            Arc::new(MockBackend::new()),
            store.clone(),
            cfg.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let rpc = Arc::new(IpcServerImpl::new(
            manager.clone(),
            stats,
            cfg,
            store.clone(),
        ));
        let (a, b) = mem_pair();
        let first_rpc = rpc.clone();
        let server = tokio::spawn(async move { IpcServer::new(first_rpc).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        // Separate connections let the server execute both mutations concurrently.
        let (other_a, other_b) = mem_pair();
        let other_server =
            tokio::spawn(async move { IpcServer::new(rpc).serve_stream(other_b).await });
        let other_client = IpcClient::from_stream(other_a).await.unwrap();
        if !restart {
            assert_eq!(
                user_json(client.call("list_prepared_clips", json!({})).await.unwrap()),
                json!([])
            );
            saved = Some(
                client
                    .call(
                        "import_prepared_clip",
                        json!({"name":"Saved","clip":clip()}),
                    )
                    .await
                    .unwrap(),
            );
        }
        let saved = saved.as_ref().unwrap();
        assert_eq!(
            client
                .call("load_media_first_frame", json!({"key":saved["key"]}))
                .await
                .unwrap(),
            serde_json::to_value(&clip().units[0]).unwrap()
        );
        assert_eq!(
            user_json(client.call("list_prepared_clips", json!({})).await.unwrap()),
            json!([saved])
        );
        assert_eq!(
            client
                .call("load_prepared_clip", json!({"key":saved["key"]}))
                .await
                .unwrap(),
            json!(clip())
        );
        assert!(client
            .call("load_prepared_clip", json!({"key":"../escape"}))
            .await
            .is_err());
        assert!(client
            .call(
                "import_prepared_clip",
                json!({"name":"Bad","clip":{"fps":24,"units":[]}})
            )
            .await
            .is_err());
        assert_eq!(
            user_json(client.call("list_prepared_clips", json!({})).await.unwrap()),
            json!([saved])
        );
        if restart {
            let key = saved["key"].as_str().unwrap();
            let profile =
                json!({"slot":7,"name":"Cinema","content":{"kind":"screen_media","value":key}});
            let library = client
                .call(
                    "edit_profile",
                    json!({"expected_revision":0,"edit":{"action":"save","profile":profile}}),
                )
                .await
                .unwrap();
            assert_eq!(library["revision"], 1);
            assert!(client
                .call("delete_prepared_clip", json!({"key":key}))
                .await
                .is_err());
            let missing = json!({"slot":6,"name":"Missing","content":{"kind":"screen_media","value":"f".repeat(32)}});
            assert!(client
                .call(
                    "edit_profile",
                    json!({"expected_revision":1,"edit":{"action":"save","profile":missing}})
                )
                .await
                .is_err());
            assert_eq!(
                client.call("list_profiles", json!({})).await.unwrap(),
                library
            );
            // Simulated restart reads the saved reference; no session pin remains.
            let restarted = lconnect_daemon::profiles::ProfileStore::new(tmp.path().into());
            assert_eq!(
                serde_json::to_value(restarted.list().unwrap()).unwrap(),
                library
            );
            client.call("edit_profile", json!({"expected_revision":1,"edit":{"action":"reset","group":"screen","slot":7,"sensor_id":null}})).await.unwrap();

            let composition = json!({"version":2,"name":"Video with layers","background":[0,0,0],"background_video":key,"layers":[]});
            let profile = json!({"slot":7,"name":"Composition","content":{"kind":"screen","value":composition}});
            let composed_library = client
                .call(
                    "edit_profile",
                    json!({"expected_revision":2,"edit":{"action":"save","profile":profile}}),
                )
                .await
                .unwrap();
            assert_eq!(
                composed_library["profiles"][0]["content"]["value"],
                composition
            );
            assert!(client
                .call("delete_prepared_clip", json!({"key":key}))
                .await
                .is_err());
            assert_eq!(
                serde_json::to_value(restarted.list().unwrap()).unwrap(),
                composed_library
            );
            let mut missing = profile.clone();
            missing["content"]["value"]["background_video"] = json!("f".repeat(32));
            assert!(client
                .call(
                    "edit_profile",
                    json!({"expected_revision":3,"edit":{"action":"save","profile":missing}})
                )
                .await
                .is_err());
            assert_eq!(
                client.call("list_profiles", json!({})).await.unwrap(),
                composed_library
            );
            client.call("edit_profile", json!({"expected_revision":3,"edit":{"action":"reset","group":"screen","slot":7,"sensor_id":null}})).await.unwrap();

            // Both orders are valid: deletion wins before pin acquisition, or
            // the pending/profile reference wins. Neither may leave a dangling slot.
            for iteration in 0..8 {
                let clip_info = client
                    .call("import_prepared_clip", json!({"name":"Race","clip":clip()}))
                    .await
                    .unwrap();
                let race_key = clip_info["key"].as_str().unwrap();
                let before = client.call("list_profiles", json!({})).await.unwrap();
                let content = if iteration % 2 == 0 {
                    json!({"kind":"screen_media","value":race_key})
                } else {
                    json!({"kind":"screen","value":{"version":2,"name":"Race","background":[0,0,0],"background_video":race_key,"layers":[]}})
                };
                let (save, delete) = tokio::join!(
                    client.call("edit_profile", json!({"expected_revision":before["revision"],"edit":{"action":"save","profile":{"slot":5,"name":"Race","content":content}}})),
                    other_client.call("delete_prepared_clip", json!({"key":race_key})),
                );
                assert_ne!(
                    save.is_ok(),
                    delete.is_ok(),
                    "save={save:?}, delete={delete:?}"
                );
                if let Ok(library) = save {
                    assert!(client
                        .call("load_prepared_clip", json!({"key":race_key}))
                        .await
                        .is_ok());
                    client.call("edit_profile", json!({"expected_revision":library["revision"],"edit":{"action":"delete","group":"screen","slot":5}})).await.unwrap();
                    client
                        .call("delete_prepared_clip", json!({"key":race_key}))
                        .await
                        .unwrap();
                }
            }
            let (_, pin) = store.media_assets().acquire(key).unwrap();
            assert!(client
                .call("delete_prepared_clip", json!({"key":key}))
                .await
                .is_err());
            drop(pin);
            let mut settings = lconnect_daemon::persistence::DeviceSettings {
                media_clip: Some(key.into()),
                ..Default::default()
            };
            store.write("offline", &settings).unwrap();
            assert!(client
                .call("delete_prepared_clip", json!({"key":key}))
                .await
                .is_err());
            settings.media_clip = None;
            store.write("offline", &settings).unwrap();
            assert!(client
                .call("delete_prepared_clip", json!({"key":"../escape"}))
                .await
                .is_err());
            assert_eq!(
                client
                    .call("delete_prepared_clip", json!({"key":key}))
                    .await
                    .unwrap(),
                serde_json::Value::Null
            );
            assert_eq!(
                user_json(client.call("list_prepared_clips", json!({})).await.unwrap()),
                json!([])
            );
            assert!(MediaAssetStore::new(tmp.path().join("prepared-clips"))
                .user_list()
                .unwrap()
                .is_empty());
            let media = store.media_assets();
            let mut writer = media.create_file("Long saved profile".into()).unwrap();
            for _ in 0..586 {
                writer.append(&clip().units[0]).unwrap();
            }
            let long = media.publish_file(writer).unwrap();
            let before = client.call("list_profiles", json!({})).await.unwrap();
            let named = client.call("edit_profile", json!({"expected_revision":before["revision"],"edit":{"action":"save","profile":{"slot":6,"name":"Long","content":{"kind":"screen_media","value":long.key}}}})).await.unwrap();
            assert!(client
                .call("delete_prepared_clip", json!({"key":long.key}))
                .await
                .is_err());
            let listed = user_json(client.call("list_prepared_clips", json!({})).await.unwrap());
            assert_eq!(listed[0]["frames"], 586);
            client.call("edit_profile",json!({"expected_revision":named["revision"],"edit":{"action":"delete","group":"screen","slot":6}})).await.unwrap();
            client
                .call("delete_prepared_clip", json!({"key":long.key}))
                .await
                .unwrap();
            assert!(media.user_list().unwrap().is_empty());
        }
        drop(client);
        drop(other_client);
        other_server.abort();
        server.abort();
        let _ = other_server.await;
        let _ = server.await;
        manager.shutdown().await;
    }
}

#[test]
fn deletion_waits_for_all_playback_and_pending_write_pins() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    let saved = store.import("In use".into(), clip()).unwrap();
    let (loaded, playback) = store.acquire(&saved.key).unwrap();
    assert_eq!(loaded, clip());
    let pending_write = playback.clone();
    let (_, second_reader) = store.clone().acquire(&saved.key).unwrap();
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    drop(playback);
    drop(second_reader);
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    drop(pending_write);
    // A durable offline reference still protects the file after every pin drops.
    assert!(store.delete_unreferenced(&saved.key, || Ok(true)).is_err());
    assert_eq!(store.load(&saved.key).unwrap(), clip());
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    assert!(store.user_list().unwrap().is_empty());
    assert!(store.acquire(&saved.key).is_err());
}

#[test]
fn deletion_refuses_unknown_references_and_nonregular_paths_but_can_remove_corrupt_media() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    let saved = store.import("Corrupt".into(), clip()).unwrap();
    let other = store.import("Keep".into(), clip()).unwrap();
    let path = tmp.path().join("media").join(format!("{}.json", saved.key));
    std::fs::write(&path, b"broken record").unwrap();
    assert!(store
        .delete_unreferenced(&saved.key, || anyhow::bail!("unreadable settings"))
        .is_err());
    assert!(path.exists());
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    assert_eq!(store.load(&other.key).unwrap(), clip());
    assert!(store
        .delete_unreferenced("../../outside", || Ok(false))
        .is_err());
    std::os::unix::fs::symlink(tmp.path().join("outside"), &path).unwrap();
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    assert!(std::fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn delete_and_acquire_are_serialized_so_a_new_consumer_cannot_load_a_removed_clip() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    let saved = store.import("Race".into(), clip()).unwrap();
    let (inside_tx, inside_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let deleting = store.clone();
    let key = saved.key.clone();
    let deletion = std::thread::spawn(move || {
        deleting.delete_unreferenced(&key, || {
            inside_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            Ok(false)
        })
    });
    inside_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let reading = store.clone();
    let reader = std::thread::spawn(move || reading.acquire(&saved.key));
    release_tx.send(()).unwrap();
    deletion.join().unwrap().unwrap();
    assert!(reader.join().unwrap().is_err());
}

#[test]
fn deleted_clip_releases_quota_and_plain_thumbnail_reads_do_not_pin_it() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    let first = store.import("First".into(), clip()).unwrap();
    for i in 1..64 {
        store.import(i.to_string(), clip()).unwrap();
    }
    assert!(store.import("Overflow".into(), clip()).is_err());
    assert_eq!(store.load(&first.key).unwrap(), clip());
    store.delete_unreferenced(&first.key, || Ok(false)).unwrap();
    store.import("Replacement".into(), clip()).unwrap();
    assert_eq!(store.user_list().unwrap().len(), 64);
}

#[test]
fn indexed_and_legacy_media_share_listing_pins_and_durable_delete_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("media");
    let store = MediaAssetStore::new(root.clone());
    let legacy = store.import("Legacy".into(), clip()).unwrap();
    let mut writer = store.create_file("Long".into()).unwrap();
    for _ in 0..586 {
        writer.append(&clip().units[0]).unwrap();
    }
    assert_eq!(
        store.user_list().unwrap().len(),
        1,
        "staging is not published"
    );
    let indexed = store.publish_file(writer).unwrap();
    assert_eq!(indexed.frames, 586);
    assert_eq!(store.user_list().unwrap().len(), 2);
    assert!(
        store.load(&indexed.key).is_err(),
        "v2 never materializes as a diagnostic clip"
    );
    let mut source = store.acquire_source(&indexed.key).unwrap();
    assert_eq!(source.frames(), 586);
    assert_eq!(source.fps(), 24);
    assert_eq!(source.read_unit(585).unwrap(), clip().units[0]);
    let pending = source.pin();
    assert!(store
        .delete_unreferenced(&indexed.key, || Ok(false))
        .is_err());
    drop(source);
    assert!(store
        .delete_unreferenced(&indexed.key, || Ok(false))
        .is_err());
    drop(pending);
    assert!(store
        .delete_unreferenced(&indexed.key, || Ok(true))
        .is_err());
    assert!(store
        .delete_unreferenced(&indexed.key, || anyhow::bail!("unreadable references"))
        .is_err());
    let restarted = MediaAssetStore::new(root.clone());
    assert_eq!(restarted.user_list().unwrap().len(), 2);
    assert_eq!(
        restarted
            .acquire_source(&indexed.key)
            .unwrap()
            .read_unit(0)
            .unwrap(),
        clip().units[0]
    );
    store
        .delete_unreferenced(&indexed.key, || Ok(false))
        .unwrap();
    assert_eq!(store.load(&legacy.key).unwrap(), clip());
    let mut old_source = store.acquire_source(&legacy.key).unwrap();
    assert_eq!(old_source.read_unit(0).unwrap(), clip().units[0]);
    assert!(store
        .delete_unreferenced(&legacy.key, || Ok(false))
        .is_err());
    drop(old_source);
    store
        .delete_unreferenced(&legacy.key, || Ok(false))
        .unwrap();
    assert!(store.user_list().unwrap().is_empty());
}

#[test]
fn indexed_media_duplicates_corruption_and_quota_fail_without_hiding_records() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("media");
    let store = MediaAssetStore::new(root.clone());
    let legacy = store.import("Original".into(), clip()).unwrap();
    let mut writer = store.create_file("Indexed".into()).unwrap();
    writer.append(&clip().units[0]).unwrap();
    let indexed = store.publish_file(writer).unwrap();
    let collision = root.join(format!("{}.lcm", legacy.key));
    std::fs::copy(root.join(format!("{}.lcm", indexed.key)), &collision).unwrap();
    assert!(store.user_list().is_err());
    assert!(store.load(&legacy.key).is_err());
    assert!(store.acquire_source(&legacy.key).is_err());
    assert!(store
        .delete_unreferenced(&legacy.key, || Ok(false))
        .is_err());
    std::fs::remove_file(collision).unwrap();
    std::fs::write(root.join(format!("{}.lcm", indexed.key)), b"corrupt").unwrap();
    assert!(store.user_list().is_err());
    assert!(store.acquire_source(&indexed.key).is_err());
    store
        .delete_unreferenced(&indexed.key, || Ok(false))
        .unwrap();
    // Staging may begin below quota but must not bypass the commit-time check.
    let mut pending = store.create_file("Last".into()).unwrap();
    pending.append(&clip().units[0]).unwrap();
    for _ in 1..64 {
        store.import("Fill".into(), clip()).unwrap();
    }
    assert!(store.publish_file(pending).is_err());
    assert_eq!(store.user_list().unwrap().len(), 64);
    assert_eq!(
        std::fs::read_dir(root).unwrap().count(),
        64,
        "rejected staging cleaned up"
    );
}

// These lifecycle/quota assertions count user publications, excluding the immutable catalog.
trait UserMediaList {
    fn user_list(&self) -> anyhow::Result<Vec<lconnect_ipc::PreparedClipInfo>>;
}
impl UserMediaList for lconnect_daemon::media_assets::MediaAssetStore {
    fn user_list(&self) -> anyhow::Result<Vec<lconnect_ipc::PreparedClipInfo>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|clip| !clip.is_default)
            .collect())
    }
}

fn user_json(value: serde_json::Value) -> serde_json::Value {
    serde_json::Value::Array(
        value
            .as_array()
            .unwrap()
            .iter()
            .filter(|clip| clip["is_default"] != true)
            .cloned()
            .collect(),
    )
}

#[test]
fn inactive_page_videos_are_pinned_as_one_group_until_every_owner_releases() {
    let tmp = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(tmp.path().join("media"));
    let first = store.import("Rain".into(), clip()).unwrap();
    let second = store.import("Snow".into(), clip()).unwrap();
    let mut root:lconnect_ipc::LcdTheme=serde_json::from_value(serde_json::json!({"version":1,"name":"Basic","background":[0,0,0],"layers":[],"page_cache":[{"page":"rain","theme":{"version":3,"name":"Rain","background":[0,0,0],"background_video":first.key,"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":42}}]}},{"page":"snow","theme":{"version":3,"name":"Snow","background":[0,0,0],"background_video":second.key,"layers":[{"kind":"weather","effect":{"kind":"snow","intensity":1,"dynamic":false,"sensor":null,"seed":42}}]}}]})).unwrap();
    let group = store.pin_theme(&root).unwrap().unwrap();
    let pending = group.clone();
    drop(group);
    for key in [&first.key, &second.key] {
        assert!(store.delete_unreferenced(key, || Ok(false)).is_err());
    }
    drop(pending);
    for key in [&first.key, &second.key] {
        store.delete_unreferenced(key, || Ok(false)).unwrap();
    }
    assert!(store.pin_theme(&root).is_err());
    root.page_cache.clear();
    assert!(store.pin_theme(&root).unwrap().is_none());
}
