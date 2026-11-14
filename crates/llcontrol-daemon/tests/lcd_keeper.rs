//! Host-side LCD keeper stream ownership, admission and lifecycle regressions.

use bytes::Bytes;
use llcontrol_daemon::actor::{hid_io, lcd_keeper, LcdCommand};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};

fn jpeg(byte: u8) -> Bytes {
    Bytes::from_static(if byte & 1 == 0 {
        include_bytes!("fixtures/lcd-480.jpg")
    } else {
        include_bytes!("fixtures/lcd-480-alternate.jpg")
    })
}

fn lcdv2_info() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        None,
        PathBuf::from("/mock/hidraw0"),
    )
}

struct Harness {
    tx: mpsc::Sender<LcdCommand>,
    hid: mpsc::Receiver<llcontrol_daemon::actor::HidCommand>,
    settings: mpsc::Receiver<llcontrol_daemon::actor::command_dispatch::SettingsUpdate>,
    _shutdown: watch::Sender<bool>,
}
impl Harness {
    fn new() -> Self {
        Self::with_settings(llcontrol_daemon::persistence::LcdSection {
            brightness: 73,
            rotation: "0".into(),
            control_mode: "LocalUI".into(),
        })
    }
    fn with_settings(initial: llcontrol_daemon::persistence::LcdSection) -> Self {
        let (hid_tx, hid) = mpsc::channel(16);
        let (tx, rx) = mpsc::channel(8);
        let (settings_tx, settings) = mpsc::channel(16);
        let (shutdown, sd) = watch::channel(false);
        lcd_keeper::spawn(hid_tx, 15000, initial, settings_tx, rx, sd);
        Self {
            tx,
            hid,
            settings,
            _shutdown: shutdown,
        }
    }
    async fn state(&self) -> llcontrol_ipc::LcdState {
        let (reply, rx) = oneshot::channel();
        self.tx.send(LcdCommand::State { reply }).await.unwrap();
        rx.await.unwrap()
    }
    async fn stats(&self) -> llcontrol_ipc::LcdPlaybackStats {
        let (reply, rx) = oneshot::channel();
        self.tx.send(LcdCommand::Stats { reply }).await.unwrap();
        rx.await.unwrap()
    }
    async fn image(
        &self,
        byte: u8,
    ) -> oneshot::Receiver<llcontrol_ipc::Result<llcontrol_ipc::LcdState>> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LcdCommand::SetImage {
                jpeg: jpeg(byte),
                reply,
            })
            .await
            .unwrap();
        rx
    }
    async fn play(
        &self,
        loops: u16,
    ) -> oneshot::Receiver<llcontrol_ipc::Result<llcontrol_ipc::LcdState>> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LcdCommand::PlayClip {
                clip: clip(),
                loops,
                reply,
            })
            .await
            .unwrap();
        rx
    }
    async fn firmware(&mut self, version: &str) {
        use llcontrol_daemon::actor::HidCommand;
        let HidCommand::Firmware { reply } = self.hid.recv().await.unwrap() else {
            panic!("expected one classified firmware query")
        };
        let token = format!("N9,01,HS,SQ,CA_II-Vision,V2.01.041,{version}");
        let packet = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
            134,
            token.as_bytes(),
        );
        reply
            .send(Ok(llcontrol_daemon::actor::FirmwareResponses {
                version: *packet.as_bytes(),
                datetime: None,
            }))
            .unwrap();
    }
    async fn setup(&mut self) {
        let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } =
            self.hid.recv().await.unwrap()
        else {
            panic!("expected setup")
        };
        assert_eq!(bytes[1], 12);
        assert_eq!(&bytes[11..19], &[4, 73, 0, 0, 0, 0, 0, 24]);
        reply.send(Ok(())).unwrap();
    }
    async fn frame(
        &mut self,
        diagnostic: bool,
    ) -> (
        Vec<Vec<u8>>,
        oneshot::Sender<llcontrol_hid::Result<llcontrol_daemon::actor::FrameOutcome>>,
    ) {
        let command = loop {
            let command = self.hid.recv().await.unwrap();
            match command {
                llcontrol_daemon::actor::HidCommand::Firmware { reply } if !diagnostic => {
                    let packet =
                        llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                            134,
                            b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
                        );
                    reply
                        .send(Ok(llcontrol_daemon::actor::FirmwareResponses {
                            version: *packet.as_bytes(),
                            datetime: None,
                        }))
                        .unwrap();
                }
                llcontrol_daemon::actor::HidCommand::Write { bytes, reply } if !diagnostic => {
                    assert_eq!(bytes[1], 12);
                    assert_eq!(bytes[11], 4);
                    assert!(bytes[12] <= 100);
                    assert_eq!(&bytes[13..19], &[0, 0, 0, 0, 0, 24]);
                    reply.send(Ok(())).unwrap();
                }
                command => break command,
            }
        };
        let llcontrol_daemon::actor::HidCommand::FrameBatch {
            packets,
            read_policy,
            reply,
        } = command
        else {
            panic!("expected indivisible frame")
        };
        {
            assert!(packets
                .iter()
                .all(|p| p[0] == 3 && p[1] == 13 && p.len() == 512));
            assert_eq!(
                read_policy,
                llcontrol_daemon::actor::FrameReadPolicy::ObserveOnce512 { timeout_ms: 20 }
            );
        }
        (packets, reply)
    }
}
fn complete(reply: oneshot::Sender<llcontrol_hid::Result<llcontrol_daemon::actor::FrameOutcome>>) {
    reply
        .send(Ok(llcontrol_daemon::actor::FrameOutcome {
            written_reports: 1,
            readback: vec![],
            elapsed: Duration::from_millis(1),
        }))
        .unwrap();
}
fn clip() -> llcontrol_ipc::H264Clip {
    llcontrol_ipc::H264Clip {
        fps: 24,
        units: vec![
            llcontrol_ipc::H264AccessUnit {
                bytes: vec![0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                keyframe: true,
            },
            llcontrol_ipc::H264AccessUnit {
                bytes: vec![0, 0, 1, 0x41, 2],
                keyframe: false,
            },
        ],
    }
}
#[tokio::test]
async fn clear_is_a_barrier_after_inflight_frame_and_preserves_brightness() {
    let mut h = Harness::new();
    let image = h.image(42).await;
    let (_, gate) = h.frame(false).await;
    let (reply, mut clear) = oneshot::channel();
    h.tx.send(LcdCommand::Clear { reply }).await.unwrap();
    assert!(matches!(
        clear.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(h.hid.try_recv().is_err());
    complete(gate);
    image.await.unwrap().unwrap();
    let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } = h.hid.recv().await.unwrap()
    else {
        panic!()
    };
    assert_eq!(bytes[1], 12);
    assert_eq!(&bytes[11..14], &[0, 73, 0]);
    reply.send(Ok(())).unwrap();
    assert_eq!(
        clear.await.unwrap().unwrap().source,
        llcontrol_ipc::LcdSourceKind::Idle
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    h.state().await;
    assert!(h.hid.try_recv().is_err());
    assert!(matches!(
        h.settings.recv().await,
        Some(llcontrol_daemon::actor::command_dispatch::SettingsUpdate::Lcd(_))
    ));
}
#[tokio::test]
async fn invalid_patch_and_image_preserve_source_without_io() {
    let mut h = Harness::new();
    let image = h.image(1).await;
    complete(h.frame(false).await.1);
    image.await.unwrap().unwrap();
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: llcontrol_ipc::LcdConfigPatch {
            mode: None,
            brightness_percent: Some(101),
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::SetImage {
        jpeg: Bytes::new(),
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    assert_eq!(h.state().await.source, llcontrol_ipc::LcdSourceKind::Jpeg);
    assert!(h.hid.try_recv().is_err());
}
#[tokio::test]
async fn failed_patch_retains_applied_and_does_not_persist_failure() {
    let mut h = Harness::new();
    for fail in [false, true] {
        let (reply, rx) = oneshot::channel();
        h.tx.send(LcdCommand::Patch {
            patch: llcontrol_ipc::LcdConfigPatch {
                mode: None,
                brightness_percent: Some(if fail { 40 } else { 60 }),
                rotation_degrees: None,
            },
            reply,
        })
        .await
        .unwrap();
        let llcontrol_daemon::actor::HidCommand::Write { reply, .. } = h.hid.recv().await.unwrap()
        else {
            panic!()
        };
        reply
            .send(if fail {
                Err(llcontrol_hid::Error::DeviceNotFound("test failure".into()))
            } else {
                Ok(())
            })
            .unwrap();
        assert_eq!(rx.await.unwrap().is_err(), fail);
    }
    let state = h.state().await;
    assert_eq!(state.requested.brightness_percent, 40);
    assert_eq!(state.applied.unwrap().brightness_percent, 60);
    assert!(state.last_error.is_some());
    h.settings.recv().await.unwrap();
    assert!(h.settings.try_recv().is_err());
}
#[tokio::test]
async fn clips_repeat_every_au_without_drops_and_freeze_stats_at_completion() {
    let mut h = Harness::new();
    let play = h.play(2).await;
    h.firmware("1.6").await;
    h.setup().await;
    let mut seen = Vec::new();
    let (packets, reply) = h.frame(true).await;
    seen.push(packets);
    complete(reply);
    play.await.unwrap().unwrap();
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(42)).await;
        let (packets, reply) = h.frame(true).await;
        seen.push(packets);
        complete(reply);
        h.state().await;
    }
    assert_eq!(seen[0], seen[2]);
    assert_eq!(seen[1], seen[3]);
    assert_ne!(seen[0], seen[1]);
    assert_eq!(h.state().await.source, llcontrol_ipc::LcdSourceKind::Idle);
    let stats = h.stats().await;
    assert_eq!(stats.sent_frames, 4);
    assert_eq!(stats.pending_frames, 0);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(h.stats().await, stats);
    assert!(h.hid.try_recv().is_err());
}
#[tokio::test]
async fn first_and_later_frame_failures_stop_without_retry() {
    for later in [false, true] {
        let mut h = Harness::new();
        let play = h.play(2).await;
        h.firmware("1.6").await;
        h.setup().await;
        let (_, reply) = h.frame(true).await;
        if later {
            complete(reply);
            play.await.unwrap().unwrap();
            tokio::time::sleep(Duration::from_millis(42)).await;
            h.frame(true)
                .await
                .1
                .send(Err(llcontrol_hid::Error::DeviceNotFound(
                    "test failure".into(),
                )))
                .unwrap();
        } else {
            reply
                .send(Err(llcontrol_hid::Error::DeviceNotFound(
                    "test failure".into(),
                )))
                .unwrap();
            assert!(play.await.unwrap().is_err());
        }
        assert!(h.state().await.last_error.is_some());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(h.hid.try_recv().is_err());
        let stats = h.stats().await;
        assert_eq!(stats.failed_transactions, 1);
        assert_eq!(stats.sent_frames, u64::from(later));
    }
}
#[tokio::test]
async fn unsupported_firmware_refuses_setup_and_preserves_source() {
    for version in ["1.4", "unknown"] {
        let mut h = Harness::new();
        let play = h.play(1).await;
        h.firmware(version).await;
        assert!(play.await.unwrap().is_err());
        assert!(h.hid.try_recv().is_err());
        assert_eq!(h.state().await.generation, 0);
    }
}

#[tokio::test]
async fn image_replacement_waits_for_old_batch_and_replies_after_new_batch() {
    let mut h = Harness::new();
    let first = h.image(1).await;
    let (a, gate) = h.frame(false).await;
    let mut second = h.image(2).await;
    assert!(h.hid.try_recv().is_err());
    complete(gate);
    first.await.unwrap().unwrap();
    let (b, gate) = h.frame(false).await;
    assert_ne!(a, b);
    assert!(matches!(
        second.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    complete(gate);
    assert_eq!(second.await.unwrap().unwrap().generation, 2);
    tokio::time::sleep(Duration::from_millis(42)).await;
    let (current, gate) = h.frame(false).await;
    assert!(current
        .iter()
        .all(|packet| packet[0] == 3 && packet[1] == 13));
    complete(gate);
}

#[tokio::test]
async fn clip_clear_and_jpeg_replace_cancel_remaining_gop_at_boundary() {
    for clear in [false, true] {
        let mut h = Harness::new();
        let play = h.play(2).await;
        h.firmware("1.6").await;
        h.setup().await;
        let (_, gate) = h.frame(true).await;
        let (reply, result) = oneshot::channel();
        h.tx.send(if clear {
            LcdCommand::Clear { reply }
        } else {
            LcdCommand::SetImage {
                jpeg: jpeg(42),
                reply,
            }
        })
        .await
        .unwrap();
        assert!(h.hid.try_recv().is_err());
        complete(gate);
        play.await.unwrap().unwrap();
        if clear {
            let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } =
                h.hid.recv().await.unwrap()
            else {
                panic!()
            };
            assert_eq!(bytes[1], 12);
            reply.send(Ok(())).unwrap();
        } else {
            let (packets, reply) = h.frame(false).await;
            assert_eq!(packets[0][1], 13);
            complete(reply);
        }
        result.await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        if clear {
            h.state().await;
            assert!(h.hid.try_recv().is_err());
            assert_eq!(h.stats().await.sent_frames, 1);
        } else {
            complete(h.frame(true).await.1);
            h.state().await;
            assert!(h.stats().await.sent_frames >= 2);
        }
    }
}

#[tokio::test]
async fn truncated_firmware_echo_cannot_reuse_stale_token_or_start_playback() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let wrong_echo = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
        133,
        b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
    );
    mock.queue_response(&lcdv2_info(), wrong_echo.as_bytes().to_vec());
    mock.queue_response(&lcdv2_info(), vec![1, 134]);
    let (_stop, shutdown) = watch::channel(false);
    let hid = hid_io::spawn(
        Arc::new(mock.clone()),
        lcdv2_info(),
        Arc::new(llcontrol_daemon::stats::Stats::new()),
        shutdown.clone(),
    )
    .unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (settings, _settings) = mpsc::channel(16);
    let _actor = lcd_keeper::spawn(hid, 15000, Default::default(), settings, rx, shutdown);
    let (reply, response) = oneshot::channel();
    tx.send(LcdCommand::PlayClip {
        clip: clip(),
        loops: 1,
        reply,
    })
    .await
    .unwrap();
    assert!(response.await.unwrap().is_err());
    assert_eq!(
        mock.write_history(&lcdv2_info())
            .iter()
            .map(|bytes| bytes[1])
            .collect::<Vec<_>>(),
        [134]
    );
}

#[tokio::test]
async fn diagnostic_applied_mode_tracks_actual_setup_and_brightness_patch() {
    let mut h = Harness::new();
    let play = h.play(2).await;
    h.firmware("1.6").await;
    h.setup().await;
    complete(h.frame(true).await.1);
    let state = play.await.unwrap().unwrap();
    assert_eq!(state.requested.mode, "LocalUI");
    assert_eq!(
        state.applied.unwrap(),
        llcontrol_ipc::LcdConfig::new("LCDSetting", 73, 0)
    );
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: llcontrol_ipc::LcdConfigPatch {
            mode: None,
            brightness_percent: Some(40),
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } = h.hid.recv().await.unwrap()
    else {
        panic!()
    };
    assert_eq!(&bytes[11..19], &[4, 40, 0, 0, 0, 0, 0, 24]);
    reply.send(Ok(())).unwrap();
    let state = rx.await.unwrap().unwrap();
    assert_eq!(
        state.applied.unwrap(),
        llcontrol_ipc::LcdConfig::new("LCDSetting", 40, 0)
    );
    assert_eq!(state.requested.mode, "LocalUI");
    let Some(llcontrol_daemon::actor::command_dispatch::SettingsUpdate::Lcd(saved)) =
        h.settings.recv().await
    else {
        panic!()
    };
    assert_eq!(saved.control_mode, "LocalUI");
    assert_eq!(saved.brightness, 40);
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: llcontrol_ipc::LcdConfigPatch {
            mode: Some("LCDSetting".into()),
            brightness_percent: None,
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
}

#[tokio::test]
async fn malformed_restore_uses_safe_seed_and_allows_image_then_clear() {
    for initial in [
        llcontrol_daemon::persistence::LcdSection {
            brightness: 101,
            rotation: "90".into(),
            control_mode: "LocalUI".into(),
        },
        llcontrol_daemon::persistence::LcdSection {
            brightness: 70,
            rotation: "broken".into(),
            control_mode: "LocalUI".into(),
        },
        llcontrol_daemon::persistence::LcdSection {
            brightness: 70,
            rotation: "0".into(),
            control_mode: "LCDSetting".into(),
        },
    ] {
        let mut h = Harness::with_settings(initial);
        let state = h.state().await;
        assert!(state.last_error.is_some());
        assert_eq!(
            state.requested,
            llcontrol_ipc::LcdConfig::new("LocalUI", 100, 0)
        );
        assert!(state.applied.is_none());
        let image = h.image(42).await;
        complete(h.frame(false).await.1);
        image.await.unwrap().unwrap();
        let (reply, rx) = oneshot::channel();
        h.tx.send(LcdCommand::Clear { reply }).await.unwrap();
        let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } =
            h.hid.recv().await.unwrap()
        else {
            panic!()
        };
        assert_eq!(&bytes[11..14], &[0, 100, 0]);
        reply.send(Ok(())).unwrap();
        assert_eq!(
            rx.await.unwrap().unwrap().source,
            llcontrol_ipc::LcdSourceKind::Idle
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        h.state().await;
        assert!(h.hid.try_recv().is_err());
    }
}

#[derive(Clone)]
struct FailingObservationBackend {
    history: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    fail_frame_report: Option<usize>,
}
impl HidBackend for FailingObservationBackend {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        Ok(vec![lcdv2_info()])
    }
    fn open(
        &self,
        _info: &DeviceInfo,
    ) -> llcontrol_hid::Result<Box<dyn llcontrol_hid::OpenHandle>> {
        Ok(Box::new(self.clone()))
    }
}
impl llcontrol_hid::OpenHandle for FailingObservationBackend {
    fn write(&self, bytes: &[u8]) -> llcontrol_hid::Result<()> {
        let mut history = self.history.lock().unwrap();
        let written = history.iter().filter(|bytes| bytes[1] == 13).count();
        if bytes[1] == 13 && self.fail_frame_report == Some(written) {
            return Err(llcontrol_hid::Error::DeviceNotFound(
                "injected partial write".into(),
            ));
        }
        history.push(bytes.to_vec());
        Ok(())
    }
    fn read_timeout(&self, buffer: &mut [u8], _timeout_ms: i32) -> llcontrol_hid::Result<usize> {
        if self
            .history
            .lock()
            .unwrap()
            .last()
            .is_some_and(|bytes| bytes[1] == 134)
        {
            let firmware = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                134,
                b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
            );
            buffer[..64].copy_from_slice(firmware.as_bytes());
            Ok(64)
        } else {
            Err(llcontrol_hid::Error::DeviceNotFound(
                "injected observation failure".into(),
            ))
        }
    }
}

#[tokio::test]
async fn completed_au_is_counted_when_read_fails_but_partial_au_is_not() {
    for partial in [false, true] {
        let backend = FailingObservationBackend {
            history: Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_frame_report: if partial { Some(1) } else { None },
        };
        let (_stop, shutdown) = watch::channel(false);
        let hid = hid_io::spawn(
            Arc::new(backend.clone()),
            lcdv2_info(),
            Arc::new(llcontrol_daemon::stats::Stats::new()),
            shutdown.clone(),
        )
        .unwrap();
        let (tx, rx) = mpsc::channel(8);
        let (settings, _settings) = mpsc::channel(16);
        let _actor = lcd_keeper::spawn(hid, 15000, Default::default(), settings, rx, shutdown);
        let mut clip = clip();
        clip.units[0].bytes.resize(1200, 42);
        let (reply, rx) = oneshot::channel();
        tx.send(LcdCommand::PlayClip {
            clip,
            loops: 2,
            reply,
        })
        .await
        .unwrap();
        let error = rx.await.unwrap().unwrap_err();
        assert!(error.to_string().contains(if partial {
            "after 1 written reports"
        } else {
            "after 3 written reports"
        }));
        let (reply, rx) = oneshot::channel();
        tx.send(LcdCommand::State { reply }).await.unwrap();
        let state = rx.await.unwrap();
        assert_eq!(state.source, llcontrol_ipc::LcdSourceKind::Idle);
        assert!(state.last_error.is_some());
        let (reply, rx) = oneshot::channel();
        tx.send(LcdCommand::Stats { reply }).await.unwrap();
        let stats = rx.await.unwrap();
        assert_eq!(stats.failed_transactions, 1);
        assert_eq!(stats.sent_frames, u64::from(!partial));
        assert_eq!(stats.sent_bytes, if partial { 0 } else { 1200 });
        let written = backend.history.lock().unwrap().len();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (reply, rx) = oneshot::channel();
        tx.send(LcdCommand::State { reply }).await.unwrap();
        rx.await.unwrap();
        assert_eq!(backend.history.lock().unwrap().len(), written);
    }
}

#[tokio::test]
async fn stale_prepared_media_cannot_replace_a_newer_image_or_persist_selection() {
    let mut h = Harness::new();
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::BeginThemePreparation { reply })
        .await
        .unwrap();
    let ticket = rx.await.unwrap();
    let image = h.image(42).await;
    complete(h.frame(false).await.1);
    let image_state = image.await.unwrap().unwrap();
    let (reply, rx) = oneshot::channel();
    let tmp = tempfile::tempdir().unwrap();
    let store = llcontrol_daemon::media_assets::MediaAssetStore::new(tmp.path().to_path_buf());
    let saved = store.import("Stale".into(), clip()).unwrap();
    let source = store.acquire_source(&saved.key).unwrap();
    h.tx.send(LcdCommand::SetMediaClip {
        key: saved.key.clone(),
        source,
        preparation: Some(ticket),
        reply,
    })
    .await
    .unwrap();
    assert!(rx
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    // The retained image continues streaming on its own timer.
    let current = h.state().await;
    assert_eq!(current.generation, image_state.generation);
    assert_eq!(current.source, llcontrol_ipc::LcdSourceKind::Jpeg);
    if let Ok(command) = h.hid.try_recv() {
        match command {
            llcontrol_daemon::actor::HidCommand::FrameBatch { reply, .. } => complete(reply),
            _ => panic!("unexpected HID command after stale selection"),
        }
    }
    assert!(h.settings.try_recv().is_err());
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::MediaClip { reply }).await.unwrap();
    assert_eq!(rx.await.unwrap(), None);
}

#[tokio::test]
async fn indexed_media_sends_all_586_frames_loops_and_clear_joins_before_local_ui() {
    let (tmp, store, saved, source, units) = tokio::task::spawn_blocking(|| {
        let tmp = tempfile::tempdir().unwrap();
        let store = llcontrol_daemon::media_assets::MediaAssetStore::new(tmp.path().into());
        let mut writer = store.create_file("Long video".into()).unwrap();
        let units: Vec<_> = (0..586)
            .map(|index| {
                let mut unit = clip().units[0].clone();
                unit.bytes
                    .extend_from_slice(format!("frame-{index}").as_bytes());
                writer.append(&unit).unwrap();
                unit
            })
            .collect();
        let saved = store.publish_file(writer).unwrap();
        let source = store.acquire_source(&saved.key).unwrap();
        (tmp, store, saved, source, units)
    })
    .await
    .unwrap();
    let mut h = Harness::new();
    let (reply, started) = oneshot::channel();
    h.tx.send(LcdCommand::SetMediaClip {
        key: saved.key.clone(),
        source,
        preparation: None,
        reply,
    })
    .await
    .unwrap();
    h.firmware("1.6").await;
    h.setup().await;
    // Drop the settings message's independent persistence pin. The actor alone
    // must retain the source until Clear has joined its reader.
    drop(h.settings.recv().await.unwrap());
    for index in 0..587 {
        let (packets, done) = h.frame(true).await;
        let expected: Vec<_> =
            llcontrol_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(
                &units[index % 586].bytes,
            )
            .unwrap()
            .into_iter()
            .map(|p| p.to_vec())
            .collect();
        assert_eq!(packets, expected, "complete frame {index}");
        complete(done);
        tokio::time::sleep(Duration::from_millis(42)).await;
    }
    started.await.unwrap().unwrap();
    let (_, gate) = h.frame(true).await;
    let (reply, mut cleared) = oneshot::channel();
    h.tx.send(LcdCommand::Clear { reply }).await.unwrap();
    tokio::task::yield_now().await;
    assert!(matches!(
        cleared.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
    complete(gate);
    let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } = h.hid.recv().await.unwrap()
    else {
        panic!("Clear must send LocalUI after old batch and reader finish")
    };
    assert_eq!(bytes[1], 12);
    assert_eq!(bytes[11], 0);
    reply.send(Ok(())).unwrap();
    assert_eq!(
        cleared.await.unwrap().unwrap().source,
        llcontrol_ipc::LcdSourceKind::Idle
    );
    while h.settings.try_recv().is_ok() {}
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    h.state().await;
    assert!(
        h.hid.try_recv().is_err(),
        "no old queued frames after Clear"
    );
    drop(tmp);
}

#[tokio::test]
async fn image_replacement_discards_prefetched_media_and_releases_its_pin() {
    let (tmp, store, saved, source) = tokio::task::spawn_blocking(|| {
        let tmp = tempfile::tempdir().unwrap();
        let store = llcontrol_daemon::media_assets::MediaAssetStore::new(tmp.path().into());
        let mut writer = store.create_file("Replace me".into()).unwrap();
        for unit in clip().units {
            writer.append(&unit).unwrap();
        }
        let saved = store.publish_file(writer).unwrap();
        let source = store.acquire_source(&saved.key).unwrap();
        (tmp, store, saved, source)
    })
    .await
    .unwrap();
    let mut h = Harness::new();
    let (reply, started) = oneshot::channel();
    h.tx.send(LcdCommand::SetMediaClip {
        key: saved.key.clone(),
        source,
        preparation: None,
        reply,
    })
    .await
    .unwrap();
    h.firmware("1.6").await;
    h.setup().await;
    drop(h.settings.recv().await.unwrap());
    let (_, gate) = h.frame(true).await;
    let mut replaced = h.image(42).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        replaced.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    complete(gate);
    let initial = started.await.unwrap().unwrap();
    let (packets, complete_image) = h.frame(false).await;
    assert!(packets.iter().all(|p| p[1] == 13));
    complete(complete_image);
    let image = replaced.await.unwrap().unwrap();
    assert_eq!(image.source, llcontrol_ipc::LcdSourceKind::Jpeg);
    assert!(image.generation > initial.generation);
    while h.settings.try_recv().is_ok() {}
    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    tokio::time::sleep(Duration::from_millis(42)).await;
    let (packets, refresh) = h.frame(false).await;
    assert!(packets.iter().all(|p| p[1] == 13));
    complete(refresh);
    h.state().await;
    assert!(h.hid.try_recv().is_err());
    drop(tmp);
}

async fn composition_fixture() -> (
    tempfile::TempDir,
    llcontrol_daemon::media_assets::MediaAssetStore,
    String,
    Arc<llcontrol_daemon::theme_runtime::ThemeRuntime>,
) {
    use llcontrol_daemon::{media_assets::MediaAssetStore, theme_runtime::ThemeRuntime};
    use llcontrol_image::theme::ThemeAssets;
    let (_root, store, key) = tokio::task::spawn_blocking(|| {
        let root = tempfile::tempdir().unwrap();
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=480x480:r=24",
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
                "-f",
                "h264",
                "-",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let store = MediaAssetStore::new(root.path().join("media"));
        let key = store
            .import(
                "Video".into(),
                llcontrol_ipc::H264Clip {
                    fps: 24,
                    units: vec![llcontrol_ipc::H264AccessUnit {
                        bytes: output.stdout,
                        keyframe: true,
                    }],
                },
            )
            .unwrap()
            .key;
        (root, store, key)
    })
    .await
    .unwrap();
    let theme: llcontrol_ipc::LcdTheme = serde_json::from_value(serde_json::json!({
        "version":2,"name":"Video","background":[0,0,0],"background_video":key,"layers":[]
    }))
    .unwrap();
    let runtime = Arc::new(ThemeRuntime::new(theme.clone(), ThemeAssets::new(), vec![]).unwrap());
    (_root, store, key, runtime)
}

#[tokio::test]
async fn composed_video_uses_frame_barriers_and_pins_pending_persistence() {
    use llcontrol_daemon::media_composition::PreparedComposition;
    let (_root, store, key, runtime) = composition_fixture().await;
    let theme = runtime.theme().clone();
    let prepared = PreparedComposition::prepare(store.clone(), runtime.clone())
        .await
        .unwrap();
    let mut h = Harness::new();
    let (reply, started) = oneshot::channel();
    h.tx.send(LcdCommand::SetComposition {
        prepared,
        preparation: None,
        reply,
    })
    .await
    .unwrap();
    h.firmware("1.6").await;
    h.setup().await;
    let pending_settings = h.settings.recv().await.unwrap();
    assert!(
        matches!(&pending_settings, llcontrol_daemon::actor::command_dispatch::SettingsUpdate::ComposedTheme { theme: saved, .. } if saved == &theme)
    );
    let (_, first) = h.frame(true).await;
    complete(first);
    assert_eq!(
        started.await.unwrap().unwrap().source,
        llcontrol_ipc::LcdSourceKind::Theme
    );
    let (reply, got) = oneshot::channel();
    h.tx.send(LcdCommand::Theme { reply }).await.unwrap();
    assert_eq!(got.await.unwrap(), Some(theme));
    let (_, gate) = h.frame(true).await;
    let (reply, mut cleared) = oneshot::channel();
    h.tx.send(LcdCommand::Clear { reply }).await.unwrap();
    tokio::task::yield_now().await;
    assert!(matches!(
        cleared.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    complete(gate);
    let llcontrol_daemon::actor::HidCommand::Write { bytes, reply } = h.hid.recv().await.unwrap()
    else {
        panic!("expected LocalUI after joined stop")
    };
    assert_eq!(bytes[11], 0);
    reply.send(Ok(())).unwrap();
    cleared.await.unwrap().unwrap();
    assert!(
        store.delete_unreferenced(&key, || Ok(false)).is_err(),
        "pending settings must retain pin"
    );
    drop(pending_settings);
    // A late preparation must be joined and discarded without any hardware writes.
    let prepared = PreparedComposition::prepare(store.clone(), runtime)
        .await
        .unwrap();
    let (reply, rejected) = oneshot::channel();
    h.tx.send(LcdCommand::SetComposition {
        prepared,
        preparation: Some(u64::MAX),
        reply,
    })
    .await
    .unwrap();
    assert!(rejected
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    store.delete_unreferenced(&key, || Ok(false)).unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(
        h.hid.try_recv().is_err(),
        "old frame after Clear/rejected Apply"
    );
}

#[tokio::test]
async fn shutdown_drains_queued_composition_after_inflight_hid_batch() {
    use llcontrol_daemon::{actor::HidCommand, media_composition::PreparedComposition};
    let (_root, store, key, runtime) = composition_fixture().await;
    let prepared = PreparedComposition::prepare(store.clone(), runtime)
        .await
        .unwrap();
    let (hid, mut requests) = mpsc::channel(8);
    let (commands, rx) = mpsc::channel(8);
    let (settings, _updates) = mpsc::channel(8);
    let (shutdown, sd) = watch::channel(false);
    let actor = lcd_keeper::spawn(hid, 15000, Default::default(), settings, rx, sd);
    let (reply, image) = oneshot::channel();
    commands
        .send(LcdCommand::SetImage {
            jpeg: jpeg(42),
            reply,
        })
        .await
        .unwrap();
    let HidCommand::Firmware { reply } = requests.recv().await.unwrap() else {
        panic!("expected firmware gate");
    };
    let packet = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
        134,
        b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
    );
    reply
        .send(Ok(llcontrol_daemon::actor::FirmwareResponses {
            version: *packet.as_bytes(),
            datetime: None,
        }))
        .unwrap();
    let HidCommand::Write { bytes, reply } = requests.recv().await.unwrap() else {
        panic!("expected stream setup");
    };
    assert_eq!(&bytes[11..19], &[4, 100, 0, 0, 0, 0, 0, 24]);
    reply.send(Ok(())).unwrap();
    let HidCommand::FrameBatch { reply: held, .. } = requests.recv().await.unwrap() else {
        panic!("expected image batch")
    };
    let (reply, queued) = oneshot::channel();
    commands
        .send(LcdCommand::SetComposition {
            prepared,
            preparation: None,
            reply,
        })
        .await
        .unwrap();
    shutdown.send(true).unwrap();
    assert!(!actor.is_finished());
    complete(held);
    image.await.unwrap().unwrap();
    tokio::time::timeout(Duration::from_secs(3), actor)
        .await
        .unwrap()
        .unwrap();
    assert!(queued
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("shutting down"));
    store.delete_unreferenced(&key, || Ok(false)).unwrap();
    assert!(
        requests.recv().await.is_none(),
        "queued Apply wrote after shutdown"
    );
}

#[tokio::test]
async fn delayed_datetime_before_version_does_not_block_h264_startup_gate() {
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info());
    let mut datetime = llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
        134,
        b"Apr 12 2024 14:33:21",
    )
    .as_bytes()
    .to_vec();
    datetime[4] = 1;
    mock.queue_response(&lcdv2_info(), datetime);
    mock.queue_response(
        &lcdv2_info(),
        llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
            134,
            b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
        )
        .as_bytes()
        .to_vec(),
    );
    let (stop, shutdown) = watch::channel(false);
    let (hid, hid_task) = hid_io::spawn_tracked(
        Arc::new(mock.clone()),
        lcdv2_info(),
        Arc::new(llcontrol_daemon::stats::Stats::new()),
        shutdown.clone(),
    )
    .unwrap();
    let (tx, rx) = mpsc::channel(8);
    let (settings, _settings) = mpsc::channel(16);
    let _actor = lcd_keeper::spawn(hid, 15000, Default::default(), settings, rx, shutdown);
    let (reply, response) = oneshot::channel();
    tx.send(LcdCommand::PlayClip {
        clip: clip(),
        loops: 1,
        reply,
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), response)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let history = mock.write_history(&lcdv2_info());
    assert_eq!(history.iter().filter(|bytes| bytes[1] == 134).count(), 1);
    assert!(history.iter().any(|bytes| bytes[0] == 3 && bytes[1] == 13));
    stop.send(true).unwrap();
    hid_task.await.unwrap();
}

#[tokio::test]
async fn jpeg_rotation_patch_keeps_the_host_stream_running() {
    let mut h = Harness::new();
    let image = h.image(42).await;
    let (_, first) = h.frame(false).await;
    complete(first);
    image.await.unwrap().unwrap();
    let before = h.stats().await;
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: llcontrol_ipc::LcdConfigPatch {
            mode: None,
            brightness_percent: None,
            rotation_degrees: Some(90),
        },
        reply,
    })
    .await
    .unwrap();
    let state = response.await.unwrap().unwrap();
    assert_eq!(state.source, llcontrol_ipc::LcdSourceKind::Jpeg);
    assert_eq!(state.requested.rotation_degrees, 90);
    assert_eq!(state.applied.unwrap().rotation_degrees, 90);
    assert!(
        h.hid.try_recv().is_err(),
        "rotation must not send hardware settings"
    );
    tokio::time::sleep(Duration::from_millis(42)).await;
    let (_, next) = h.frame(true).await;
    complete(next);
    let after = h.stats().await;
    assert!(after.sent_frames > before.sent_frames);
    assert_eq!(after.failed_transactions, 0);
}

#[tokio::test]
async fn still_image_streams_continuously_at_24_fps() {
    let mut h = Harness::new();
    let image = h.image(42).await;
    let (_, first) = h.frame(false).await;
    complete(first);
    image.await.unwrap().unwrap();
    let started = std::time::Instant::now();
    for _ in 0..4 {
        complete(h.frame(true).await.1);
        h.state().await;
    }
    assert_eq!(h.stats().await.sent_frames, 5);
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "frames must be paced, not burst"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "legacy multi-second JPEG refresh returned"
    );
}

#[tokio::test]
async fn unsupported_firmware_refuses_still_image_before_setup() {
    for version in ["1.4", "unknown"] {
        let mut h = Harness::new();
        let image = h.image(42).await;
        h.firmware(version).await;
        assert!(image.await.unwrap().is_err());
        assert!(h.hid.try_recv().is_err());
        assert_eq!(h.state().await.generation, 0);
    }
}
