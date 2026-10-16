use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, PersistenceStore},
    sensors::SensorCache,
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, SensorReading, SensorStatus,
    SensorUnit,
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
async fn dynamic_policy_rpc_updates_replays_and_disables_durably() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("dynamic-test".into()),
        PathBuf::from("/mock/dynamic"),
    );
    let key = settings_key(&info);
    let mock = MockBackend::new();
    mock.add_device(info);
    let cache = SensorCache::default();
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let mut policy = json!({"sensor_id":"cpu:test","unit":"celsius","low_value":30,"high_value":60,
        "low_colors":[{"r":0,"g":0,"b":255}],"high_colors":[{"r":255,"g":0,"b":0}],
        "low_speed_percent":null,"high_speed_percent":null});
    // Three independent manager lifetimes: enable/update, replay/disable, replay disabled.
    for cycle in 0..3 {
        cache.publish(
            Instant::now(),
            vec![
                SensorReading {
                    id: "cpu:test".into(),
                    label: "CPU".into(),
                    unit: SensorUnit::Celsius,
                    value: Some(65.0),
                    status: SensorStatus::Valid,
                    observed_unix_ms: Some(1),
                    age_ms: Some(0),
                },
                SensorReading {
                    id: "gpu:load".into(),
                    label: "GPU load".into(),
                    unit: SensorUnit::Percent,
                    value: Some(50.0),
                    status: SensorStatus::Valid,
                    observed_unix_ms: Some(1),
                    age_ms: Some(0),
                },
            ],
        );
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
        if cycle == 0 {
            client
                .call(
                    "set_lighting",
                    json!({"dev":dev,"kind":"pump","cfg":{
                        "mode":"static","color":{"r":4,"g":5,"b":6},"brightness_percent":50,
                        "speed_percent":50,"direction":"forward","scope":"outer","disabled":false
                    }}),
                )
                .await
                .unwrap();
            let enabled = client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":policy}))
                .await
                .unwrap();
            assert_eq!(enabled["requested"], policy);
            assert!(enabled["last_error"].is_null(), "{enabled}");
            assert_eq!(
                enabled["applied"]["colors"][0],
                json!({"r":255,"g":0,"b":0})
            );
            policy["high_colors"] = json!([{"r":0,"g":255,"b":0}]);
            let updated = client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":policy}))
                .await
                .unwrap();
            assert_eq!(updated["applied"]["colors"][0], policy["high_colors"][0]);
            client.call("set_lighting",json!({"dev":dev,"kind":"pump","cfg":{"mode":"breathing","color":{"r":4,"g":5,"b":6},"brightness_percent":50,"speed_percent":50,"direction":"forward","scope":"outer","disabled":false}})).await.unwrap();
            let mut other = policy.clone();
            other["sensor_id"] = json!("gpu:load");
            other["unit"] = json!("percent");
            other["low_speed_percent"] = json!(25);
            other["high_speed_percent"] = json!(75);
            other["brightness_percent"] = json!(75);
            other["direction"] = json!("forward");
            client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":other}))
                .await
                .unwrap();
            other["sensor_id"] = json!("cpu:test");
            other["unit"] = json!("celsius");
            other["low_value"] = json!(40);
            other["high_value"] = json!(70);
            client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":other}))
                .await
                .unwrap();
            client.call("set_lighting",json!({"dev":dev,"kind":"pump","cfg":{"mode":"static","color":{"r":4,"g":5,"b":6},"brightness_percent":50,"speed_percent":50,"direction":"forward","scope":"outer","disabled":false}})).await.unwrap();
            let inactive = client
                .call("get_dynamic_rgb", json!({"dev":dev}))
                .await
                .unwrap();
            assert!(inactive["requested"].is_null());
            assert_eq!(inactive["profiles"]["modes"].as_array().unwrap().len(), 2);
            let info = client
                .call("get_lighting_info", json!({"dev":dev,"kind":"pump"}))
                .await
                .unwrap();
            let applied_modes = info["profiles"].as_array().unwrap();
            assert_eq!(
                applied_modes
                    .iter()
                    .find(|p| p["mode"] == "static")
                    .unwrap()["dynamic_mode"],
                false
            );
            assert_eq!(
                applied_modes
                    .iter()
                    .find(|p| p["mode"] == "breathing")
                    .unwrap()["dynamic_mode"],
                true
            );
            for mode in ["breathing", "static"] {
                let mut selected = applied_modes
                    .iter()
                    .find(|p| p["mode"] == mode)
                    .unwrap()
                    .clone();
                selected["dynamic_mode"] = json!(true);
                client
                    .call(
                        "set_lighting",
                        json!({"dev":dev,"kind":"pump","cfg":selected}),
                    )
                    .await
                    .unwrap();
                let resumed = client
                    .call("get_dynamic_rgb", json!({"dev":dev}))
                    .await
                    .unwrap();
                let expected = if mode == "static" { &policy } else { &other };
                assert_eq!(&resumed["requested"], expected);
                assert!(resumed["applied"].is_object(), "{resumed}");
                assert_eq!(resumed["degraded"], false);
                let base = client
                    .call("get_lighting_info", json!({"dev":dev,"kind":"pump"}))
                    .await
                    .unwrap();
                assert_eq!(base["base"]["dynamic_mode"], true);
            }
            client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":policy}))
                .await
                .unwrap();
        } else if cycle == 1 {
            let restored = client
                .call("get_dynamic_rgb", json!({"dev":dev}))
                .await
                .unwrap();
            assert_eq!(restored["requested"], policy);
            assert_eq!(restored["applied"]["colors"][0], policy["high_colors"][0]);
            assert!(restored["last_error"].is_null(), "{restored}");
            let disabled = client
                .call("set_dynamic_rgb", json!({"dev":dev,"config":null}))
                .await
                .unwrap();
            assert!(disabled["requested"].is_null());
            assert!(disabled["applied"].is_null());
        } else {
            let restored = client
                .call("get_dynamic_rgb", json!({"dev":dev}))
                .await
                .unwrap();
            assert!(restored["requested"].is_null());
            assert!(restored["applied"].is_null());
            assert_eq!(restored["base_mode"], "static");
        }
        let state = client
            .call("get_dynamic_rgb", json!({"dev":dev}))
            .await
            .unwrap();
        let modes = state["profiles"]["modes"].as_array().unwrap();
        assert_eq!(modes.len(), 2);
        let breathing = modes.iter().find(|p| p["mode"] == "breathing").unwrap();
        assert_eq!(breathing["bindings"].as_array().unwrap().len(), 2);
        assert_eq!(breathing["bindings"][0]["sensor_id"], "gpu:load");
        assert_eq!(breathing["bindings"][1]["low_value"], 40);
        assert_eq!(breathing["bindings"][1]["brightness_percent"], 75);
        assert_eq!(breathing["enabled"], true);
        let static_mode = modes.iter().find(|p| p["mode"] == "static").unwrap();
        assert_eq!(static_mode["enabled"], cycle == 0);

        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        let saved = store.read(&key).unwrap();
        if cycle == 0 {
            assert_eq!(
                serde_json::to_value(saved.dynamic_rgb.unwrap()).unwrap(),
                policy
            );
        } else {
            assert!(saved.dynamic_rgb.is_none());
        }
    }
}
