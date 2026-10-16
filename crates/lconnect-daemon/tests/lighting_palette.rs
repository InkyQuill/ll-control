//! Full mode palettes through real IPC, sole RGB owner, persistence and replay.
use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[tokio::test]
async fn four_color_fan_and_pump_palettes_survive_restart_and_invalid_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("palette-test".into()),
        PathBuf::from("/mock/palette"),
    );
    let key = settings_key(&info);
    let mock = MockBackend::new();
    mock.add_device(info.clone());
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let extra = json!([{"r":4,"g":5,"b":6},{"r":7,"g":8,"b":9},{"r":10,"g":11,"b":12}]);
    for cycle in 0..2 {
        let before = mock.write_history(&info).len();
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
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone());
        let (a, b) = mem_pair();
        let server =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if cycle == 0 {
            for kind in ["fan", "pump"] {
                let mut palette = json!({"mode":"meteor","color":{"r":1,"g":2,"b":3},"additional_colors":extra,"brightness_percent":100,"speed_percent":75,"direction":"forward","scope":"outer","disabled":false});
                let mut two = palette.clone();
                two["mode"] = json!("runway");
                two["additional_colors"] = json!([{"r":4,"g":5,"b":6}]);
                client
                    .call("set_lighting", json!({"dev":dev,"kind":kind,"cfg":two}))
                    .await
                    .unwrap();
                let (cmd, offset) = if kind == "fan" { (133, 9) } else { (131, 10) };
                assert!(mock.write_history(&info).iter().any(|p| p[1] == cmd
                    && p[offset..offset + 12] == [1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0]));
                client
                    .call("set_lighting", json!({"dev":dev,"kind":kind,"cfg":palette}))
                    .await
                    .unwrap();
                // The wrong mode/count cannot replace a successfully applied palette.
                palette["mode"] = json!("static");
                assert!(client
                    .call("set_lighting", json!({"dev":dev,"kind":kind,"cfg":palette}))
                    .await
                    .is_err());
            }
        }
        if cycle == 0 {
            let mut cfg = json!({"mode":"meteor","color":{"r":1,"g":2,"b":3},"additional_colors":extra,"brightness_percent":100,"speed_percent":75,"direction":"forward","disabled":false});
            for (sync, size, count) in [(true, "280", 16), (true, "360", 24), (false, "360", 31)] {
                let topology = json!({"sync_to_pump":sync,"size":size,"manual_led_count":31});
                cfg["fan_topology"] = topology.clone();
                client
                    .call("set_lighting", json!({"dev":dev,"kind":"fan","cfg":cfg}))
                    .await
                    .unwrap();
                let packet = mock
                    .write_history(&info)
                    .into_iter()
                    .rev()
                    .find(|p| p[1] == 133)
                    .unwrap();
                assert_eq!((packet[24], packet[25]), (u8::from(sync), count));
                let state = client
                    .call("get_lighting_info", json!({"dev":dev,"kind":"fan"}))
                    .await
                    .unwrap();
                assert_eq!(state["fan_topology"], topology);
                assert!(client
                    .call("set_lighting", json!({"dev":dev,"kind":"pump","cfg":cfg}))
                    .await
                    .is_err());
            }
            for count in [7, 51] {
                cfg["fan_topology"]["manual_led_count"] = json!(count);
                assert!(client
                    .call("set_lighting", json!({"dev":dev,"kind":"fan","cfg":cfg}))
                    .await
                    .is_err());
            }
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let history = mock.write_history(&info);
                let matching = |cmd, offset| {
                    history[before..].iter().any(|p| {
                        p[1] == cmd
                            && p[offset..offset + 12] == [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
                    })
                };
                if matching(133, 9) && matching(131, 10) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for kind in ["fan", "pump"] {
            let info = client
                .call("get_lighting_info", json!({"dev":dev,"kind":kind}))
                .await
                .unwrap();
            assert_eq!(info["base"]["mode"], "meteor");
            if kind == "fan" {
                assert_eq!(
                    info["fan_topology"],
                    json!({"sync_to_pump":false,"size":"360","manual_led_count":31})
                );
                for profile in info["profiles"].as_array().unwrap() {
                    assert_eq!(
                        profile["fan_topology"], info["fan_topology"],
                        "mode profiles must expose current global topology"
                    );
                }
            }

            assert_eq!(info["base"]["color"], json!({"r":1,"g":2,"b":3}));
            assert_eq!(info["base"]["additional_colors"], extra);
            assert_eq!(info["base"]["brightness_percent"], 100);
            assert_eq!(info["base"]["speed_percent"], 75);
            let profiles = info["profiles"].as_array().unwrap();
            assert_eq!(profiles.len(), 2);
            let runway = profiles.iter().find(|p| p["mode"] == "runway").unwrap();
            assert_eq!(runway["color"], json!({"r":1,"g":2,"b":3}));
            assert_eq!(runway["additional_colors"], json!([{"r":4,"g":5,"b":6}]));
            assert_eq!(
                runway["speed_percent"], 75,
                "custom mode speed survives selection/restart"
            );
            let catalog = info["catalog"].as_array().unwrap();
            let static_mode = catalog.iter().find(|c| c["mode"] == "static").unwrap();
            assert_eq!(static_mode["speed"], false);
            assert_eq!(static_mode["direction"], false);
        }
        if cycle == 1 {
            assert!(
                mock.write_history(&info)[before..]
                    .iter()
                    .all(|p| !(p[1] == 133 && p[6] == 5 || p[1] == 131 && p[7] == 5)),
                "inactive Runway profile must not replay"
            );
        }
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        let saved = store.read(&key).unwrap();
        let fan = saved.lighting.fan.unwrap();
        let pump = saved.lighting.pump.unwrap();
        assert_eq!(fan.mode, "meteor");
        assert_eq!(pump.mode, "meteor");
        assert_eq!(fan.color, "#010203");
        assert_eq!(pump.color, "#010203");
        assert_eq!(fan.additional_colors, ["#040506", "#070809", "#0a0b0c"]);
        assert_eq!(pump.additional_colors, fan.additional_colors);
    }
}

#[test]
fn legacy_settings_remain_readable_and_malformed_saved_palettes_cannot_replace_them() {
    use lconnect_daemon::persistence::DeviceSettings;
    let tmp = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let mut settings: DeviceSettings =
        serde_json::from_value(json!({"lighting":{"pump":{"mode":"meteor","color":"#010203"}}}))
            .unwrap();
    store.write("palette", &settings).unwrap();
    assert!(store
        .read("palette")
        .unwrap()
        .lighting
        .pump
        .unwrap()
        .additional_colors
        .is_empty());
    for extras in [
        vec!["#010203"; 4],
        vec!["#010203"],
        vec!["bad", "#010203", "#040506"],
    ] {
        settings.lighting.pump.as_mut().unwrap().additional_colors =
            extras.into_iter().map(String::from).collect();
        assert!(store.write("palette", &settings).is_err());
        assert!(store
            .read("palette")
            .unwrap()
            .lighting
            .pump
            .unwrap()
            .additional_colors
            .is_empty());
    }
}

#[test]
fn invalid_mode_history_cannot_replace_legacy_settings() {
    use lconnect_daemon::persistence::{DeviceSettings, LightingState};
    let tmp = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let mut s = DeviceSettings::default();
    store.write("mode-history", &s).unwrap();
    for (key, mode) in [("static", "meteor"), ("transmit", "transmit")] {
        s.lighting.fan_modes.clear();
        s.lighting.fan_modes.insert(
            key.into(),
            LightingState {
                mode: mode.into(),
                ..Default::default()
            },
        );
        assert!(store.write("mode-history", &s).is_err());
        assert!(store
            .read("mode-history")
            .unwrap()
            .lighting
            .fan_modes
            .is_empty());
    }
}

#[test]
fn malformed_global_topology_cannot_replace_saved_settings() {
    use lconnect_daemon::persistence::DeviceSettings;
    let tmp = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let mut s = DeviceSettings::default();
    store.write("topology", &s).unwrap();
    for count in [7, 51] {
        s.lighting.fan_topology.manual_led_count = count;
        assert!(store.write("topology", &s).is_err());
        assert_eq!(
            store
                .read("topology")
                .unwrap()
                .lighting
                .fan_topology
                .manual_led_count,
            24
        );
    }
}
