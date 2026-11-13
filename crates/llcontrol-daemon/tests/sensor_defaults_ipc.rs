use llcontrol_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, sensors::SensorCache, stats::Stats,
};
use llcontrol_hid::MockBackend;
use llcontrol_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair, *};
use serde_json::json;
use std::{sync::Arc, time::Instant};
fn row(id: &str) -> SensorReading {
    SensorReading {
        id: id.into(),
        label: "same GPU label".into(),
        unit: SensorUnit::Percent,
        value: Some(25.0),
        status: SensorStatus::Valid,
        observed_unix_ms: Some(1),
        age_ms: Some(0),
    }
}
fn choice(gpu: &str, id: &str) -> SensorDefaults {
    SensorDefaults {
        gpu: Some(GpuSensorDefaults {
            device_id: gpu.into(),
            load: Some(id.into()),
            temperature: None,
            clock: None,
            power: None,
            fan: None,
            voltage: None,
            memory_used: None,
        }),
        ..Default::default()
    }
}
#[tokio::test]
async fn ipc_selection_revision_restart_missing_and_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let persistence = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let cache = SensorCache::default();
    let fan_id = "hwmon:2f63:nct6775:fan1";
    let mut fan = row(fan_id);
    fan.unit = SensorUnit::Rpm;
    fan.value = Some(0.0);
    let rows = vec![
        fan,
        row("amdgpu:2f61:utilization"),
        row("amdgpu:2f62:utilization"),
    ];
    cache.publish(Instant::now(), rows.clone());
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        persistence.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(
        manager.clone(),
        stats.clone(),
        config.clone(),
        persistence.clone(),
    )
    .with_sensors(cache.clone());
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let initial: SensorDefaultsState =
        serde_json::from_value(client.call("get_sensor_defaults", json!({})).await.unwrap())
            .unwrap();
    assert_eq!(initial.candidates.len(), 3);
    assert!(initial.recommended.cpu.fan.is_none());
    assert!(initial.values.iter().all(|v| v.reading.is_none()));
    let bad = choice("sysfs:2f62:amdgpu", "amdgpu:2f61:utilization");
    assert!(client
        .call("set_sensor_defaults", json!({"config":bad}))
        .await
        .is_err());
    assert!(!dir.path().join("sensor-defaults.json").exists());
    let mut proposed = choice("sysfs:2f62:amdgpu", "amdgpu:2f62:utilization");
    proposed.cpu.fan = Some(fan_id.into());
    let saved: SensorDefaults = serde_json::from_value(
        client
            .call("set_sensor_defaults", json!({"config":proposed}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved.revision, 1);
    assert!(client
        .call("set_sensor_defaults", json!({"config":proposed}))
        .await
        .is_err());
    assert!(client
        .call(
            "set_sensor_defaults",
            json!({"config":saved,"candidates":initial.candidates})
        )
        .await
        .is_err());
    let state: SensorDefaultsState =
        serde_json::from_value(client.call("get_sensor_defaults", json!({})).await.unwrap())
            .unwrap();
    assert_eq!(
        state
            .values
            .iter()
            .find(|v| v.role == DefaultSensorRole::GpuLoad)
            .unwrap()
            .reading
            .as_ref()
            .unwrap()
            .id,
        "amdgpu:2f62:utilization"
    );
    assert_eq!(
        state
            .values
            .iter()
            .find(|v| v.role == DefaultSensorRole::CpuFan)
            .unwrap()
            .reading
            .as_ref()
            .unwrap()
            .value,
        Some(0.0)
    );
    // A newly constructed RPC instance reloads durable choices, not process memory.
    drop(client);
    server.abort();
    let _ = server.await;
    let restarted =
        IpcServerImpl::new(manager.clone(), stats, config, persistence).with_sensors(cache.clone());
    assert_eq!(restarted.get_sensor_defaults().await.unwrap().config, saved);
    cache.publish(Instant::now(), vec![rows[1].clone()]);
    let missing = restarted.get_sensor_defaults().await.unwrap();
    assert_eq!(missing.config, saved);
    assert!(missing
        .values
        .iter()
        .find(|v| v.role == DefaultSensorRole::GpuLoad)
        .unwrap()
        .reading
        .is_none());
    assert!(missing
        .values
        .iter()
        .find(|v| v.role == DefaultSensorRole::CpuFan)
        .unwrap()
        .reading
        .is_none());
    cache.publish(Instant::now(), rows);
    let restored = restarted.get_sensor_defaults().await.unwrap();
    assert_eq!(
        restored
            .values
            .iter()
            .find(|v| v.role == DefaultSensorRole::CpuFan)
            .unwrap()
            .reading
            .as_ref()
            .unwrap()
            .id,
        fan_id
    );
    assert!(restarted
        .get_sensor_defaults()
        .await
        .unwrap()
        .values
        .iter()
        .find(|v| v.role == DefaultSensorRole::GpuLoad)
        .unwrap()
        .reading
        .is_some());
    // A hand-edited, structurally valid file cannot turn a GPU A reading into GPU B.
    let wrong_membership = choice("sysfs:2f62:amdgpu", "amdgpu:2f61:utilization");
    std::fs::write(
        dir.path().join("sensor-defaults.json"),
        serde_json::to_vec(&wrong_membership).unwrap(),
    )
    .unwrap();
    let invalid = restarted.get_sensor_defaults().await.unwrap();
    assert!(invalid.values.iter().all(|value| value.reading.is_none()));
    let corrupt = b"future schema or damaged bytes";
    std::fs::write(dir.path().join("sensor-defaults.json"), corrupt).unwrap();
    assert!(restarted.get_sensor_defaults().await.is_err());
    assert!(restarted
        .set_sensor_defaults(SensorDefaults::default())
        .await
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("sensor-defaults.json")).unwrap(),
        corrupt
    );
    manager.shutdown().await;
}
