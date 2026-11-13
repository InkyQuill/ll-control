use super::*;
use crate::theme_runtime::ThemeRuntime;
use llcontrol_image::theme::ThemeAssets;
use std::sync::Arc;
use tokio::sync::watch;

fn display(hid: mpsc::Sender<HidCommand>, settings: mpsc::Sender<SettingsUpdate>) -> Display {
    Display {
        cleanup_errors: Vec::new(),
        intent: crate::actor::group_barrier::RuntimeRevision::new(),
        shutdown: tokio::sync::watch::channel(false).1,
        reset_recovery_required: false,
        reset_retry: None,
        source_saved: true,
        media_key: None,
        media_pin: None,
        preparation: 0,
        hid,
        settings,
        state: LcdState {
            persistence: None,
            requested: LcdConfig::new("LCDSetting", 80, 0),
            applied: Some(LcdConfig::new("LCDSetting", 80, 0)),
            source: LcdSourceKind::Theme,
            generation: 1,
            last_error: None,
        },
        source: Source::Idle,
        stats: Default::default(),
        started: None,
        deadline: Instant::now(),
    }
}

#[tokio::test]
async fn direct_media_rotation_config_failure_or_shutdown_keeps_old_position_and_no_new_angle_frame(
) {
    for shutdown_during_ack in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let store = crate::media_assets::MediaAssetStore::new(root.path().into());
        let source = store
            .acquire_source(llcontrol_ipc::BUILTIN_VIDEO_KEY)
            .unwrap();
        let pin = source.pin();
        let mut direct = MediaReader::spawn(source);
        for _ in 0..3 {
            direct.next_unit().await.unwrap();
        }
        let (hid, mut requests) = mpsc::channel(8);
        let (settings, mut updates) = mpsc::channel(8);
        let mut d = display(hid, settings);
        let (stop, shutdown) = watch::channel(false);
        d.shutdown = shutdown;
        d.source = Source::Media(direct);
        d.state.source = LcdSourceKind::DiagnosticClip;
        d.state.requested.mode = "Application".into();
        d.media_key = Some(llcontrol_ipc::BUILTIN_VIDEO_KEY.into());
        d.media_pin = Some(pin);
        d.stats.sent_frames = 3;
        let generation = d.state.generation;
        let previous = d.state.requested.clone();
        let io = tokio::spawn(async move {
            let Some(HidCommand::Write { reply, .. }) = requests.recv().await else {
                panic!("combined brightness patch must configure HID after preparation");
            };
            if shutdown_during_ack {
                stop.send(true).unwrap();
                reply.send(Ok(())).unwrap();
            } else {
                reply
                    .send(Err(llcontrol_hid::Error::BlockingTaskFailed(
                        "injected brightness failure".into(),
                    )))
                    .unwrap();
            }
            requests
        });
        let error = tokio::time::timeout(
            Duration::from_secs(5),
            d.patch(LcdConfigPatch {
                mode: None,
                brightness_percent: Some(85),
                rotation_degrees: Some(90),
            }),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error.to_string().contains(if shutdown_during_ack {
            "shutting down"
        } else {
            "brightness failure"
        }));
        assert_eq!(d.state.requested, previous);
        assert_eq!(d.state.applied.as_ref().unwrap().rotation_degrees, 0);
        assert_eq!(d.state.generation, generation);
        assert_eq!(d.stats.sent_frames, 3);
        let Source::Media(reader) = &mut d.source else {
            panic!("old direct source must be recoverable");
        };
        assert_eq!(reader.next_index(), 3);
        let next = reader.next_unit().await.unwrap();
        let mut expected = store
            .acquire_source(llcontrol_ipc::BUILTIN_VIDEO_KEY)
            .unwrap();
        assert_eq!(next.bytes, expected.read_unit(3).unwrap().bytes);
        assert!(
            updates.try_recv().is_err(),
            "failed transition must not persist new angle"
        );
        let mut requests = io.await.unwrap();
        assert!(
            requests.try_recv().is_err(),
            "failed transition must not send new-angle AU"
        );
        d.stop().await.unwrap();
    }
}

#[tokio::test]
async fn direct_media_rotation_decoder_failure_resumes_original_stream_without_config_or_settings()
{
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("oversize.h264");
    let input = path.clone();
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=1024x1024:r=24",
                "-frames:v",
                "1",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-profile:v",
                "baseline",
                "-pix_fmt",
                "yuv420p",
                "-threads",
                "1",
            ])
            .arg(input)
            .output()
            .unwrap();
        assert!(output.status.success());
    })
    .await
    .unwrap();
    let bytes = std::fs::read(path).unwrap();
    let store = crate::media_assets::MediaAssetStore::new(root.path().join("media"));
    let info = store
        .import(
            "Oversized input".into(),
            H264Clip {
                fps: 24,
                units: vec![llcontrol_ipc::H264AccessUnit {
                    bytes: bytes.clone(),
                    keyframe: true,
                }],
            },
        )
        .unwrap();
    let source = store.acquire_source(&info.key).unwrap();
    let (hid, mut requests) = mpsc::channel(8);
    let (settings, mut updates) = mpsc::channel(8);
    let mut d = display(hid, settings);
    let (_stop, shutdown) = watch::channel(false);
    d.shutdown = shutdown;
    d.source = Source::Media(MediaReader::spawn(source));
    d.media_key = Some(info.key);
    d.state.source = LcdSourceKind::DiagnosticClip;
    d.state.requested.mode = "Application".into();
    let previous = d.state.requested.clone();
    let error = tokio::time::timeout(
        Duration::from_secs(3),
        d.patch(LcdConfigPatch {
            mode: None,
            brightness_percent: None,
            rotation_degrees: Some(90),
        }),
    )
    .await
    .expect("decoder limit should fail promptly")
    .unwrap_err();
    assert!(!error.to_string().contains("timed out"), "{error}");
    assert_eq!(d.state.requested, previous);
    assert!(requests.try_recv().is_err());
    assert!(updates.try_recv().is_err());
    let Source::Media(reader) = &mut d.source else {
        panic!("direct reader lost on decoder error");
    };
    assert_eq!(reader.next_index(), 0);
    assert_eq!(reader.next_unit().await.unwrap().bytes, bytes);
    d.stop().await.unwrap();
}

#[tokio::test]
async fn failed_media_reader_join_is_retained_after_source_becomes_idle() {
    let (hid, _requests) = mpsc::channel(8);
    let (settings, _updates) = mpsc::channel(8);
    let mut d = display(hid, settings);
    let mut reader = crate::media_reader::MediaReader::spawn_with(24, 1, |_| {
        panic!("synthetic reader cleanup panic")
    });
    assert!(reader.next_unit().await.is_err());
    d.source = Source::Media(reader);
    assert!(d.stop().await.is_err());
    assert!(matches!(d.source, Source::Idle));
    assert!(d.stop().await.is_ok());
    assert!(d
        .cleanup_errors
        .iter()
        .any(|e| e.contains("synthetic reader cleanup panic")));
}

#[tokio::test]
async fn queued_codec_join_failures_veto_actor_tree_quiesce_instead_of_being_only_logged() {
    use crate::{
        actor::{
            self,
            keeper_lifecycle::{QuiesceError, State},
        },
        config::Config,
        persistence::PersistenceStore,
        stats::Stats,
    };
    use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let initial = store.read("test").unwrap();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        "/mock/cleanup".into(),
    );
    let backend = MockBackend::new();
    backend.add_device(info.clone());
    // Current-thread runtime: enqueue owned workers before actor tasks poll.
    // Already-stopped shutdown guarantees queued draining, never LCD playback.
    let (_root_stop, shutdown) = watch::channel(true);
    let handles = actor::spawn_device_actor_tree(
        Arc::new(backend.clone()),
        info.clone(),
        &Config::default(),
        store,
        "test".into(),
        initial,
        Arc::new(Stats::new()),
        shutdown,
    )
    .unwrap();
    let theme = serde_json::from_value(
        serde_json::json!({"version":1,"name":"queued cleanup","background":[0,0,0],"layers":[]}),
    )
    .unwrap();
    let runtime = Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![]).unwrap());
    let mut replies = vec![];
    for media in [false, true] {
        let prepared = crate::media_composition::PreparedComposition {
            worker: crate::media_composition::MediaComposition::panicked_supervisor_for_test(),
            first: llcontrol_ipc::H264AccessUnit {
                bytes: vec![0, 0, 0, 1, 0x65, 0x80],
                keyframe: true,
            },
            runtime: runtime.clone(),
            pin: None,
        };
        let (reply, result) = oneshot::channel();
        replies.push(result);
        let command = if media {
            LcdCommand::SetMediaComposition {
                key: "synthetic".into(),
                prepared,
                preparation: None,
                reply,
            }
        } else {
            LcdCommand::SetComposition {
                prepared,
                preparation: None,
                reply,
            }
        };
        handles.lcd_tx.try_send(command).unwrap();
    }
    assert!(matches!(
        handles.quiesce_checked().await,
        Err(QuiesceError::Cleanup(_))
    ));
    for result in replies {
        assert!(result.await.unwrap().is_err());
    }
    let State::Failed(errors) = handles.keepers.state() else {
        panic!("failed codec joins must veto quiescence");
    };
    assert_eq!(
        errors
            .iter()
            .filter(|e| e.contains("synthetic codec cleanup panic"))
            .count(),
        2
    );
    assert!(matches!(
        handles.quiesce_checked().await,
        Err(QuiesceError::Cleanup(_))
    ));
    let mut teardown = handles.teardown_rx.clone();
    while !*teardown.borrow() {
        teardown.changed().await.unwrap();
    }
    assert!(
        backend.write_history(&info).is_empty(),
        "cleanup failure must never start HID work"
    );
}
