//! Explicit release qualification: real codec work at the public duration limit.
use lconnect_media::media_prepare::{prepare_file, Fit};
use std::{fs::File, io::Write, process::Command, sync::atomic::AtomicBool};

#[test]
#[ignore = "encodes and decodes one hour at 24fps; run explicitly for release qualification"]
fn one_hour_source_prepares_all_frames_and_decodes_after_source_removal() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("hour.mp4");
    // A low-resolution, low-rate owned source keeps fixture creation cheap;
    // the production preparer still scales and emits every 480x480/24fps frame.
    let output = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=32x32:r=1:d=3600",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-threads",
            "1",
        ])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut prepared =
        prepare_file(&source, Fit::Contain, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(prepared.frames(), lconnect_ipc::MAX_MEDIA_UPLOAD_FRAMES);
    assert!(prepared.encoded_bytes() <= lconnect_ipc::MAX_MEDIA_UPLOAD_BYTES);
    std::fs::remove_file(&source).unwrap();
    let stream = root.path().join("hour.h264");
    {
        let mut file = File::create(&stream).unwrap();
        for index in 0..prepared.frames() {
            file.write_all(&prepared.read_unit(index).unwrap().bytes)
                .unwrap();
        }
    }
    assert_eq!(
        std::fs::metadata(&stream).unwrap().len(),
        prepared.encoded_bytes()
    );
    assert!(prepared.read_unit(prepared.frames()).is_err());
    drop(prepared);
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,nb_read_frames",
            "-of",
            "json",
        ])
        .arg(&stream)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["streams"][0]["width"], 480);
    assert_eq!(value["streams"][0]["height"], 480);
    assert_eq!(value["streams"][0]["nb_read_frames"], "86400");
}
