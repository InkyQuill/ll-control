use super::*;
use std::process::Command as Process;

fn caps(width: i32, height: i32) -> gst::Caps {
    gst::Caps::builder("video/x-raw")
        .field("width", width)
        .field("height", height)
        .field("framerate", gst::Fraction::new(24, 1))
        .build()
}

fn source() -> (Source, gst::Element) {
    gst::init().unwrap();
    let generator = element("videotestsrc").unwrap();
    generator.set_property("is-live", true);
    let filter = element("capsfilter").unwrap();
    filter.set_property("caps", caps(320, 240));
    let bin = gst::Bin::new();
    bin.add_many([&generator, &filter]).unwrap();
    generator.link(&filter).unwrap();
    let pad = gst::GhostPad::with_target(&filter.static_pad("src").unwrap()).unwrap();
    bin.add_pad(&pad).unwrap();
    (Source::Controlled(bin.upcast()), filter)
}

fn wait(recorder: &Recorder, predicate: impl Fn(&Status) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = recorder.status();
        assert!(!matches!(status.phase, Phase::Failed(_)), "{status:?}");
        if predicate(&status) {
            return;
        }
        assert!(Instant::now() < deadline, "{status:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_source(recorder: &Recorder, width: i32, height: i32) {
    wait(recorder, |_| {
        recorder
            .progress
            .latest
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|sample| {
                let caps = sample.caps().unwrap();
                let s = caps.structure(0).unwrap();
                s.get::<i32>("width").unwrap() == width && s.get::<i32>("height").unwrap() == height
            })
    });
}

fn red() -> crate::media_prepare::VideoCrop {
    crate::media_prepare::VideoCrop {
        x: 0.75,
        y: 0.2,
        width: 0.06,
        height: 0.05,
    }
}

fn pixels(file: &RecordingFile) -> Vec<[u8; 3]> {
    let info = super::tests::probe(file);
    assert_eq!(info["streams"][0]["width"], 320);
    assert_eq!(info["streams"][0]["height"], 240);
    assert_eq!(info["streams"][0]["sample_aspect_ratio"], "1:1");
    assert_eq!(info["streams"][0]["r_frame_rate"], "24/1");
    let duration = info["streams"][0]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((duration - file.frames as f64 / FPS as f64).abs() < 0.05);

    let output = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
        .args([
            "-vf",
            "scale=1:1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout.len() as u64, file.frames * 3);
    output
        .stdout
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn assert_red_file(file: &RecordingFile) {
    for (index, pixel) in pixels(file).iter().enumerate() {
        assert!(
            pixel[0] > 200 && pixel[1] < 35 && pixel[2] < 35,
            "Normalized red crop changed at frame {index}: {pixel:?}"
        );
    }
}

fn assert_full_snapshot(recorder: &Recorder, width: usize, height: usize) {
    let temporary = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temporary.path(), recorder.snapshot().unwrap()).unwrap();
    let output = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(temporary.path())
        .args([
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert_eq!(output.stdout.len(), width * height * 3);
    for (x, channel) in [(0.75, 0), (0.9, 2)] {
        let offset = ((height as f64 * 0.2) as usize * width + (width as f64 * x) as usize) * 3;
        let p = &output.stdout[offset..offset + 3];
        assert!(
            p[channel] > 200 && p[1] < 35 && p[2 - channel] < 35,
            "{p:?}"
        );
    }
}

#[test]
fn growing_source_preserves_normalized_crop_and_encoded_canvas() {
    let (source, control) = source();
    let recorder = Recorder::spawn_mode(source, MAX_FRAMES, MAX_BYTES, true).unwrap();
    wait_source(&recorder, 320, 240);
    recorder.set_region(red()).unwrap();
    recorder.resume().unwrap();
    wait(&recorder, |s| s.frames >= 6);
    control.set_property("caps", caps(640, 480));
    wait_source(&recorder, 640, 480);
    let frames = recorder.status().frames;
    wait(&recorder, |s| s.frames >= frames + 6);
    let file = recorder.finish().unwrap();
    assert_red_file(&file);
}

#[test]
fn shrinking_and_repeated_source_changes_preserve_crop_and_full_snapshot() {
    let (source, control) = source();
    let recorder = Recorder::spawn_mode(source, MAX_FRAMES, MAX_BYTES, true).unwrap();
    wait_source(&recorder, 320, 240);
    recorder.set_region(red()).unwrap();
    recorder.resume().unwrap();
    wait(&recorder, |s| s.frames >= 6);
    for (width, height) in [(160, 90), (640, 480), (144, 192), (320, 240)] {
        control.set_property("caps", caps(width, height));
        wait_source(&recorder, width, height);
        let frames = recorder.status().frames;
        wait(&recorder, |s| s.frames >= frames + 6);
        recorder.pause().unwrap();
        assert_full_snapshot(&recorder, width as usize, height as usize);
        recorder.resume().unwrap();
    }
    let file = recorder.finish().unwrap();
    assert_red_file(&file);
}

#[test]
fn setup_resize_keeps_first_canvas_but_preview_has_current_source_geometry() {
    let (source, control) = source();
    let recorder = Recorder::spawn_mode(source, MAX_FRAMES, MAX_BYTES, true).unwrap();
    wait_source(&recorder, 320, 240);
    control.set_property("caps", caps(640, 360));
    wait_source(&recorder, 640, 360);
    assert_full_snapshot(&recorder, 640, 360);
    assert_eq!(recorder.status().frames, 0);
    assert_eq!(recorder.status().recorded_millis(), 0);
    recorder.set_region(red()).unwrap();
    recorder.resume().unwrap();
    wait(&recorder, |s| s.frames >= 6);
    let file = recorder.finish().unwrap();
    assert_red_file(&file);
}

#[test]
fn paused_resize_accepts_new_normalized_region_without_changing_old_frames() {
    let (source, control) = source();
    let recorder = Recorder::spawn_mode(source, MAX_FRAMES, MAX_BYTES, true).unwrap();
    wait_source(&recorder, 320, 240);
    recorder.set_region(red()).unwrap();
    recorder.resume().unwrap();
    wait(&recorder, |s| s.frames >= 6);
    recorder.pause().unwrap();
    let before = recorder.status();
    control.set_property("caps", caps(144, 192));
    recorder
        .set_region(crate::media_prepare::VideoCrop { x: 0.9, ..red() })
        .unwrap();
    assert_eq!(recorder.status().frames, before.frames);
    assert_eq!(
        recorder.status().recorded_millis(),
        before.recorded_millis()
    );
    recorder.resume().unwrap();
    wait_source(&recorder, 144, 192);
    wait(&recorder, |s| s.frames >= before.frames + 6);
    recorder.pause().unwrap();
    assert_full_snapshot(&recorder, 144, 192);
    let file = recorder.finish().unwrap();
    for (index, pixel) in pixels(&file).iter().enumerate() {
        let channel = if (index as u64) < before.frames { 0 } else { 2 };
        assert!(
            pixel[channel] > 200 && pixel[1] < 35 && pixel[2 - channel] < 35,
            "Frame {index} changed interval color: {pixel:?}"
        );
    }
}

#[test]
fn oversized_source_change_fails_without_retaining_recording_files() {
    let (source, control) = source();
    let staging = tempfile::tempdir().unwrap();
    let recorder = Recorder::spawn_mode_in(
        source,
        MAX_FRAMES,
        MAX_BYTES,
        true,
        Some(staging.path().to_owned()),
    )
    .unwrap();
    wait_source(&recorder, 320, 240);
    recorder.resume().unwrap();
    wait(&recorder, |s| s.frames >= 6);
    control.set_property("caps", caps(5000, 300));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(recorder.status().phase, Phase::Failed(_)) {
        assert!(Instant::now() < deadline, "{:?}", recorder.status());
        thread::sleep(Duration::from_millis(10));
    }
    assert!(recorder.finish().is_err());
    assert_eq!(std::fs::read_dir(staging.path()).unwrap().count(), 0);
}
