use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    sensors::SensorCache,
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{
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
async fn mode_save_is_revision_guarded_storage_only_and_retryable() {
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
    let banks = lconnect_ipc::CoolingModeBanks {
        version: 1,
        fan: vec![lconnect_ipc::CoolingModeProfile {
            use_default_name: false,
            id: "Saved".into(),
            name: "Inactive fan mode".into(),
            policy: CoolingPolicy::Fixed { percent: 55 },
        }],
        pump: vec![],
        source: Some(lconnect_ipc::CoolingBankSource::Windows2128 {
            fan_json: r#"{"OwnedPrivateMetadata":"not-for-mode-list"}"#.into(),
            pump_json: "{}".into(),
        }),
    };
    let mut initial = DeviceSettings {
        cooling_banks: Some(banks.clone()),
        ..Default::default()
    };
    initial.cooling.fan = Some(CoolingPolicy::Fixed { percent: 85 });
    initial.cooling.pump = Some(CoolingPolicy::Fixed { percent: 70 });
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

    let read = || async {
        client
            .call("get_cooling_modes", json!({"dev":dev}))
            .await
            .unwrap()
    };
    let before = read().await;
    let profile = json!({"id":"Saved","name":"Edited mode","policy":{"mode":"fixed","percent":63}});
    let save = |revision: serde_json::Value, profile: serde_json::Value| {
        let client = &client;
        let dev = &dev;
        async move {
            client.call("save_cooling_mode",json!({"dev":dev,"target":"fan","expected_revision":revision,"profile":profile})).await
        }
    };
    let edited = save(before["revision"].clone(), profile.clone())
        .await
        .unwrap();
    assert_ne!(before["revision"], edited["revision"]);
    assert_eq!(edited["fan"][0], profile);
    assert_eq!(edited["persistence"]["pending"], false);
    assert!(!edited.to_string().contains("not-for-mode-list"));
    let state: CoolingState = serde_json::from_value(
        client
            .call("get_cooling_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.fan.requested, initial.cooling.fan);
    assert_eq!(state.pump.requested, initial.cooling.pump);
    let persisted = store.read(&key).unwrap();
    assert_eq!(
        persisted.cooling_banks.as_ref().unwrap().source,
        banks.source
    );
    assert_eq!(persisted.cooling, initial.cooling);
    assert_eq!(persisted.cooling_modes_revision, 1);
    // No-op saves are still new intent: they invalidate already queued tokens.
    let noop = save(edited["revision"].clone(), profile.clone())
        .await
        .unwrap();
    assert_ne!(noop["revision"], edited["revision"]);
    assert!(save(edited["revision"].clone(), profile.clone())
        .await
        .is_err());
    assert!(save(before["revision"].clone(), profile.clone())
        .await
        .is_err());
    // Unrelated active-policy changes do not conflict with the stored bank.
    client
        .call(
            "set_cooling_policy",
            json!({"dev":dev,"target":"fan","policy":{"mode":"fixed","percent":77}}),
        )
        .await
        .unwrap();
    assert_eq!(read().await["revision"], noop["revision"]);
    let before_invalid =
        std::fs::read(tmp.path().join("devices").join(format!("{key}.toml"))).unwrap();
    for invalid in [
        json!({"id":"Saved","name":"","policy":{"mode":"fixed","percent":63}}),
        json!({"id":"Missing","name":"Valid","policy":{"mode":"fixed","percent":63}}),
        json!({"id":"Saved","name":"Valid","policy":{"mode":"fixed_rpm","rpm":2400}}),
    ] {
        assert!(save(noop["revision"].clone(), invalid).await.is_err());
    }
    assert_eq!(read().await["revision"], noop["revision"]);
    // Force the atomic-rename staging path to fail after accepting intent.
    let staging = tmp.path().join("devices").join(format!("{key}.toml.tmp"));
    std::fs::create_dir(&staging).unwrap();
    let changed = json!({"id":"Saved","name":"Retry mode","policy":{"mode":"fixed","percent":66}});
    assert!(save(noop["revision"].clone(), changed.clone())
        .await
        .is_err());
    let pending = read().await;
    assert_eq!(pending["fan"][0], changed);
    assert_eq!(pending["persistence"]["pending"], true);
    assert!(pending["persistence"]["last_error"].is_string());
    assert_eq!(
        std::fs::read(tmp.path().join("devices").join(format!("{key}.toml"))).unwrap(),
        before_invalid
    );
    std::fs::remove_dir(staging).unwrap();
    tokio::time::sleep(Duration::from_millis(1300)).await;
    let durable = read().await;
    assert_eq!(durable["persistence"]["pending"], false);
    assert!(durable["persistence"]["last_error"].is_null());
    assert_eq!(durable["revision"], pending["revision"]);
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    let saved = store.read(&key).unwrap();
    assert_eq!(
        saved.cooling.fan,
        Some(CoolingPolicy::Fixed { percent: 77 })
    );
    assert_eq!(saved.cooling_banks.unwrap().fan[0].name, "Retry mode");
    assert_eq!(saved.cooling_modes_revision, 3);
}

#[tokio::test]
async fn stored_selection_keeps_requested_snapshot_through_save_archive_restart_and_manual_override(
) {
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
    let banks = lconnect_ipc::CoolingModeBanks {
        version: 1,
        fan: vec![lconnect_ipc::CoolingModeProfile {
            use_default_name: false,
            id: "Saved".into(),
            name: "Inactive fan mode".into(),
            policy: CoolingPolicy::Fixed { percent: 55 },
        }],
        pump: vec![lconnect_ipc::CoolingModeProfile {
            use_default_name: false,
            id: "P0".into(),
            name: "Stored pump".into(),
            policy: CoolingPolicy::FixedRpm { rpm: 2400 },
        }],
        source: Some(lconnect_ipc::CoolingBankSource::Windows2128 {
            fan_json: r#"{"OwnedPrivateMetadata":"not-for-mode-list"}"#.into(),
            pump_json: "{}".into(),
        }),
    };
    let mut initial = DeviceSettings {
        cooling_banks: Some(banks.clone()),
        ..Default::default()
    };
    initial.cooling.fan = Some(CoolingPolicy::Fixed { percent: 85 });
    initial.cooling.pump = Some(CoolingPolicy::Fixed { percent: 70 });
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

    let read = client
        .call("get_cooling_modes", json!({"dev":dev}))
        .await
        .unwrap();
    for (revision, id) in [
        (read["revision"].clone(), "Missing"),
        (json!("0".repeat(64)), "P0"),
    ] {
        assert!(client
            .call(
                "apply_cooling_mode",
                json!({"dev":dev,"target":"pump","expected_revision":revision,"id":id})
            )
            .await
            .is_err());
        let state: CoolingState = serde_json::from_value(
            client
                .call("get_cooling_state", json!({"dev":dev}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(state.pump.requested, initial.cooling.pump);
        assert!(state.pump.mode_id.is_none());
    }
    let applied: CoolingState = serde_json::from_value(
        client
            .call(
                "apply_cooling_mode",
                json!({"dev":dev,"target":"pump","expected_revision":read["revision"],"id":"P0"}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(applied.pump.mode_id.as_deref(), Some("P0"));
    assert_eq!(
        applied.pump.requested,
        Some(CoolingPolicy::FixedRpm { rpm: 2400 })
    );
    assert_eq!(applied.pump.applied_percent, Some(60));
    let newer=client.call("save_cooling_mode",json!({"dev":dev,"target":"pump","expected_revision":read["revision"],"profile":{"id":"P0","name":"Edited while running","policy":{"mode":"fixed_rpm","rpm":2600}}})).await.unwrap();
    let state: CoolingState = serde_json::from_value(
        client
            .call("get_cooling_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.pump.mode_id.as_deref(), Some("P0"));
    assert_eq!(
        state.pump.requested, applied.pump.requested,
        "Save does not retroactively change the requested snapshot"
    );
    assert!(client
        .call(
            "apply_cooling_mode",
            json!({"dev":dev,"target":"pump","expected_revision":read["revision"],"id":"P0"})
        )
        .await
        .is_err());
    let state:CoolingState=serde_json::from_value(client.call("apply_cooling_mode",json!({"dev":dev,"target":"fan","expected_revision":newer["revision"],"id":"Saved"})).await.unwrap()).unwrap();
    assert_eq!(state.fan.mode_id.as_deref(), Some("Saved"));
    assert_eq!(state.pump.mode_id.as_deref(), Some("P0"));
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    let saved = store.read(&key).unwrap();
    assert_eq!(saved.cooling.fan_mode.as_deref(), Some("Saved"));
    assert_eq!(saved.cooling.pump_mode.as_deref(), Some("P0"));
    let snapshot = lconnect_daemon::profile_group::GroupProfile::capture(
        &saved,
        &store.profiles().list().unwrap(),
        lconnect_daemon::profile_group::GroupKind::FanAndLighting,
    )
    .unwrap();
    let target = tempfile::tempdir().unwrap();
    let target_store = PersistenceStore::new_with_root(target.path().into());
    let expected = lconnect_daemon::profile_group::GroupProfile::capture(
        &DeviceSettings::default(),
        &target_store.profiles().list().unwrap(),
        lconnect_daemon::profile_group::GroupKind::FanAndLighting,
    )
    .unwrap()
    .revision;
    target_store
        .commit_profile_group(
            "target",
            &DeviceSettings::default(),
            &expected,
            &snapshot.profile,
        )
        .unwrap();
    assert_eq!(target_store.read("target").unwrap().cooling,saved.cooling,"portable group retains identities and the applied snapshots separately from edited bank policies");
    let mut invalid = saved.clone();
    invalid.cooling.pump = None;
    assert!(store.write(&key, &invalid).is_err());
    invalid = saved.clone();
    invalid.cooling.fan_mode = Some("Missing".into());
    assert!(store.write(&key, &invalid).is_err());
    assert_eq!(store.read(&key).unwrap().cooling, saved.cooling);
    let restarted = MockBackend::new();
    restarted.add_device(info.clone());
    let manager = DeviceManager::new(
        Arc::new(restarted),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.list().await.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let state: CoolingState = serde_json::from_value(
        client
            .call("get_cooling_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.fan.mode_id.as_deref(), Some("Saved"));
    assert_eq!(state.pump.mode_id.as_deref(), Some("P0"));
    assert_eq!(state.pump.requested, saved.cooling.pump);
    let state: CoolingState = serde_json::from_value(
        client
            .call(
                "set_cooling_policy",
                json!({"dev":dev,"target":"fan","policy":{"mode":"fixed","percent":55}}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        state.fan.mode_id.is_none(),
        "equal policy applied generically must not infer a stored identity"
    );
    assert_eq!(state.pump.mode_id.as_deref(), Some("P0"));
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
    assert!(state.pump.mode_id.is_none() && state.pump.requested.is_none());
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    let saved = store.read(&key).unwrap();
    assert!(saved.cooling.fan_mode.is_none() && saved.cooling.pump_mode.is_none());
}

#[tokio::test]
async fn standard_mode_initialization_is_absence_guarded_storage_only_and_retains_failed_disk_intent(
) {
    for fail in [false, true] {
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
        let mut initial = DeviceSettings {
            cooling_banks: None,
            ..Default::default()
        };
        initial.cooling.fan = Some(CoolingPolicy::Fixed { percent: 85 });
        initial.cooling.pump = Some(CoolingPolicy::Fixed { percent: 70 });
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
            .with_sensors(cache.clone());
        let (a, b) = mem_pair();
        let server =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();

        assert!(client
            .call("get_cooling_modes", json!({"dev":dev}))
            .await
            .unwrap()
            .is_null());
        cache.publish(
            Instant::now(),
            vec![
                SensorReading {
                    id: "cpu:test".into(),
                    label: "CPU".into(),
                    unit: SensorUnit::Celsius,
                    value: Some(50.0),
                    status: SensorStatus::Valid,
                    observed_unix_ms: Some(1),
                    age_ms: Some(0),
                },
                SensorReading {
                    id: "wrong:unit".into(),
                    label: "RPM".into(),
                    unit: SensorUnit::Rpm,
                    value: Some(2000.0),
                    status: SensorStatus::Valid,
                    observed_unix_ms: Some(1),
                    age_ms: Some(0),
                },
            ],
        );
        for sensor in ["cpu:missing", "wrong:unit", ""] {
            assert!(client
                .call(
                    "initialize_cooling_modes",
                    json!({"dev":dev,"sensor":sensor})
                )
                .await
                .is_err());
        }
        assert!(client
            .call("get_cooling_modes", json!({"dev":dev}))
            .await
            .unwrap()
            .is_null());
        let staging = tmp.path().join("devices").join(format!("{key}.toml.tmp"));
        if fail {
            std::fs::create_dir(&staging).unwrap();
        }
        let (first, second) = tokio::join!(
            client.call(
                "initialize_cooling_modes",
                json!({"dev":dev,"sensor":"cpu:test"})
            ),
            client.call(
                "initialize_cooling_modes",
                json!({"dev":dev,"sensor":"cpu:test"})
            )
        );
        if fail {
            assert!(first.is_err() && second.is_err());
        } else {
            assert_ne!(
                first.is_ok(),
                second.is_ok(),
                "exactly one concurrent initializer may win"
            );
        }
        let library = client
            .call("get_cooling_modes", json!({"dev":dev}))
            .await
            .unwrap();
        assert_eq!(library["fan"].as_array().unwrap().len(), 8);
        assert_eq!(library["pump"].as_array().unwrap().len(), 3);
        assert_eq!(library["pump"][0]["policy"]["rpm"], 2200);
        assert_eq!(library["persistence"]["pending"], fail);
        assert!(library["fan"][0]["use_default_name"].as_bool().unwrap());
        let state: CoolingState = serde_json::from_value(
            client
                .call("get_cooling_state", json!({"dev":dev}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(state.fan.requested, initial.cooling.fan);
        assert_eq!(state.pump.requested, initial.cooling.pump);
        assert!(state.fan.mode_id.is_none() && state.pump.mode_id.is_none());
        if fail {
            assert!(store.read(&key).unwrap().cooling_banks.is_none());
            std::fs::remove_dir(staging).unwrap();
            tokio::time::sleep(Duration::from_millis(1300)).await;
            assert_eq!(
                client
                    .call("get_cooling_modes", json!({"dev":dev}))
                    .await
                    .unwrap()["persistence"]["pending"],
                false
            );
        }
        let before = store.read(&key).unwrap();
        assert_eq!(before.cooling_modes_revision, 1);
        assert!(client
            .call(
                "initialize_cooling_modes",
                json!({"dev":dev,"sensor":"cpu:test"})
            )
            .await
            .is_err());
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        let reopened = PersistenceStore::new_with_root(tmp.path().into());
        let saved = reopened.read(&key).unwrap();
        assert_eq!(saved.cooling, initial.cooling);
        assert_eq!(
            serde_json::to_value(saved.lighting).unwrap(),
            serde_json::to_value(initial.lighting).unwrap()
        );
        assert_eq!(saved.cooling_modes_revision, 1);
        assert_eq!(
            saved.cooling_banks,
            Some(lconnect_ipc::lcdv2_default_cooling_modes("cpu:test").unwrap())
        );
    }
}
