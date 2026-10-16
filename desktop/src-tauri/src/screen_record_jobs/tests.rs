use super::*;
fn id(c: char) -> String {
    c.to_string().repeat(32)
}
async fn wait(owner: &Recordings, phase: Phase) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let s = owner.status().unwrap().unwrap();
            assert_ne!(s.phase, Phase::Failed, "{s:?}");
            if s.phase == phase {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
fn synthetic(owner: &Recordings, key: &str) {
    owner
        .start_with(key, |cancel| async move {
            Ok(Some(Capture {
                recorder: Some(Recorder::synthetic_with_cancel(cancel)?),
                session: None,
            }))
        })
        .unwrap();
}
#[tokio::test]
async fn lost_admission_reply_and_reserved_cancel_never_open_capture() {
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    assert_eq!(owner.begin(id('a')).unwrap().phase, Phase::Reserved);
    assert!(owner.begin(id('b')).is_err());
    owner.cancel(&id('a')).await.unwrap();
    assert!(owner.start(&id('a')).is_err());
    owner.begin(id('b')).unwrap();
    owner.shutdown().await.unwrap();
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Cancelled);
    assert!(owner.begin(id('c')).is_err());
}
#[tokio::test]
async fn cancel_joins_selecting_worker_and_retains_admission_until_return() {
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    let (release, released) = oneshot::channel();
    owner
        .start_with(&id('a'), |mut cancel| async move {
            cancel.wait_for(|v| *v).await.unwrap();
            released.await.unwrap();
            Ok(None)
        })
        .unwrap();
    let cancelling = owner.clone();
    let task = tokio::spawn(async move { cancelling.cancel(&id('a')).await });
    wait(&owner, Phase::Cancelling).await;
    assert!(!task.is_finished());
    assert!(owner.begin(id('b')).is_err());
    release.send(()).unwrap();
    task.await.unwrap().unwrap();
    owner.begin(id('b')).unwrap();
    owner.shutdown().await.unwrap();
}
#[tokio::test]
async fn recording_controls_finalize_once_and_shutdown_removes_file() {
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    owner.start(&id('a')).unwrap(); // lost start reply is idempotent
    owner.command(&id('a'), Action::Pause).await.unwrap();
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Paused);
    owner.command(&id('a'), Action::Resume).await.unwrap();
    owner.command(&id('a'), Action::Finish).await.unwrap();
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Ready);
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let path = owner
        .lock()
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .file
        .as_ref()
        .unwrap()
        .path();
    assert!(path.exists());
    assert!(owner.command(&id('b'), Action::Pause).await.is_err());
    owner.shutdown().await.unwrap();
    assert!(!path.exists());
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Cancelled);
}
#[tokio::test]
async fn dropped_control_reply_does_not_abandon_recording_owner() {
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    let (reply, response) = oneshot::channel();
    drop(response);
    owner
        .lock()
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .commands
        .send(Command::Control {
            action: Action::Finish,
            reply,
        })
        .unwrap();
    wait(&owner, Phase::Ready).await;
    owner.cancel(&id('a')).await.unwrap();
}

#[tokio::test]
async fn failed_open_is_readable_and_releases_admission_after_join() {
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    owner
        .start_with(&id('a'), |_| async { bail!("fixture portal unavailable") })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if owner.status().unwrap().unwrap().phase == Phase::Failed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    owner.cancel(&id('a')).await.unwrap();
    assert!(owner
        .status()
        .unwrap()
        .unwrap()
        .message
        .unwrap()
        .contains("fixture portal unavailable"));
    owner.begin(id('b')).unwrap();
    owner.shutdown().await.unwrap();
}
#[test]
fn ipc_status_contains_no_native_paths_and_actions_are_closed() {
    let status = Status {
        region: None,
        id: id('a'),
        phase: Phase::LimitReached,
        recorded_millis: 180000,
        frames: 4320,
        message: None,
        metadata: None,
    };
    let value = serde_json::to_value(status).unwrap();
    assert_eq!(value["phase"], "limit_reached");
    assert_eq!(value["recordedMillis"], 180000);
    assert!(value.get("path").is_none());
    assert!(serde_json::from_str::<Action>("\"arbitrary_native_command\"").is_err());
}

#[tokio::test]
async fn admitted_recording_survives_capture_cancel_and_requires_explicit_review_save() {
    use crate::{
        media_import::{EditOptions, MediaImports, Phase as ImportPhase},
        media_prepare::Fit,
    };
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    tokio::time::sleep(Duration::from_millis(250)).await;
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let imports = MediaImports::default();
    let recording = owner.clone();
    let input = imports.clone();
    let (path, frames) = tokio::task::spawn_blocking(move || {
        let file = recording.lease(&id('a')).unwrap();
        let path = file.path();
        let frames = file.frames;
        let status = input
            .begin_recording(
                &recording,
                &id('a'),
                id('b'),
                "screen.mp4".into(),
                EditOptions {
                    range: None,
                    fit: Fit::Contain,
                    cut: None,
                    adjustments: Default::default(),
                },
            )
            .unwrap();
        assert_eq!(status.phase, ImportPhase::Uploading);
        assert_eq!(status.received, status.total);
        assert!(input
            .begin_recording(
                &recording,
                &id('a'),
                id('c'),
                "duplicate.mp4".into(),
                EditOptions {
                    range: None,
                    fit: Fit::Contain,
                    cut: None,
                    adjustments: Default::default(),
                }
            )
            .is_err());
        (path, frames)
    })
    .await
    .unwrap();
    owner.cancel(&id('a')).await.unwrap();
    assert!(owner.lease(&id('a')).is_err());
    assert!(path.exists(), "admitted input must retain the recording");
    imports.prepare_review(&id('b')).await.unwrap();
    let status = imports.status().unwrap().unwrap();
    assert_eq!(status.phase, ImportPhase::ReviewReady);
    assert!(
        status.key.is_none(),
        "review must not publish a daemon asset"
    );
    assert!(
        !path.exists(),
        "preparation released the last recording lease"
    );
    tokio::task::spawn_blocking(move || {
        assert!(imports.preview_length(&id('b')).unwrap() > 0);
        let reviewed = imports.take_review(&id('b')).unwrap();
        assert_eq!(reviewed.file.frames(), frames as usize);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelled_admission_rejects_late_prepare_without_starting_encoder() {
    use crate::{
        media_import::{EditOptions, MediaImports, Phase as ImportPhase},
        media_prepare::Fit,
    };
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let path = owner.lease(&id('a')).unwrap().path();
    let imports = MediaImports::default();
    // Same synchronous admission used by the Tauri handler; no delayed task
    // can create this ID after the cancellation acknowledgement.
    imports
        .begin_recording(
            &owner,
            &id('a'),
            id('b'),
            "screen.mp4".into(),
            EditOptions {
                range: None,
                fit: Fit::Contain,
                cut: None,
                adjustments: Default::default(),
            },
        )
        .unwrap();
    imports.cancel(&id('b')).unwrap();
    owner.cancel(&id('a')).await.unwrap();
    assert!(!path.exists());
    assert!(imports.prepare_review(&id('b')).await.is_err());
    let status = imports.status().unwrap().unwrap();
    assert_eq!(status.phase, ImportPhase::Cancelled);
    assert!(status.key.is_none());
}

#[tokio::test]
async fn recording_http_preview_serves_exact_ranges_and_revokes_after_cancel() {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };
    async fn request(url: &str, range: &str) -> Vec<u8> {
        let uri: tauri::http::Uri = url.parse().unwrap();
        let authority = uri.authority().unwrap().as_str();
        let mut socket = TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(
                format!(
                    "GET {} HTTP/1.1\r\nHost: {authority}\r\nRange: {range}\r\n\r\n",
                    uri.path()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await.unwrap();
        bytes
    }
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    assert!(owner.preview_length(&id('a')).is_err());
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let status = owner.status().unwrap().unwrap();
    let metadata = status.metadata.unwrap();
    assert!(metadata.width > 0 && metadata.height > 0);
    assert!((metadata.duration_seconds.unwrap() - status.frames as f64 / 24.0).abs() < 0.05);
    assert!(owner
        .preview_range(&id('a'), 0, crate::media_import::MAX_CHUNK + 1)
        .is_err());
    assert!(owner.preview_range(&id('a'), u64::MAX, 1).is_err());
    let expected = owner.preview_range(&id('a'), 4, 28).unwrap();
    let server = crate::media_http::MediaHttp::default();
    let url = server
        .url(
            "recording",
            &id('a'),
            Default::default(),
            Default::default(),
            owner.clone(),
        )
        .await
        .unwrap();
    let response = request(&url, "bytes=4-31").await;
    let boundary = response.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
    assert!(response.starts_with(b"HTTP/1.1 206"));
    assert_eq!(&response[boundary..], expected);
    owner.cancel(&id('a')).await.unwrap();
    assert!(owner.status().unwrap().unwrap().metadata.is_none());
    assert!(request(&url, "bytes=4-31")
        .await
        .starts_with(b"HTTP/1.1 404"));
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn recording_cut_review_decodes_ordered_crops_after_capture_cancellation() {
    use crate::{
        media_import::{EditOptions, MediaImports, Phase as ImportPhase, MAX_CHUNK},
        media_prepare::{Fit, Segment, VideoCrop, VideoCut},
    };
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    tokio::time::timeout(Duration::from_secs(8), async {
        while owner.status().unwrap().unwrap().frames < 25 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let original = owner.lease(&id('a')).unwrap().path();
    let imports = MediaImports::default();
    // The default SMPTE pattern has solid red/blue bars in its top portion.
    // Reverse temporal order, selecting a different spatial region per interval.
    imports
        .begin_recording(
            &owner,
            &id('a'),
            id('b'),
            "recorded-crops.mp4".into(),
            EditOptions {
                range: None,
                fit: Fit::Stretch,
                cut: Some(VideoCut {
                    segments: vec![
                        Segment {
                            start_ms: 500,
                            end_ms: 1000,
                            crop: Some(VideoCrop {
                                x: 0.75,
                                y: 0.2,
                                width: 0.06,
                                height: 0.05,
                            }),
                        },
                        Segment {
                            start_ms: 0,
                            end_ms: 500,
                            crop: Some(VideoCrop {
                                x: 0.9,
                                y: 0.2,
                                width: 0.06,
                                height: 0.05,
                            }),
                        },
                    ],
                }),
                adjustments: Default::default(),
            },
        )
        .unwrap();
    owner.cancel(&id('a')).await.unwrap();
    assert!(
        original.exists(),
        "admitted source must survive capture cancel"
    );
    imports.prepare_review(&id('b')).await.unwrap();
    assert!(
        !original.exists(),
        "prepared review releases the source lease"
    );
    let status = imports.status().unwrap().unwrap();
    assert_eq!(status.phase, ImportPhase::ReviewReady);
    assert!(status.key.is_none(), "preparation must not publish");
    let preview = imports.clone();
    tokio::task::spawn_blocking(move || {
        let length = preview.preview_length(&id('b')).unwrap();
        let mut bytes = Vec::new();
        while (bytes.len() as u64) < length {
            let chunk = preview
                .preview_range(&id('b'), bytes.len() as u64, MAX_CHUNK)
                .unwrap();
            assert!(!chunk.is_empty());
            bytes.extend(chunk);
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("review.mp4");
        std::fs::write(&path, bytes).unwrap();
        let decoded = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-threads", "1", "-i"])
            .arg(path)
            .args([
                "-vf",
                "scale=1:1",
                "-pix_fmt",
                "rgb24",
                "-threads",
                "1",
                "-f",
                "rawvideo",
                "-",
            ])
            .output()
            .unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(
            decoded.stdout.len(),
            24 * 3,
            "two half-second intervals at24fps"
        );
        for (frame, rgb) in decoded.stdout.chunks_exact(3).enumerate() {
            let (strong, weak) = if frame < 12 { (0, 2) } else { (2, 0) };
            assert!(
                rgb[strong] > 200 && rgb[weak] < 35 && rgb[1] < 35,
                "frame{frame}: {rgb:?}"
            );
        }
    })
    .await
    .unwrap();
    imports.cancel(&id('b')).unwrap();
    assert!(imports.preview_length(&id('b')).is_err());
}

#[tokio::test]
async fn recording_cut_selects_source_frames_in_reversed_temporal_order() {
    use crate::{
        media_import::{EditOptions, MediaImports, MAX_CHUNK},
        media_prepare::{Fit, Segment, VideoCrop, VideoCut},
    };
    fn decode(path: &std::path::Path, filter: &str) -> Vec<u8> {
        let output = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-threads", "1", "-i"])
            .arg(path)
            .args([
                "-vf", filter, "-pix_fmt", "rgb24", "-threads", "1", "-f", "rawvideo", "-",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    let owner = Recordings::default();
    owner.begin(id('a')).unwrap();
    synthetic(&owner, &id('a'));
    wait(&owner, Phase::Recording).await;
    tokio::time::timeout(Duration::from_secs(8), async {
        while owner.status().unwrap().unwrap().frames < 25 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    owner.command(&id('a'), Action::Finish).await.unwrap();
    let original = owner.lease(&id('a')).unwrap();
    // SMPTE's lower-right noise changes every frame. Derive an independent
    // oracle from the completed source, without applying trim/concat to it.
    let source = tokio::task::spawn_blocking(move || {
        decode(
            &original.path(),
            "format=rgb24,crop=iw/4:ih/4:iw*3/4:ih*3/4:exact=1,scale=480:480,scale=64:64",
        )
    })
    .await
    .unwrap();
    let imports = MediaImports::default();
    let crop = VideoCrop {
        x: 0.75,
        y: 0.75,
        width: 0.25,
        height: 0.25,
    };
    imports
        .begin_recording(
            &owner,
            &id('a'),
            id('b'),
            "temporal.mp4".into(),
            EditOptions {
                range: None,
                fit: Fit::Stretch,
                adjustments: Default::default(),
                cut: Some(VideoCut {
                    segments: vec![
                        Segment {
                            start_ms: 500,
                            end_ms: 1000,
                            crop: Some(crop),
                        },
                        Segment {
                            start_ms: 0,
                            end_ms: 500,
                            crop: Some(crop),
                        },
                    ],
                }),
            },
        )
        .unwrap();
    owner.cancel(&id('a')).await.unwrap();
    imports.prepare_review(&id('b')).await.unwrap();
    let preview = imports.clone();
    tokio::task::spawn_blocking(move || {
        const FRAME: usize = 64 * 64 * 3;
        assert!(source.len() >= 25 * FRAME);
        // The live output clock may legitimately repeat a source frame.
        // Retain a discriminating temporal oracle despite those duplicates.
        let distinct: std::collections::HashSet<_> = source.chunks_exact(FRAME).take(24).collect();
        assert!(
            distinct.len() >= 12,
            "Source does not distinguish temporal intervals"
        );
        let length = preview.preview_length(&id('b')).unwrap();
        let mut bytes = Vec::new();
        while (bytes.len() as u64) < length {
            let chunk = preview
                .preview_range(&id('b'), bytes.len() as u64, MAX_CHUNK)
                .unwrap();
            assert!(!chunk.is_empty());
            bytes.extend(chunk);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review.mp4");
        std::fs::write(&path, bytes).unwrap();
        let output = decode(&path, "scale=64:64");
        assert_eq!(output.len(), 24 * FRAME);
        for (index, frame) in output.chunks_exact(FRAME).enumerate() {
            // Nearest decoded source frame tolerates lossy re-encoding while
            // detecting wrong timestamps, reordered cuts and duplicated frames.
            let mut distances: Vec<(usize, u64)> = source
                .chunks_exact(FRAME)
                .take(24)
                .enumerate()
                .map(|(candidate, pixels)| {
                    (
                        candidate,
                        frame
                            .iter()
                            .zip(pixels)
                            .map(|(a, b)| u64::from(a.abs_diff(*b)))
                            .sum(),
                    )
                })
                .collect();
            distances.sort_by_key(|(_, distance)| *distance);
            let expected = if index < 12 { index + 12 } else { index - 12 };
            assert_eq!(
                &source[distances[0].0 * FRAME..(distances[0].0 + 1) * FRAME],
                &source[expected * FRAME..(expected + 1) * FRAME],
                "output frame {index}: closest position {} instead of {expected}",
                distances[0].0
            );
        }
    })
    .await
    .unwrap();
    imports.cancel(&id('b')).unwrap();
}

#[tokio::test]
async fn paused_region_command_has_readback_and_rejects_invalid_or_stale_updates() {
    let owner = Recordings::default();
    let key = id('c');
    owner.begin(key.clone()).unwrap();
    synthetic(&owner, &key);
    wait(&owner, Phase::Recording).await;
    tokio::time::timeout(Duration::from_secs(8), async {
        while owner.status().unwrap().unwrap().frames < 6 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let action: Action = serde_json::from_value(
        serde_json::json!({"set_region":{"x":0.5,"y":0.0,"width":0.5,"height":1.0}}),
    )
    .unwrap();
    assert!(owner.command(&key, action).await.is_err());
    assert!(owner.snapshot(&key).await.is_err());
    owner.command(&key, Action::Pause).await.unwrap();
    assert!(owner.snapshot(&id('d')).await.is_err());
    let jpeg = owner.snapshot(&key).await.unwrap();
    assert!(jpeg.starts_with(&[0xff, 0xd8]));

    assert!(owner.command(&id('d'), action).await.is_err());
    owner.command(&key, action).await.unwrap();
    let region = owner.status().unwrap().unwrap().region.unwrap();
    assert_eq!(region.x, 0.5);
    assert_eq!(region.width, 0.5);
    let invalid = Action::SetRegion(crate::media_prepare::VideoCrop {
        width: 1.0,
        ..region
    });
    assert!(owner.command(&key, invalid).await.is_err());
    assert_eq!(owner.status().unwrap().unwrap().region, Some(region));
    owner.command(&key, Action::Resume).await.unwrap();
    owner.command(&key, Action::Finish).await.unwrap();
    assert_eq!(owner.status().unwrap().unwrap().region, Some(region));
    assert!(owner.command(&key, action).await.is_err());
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Ready);
    assert!(owner.snapshot(&key).await.is_err());
    owner.shutdown().await.unwrap();
}

#[tokio::test]
async fn setup_survives_rejected_finish_and_starts_only_on_explicit_resume() {
    let owner = Recordings::default();
    let key = id('a');
    owner.begin(key.clone()).unwrap();
    owner
        .start_with(&key, |cancel| async move {
            Ok(Some(Capture {
                recorder: Some(Recorder::synthetic_prepare_with_cancel(cancel)?),
                session: None,
            }))
        })
        .unwrap();
    wait(&owner, Phase::Setup).await;
    assert!(owner.command(&key, Action::Finish).await.is_err());
    assert!(owner.command(&key, Action::Pause).await.is_err());
    let crop = crate::media_prepare::VideoCrop {
        x: 0.75,
        y: 0.2,
        width: 0.06,
        height: 0.05,
    };
    owner.command(&key, Action::SetRegion(crop)).await.unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while owner.snapshot(&key).await.is_err() {
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let status = owner.status().unwrap().unwrap();
    assert_eq!(status.phase, Phase::Setup);
    assert_eq!(status.frames, 0);
    assert_eq!(status.recorded_millis, 0);
    assert_eq!(status.region, Some(crop));
    owner.command(&key, Action::Resume).await.unwrap();
    wait(&owner, Phase::Recording).await;
    owner.cancel(&key).await.unwrap();
    assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Cancelled);
}

#[tokio::test]
async fn cancel_and_shutdown_join_stalled_process_before_releasing_capture() {
    for shutdown in [false, true] {
        let owner = Recordings::default();
        let key = id(if shutdown { 'd' } else { 'c' });
        owner.begin(key.clone()).unwrap();
        let (ready, worker) = oneshot::channel();
        owner
            .start_with(&key, |cancel| async move {
                let recorder = Recorder::stalled_for_test(cancel)?;
                let _ = ready.send(recorder.worker_for_test());
                Ok(Some(Capture {
                    recorder: Some(recorder),
                    session: None,
                }))
            })
            .unwrap();
        let (pid, staging) = worker.await.unwrap();
        // The first owner status request is now allowed to block in the child.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let start = std::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(2), async {
            if shutdown {
                owner.shutdown().await
            } else {
                owner.cancel(&key).await
            }
        })
        .await
        .expect("stalled native process must not retain recording owner")
        .unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(owner.status().unwrap().unwrap().phase, Phase::Cancelled);
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        assert!(!staging.exists());
        if !shutdown {
            let next = id('e');
            owner.begin(next.clone()).unwrap();
            synthetic(&owner, &next);
            wait(&owner, Phase::Recording).await;
            owner.cancel(&next).await.unwrap();
        }
    }
}
