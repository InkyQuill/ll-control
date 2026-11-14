#[path = "support/firmware_fixture.rs"]
mod firmware_fixture;
use llcontrol_daemon::{
    actor::{
        command_dispatch::spawn_updates, lcd_keeper, FrameOutcome, HidCommand, LcdCommand,
        ScreenResetState,
    },
    persistence::{DeviceSettings, PersistenceStore},
    theme_runtime::{PreparedTheme, ThemeRuntime},
};
use llcontrol_image::theme::ThemeAssets;
use llcontrol_ipc::{LcdConfigPatch, LcdSourceKind, NamedProfile, ProfileContent, ProfileGroup};
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

async fn default_theme() -> PreparedTheme {
    let ProfileContent::Screen(theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        panic!("screen theme");
    };
    Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![]).unwrap())
        .prepare()
        .await
        .unwrap()
}

struct Harness {
    _dir: tempfile::TempDir,
    store: Arc<PersistenceStore>,
    commands: mpsc::Sender<LcdCommand>,
    shutdown: watch::Sender<bool>,
    lcd: JoinHandle<()>,
    settings: JoinHandle<()>,
    hid: JoinHandle<()>,
    // Whether each transport request observed a committed reset receipt.
    observed: Arc<Mutex<Vec<bool>>>,
    failure: Arc<AtomicU8>,
}
impl Harness {
    fn new(failure: u8) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
        let (settings, settings_rx) = mpsc::channel(16);
        let (shutdown, stop) = watch::channel(false);
        let writer = spawn_updates(
            store.clone(),
            "device".into(),
            DeviceSettings::default(),
            settings_rx,
            stop.clone(),
        );
        let (hid, mut requests) = mpsc::channel(16);
        let observed = Arc::new(Mutex::new(vec![]));
        let observations = observed.clone();
        let read_store = store.clone();
        let failure = Arc::new(AtomicU8::new(failure));
        let injected = failure.clone();
        let transport = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                if let HidCommand::Firmware { reply } = request {
                    let _ = reply.send(Ok(firmware_fixture::reply()));
                    continue;
                }
                let read_store = read_store.clone();
                let committed = tokio::task::spawn_blocking(move || {
                    read_store
                        .read("device")
                        .unwrap()
                        .screen_reset_receipt
                        .is_some()
                })
                .await
                .unwrap();
                observations.lock().unwrap().push(committed);
                match request {
                    HidCommand::Write { reply, .. } => {
                        let _ = reply.send(if injected.load(Ordering::SeqCst) == 1 {
                            Err(llcontrol_hid::Error::BlockingTaskFailed(
                                "injected config failure".into(),
                            ))
                        } else {
                            Ok(())
                        });
                    }
                    HidCommand::FrameBatch { packets, reply, .. } => {
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
                    _ => panic!("unexpected transport request"),
                }
            }
        });
        let (commands, rx) = mpsc::channel(16);
        let lcd = lcd_keeper::spawn(hid, 60000, Default::default(), settings, rx, stop);
        Self {
            _dir: dir,
            store,
            commands,
            shutdown,
            lcd,
            settings: writer,
            hid: transport,
            observed,
            failure,
        }
    }
    async fn reset(&self, id: &str, revision: u64) -> llcontrol_ipc::Result<ScreenResetState> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(LcdCommand::ResetScreen {
                preparation: None,
                operation_id: id.into(),
                expected_profile_revision: revision,
                expected_collection_revision: 0,
                prepared: default_composition().await,
                reply,
            })
            .await
            .unwrap();
        response.await.unwrap()
    }
    async fn finish(self) {
        self.shutdown.send(true).unwrap();
        drop(self.commands);
        self.lcd.await.unwrap();
        self.settings.await.unwrap();
        self.hid.await.unwrap();
    }
}

#[tokio::test]
async fn reset_commits_before_transport_and_invalidates_old_preparation_without_duplicate_apply() {
    let harness = Harness::new(0);
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::BeginThemePreparation { reply })
        .await
        .unwrap();
    let old_ticket = response.await.unwrap();
    let id = "a".repeat(32);
    let applied = harness.reset(&id, 0).await.unwrap();
    assert!(!applied.repeated);
    assert_eq!(applied.receipt.profile_revision, 1);
    assert_eq!(applied.lcd.source, LcdSourceKind::Theme);
    assert_eq!(applied.lcd.applied.as_ref().unwrap().brightness_percent, 50);
    assert_eq!(applied.lcd.applied.as_ref().unwrap().rotation_degrees, 0);
    assert!(harness
        .observed
        .lock()
        .unwrap()
        .iter()
        .all(|committed| *committed));
    assert!(harness.observed.lock().unwrap().len() >= 2);
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::SetTheme {
            preparation: Some(old_ticket),
            prepared: default_theme().await,
            reply,
        })
        .await
        .unwrap();
    assert!(response
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::Patch {
            patch: LcdConfigPatch {
                mode: None,
                brightness_percent: Some(73),
                rotation_degrees: None,
            },
            reply,
        })
        .await
        .unwrap();
    response.await.unwrap().unwrap();
    let repeated = harness.reset(&id, 0).await.unwrap();
    assert!(repeated.repeated);
    assert_eq!(repeated.lcd.requested.brightness_percent, 73);
    assert_eq!(repeated.lcd.generation, applied.lcd.generation);
    assert_eq!(harness.store.profiles().list().unwrap().revision, 1);
    // A rejected new operation neither replaces the live source nor changes config.
    assert!(harness.reset(&"b".repeat(32), 0).await.is_err());
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::State { reply })
        .await
        .unwrap();
    let state = response.await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Theme);
    assert_eq!(state.requested.brightness_percent, 73);
    harness.finish().await;
}

#[tokio::test]
async fn config_or_frame_failure_reports_durable_intent_and_does_not_repeat_on_retry() {
    for failure in [1, 2] {
        let harness = Harness::new(failure);
        let id = "c".repeat(32);
        let result = harness.reset(&id, 0).await.unwrap();
        assert_eq!(result.lcd.source, LcdSourceKind::Idle);
        assert!(result
            .lcd
            .last_error
            .as_deref()
            .unwrap()
            .contains("injected"));
        let saved = harness.store.read("device").unwrap();
        assert_eq!(saved.screen_reset_receipt.unwrap().operation_id, id);
        assert!(saved.theme.is_some());
        assert_eq!(saved.lcd.brightness, 50);
        let writes = harness.observed.lock().unwrap().len();
        let retry = harness.reset(&id, 0).await.unwrap();
        assert!(retry.repeated);
        assert!(retry.lcd.last_error.is_some());
        assert_eq!(harness.observed.lock().unwrap().len(), writes);
        harness.finish().await;
    }
}

#[tokio::test]
async fn lost_reset_reply_still_finishes_and_can_be_read_back_without_reapplication() {
    let harness = Harness::new(0);
    let (reply, response) = oneshot::channel();
    drop(response);
    let id = "d".repeat(32);
    harness
        .commands
        .send(LcdCommand::ResetScreen {
            preparation: None,
            operation_id: id.clone(),
            expected_profile_revision: 0,
            expected_collection_revision: 0,
            prepared: default_composition().await,
            reply,
        })
        .await
        .unwrap();
    let result = harness.reset(&id, 0).await.unwrap();
    assert!(result.repeated);
    assert_eq!(result.lcd.source, LcdSourceKind::Theme);
    assert_eq!(result.receipt.profile_revision, 1);
    harness.finish().await;
}

#[tokio::test]
async fn uncertain_recovery_keeps_readback_but_blocks_further_display_mutations() {
    let harness = Harness::new(0);
    std::fs::write(
        harness._dir.path().join("screen-reset-transaction.json"),
        "broken journal",
    )
    .unwrap();
    let error = harness.reset(&"e".repeat(32), 0).await.unwrap_err();
    assert!(error.to_string().contains("recovery required"));
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::State { reply })
        .await
        .unwrap();
    let state = response.await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert!(state
        .last_error
        .as_deref()
        .unwrap()
        .contains("recovery required"));
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::Patch {
            patch: LcdConfigPatch {
                mode: None,
                brightness_percent: Some(20),
                rotation_degrees: None,
            },
            reply,
        })
        .await
        .unwrap();
    assert!(response
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("restart"));
    assert!(harness.observed.lock().unwrap().is_empty());
    harness.finish().await;
}

#[tokio::test]
async fn delayed_reset_is_rejected_before_storage_after_newer_clear_or_apply() {
    for clear in [true, false] {
        let harness = Harness::new(0);
        let id = "e".repeat(32);
        let (reply, response) = oneshot::channel();
        harness
            .commands
            .send(LcdCommand::BeginScreenReset {
                operation_id: id.clone(),
                reply,
            })
            .await
            .unwrap();
        let llcontrol_daemon::actor::ScreenResetAdmission::Prepare(ticket) =
            response.await.unwrap().unwrap()
        else {
            panic!("new reset must reserve preparation");
        };
        // Hold reset preparation while a later user action completes.
        let (reply, response) = oneshot::channel();
        let command = if clear {
            LcdCommand::Clear { reply }
        } else {
            LcdCommand::SetTheme {
                preparation: None,
                prepared: default_theme().await,
                reply,
            }
        };
        harness.commands.send(command).await.unwrap();
        let newer = response.await.unwrap().unwrap();
        let (reply, response) = oneshot::channel();
        harness
            .commands
            .send(LcdCommand::ResetScreen {
                preparation: Some(ticket),
                operation_id: id,
                expected_profile_revision: 0,
                expected_collection_revision: 0,
                prepared: default_composition().await,
                reply,
            })
            .await
            .unwrap();
        assert!(response
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("superseded"));
        assert_eq!(harness.store.profiles().list().unwrap().revision, 0);
        let saved = harness.store.read("device").unwrap();
        assert!(saved.screen_reset_receipt.is_none());
        assert!(saved.screen_collections.is_none());
        let (reply, response) = oneshot::channel();
        harness
            .commands
            .send(LcdCommand::State { reply })
            .await
            .unwrap();
        let current = response.await.unwrap();
        assert_eq!(current.generation, newer.generation);
        assert_eq!(current.source, newer.source);
        harness.finish().await;
    }
}

#[tokio::test]
async fn duplicate_reset_admission_preserves_pending_theme_ticket() {
    let harness = Harness::new(0);
    let id = "f".repeat(32);
    let first = harness.reset(&id, 0).await.unwrap();
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::BeginThemePreparation { reply })
        .await
        .unwrap();
    let ticket = response.await.unwrap();
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::BeginScreenReset {
            operation_id: id,
            reply,
        })
        .await
        .unwrap();
    let llcontrol_daemon::actor::ScreenResetAdmission::Repeated(repeated) =
        response.await.unwrap().unwrap()
    else {
        panic!("completed reset must not reserve preparation");
    };
    assert!(repeated.repeated);
    assert_eq!(repeated.lcd.generation, first.lcd.generation);
    let (reply, response) = oneshot::channel();
    harness
        .commands
        .send(LcdCommand::SetTheme {
            preparation: Some(ticket),
            prepared: default_theme().await,
            reply,
        })
        .await
        .unwrap();
    response.await.unwrap().unwrap();
    assert_eq!(harness.store.profiles().list().unwrap().revision, 1);
    harness.finish().await;
}

async fn default_composition() -> llcontrol_daemon::media_composition::PreparedComposition {
    let ProfileContent::Screen(theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        panic!("screen theme")
    };
    let runtime = Arc::new(ThemeRuntime::new(theme, ThemeAssets::new(), vec![]).unwrap());
    llcontrol_daemon::media_composition::PreparedComposition::prepare_canvas(runtime)
        .await
        .unwrap()
}

async fn prepare_reapply(
    h: &Harness,
    id: &str,
    generation: u64,
) -> (
    llcontrol_daemon::media_composition::PreparedComposition,
    u64,
) {
    let (reply, response) = oneshot::channel();
    h.commands
        .send(LcdCommand::PrepareScreenResetReapply {
            operation_id: id.into(),
            expected_generation: generation,
            reply,
        })
        .await
        .unwrap();
    let (runtime, ticket) = response.await.unwrap().unwrap();
    (
        llcontrol_daemon::media_composition::PreparedComposition::prepare_canvas(runtime)
            .await
            .unwrap(),
        ticket,
    )
}

#[tokio::test]
async fn explicit_reapply_retries_transport_without_resetting_storage_again() {
    for failure in [1, 2] {
        let h = Harness::new(failure);
        let id = "a".repeat(32);
        let first = h.reset(&id, 0).await.unwrap();
        assert!(first.lcd.last_error.is_some());
        let (prepared, preparation) = prepare_reapply(&h, &id, first.lcd.generation).await;
        h.failure.store(0, Ordering::SeqCst);
        let (reply, response) = oneshot::channel();
        h.commands
            .send(LcdCommand::ReapplyScreenReset {
                operation_id: id.clone(),
                expected_generation: first.lcd.generation,
                preparation,
                prepared,
                reply,
            })
            .await
            .unwrap();
        let applied = response.await.unwrap().unwrap();
        assert!(applied.last_error.is_none());
        assert_eq!(applied.source, LcdSourceKind::Theme);
        assert!(applied.generation > first.lcd.generation);
        assert_eq!(h.store.profiles().list().unwrap().revision, 1);
        let saved = h.store.read("device").unwrap();
        assert_eq!(saved.screen_collections.unwrap().revision, 1);
        let (reply, response) = oneshot::channel();
        h.commands
            .send(LcdCommand::PrepareScreenResetReapply {
                operation_id: id,
                expected_generation: first.lcd.generation,
                reply,
            })
            .await
            .unwrap();
        assert!(response.await.unwrap().is_err());
        h.finish().await;
    }
}

#[tokio::test]
async fn reapply_preparation_cannot_overwrite_new_brightness_or_pending_theme() {
    for brightness in [true, false] {
        let h = Harness::new(1);
        let id = "b".repeat(32);
        let first = h.reset(&id, 0).await.unwrap();
        let (prepared, preparation) = prepare_reapply(&h, &id, first.lcd.generation).await;
        h.failure.store(0, Ordering::SeqCst);
        if brightness {
            let (reply, response) = oneshot::channel();
            h.commands
                .send(LcdCommand::Patch {
                    patch: LcdConfigPatch {
                        mode: None,
                        brightness_percent: Some(73),
                        rotation_degrees: None,
                    },
                    reply,
                })
                .await
                .unwrap();
            response.await.unwrap().unwrap();
        } else {
            let (reply, response) = oneshot::channel();
            h.commands
                .send(LcdCommand::BeginThemePreparation { reply })
                .await
                .unwrap();
            response.await.unwrap();
        }
        let writes = h.observed.lock().unwrap().len();
        let (reply, response) = oneshot::channel();
        h.commands
            .send(LcdCommand::ReapplyScreenReset {
                operation_id: id,
                expected_generation: first.lcd.generation,
                preparation,
                prepared,
                reply,
            })
            .await
            .unwrap();
        assert!(response.await.unwrap().is_err());
        assert_eq!(h.observed.lock().unwrap().len(), writes);
        let (reply, response) = oneshot::channel();
        h.commands
            .send(LcdCommand::ReadScreenReset { reply })
            .await
            .unwrap();
        let current = response.await.unwrap().unwrap();
        assert!(!current.reapply_available);
        if brightness {
            assert_eq!(current.lcd.requested.brightness_percent, 73);
        }
        h.finish().await;
    }
}

#[tokio::test]
async fn reapply_failure_can_be_retried_and_lost_success_reply_is_readable() {
    let h = Harness::new(2);
    let id = "c".repeat(32);
    let first = h.reset(&id, 0).await.unwrap();
    let (prepared, preparation) = prepare_reapply(&h, &id, first.lcd.generation).await;
    let (reply, response) = oneshot::channel();
    h.commands
        .send(LcdCommand::ReapplyScreenReset {
            operation_id: id.clone(),
            expected_generation: first.lcd.generation,
            preparation,
            prepared,
            reply,
        })
        .await
        .unwrap();
    let failed = response.await.unwrap().unwrap();
    assert!(failed.last_error.is_some());
    assert!(failed.generation > first.lcd.generation);
    let (prepared, preparation) = prepare_reapply(&h, &id, failed.generation).await;
    h.failure.store(0, Ordering::SeqCst);
    let (reply, response) = oneshot::channel();
    drop(response);
    h.commands
        .send(LcdCommand::ReapplyScreenReset {
            operation_id: id.clone(),
            expected_generation: failed.generation,
            preparation,
            prepared,
            reply,
        })
        .await
        .unwrap();
    let (reply, response) = oneshot::channel();
    h.commands
        .send(LcdCommand::ReadScreenReset { reply })
        .await
        .unwrap();
    let read = response.await.unwrap().unwrap();
    assert_eq!(read.receipt.unwrap().operation_id, id);
    assert!(!read.reapply_available);
    assert!(read.lcd.last_error.is_none());
    assert_eq!(read.lcd.source, LcdSourceKind::Theme);
    assert!(read.lcd.generation > failed.generation);
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    h.finish().await;
}
