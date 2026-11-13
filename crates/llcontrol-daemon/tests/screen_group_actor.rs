use llcontrol_daemon::{
    actor::{
        command_dispatch::spawn_updates,
        lcd_keeper,
        screen_group::{PreparedScreen, PreparedScreenGroup, ScreenGroupResult},
        FrameOutcome, HidCommand, LcdCommand,
    },
    persistence::{DeviceSettings, PersistenceStore},
    profile_archive::{PreparedGroup, ProfileArchives},
    profile_group::{GroupKind, GroupProfile, GroupRevision},
    sensors::SensorCache,
};
use llcontrol_ipc::*;
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

struct Harness {
    _dir: tempfile::TempDir,
    store: Arc<PersistenceStore>,
    tx: mpsc::Sender<LcdCommand>,
    stop: watch::Sender<bool>,
    lcd: JoinHandle<()>,
    settings: JoinHandle<()>,
    hid: JoinHandle<()>,
    writes: Arc<Mutex<Vec<bool>>>,
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    failure: Arc<AtomicU8>,
    firmware: Arc<AtomicU8>,
    cache: SensorCache,
    config_gate: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    configs: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl Harness {
    fn new() -> Self {
        Self::from_dir(tempfile::tempdir().unwrap())
    }
    fn from_dir(dir: tempfile::TempDir) -> Self {
        let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
        let (settings, rx) = mpsc::channel(16);
        let (stop, shutdown) = watch::channel(false);
        let initial = store.read("device").unwrap();
        let initial_lcd = initial.lcd.clone();
        let writer = spawn_updates(
            store.clone(),
            "device".into(),
            initial,
            rx,
            shutdown.clone(),
        );
        let (hid, mut requests) = mpsc::channel(16);
        let writes = Arc::new(Mutex::new(Vec::new()));
        let frames = Arc::new(Mutex::new(Vec::new()));
        let configs = Arc::new(Mutex::new(Vec::new()));
        let saved_configs = configs.clone();
        let observed = writes.clone();
        let output = frames.clone();
        let persisted = store.clone();
        let failure = Arc::new(AtomicU8::new(0));
        let injected = failure.clone();
        let firmware = Arc::new(AtomicU8::new(6));
        let version = firmware.clone();
        let config_gate = Arc::new(Mutex::new(None::<oneshot::Receiver<()>>));
        let hold_config = config_gate.clone();
        let transport = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                match request {
                    HidCommand::Firmware { reply } => {
                        let token = format!(
                            "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.{}",
                            version.load(Ordering::SeqCst)
                        );
                        let bytes=*llcontrol_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(134,token.as_bytes()).as_bytes();
                        let _ = reply.send(Ok(llcontrol_daemon::actor::FirmwareResponses {
                            version: bytes,
                            datetime: None,
                        }));
                    }
                    request => {
                        let store = persisted.clone();
                        let committed = tokio::task::spawn_blocking(move || {
                            !store.read("device").unwrap().group_imports.is_empty()
                        })
                        .await
                        .unwrap();
                        observed.lock().unwrap().push(committed);
                        match request {
                            HidCommand::Write { bytes, reply } => {
                                saved_configs.lock().unwrap().push(bytes);

                                let wait = hold_config.lock().unwrap().take();
                                if let Some(wait) = wait {
                                    let _ = wait.await;
                                }

                                let _ = reply.send(if injected.load(Ordering::SeqCst) == 1 {
                                    Err(llcontrol_hid::Error::BlockingTaskFailed(
                                        "injected config failure".into(),
                                    ))
                                } else {
                                    Ok(())
                                });
                            }
                            HidCommand::FrameBatch { packets, reply, .. } => {
                                output.lock().unwrap().push(packets.concat());
                                let _ = reply.send(if injected.load(Ordering::SeqCst) == 2 {
                                    Err(llcontrol_hid::Error::BlockingTaskFailed(
                                        "injected frame failure".into(),
                                    ))
                                } else {
                                    Ok(FrameOutcome {
                                        written_reports: packets.len(),
                                        readback: vec![],
                                        elapsed: Duration::ZERO,
                                    })
                                });
                            }
                            _ => panic!("unexpected HID request"),
                        }
                    }
                }
            }
        });
        let (tx, rx) = mpsc::channel(16);
        let lcd = lcd_keeper::spawn(hid, 60000, initial_lcd, settings, rx, shutdown);
        Self {
            _dir: dir,
            store,
            tx,
            stop,
            lcd,
            settings: writer,
            hid: transport,
            writes,
            frames,
            failure,
            firmware,
            cache: SensorCache::default(),
            config_gate,
            configs,
        }
    }
    async fn archive(&self, kind: &str, rotation: u16) -> Arc<PreparedGroup> {
        let dir = tempfile::tempdir().unwrap();
        let source = PersistenceStore::new_with_root(dir.path().into());
        let mut settings = DeviceSettings::default();
        settings.lcd.brightness = 73;
        settings.lcd.rotation = rotation.to_string();
        settings.lcd.control_mode = "Application".into();
        settings.screen_collections = Some(ScreenCollections::defaults(1));
        if kind == "builtin_media" {
            settings.media_clip = Some(BUILTIN_VIDEO_KEY.into());
        } else if kind == "media" {
            let media = source
                .media_assets()
                .import(
                    "Stored video".into(),
                    H264Clip {
                        fps: 24,
                        units: vec![H264AccessUnit {
                            bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                            keyframe: true,
                        }],
                    },
                )
                .unwrap();
            settings.media_clip = Some(media.key);
        } else if kind != "idle" {
            let theme = if kind == "composition" {
                serde_json::json!({"version":3,"name":"Rain","background":[0,0,0],"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":42}}]})
            } else {
                serde_json::json!({"version":1,"name":"Realtime","background":[0,0,0],"layers":[{"kind":"sensor","sensor_id":"test:cpu","label":"CPU","unit":"percent","temperature_unit":"celsius","decimals":0,"style":{"x":240,"y":240,"size":60,"color":[255,255,255],"align":"center"}}]})
            };
            settings.theme = Some(serde_json::from_value(theme).unwrap());
        }
        if kind == "editor" {
            let video = source
                .media_assets()
                .import(
                    "Unapplied editor clip".into(),
                    H264Clip {
                        fps: 24,
                        units: vec![H264AccessUnit {
                            bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                            keyframe: true,
                        }],
                    },
                )
                .unwrap()
                .key;
            let mut editor: LcdTheme = serde_json::from_str(include_str!(
                "../../../desktop/tests/fixtures/basic-editor-theme.json"
            ))
            .unwrap();
            editor.background_image = None;
            editor.background_video = Some(video.clone());
            editor.version = 2;
            settings.screen_editor=Some(llcontrol_daemon::screen_editor::StoredScreenEditor {
                version:1,theme:Some(editor),resources:std::collections::BTreeMap::from([("archive:wmsq/clip.mp4".into(),ScreenBackgroundAsset::Video(video))]),
                source_json:serde_json::json!({"version":1,"format":"windows_llcontrol3_2_1_28","source":{"archive":{"backup_sha256":"a".repeat(64)},"unassigned":"private history"}}).to_string(),
            });
        }
        let profile =
            GroupProfile::capture(&settings, &ProfileLibrary::default(), GroupKind::Screen)
                .unwrap()
                .profile;
        let source_archives = ProfileArchives::new(
            source.theme_assets(),
            source.theme_fonts(),
            source.media_assets(),
        );
        let mut bytes = Vec::new();
        source_archives.export_group(&profile, &mut bytes).unwrap();
        Arc::new(
            ProfileArchives::new(
                self.store.theme_assets(),
                self.store.theme_fonts(),
                self.store.media_assets(),
            )
            .prepare_group(&mut Cursor::new(bytes))
            .unwrap(),
        )
    }
    async fn prepared(&self, archive: Arc<PreparedGroup>) -> PreparedScreenGroup {
        PreparedScreenGroup::prepare(archive, self.store.clone(), vec![self.cache.clone()])
            .await
            .unwrap()
    }
    fn expected(&self) -> GroupRevision {
        self.store
            .snapshot_profile_group(
                "device",
                &self.store.read("device").unwrap(),
                GroupKind::Screen,
            )
            .unwrap()
            .revision
    }
    async fn import(
        &self,
        id: &str,
        expected: GroupRevision,
        archive: Arc<PreparedGroup>,
    ) -> llcontrol_ipc::Result<ScreenGroupResult> {
        let prepared = self.prepared(archive).await;
        let (reply, response) = oneshot::channel();
        self.tx
            .send(LcdCommand::CommitScreenGroup {
                operation_id: id.into(),
                expected,
                preparation: None,
                prepared,
                reply,
            })
            .await
            .unwrap();
        response.await.unwrap()
    }
    async fn state(&self) -> LcdState {
        let (reply, response) = oneshot::channel();
        self.tx.send(LcdCommand::State { reply }).await.unwrap();
        response.await.unwrap()
    }
    async fn ticket(&self) -> u64 {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(LcdCommand::BeginThemePreparation { reply })
            .await
            .unwrap();
        response.await.unwrap()
    }
    async fn clear(&self) -> LcdState {
        let (reply, response) = oneshot::channel();
        self.tx.send(LcdCommand::Clear { reply }).await.unwrap();
        response.await.unwrap().unwrap()
    }
    fn sample(&self, value: f64) {
        self.cache.publish(
            Instant::now(),
            vec![SensorReading {
                id: "test:cpu".into(),
                label: "CPU".into(),
                unit: SensorUnit::Percent,
                value: Some(value),
                status: SensorStatus::Valid,
                observed_unix_ms: None,
                age_ms: None,
            }],
        );
    }
    async fn restart(self) -> Self {
        let _ = self.stop.send(true);
        drop(self.tx);
        self.lcd.await.unwrap();
        self.settings.await.unwrap();
        self.hid.await.unwrap();
        Self::from_dir(self._dir)
    }
    async fn finish(self) {
        let _ = self.stop.send(true);
        drop(self.tx);
        self.lcd.await.unwrap();
        self.settings.await.unwrap();
        self.hid.await.unwrap();
    }
}

#[tokio::test]
async fn jpeg_group_commits_before_writes_retains_rotation_and_renders_live_sensor_updates() {
    let h = Harness::new();
    h.sample(15.);
    let archive = h.archive("theme", 90).await;
    let result = h
        .import(&"a".repeat(32), h.expected(), archive)
        .await
        .unwrap();
    assert!(!result.commit.repeated);
    assert_eq!(result.lcd.source, LcdSourceKind::Theme);
    assert_eq!(result.lcd.applied.as_ref().unwrap().rotation_degrees, 90);
    assert_eq!(result.lcd.applied.as_ref().unwrap().brightness_percent, 73);
    assert!(h.writes.lock().unwrap().iter().all(|committed| *committed));
    let first = h.frames.lock().unwrap()[0].clone();
    h.sample(88.);
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if h.frames.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_ne!(
        h.frames.lock().unwrap()[1],
        first,
        "sensor-only scene must use fresh sensor values after import"
    );
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    assert_eq!(h.store.read("device").unwrap().lcd.rotation, "90");
    h.finish().await;
}

#[tokio::test]
async fn lost_reply_and_repeat_preserve_new_clear_and_invalidate_old_preparation() {
    let h = Harness::new();
    let archive = h.archive("theme", 0).await;
    let expected = h.expected();
    let old_ticket = h.ticket().await;
    let id = "b".repeat(32);
    let prepared = h.prepared(archive.clone()).await;
    let (reply, lost) = oneshot::channel();
    drop(lost);
    h.tx.send(LcdCommand::CommitScreenGroup {
        operation_id: id.clone(),
        expected: expected.clone(),
        preparation: Some(old_ticket),
        prepared,
        reply,
    })
    .await
    .unwrap();
    assert_eq!(h.state().await.source, LcdSourceKind::Theme);
    let delayed = h.prepared(archive.clone()).await;
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::CommitScreenGroup {
        operation_id: "f".repeat(32),
        expected: h.expected(),
        preparation: Some(old_ticket),
        prepared: delayed,
        reply,
    })
    .await
    .unwrap();
    assert!(response.await.unwrap().is_err());
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    let cleared = h.clear().await;
    let count = h.writes.lock().unwrap().len();
    let repeated = h.import(&id, expected, archive).await.unwrap();
    assert!(repeated.commit.repeated);
    assert_eq!(repeated.lcd.generation, cleared.generation);
    assert_eq!(repeated.lcd.source, LcdSourceKind::Idle);
    assert_eq!(h.writes.lock().unwrap().len(), count);
    assert!(!repeated.commit.readback.selected_content_current);
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    h.finish().await;
}

#[tokio::test]
async fn config_and_frame_failures_keep_receipt_and_explicit_reapply_never_reimports() {
    for failure in [1, 2] {
        let h = Harness::new();
        let archive = h.archive("theme", 0).await;
        let expected = h.expected();
        let id = "c".repeat(32);
        h.failure.store(failure, Ordering::SeqCst);
        let failed = h
            .import(&id, expected.clone(), archive.clone())
            .await
            .unwrap();
        assert!(failed.lcd.last_error.is_some());
        assert_eq!(failed.lcd.source, LcdSourceKind::Idle);
        let count = h.writes.lock().unwrap().len();
        let repeated = h.import(&id, expected, archive).await.unwrap();
        assert!(repeated.commit.repeated);
        assert_eq!(h.writes.lock().unwrap().len(), count);
        h.failure.store(0, Ordering::SeqCst);
        let latest = h.store.read("device").unwrap();
        let prepared = PreparedScreen::prepare(&latest, h.store.clone(), vec![h.cache.clone()])
            .await
            .unwrap();
        let ticket = h.ticket().await;
        let (reply, response) = oneshot::channel();
        h.tx.send(LcdCommand::ReapplyScreenGroup {
            operation_id: id,
            expected_generation: failed.lcd.generation,
            preparation: ticket,
            prepared,
            reply,
        })
        .await
        .unwrap();
        let state = response.await.unwrap().unwrap();
        assert_eq!(state.source, LcdSourceKind::Theme);
        assert!(state.last_error.is_none());
        assert_eq!(h.store.profiles().list().unwrap().revision, 1);
        h.finish().await;
    }
}

#[tokio::test]
async fn old_reapply_cannot_undo_a_newer_brightness_or_clear() {
    let h = Harness::new();
    let archive = h.archive("theme", 0).await;
    let id = "d".repeat(32);
    let imported = h.import(&id, h.expected(), archive).await.unwrap();
    let latest = h.store.read("device").unwrap();
    let prepared = PreparedScreen::prepare(&latest, h.store.clone(), vec![])
        .await
        .unwrap();
    let ticket = h.ticket().await;
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: LcdConfigPatch {
            mode: None,
            brightness_percent: Some(87),
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    response.await.unwrap().unwrap();
    let count = h.writes.lock().unwrap().len();
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::ReapplyScreenGroup {
        operation_id: id,
        expected_generation: imported.lcd.generation,
        preparation: ticket,
        prepared,
        reply,
    })
    .await
    .unwrap();
    assert!(response.await.unwrap().is_err());
    assert_eq!(h.writes.lock().unwrap().len(), count);
    assert_eq!(h.state().await.requested.brightness_percent, 87);
    h.finish().await;
}

#[tokio::test]
async fn streaming_media_and_composition_apply_after_commit_with_known_firmware() {
    for kind in ["media", "composition"] {
        let h = Harness::new();
        let archive = h.archive(kind, 0).await;
        let result = h
            .import(&"e".repeat(32), h.expected(), archive)
            .await
            .unwrap();
        assert!(
            result.lcd.last_error.is_none(),
            "kind={kind}: {:?}",
            result.lcd.last_error
        );
        assert_eq!(result.lcd.applied.as_ref().unwrap().mode, "LCDSetting");
        assert!(h.writes.lock().unwrap().iter().all(|saved| *saved));
        assert!(!h.frames.lock().unwrap().is_empty());
        h.clear().await;
        h.finish().await;
    }
}

#[tokio::test]
async fn invalid_rotation_and_firmware_reject_before_storage_or_output_mutation() {
    let h = Harness::new();
    let mut invalid = DeviceSettings::default();
    invalid.lcd.rotation = "45".into();
    assert!(PreparedScreen::prepare(&invalid, h.store.clone(), vec![])
        .await
        .is_err());
    assert_eq!(h.store.profiles().list().unwrap().revision, 0);
    h.firmware.store(4, Ordering::SeqCst);
    let archive = h.archive("composition", 0).await;
    assert!(h
        .import(&"f".repeat(32), h.expected(), archive)
        .await
        .is_err());
    assert_eq!(h.store.profiles().list().unwrap().revision, 0);
    assert!(h.writes.lock().unwrap().is_empty());
    h.finish().await;
}

#[tokio::test]
async fn queued_composition_group_is_joined_on_shutdown_without_commit_or_writes() {
    let h = Harness::new();
    let archive = h.archive("composition", 0).await;
    let prepared = h.prepared(archive).await;
    let expected = h.expected();
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::CommitScreenGroup {
        operation_id: "a".repeat(32),
        expected,
        preparation: None,
        prepared,
        reply,
    })
    .await
    .unwrap();
    h.stop.send(true).unwrap();
    assert!(response.await.unwrap().is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    assert_eq!(h.store.profiles().list().unwrap().revision, 0);
    h.finish().await;
}

#[tokio::test]
async fn durable_readback_can_prepare_reapply_on_a_fresh_actor_after_restart() {
    let h = Harness::new();
    let archive = h.archive("theme", 180).await;
    let id = "a".repeat(32);
    h.import(&id, h.expected(), archive).await.unwrap();
    let h = h.restart().await;
    assert_eq!(h.state().await.generation, 0);
    assert_eq!(h.state().await.requested.brightness_percent, 73);
    let latest = h.store.read("device").unwrap();
    let readback = h
        .store
        .read_group_import("device", &latest, &id)
        .unwrap()
        .unwrap();
    assert!(readback.selected_content_current);
    let prepared =
        PreparedScreen::prepare(&readback.settings, h.store.clone(), vec![h.cache.clone()])
            .await
            .unwrap();
    let ticket = h.ticket().await;
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::ReapplyScreenGroup {
        operation_id: id,
        expected_generation: 0,
        preparation: ticket,
        prepared,
        reply,
    })
    .await
    .unwrap();
    let state = response.await.unwrap().unwrap();
    assert_eq!(state.source, LcdSourceKind::Theme);
    assert_eq!(state.applied.as_ref().unwrap().rotation_degrees, 180);
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    h.finish().await;
}

#[tokio::test]
async fn unresolved_storage_outcome_halts_lcd_mutations_and_keeps_readonly_state() {
    let h = Harness::new();
    let archive = h.archive("composition", 0).await;
    let prepared = h.prepared(archive).await;
    let expected = h.expected();
    std::fs::write(
        h._dir.path().join("screen-reset-transaction.json"),
        "broken journal",
    )
    .unwrap();
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::CommitScreenGroup {
        operation_id: "b".repeat(32),
        expected,
        preparation: None,
        prepared,
        reply,
    })
    .await
    .unwrap();
    assert!(response.await.unwrap().is_err());
    assert_eq!(h.state().await.source, LcdSourceKind::Idle);
    assert!(h.state().await.last_error.is_some());
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::Clear { reply }).await.unwrap();
    assert!(response.await.unwrap().is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(h._dir.path().join("screen-reset-transaction.json")).unwrap(),
        "broken journal"
    );
    h.finish().await;
}

#[tokio::test]
async fn shutdown_during_config_ack_keeps_receipt_but_never_starts_a_new_frame() {
    let h = Harness::new();
    let archive = h.archive("theme", 0).await;
    let prepared = h.prepared(archive).await;
    let expected = h.expected();
    let (release, wait) = oneshot::channel();
    *h.config_gate.lock().unwrap() = Some(wait);
    let (reply, response) = oneshot::channel();
    h.tx.send(LcdCommand::CommitScreenGroup {
        operation_id: "c".repeat(32),
        expected,
        preparation: None,
        prepared,
        reply,
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !h.writes.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    h.stop.send(true).unwrap();
    release.send(()).unwrap();
    let result = response.await.unwrap().unwrap();
    assert_eq!(result.commit.readback.receipt.profile_revision, 1);
    assert_eq!(result.lcd.source, LcdSourceKind::Idle);
    assert!(result.lcd.last_error.is_some());
    assert!(h.frames.lock().unwrap().is_empty());
    h.finish().await;
}

#[tokio::test]
async fn replacing_imported_media_joins_old_reader_and_never_emits_old_video_after_new_jpeg() {
    let h = Harness::new();
    let media = h.archive("media", 0).await;
    h.import(&"a".repeat(32), h.expected(), media)
        .await
        .unwrap();
    let video = h.frames.lock().unwrap()[0].clone();
    let theme = h.archive("theme", 0).await;
    let imported = h
        .import(&"b".repeat(32), h.expected(), theme)
        .await
        .unwrap();
    assert_eq!(imported.lcd.source, LcdSourceKind::Theme);
    let boundary = h.frames.lock().unwrap().len();
    assert_ne!(h.frames.lock().unwrap()[boundary - 1], video);
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if h.frames.lock().unwrap().len() > boundary {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(h.frames.lock().unwrap()[boundary..]
        .iter()
        .all(|frame| *frame != video));
    assert_eq!(h.store.profiles().list().unwrap().revision, 2);
    h.finish().await;
}

#[tokio::test]
async fn rotated_group_media_retains_media_identity_brightness_and_durable_reapply() {
    for rotation in [90, 180, 270] {
        let h = Harness::new();
        let archive = h.archive("builtin_media", rotation).await;
        let id = "a".repeat(32);
        let imported = h.import(&id, h.expected(), archive).await.unwrap();
        assert_eq!(imported.lcd.source, LcdSourceKind::DiagnosticClip);
        assert_eq!(
            imported.lcd.applied.as_ref().unwrap().rotation_degrees,
            rotation
        );
        let (reply, response) = oneshot::channel();
        h.tx.send(LcdCommand::Theme { reply }).await.unwrap();
        assert!(response.await.unwrap().is_none());
        let latest = h.store.read("device").unwrap();
        assert!(latest.theme.is_none());
        assert!(latest.media_clip.is_some());
        let packet = h.configs.lock().unwrap().last().unwrap().clone();
        let parsed =
            llcontrol_protocol::galahad2_lcdv2::bpacket::BCommandPacket::from_response(&packet, 12)
                .unwrap();
        assert_eq!(
            parsed.data()[2],
            0,
            "H264 rotation must be performed once, in software"
        );
        let h = h.restart().await;
        let latest = h.store.read("device").unwrap();
        let prepared = PreparedScreen::prepare(&latest, h.store.clone(), vec![])
            .await
            .unwrap();
        let ticket = h.ticket().await;
        let (reply, response) = oneshot::channel();
        h.tx.send(LcdCommand::ReapplyScreenGroup {
            operation_id: id,
            expected_generation: 0,
            preparation: ticket,
            prepared,
            reply,
        })
        .await
        .unwrap();
        let reapplied = response.await.unwrap().unwrap();
        assert_eq!(reapplied.source, LcdSourceKind::DiagnosticClip);
        assert_eq!(reapplied.requested.rotation_degrees, rotation);
        let (reply, response) = oneshot::channel();
        h.tx.send(LcdCommand::Patch {
            patch: LcdConfigPatch {
                mode: None,
                brightness_percent: Some(85),
                rotation_degrees: None,
            },
            reply,
        })
        .await
        .unwrap();
        assert_eq!(
            response.await.unwrap().unwrap().requested.rotation_degrees,
            rotation
        );
        assert_eq!(h.store.profiles().list().unwrap().revision, 1);
        h.clear().await;
        h.finish().await;
    }
}

async fn group_state(h: &Harness) -> llcontrol_daemon::actor::screen_group::ScreenGroupState {
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::GroupState { reply }).await.unwrap();
    rx.await.unwrap()
}
async fn checked_begin(
    h: &Harness,
    id: &str,
) -> llcontrol_daemon::actor::screen_group::ScreenReapplyAdmission {
    let expected = group_state(h).await.revision;
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::BeginScreenGroupReapply {
        operation_id: id.into(),
        expected,
        reply,
    })
    .await
    .unwrap();
    rx.await.unwrap().unwrap()
}
async fn checked_apply(
    h: &Harness,
    id: &str,
    a: llcontrol_daemon::actor::screen_group::ScreenReapplyAdmission,
) -> llcontrol_ipc::Result<llcontrol_daemon::actor::screen_group::ScreenReapplyResult> {
    let prepared =
        PreparedScreen::prepare(&a.readback.settings, h.store.clone(), vec![h.cache.clone()])
            .await
            .unwrap();
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::ReapplyScreenGroupChecked {
        operation_id: id.into(),
        expected: a.revision,
        expected_generation: a.generation,
        preparation: a.preparation,
        prepared,
        reply,
    })
    .await
    .unwrap();
    rx.await.unwrap()
}
#[tokio::test]
async fn checked_screen_reapply_rejects_failed_newer_intent_and_retries_known_frame_failure_without_storage_changes(
) {
    let h = Harness::new();
    let archive = h.archive("theme", 0).await;
    let id = "d".repeat(32);
    h.import(&id, h.expected(), archive).await.unwrap();
    let files = || {
        (
            std::fs::read(h._dir.path().join("devices/device.toml")).unwrap(),
            std::fs::read(h._dir.path().join("profiles.json")).unwrap(),
        )
    };
    let bytes = files();
    let admission = checked_begin(&h, &id).await;
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: LcdConfigPatch {
            mode: None,
            brightness_percent: Some(101),
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    let config_count = h.configs.lock().unwrap().len();
    let generation = h.state().await.generation;
    assert!(checked_apply(&h, &id, admission).await.is_err());
    // Refresh of the currently running scene remains legitimate; the rejected
    // candidate must not configure the LCD or replace its generation.
    assert_eq!(h.configs.lock().unwrap().len(), config_count);
    assert_eq!(h.state().await.generation, generation);
    assert_eq!(bytes, files());
    h.failure.store(2, Ordering::SeqCst);
    let result = checked_apply(&h, &id, checked_begin(&h, &id).await)
        .await
        .unwrap();
    assert!(result.runtime.lcd.last_error.is_some());
    assert!(!result.runtime.recovery_required);
    assert_eq!(bytes, files());
    h.failure.store(0, Ordering::SeqCst);
    let result = checked_apply(&h, &id, checked_begin(&h, &id).await)
        .await
        .unwrap();
    assert!(result.runtime.lcd.last_error.is_none());
    assert_eq!(result.runtime.lcd.source, LcdSourceKind::Theme);
    assert_eq!(bytes, files());
    h.finish().await;
}
#[tokio::test]
async fn checked_screen_reapply_runtime_instance_rejects_old_token_after_restart_with_current_receipt(
) {
    let h = Harness::new();
    let archive = h.archive("theme", 0).await;
    let id = "e".repeat(32);
    h.import(&id, h.expected(), archive).await.unwrap();
    let before = group_state(&h).await;
    let h = h.restart().await;
    // Recreate the old numeric generation without changing the stored group:
    // rejection must depend on the new instance, not on unequal counters.
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::Patch {
        patch: LcdConfigPatch {
            mode: None,
            brightness_percent: Some(101),
            rotation_degrees: None,
        },
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    let after = group_state(&h).await;
    let before_wire = serde_json::to_value(&before.revision).unwrap();
    let after_wire = serde_json::to_value(&after.revision).unwrap();
    assert_eq!(before_wire["generation"], after_wire["generation"]);
    assert_ne!(before_wire["instance"], after_wire["instance"]);
    assert_ne!(before.revision, after.revision);
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::BeginScreenGroupReapply {
        operation_id: id.clone(),
        expected: before.revision,
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    let result = checked_apply(&h, &id, checked_begin(&h, &id).await)
        .await
        .unwrap();
    assert!(result.readback.selected_content_current);
    assert!(result.runtime.lcd.last_error.is_none());
    assert_eq!(h.store.read("device").unwrap().group_imports.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn queued_checked_composition_reapply_is_joined_on_shutdown_without_output_or_commit() {
    let h = Harness::new();
    let archive = h.archive("composition", 0).await;
    let llcontrol_daemon::profile_group::GroupSettings::Screen(g) = &archive.profile().content
    else {
        panic!()
    };
    let settings = DeviceSettings {
        lcd: g.lcd.clone(),
        theme: g.theme.clone(),
        media_clip: g.media_clip.clone(),
        screen_collections: g.collections.clone(),
        ..Default::default()
    };
    let prepared = PreparedScreen::prepare(&settings, h.store.clone(), vec![h.cache.clone()])
        .await
        .unwrap();
    let expected = group_state(&h).await.revision;
    let (reply, rx) = oneshot::channel();
    h.tx.send(LcdCommand::ReapplyScreenGroupChecked {
        operation_id: "f".repeat(32),
        expected,
        expected_generation: 0,
        preparation: 0,
        prepared,
        reply,
    })
    .await
    .unwrap();
    h.stop.send(true).unwrap();
    assert!(rx.await.unwrap().is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    assert!(h.store.read("device").unwrap().group_imports.is_empty());
    h.finish().await;
}

#[tokio::test]
async fn imported_editor_video_is_retained_but_only_the_independent_active_jpeg_scene_runs() {
    let h = Harness::new();
    h.sample(42.);
    let archive = h.archive("editor", 0).await;
    let result = h
        .import(&"9".repeat(32), h.expected(), archive)
        .await
        .unwrap();
    assert!(
        result.lcd.last_error.is_none(),
        "{:?}",
        result.lcd.last_error
    );
    let settings = h.store.read("device").unwrap();
    assert_eq!(settings.theme.as_ref().unwrap().name, "Realtime");
    let editor = settings.screen_editor.as_ref().unwrap();
    assert!(editor.theme.as_ref().unwrap().requires_streaming());
    assert!(!settings.theme.as_ref().unwrap().requires_streaming());
    {
        let frames = h.frames.lock().unwrap();
        assert!(!frames.is_empty());
        assert!(
            frames[0].windows(2).any(|pair| pair == [0xff, 0xd8]),
            "actual active frame is JPEG, not the editor H264 unit"
        );
    }
    assert!(h.writes.lock().unwrap().iter().all(|committed| *committed));
    let editor_before = editor.clone();
    let h = h.restart().await;
    assert_eq!(
        h.store.read("device").unwrap().screen_editor,
        Some(editor_before)
    );
    h.finish().await;
}
