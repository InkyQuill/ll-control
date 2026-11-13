use llcontrol_daemon::{media_assets::MediaAssetStore, media_upload::MediaUploads};
use llcontrol_ipc::{
    H264AccessUnit, H264Clip, MediaUploadPhase as Phase, MediaUploadRequest as Request,
};

fn unit() -> H264AccessUnit {
    H264AccessUnit {
        bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
        keyframe: true,
    }
}
fn begin(id: &str, frames: usize) -> Request {
    Request::Begin {
        id: id.into(),
        name: "Video".into(),
        frames,
        encoded_bytes: (frames * unit().bytes.len()) as u64,
    }
}
fn chunk(id: &str, index: usize) -> Request {
    Request::Chunk {
        id: id.into(),
        index,
        unit: unit(),
    }
}

#[test]
fn ordered_retry_finish_restart_and_post_commit_cancel_do_not_duplicate_or_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(dir.path().into());
    let uploads = MediaUploads::new(store.clone());
    let id = "a".repeat(32);
    uploads.handle(begin(&id, 2)).unwrap();
    assert!(uploads.handle(chunk(&id, 1)).is_err());
    assert!(uploads.handle(Request::Finish { id: id.clone() }).is_err());
    uploads.handle(chunk(&id, 0)).unwrap();
    assert_eq!(uploads.handle(chunk(&id, 0)).unwrap().received_frames, 1);
    let mut changed = unit();
    changed.bytes.push(99);
    assert!(uploads
        .handle(Request::Chunk {
            id: id.clone(),
            index: 0,
            unit: changed
        })
        .is_err());
    assert!(uploads.handle(begin(&id, 3)).is_err());
    assert_eq!(uploads.handle(begin(&id, 2)).unwrap().received_frames, 1);
    uploads.handle(chunk(&id, 1)).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let finishes: Vec<_> = (0..2)
        .map(|_| {
            let uploads = uploads.clone();
            let id = id.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                uploads.handle(Request::Finish { id }).unwrap()
            })
        })
        .collect();
    let receipts: Vec<_> = finishes
        .into_iter()
        .map(|job| job.join().unwrap())
        .collect();
    assert_eq!(
        serde_json::to_value(&receipts[0]).unwrap(),
        serde_json::to_value(&receipts[1]).unwrap()
    );
    let published = receipts.into_iter().next().unwrap();
    assert_eq!(published.phase, Phase::Published);
    assert_eq!(published.asset.unwrap().key, id);
    assert_eq!(
        uploads
            .handle(Request::Finish { id: id.clone() })
            .unwrap()
            .phase,
        Phase::Published
    );
    assert_eq!(
        uploads
            .handle(Request::Cancel { id: id.clone() })
            .unwrap()
            .phase,
        Phase::Published
    );
    assert!(uploads.handle(chunk(&id, 1)).is_err());
    assert_eq!(store.user_list().unwrap().len(), 1);
    drop(uploads);
    let restarted = MediaUploads::new(store.clone());
    for request in [
        begin(&id, 2),
        Request::Status { id: id.clone() },
        Request::Finish { id: id.clone() },
        Request::Cancel { id: id.clone() },
    ] {
        assert_eq!(restarted.handle(request).unwrap().phase, Phase::Published);
    }
    let mut source = store.acquire_source(&id).unwrap();
    assert_eq!(source.read_unit(1).unwrap(), unit());
    drop(source);
    store.delete_unreferenced(&id, || Ok(false)).unwrap();
    assert!(restarted.handle(Request::Status { id }).is_err());
}

#[test]
fn cancellation_bad_bounds_and_capacity_never_publish_partial_media() {
    let dir = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(dir.path().into());
    let uploads = MediaUploads::new(store.clone());
    assert!(uploads.handle(begin("../escape", 1)).is_err());
    assert!(uploads.handle(begin(&"e".repeat(32), 0)).is_err());
    assert!(uploads
        .handle(begin(
            &"e".repeat(32),
            llcontrol_ipc::MAX_MEDIA_UPLOAD_FRAMES + 1
        ))
        .is_err());
    assert!(uploads
        .handle(Request::Begin {
            id: "e".repeat(32),
            name: "Big".into(),
            frames: 1,
            encoded_bytes: llcontrol_ipc::MAX_MEDIA_UPLOAD_BYTES + 1
        })
        .is_err());
    for i in 0..4 {
        uploads.handle(begin(&format!("{i:032x}"), 2)).unwrap();
    }
    assert!(uploads.handle(begin(&"e".repeat(32), 2)).is_err());
    let id = format!("{:032x}", 0);
    let mut oversized = unit();
    oversized.bytes.resize(128 * 1024 + 1, 255);
    assert!(uploads
        .handle(Request::Chunk {
            id: id.clone(),
            index: 0,
            unit: oversized
        })
        .is_err());
    let mut invalid = unit();
    invalid.keyframe = false;
    assert!(uploads
        .handle(Request::Chunk {
            id: id.clone(),
            index: 0,
            unit: invalid
        })
        .is_err());
    uploads.handle(chunk(&id, 0)).unwrap();
    for _ in 0..2 {
        assert_eq!(
            uploads
                .handle(Request::Cancel { id: id.clone() })
                .unwrap()
                .phase,
            Phase::Cancelled
        );
    }
    assert_eq!(
        uploads.handle(begin(&id, 2)).unwrap().phase,
        Phase::Cancelled
    );
    assert!(uploads.handle(chunk(&id, 1)).is_err());
    assert!(uploads.handle(Request::Finish { id }).is_err());
    uploads.handle(begin(&"e".repeat(32), 2)).unwrap();
    assert!(store.user_list().unwrap().is_empty());
    drop(uploads);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn quota_failure_cleans_consumed_writer_and_cannot_retry_into_a_second_asset() {
    let dir = tempfile::tempdir().unwrap();
    let store = MediaAssetStore::new(dir.path().into());
    let uploads = MediaUploads::new(store.clone());
    let id = "b".repeat(32);
    uploads.handle(begin(&id, 1)).unwrap();
    uploads.handle(chunk(&id, 0)).unwrap();
    for index in 0..64 {
        store
            .import(
                index.to_string(),
                H264Clip {
                    fps: 24,
                    units: vec![unit()],
                },
            )
            .unwrap();
    }
    let result = uploads.handle(Request::Finish { id: id.clone() }).unwrap();
    assert_eq!(result.phase, Phase::Failed);
    assert!(result.error.unwrap().contains("full"));
    assert!(uploads.handle(Request::Finish { id }).is_err());
    assert_eq!(store.user_list().unwrap().len(), 64);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 64);
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
