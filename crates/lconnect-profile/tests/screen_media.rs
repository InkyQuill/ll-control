#[path = "support/screen_images.rs"]
mod archive_fixture;
#[path = "support/screen_media.rs"]
mod media_fixture;
use lconnect_profile::screen_media::inspect_and_prepare;
use std::{io::Cursor, process::Command, sync::atomic::AtomicBool};
const PNG: &[u8] = include_bytes!("fixtures/ScreenImage.png");
#[test]
fn gif_and_mp4_keep_multiple_moving_frames_in_owned_validated_clips_and_drop_cleans_staging() {
    for extension in ["gif", "mp4"] {
        let bytes = media_fixture::motion(extension);
        let archive = archive_fixture::archive(PNG, &format!("wmsq/motion.{extension}"), &bytes);
        let cancel = AtomicBool::new(false);
        let (_, mut prepared) = inspect_and_prepare(Cursor::new(archive), 0, &cancel).unwrap();
        assert_eq!(prepared.items.len(), 1);
        assert_eq!(prepared.deferred_entries, vec!["wmsq/art.png"]);
        assert!(!prepared.assets_published && !prepared.complete_screen_group_supported);
        let item = &mut prepared.items[0];
        assert_eq!(item.fps, 24);
        assert!(item.frames >= 6);
        let staging = item.file.staging_path().to_owned();
        assert!(staging.exists());
        for index in 0..item.frames {
            let unit = item.file.read_unit(index).unwrap();
            lconnect_ipc::validate_h264_access_unit(&unit, index == 0).unwrap();
        }
        let preview = item.file.preview(&cancel).unwrap();
        let decoded = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-threads",
                "1",
                "-filter_threads",
                "1",
                "-i",
            ])
            .arg(preview.lease().path())
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        assert!(decoded.status.success());
        let frame_bytes = 480 * 480 * 3;
        assert_eq!(decoded.stdout.len(), item.frames * frame_bytes);
        assert_ne!(
            &decoded.stdout[..frame_bytes],
            &decoded.stdout[decoded.stdout.len() - frame_bytes..]
        );
        let report = serde_json::to_value(&prepared).unwrap();
        assert!(report["items"][0].get("file").is_none());
        drop(prepared);
        assert!(!staging.exists());
    }
}
#[test]
fn early_cancellation_and_late_bad_media_never_return_a_partial_batch() {
    let bytes = media_fixture::motion("gif");
    let archive = archive_fixture::archive(PNG, "wmsq/motion.gif", &bytes);
    assert!(inspect_and_prepare(Cursor::new(&archive), 0, &AtomicBool::new(true)).is_err());
    // Both members are GIF candidates: first succeeds, second must fail.
    use std::io::Write;
    let mut zip = zip::ZipWriter::new_append(Cursor::new(archive.clone())).unwrap();
    zip.start_file("wmsq/z-bad.gif", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"not GIF").unwrap();
    let invalid = zip.finish().unwrap().into_inner();
    assert!(inspect_and_prepare(Cursor::new(invalid), 0, &AtomicBool::new(false)).is_err());
    let mut source =
        lconnect_profile::screen::inspect_windows_screen(Cursor::new(&archive), 0).unwrap();
    let mut broken = source
        .payloads
        .iter()
        .find(|p| p.entry.ends_with(".gif"))
        .unwrap()
        .sha256
        .clone();
    broken.replace_range(..1, if broken.starts_with('0') { "1" } else { "0" });
    source
        .payloads
        .iter_mut()
        .find(|p| p.entry.ends_with(".gif"))
        .unwrap()
        .sha256 = broken;
    assert!(lconnect_profile::screen_media::prepare_media(
        Cursor::new(archive),
        &source,
        &AtomicBool::new(false)
    )
    .is_err());
    let archive = archive_fixture::archive(PNG, "wmsq/bad.mp4", b"not media");
    assert!(inspect_and_prepare(Cursor::new(archive), 0, &AtomicBool::new(false)).is_err());
}
#[test]
fn remaining_batch_budget_rejects_overlong_source_and_zero_without_starting_a_decoder() {
    use lconnect_media::media_prepare::{prepare_file_budgeted, Fit};
    let cancel = AtomicBool::new(false);
    match prepare_file_budgeted(
        std::path::Path::new("/not-opened.mp4"),
        Fit::Stretch,
        0,
        1024,
        &cancel,
        |_| {},
    ) {
        Err(error) => assert!(error
            .to_string()
            .contains("Invalid remaining media preparation budget")),
        Ok(_) => panic!("zero budget accepted"),
    }
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("source.gif");
    std::fs::write(&file, media_fixture::motion("gif")).unwrap();
    assert!(prepare_file_budgeted(&file, Fit::Stretch, 4, 1024 * 1024, &cancel, |_| {}).is_err());
}
