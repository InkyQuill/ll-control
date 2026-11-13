//! Opt-in real PipeWire transport qualification. The wrapper owns the private
//! server and explicitly links its sole synthetic source (no session manager).
use super::*;

#[test]
#[ignore = "requires scripts/test-recording-pipewire.py private server"]
fn selected_pipewire_fd_records_real_plugin_frames() {
    let root =
        PathBuf::from(std::env::var_os("LLCONTROL_PRIVATE_PIPEWIRE").expect("private wrapper"));
    assert!(root.starts_with("/tmp"));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("llcontrol-private-pipewire-"));
    assert_eq!(
        std::fs::read_to_string(root.join("fixture-marker")).unwrap(),
        "synthetic-only\n"
    );
    assert_eq!(
        std::env::var_os("PIPEWIRE_RUNTIME_DIR"),
        Some(root.clone().into_os_string())
    );
    assert_eq!(
        std::env::var("PIPEWIRE_REMOTE").unwrap(),
        "nonexistent-recorder-remote"
    );
    assert!(std::env::var_os("DISPLAY").is_none());
    assert!(std::env::var_os("WAYLAND_DISPLAY").is_none());
    let node = std::env::var("LLCONTROL_PRIVATE_NODE")
        .unwrap()
        .parse()
        .unwrap();
    let serial = std::env::var("LLCONTROL_PRIVATE_SERIAL")
        .unwrap()
        .parse()
        .unwrap();
    let fd = UnixStream::connect(root.join("llcontrol-private"))
        .unwrap()
        .into();
    let recorder = Recorder::open(
        Source::Portal {
            node,
            serial: Some(serial),
        },
        Some(fd),
        true,
        None,
        None,
    )
    .unwrap();
    let child = recorder.worker.lock().unwrap().child.id();
    let staging = recorder.directory.as_ref().unwrap().path().to_owned();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match recorder.snapshot() {
            Ok(jpeg) => {
                assert_eq!(&jpeg[..2], &[255, 216]);
                break;
            }
            Err(error) => {
                assert!(Instant::now() < deadline, "snapshot: {error:#}");
                std::thread::sleep(IO_TICK);
            }
        }
    }
    assert_eq!(recorder.status().phase, Phase::Setup);
    assert_eq!(recorder.status().frames, 0);
    recorder
        .set_region(VideoCrop {
            x: 0.75,
            y: 0.2,
            width: 0.06,
            height: 0.05,
        })
        .unwrap();
    recorder.resume().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while recorder.status().frames < 8 {
        assert!(Instant::now() < deadline, "{:?}", recorder.status());
        std::thread::sleep(IO_TICK);
    }
    recorder.pause().unwrap();
    let frames = recorder.status().frames;
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(recorder.status().frames, frames);
    recorder.resume().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while recorder.status().frames < frames + 6 {
        assert!(Instant::now() < deadline, "{:?}", recorder.status());
        std::thread::sleep(IO_TICK);
    }
    let file = recorder.finish().unwrap();
    assert!(!PathBuf::from(format!("/proc/{child}")).exists());
    assert!(file.frames > frames);
    let probe = Command::new("ffprobe")
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
    assert!(probe.status.success(), "{:?}", probe.stderr);
    let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let stream = &metadata["streams"][0];
    assert_eq!(stream["width"], 320);
    assert_eq!(stream["height"], 240);
    assert_eq!(stream["r_frame_rate"], "24/1");
    assert_eq!(stream["sample_aspect_ratio"], "1:1");
    assert_eq!(
        stream["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        file.frames
    );
    let duration = stream["duration"].as_str().unwrap().parse::<f64>().unwrap();
    assert!((duration - file.frames as f64 / 24.0).abs() < 0.05);
    let decoded = Command::new("ffmpeg")
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
    assert!(decoded.status.success(), "{:?}", decoded.stderr);
    assert_eq!(decoded.stdout.len() as u64, file.frames * 3);
    for pixel in decoded.stdout.chunks_exact(3) {
        assert!(
            pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40,
            "selected red crop: {pixel:?}"
        );
    }
    eprintln!(
        "LLCONTROL_PIPEWIRE_RESULT frames={} worker={child}",
        file.frames
    );
    drop(file);
    assert!(!staging.exists());

    // A dead remote is not an unchanged desktop. It must fail, reap and discard
    // the unfinished MP4 instead of reporting a healthy frozen recording.
    let fd = UnixStream::connect(root.join("llcontrol-private"))
        .unwrap()
        .into();
    let recorder = Recorder::open(
        Source::Portal {
            node,
            serial: Some(serial),
        },
        Some(fd),
        true,
        None,
        None,
    )
    .unwrap();
    let child = recorder.worker.lock().unwrap().child.id();
    let staging = recorder.directory.as_ref().unwrap().path().to_owned();
    let deadline = Instant::now() + Duration::from_secs(5);
    while recorder.snapshot().is_err() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(IO_TICK);
    }
    let loss_phase = std::env::var("LLCONTROL_PRIVATE_LOSS_PHASE").unwrap();
    assert!(["setup", "recording", "paused"].contains(&loss_phase.as_str()));
    if loss_phase != "setup" {
        recorder.resume().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.status().frames < 6 {
            assert!(Instant::now() < deadline, "{:?}", recorder.status());
            std::thread::sleep(IO_TICK);
        }
        if loss_phase == "paused" {
            recorder.pause().unwrap();
        }
    }
    let expected = match loss_phase.as_str() {
        "setup" => Phase::Setup,
        "paused" => Phase::Paused,
        _ => Phase::Recording,
    };
    assert_eq!(recorder.status().phase, expected);
    let disconnect_started = Instant::now();
    std::fs::write(root.join("disconnect-server"), "disconnect").unwrap();
    let deadline = Instant::now() + Duration::from_secs(7);
    loop {
        let status = recorder.status();
        if let Phase::Failed(message) = &status.phase {
            assert!(message.contains("PipeWire connection closed"), "{message}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "dead PipeWire remote: {status:?}"
        );
        std::thread::sleep(IO_TICK);
    }
    assert!(recorder.finish().is_err());
    assert!(!PathBuf::from(format!("/proc/{child}")).exists());
    assert!(!staging.exists());
    eprintln!(
        "LLCONTROL_PIPEWIRE_DISCONNECT_RESULT worker={child} phase={loss_phase} reaped=true staging_removed=true millis={}", disconnect_started.elapsed().as_millis()
    );
}
