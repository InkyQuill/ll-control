//! Whole actor-tree sleep recovery with synthetic sensors and MockBackend only.
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
    DaemonRpc, LcdSourceKind, LcdTheme, Rgb, SensorReading, SensorStatus, SensorUnit, ThermalLayer,
    ThermalSource,
};
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn publish(cache: &SensorCache, celsius: f64) {
    cache.publish(
        Instant::now(),
        vec![SensorReading {
            id: "fixture:cpu".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(celsius),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(123),
            age_ms: Some(0),
        }],
    );
}
async fn wait_for<F: std::future::Future<Output = bool>>(mut check: impl FnMut() -> F) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("combined actor state must converge");
}
fn packets(jpeg: &[u8]) -> Vec<Vec<u8>> {
    llcontrol_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(jpeg)
        .unwrap()
        .into_iter()
        .map(|packet| packet.to_vec())
        .collect()
}
fn last_packet(mock: &MockBackend, device: &DeviceInfo, command: u8) -> Vec<u8> {
    mock.write_history(device)
        .into_iter()
        .rev()
        .find(|p| p.get(1) == Some(&command))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sleep_wake_restores_theme_cooling_and_thermal_rgb_through_stale_hot_and_cool() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let mock = MockBackend::new();
    let device = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("combined-sleep".into()),
        "/mock/combined-sleep".into(),
    );
    mock.add_device(device.clone());
    let theme: LcdTheme = serde_json::from_value(json!({
        "version":1,"name":"Realtime after wake","background":[10,20,30],"layers":[{
            "kind":"sensor","sensor_id":"fixture:cpu","label":"CPU","unit":"celsius",
            "temperature_unit":"celsius","decimals":0,
            "style":{"x":240,"y":220,"size":32,"color":[255,255,255],"align":"center"}
        }]
    }))
    .unwrap();
    let settings: DeviceSettings = serde_json::from_value(json!({
        "theme":theme,
        "cooling":{
            "fan":{"mode":"curve","sensor_id":"fixture:cpu","points":[{"celsius":30,"percent":30},{"celsius":70,"percent":90}],"hysteresis_c":0,"fallback_percent":100},
            "pump":{"mode":"curve","sensor_id":"fixture:cpu","points":[{"celsius":30,"percent":40},{"celsius":70,"percent":80}],"hysteresis_c":0,"fallback_percent":100}
        },
        "lighting":{
            "fan":{"mode":"static","brightness":50,"speed":25,"direction":"forward","color":"#010203","disabled":false},
            "pump":{"scope":"outer","mode":"static","brightness":50,"speed":25,"direction":"forward","color":"#040506","disabled":false}
        },
        "thermal":{"cpu":{"sensor_id":"fixture:cpu","threshold_c":70,"color":{"r":255,"g":0,"b":0}}}
    })).unwrap();
    store.write(&settings_key(&device), &settings).unwrap();
    let cache = SensorCache::default();
    publish(&cache, 80.0);
    let stats = Arc::new(Stats::new());
    let mut config = Config::default();
    config.poll.hotplug_ms = 20;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .with_sensors(cache.clone())
    .spawn()
    .await;
    wait_for(|| async { !manager.list().await.is_empty() }).await;
    let old = manager.list().await[0].id.clone();
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone())
        .with_sensors(cache.clone());
    let alert = ThermalLayer::Alert {
        color: Rgb { r: 255, g: 0, b: 0 },
    };
    wait_for(|| async {
        let thermal = rpc.get_thermal_state(old.clone()).await.unwrap();
        let cooling = rpc.get_cooling_state(old.clone()).await.unwrap();
        thermal.fan.applied == Some(alert.clone())
            && thermal.pump.applied == Some(alert.clone())
            && cooling.fan.applied_percent == Some(90)
            && cooling.pump.applied_percent == Some(80)
    })
    .await;
    let hot_jpeg = rpc.preview_lcd_theme(None, theme.clone()).await.unwrap();
    let hot_packets = packets(&hot_jpeg);
    let hot_fan = last_packet(&mock, &device, 133);
    let hot_pump = last_packet(&mock, &device, 131);
    assert!(mock
        .write_history(&device)
        .windows(hot_packets.len())
        .any(|p| p == hot_packets));

    manager.prepare_for_sleep(true).await.unwrap();
    let before_wake = mock.write_history(&device).len();
    assert!(manager.list().await.is_empty());
    assert!(rpc.get_cooling_state(old.clone()).await.is_err());
    assert_eq!(cache.readings()[0].status, SensorStatus::Stale);
    let stale_jpeg = rpc.preview_lcd_theme(None, theme.clone()).await.unwrap();
    assert_ne!(stale_jpeg, hot_jpeg);
    let stale_packets = packets(&stale_jpeg);
    manager.prepare_for_sleep(false).await.unwrap();
    let current = manager.list().await[0].id.clone();
    assert_ne!(current, old);
    wait_for(|| async {
        let cooling = rpc.get_cooling_state(current.clone()).await.unwrap();
        let thermal = rpc.get_thermal_state(current.clone()).await.unwrap();
        cooling.fan.fallback_active
            && cooling.pump.fallback_active
            && cooling.fan.applied_percent == Some(100)
            && cooling.pump.applied_percent == Some(100)
            && thermal.degraded_sources == vec![ThermalSource::Cpu]
            && thermal.fan.applied == Some(ThermalLayer::Base)
            && thermal.pump.applied == Some(ThermalLayer::Base)
    })
    .await;
    let writes = mock.write_history(&device);
    assert!(writes[before_wake..]
        .windows(stale_packets.len())
        .any(|p| p == stale_packets));
    assert!(
        !writes[before_wake..]
            .windows(hot_packets.len())
            .any(|p| p == hot_packets),
        "pre-sleep value must not be rendered as current"
    );
    for command in [138, 139] {
        let pwm: Vec<_> = writes[before_wake..]
            .iter()
            .filter(|p| p.get(1) == Some(&command))
            .collect();
        assert!(!pwm.is_empty());
        assert!(
            pwm.iter().all(|p| p[7] == 100),
            "wake must use fallback until a fresh sample"
        );
    }
    let base_fan = last_packet(&mock, &device, 133);
    let base_pump = last_packet(&mock, &device, 131);
    assert_ne!(base_fan, hot_fan);
    assert_ne!(base_pump, hot_pump);
    assert_eq!(
        rpc.get_lcd_theme(current.clone()).await.unwrap(),
        Some(theme.clone())
    );
    let lcd = rpc.get_lcd_state(current.clone()).await.unwrap();
    assert_eq!(lcd.source, LcdSourceKind::Theme);
    assert!(lcd.last_error.is_none());

    let fresh_start = mock.write_history(&device).len();
    publish(&cache, 80.0);
    wait_for(|| async {
        let cooling = rpc.get_cooling_state(current.clone()).await.unwrap();
        let thermal = rpc.get_thermal_state(current.clone()).await.unwrap();
        !cooling.fan.fallback_active
            && !cooling.pump.fallback_active
            && cooling.fan.applied_percent == Some(90)
            && cooling.pump.applied_percent == Some(80)
            && thermal.active_source == Some(ThermalSource::Cpu)
            && thermal.degraded_sources.is_empty()
            && thermal.fan.applied == Some(alert.clone())
            && thermal.pump.applied == Some(alert.clone())
            && mock.write_history(&device)[fresh_start..]
                .windows(hot_packets.len())
                .any(|p| p == hot_packets)
    })
    .await;
    assert_eq!(last_packet(&mock, &device, 139)[7], 90);
    assert_eq!(last_packet(&mock, &device, 138)[7], 80);
    assert_eq!(last_packet(&mock, &device, 133), hot_fan);
    assert_eq!(last_packet(&mock, &device, 131), hot_pump);

    let cool_start = mock.write_history(&device).len();
    publish(&cache, 40.0);
    let cool_packets = packets(&rpc.preview_lcd_theme(None, theme.clone()).await.unwrap());
    assert_ne!(cool_packets, hot_packets);
    assert_ne!(cool_packets, stale_packets);
    wait_for(|| async {
        let cooling = rpc.get_cooling_state(current.clone()).await.unwrap();
        let thermal = rpc.get_thermal_state(current.clone()).await.unwrap();
        cooling.fan.applied_percent == Some(45)
            && cooling.pump.applied_percent == Some(50)
            && !cooling.fan.fallback_active
            && !cooling.pump.fallback_active
            && thermal.active_source.is_none()
            && thermal.degraded_sources.is_empty()
            && thermal.fan.applied == Some(ThermalLayer::Base)
            && thermal.pump.applied == Some(ThermalLayer::Base)
            && mock.write_history(&device)[cool_start..]
                .windows(cool_packets.len())
                .any(|p| p == cool_packets)
    })
    .await;
    assert_eq!(last_packet(&mock, &device, 139)[7], 45);
    assert_eq!(last_packet(&mock, &device, 138)[7], 50);
    assert_eq!(last_packet(&mock, &device, 133), base_fan);
    assert_eq!(last_packet(&mock, &device, 131), base_pump);
    manager.shutdown().await;
    let persisted = store.read(&settings_key(&device)).unwrap();
    assert_eq!(persisted.theme, Some(theme));
    assert_eq!(persisted.cooling, settings.cooling);
    assert_eq!(persisted.thermal, settings.thermal);
    assert_eq!(persisted.lighting.fan.unwrap().color, "#010203");
    assert_eq!(persisted.lighting.pump.unwrap().color, "#040506");
}
