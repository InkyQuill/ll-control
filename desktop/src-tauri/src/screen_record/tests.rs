use super::*;
use std::process::Command as Process;

fn wait_for(recorder: &Recorder, predicate: impl Fn(&Status) -> bool) {
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let status = recorder.status();
        if predicate(&status) {
            return;
        }
        assert!(!matches!(status.phase, Phase::Failed(_)), "{status:?}");
        assert!(Instant::now() < until, "{status:?}");
        thread::sleep(Duration::from_millis(10));
    }
}
fn probe(file: &RecordingFile) -> serde_json::Value {
    let output = Process::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(file.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn pause_resume_and_finish_produce_decodable_video_without_pause_gap() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.frames >= 6);
    recorder.pause().unwrap();
    let paused = recorder.status().frames;
    let paused_millis = recorder.status().recorded_millis();
    thread::sleep(Duration::from_millis(400));
    assert_eq!(recorder.status().frames, paused);
    assert_eq!(recorder.status().recorded_millis(), paused_millis);
    recorder.resume().unwrap();
    wait_for(&recorder, |s| s.frames >= paused + 6);
    recorder.pause().unwrap();
    let file = recorder.finish().unwrap();
    let info = probe(&file);
    let stream = &info["streams"][0];
    assert_eq!(stream["codec_name"], "h264");
    assert_eq!(
        stream["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        file.frames
    );
    let duration = stream["duration"].as_str().unwrap().parse::<f64>().unwrap();
    assert!(
        (duration - file.frames as f64 / FPS as f64).abs() < 0.06,
        "{info}"
    );
    let path = file.path();
    drop(file);
    assert!(!path.exists());
}
#[test]
fn automatic_limit_pauses_without_ui_poll_and_can_finalize() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        12,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.phase == Phase::LimitReached);
    assert_eq!(recorder.status().frames, 12);
    assert!(recorder.resume().is_err());
    let file = recorder.finish().unwrap();
    assert_eq!(probe(&file)["streams"][0]["nb_read_frames"], "12");
}
#[test]
fn output_budget_failure_and_cancellation_join_worker() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        1024,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    while !matches!(recorder.status().phase, Phase::Failed(_)) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(10));
    }
    assert!(recorder
        .finish()
        .unwrap_err()
        .to_string()
        .contains("size limit"));
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    let progress = recorder.progress.clone();
    drop(recorder);
    assert_eq!(*progress.phase.lock().unwrap(), Phase::Cancelled);
}

#[test]
fn source_session_closure_fails_recording_and_releases_worker() {
    let (close, closed) = tokio::sync::watch::channel(false);
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: Some(closed),
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    close.send(true).unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    while !matches!(recorder.status().phase, Phase::Failed(_)) {
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(10));
    }
    assert!(recorder
        .finish()
        .unwrap_err()
        .to_string()
        .contains("session closed"));
}

#[test]
fn sparse_updates_advance_recording_time_and_finalize_at_limit() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: true,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        48,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.phase == Phase::LimitReached);
    let status = recorder.status();
    assert!(status.recorded_millis() > 1500, "{status:?}");
    let file = recorder.finish().unwrap();
    let info = probe(&file);
    let duration = info["streams"][0]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((1.75..=2.05).contains(&duration), "{info}");
}

#[test]
fn repeated_resume_does_not_reset_active_time() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(250) {
        recorder.resume().unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    recorder.pause().unwrap();
    assert!(
        recorder.status().recorded_millis() >= 240,
        "{:?}",
        recorder.status()
    );
}

/// Explicit acceptance run: exercises the production budget in real time.
#[test]
#[ignore = "180-second synthetic acceptance; run explicitly"]
fn full_production_limit_pauses_and_finalizes_three_minute_video() {
    let started = Instant::now();
    let recorder = Recorder::synthetic().unwrap();
    // Observe sparsely; the engine must enforce the limit without UI commands.
    let mut next_report = 30;
    loop {
        let status = recorder.status();
        assert!(started.elapsed() < Duration::from_secs(200), "{status:?}");
        if status.phase == Phase::LimitReached {
            break;
        }
        assert_eq!(status.phase, Phase::Recording, "{status:?}");
        if started.elapsed().as_secs() >= next_report {
            eprintln!("elapsed={:?}, status={status:?}", started.elapsed());
            next_report += 30;
        }
        thread::sleep(Duration::from_millis(200));
    }
    let limited = recorder.status();
    assert!(started.elapsed() >= Duration::from_secs(179));
    assert!(
        (179_000..=180_000).contains(&limited.recorded_millis()),
        "{limited:?}"
    );
    assert!(limited.frames <= MAX_FRAMES, "{limited:?}");
    assert!(recorder.resume().is_err());
    thread::sleep(Duration::from_millis(500));
    let still_limited = recorder.status();
    assert_eq!(still_limited.phase, Phase::LimitReached);
    assert_eq!(still_limited.frames, limited.frames);
    assert_eq!(still_limited.recorded_millis(), limited.recorded_millis());
    let file = recorder.finish().unwrap();
    assert!(file.length() > 0 && file.length() <= MAX_BYTES);
    let info = probe(&file);
    let stream = &info["streams"][0];
    assert_eq!(stream["codec_name"], "h264");
    let decoded = stream["nb_read_frames"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert_eq!(decoded, limited.frames);
    assert_eq!(decoded, file.frames);
    let duration = stream["duration"].as_str().unwrap().parse::<f64>().unwrap();
    assert!((179.9..=180.05).contains(&duration), "{info}");
    assert!(
        (duration - decoded as f64 / FPS as f64).abs() < 0.06,
        "{info}"
    );
    eprintln!(
        "full limit: {limited:?}; decoded={decoded}; duration={duration}; bytes={}",
        file.length()
    );
    let path = file.path();
    drop(file);
    assert!(!path.exists());
}

#[test]
fn stopped_source_holds_last_frame_through_the_full_recording_duration() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: Some(6),
            resume_after: None,
            end_after: None,
        },
        48,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.frames >= 3);
    thread::sleep(Duration::from_millis(400));
    let stalled = recorder.status().frames;
    thread::sleep(Duration::from_millis(300));
    assert!(recorder.status().frames > stalled);
    assert!(stalled < 48);
    wait_for(&recorder, |s| s.phase == Phase::LimitReached);
    assert_eq!(recorder.status().recorded_millis(), 2000);
    assert!((47..=48).contains(&recorder.status().frames));
    assert!(recorder.resume().is_err());
    let file = recorder.finish().unwrap();
    let info = probe(&file);
    assert_eq!(info["streams"][0]["codec_name"], "h264");
    assert_eq!(
        info["streams"][0]["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        file.frames
    );
    let duration = info["streams"][0]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((1.95..=2.05).contains(&duration), "{info}");
    let output = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
        .args([
            "-vf",
            "crop=64:64:256:176",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let frames: Vec<_> = output.stdout.chunks_exact(64 * 64 * 3).collect();
    assert_eq!(frames.len() as u64, file.frames);
    // The moving noise patch changes early, then the stalled tail holds still.
    assert!(frames[..8].windows(2).any(|pair| pair[0] != pair[1]));
    let held = frames[24];
    for frame in &frames[25..] {
        let mean_error = frame
            .iter()
            .zip(held)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum::<u64>() as f64
            / held.len() as f64;
        assert!(mean_error < 3.0, "Held frame changed: {mean_error}");
    }
    let path = file.path();
    drop(file);
    assert!(!path.exists());
}

#[test]
fn cancellation_joins_after_source_stops_delivering_frames() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: Some(6),
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.frames >= 3);
    thread::sleep(Duration::from_millis(400));
    let frames = recorder.status().frames;
    thread::sleep(Duration::from_millis(300));
    assert!(recorder.status().frames > frames);
    let progress = recorder.progress.clone();
    let started = Instant::now();
    drop(recorder);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(*progress.phase.lock().unwrap(), Phase::Cancelled);
}

#[test]
fn paused_region_changes_crop_future_frames_without_changing_video_dimensions() {
    use crate::media_prepare::VideoCrop;
    let recorder = Recorder::synthetic().unwrap();
    let red = VideoCrop {
        x: 0.75,
        y: 0.2,
        width: 0.06,
        height: 0.05,
    };
    let blue = VideoCrop { x: 0.9, ..red };
    assert!(recorder.set_region(red).is_err());
    wait_for(&recorder, |s| s.frames >= 6);
    recorder.pause().unwrap();
    let first = recorder.status().frames;
    assert!(recorder
        .set_region(VideoCrop {
            width: f64::NAN,
            ..red
        })
        .is_err());
    recorder.set_region(red).unwrap();
    assert_eq!(recorder.status().frames, first);
    recorder
        .resume()
        .unwrap_or_else(|error| panic!("{error:#}; {:?}", recorder.status()));
    wait_for(&recorder, |s| s.frames >= first + 6);
    recorder.pause().unwrap();
    let second = recorder.status().frames;
    recorder.set_region(blue).unwrap();
    recorder
        .resume()
        .unwrap_or_else(|error| panic!("{error:#}; {:?}", recorder.status()));
    wait_for(&recorder, |s| s.frames >= second + 6);
    recorder.pause().unwrap();
    let file = recorder.finish().unwrap();
    let info = probe(&file);
    assert_eq!(info["streams"][0]["width"], 320);
    assert_eq!(info["streams"][0]["height"], 240);
    assert_eq!(info["streams"][0]["sample_aspect_ratio"], "1:1");
    let initial = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
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
    assert!(initial.status.success());
    assert_eq!(initial.stdout.len(), 320 * 240 * 3);
    let red_at = (48 * 320 + 240) * 3;
    let blue_at = (48 * 320 + 288) * 3;
    let r = &initial.stdout[red_at..red_at + 3];
    let b = &initial.stdout[blue_at..blue_at + 3];
    assert!(r[0] > 200 && r[1] < 35 && r[2] < 35, "initial red: {r:?}");
    assert!(b[2] > 200 && b[0] < 35 && b[1] < 35, "initial blue: {b:?}");

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
    let pixels: Vec<_> = output.stdout.chunks_exact(3).collect();
    assert_eq!(pixels.len() as u64, file.frames);
    for pixel in &pixels[first as usize..second as usize] {
        assert!(
            pixel[0] > 200 && pixel[1] < 35 && pixel[2] < 35,
            "red interval: {pixel:?}"
        );
    }
    for pixel in &pixels[second as usize..] {
        assert!(
            pixel[2] > 200 && pixel[0] < 35 && pixel[1] < 35,
            "blue interval: {pixel:?}"
        );
    }
}

#[test]
fn paused_snapshot_preserves_full_source_after_crop_without_recording_more_frames() {
    let recorder = Recorder::synthetic().unwrap();
    assert!(recorder.snapshot().is_err());
    wait_for(&recorder, |s| s.frames >= 6);
    recorder.pause().unwrap();
    recorder
        .set_region(crate::media_prepare::VideoCrop {
            x: 0.75,
            y: 0.2,
            width: 0.06,
            height: 0.05,
        })
        .unwrap();
    let before = recorder.status().frames;
    recorder.resume().unwrap();
    wait_for(&recorder, |s| s.frames >= before + 6);
    recorder.pause().unwrap();
    let paused = recorder.status();
    let jpeg = recorder.snapshot().unwrap();
    assert!(jpeg.starts_with(&[0xff, 0xd8]));
    assert!(jpeg.len() < 2 * 1024 * 1024);
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), jpeg).unwrap();
    let output = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(input.path())
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
    assert_eq!(output.stdout.len(), 320 * 240 * 3);
    let red = &output.stdout[(48 * 320 + 240) * 3..(48 * 320 + 240) * 3 + 3];
    let blue = &output.stdout[(48 * 320 + 288) * 3..(48 * 320 + 288) * 3 + 3];
    assert!(red[0] > 200 && red[1] < 35 && red[2] < 35, "{red:?}");
    assert!(blue[2] > 200 && blue[0] < 35 && blue[1] < 35, "{blue:?}");
    assert_eq!(recorder.status().frames, paused.frames);
    assert_eq!(
        recorder.status().recorded_millis(),
        paused.recorded_millis()
    );
}

#[test]
fn source_snapshot_downscales_large_rgba_frame_with_bounded_dimensions() {
    gst::init().unwrap();
    let buffer = gst::Buffer::from_mut_slice(vec![255u8; 1280 * 720 * 4]);
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("width", 1280i32)
        .field("height", 720i32)
        .field("framerate", gst::Fraction::new(24, 1))
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build();
    let sample = gst::Sample::builder().buffer(&buffer).caps(&caps).build();
    let bytes = encode_snapshot(&sample).unwrap();
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    let output = Process::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(file.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(info["streams"][0]["width"], 640);
    assert_eq!(info["streams"][0]["height"], 360);
}

#[test]
fn setup_preview_records_nothing_and_first_saved_frame_uses_selected_region() {
    let recorder = Recorder::spawn_mode(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: None,
        },
        FPS,
        MAX_BYTES,
        true,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while recorder.snapshot().is_err() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    // Setup lasts longer than the recording budget without consuming it.
    thread::sleep(Duration::from_millis(1200));
    assert_eq!(recorder.status().phase, Phase::Setup);
    assert_eq!(recorder.status().frames, 0);
    assert_eq!(recorder.status().active_millis, 0);
    assert_eq!(recorder.progress.bytes.load(Ordering::Acquire), 0);
    assert!(recorder.pause().is_err());
    let region = crate::media_prepare::VideoCrop {
        x: 0.75,
        y: 0.2,
        width: 0.06,
        height: 0.05,
    };
    recorder.set_region(region).unwrap();
    assert_eq!(recorder.status().region, Some(region));
    assert!(recorder
        .set_region(crate::media_prepare::VideoCrop {
            width: 2.0,
            ..region
        })
        .is_err());
    assert_eq!(recorder.status().region, Some(region));
    recorder.resume().unwrap();
    wait_for(&recorder, |s| s.frames >= 6);
    recorder.pause().unwrap();
    let paused = recorder.status();
    thread::sleep(Duration::from_millis(400));
    assert_eq!(recorder.status().frames, paused.frames);
    assert_eq!(recorder.status().active_millis, paused.active_millis);
    recorder.resume().unwrap();
    wait_for(&recorder, |s| s.phase == Phase::LimitReached);
    let file = recorder.finish().unwrap();
    let info = probe(&file);
    let duration = info["streams"][0]["duration"]
        .as_str()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!(
        (duration - file.frames as f64 / FPS as f64).abs() < 0.06,
        "{info}"
    );
    // One-frame scheduling tolerance; setup must not spend the recording budget.
    assert!((FPS - 1..=FPS).contains(&file.frames), "{info}");
    assert!(
        (0.95..=1.05).contains(&duration),
        "Setup/pause changed the recording budget: {info}"
    );
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
    assert!(file.frames > 0);
    for pixel in output.stdout.chunks_exact(3) {
        assert!(
            pixel[0] > 200 && pixel[1] < 35 && pixel[2] < 35,
            "Uncropped frame: {pixel:?}"
        );
    }
}

#[test]
fn setup_without_frames_can_be_cancelled_and_cannot_start_recording() {
    let recorder = Recorder::spawn_mode(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: Some(0),
            resume_after: None,
            end_after: None,
        },
        MAX_FRAMES,
        MAX_BYTES,
        true,
    )
    .unwrap();
    assert!(recorder.snapshot().is_err());
    assert!(recorder.resume().is_err());
    assert_eq!(recorder.status().phase, Phase::Setup);
    assert_eq!(recorder.status().frames, 0);
    drop(recorder);
}

#[test]
fn source_recovers_from_delivery_gap_and_replaces_the_held_frame() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: Some(6),
            resume_after: Some(30),
            end_after: None,
        },
        60,
        MAX_BYTES,
    )
    .unwrap();
    wait_for(&recorder, |s| s.phase == Phase::LimitReached);
    let file = recorder.finish().unwrap();
    assert!((59..=60).contains(&file.frames));
    let output = Process::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file.path())
        .args([
            "-vf",
            "crop=64:64:256:176",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let frames: Vec<_> = output.stdout.chunks_exact(64 * 64 * 3).collect();
    assert_eq!(frames.len() as u64, file.frames);
    let distance = |a: &[u8], b: &[u8]| {
        a.iter()
            .zip(b)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum::<u64>() as f64
            / a.len() as f64
    };
    for pair in frames[12..24].windows(2) {
        assert!(distance(pair[0], pair[1]) < 3.0, "Stalled frame changed");
    }
    assert!(
        frames[42..]
            .windows(2)
            .any(|pair| distance(pair[0], pair[1]) > 5.0),
        "Recovered frames remained frozen"
    );
}

#[test]
fn finite_source_eos_is_not_hidden_by_last_frame_repetition() {
    let recorder = Recorder::spawn(
        Source::Synthetic {
            closed: None,
            sparse: false,
            stop_after: None,
            resume_after: None,
            end_after: Some(6),
        },
        MAX_FRAMES,
        MAX_BYTES,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(recorder.status().phase, Phase::Failed(_)) {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert!(recorder
        .finish()
        .unwrap_err()
        .to_string()
        .contains("source ended"));
}
