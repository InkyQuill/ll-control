//! Real cooling/RGB actors with a private fake HID queue; never opens a device.
use lconnect_daemon::{
    actor::{
        command_dispatch::SettingsUpdate,
        cooling::{self, CoolingCommand},
        group_barrier::GroupBarrierOwner,
        lighting_keeper, HidCommand, LightingStateMsg,
    },
    persistence::LightingState,
    sensors::SensorCache,
};
use lconnect_ipc::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

fn base() -> LightingState {
    LightingState {
        mode: "static".into(),
        brightness: 80,
        speed: 50,
        direction: "forward".into(),
        color: "#00ff00".into(),
        ..Default::default()
    }
}
fn temperature(cache: &SensorCache, value: f64) {
    cache.publish(
        Instant::now(),
        vec![SensorReading {
            id: "cpu:test".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(value),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        }],
    );
}
struct Harness {
    cooling: mpsc::Sender<CoolingCommand>,
    rgb: mpsc::Sender<LightingStateMsg>,
    settings: mpsc::Receiver<SettingsUpdate>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail: Arc<AtomicBool>,
    host: SensorCache,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}
impl Harness {
    fn new(gate: Option<oneshot::Receiver<()>>) -> Self {
        Self::with_thermal(gate, Default::default())
    }
    fn with_thermal(gate: Option<oneshot::Receiver<()>>, config: ThermalConfig) -> Self {
        let (hid, mut requests) = mpsc::channel(16);
        let writes = Arc::new(Mutex::new(Vec::new()));
        let output = writes.clone();
        let fail = Arc::new(AtomicBool::new(false));
        let injected = fail.clone();
        let io = tokio::spawn(async move {
            let mut gate = gate;
            while let Some(HidCommand::Write { bytes, reply }) = requests.recv().await {
                output.lock().unwrap().push(bytes);
                if let Some(gate) = gate.take() {
                    gate.await.unwrap();
                }
                let _ = reply.send(if injected.load(Ordering::SeqCst) {
                    Err(lconnect_hid::Error::BlockingTaskFailed(
                        "injected group write failure".into(),
                    ))
                } else {
                    Ok(())
                });
            }
        });
        let (stop, shutdown) = watch::channel(false);
        let (settings_tx, settings) = mpsc::channel(32);
        let host = SensorCache::default();
        temperature(&host, 40.0);
        let (cooling, cool_task) = cooling::spawn(
            cooling::CoolingInputs {
                hid: hid.clone(),
                settings: settings_tx.clone(),
                host: host.clone(),
                device: Default::default(),
                initial: Default::default(),
            },
            shutdown.clone(),
        );
        let (rgb, rx) = mpsc::channel(16);
        let rgb_task = lighting_keeper::spawn_with_thermal(
            hid,
            100,
            None,
            None,
            lighting_keeper::ThermalInputs {
                config,
                host: host.clone(),
                ..Default::default()
            },
            rx,
            settings_tx,
            shutdown,
        );
        Self {
            cooling,
            rgb,
            settings,
            writes,
            fail,
            host,
            stop,
            tasks: vec![cool_task, rgb_task, io],
        }
    }
    async fn freeze(
        &self,
        owner: &GroupBarrierOwner,
    ) -> (oneshot::Receiver<Result<()>>, oneshot::Receiver<Result<()>>) {
        let (reply, a) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::FreezeGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        let (reply, b) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::FreezeGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        (a, b)
    }
    async fn frozen(&self, owner: &GroupBarrierOwner) {
        let (a, b) = self.freeze(owner).await;
        a.await.unwrap().unwrap();
        b.await.unwrap().unwrap();
    }
    async fn state(&self) -> CoolingState {
        let (reply, rx) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::State(reply))
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn rgb_state(&self) -> Option<Rgb> {
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::Base {
                kind: LightingKind::Fan,
                reply,
            })
            .await
            .unwrap();
        let result = rx.await.unwrap();
        result.current.map(|c| c.color)
    }
    async fn blocked(&self) {
        let (reply, rx) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::Manual {
                target: CoolingTarget::Fan,
                percent: 90,
                motherboard: false,
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::Set {
                target: CoolingTarget::Pump,
                policy: CoolingPolicy::Fixed { percent: 90 },
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        for kind in [LightingKind::Fan, LightingKind::Pump] {
            let (reply, rx) = oneshot::channel();
            self.rgb
                .send(LightingStateMsg::Clear { kind, reply })
                .await
                .unwrap();
            assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        }
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::SetFan {
                state: base(),
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::SetPump {
                state: Default::default(),
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::SetPumpWithDynamic {
                state: Default::default(),
                restore_dynamic: true,
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::Direct {
                target: LedTarget::Fan,
                colors: vec![Rgb { r: 1, g: 2, b: 3 }],
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::SetThermal {
                config: Default::default(),
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::SetDynamicRgb {
                config: None,
                reply,
            })
            .await
            .unwrap();
        assert!(matches!(rx.await.unwrap(), Err(IpcError::DeviceBusy(_))));
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::ThermalState(reply))
            .await
            .unwrap();
        rx.await.unwrap();
        let (reply, rx) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::DynamicRgbState(reply))
            .await
            .unwrap();
        rx.await.unwrap();
    }
    async fn prepare_group(
        &self,
        owner: &GroupBarrierOwner,
        group: Arc<lconnect_daemon::profile_group::FanAndLightingGroup>,
    ) -> (Result<()>, Result<()>) {
        let (reply, a) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::PrepareGroup {
                barrier: owner.ticket(),
                group: group.clone(),
                reply,
            })
            .await
            .unwrap();
        let (reply, b) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::PrepareGroup {
                barrier: owner.ticket(),
                group,
                reply,
            })
            .await
            .unwrap();
        (a.await.unwrap(), b.await.unwrap())
    }
    async fn install_group(&self, owner: &GroupBarrierOwner) -> (Result<()>, Result<()>) {
        let (reply, a) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::InstallGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        let (reply, b) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::InstallGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        (a.await.unwrap(), b.await.unwrap())
    }
    async fn apply_group(
        &self,
        owner: &GroupBarrierOwner,
    ) -> (
        Result<CoolingState>,
        Result<lconnect_daemon::actor::LightingRuntimeState>,
    ) {
        let (reply, a) = oneshot::channel();
        self.cooling
            .send(CoolingCommand::ApplyGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        let (reply, b) = oneshot::channel();
        self.rgb
            .send(LightingStateMsg::ApplyGroup {
                barrier: owner.ticket(),
                reply,
            })
            .await
            .unwrap();
        (a.await.unwrap(), b.await.unwrap())
    }
    async fn finish(self) {
        self.stop.send(true).unwrap();
        for task in self.tasks {
            task.await.unwrap();
        }
    }
}

#[tokio::test(start_paused = true)]
async fn both_barriers_join_prior_io_and_settings_stop_refresh_and_reject_every_mutation() {
    let (release, hold) = oneshot::channel();
    let mut h = Harness::new(Some(hold));
    let (reply, cooling) = oneshot::channel();
    h.cooling
        .send(CoolingCommand::Set {
            target: CoolingTarget::Fan,
            policy: CoolingPolicy::Curve {
                sensor_id: "cpu:test".into(),
                points: vec![
                    CurvePoint {
                        celsius: 40.0,
                        percent: 40,
                    },
                    CurvePoint {
                        celsius: 80.0,
                        percent: 80,
                    },
                ],
                hysteresis_c: 0.0,
                fallback_percent: 100,
            },
            reply,
        })
        .await
        .unwrap();
    let (reply, rgb) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::SetFan {
            state: base(),
            reply,
        })
        .await
        .unwrap();
    let owner = GroupBarrierOwner::new();
    let (mut a, mut b) = h.freeze(&owner).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        a.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(matches!(
        b.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    a.await.unwrap().unwrap();
    b.await.unwrap().unwrap();
    cooling.await.unwrap().unwrap();
    rgb.await.unwrap().unwrap();
    let deltas = [
        h.settings.try_recv().unwrap(),
        h.settings.try_recv().unwrap(),
    ];
    assert!(deltas
        .iter()
        .any(|d| matches!(d, SettingsUpdate::Cooling(_))));
    assert!(deltas
        .iter()
        .any(|d| matches!(d, SettingsUpdate::LightingFan(_))));
    let count = h.writes.lock().unwrap().len();
    temperature(&h.host, 80.0);
    tokio::time::advance(Duration::from_secs(15)).await;
    h.blocked().await;
    assert_eq!(h.state().await.fan.applied_percent, Some(40));
    assert_eq!(h.rgb_state().await, Some(Rgb { r: 0, g: 255, b: 0 }));
    assert_eq!(
        h.writes.lock().unwrap().len(),
        count,
        "neither policies nor RGB cadence may cross the barrier"
    );
    assert!(h.settings.try_recv().is_err());
    owner.resume();
    tokio::time::advance(Duration::from_secs(2)).await;
    // Read-only queries yield until the resumed sensor tick has completed.
    for _ in 0..20 {
        if h.state().await.fan.applied_percent == Some(80) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(h.state().await.fan.applied_percent, Some(80));
    assert!(h.writes.lock().unwrap().len() > count);
    assert!(
        h.settings.try_recv().is_err(),
        "resume is not a settings mutation"
    );
    h.finish().await;
}

#[tokio::test(start_paused = true)]
async fn abandoned_unsealed_owner_releases_both_actors_but_sealed_unknown_outcome_stays_blocked() {
    for sealed in [false, true] {
        let mut h = Harness::new(None);
        let mut owner = GroupBarrierOwner::new();
        h.frozen(&owner).await;
        h.frozen(&owner).await; // Idempotent acquisition by the same coordinator.
        let other = GroupBarrierOwner::new();
        let (a, b) = h.freeze(&other).await;
        assert!(a.await.unwrap().is_err());
        assert!(b.await.unwrap().is_err());
        drop(other);
        if sealed {
            owner.seal();
        }
        drop(owner);
        tokio::time::advance(Duration::from_secs(5)).await;
        if sealed {
            h.blocked().await;
            assert!(h.writes.lock().unwrap().is_empty());
            assert!(h.settings.try_recv().is_err());
        } else {
            let (reply, rx) = oneshot::channel();
            h.cooling
                .send(CoolingCommand::Manual {
                    target: CoolingTarget::Fan,
                    percent: 60,
                    motherboard: false,
                    reply,
                })
                .await
                .unwrap();
            rx.await.unwrap().unwrap();
            let (reply, rx) = oneshot::channel();
            h.rgb
                .send(LightingStateMsg::SetFan {
                    state: base(),
                    reply,
                })
                .await
                .unwrap();
            rx.await.unwrap().unwrap();
            assert_eq!(h.state().await.fan.applied_percent, Some(60));
            assert!(matches!(
                h.settings.recv().await.unwrap(),
                SettingsUpdate::CoolingManual { percent: 60, .. }
            ));
            assert!(matches!(
                h.settings.recv().await.unwrap(),
                SettingsUpdate::LightingFan(_)
            ));
        }
        h.finish().await; // Shutdown never waits for the barrier owner.
    }
}

#[tokio::test(start_paused = true)]
async fn late_released_freeze_ticket_cannot_freeze_an_actor_after_caller_cancellation() {
    let h = Harness::new(None);
    let owner = GroupBarrierOwner::new();
    let (a, b) = h.freeze(&owner).await;
    drop(owner); // Admission is queued but has not yet been polled.
    assert!(a.await.unwrap().is_err());
    assert!(b.await.unwrap().is_err());
    let (reply, rx) = oneshot::channel();
    h.cooling
        .send(CoolingCommand::Manual {
            target: CoolingTarget::Fan,
            percent: 60,
            motherboard: false,
            reply,
        })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    assert_eq!(h.state().await.fan.applied_percent, Some(60));
    h.finish().await;
}

fn candidate() -> Arc<lconnect_daemon::profile_group::FanAndLightingGroup> {
    use lconnect_daemon::{
        persistence::{DeviceSettings, PumpLightingState},
        profile_group::{GroupKind, GroupProfile, GroupSettings},
    };
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 65;
    settings.pump.pwm_percent = 73;
    settings.pump.source = "application".into();
    settings.cooling.fan = Some(CoolingPolicy::Curve {
        sensor_id: "cpu:test".into(),
        points: vec![
            CurvePoint {
                celsius: 40.0,
                percent: 40,
            },
            CurvePoint {
                celsius: 80.0,
                percent: 80,
            },
        ],
        hysteresis_c: 0.0,
        fallback_percent: 100,
    });
    settings.lighting.fan = Some(base());
    let mut inactive = base();
    inactive.mode = "breathing".into();
    inactive.color = "#ff0000".into();
    settings
        .lighting
        .fan_modes
        .insert("breathing".into(), inactive);
    settings.lighting.pump = Some(PumpLightingState {
        scope: "all".into(),
        mode: "static".into(),
        brightness: 85,
        speed: 50,
        direction: "forward".into(),
        color: "#0000ff".into(),
        ..Default::default()
    });
    settings.dynamic_rgb = Some(DynamicRgbConfig {
        sensor_id: "cpu:test".into(),
        unit: SensorUnit::Celsius,
        low_value: 30,
        high_value: 60,
        low_colors: vec![Rgb { r: 0, g: 0, b: 255 }],
        high_colors: vec![Rgb { r: 255, g: 0, b: 0 }],
        low_speed_percent: None,
        high_speed_percent: None,
        brightness_percent: None,
        direction: None,
    });
    settings
        .dynamic_rgb_profiles
        .remember("static", settings.dynamic_rgb.clone().unwrap())
        .unwrap();
    let GroupSettings::FanAndLighting(group) = GroupProfile::capture(
        &settings,
        &ProfileLibrary::default(),
        GroupKind::FanAndLighting,
    )
    .unwrap()
    .profile
    .content
    else {
        panic!()
    };
    Arc::new(*group)
}

#[tokio::test(start_paused = true)]
async fn staged_group_install_is_silent_and_apply_keeps_dynamic_sensor_policy_inactive_modes_and_manual_source(
) {
    let mut h = Harness::new(None);
    let owner = GroupBarrierOwner::new();
    let group = candidate();
    h.frozen(&owner).await;
    let (a, b) = h.prepare_group(&owner, group.clone()).await;
    a.unwrap();
    b.unwrap();
    assert!(h.writes.lock().unwrap().is_empty());
    assert!(
        h.state().await.fan.requested.is_none(),
        "prepare must not change active policy"
    );
    assert!(h.rgb_state().await.is_none());
    let (a, b) = h.install_group(&owner).await;
    a.unwrap();
    b.unwrap();
    assert_eq!(h.state().await.fan.requested, group.cooling.fan);
    assert!(h.state().await.fan.applied_percent.is_none());
    assert!(
        h.writes.lock().unwrap().is_empty(),
        "both actors must install before device apply"
    );
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::Base {
            kind: LightingKind::Fan,
            reply,
        })
        .await
        .unwrap();
    let installed = rx.await.unwrap();
    assert!(installed
        .profiles
        .iter()
        .any(|p| p.mode == "breathing" && p.color == Rgb { r: 255, g: 0, b: 0 }));
    let (a, b) = h.apply_group(&owner).await;
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.fan.applied_percent, Some(40));
    assert_eq!(a.pump.applied_percent, Some(73));
    assert!(a.pump.motherboard);
    assert!(a.fan.last_error.is_none() && a.pump.last_error.is_none());
    assert_eq!(b.dynamic.requested, group.dynamic_rgb);
    assert_eq!(b.dynamic.applied.as_ref().unwrap().value, 40);
    let old_colors = b.dynamic.applied.unwrap().colors;
    assert!(
        h.settings.try_recv().is_err(),
        "group installation/apply cannot re-persist imported settings"
    );
    let count = h.writes.lock().unwrap().len();
    temperature(&h.host, 80.0);
    tokio::time::advance(Duration::from_secs(5)).await;
    assert_eq!(h.state().await.fan.applied_percent, Some(40));
    assert_eq!(
        h.writes.lock().unwrap().len(),
        count,
        "apply must not release coordinator barrier"
    );
    owner.resume();
    tokio::time::advance(Duration::from_secs(2)).await;
    for _ in 0..20 {
        if h.state().await.fan.applied_percent == Some(80) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(h.state().await.fan.applied_percent, Some(80));
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::DynamicRgbState(reply))
        .await
        .unwrap();
    let live = rx.await.unwrap();
    assert_eq!(live.applied.as_ref().unwrap().value, 80);
    assert_ne!(live.applied.unwrap().colors, old_colors);
    assert!(h.settings.try_recv().is_err());
    h.finish().await;
}

#[tokio::test(start_paused = true)]
async fn group_preflight_rejects_wrong_units_and_stale_owners_before_install_or_output() {
    let mut h = Harness::new(None);
    let old = GroupBarrierOwner::new();
    h.frozen(&old).await;
    let (a, b) = h.prepare_group(&old, candidate()).await;
    a.unwrap();
    b.unwrap();
    let mut bad = (*candidate()).clone();
    bad.cooling.fan = Some(CoolingPolicy::Curve {
        sensor_id: "cpu:test".into(),
        points: vec![
            CurvePoint {
                celsius: 40.0,
                percent: 40,
            },
            CurvePoint {
                celsius: 80.0,
                percent: 80,
            },
        ],
        hysteresis_c: 0.0,
        fallback_percent: 100,
    });
    h.host.publish(
        Instant::now(),
        vec![SensorReading {
            id: "cpu:test".into(),
            label: "wrong rail".into(),
            unit: SensorUnit::Volts,
            value: Some(1.0),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        }],
    );
    let (a, b) = h.prepare_group(&old, Arc::new(bad)).await;
    assert!(a.is_err());
    assert!(b.is_err());
    let (a, b) = h.install_group(&old).await;
    assert!(a.is_err());
    assert!(b.is_err());
    assert!(h.writes.lock().unwrap().is_empty());
    temperature(&h.host, 40.0);
    let (a, b) = h.prepare_group(&old, candidate()).await;
    a.unwrap();
    b.unwrap();
    let stale = old.ticket();
    drop(old);
    let current = GroupBarrierOwner::new();
    h.frozen(&current).await;
    // A retained preparation from the released coordinator cannot be installed
    // under either its stale ticket or a new owner's valid current ticket.
    let (reply, rx) = oneshot::channel();
    h.cooling
        .send(CoolingCommand::InstallGroup {
            barrier: stale.clone(),
            reply,
        })
        .await
        .unwrap();
    assert!(rx.await.unwrap().is_err());
    let (reply, rx) = oneshot::channel();
    h.rgb
        .send(LightingStateMsg::InstallGroup {
            barrier: stale,
            reply,
        })
        .await
        .unwrap();
    assert!(rx.await.unwrap().is_err());
    let (a, b) = h.install_group(&current).await;
    assert!(a.is_err());
    assert!(b.is_err());
    let (a, b) = h.apply_group(&current).await;
    assert!(a.is_err());
    assert!(b.is_err());
    assert!(h.state().await.fan.requested.is_none());
    assert!(h.rgb_state().await.is_none());
    assert!(h.writes.lock().unwrap().is_empty());
    assert!(h.settings.try_recv().is_err());
    current.resume();
    h.finish().await;
}

#[tokio::test(start_paused = true)]
async fn group_apply_errors_preserve_installed_intent_and_retry_without_new_settings_or_install() {
    let mut h = Harness::new(None);
    let owner = GroupBarrierOwner::new();
    h.frozen(&owner).await;
    let (a, b) = h.prepare_group(&owner, candidate()).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.install_group(&owner).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.prepare_group(&owner, candidate()).await;
    assert!(a.is_err());
    assert!(
        b.is_err(),
        "an installed transaction cannot be restaged under the same ticket"
    );
    h.fail.store(true, Ordering::SeqCst);
    let (a, b) = h.apply_group(&owner).await;
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(a.fan.last_error.is_some() && a.pump.last_error.is_some());
    assert!(a.fan.applied_percent.is_none() && a.pump.applied_percent.is_none());
    assert!(b.thermal.fan.last_error.is_some() && b.thermal.pump.last_error.is_some());
    assert!(b.dynamic.applied.is_none());
    assert_eq!(a.fan.requested, candidate().cooling.fan);
    h.fail.store(false, Ordering::SeqCst);
    let (a, b) = h.apply_group(&owner).await;
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.fan.applied_percent, Some(40));
    assert_eq!(a.pump.applied_percent, Some(73));
    assert!(b.thermal.fan.last_error.is_none() && b.thermal.pump.last_error.is_none());
    assert!(b.dynamic.applied.is_some());
    assert!(h.settings.try_recv().is_err());
    owner.resume();
    h.finish().await;
}

#[tokio::test(start_paused = true)]
async fn group_replacement_retains_global_thermal_history_on_missing_samples_and_checks_new_bases()
{
    let config = ThermalConfig {
        cpu: Some(ThermalRule {
            sensor_id: "cpu:test".into(),
            threshold_c: 70.0,
            color: Rgb {
                r: 255,
                g: 255,
                b: 0,
            },
        }),
        gpu: None,
    };
    let mut h = Harness::with_thermal(None, config.clone());
    temperature(&h.host, 80.0);
    let owner = GroupBarrierOwner::new();
    h.frozen(&owner).await;
    let (a, b) = h.prepare_group(&owner, candidate()).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.install_group(&owner).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.apply_group(&owner).await;
    a.unwrap();
    let b = b.unwrap();
    assert_eq!(b.thermal.requested, config);
    assert_eq!(b.thermal.active_source, Some(ThermalSource::Cpu));
    assert!(b.dynamic.suppressed_by_thermal);
    owner.resume();
    let again = GroupBarrierOwner::new();
    h.frozen(&again).await;
    h.host.publish(Instant::now(), vec![]);
    let mut no_base = (*candidate()).clone();
    no_base.lighting.fan = None;
    let (_, b) = h.prepare_group(&again, Arc::new(no_base)).await;
    assert!(b.is_err(), "global thermal needs both replacement bases");
    let (a, b) = h.prepare_group(&again, candidate()).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.install_group(&again).await;
    a.unwrap();
    b.unwrap();
    let (a, b) = h.apply_group(&again).await;
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(a.fan.fallback_active);
    assert_eq!(a.fan.applied_percent, Some(100));
    assert_eq!(b.thermal.requested, config);
    assert_eq!(
        b.thermal.active_source,
        Some(ThermalSource::Cpu),
        "replacing RGB bases must not forget an active warning while samples are missing"
    );
    assert_eq!(
        b.thermal.fan.applied,
        Some(ThermalLayer::Alert {
            color: Rgb {
                r: 255,
                g: 255,
                b: 0
            }
        })
    );
    assert!(b.dynamic.suppressed_by_thermal);
    assert!(h.settings.try_recv().is_err());
    again.resume();
    h.finish().await;
}
