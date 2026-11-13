//! Crash recovery uses a child process: no Rust Drop runs after kill().
use llcontrol_daemon::{
    media_assets::MediaAssetStore, media_file::MediaFileWriter, media_upload::MediaUploads,
};
use llcontrol_ipc::{H264AccessUnit, MediaUploadRequest as Request};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn unit() -> H264AccessUnit {
    H264AccessUnit {
        bytes: vec![0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1],
        keyframe: true,
    }
}
fn begin() -> Request {
    Request::Begin {
        id: "a".repeat(32),
        name: "Interrupted".into(),
        frames: 2,
        encoded_bytes: 30,
    }
}

#[test]
fn upload_crash_child() {
    let Some(root) = std::env::var_os("LLCONTROL_MEDIA_CRASH_FIXTURE") else {
        return;
    };
    let root = Path::new(&root);
    let store = MediaAssetStore::new(root.join("media"));
    let uploads = MediaUploads::new(store.clone());
    uploads.handle(begin()).unwrap();
    uploads
        .handle(Request::Chunk {
            id: "a".repeat(32),
            index: 0,
            unit: unit(),
        })
        .unwrap();
    let mut writer = store.create_file("Published".into()).unwrap();
    writer.append(&unit()).unwrap();
    let saved = store.publish_file(writer).unwrap();
    fs::write(root.join("saved-key"), &saved.key).unwrap();
    // Simulate an interrupted copy in the exact publication namespace, holding
    // the same directory lock as finish. Its payload remains intentionally invalid.
    let lock = fs::File::open(root.join("media")).unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
    fs::write(
        root.join("media/.llcontrol-media-publish-Ab12CD.tmp"),
        b"unfinished copy",
    )
    .unwrap();
    fs::write(root.join("ready"), b"ready").unwrap();
    loop {
        std::thread::park();
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn crashed_upload_is_cleaned_without_touching_live_publication_or_saved_media() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "upload_crash_child", "--nocapture"])
            .env("LLCONTROL_MEDIA_CRASH_FIXTURE", dir.path())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let start = Instant::now();
    while !dir.path().join("ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before staging"
        );
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "child readiness timeout"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let root = dir.path().join("media");
    let orphan = root.join(".llcontrol-media-publish-Ab12CD.tmp");
    assert!(MediaFileWriter::new(&root, "Concurrent".into()).is_err());
    assert!(orphan.exists(), "a live publication is not garbage");
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    assert!(orphan.exists(), "no Drop after crash");
    let unrelated = root.join(".tmp-unrelated");
    fs::write(&unrelated, b"keep").unwrap();
    let target = dir.path().join("outside");
    fs::write(&target, b"keep target").unwrap();
    let link = root.join(".llcontrol-media-publish-ABCD12.tmp");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let store = MediaAssetStore::new(root.clone());
    let uploads = MediaUploads::new(store.clone());
    uploads.handle(begin()).unwrap();
    assert!(!orphan.exists());
    assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read(target).unwrap(), b"keep target");
    let key = fs::read_to_string(dir.path().join("saved-key")).unwrap();
    assert_eq!(store.user_list().unwrap().len(), 1);
    assert_eq!(
        store.acquire_source(&key).unwrap().read_unit(0).unwrap(),
        unit()
    );
    drop(uploads);
    assert_eq!(
        fs::read_dir(root).unwrap().count(),
        3,
        "only saved, unrelated and symlink remain"
    );
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
