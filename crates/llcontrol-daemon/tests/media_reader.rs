use llcontrol_daemon::{media_assets::MediaAssetStore, media_reader::MediaReader};
use llcontrol_ipc::{H264AccessUnit, H264Clip};
use std::time::Duration;

fn unit(index: usize) -> H264AccessUnit {
    let mut bytes = if index == 0 {
        vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1]
    } else {
        vec![0, 0, 1, 1, 1]
    };
    bytes.extend_from_slice(format!("frame-{index}").as_bytes());
    H264AccessUnit {
        bytes,
        keyframe: index == 0,
    }
}

// Framing fixtures exercise storage/ownership; decoder qualification lives in media_file.rs.
#[tokio::test]
async fn legacy_and_indexed_sources_loop_in_order_and_release_only_their_own_pin() {
    for (indexed, keep_pending_save) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let (tmp, store, key, source) = tokio::task::spawn_blocking(move || {
            let tmp = tempfile::tempdir().unwrap();
            let store = MediaAssetStore::new(tmp.path().join("media"));
            let saved = if indexed {
                let mut writer = store.create_file("Long video".into()).unwrap();
                for index in 0..586 {
                    writer.append(&unit(index)).unwrap();
                }
                store.publish_file(writer).unwrap()
            } else {
                store
                    .import(
                        "Legacy".into(),
                        H264Clip {
                            fps: 24,
                            units: (0..3).map(unit).collect(),
                        },
                    )
                    .unwrap()
            };
            let source = store.acquire_source(&saved.key).unwrap();
            (tmp, store, saved.key, source)
        })
        .await
        .unwrap();
        let pending_save = keep_pending_save.then(|| source.pin());
        let mut reader = MediaReader::spawn(source);
        let frames = if indexed { 586 } else { 3 };
        assert_eq!(reader.fps(), 24);
        assert_eq!(reader.frame_count(), frames);
        tokio::time::timeout(Duration::from_secs(5), async {
            for index in 0..frames + 2 {
                assert_eq!(reader.next_unit().await.unwrap(), unit(index % frames));
            }
            let checking = store.clone();
            let checking_key = key.clone();
            tokio::task::spawn_blocking(move || {
                assert!(checking
                    .delete_unreferenced(&checking_key, || Ok(false))
                    .is_err());
            })
            .await
            .unwrap();
            reader.stop().await.unwrap();
            reader.stop().await.unwrap();
        })
        .await
        .unwrap();
        assert!(reader.next_unit().await.is_err());
        tokio::task::spawn_blocking(move || {
            // Playback ended, but a pending durable save still owns its reference.
            if pending_save.is_some() {
                assert!(store.delete_unreferenced(&key, || Ok(false)).is_err());
            }
            drop(pending_save);
            store.delete_unreferenced(&key, || Ok(false)).unwrap();
            assert!(store.user_list().unwrap().is_empty());
            drop(tmp);
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn truncated_indexed_payload_ends_stream_and_join_releases_file_pin() {
    let (tmp, store, key, source) = tokio::task::spawn_blocking(|| {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("media");
        let store = MediaAssetStore::new(root.clone());
        let mut writer = store.create_file("Corrupted after opening".into()).unwrap();
        writer.append(&unit(0)).unwrap();
        writer.append(&unit(1)).unwrap();
        let saved = store.publish_file(writer).unwrap();
        let source = store.acquire_source(&saved.key).unwrap();
        let path = root.join(format!("{}.lcm", saved.key));
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_len(file.metadata().unwrap().len() - 1).unwrap();
        (tmp, store, saved.key, source)
    })
    .await
    .unwrap();
    let mut reader = MediaReader::spawn(source);
    tokio::time::timeout(Duration::from_secs(5), async {
        assert_eq!(reader.next_unit().await.unwrap(), unit(0));
        assert!(reader.next_unit().await.is_err());
        assert!(reader.next_unit().await.is_err());
        reader.stop().await.unwrap();
    })
    .await
    .unwrap();
    tokio::task::spawn_blocking(move || {
        store.delete_unreferenced(&key, || Ok(false)).unwrap();
        drop(tmp);
    })
    .await
    .unwrap();
}

// These lifecycle/quota assertions count user publications, excluding the immutable catalog.
trait UserMediaList {
    fn user_list(&self) -> anyhow::Result<Vec<llcontrol_ipc::PreparedClipInfo>>;
}
impl UserMediaList for llcontrol_daemon::media_assets::MediaAssetStore {
    fn user_list(&self) -> anyhow::Result<Vec<llcontrol_ipc::PreparedClipInfo>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|clip| !clip.is_default)
            .collect())
    }
}
