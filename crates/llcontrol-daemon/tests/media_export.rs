use futures::StreamExt;
use llcontrol_daemon::{builtin_media, media_assets::MediaAssetStore, media_export::MediaExports};
use llcontrol_ipc::{H264AccessUnit, H264Clip};

fn clip() -> H264Clip {
    H264Clip {
        fps: 24,
        units: vec![
            H264AccessUnit {
                bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
                keyframe: true,
            },
            H264AccessUnit {
                bytes: vec![0, 0, 1, 1, 2],
                keyframe: false,
            },
        ],
    }
}

#[tokio::test]
async fn finite_source_preserves_order_and_pin_until_last_frame_or_drop() {
    let root = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(root.path().join("media"));
    let saved = store.import("Saved".into(), clip()).unwrap();
    let exports = MediaExports::new(store.clone());
    assert!(exports.open(builtin_media::KEY.into()).await.is_err());
    assert!(exports.open("../../outside".into()).await.is_err());
    let mut first = exports.open(saved.key.clone()).await.unwrap();
    let second = exports.clone().open(saved.key.clone()).await.unwrap();
    assert!(exports.open(saved.key.clone()).await.is_err());
    assert_eq!((first.fps, first.frames), (24, 2));
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    assert_eq!(first.units.next().await.unwrap().unwrap(), clip().units[0]);
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    drop(second);
    assert_eq!(first.units.next().await.unwrap().unwrap(), clip().units[1]);
    // Final frame releases the pin even without another next() or stream drop.
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    assert!(first.units.next().await.is_none());
    assert!(exports.open(saved.key).await.is_err());
}

#[tokio::test]
async fn indexed_long_source_is_finite_and_late_corruption_releases_admission_and_pin() {
    for corrupt in [false, true] {
        let (root, store, saved) = tokio::task::spawn_blocking(|| {
            let root = tempfile::tempdir().unwrap();
            let store = MediaAssetStore::new(root.path().join("media"));
            let mut writer = store.create_file("Long source".into()).unwrap();
            for index in 0..586 {
                let mut unit = clip().units[usize::from(index != 0)].clone();
                unit.bytes
                    .extend_from_slice(format!("unit-{index}").as_bytes());
                writer.append(&unit).unwrap();
            }
            let saved = store.publish_file(writer).unwrap();
            (root, store, saved)
        })
        .await
        .unwrap();
        let exports = MediaExports::new(store.clone());
        let mut source = exports.open(saved.key.clone()).await.unwrap();
        assert_eq!(source.frames, 586);
        let mut count = 0;
        let mut transferred = 0;
        let mut failed = false;
        while let Some(result) = source.units.next().await {
            match result {
                Ok(unit) => {
                    assert!(unit.bytes.ends_with(format!("unit-{count}").as_bytes()));
                    transferred += unit.bytes.len() as u64;
                    count += 1;
                    if corrupt && count == 585 {
                        let path = root.path().join("media").join(format!("{}.lcm", saved.key));
                        tokio::task::spawn_blocking(move || {
                            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
                            file.set_len(file.metadata().unwrap().len() - 1).unwrap();
                        })
                        .await
                        .unwrap();
                    }
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if !corrupt {
            assert_eq!(transferred, source.encoded_bytes);
        }
        assert_eq!(failed, corrupt);
        assert_eq!(count, if corrupt { 585 } else { 586 });
        assert!(source.units.next().await.is_none());
        let key = saved.key;
        tokio::task::spawn_blocking(move || store.delete_unreferenced(&key, || Ok(false)))
            .await
            .unwrap()
            .unwrap();
        drop(root);
    }
}
