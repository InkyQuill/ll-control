use lconnect_daemon::{
    actor::{
        command_dispatch::SettingsUpdate,
        cooling::{self, CoolingCommand, CoolingInputs},
        HidCommand,
    },
    sensors::SensorCache,
};
use lconnect_ipc::*;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, watch};

fn curve() -> CoolingPolicy {
    CoolingPolicy::Curve {
        sensor_id: "cpu:test".into(),
        points: vec![
            CurvePoint {
                celsius: 30.0,
                percent: 30,
            },
            CurvePoint {
                celsius: 70.0,
                percent: 100,
            },
        ],
        hysteresis_c: 2.0,
        fallback_percent: 100,
    }
}
fn row(value: f64) -> SensorReading {
    SensorReading {
        id: "cpu:test".into(),
        label: "test".into(),
        unit: SensorUnit::Celsius,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
#[test]
fn interpolation_rising_response_falling_hysteresis_and_fallback_recovery() {
    let p = curve();
    p.validate(CoolingTarget::Fan).unwrap();
    let mut anchor = None;
    assert_eq!(
        cooling::evaluate(&p, &[row(50.0)], &mut anchor),
        (65, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(49.0)], &mut anchor),
        (65, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(48.0)], &mut anchor),
        (62, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(60.0)], &mut anchor),
        (83, false, false)
    );
    assert_eq!(cooling::evaluate(&p, &[], &mut anchor), (100, false, true));
    assert_eq!(anchor, None);
    assert_eq!(
        cooling::evaluate(&p, &[row(30.0)], &mut anchor),
        (30, false, false)
    );
    let mut invalid = row(0.0);
    invalid.status = SensorStatus::Stale;
    assert_eq!(
        cooling::evaluate(&p, &[invalid], &mut anchor),
        (100, false, true)
    );
}
#[test]
fn policies_reject_dangerous_or_ambiguous_inputs() {
    assert!(CoolingPolicy::Fixed { percent: 19 }
        .validate(CoolingTarget::Pump)
        .is_err());
    assert!(CoolingPolicy::Motherboard { percent: 80 }
        .validate(CoolingTarget::Fan)
        .is_err());
    for (a, b) in [(30.0, 30.0), (70.0, 30.0), (f64::NAN, 70.0)] {
        let p = CoolingPolicy::Curve {
            sensor_id: "x".into(),
            points: vec![
                CurvePoint {
                    celsius: a,
                    percent: 30,
                },
                CurvePoint {
                    celsius: b,
                    percent: 100,
                },
            ],
            hysteresis_c: 2.0,
            fallback_percent: 100,
        };
        assert!(p.validate(CoolingTarget::Fan).is_err());
    }
    let mut p = curve();
    if let CoolingPolicy::Curve { points, .. } = &mut p {
        points[1].percent = 10;
    }
    assert!(p.validate(CoolingTarget::Fan).is_err());
}
struct Harness {
    tx: mpsc::Sender<CoolingCommand>,
    writes: mpsc::Receiver<Vec<u8>>,
    settings: mpsc::Receiver<SettingsUpdate>,
    cache: SensorCache,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    hid: tokio::task::JoinHandle<()>,
}
impl Harness {
    fn new(initial: CoolingConfig, fail: bool) -> Self {
        let (hidtx, mut hidrx) = mpsc::channel(16);
        let (wtx, writes) = mpsc::channel(64);
        let (stx, settings) = mpsc::channel(16);
        let hid = tokio::spawn(async move {
            while let Some(HidCommand::Write { bytes, reply }) = hidrx.recv().await {
                wtx.send(bytes).await.unwrap();
                let result = if fail {
                    Err(lconnect_hid::Error::BlockingTaskFailed("injected".into()))
                } else {
                    Ok(())
                };
                let _ = reply.send(result);
            }
        });
        let cache = SensorCache::default();
        cache.publish(Instant::now(), vec![row(50.0)]);
        let (stop, rx) = watch::channel(false);
        let (tx, task) = cooling::spawn(
            CoolingInputs {
                hid: hidtx,
                settings: stx,
                host: cache.clone(),
                device: Default::default(),
                initial,
            },
            rx,
        );
        Self {
            tx,
            writes,
            settings,
            cache,
            stop,
            task,
            hid,
        }
    }
    async fn set(&self, target: CoolingTarget, policy: CoolingPolicy) -> Result<CoolingState> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(CoolingCommand::Set {
                target,
                policy,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn state(&self) -> CoolingState {
        let (tx, rx) = oneshot::channel();
        self.tx.send(CoolingCommand::State(tx)).await.unwrap();
        rx.await.unwrap()
    }
    async fn write(&mut self) -> Vec<u8> {
        tokio::time::timeout(Duration::from_secs(3), self.writes.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn stop(self) {
        drop(self.stop);
        tokio::time::timeout(Duration::from_secs(3), self.task)
            .await
            .unwrap()
            .unwrap();
        self.hid.abort();
    }
}
#[tokio::test]
async fn curve_writes_fallback_on_stale_data_and_manual_override_cancels_future_ticks() {
    let mut h = Harness::new(Default::default(), false);
    h.set(CoolingTarget::Fan, curve()).await.unwrap();
    let packet = h.write().await;
    assert_eq!((packet[1], packet[7]), (139, 65));
    h.cache
        .publish(Instant::now() - Duration::from_secs(4), vec![row(50.0)]);
    let packet = h.write().await;
    assert_eq!(packet[7], 100);
    assert!(h.state().await.fan.fallback_active);
    let (reply, rx) = oneshot::channel();
    h.tx.send(CoolingCommand::Manual {
        target: CoolingTarget::Fan,
        percent: 60,
        motherboard: false,
        reply,
    })
    .await
    .unwrap();
    rx.await.unwrap().unwrap();
    assert_eq!(h.write().await[7], 60);
    assert!(h.state().await.fan.requested.is_none());
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(h.writes.try_recv().is_err());
    assert!(matches!(
        h.settings.recv().await.unwrap(),
        SettingsUpdate::Cooling(_)
    ));
    assert!(matches!(
        h.settings.recv().await.unwrap(),
        SettingsUpdate::CoolingManual { percent: 60, .. }
    ));
    h.stop().await;
}
#[tokio::test]
async fn rpm_binding_rejected_and_write_failure_retains_intent_without_applied_claim() {
    let mut h = Harness::new(Default::default(), true);
    let mut r = row(50.0);
    r.unit = SensorUnit::Rpm;
    h.cache.publish(Instant::now(), vec![r]);
    assert!(h.set(CoolingTarget::Fan, curve()).await.is_err());
    assert!(h.writes.try_recv().is_err());
    let state = h
        .set(
            CoolingTarget::Pump,
            CoolingPolicy::Motherboard { percent: 80 },
        )
        .await
        .unwrap();
    let packet = h.write().await;
    assert_eq!((packet[1], packet[6], packet[7]), (138, 1, 80));
    assert_eq!(state.pump.applied_percent, None);
    assert!(state.pump.last_error.is_some());
    assert!(state.pump.requested.is_some());
    h.stop().await;
}
#[tokio::test]
async fn persisted_curve_uses_fallback_before_first_sample_and_invalid_policy_does_not_write() {
    let mut h = Harness::new(
        CoolingConfig {
            fan: Some(curve()),
            pump: None,
        },
        false,
    );
    h.cache.publish(Instant::now(), vec![]);
    assert_eq!(h.write().await[7], 100);
    h.stop().await;
    let mut h = Harness::new(
        CoolingConfig {
            fan: None,
            pump: Some(CoolingPolicy::Fixed { percent: 0 }),
        },
        false,
    );
    assert!(h.state().await.pump.last_error.is_some());
    assert!(h.writes.try_recv().is_err());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn queued_manual_wins_after_a_slow_failed_policy_write() {
    let (hid, mut writes) = mpsc::channel(16);
    let (settings, _settings_rx) = mpsc::channel(16);
    let (stop, rx) = watch::channel(false);
    let (tx, task) = cooling::spawn(
        CoolingInputs {
            hid,
            settings,
            host: Default::default(),
            device: Default::default(),
            initial: CoolingConfig {
                fan: Some(CoolingPolicy::Fixed { percent: 70 }),
                pump: None,
            },
        },
        rx,
    );
    let Some(HidCommand::Write { bytes, reply }) = writes.recv().await else {
        panic!("first policy write")
    };
    assert_eq!(bytes[7], 70);
    let (manual_reply, manual_done) = oneshot::channel();
    tx.send(CoolingCommand::Manual {
        target: CoolingTarget::Fan,
        percent: 55,
        motherboard: false,
        reply: manual_reply,
    })
    .await
    .unwrap();
    tokio::time::advance(Duration::from_secs(10)).await;
    reply
        .send(Err(lconnect_hid::Error::BlockingTaskFailed(
            "slow failed write".into(),
        )))
        .unwrap();
    let Some(HidCommand::Write { bytes, reply }) = writes.recv().await else {
        panic!("manual write")
    };
    assert_eq!(
        bytes[7], 55,
        "overdue policy retry must not overtake manual"
    );
    reply.send(Ok(())).unwrap();
    manual_done.await.unwrap().unwrap();
    drop(stop);
    task.await.unwrap();
}

#[tokio::test]
async fn persisted_known_wrong_unit_is_disabled_with_visible_error() {
    let mut h = Harness::new(
        CoolingConfig {
            fan: Some(curve()),
            pump: None,
        },
        false,
    );
    let mut rpm = row(50.0);
    rpm.unit = SensorUnit::Rpm;
    h.cache.publish(Instant::now(), vec![rpm]);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let state = h.state().await;
    assert!(state
        .fan
        .last_error
        .as_deref()
        .unwrap()
        .contains("not a temperature"));
    assert!(state.fan.requested.is_some());
    assert!(h.writes.try_recv().is_err());
    h.stop().await;
}

#[test]
fn reference_rpm_mapping_and_two_stage_rounding() {
    let expected = [
        20, 23, 25, 27, 30, 31, 34, 36, 39, 41, 44, 47, 49, 52, 55, 58, 60, 64, 67, 70, 73, 77, 80,
        83, 87, 90, 94, 98, 100,
    ];
    for (i, percent) in expected.into_iter().enumerate() {
        assert_eq!(cooling::pump_rpm_to_percent(800 + i as u16 * 100), percent);
    }
    assert_eq!(cooling::pump_rpm_to_percent(850), 21);
    assert_eq!(cooling::pump_rpm_to_percent(2750), 71);
    let p = PumpPreset::Performance.policy("cpu:test".into(), 0.0, 100);
    // 41.9C -> 41C -> 2535RPM -> 65%. Flattened duty knots produce 66%.
    assert_eq!(
        cooling::evaluate(&p, &[row(41.9)], &mut None),
        (65, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(-20.0)], &mut None),
        (55, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(110.0)], &mut None),
        (100, false, false)
    );
    let quiet = PumpPreset::Quiet.policy("cpu:test".into(), 2.0, 100);
    assert_eq!(
        cooling::evaluate(&quiet, &[row(85.0)], &mut None),
        (80, false, false)
    );
    let mut anchor = None;
    assert_eq!(
        cooling::evaluate(&quiet, &[row(70.0)], &mut anchor),
        (67, false, false)
    );
    assert_eq!(
        cooling::evaluate(&quiet, &[row(69.0)], &mut anchor),
        (67, false, false)
    );
    assert_eq!(
        cooling::evaluate(&quiet, &[], &mut anchor),
        (100, false, true)
    );
    assert_eq!(anchor, None);
    assert_eq!(
        cooling::evaluate(&quiet, &[row(30.0)], &mut anchor),
        (49, false, false)
    );
}

#[test]
fn rpm_validation_rejects_fan_out_of_range_and_malformed_curves() {
    for rpm in [0, 799, 3601, u16::MAX] {
        assert!(CoolingPolicy::FixedRpm { rpm }
            .validate(CoolingTarget::Pump)
            .is_err());
    }
    assert!(CoolingPolicy::FixedRpm { rpm: 2200 }
        .validate(CoolingTarget::Fan)
        .is_err());
    let p = PumpPreset::Quiet.policy("cpu:test".into(), 0.0, 100);
    assert!(p.validate(CoolingTarget::Fan).is_err());
    for (points, hysteresis, fallback) in [
        (vec![], 2.0, 100),
        (
            vec![
                RpmCurvePoint {
                    celsius: 30,
                    rpm: 2000,
                },
                RpmCurvePoint {
                    celsius: 30,
                    rpm: 2200,
                },
            ],
            2.0,
            100,
        ),
        (
            vec![
                RpmCurvePoint {
                    celsius: 30,
                    rpm: 2200,
                },
                RpmCurvePoint {
                    celsius: 40,
                    rpm: 2000,
                },
            ],
            2.0,
            100,
        ),
        (
            vec![
                RpmCurvePoint {
                    celsius: 30,
                    rpm: 2000,
                },
                RpmCurvePoint {
                    celsius: 40,
                    rpm: 2200,
                },
            ],
            f64::NAN,
            100,
        ),
        (
            vec![
                RpmCurvePoint {
                    celsius: 30,
                    rpm: 2000,
                },
                RpmCurvePoint {
                    celsius: 40,
                    rpm: 2200,
                },
            ],
            2.0,
            19,
        ),
    ] {
        let p = CoolingPolicy::CurveRpm {
            sensor_id: "cpu:test".into(),
            points,
            hysteresis_c: hysteresis,
            fallback_percent: fallback,
        };
        assert!(p.validate(CoolingTarget::Pump).is_err());
    }
}

#[tokio::test]
async fn rpm_policy_uses_existing_mcu_write_path_and_retains_rpm_intent() {
    let mut h = Harness::new(Default::default(), false);
    let policy = PumpPreset::Performance.policy("cpu:test".into(), 2.0, 100);
    let state = h.set(CoolingTarget::Pump, policy.clone()).await.unwrap();
    assert_eq!(state.pump.requested, Some(policy));
    let packet = h.write().await;
    // 50C -> 2825RPM -> 74%.
    assert_eq!((packet[1], packet[6], packet[7]), (138, 0, 74));
    assert_eq!(state.pump.applied_percent, Some(74));
    let fixed = h
        .set(CoolingTarget::Pump, CoolingPolicy::FixedRpm { rpm: 2200 })
        .await
        .unwrap();
    assert_eq!(fixed.pump.applied_percent, Some(55));
    assert_eq!(h.write().await[7], 55);
    let mut wrong = row(50.0);
    wrong.unit = SensorUnit::Rpm;
    h.cache.publish(Instant::now(), vec![wrong]);
    assert!(h
        .set(
            CoolingTarget::Pump,
            PumpPreset::Quiet.policy("cpu:test".into(), 2.0, 100)
        )
        .await
        .is_err());
    assert!(h.writes.try_recv().is_err());
    h.stop().await;
}

#[test]
fn fan_presets_match_reference_knots_truncation_and_limits() {
    for (preset, points) in [
        (
            FanPreset::Quiet,
            [(25, 20), (45, 45), (65, 52), (80, 75), (90, 100)],
        ),
        (
            FanPreset::Standard,
            [(25, 20), (40, 50), (55, 62), (70, 75), (90, 100)],
        ),
        (
            FanPreset::High,
            [(25, 45), (35, 55), (50, 70), (70, 85), (85, 100)],
        ),
        (
            FanPreset::Full,
            [(25, 100), (40, 100), (55, 100), (70, 100), (90, 100)],
        ),
    ] {
        let policy = preset.policy("cpu:test".into(), 0.0, 100);
        policy.validate(CoolingTarget::Fan).unwrap();
        assert!(policy.validate(CoolingTarget::Pump).is_err());
        for (celsius, percent) in points {
            assert_eq!(
                cooling::evaluate(&policy, &[row(f64::from(celsius))], &mut None),
                (percent, false, false)
            );
        }
        assert_eq!(
            cooling::evaluate(&policy, &[row(-20.0)], &mut None),
            (points[0].1, false, false)
        );
        assert_eq!(
            cooling::evaluate(&policy, &[row(120.0)], &mut None),
            (100, false, false)
        );
    }
    let p = FanPreset::Quiet.policy("cpu:test".into(), 0.0, 100);
    assert_eq!(
        cooling::evaluate(&p, &[row(26.9)], &mut None),
        (21, false, false)
    );
    assert_eq!(
        cooling::evaluate(&p, &[row(46.9)], &mut None),
        (45, false, false)
    );
    let CoolingPolicy::FanCurve {
        sensor_id,
        points,
        hysteresis_c,
        fallback_percent,
    } = p
    else {
        panic!("fan curve")
    };
    let ordinary = CoolingPolicy::Curve {
        sensor_id,
        points,
        hysteresis_c,
        fallback_percent,
    };
    assert_eq!(
        cooling::evaluate(&ordinary, &[row(26.9)], &mut None),
        (23, false, false),
        "existing curve rounding unchanged"
    );
}
#[tokio::test(start_paused = true)]
async fn fan_reference_policy_writes_pwm_fallback_and_retains_serialized_intent() {
    let mut h = Harness::new(Default::default(), false);
    h.cache.publish(Instant::now(), vec![row(26.9)]);
    let policy = FanPreset::Quiet.policy("cpu:test".into(), 2.0, 100);
    let state = h.set(CoolingTarget::Fan, policy.clone()).await.unwrap();
    let packet = h.write().await;
    assert_eq!((packet[1], packet[7]), (139, 21));
    assert_eq!(state.fan.requested, Some(policy.clone()));
    h.cache
        .publish(Instant::now() - Duration::from_secs(4), vec![row(26.9)]);
    assert_eq!(h.write().await[7], 100);
    assert!(h.state().await.fan.fallback_active);
    let mut unrelated = row(26.9);
    unrelated.id = "cpu:other".into();
    h.cache.publish(Instant::now(), vec![unrelated]);
    tokio::time::advance(Duration::from_secs(2)).await;
    // The actor serves at most one command before an already-due tick.
    // Two state replies therefore confirm evaluation of the unrelated sample.
    h.state().await;
    assert!(h.state().await.fan.fallback_active);
    assert!(
        h.writes.try_recv().is_err(),
        "another sensor must not replace the binding"
    );
    h.cache.publish(Instant::now(), vec![row(26.9)]);
    assert_eq!(h.write().await[7], 21);
    let recovered = h.state().await.fan;
    assert!(!recovered.fallback_active);
    assert_eq!(recovered.requested, Some(policy));
    h.stop().await;
}
#[test]
fn fan_reference_curve_rejects_fractional_knots_and_roundtrips() {
    let p = FanPreset::Standard.policy("cpu:test".into(), 2.0, 100);
    let mut saved = lconnect_daemon::persistence::DeviceSettings::default();
    saved.cooling.fan = Some(p.clone());
    let text = toml::to_string(&saved).unwrap();
    let loaded: lconnect_daemon::persistence::DeviceSettings = toml::from_str(&text).unwrap();
    assert_eq!(loaded.cooling.fan, Some(p.clone()));
    let mut bad = p;
    let CoolingPolicy::FanCurve { points, .. } = &mut bad else {
        panic!("fan curve")
    };
    points[0].celsius = 25.5;
    assert!(bad.validate(CoolingTarget::Fan).is_err());
}
