use llcontrol_daemon::{
    actor::{
        lighting_keeper::{self, ThermalInputs},
        thermal::Evaluator,
        HidCommand, LightingStateMsg,
    },
    persistence::{LightingState, PumpLightingState},
    sensors::SensorCache,
};
use llcontrol_ipc::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};
fn row(id: &str, value: f64) -> SensorReading {
    SensorReading {
        id: id.into(),
        label: id.into(),
        unit: SensorUnit::Celsius,
        value: Some(value),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
fn rule(id: &str, color: Rgb) -> ThermalRule {
    ThermalRule {
        sensor_id: id.into(),
        threshold_c: 70.0,
        color,
    }
}
const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };
fn config() -> ThermalConfig {
    ThermalConfig {
        cpu: Some(rule("cpu", RED)),
        gpu: Some(rule("gpu", BLUE)),
    }
}
fn fan() -> LightingState {
    LightingState {
        mode: "static".into(),
        brightness: 50,
        speed: 25,
        color: "#010203".into(),
        direction: "reverse".into(),
        additional_colors: Vec::new(),
        fan_topology: None,
        disabled: false,
    }
}
fn pump() -> PumpLightingState {
    PumpLightingState {
        scope: "outer".into(),
        mode: "static".into(),
        brightness: 50,
        speed: 25,
        color: "#040506".into(),
        direction: "reverse".into(),
        additional_colors: Vec::new(),
        disabled: false,
    }
}
#[test]
fn newest_trigger_wins_stale_retains_and_valid_cool_sample_removes() {
    let mut e = Evaluator::default();
    let c = config();
    assert_eq!(e.evaluate(&c, &[row("cpu", 69.9), row("gpu", 69.9)]), None);
    assert_eq!(
        e.evaluate(&c, &[row("cpu", 70.0), row("gpu", 69.9)]),
        Some((ThermalSource::Cpu, RED))
    );
    assert_eq!(
        e.evaluate(&c, &[row("cpu", 80.0), row("gpu", 70.0)]),
        Some((ThermalSource::Gpu, BLUE))
    );
    assert_eq!(e.evaluate(&c, &[]), Some((ThermalSource::Gpu, BLUE)));
    assert_eq!(e.degraded, vec![ThermalSource::Cpu, ThermalSource::Gpu]);
    assert_eq!(
        e.evaluate(&c, &[row("cpu", 80.0), row("gpu", 60.0)]),
        Some((ThermalSource::Cpu, RED))
    );
    assert_eq!(e.evaluate(&c, &[row("cpu", 60.0), row("gpu", 60.0)]), None);
    assert_eq!(
        e.evaluate(&c, &[row("cpu", 80.0), row("gpu", 80.0)]),
        Some((ThermalSource::Gpu, BLUE))
    );
    assert_eq!(e.evaluate(&ThermalConfig::default(), &[]), None);
}
struct Harness {
    tx: mpsc::Sender<LightingStateMsg>,
    cache: SensorCache,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail: Arc<AtomicBool>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    hid: tokio::task::JoinHandle<()>,
    persist: tokio::task::JoinHandle<()>,
    updates: Arc<Mutex<Vec<llcontrol_daemon::actor::command_dispatch::SettingsUpdate>>>,
}
impl Harness {
    fn new(initial: ThermalConfig, bases: bool) -> Self {
        Self::with_dynamic(initial, bases, None)
    }
    fn with_dynamic(
        initial: ThermalConfig,
        bases: bool,
        dynamic: Option<DynamicRgbConfig>,
    ) -> Self {
        Self::with_bases(initial, bases.then(fan), bases.then(pump), dynamic)
    }
    fn with_bases(
        initial: ThermalConfig,
        fan_base: Option<LightingState>,
        pump_base: Option<PumpLightingState>,
        dynamic: Option<DynamicRgbConfig>,
    ) -> Self {
        let (hidtx, mut hidrx) = mpsc::channel(32);
        let writes = Arc::new(Mutex::new(Vec::new()));
        let w = writes.clone();
        let fail = Arc::new(AtomicBool::new(false));
        let f = fail.clone();
        let hid = tokio::spawn(async move {
            while let Some(HidCommand::Write { bytes, reply }) = hidrx.recv().await {
                assert!(
                    matches!(bytes[1], 131 | 133 | 20),
                    "no cooling or LCD side effects"
                );
                let error = matches!(bytes[1], 131 | 20) && f.load(Ordering::Relaxed);
                w.lock().unwrap().push(bytes);
                let _ = reply.send(if error {
                    Err(llcontrol_hid::Error::BlockingTaskFailed(
                        "injected pump failure".into(),
                    ))
                } else {
                    Ok(())
                });
            }
        });
        let (settings, mut rx) = mpsc::channel(32);
        let updates = Arc::new(Mutex::new(Vec::new()));
        let u = updates.clone();
        let persist = tokio::spawn(async move {
            while let Some(v) = rx.recv().await {
                u.lock().unwrap().push(v);
            }
        });
        let cache = SensorCache::default();
        cache.publish(Instant::now(), vec![row("cpu", 80.0), row("gpu", 60.0)]);
        let (tx, rx) = mpsc::channel(16);
        let (stop, shutdown) = watch::channel(false);
        let task = lighting_keeper::spawn_with_thermal(
            hidtx,
            100,
            fan_base,
            pump_base,
            ThermalInputs {
                dynamic_profiles: Default::default(),
                dynamic,
                config: initial,
                host: cache.clone(),
                device: Default::default(),
            },
            rx,
            settings,
            shutdown,
        );
        Self {
            tx,
            cache,
            writes,
            fail,
            stop,
            task,
            hid,
            persist,
            updates,
        }
    }
    async fn set(&self, c: ThermalConfig) -> Result<ThermalState> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::SetThermal { config: c, reply })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn state(&self) -> ThermalState {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::ThermalState(reply))
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn tick(&self, d: Duration) {
        tokio::time::advance(d).await;
        for _ in 0..30 {
            tokio::task::yield_now().await;
        }
    }
    async fn manual(&self) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        let mut f = fan();
        f.color = "#abcdef".into();
        self.tx
            .send(LightingStateMsg::SetFan { state: f, reply })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn stop(self) {
        self.stop.send(true).unwrap();
        self.task.await.unwrap();
        self.hid.await.unwrap();
        self.persist.await.unwrap();
    }
}
#[tokio::test(start_paused = true)]
async fn overlay_grace_restart_latest_base_restore_and_persistence() {
    let h = Harness::new(Default::default(), true);
    let s = h.set(config()).await.unwrap();
    assert_eq!(s.active_source, Some(ThermalSource::Cpu));
    assert_eq!(s.fan.applied, Some(ThermalLayer::Alert { color: RED }));
    h.manual().await.unwrap();
    let s = h.state().await;
    assert_eq!(s.fan.applied, Some(ThermalLayer::Base));
    assert_eq!(s.pump.applied, Some(ThermalLayer::Base));
    h.tick(Duration::from_secs(9)).await;
    assert!(h.state().await.grace_remaining_ms > 0);
    h.manual().await.unwrap();
    h.tick(Duration::from_secs(9)).await;
    assert_eq!(h.state().await.fan.applied, Some(ThermalLayer::Base));
    h.tick(Duration::from_secs(2)).await;
    assert_eq!(
        h.state().await.pump.applied,
        Some(ThermalLayer::Alert { color: RED })
    );
    let s = h.set(Default::default()).await.unwrap();
    assert_eq!(s.fan.applied, Some(ThermalLayer::Base));
    assert_eq!(s.pump.applied, Some(ThermalLayer::Base));
    {
        let w = h.writes.lock().unwrap();
        let last = w.iter().rev().find(|p| p[1] == 133).unwrap();
        assert!(last.windows(3).any(|v| v == [0xab, 0xcd, 0xef]));
    }
    {
        let u = h.updates.lock().unwrap();
        assert!(u.iter().any(|u|matches!(u,llcontrol_daemon::actor::command_dispatch::SettingsUpdate::Thermal(c) if c==&config())));
        assert!(u
            .iter()
            .filter_map(|u| match u {
                llcontrol_daemon::actor::command_dispatch::SettingsUpdate::LightingFan(s) =>
                    Some(s),
                _ => None,
            })
            .all(|s| s.color == "#abcdef"));
    }
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn partial_failure_retry_stale_recovery_and_direct_restore_failure() {
    let h = Harness::new(Default::default(), true);
    h.fail.store(true, Ordering::Relaxed);
    let s = h.set(config()).await.unwrap();
    assert!(s.fan.last_error.is_none());
    assert!(s.pump.last_error.is_some());
    assert!(s.pump.applied.is_none());
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::Direct {
        target: LedTarget::Fan,
        colors: vec![RED],
        reply,
    })
    .await
    .unwrap();
    assert!(rx.await.unwrap().is_err());
    h.fail.store(false, Ordering::Relaxed);
    h.tick(Duration::from_secs(1)).await;
    assert!(h.state().await.pump.last_error.is_none());
    h.cache.publish(
        Instant::now() - Duration::from_secs(4),
        vec![row("cpu", 10.0)],
    );
    h.tick(Duration::from_secs(1)).await;
    let s = h.state().await;
    assert_eq!(s.active_source, Some(ThermalSource::Cpu));
    assert!(s.degraded_sources.contains(&ThermalSource::Cpu));
    h.cache
        .publish(Instant::now(), vec![row("cpu", 10.0), row("gpu", 10.0)]);
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.state().await.fan.applied, Some(ThermalLayer::Base));
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn bad_binding_and_invalid_persisted_config_fail_closed() {
    let h = Harness::new(Default::default(), true);
    let mut wrong = row("cpu", 80.0);
    wrong.unit = SensorUnit::Percent;
    h.cache.publish(Instant::now(), vec![wrong]);
    assert!(h.set(config()).await.is_err());
    h.stop().await;
    let mut c = config();
    c.cpu.as_mut().unwrap().threshold_c = -1.0;
    let h = Harness::new(c, true);
    h.tick(Duration::from_secs(1)).await;
    assert!(h.state().await.blocked_reason.is_some());
    assert_eq!(h.state().await.active_source, None);
    h.stop().await;
}
#[test]
fn persistence_roundtrip_and_legacy_defaults() {
    let s = llcontrol_daemon::persistence::DeviceSettings {
        thermal: config(),
        ..Default::default()
    };
    let text = toml::to_string(&s).unwrap();
    let r: llcontrol_daemon::persistence::DeviceSettings = toml::from_str(&text).unwrap();
    assert_eq!(r.thermal, s.thermal);
    let legacy: llcontrol_daemon::persistence::DeviceSettings =
        toml::from_str("schema_version=1").unwrap();
    assert!(!legacy.thermal.enabled());
}

#[tokio::test(start_paused = true)]
async fn replay_activates_saved_rules_and_manual_scope_change_restores_old_scope_first() {
    let h = Harness::new(config(), true);
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.state().await.active_source, Some(ThermalSource::Cpu));
    h.writes.lock().unwrap().clear();
    let (reply, rx) = oneshot::channel();
    let mut new = pump();
    new.scope = "inner".into();
    h.tx.send(LightingStateMsg::SetPump { state: new, reply })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    {
        let writes = h.writes.lock().unwrap();
        let pump_writes: Vec<_> = writes.iter().filter(|w| w[1] == 131).collect();
        assert_eq!(
            pump_writes.len(),
            2,
            "restore old scope before applying new scope"
        );
        assert_ne!(pump_writes[0][6], pump_writes[1][6]);
    }
    h.tick(Duration::from_secs(11)).await;
    assert!(matches!(
        h.state().await.pump.applied,
        Some(ThermalLayer::Alert { .. })
    ));
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn identical_config_preserves_priority_and_warning_while_stale() {
    let h = Harness::new(Default::default(), true);
    h.set(config()).await.unwrap();
    h.cache
        .publish(Instant::now(), vec![row("cpu", 60.0), row("gpu", 80.0)]);
    h.tick(Duration::from_secs(1)).await;
    h.cache
        .publish(Instant::now(), vec![row("cpu", 80.0), row("gpu", 80.0)]);
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.state().await.active_source, Some(ThermalSource::Cpu));
    assert_eq!(
        h.set(config()).await.unwrap().active_source,
        Some(ThermalSource::Cpu)
    );
    h.cache.publish(
        Instant::now() - Duration::from_secs(4),
        vec![row("cpu", 1.0), row("gpu", 1.0)],
    );
    let s = h.set(config()).await.unwrap();
    assert_eq!(s.active_source, Some(ThermalSource::Cpu));
    assert_eq!(s.fan.applied, Some(ThermalLayer::Alert { color: RED }));
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn ordinary_first_direct_failure_without_base_allows_direct_retry() {
    let h = Harness::new(Default::default(), false);
    h.fail.store(true, Ordering::Relaxed);
    for should_fail in [true, false] {
        h.fail.store(should_fail, Ordering::Relaxed);
        let (reply, rx) = oneshot::channel();
        h.tx.send(LightingStateMsg::Direct {
            target: LedTarget::Screen,
            colors: vec![RED],
            reply,
        })
        .await
        .unwrap();
        assert_eq!(rx.await.unwrap().is_err(), should_fail);
    }
    assert_eq!(h.state().await.pump.applied, Some(ThermalLayer::Base));
    h.stop().await;
}

impl Harness {
    async fn direct(&self, target: LedTarget, colors: Vec<Rgb>) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::Direct {
                target,
                colors,
                reply,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    fn last_direct(&self, target: LedTarget) -> Vec<u8> {
        self.writes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|p| p[1] == 20 && p[11] == u8::from(target == LedTarget::Fan))
            .unwrap()
            .clone()
    }
}
fn direct_packet(target: LedTarget, colors: &[Rgb]) -> Vec<u8> {
    use llcontrol_protocol::galahad2_lcdv2::{set_led_colors::*, Rgb as P};
    let colors: Vec<_> = colors
        .iter()
        .map(|c| P {
            r: c.r,
            g: c.g,
            b: c.b,
        })
        .collect();
    match target {
        LedTarget::Fan => build_set_fan_led_colors_request(&colors),
        _ => build_set_screen_led_colors_request(&colors),
    }
    .unwrap()[0]
        .to_vec()
}
#[tokio::test(start_paused = true)]
async fn direct_alert_restores_exact_prefix_and_short_updates_keep_known_tail() {
    let h = Harness::new(Default::default(), true);
    for target in [LedTarget::Fan, LedTarget::Screen] {
        h.direct(target, vec![BLUE, RED, BLUE]).await.unwrap();
    }
    let state = h.set(config()).await.unwrap();
    assert_eq!(state.active_source, Some(ThermalSource::Cpu));
    assert_eq!(state.fan.direct_led_count, Some(3));
    assert_eq!(state.pump.direct_led_count, Some(3));
    for target in [LedTarget::Fan, LedTarget::Screen] {
        assert_eq!(h.last_direct(target), direct_packet(target, &[RED; 3]));
    }
    h.writes.lock().unwrap().clear();
    h.direct(LedTarget::Screen, vec![BLUE]).await.unwrap();
    assert_eq!(
        h.last_direct(LedTarget::Screen),
        direct_packet(LedTarget::Screen, &[BLUE, RED, BLUE])
    );
    assert_eq!(h.state().await.pump.applied, Some(ThermalLayer::Base));
    assert!(
        h.writes.lock().unwrap().iter().all(|p| p[1] == 20),
        "no source-mode reset on ordinary direct updates/restore"
    );
    h.tick(Duration::from_secs(11)).await;
    assert_eq!(
        h.last_direct(LedTarget::Screen),
        direct_packet(LedTarget::Screen, &[RED; 3])
    );
    h.set(Default::default()).await.unwrap();
    for target in [LedTarget::Fan, LedTarget::Screen] {
        assert_eq!(
            h.last_direct(target),
            direct_packet(target, &[BLUE, RED, BLUE])
        );
    }
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn direct_transition_during_alert_gets_grace_and_failed_restore_can_retry() {
    let h = Harness::new(Default::default(), true);
    h.set(config()).await.unwrap();
    h.direct(LedTarget::Fan, vec![BLUE, BLUE]).await.unwrap();
    assert!(h.state().await.grace_remaining_ms > 0);
    assert!(h
        .writes
        .lock()
        .unwrap()
        .iter()
        .any(|p| p[1] == 133 && p[23] == 2));
    h.tick(Duration::from_secs(11)).await;
    h.fail.store(true, Ordering::Relaxed);
    let s = h.set(Default::default()).await.unwrap();
    assert!(s.fan.last_error.is_some());
    h.fail.store(false, Ordering::Relaxed);
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.state().await.fan.applied, Some(ThermalLayer::Base));
    assert_eq!(
        h.last_direct(LedTarget::Fan),
        direct_packet(LedTarget::Fan, &[BLUE, BLUE])
    );
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn oversized_or_empty_direct_update_cannot_replace_retained_base() {
    let h = Harness::new(Default::default(), true);
    h.direct(LedTarget::Screen, vec![BLUE, RED]).await.unwrap();
    for colors in [vec![], vec![RED; 330]] {
        assert!(h.direct(LedTarget::Screen, colors).await.is_err());
    }
    h.set(config()).await.unwrap();
    h.set(Default::default()).await.unwrap();
    assert_eq!(
        h.last_direct(LedTarget::Screen),
        direct_packet(LedTarget::Screen, &[BLUE, RED])
    );
    h.stop().await;
}

fn dynamic_config() -> DynamicRgbConfig {
    DynamicRgbConfig {
        sensor_id: "gpu".into(),
        unit: SensorUnit::Celsius,
        low_value: 30,
        high_value: 60,
        low_colors: vec![RED],
        high_colors: vec![BLUE],
        low_speed_percent: None,
        high_speed_percent: None,
        brightness_percent: None,
        direction: None,
    }
}
impl Harness {
    async fn dynamic(&self, config: Option<DynamicRgbConfig>) -> Result<DynamicRgbState> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::SetDynamicRgb { config, reply })
            .await
            .unwrap();
        rx.await.unwrap()
    }
    async fn dynamic_state(&self) -> DynamicRgbState {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(LightingStateMsg::DynamicRgbState(reply))
            .await
            .unwrap();
        rx.await.unwrap()
    }
    fn pump_color(&self) -> Vec<u8> {
        self.writes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|p| p[1] == 131)
            .unwrap()[10..13]
            .to_vec()
    }
}
#[tokio::test(start_paused = true)]
async fn sensor_rgb_thermal_priority_restore_manual_cancel_and_stale_fallback() {
    let h = Harness::new(Default::default(), true);
    h.cache
        .publish(Instant::now(), vec![row("cpu", 80.0), row("gpu", 45.0)]);
    let state = h.dynamic(Some(dynamic_config())).await.unwrap();
    assert_eq!(state.applied.unwrap().colors, [Rgb { r: 0, g: 255, b: 0 }]);
    assert_eq!(h.pump_color(), [0, 255, 0]);
    h.set(config()).await.unwrap();
    assert!(h.dynamic_state().await.suppressed_by_thermal);
    assert_eq!(h.pump_color(), [255, 0, 0]);
    h.manual().await.unwrap(); // Fan edit grants thermal grace without disabling pump policy.
    assert!(h.dynamic_state().await.requested.is_some());
    assert_eq!(h.pump_color(), [0, 255, 0]);
    h.set(Default::default()).await.unwrap();
    h.cache.publish(Instant::now(), vec![row("gpu", 60.0)]);
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.pump_color(), [0, 0, 255]);
    h.cache.publish(
        Instant::now() - Duration::from_secs(4),
        vec![row("gpu", 45.0)],
    );
    h.tick(Duration::from_secs(1)).await;
    assert!(h.dynamic_state().await.degraded);
    assert_eq!(h.pump_color(), [4, 5, 6]);
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::SetPump {
        state: pump(),
        reply,
    })
    .await
    .unwrap();
    rx.await.unwrap().unwrap();
    assert!(h.dynamic_state().await.requested.is_none());
    h.tick(Duration::from_secs(1)).await;
    assert!(h.updates.lock().unwrap().iter().any(|u| matches!(
        u,
        llcontrol_daemon::actor::command_dispatch::SettingsUpdate::LightingPumpDynamic {
            config: None,
            ..
        }
    )));
    h.stop().await;
}
#[tokio::test(start_paused = true)]
async fn sensor_rgb_four_wire_colors_speed_retry_and_mode_validation() {
    let h = Harness::new(Default::default(), true);
    let mut config = dynamic_config();
    config.low_colors = vec![RED; 4];
    config.high_colors = vec![BLUE; 4];
    config.low_speed_percent = Some(0);
    config.high_speed_percent = Some(100);
    assert!(h.dynamic(Some(config.clone())).await.is_err()); // Static mode supports one color/no speed.
    let (reply, rx) = oneshot::channel();
    let mut base = pump();
    base.mode = "meteor".into();
    h.tx.send(LightingStateMsg::SetPump { state: base, reply })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    h.cache.publish(Instant::now(), vec![row("gpu", 45.0)]);
    h.fail.store(true, Ordering::Relaxed);
    let state = h.dynamic(Some(config)).await.unwrap();
    assert!(state.applied.is_none());
    assert!(state.last_error.is_some());
    h.fail.store(false, Ordering::Relaxed);
    h.tick(Duration::from_secs(1)).await;
    assert!(h.dynamic_state().await.last_error.is_none());
    {
        let writes = h.writes.lock().unwrap();
        let packet = writes.iter().rev().find(|p| p[1] == 131).unwrap();
        assert_eq!(packet[9], 2); // 50% => wire speed2.
        assert_eq!(
            &packet[10..22],
            &[0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0]
        );
    }
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::Direct {
        target: LedTarget::Screen,
        colors: vec![RED],
        reply,
    })
    .await
    .unwrap();
    rx.await.unwrap().unwrap();
    assert!(h.dynamic_state().await.requested.is_none());
    h.stop().await;
}
#[test]
fn sensor_rgb_policy_survives_persistence_without_changing_manual_base() {
    let dir = tempfile::tempdir().unwrap();
    let store = llcontrol_daemon::persistence::PersistenceStore::new_with_root(dir.path().into());
    let mut settings = llcontrol_daemon::persistence::DeviceSettings {
        dynamic_rgb: Some(dynamic_config()),
        ..Default::default()
    };
    settings.lighting.pump = Some(pump());
    store.write("test", &settings).unwrap();
    let loaded = store.read("test").unwrap();
    assert_eq!(loaded.dynamic_rgb, Some(dynamic_config()));
    assert_eq!(loaded.lighting.pump.unwrap().color, "#040506");
}

#[tokio::test(start_paused = true)]
async fn sensor_rgb_replay_retains_binding_and_recovers_when_source_returns() {
    let h = Harness::with_dynamic(Default::default(), true, Some(dynamic_config()));
    h.tick(Duration::from_secs(1)).await;
    assert_eq!(h.pump_color(), [0, 0, 255]);
    h.cache
        .publish(Instant::now(), vec![row("unrelated", 45.0)]);
    h.tick(Duration::from_secs(1)).await;
    let state = h.dynamic_state().await;
    assert_eq!(state.requested.unwrap().sensor_id, "gpu");
    assert!(state.degraded);
    assert!(state.applied.is_none());
    assert_eq!(h.pump_color(), [4, 5, 6]);
    h.cache.publish(Instant::now(), vec![row("gpu", 30.0)]);
    h.tick(Duration::from_secs(1)).await;
    assert!(!h.dynamic_state().await.degraded);
    assert_eq!(h.pump_color(), [255, 0, 0]);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn thermal_temporarily_disables_fan_sync_and_restores_global_topology() {
    let h = Harness::new(Default::default(), true);
    let mut state = fan();
    state.fan_topology = Some(FanLightingTopology {
        sync_to_pump: true,
        size: FanLedSize::Mm280,
        manual_led_count: 31,
    });
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::SetFan { state, reply })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    let last = || {
        h.writes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|p| p[1] == 133)
            .unwrap()
            .clone()
    };
    assert_eq!((last()[24], last()[25]), (1, 16));
    h.set(config()).await.unwrap();
    assert_eq!((last()[24], last()[25]), (0, 16));
    h.set(Default::default()).await.unwrap();
    assert_eq!((last()[24], last()[25]), (1, 16));
    h.manual().await.unwrap();
    assert_eq!(
        (last()[24], last()[25]),
        (1, 16),
        "legacy mode edit must retain topology"
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn per_source_dynamic_brightness_and_direction_are_fixed_and_restore_manual_base() {
    let h = Harness::new(Default::default(), true);
    let mut base = pump();
    base.mode = "meteor".into();
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::SetPump { state: base, reply })
        .await
        .unwrap();
    rx.await.unwrap().unwrap();
    let mut c = dynamic_config();
    c.low_colors = vec![RED; 4];
    c.high_colors = vec![BLUE; 4];
    c.low_speed_percent = Some(25);
    c.high_speed_percent = Some(75);
    c.brightness_percent = Some(75);
    c.direction = Some("reverse".into());
    let state = h.dynamic(Some(c)).await.unwrap();
    assert!(state.last_error.is_none());
    let packet = || {
        h.writes
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|p| p[1] == 131)
            .unwrap()
            .clone()
    };
    assert_eq!(
        (packet()[8], packet()[22]),
        (3, 1),
        "source brightness/direction use fixed values"
    );
    h.dynamic(None).await.unwrap();
    assert_eq!(
        packet()[8],
        2,
        "disable restores50 percent manual brightness"
    );
    let state = h.dynamic_state().await;
    assert!(state.requested.is_none());
    assert_eq!(
        state.profiles.modes[0].bindings[0].brightness_percent,
        Some(75)
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn saved_dynamic_mode_restore_rejects_before_writes_and_retains_intent_on_missing_source() {
    let h = Harness::new(Default::default(), true);
    h.dynamic(Some(dynamic_config())).await.unwrap();
    let saved = h.dynamic_state().await.profiles;
    let restore = |state| {
        let (reply, rx) = oneshot::channel();
        (
            LightingStateMsg::SetPumpWithDynamic {
                state,
                restore_dynamic: true,
                reply,
            },
            rx,
        )
    };
    let mut unknown = pump();
    unknown.mode = "breathing".into();
    let before = h.writes.lock().unwrap().len();
    let (msg, rx) = restore(unknown);
    h.tx.send(msg).await.unwrap();
    assert!(rx.await.unwrap().is_err());
    assert_eq!(h.writes.lock().unwrap().len(), before);
    assert_eq!(h.dynamic_state().await.profiles, saved);
    h.fail.store(true, Ordering::Relaxed);
    let mut changed = pump();
    changed.color = "#abcdef".into();
    let (msg, rx) = restore(changed);
    h.tx.send(msg).await.unwrap();
    assert!(rx.await.unwrap().is_err());
    assert_eq!(h.dynamic_state().await.profiles, saved);
    let (reply, rx) = oneshot::channel();
    h.tx.send(LightingStateMsg::Base {
        kind: LightingKind::Pump,
        reply,
    })
    .await
    .unwrap();
    assert_eq!(
        rx.await.unwrap().current.unwrap().color,
        Rgb { r: 4, g: 5, b: 6 }
    );
    h.fail.store(false, Ordering::Relaxed);
    h.cache.publish(Instant::now(), vec![]);
    let (msg, rx) = restore(pump());
    h.tx.send(msg).await.unwrap();
    rx.await.unwrap().unwrap();
    let state = h.dynamic_state().await;
    assert!(state.requested.is_some());
    assert!(state.degraded);
    assert!(state.applied.is_none());
    assert_eq!(h.pump_color(), [4, 5, 6]);
    h.cache.publish(Instant::now(), vec![row("gpu", 80.0)]);
    h.tick(Duration::from_secs(1)).await;
    assert!(h.dynamic_state().await.applied.is_some());
    h.dynamic(None).await.unwrap();
    assert!(
        !h.dynamic_state()
            .await
            .profiles
            .selected("static")
            .unwrap()
            .enabled
    );
    assert_eq!(
        h.dynamic_state()
            .await
            .profiles
            .selected("static")
            .unwrap()
            .bindings
            .len(),
        1
    );
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn thermal_without_saved_effects_triggers_and_replays_only_available_bases() {
    for (saved_fan, saved_pump) in [(false, false), (true, false), (false, true)] {
        for replay in [false, true] {
            let h = Harness::with_bases(
                if replay {
                    config()
                } else {
                    ThermalConfig::default()
                },
                saved_fan.then(fan),
                saved_pump.then(pump),
                None,
            );
            if replay {
                h.tick(Duration::from_secs(1)).await;
            } else {
                h.set(config()).await.unwrap();
            }
            let state = h.state().await;
            assert!(state.blocked_reason.is_none());
            assert_eq!(state.active_source, Some(ThermalSource::Cpu));
            assert_eq!(state.fan.applied, Some(ThermalLayer::Alert { color: RED }));
            assert_eq!(state.pump.applied, Some(ThermalLayer::Alert { color: RED }));
            let last_packet = |command| {
                h.writes
                    .lock()
                    .unwrap()
                    .iter()
                    .rev()
                    .find(|packet| packet[1] == command)
                    .unwrap()
                    .clone()
            };
            let alert_fan = last_packet(133);
            let alert_pump = last_packet(131);
            assert!(alert_fan.windows(3).any(|bytes| bytes == [255, 0, 0]));
            assert_eq!(&alert_pump[10..13], &[255, 0, 0]);

            // Cooling below the threshold runs ApplyAll: replay saved effects,
            // retaining the last alert color for any zone with no saved effect.
            h.writes.lock().unwrap().clear();
            h.cache
                .publish(Instant::now(), vec![row("cpu", 10.0), row("gpu", 10.0)]);
            h.tick(Duration::from_secs(1)).await;
            assert_eq!(h.state().await.active_source, None);
            if saved_fan {
                assert!(last_packet(133).windows(3).any(|bytes| bytes == [1, 2, 3]));
            } else {
                assert_eq!(last_packet(133), alert_fan);
            }
            if saved_pump {
                assert_eq!(&last_packet(131)[10..13], &[4, 5, 6]);
            } else {
                assert_eq!(last_packet(131), alert_pump);
            }
            // Disabling a currently active rule follows the same ApplyAll path.
            h.writes.lock().unwrap().clear();
            h.cache
                .publish(Instant::now(), vec![row("cpu", 80.0), row("gpu", 10.0)]);
            h.tick(Duration::from_secs(1)).await;
            assert_eq!(h.state().await.active_source, Some(ThermalSource::Cpu));
            assert_eq!(last_packet(133), alert_fan);
            assert_eq!(last_packet(131), alert_pump);
            h.writes.lock().unwrap().clear();
            h.set(ThermalConfig::default()).await.unwrap();
            assert_eq!(h.state().await.active_source, None);
            if saved_fan {
                assert!(last_packet(133).windows(3).any(|bytes| bytes == [1, 2, 3]));
            } else {
                assert_eq!(last_packet(133), alert_fan);
            }
            if saved_pump {
                assert_eq!(&last_packet(131)[10..13], &[4, 5, 6]);
            } else {
                assert_eq!(last_packet(131), alert_pump);
            }
            for (kind, saved) in [
                (LightingKind::Fan, saved_fan),
                (LightingKind::Pump, saved_pump),
            ] {
                let (reply, rx) = oneshot::channel();
                h.tx.send(LightingStateMsg::Base { kind, reply })
                    .await
                    .unwrap();
                assert_eq!(rx.await.unwrap().current.is_some(), saved);
            }
            h.tick(Duration::from_secs(1)).await;
            assert!(
                h.updates.lock().unwrap().iter().all(|update| !matches!(
                    update,
                    llcontrol_daemon::actor::command_dispatch::SettingsUpdate::LightingFan(_)
                        | llcontrol_daemon::actor::command_dispatch::SettingsUpdate::LightingPump(
                            _
                        )
                )),
                "thermal fallback must never manufacture persisted manual effects"
            );
            h.stop().await;
        }
    }
}
