use llcontrol_daemon::{builtin_media, media_assets::MediaAssetStore};
use std::process::Command;

#[test]
fn builtin_video_is_immutable_streamable_and_survives_disk_shadowing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("absent");
    let store = MediaAssetStore::new(root.clone());
    let info = builtin_media::info();
    assert_eq!(
        store.list().unwrap().as_slice(),
        std::slice::from_ref(&info)
    );
    assert!(!root.exists());
    assert_eq!((info.frames, info.fps), (576, 24));
    assert!(info.is_default);
    assert!(store
        .delete_unreferenced(builtin_media::KEY, || Ok(false))
        .is_err());
    assert!(
        store.load(builtin_media::KEY).is_err(),
        "long video must use streaming"
    );
    let mut source = store.acquire_source(builtin_media::KEY).unwrap();
    let mut bytes = Vec::new();
    for frame in 0..source.frames() {
        let unit = source.read_unit(frame).unwrap();
        llcontrol_ipc::validate_h264_access_unit(&unit, frame == 0).unwrap();
        bytes.extend(unit.bytes);
    }
    assert_eq!(bytes.len() as u64, info.encoded_bytes);
    assert!(source.read_unit(source.frames()).is_err());
    let first = source.read_unit(0).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    for ext in ["json", "lcm"] {
        std::fs::write(
            root.join(format!("{}.{ext}", builtin_media::KEY)),
            b"corrupt",
        )
        .unwrap();
    }
    let restarted = MediaAssetStore::new(root);
    assert_eq!(restarted.list().unwrap(), [info]);
    assert_eq!(
        restarted
            .acquire_source(builtin_media::KEY)
            .unwrap()
            .read_unit(0)
            .unwrap(),
        first
    );
    assert!(restarted
        .delete_unreferenced(builtin_media::KEY, || Ok(false))
        .is_err());

    // Decode every AU, rather than inferring codec validity from NAL markers.
    let h264 = temp.path().join("builtin.h264");
    std::fs::write(&h264, bytes).unwrap();
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(&h264)
        .args(["-threads", "1", "-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let frames: Vec<_> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    assert_eq!(frames.len(), 576);
    assert_ne!(
        frames[0].rsplit(',').next(),
        frames[288].rsplit(',').next(),
        "video must animate"
    );
}
