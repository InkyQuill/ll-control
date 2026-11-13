use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    sensors::SensorCache,
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, CoolingPolicy, CoolingState,
    SensorReading, SensorStatus, SensorUnit,
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
async fn policy_ipc_manual_override_atomic_persistence_and_restart_do_not_replay_old_curve() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("M2-test".into()),
        PathBuf::from("/mock/m2"),
    );
    let key = settings_key(&info);
    let banks = llcontrol_ipc::CoolingModeBanks {
        version: 1,
        fan: vec![llcontrol_ipc::CoolingModeProfile {
            use_default_name: false,
            id: "Saved".into(),
            name: "Inactive fan mode".into(),
            policy: CoolingPolicy::Fixed { percent: 55 },
        }],
        pump: vec![],
        source: Some(llcontrol_ipc::CoolingBankSource::Windows2128 {
            fan_json: r#"{"OwnedPrivateMetadata":"not-for-mode-list"}"#.into(),
            pump_json: "{}".into(),
        }),
    };
    let initial = DeviceSettings {
        cooling_banks: Some(banks.clone()),
        ..Default::default()
    };
    store.write(&key, &initial).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info.clone());
    let cache = SensorCache::default();
    cache.publish(
        Instant::now(),
        vec![SensorReading {
            id: "cpu:test".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(50.0),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        }],
    );
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .with_sensors(cache.clone())
    .spawn()
    .await;
    let dev = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(d) = manager.list().await.first() {
                break d.id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone())
        .with_sensors(cache);
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let profile_path = tmp.path().join("profiles.json");
    let previous_profiles = std::fs::read(&profile_path).ok();
    std::fs::write(&profile_path, b"{broken unrelated named profile library").unwrap();
    let modes = client
        .call("get_cooling_modes", json!({"dev":dev}))
        .await
        .unwrap();
    if let Some(bytes) = previous_profiles {
        std::fs::write(&profile_path, bytes).unwrap();
    } else {
        std::fs::remove_file(&profile_path).unwrap();
    }
    assert!(
        modes.get("source").is_none(),
        "mode list omits historical source metadata"
    );
    assert!(!modes.to_string().contains("not-for-mode-list"));
    let modes: llcontrol_ipc::CoolingModeLibrary = serde_json::from_value(modes).unwrap();
    assert_eq!(modes.fan, banks.fan);
    assert_eq!(modes.pump, banks.pump);
    assert_eq!(modes.revision.len(), 64);
    assert_eq!(
        store.read(&key).unwrap().cooling_banks,
        Some(banks.clone()),
        "read-only projection must not strip persisted provenance"
    );
    let policy = json!({"mode":"curve","sensor_id":"cpu:test","points":[{"celsius":30,"percent":30},{"celsius":70,"percent":100}],"hysteresis_c":2,"fallback_percent":100});
    let value = client
        .call(
            "set_cooling_policy",
            json!({"dev":dev,"target":"fan","policy":policy}),
        )
        .await
        .unwrap();
    let state: CoolingState = serde_json::from_value(value).unwrap();
    assert_eq!(state.fan.applied_percent, Some(65));
    client
        .call("set_fan_pwm", json!({"dev":dev,"percent":75}))
        .await
        .unwrap();
    let state: CoolingState = serde_json::from_value(
        client
            .call("get_cooling_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(state.fan.requested.is_none());
    assert_eq!(state.fan.applied_percent, Some(75));
    let rpm_policy = llcontrol_ipc::PumpPreset::Performance.policy("cpu:test".into(), 2.0, 100);
    let value = client
        .call(
            "set_cooling_policy",
            json!({"dev":dev,"target":"pump","policy":rpm_policy}),
        )
        .await
        .unwrap();
    let state: CoolingState = serde_json::from_value(value).unwrap();
    assert_eq!(state.pump.applied_percent, Some(74));
    assert_eq!(state.pump.requested, Some(rpm_policy.clone()));
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    let saved = store.read(&key).unwrap();
    assert_eq!(saved.fan.pwm_percent, 75);
    assert!(saved.cooling.fan.is_none());
    assert_eq!(saved.cooling.pump, Some(rpm_policy.clone()));
    assert_eq!(
        saved.cooling_banks,
        Some(banks.clone()),
        "manual PWM and later policy updates retain inactive banks"
    );
    // Seed an active policy plus a conflicting legacy value, then reattach.
    let mut saved = DeviceSettings::default();
    saved.fan.pwm_percent = 17;
    saved.pump.pwm_percent = 22;
    saved.cooling.pump = Some(rpm_policy);
    saved.cooling.fan = Some(CoolingPolicy::Fixed { percent: 85 });
    saved.cooling_banks = Some(banks.clone());
    store.write(&key, &saved).unwrap();
    let fresh = MockBackend::new();
    fresh.add_device(info.clone());
    let manager = DeviceManager::new(Arc::new(fresh.clone()), store.clone(), cfg, stats)
        .spawn()
        .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !manager.list().await.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    manager.shutdown().await;
    let writes = fresh.write_history(&info);
    assert_eq!(store.read(&key).unwrap().cooling_banks, Some(banks));
    let fan: Vec<_> = writes.iter().filter(|w| w.get(1) == Some(&139)).collect();
    assert!(!fan.is_empty());
    assert!(fan.iter().all(|w| w[7] == 85));
    // Host cache has not sampled after restart: retained RPM curve uses fallback,
    // never the obsolete raw duty. No pretend RPM measurement is returned.
    let pump: Vec<_> = writes.iter().filter(|w| w.get(1) == Some(&138)).collect();
    assert!(!pump.is_empty());
    assert!(pump.iter().all(|w| w[6] == 0 && w[7] == 100));
}

#[test]
fn curve_persistence_roundtrip_preserves_fields_and_legacy_files_default_to_no_policy() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut settings = DeviceSettings::default();
    settings.cooling.pump=Some(serde_json::from_value(json!({"mode":"curve","sensor_id":"lcdv2:stable:coolant","points":[{"celsius":25,"percent":40},{"celsius":60,"percent":100}],"hysteresis_c":1.5,"fallback_percent":100})).unwrap());
    store.write("test", &settings).unwrap();
    assert_eq!(store.read("test").unwrap().cooling, settings.cooling);
    for policy in [
        CoolingPolicy::FixedRpm { rpm: 2200 },
        llcontrol_ipc::PumpPreset::Quiet.policy("cpu:test".into(), 2.0, 100),
    ] {
        settings.cooling.pump = Some(policy);
        store.write("test", &settings).unwrap();
        assert_eq!(store.read("test").unwrap().cooling, settings.cooling);
    }
    let old: DeviceSettings = toml::from_str("schema_version=1\n[fan]\npwm_percent=60\n").unwrap();
    assert!(old.cooling.fan.is_none());
    assert!(old.cooling.pump.is_none());
}

#[tokio::test]
async fn motherboard_pump_replays_until_manual_mcu_override_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("pump-source-replay".into()),
        PathBuf::from("/mock/pump-source-replay"),
    );
    let key = settings_key(&info);
    let policy = CoolingPolicy::Motherboard { percent: 80 };
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;

    // Each boot has an independent HID history: old packets cannot prove replay.
    for boot in 0..3 {
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(mock.clone()),
            store.clone(),
            cfg.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let dev = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(device) = manager.list().await.first() {
                    break device.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("mock device should attach");
        let rpc = IpcServerImpl::new(manager.clone(), stats, cfg.clone(), store.clone());
        let (a, b) = mem_pair();
        let server =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if boot == 0 {
            client
                .call("set_fan_pwm", json!({"dev":dev,"percent":73}))
                .await
                .unwrap();
            client
                .call(
                    "set_cooling_policy",
                    json!({"dev":dev,"target":"pump","policy":policy}),
                )
                .await
                .unwrap();
        }
        let (source, percent) = if boot < 2 { (1, 80) } else { (0, 67) };
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let state: CoolingState = serde_json::from_value(
                    client
                        .call("get_cooling_state", json!({"dev":dev}))
                        .await
                        .unwrap(),
                )
                .unwrap();
                if state.pump.applied_percent == Some(percent) {
                    assert_eq!(state.fan.applied_percent, Some(73));
                    assert!(state.fan.requested.is_none());
                    assert!(!state.fan.motherboard);
                    assert!(state.fan.last_error.is_none());
                    assert_eq!(state.pump.motherboard, source == 1);
                    assert_eq!(state.pump.requested, (boot < 2).then(|| policy.clone()));
                    assert!(state.pump.last_error.is_none());
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("pump source and duty should be applied after replay");
        let writes = mock.write_history(&info);
        let pump: Vec<_> = writes.iter().filter(|w| w.get(1) == Some(&138)).collect();
        assert!(!pump.is_empty());
        assert!(pump.iter().all(|w| w[6] == source && w[7] == percent));

        if boot == 1 {
            let before_override = writes.len();
            client
                .call(
                    "set_pump_pwm",
                    json!({"dev":dev,"percent":67,"source":"mcu"}),
                )
                .await
                .unwrap();
            let state: CoolingState = serde_json::from_value(
                client
                    .call("get_cooling_state", json!({"dev":dev}))
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert!(state.pump.requested.is_none());
            assert!(!state.pump.motherboard);
            assert_eq!(state.pump.applied_percent, Some(67));
            assert!(state.pump.last_error.is_none());
            let writes = mock.write_history(&info);
            let pump: Vec<_> = writes[before_override..]
                .iter()
                .filter(|w| w.get(1) == Some(&138))
                .collect();
            assert!(!pump.is_empty());
            assert!(pump.iter().all(|w| w[6] == 0 && w[7] == 67));
        }
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        let saved = store.read(&key).unwrap();
        assert_eq!(saved.fan.pwm_percent, 73);
        assert!(saved.cooling.fan.is_none());
        if boot == 0 {
            assert_eq!(saved.cooling.pump, Some(policy.clone()));
        } else {
            assert!(saved.cooling.pump.is_none());
            assert_eq!(saved.pump.source, "mcu");
            assert_eq!(saved.pump.pwm_percent, 67);
        }
    }
}

#[tokio::test]
async fn explicit_manual_zero_survives_real_ipc_and_manager_replay_without_turning_legacy_defaults_into_commands(
) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("manual-zero".into()),
        PathBuf::from("/mock/manual-zero"),
    );
    let key = settings_key(&info);
    store.write(&key, &DeviceSettings::default()).unwrap();
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    for boot in 0..4 {
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(mock.clone()),
            store.clone(),
            cfg.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let dev = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(d) = manager.list().await.first() {
                    break d.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let rpc = IpcServerImpl::new(manager.clone(), stats, cfg.clone(), store.clone());
        let (a, b) = mem_pair();
        let server =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if boot == 0 {
            assert!(
                mock.write_history(&info)
                    .iter()
                    .all(|p| !matches!(p.get(1), Some(138 | 139))),
                "legacy unset0 must not command PWM on startup"
            );
            client
                .call("set_fan_pwm", json!({"dev":dev,"percent":0}))
                .await
                .unwrap();
            client
                .call(
                    "set_pump_pwm",
                    json!({"dev":dev,"percent":0,"source":"application"}),
                )
                .await
                .unwrap();
        }
        let state: CoolingState = serde_json::from_value(
            client
                .call("get_cooling_state", json!({"dev":dev}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            state.fan.applied_percent,
            Some(if boot < 3 { 0 } else { 67 })
        );
        assert_eq!(state.pump.applied_percent, Some(0));
        assert_eq!(state.pump.motherboard, boot < 2);
        assert!(state.fan.requested.is_none() && state.pump.requested.is_none());
        let writes = mock.write_history(&info);
        let fan: Vec<_> = writes.iter().filter(|p| p.get(1) == Some(&139)).collect();
        let pump: Vec<_> = writes.iter().filter(|p| p.get(1) == Some(&138)).collect();
        assert!(!fan.is_empty() && !pump.is_empty());
        assert!(fan.iter().all(|p| p[7] == if boot < 3 { 0 } else { 67 }));
        assert!(pump.iter().all(|p| p[7] == 0 && p[6] == u8::from(boot < 2)));
        if boot == 1 {
            client
                .call(
                    "set_pump_pwm",
                    json!({"dev":dev,"percent":0,"source":"mcu"}),
                )
                .await
                .unwrap();
        }
        if boot == 2 {
            client
                .call("set_fan_pwm", json!({"dev":dev,"percent":67}))
                .await
                .unwrap();
        }
        drop(client);
        server.await.unwrap().unwrap();
        manager.shutdown().await;
        let saved = store.read(&key).unwrap();
        assert_eq!(saved.fan.manual_zero, boot < 2);
        assert_eq!(
            saved.pump.source,
            if boot == 0 { "application" } else { "mcu" }
        );
        assert!(saved.fan.has_manual_setting() && saved.pump.has_manual_setting());
    }
}

#[test]
fn legacy_zero_absence_keeps_portable_shape_and_invalid_zero_marker_is_protected() {
    let old: DeviceSettings = toml::from_str(
        "schema_version=1\n[fan]\npwm_percent=0\n[pump]\npwm_percent=0\nsource=''\n",
    )
    .unwrap();
    assert!(!old.fan.manual_zero);
    assert!(!old.fan.has_manual_setting());
    assert!(!old.pump.has_manual_setting());
    assert_eq!(
        serde_json::to_string(&old.fan).unwrap(),
        "{\"pwm_percent\":0}",
        "absent marker must preserve pre-marker group receipt hashing"
    );
    let mut nonzero = old.fan.clone();
    nonzero.pwm_percent = 64;
    assert!(nonzero.has_manual_setting());
    assert_eq!(
        serde_json::to_string(&nonzero).unwrap(),
        "{\"pwm_percent\":64}"
    );
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let mut settings = old;
    settings.fan.manual_zero = true;
    store.write("zero", &settings).unwrap();
    assert!(store.read("zero").unwrap().fan.has_manual_setting());
    let before = std::fs::read(root.path().join("devices/zero.toml")).unwrap();
    settings.fan.pwm_percent = 64;
    assert!(store
        .write("zero", &settings)
        .unwrap_err()
        .to_string()
        .contains("manual_zero"));
    assert_eq!(
        std::fs::read(root.path().join("devices/zero.toml")).unwrap(),
        before
    );
}
