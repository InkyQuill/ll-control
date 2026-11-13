//! Actual codec/store/settings owner and cooling/RGB actors, private HID only.
use llcontrol_daemon::{
    actor::{
        command_dispatch::{self, SettingsUpdate},
        control_group::{
            self, ControlGroupCommand, ControlGroupResult, ControlGroupState, ControlReapplyResult,
            ControlRevision,
        },
        cooling::{self, CoolingCommand},
        lighting_keeper, HidCommand, LightingStateMsg,
    },
    persistence::{DeviceSettings, LightingState, PersistenceStore, PumpLightingState},
    profile_archive::{PreparedGroup, ProfileArchives},
    profile_group::{GroupKind, GroupProfile, GroupRevision},
};
use llcontrol_ipc::*;
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
fn archives(store: &PersistenceStore) -> ProfileArchives {
    ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    )
}
fn fan(color: &str) -> LightingState {
    LightingState {
        mode: "static".into(),
        brightness: 80,
        speed: 50,
        direction: "forward".into(),
        color: color.into(),
        ..Default::default()
    }
}
fn prepared(target: &PersistenceStore, missing_pump: bool) -> Arc<PreparedGroup> {
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 65;
    settings.pump.pwm_percent = 73;
    settings.pump.source = "mcu".into();
    settings.lighting.fan = Some(fan("#ff0000"));
    if !missing_pump {
        settings.lighting.pump = Some(PumpLightingState {
            scope: "all".into(),
            mode: "static".into(),
            brightness: 85,
            speed: 50,
            direction: "forward".into(),
            color: "#0000ff".into(),
            ..Default::default()
        });
    }
    prepared_settings(target, &settings)
}
fn prepared_settings(target: &PersistenceStore, settings: &DeviceSettings) -> Arc<PreparedGroup> {
    let root = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(root.path().into());
    let mut profiles = ProfileLibrary::default();
    profiles
        .profiles
        .push(NamedProfile::defaults(ProfileGroup::Fan, 2, Some("cpu:test")).unwrap());
    profiles
        .profiles
        .push(NamedProfile::defaults(ProfileGroup::Pump, 1, Some("cpu:test")).unwrap());
    let profile = GroupProfile::capture(settings, &profiles, GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let mut bytes = Vec::new();
    archives(&source)
        .export_group(&profile, &mut bytes)
        .unwrap();
    Arc::new(
        archives(target)
            .prepare_group(&mut Cursor::new(bytes))
            .unwrap(),
    )
}
type WriteHistory = Arc<Mutex<Vec<(bool, Vec<u8>)>>>;
struct StageGate {
    entered: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}
struct Harness {
    root: tempfile::TempDir,
    store: Arc<PersistenceStore>,
    group: mpsc::Sender<ControlGroupCommand>,
    settings: mpsc::Sender<SettingsUpdate>,
    cooling: mpsc::Sender<CoolingCommand>,
    rgb: mpsc::Sender<LightingStateMsg>,
    writes: WriteHistory,
    fail: Arc<AtomicBool>,
    stop: watch::Sender<bool>,
    dispatch_stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    writer: JoinHandle<()>,
    io: JoinHandle<()>,
}
impl Harness {
    fn new(thermal: ThermalConfig) -> Self {
        Self::with_hooks(thermal, None, false)
    }
    fn with_hooks(
        thermal: ThermalConfig,
        install_gate: Option<StageGate>,
        lose_commit_reply: bool,
    ) -> Self {
        Self::with_commit_gate(thermal, install_gate, lose_commit_reply, None)
    }
    fn with_commit_gate(
        thermal: ThermalConfig,
        install_gate: Option<StageGate>,
        lose_commit_reply: bool,
        commit_gate: Option<StageGate>,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let mut initial = DeviceSettings::default();
        initial.lcd.brightness = 81;
        initial.thermal = thermal;
        store.write("device", &initial).unwrap();
        Self::start(root, initial, install_gate, lose_commit_reply, commit_gate)
    }
    fn restart(root: tempfile::TempDir) -> Self {
        let store = PersistenceStore::new_with_root(root.path().into());
        let initial = store.read("device").unwrap();
        Self::start(root, initial, None, false, None)
    }
    fn start(
        root: tempfile::TempDir,
        initial: DeviceSettings,
        install_gate: Option<StageGate>,
        lose_commit_reply: bool,
        commit_gate: Option<StageGate>,
    ) -> Self {
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let (settings, rx) = mpsc::channel(32);
        let (dispatch_stop, ds) = watch::channel(false);
        let writer = command_dispatch::spawn_updates(
            store.clone(),
            "device".into(),
            initial.clone(),
            rx,
            ds,
        );
        let (hid, mut requests) = mpsc::channel(16);
        let writes = Arc::new(Mutex::new(Vec::new()));
        let output = writes.clone();
        let disk = store.clone();
        let fail = Arc::new(AtomicBool::new(false));
        let injected = fail.clone();
        let io = tokio::spawn(async move {
            while let Some(HidCommand::Write { bytes, reply }) = requests.recv().await {
                let disk = disk.clone();
                let receipt = tokio::task::spawn_blocking(move || {
                    disk.read("device")
                        .map(|s| !s.group_imports.is_empty())
                        .unwrap_or(false)
                })
                .await
                .unwrap();
                output.lock().unwrap().push((receipt, bytes));
                let _ = reply.send(if injected.load(Ordering::SeqCst) {
                    Err(llcontrol_hid::Error::BlockingTaskFailed(
                        "injected group apply error".into(),
                    ))
                } else {
                    Ok(())
                });
            }
        });
        let (stop, sd) = watch::channel(false);
        let (cooling, cool_task) = cooling::spawn(
            cooling::CoolingInputs {
                hid: hid.clone(),
                settings: settings.clone(),
                host: Default::default(),
                device: Default::default(),
                initial: initial.cooling.clone(),
            },
            sd.clone(),
        );
        let (rgb, rx) = mpsc::channel(16);
        let light_task = lighting_keeper::spawn_with_profiles(
            hid,
            60000,
            initial.lighting.clone(),
            lighting_keeper::ThermalInputs {
                config: initial.thermal.clone(),
                dynamic: initial.dynamic_rgb.clone(),
                dynamic_profiles: initial.dynamic_rgb_profiles.clone(),
                ..Default::default()
            },
            rx,
            settings.clone(),
            sd.clone(),
        );
        let mut tasks = vec![cool_task, light_task];
        let control_rgb = if let Some(gate) = install_gate {
            let (proxy, mut messages) = mpsc::channel(16);
            let destination = rgb.clone();
            tasks.push(tokio::spawn(async move {
                let mut gate = Some(gate);
                while let Some(message) = messages.recv().await {
                    match message {
                        LightingStateMsg::InstallGroup { barrier, reply } if gate.is_some() => {
                            let (relay, done) = oneshot::channel();
                            destination
                                .send(LightingStateMsg::InstallGroup {
                                    barrier,
                                    reply: relay,
                                })
                                .await
                                .unwrap();
                            let result = done.await.unwrap();
                            let gate = gate.take().unwrap();
                            gate.entered.send(()).unwrap();
                            gate.release.await.unwrap();
                            let _ = reply.send(result);
                        }
                        message => {
                            if destination.send(message).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }));
            proxy
        } else {
            rgb.clone()
        };
        let control_settings = if lose_commit_reply || commit_gate.is_some() {
            let (proxy, mut messages) = mpsc::channel(16);
            let destination = settings.clone();
            tasks.push(tokio::spawn(async move {
                let mut commit_gate = commit_gate;
                while let Some(message) = messages.recv().await {
                    match message {
                        SettingsUpdate::CommitProfileGroup {
                            operation_id,
                            expected,
                            prepared,
                            reply,
                        } => {
                            let (relay, done) = oneshot::channel();
                            destination
                                .send(SettingsUpdate::CommitProfileGroup {
                                    operation_id,
                                    expected,
                                    prepared,
                                    reply: relay,
                                })
                                .await
                                .unwrap();
                            let result = done.await.unwrap();
                            assert!(result.is_ok());
                            if let Some(gate) = commit_gate.take() {
                                gate.entered.send(()).unwrap();
                                gate.release.await.unwrap();
                            }
                            if lose_commit_reply {
                                drop(reply);
                            } else {
                                let _ = reply.send(result);
                            }
                        }
                        message => {
                            if destination.send(message).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }));
            proxy
        } else {
            settings.clone()
        };
        let (group, group_task) =
            control_group::spawn(cooling.clone(), control_rgb, control_settings, sd);
        tasks.push(group_task);
        Self {
            root,
            store,
            group,
            settings,
            cooling,
            rgb,
            writes,
            fail,
            stop,
            dispatch_stop,
            tasks,
            writer,
            io,
        }
    }
    async fn revision(&self) -> GroupRevision {
        let (reply, rx) = oneshot::channel();
        self.settings
            .send(SettingsUpdate::ReadProfileGroup {
                kind: GroupKind::FanAndLighting,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap().unwrap().revision
    }
    async fn commit(
        &self,
        id: &str,
        expected: GroupRevision,
        prepared: Arc<PreparedGroup>,
    ) -> Result<ControlGroupResult> {
        let (reply, rx) = oneshot::channel();
        self.group
            .send(ControlGroupCommand::Commit {
                operation_id: id.into(),
                expected,
                prepared,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn reapply(&self, id: &str, expected: ControlRevision) -> Result<ControlReapplyResult> {
        let (reply, rx) = oneshot::channel();
        self.group
            .send(ControlGroupCommand::Reapply {
                operation_id: id.into(),
                expected,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    fn bytes(&self) -> (Vec<u8>, Vec<u8>) {
        (
            std::fs::read(self.root.path().join("devices/device.toml")).unwrap(),
            std::fs::read(self.root.path().join("profiles.json")).unwrap(),
        )
    }
    async fn state(&self) -> ControlGroupState {
        let (reply, rx) = oneshot::channel();
        self.group
            .send(ControlGroupCommand::State(reply))
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn manual(&self, percent: u8) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::Manual {
                target: CoolingTarget::Fan,
                percent,
                motherboard: false,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn finish(self) {
        drop(self.finish_keep().await);
    }
    async fn finish_keep(self) -> tempfile::TempDir {
        self.stop.send_replace(true);
        for task in self.tasks {
            task.await.unwrap();
        }
        self.dispatch_stop.send(true).ok();
        self.writer.await.unwrap();
        self.io.await.unwrap();
        self.root
    }
}
const ID: &str = "00112233445566778899aabbccddeeff";
#[tokio::test]
async fn group_commit_receipt_precedes_every_apply_preserves_neighbors_and_full_profile_slots() {
    let h = Harness::new(Default::default());
    let prepared = prepared(&h.store, false);
    let expected = h.revision().await;
    let result = h
        .commit(ID, expected.clone(), prepared.clone())
        .await
        .unwrap();
    assert!(!result.commit.repeated);
    assert!(!result.runtime.recovery_required);
    assert!(result.runtime.last_error.is_none(), "{:?}", result.runtime);
    assert_eq!(
        result.runtime.cooling.as_ref().unwrap().fan.applied_percent,
        Some(65)
    );
    assert_eq!(
        result
            .runtime
            .cooling
            .as_ref()
            .unwrap()
            .pump
            .applied_percent,
        Some(73)
    );
    {
        let history = h.writes.lock().unwrap();
        assert_eq!(history.len(), 4);
        assert!(
            history.iter().all(|(receipt, _)| *receipt),
            "HID must follow durable receipt"
        );
    }
    let saved = h.store.read("device").unwrap();
    assert_eq!(saved.lcd.brightness, 81);
    assert_eq!(saved.fan.pwm_percent, 65);
    assert_eq!(saved.pump.pwm_percent, 73);
    assert_eq!(saved.group_imports.len(), 1);
    let profiles = h.store.profiles().list().unwrap();
    assert_eq!(profiles.revision, expected.profile_revision + 1);
    assert!(profiles
        .profiles
        .iter()
        .any(|p| p.slot == 2 && p.content.group() == ProfileGroup::Fan));
    assert!(profiles
        .profiles
        .iter()
        .any(|p| p.slot == 1 && p.content.group() == ProfileGroup::Pump));
    let repeated = h.commit(ID, expected, prepared).await.unwrap();
    assert!(repeated.commit.repeated);
    assert_eq!(h.writes.lock().unwrap().len(), 4);
    assert_eq!(
        h.store.profiles().list().unwrap().revision,
        profiles.revision
    );
    h.finish().await;
}
#[tokio::test]
async fn lost_commit_reply_completes_and_duplicate_never_overwrites_newer_manual_intent() {
    let h = Harness::new(Default::default());
    let prepared = prepared(&h.store, false);
    let expected = h.revision().await;
    let (reply, rx) = oneshot::channel();
    h.group
        .send(ControlGroupCommand::Commit {
            operation_id: ID.into(),
            expected: expected.clone(),
            prepared: prepared.clone(),
            reply,
        })
        .await
        .unwrap();
    drop(rx);
    let state = h.state().await;
    assert_eq!(state.cooling.unwrap().fan.applied_percent, Some(65));
    h.manual(42).await.unwrap();
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::SetFan {
            state: fan("#00ff00"),
            reply,
        })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    let before = h.writes.lock().unwrap().len();
    let repeated = h.commit(ID, expected, prepared).await.unwrap();
    assert!(repeated.commit.repeated);
    assert!(!repeated.commit.readback.selected_content_current);
    assert_eq!(
        repeated.runtime.cooling.unwrap().fan.applied_percent,
        Some(42)
    );
    assert_eq!(h.writes.lock().unwrap().len(), before);
    h.finish().await;
}
#[tokio::test]
async fn preflight_rejects_incompatible_retained_global_alert_without_commit_or_output_and_releases_actors(
) {
    let thermal = ThermalConfig {
        cpu: Some(ThermalRule {
            sensor_id: "cpu:test".into(),
            threshold_c: 70.0,
            color: Rgb { r: 255, g: 0, b: 0 },
        }),
        gpu: None,
    };
    let h = Harness::new(thermal.clone());
    let p = prepared(&h.store, true);
    let expected = h.revision().await;
    assert!(h.commit(ID, expected, p).await.is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    let saved = h.store.read("device").unwrap();
    assert!(saved.group_imports.is_empty());
    assert_eq!(saved.thermal, thermal);
    assert_eq!(saved.lcd.brightness, 81);
    assert_eq!(h.store.profiles().list().unwrap().revision, 0);
    h.manual(45).await.unwrap();
    assert_eq!(
        h.state().await.cooling.unwrap().fan.applied_percent,
        Some(45)
    );
    h.finish().await;
}
#[tokio::test]
async fn apply_failure_is_distinct_from_committed_receipt_and_new_commands_can_reconcile_known_intent(
) {
    let h = Harness::new(Default::default());
    h.fail.store(true, Ordering::SeqCst);
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    let result = h.commit(ID, expected, p).await.unwrap();
    assert!(!result.commit.repeated);
    assert!(result.runtime.last_error.is_some());
    assert!(!result.runtime.recovery_required);
    let state = result.runtime.cooling.unwrap();
    assert!(state.fan.last_error.is_some() && state.pump.last_error.is_some());
    assert!(state.fan.applied_percent.is_none());
    assert_eq!(h.store.read("device").unwrap().group_imports.len(), 1);
    h.fail.store(false, Ordering::SeqCst);
    h.manual(51).await.unwrap();
    assert_eq!(
        h.state().await.cooling.unwrap().fan.applied_percent,
        Some(51)
    );
    h.finish().await;
}
#[tokio::test]
async fn unknown_storage_outcome_blocks_both_actors_and_stops_old_settings_flush() {
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    let initial_bytes = std::fs::read(h.root.path().join("devices/device.toml")).unwrap();
    std::fs::write(
        h.root.path().join("screen-reset-transaction.json"),
        "broken journal",
    )
    .unwrap();
    assert!(h.commit(ID, expected, p).await.is_err());
    let state = h.state().await;
    assert!(state.recovery_required);
    assert!(state.last_error.is_some());
    assert!(h.manual(45).await.is_err());
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::SetFan {
            state: fan("#00ff00"),
            reply,
        })
        .await
        .unwrap();
    assert!(rx.await.unwrap().is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(h.root.path().join("screen-reset-transaction.json")).unwrap(),
        "broken journal"
    );
    let root = h.finish_keep().await;
    assert_eq!(
        std::fs::read(root.path().join("devices/device.toml")).unwrap(),
        initial_bytes,
        "unknown journal must not trigger an old snapshot flush"
    );
}

#[tokio::test]
async fn first_apply_waits_for_both_install_acknowledgements_after_durable_receipt() {
    let (entered, ready) = oneshot::channel();
    let (release, hold) = oneshot::channel();
    let h = Harness::with_hooks(
        Default::default(),
        Some(StageGate {
            entered,
            release: hold,
        }),
        false,
    );
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    let (reply, rx) = oneshot::channel();
    h.group
        .send(ControlGroupCommand::Commit {
            operation_id: ID.into(),
            expected,
            prepared: p,
            reply,
        })
        .await
        .unwrap();
    ready.await.unwrap();
    assert_eq!(
        h.store.read("device").unwrap().group_imports.len(),
        1,
        "installation follows receipt"
    );
    assert!(
        h.writes.lock().unwrap().is_empty(),
        "neither actor may Apply before both install ACKs"
    );
    assert!(
        h.manual(45).await.is_err(),
        "barriers remain held between commit and Apply"
    );
    release.send(()).unwrap();
    let result = rx.await.unwrap().unwrap();
    assert!(result.runtime.last_error.is_none());
    assert_eq!(h.writes.lock().unwrap().len(), 4);
    h.finish().await;
}
#[tokio::test]
async fn lost_owner_commit_acknowledgement_keeps_both_frozen_without_apply_or_old_flush() {
    let h = Harness::with_hooks(Default::default(), None, true);
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    assert!(h.commit(ID, expected, p).await.is_err());
    let state = h.state().await;
    assert!(state.recovery_required);
    assert!(state.last_error.is_some());
    let saved = h.store.read("device").unwrap();
    assert_eq!(saved.group_imports.len(), 1);
    assert_eq!(saved.fan.pwm_percent, 65);
    assert_eq!(saved.pump.pwm_percent, 73);
    assert!(
        h.writes.lock().unwrap().is_empty(),
        "unacknowledged durable outcome cannot begin Apply"
    );
    assert!(h.manual(45).await.is_err());
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::SetFan {
            state: fan("#00ff00"),
            reply,
        })
        .await
        .unwrap();
    assert!(rx.await.unwrap().is_err());
    assert_eq!(
        h.revision().await.profile_revision,
        1,
        "owner remains alive with committed intent"
    );
    let disk = h.store.clone();
    let _root = h.finish_keep().await;
    let after = disk.read("device").unwrap();
    assert_eq!(after.fan.pwm_percent, 65);
    assert_eq!(
        after.group_imports.len(),
        1,
        "shutdown cannot flush pre-import settings over the committed receipt"
    );
}
#[tokio::test]
async fn stale_target_rejection_resumes_previous_runtime_and_allows_retry_with_fresh_revision() {
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    h.manual(44).await.unwrap();
    assert!(h.commit(ID, expected, p.clone()).await.is_err());
    assert!(h.store.read("device").unwrap().group_imports.is_empty());
    assert_eq!(
        h.state().await.cooling.unwrap().fan.applied_percent,
        Some(44)
    );
    h.manual(45).await.unwrap();
    let result = h.commit(ID, h.revision().await, p.clone()).await.unwrap();
    assert!(!result.commit.repeated);
    assert_eq!(
        result.runtime.cooling.unwrap().fan.applied_percent,
        Some(65)
    );
    let count = h.writes.lock().unwrap().len();
    assert!(
        h.commit(ID, h.revision().await, p).await.is_err(),
        "committed ID cannot be reused with a new expected revision"
    );
    assert_eq!(h.writes.lock().unwrap().len(), count);
    h.finish().await;
}

#[tokio::test]
async fn shutdown_after_commit_defers_apply_and_drains_queued_import_without_second_receipt() {
    let (entered, ready) = oneshot::channel();
    let (release, hold) = oneshot::channel();
    let h = Harness::with_commit_gate(
        Default::default(),
        None,
        false,
        Some(StageGate {
            entered,
            release: hold,
        }),
    );
    let p = prepared(&h.store, false);
    let expected = h.revision().await;
    let (reply, first) = oneshot::channel();
    h.group
        .send(ControlGroupCommand::Commit {
            operation_id: ID.into(),
            expected: expected.clone(),
            prepared: p.clone(),
            reply,
        })
        .await
        .unwrap();
    ready.await.unwrap();
    assert_eq!(h.store.read("device").unwrap().group_imports.len(), 1);
    let (reply, queued) = oneshot::channel();
    h.group
        .send(ControlGroupCommand::Commit {
            operation_id: "11111111111111111111111111111111".into(),
            expected,
            prepared: p,
            reply,
        })
        .await
        .unwrap();
    h.stop.send_replace(true);
    release.send(()).unwrap();
    let result = first.await.unwrap().unwrap();
    assert!(result
        .runtime
        .last_error
        .unwrap()
        .contains("deferred by shutdown"));
    assert!(queued.await.unwrap().is_err());
    assert!(
        h.writes.lock().unwrap().is_empty(),
        "teardown after durable commit must not start Apply"
    );
    let store = h.store.clone();
    let _root = h.finish_keep().await;
    let saved = store.read("device").unwrap();
    assert_eq!(saved.group_imports.len(), 1);
    assert_eq!(saved.group_imports[0].id, ID);
    assert_eq!(saved.fan.pwm_percent, 65);
    assert_eq!(store.profiles().list().unwrap().revision, 1);
}

#[tokio::test]
async fn explicit_reapply_recovers_known_apply_errors_without_reimport_settings_or_profile_changes()
{
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let original = h.revision().await;
    h.fail.store(true, Ordering::SeqCst);
    let result = h.commit(ID, original.clone(), p.clone()).await.unwrap();
    assert!(result.runtime.last_error.is_some());
    let expected = result.runtime.revision.unwrap();
    let bytes = h.bytes();
    let revision = h.store.profiles().list().unwrap().revision;
    h.fail.store(false, Ordering::SeqCst);
    let applied = h.reapply(ID, expected.clone()).await.unwrap();
    assert!(
        applied.runtime.last_error.is_none(),
        "{:?}",
        applied.runtime
    );
    assert_eq!(
        applied
            .runtime
            .cooling
            .as_ref()
            .unwrap()
            .fan
            .applied_percent,
        Some(65)
    );
    assert_eq!(
        applied
            .runtime
            .cooling
            .as_ref()
            .unwrap()
            .pump
            .applied_percent,
        Some(73)
    );
    assert_ne!(applied.runtime.revision, Some(expected.clone()));
    assert_eq!(applied.readback.receipt.id, ID);
    assert_eq!(h.bytes(), bytes);
    assert_eq!(h.store.profiles().list().unwrap().revision, revision);
    assert_eq!(h.store.read("device").unwrap().group_imports.len(), 1);
    let count = h.writes.lock().unwrap().len();
    assert!(
        h.reapply(ID, expected).await.is_err(),
        "completed reapply invalidates an old retry token"
    );
    assert_eq!(h.writes.lock().unwrap().len(), count);
    let repeated = h.commit(ID, original, p).await.unwrap();
    assert!(repeated.commit.repeated);
    assert_eq!(h.writes.lock().unwrap().len(), count);
    h.finish().await;
}

#[tokio::test]
async fn direct_led_override_invalidates_old_reapply_even_when_durable_group_hash_is_unchanged() {
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let result = h.commit(ID, h.revision().await, p).await.unwrap();
    let old = result.runtime.revision.unwrap();
    let before = h.revision().await;
    let bytes = h.bytes();
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::Direct {
            target: LedTarget::Fan,
            colors: vec![Rgb { r: 0, g: 0, b: 255 }],
            reply,
        })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    assert_eq!(h.revision().await, before);
    assert_eq!(h.bytes(), bytes, "Direct is intentionally not persisted");
    let count = h.writes.lock().unwrap().len();
    let error = h.reapply(ID, old).await.unwrap_err();
    assert!(error.to_string().contains("superseded"));
    assert_eq!(h.writes.lock().unwrap().len(), count);
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::Base {
            kind: LightingKind::Fan,
            reply,
        })
        .await
        .unwrap();
    assert!(
        rx.await.unwrap().current.is_none(),
        "rejected reapply must leave Direct source intact"
    );
    let current = h.state().await.revision.unwrap();
    let applied = h.reapply(ID, current).await.unwrap();
    assert!(applied.runtime.last_error.is_none());
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::Base {
            kind: LightingKind::Fan,
            reply,
        })
        .await
        .unwrap();
    assert_eq!(
        rx.await.unwrap().current.unwrap().color,
        Rgb { r: 255, g: 0, b: 0 }
    );
    assert_eq!(h.bytes(), bytes);
    h.finish().await;
}

#[tokio::test]
async fn fresh_runtime_token_cannot_reapply_a_receipt_superseded_by_saved_manual_or_rgb_edits() {
    for rgb in [false, true] {
        let h = Harness::new(Default::default());
        let p = prepared(&h.store, false);
        let result = h.commit(ID, h.revision().await, p).await.unwrap();
        let old = result.runtime.revision.unwrap();
        if rgb {
            let (reply, rx) = oneshot::channel();
            h.rgb
                .send(LightingStateMsg::SetFan {
                    state: fan("#00ff00"),
                    reply,
                })
                .await
                .unwrap();
            rx.await.unwrap().unwrap();
        } else {
            h.manual(42).await.unwrap();
        }
        let count = h.writes.lock().unwrap().len();
        assert!(h
            .reapply(ID, old)
            .await
            .unwrap_err()
            .to_string()
            .contains("superseded"));
        let current = h.state().await.revision.unwrap();
        let error = h.reapply(ID, current).await.unwrap_err();
        assert!(error.to_string().contains("no longer current"), "{error}");
        assert_eq!(h.writes.lock().unwrap().len(), count);
        if !rgb {
            assert_eq!(
                h.state().await.cooling.unwrap().fan.applied_percent,
                Some(42)
            );
        }
        h.finish().await;
    }
}

#[tokio::test]
async fn lost_reapply_reply_finishes_without_new_receipt_and_old_token_cannot_repeat_apply() {
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let result = h.commit(ID, h.revision().await, p).await.unwrap();
    let expected = result.runtime.revision.unwrap();
    let bytes = h.bytes();
    let count = h.writes.lock().unwrap().len();
    let (reply, rx) = oneshot::channel();
    h.group
        .send(ControlGroupCommand::Reapply {
            operation_id: ID.into(),
            expected: expected.clone(),
            reply,
        })
        .await
        .unwrap();
    drop(rx);
    let after = h.state().await;
    assert!(after.last_error.is_none());
    assert_ne!(after.revision, Some(expected.clone()));
    assert_eq!(h.writes.lock().unwrap().len(), count + 4);
    assert!(h.reapply(ID, expected).await.is_err());
    assert_eq!(h.writes.lock().unwrap().len(), count + 4);
    assert_eq!(h.bytes(), bytes);
    h.finish().await;
}

#[tokio::test]
async fn fresh_actors_reapply_current_durable_receipt_and_reject_pre_restart_revision_even_after_same_generation_count(
) {
    let h = Harness::new(Default::default());
    let p = prepared(&h.store, false);
    let result = h.commit(ID, h.revision().await, p).await.unwrap();
    let old = result.runtime.revision.unwrap();
    let root = h.finish_keep().await;
    let h = Harness::restart(root);
    let current = h.state().await.revision.unwrap();
    let bytes = h.bytes();
    let applied = h.reapply(ID, current).await.unwrap();
    assert!(applied.runtime.last_error.is_none());
    assert_eq!(
        applied.runtime.cooling.unwrap().fan.applied_percent,
        Some(65)
    );
    // Both instances have now installed once. The opaque instance IDs still
    // reject an ABA token from actors that were stopped before this lifetime.
    let count = h.writes.lock().unwrap().len();
    assert!(h
        .reapply(ID, old)
        .await
        .unwrap_err()
        .to_string()
        .contains("superseded"));
    assert_eq!(h.writes.lock().unwrap().len(), count);
    assert_eq!(h.bytes(), bytes);
    assert_eq!(h.store.profiles().list().unwrap().revision, 1);
    assert_eq!(h.store.read("device").unwrap().group_imports.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn missing_or_screen_receipt_cannot_drive_fan_reapply_and_rejection_releases_runtime() {
    for screen in [false, true] {
        let h = Harness::new(Default::default());
        if screen {
            let source_root = tempfile::tempdir().unwrap();
            let source = PersistenceStore::new_with_root(source_root.path().into());
            let profile = GroupProfile::capture(
                &DeviceSettings::default(),
                &ProfileLibrary::default(),
                GroupKind::Screen,
            )
            .unwrap()
            .profile;
            let mut bytes = Vec::new();
            archives(&source)
                .export_group(&profile, &mut bytes)
                .unwrap();
            let p = Arc::new(
                archives(&h.store)
                    .prepare_group(&mut Cursor::new(bytes))
                    .unwrap(),
            );
            let (reply, rx) = oneshot::channel();
            h.settings
                .send(SettingsUpdate::ReadProfileGroup {
                    kind: GroupKind::Screen,
                    reply,
                })
                .await
                .unwrap();
            let expected = rx.await.unwrap().unwrap().revision;
            let (reply, rx) = oneshot::channel();
            h.settings
                .send(SettingsUpdate::CommitProfileGroup {
                    operation_id: ID.into(),
                    expected,
                    prepared: p,
                    reply,
                })
                .await
                .unwrap();
            rx.await.unwrap().unwrap();
        }
        let expected = h.state().await.revision.unwrap();
        let error = h.reapply(ID, expected).await.unwrap_err();
        assert!(
            error.to_string().contains(if screen {
                "no longer current"
            } else {
                "receipt unavailable"
            }),
            "{error}"
        );
        assert!(h.writes.lock().unwrap().is_empty());
        h.manual(45).await.unwrap();
        assert_eq!(
            h.state().await.cooling.unwrap().fan.applied_percent,
            Some(45)
        );
        h.finish().await;
    }
}

#[tokio::test]
async fn group_archive_commit_and_reapply_preserve_explicit_zero_but_never_guess_legacy_unset_zero()
{
    for explicit in [false, true] {
        let h = Harness::new(Default::default());
        let mut settings = DeviceSettings::default();
        settings.fan.manual_zero = explicit;
        if explicit {
            settings.pump.source = "application".into();
        }
        let p = prepared_settings(&h.store, &settings);
        let result = h.commit(ID, h.revision().await, p).await.unwrap();
        let state = result.runtime.cooling.as_ref().unwrap();
        assert_eq!(state.fan.applied_percent, explicit.then_some(0));
        assert_eq!(state.pump.applied_percent, explicit.then_some(0));
        assert_eq!(state.pump.motherboard, explicit);
        let writes = h.writes.lock().unwrap().clone();
        assert_eq!(writes.len(), if explicit { 2 } else { 0 });
        assert!(writes.iter().all(|(receipt, _)| *receipt));
        assert_eq!(h.store.read("device").unwrap().fan.manual_zero, explicit);
        let bytes = h.bytes();
        let reapplied = h
            .reapply(ID, result.runtime.revision.unwrap())
            .await
            .unwrap();
        assert_eq!(
            reapplied.runtime.cooling.unwrap().fan.applied_percent,
            explicit.then_some(0)
        );
        assert_eq!(h.bytes(), bytes);
        if explicit {
            let packets = h.writes.lock().unwrap();
            assert!(packets
                .iter()
                .filter(|(_, p)| p[1] == 139)
                .all(|(_, p)| p[7] == 0));
            assert!(packets
                .iter()
                .filter(|(_, p)| p[1] == 138)
                .all(|(_, p)| p[6] == 1 && p[7] == 0));
        }
        h.finish().await;
    }
}
