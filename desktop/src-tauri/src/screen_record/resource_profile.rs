//! Opt-in release-only engine profiling. Never opens a portal or hardware.
use super::*;
use std::io::Write;

fn emit(phase: &str, start: Instant, status: Option<&Status>) {
    println!(
        "LLCONTROL_RECORD_PROFILE {}",
        serde_json::json!({
            "phase": phase, "seconds": start.elapsed().as_secs_f64(),
            "frames": status.map(|s| s.frames),
            "recorded_millis": status.map(Status::recorded_millis),
        })
    );
    std::io::stdout().flush().unwrap();
}

fn check_paused_snapshot(recorder: &Recorder) -> Result<()> {
    match recorder.status().phase {
        Phase::Paused => {
            let snapshot = recorder.snapshot()?;
            ensure!(
                snapshot.starts_with(&[255, 216]) && snapshot.len() <= 2 * 1024 * 1024,
                "Invalid or oversized source snapshot"
            );
        }
        Phase::LimitReached => {
            ensure!(
                recorder.snapshot().is_err(),
                "Limit preview guard was bypassed"
            );
        }
        _ => bail!("Expected paused or limit-reached recording"),
    }
    Ok(())
}

#[test]
fn limit_profile_observes_snapshot_guard_before_finishing() {
    let recorder = Recorder::spawn_mode(
        Source::Profile {
            width: 320,
            height: 240,
        },
        4,
        MAX_BYTES,
        false,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while recorder.status().phase != Phase::LimitReached {
        assert!(!matches!(recorder.status().phase, Phase::Failed(_)));
        assert!(Instant::now() < until);
        thread::sleep(Duration::from_millis(10));
    }
    recorder.pause().unwrap();
    check_paused_snapshot(&recorder).unwrap();
    assert!(recorder.resume().is_err());
    let file = recorder.finish().unwrap();
    assert!(file.frames > 0 && file.frames <= 4);
    let info = super::tests::probe(&file);
    assert_eq!(
        info["streams"][0]["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        file.frames
    );
    let path = file.path();
    drop(file);
    assert!(!path.exists());
}

#[test]
#[ignore = "explicit release-only synthetic 1080p/4K resource qualification"]
fn profile_recording_engine_resources() -> Result<()> {
    ensure!(
        !cfg!(debug_assertions),
        "Profile an optimized release build"
    );
    let dimensions = std::env::var("LLCONTROL_RECORD_PROFILE_DIMENSIONS").unwrap();
    let (width, height) = match dimensions.as_str() {
        "320x240" => (320, 240),
        "1920x1080" => (1920, 1080),
        "3840x2160" => (3840, 2160),
        _ => panic!("Unknown profile dimensions"),
    };
    let seconds: u64 = std::env::var("LLCONTROL_RECORD_PROFILE_SECONDS")
        .unwrap()
        .parse()
        .unwrap();
    assert!((5..=180).contains(&seconds));
    let start = Instant::now();
    gst::init().unwrap();
    emit("starting", start, None);
    let recorder = Recorder::spawn_mode(
        Source::Profile { width, height },
        MAX_FRAMES,
        MAX_BYTES,
        true,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while recorder.snapshot().is_err() {
        assert!(
            Instant::now() < until,
            "Source preview did not become ready"
        );
        thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(recorder.status().frames, 0);
    emit("setup", start, Some(&recorder.status()));
    recorder.resume().unwrap();
    emit("recording", start, Some(&recorder.status()));
    thread::sleep(Duration::from_secs(seconds));
    let now = Instant::now();
    recorder.pause().unwrap();
    let pause_ms = now.elapsed().as_secs_f64() * 1000.0;
    let paused = recorder.status();
    emit("paused", start, Some(&paused));
    check_paused_snapshot(&recorder)?;
    thread::sleep(Duration::from_secs(2));
    assert_eq!(recorder.status().frames, paused.frames);
    assert_eq!(
        recorder.status().recorded_millis(),
        paused.recorded_millis()
    );
    let at_limit =
        paused.frames >= MAX_FRAMES || paused.recorded_millis() >= MAX_FRAMES * 1000 / FPS;
    if at_limit {
        assert!(recorder.resume().is_err());
        emit("limit_reached", start, Some(&recorder.status()));
    } else {
        recorder.resume().unwrap();
        emit("resumed", start, Some(&recorder.status()));
        thread::sleep(Duration::from_secs(2));
    }
    let now = Instant::now();
    let file = recorder.finish().unwrap();
    let finish_ms = now.elapsed().as_secs_f64() * 1000.0;
    emit("finished", start, None);
    let info = super::tests::probe(&file);
    let stream = &info["streams"][0];
    assert_eq!(stream["width"], width);
    assert_eq!(stream["height"], height);
    assert_eq!(
        stream["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        file.frames
    );
    let path = file.path();
    let length = file.length();
    let frames = file.frames;
    drop(file);
    assert!(!path.exists(), "Finalized recording survived its owner");
    emit("released", start, None);
    println!(
        "LLCONTROL_RECORD_PROFILE_RESULT {}",
        serde_json::json!({
            "dimensions": [width, height], "frames": frames, "bytes": length,
            "pause_ms": pause_ms, "finish_ms": finish_ms, "cleanup": true,
            "initial_interval_reached_limit": at_limit,
            "duration_seconds": stream["duration"],
        })
    );
    Ok(())
}
