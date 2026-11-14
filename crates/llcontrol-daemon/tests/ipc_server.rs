//! Integration tests for `IpcServerImpl` (P1D-11).
//!
//! Spins up a `MockBackend`-backed `DeviceManager`, builds an
//! `IpcServerImpl`, exposes it through `IpcServer::serve_stream` over the
//! in-memory transport, and exercises the JSON-RPC surface end-to-end via
//! `IpcClient::from_stream`. This validates:
//!
//! - per-method JSON wire shape acceptance,
//! - the trait dispatch table's success/error fan-out,
//! - the shared `Stats` counter increments,
//! - the reset-token registry's single-use and confirmation semantics,
//! - the `raw_send` config gate.
//!
//! Tests deliberately do NOT assert HID byte sequences: that is the
//! protocol crate's job, and the MockBackend just absorbs writes.

use llcontrol_daemon::config::Config;
use llcontrol_daemon::ipc_server::IpcServerImpl;
use llcontrol_daemon::manager::DeviceManager;
use llcontrol_daemon::persistence::PersistenceStore;
use llcontrol_daemon::stats::Stats;
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use llcontrol_ipc::client::IpcClient;
use llcontrol_ipc::server::IpcServer;
use llcontrol_ipc::transport_memory::mem_pair;
use llcontrol_ipc::{DaemonStats, DeviceDescriptor, DeviceId, IpcError, ResetToken};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Build a `Config` with a tight 50 ms hot-plug poll for fast attach
/// reconciliation in tests.
fn fast_poll_config() -> Config {
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 50;
    cfg
}

fn lcdv2_info(serial: &str, path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some(serial.to_string()),
        PathBuf::from(path),
    )
}

/// Spin up the manager + IpcServerImpl + JSON-RPC server over an
/// in-memory duplex pair, returning a connected client and a guard that
/// keeps the underlying tempdir alive for the test's duration.
struct Harness {
    client: IpcClient,
    server_task: tokio::task::JoinHandle<()>,
    /// Cheap clone of the `MockBackend` used by the manager. Tests can
    /// peek at `write_history` to assert the daemon issued the right
    /// HID writes -- the manager owns its `Arc<dyn HidBackend>`, but
    /// `MockBackend` is `Arc<Mutex<...>>`-internal so this clone shares
    /// the same backing state.
    mock: MockBackend,
    _tmp: TempDir,
}

impl Harness {
    async fn shutdown(self) {
        drop(self.client);
        self.server_task.abort();
        let _ = self.server_task.await;
    }
}

async fn make_harness(allow_raw_send: bool) -> Harness {
    make_harness_with_sensors(allow_raw_send, Default::default()).await
}
async fn make_harness_with_sensors(
    allow_raw_send: bool,
    sensors: llcontrol_daemon::sensors::SensorCache,
) -> Harness {
    let tmp = TempDir::new().expect("tempdir");
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info("SN-A", "/mock/hidraw0"));
    let mock_for_inspect = mock.clone();
    let backend: Arc<dyn HidBackend> = Arc::new(mock);

    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let manager = DeviceManager::new(
        backend,
        store.clone(),
        fast_poll_config(),
        Arc::new(Stats::new()),
    );
    let manager_handle = manager.with_sensors(sensors.clone()).spawn().await;

    // Wait for the first poll tick to attach the seeded device.
    for _ in 0..40 {
        if !manager_handle.list().await.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        !manager_handle.list().await.is_empty(),
        "harness: device manager never attached the seeded mock device"
    );

    let mut config = Config::default();
    config.daemon.allow_raw_send = allow_raw_send;

    let stats = Arc::new(Stats::new());
    let rpc =
        Arc::new(IpcServerImpl::new(manager_handle, stats, config, store).with_sensors(sensors));

    let (client_end, server_end) = mem_pair();
    let server = IpcServer::new(rpc);
    let server_task = tokio::spawn(async move {
        let _ = server.serve_stream(server_end).await;
    });

    let client = IpcClient::from_stream(client_end)
        .await
        .expect("client connect");
    Harness {
        client,
        server_task,
        mock: mock_for_inspect,
        _tmp: tmp,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_devices_returns_attached_device() {
    let h = make_harness(false).await;

    let v = h
        .client
        .call("list_devices", json!({}))
        .await
        .expect("list_devices");
    let descs: Vec<DeviceDescriptor> =
        serde_json::from_value(v).expect("decode DeviceDescriptor list");
    assert_eq!(descs.len(), 1, "exactly one device attached");
    assert_eq!(descs[0].id, DeviceId::new("galahad2-lcdv2:0001"));
    assert_eq!(descs[0].family, "galahad2-lcdv2");

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_stats_reports_attached_count_and_request_counter() {
    let h = make_harness(false).await;

    // Make any call so the ipc_request_count is at least 1.
    let _ = h.client.call("list_devices", json!({})).await.unwrap();

    let v = h.client.call("daemon_stats", json!({})).await.unwrap();
    let stats: DaemonStats = serde_json::from_value(v).expect("decode DaemonStats");
    assert_eq!(stats.devices_known, 1, "manager reports 1 attached device");
    assert!(
        stats.ipc_request_count >= 2,
        "ipc_request_count expected >= 2 (list_devices + daemon_stats), got {}",
        stats.ipc_request_count
    );

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_fan_pwm_succeeds() {
    let h = make_harness(false).await;

    // The MockBackend just appends every write to its history and
    // returns Ok; set_fan_pwm sends one Write through hid_io and awaits
    // the per-write reply. No HID-byte assertion here -- the protocol
    // crate's tests pin the wire shape.
    let _ = h
        .client
        .call(
            "set_fan_pwm",
            json!({ "dev": "galahad2-lcdv2:0001", "percent": 50 }),
        )
        .await
        .expect("set_fan_pwm");

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_lighting_fan_succeeds() {
    let h = make_harness(false).await;

    let _ = h
        .client
        .call(
            "set_lighting",
            json!({
                "dev": "galahad2-lcdv2:0001",
                "kind": "fan",
                "cfg": {
                    "mode": "static",
                    "color": { "r": 200, "g": 30, "b": 90 },
                    "brightness_percent": 80,
                    "speed_percent": 50,
                    "direction": "forward",
                    "scope": null,
                    "disabled": false,
                },
            }),
        )
        .await
        .expect("set_lighting");

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_token_roundtrip_consumes_token() {
    let h = make_harness(false).await;

    // Mint a fresh token.
    let v = h
        .client
        .call(
            "request_reset_token",
            json!({ "dev": "galahad2-lcdv2:0001" }),
        )
        .await
        .expect("request_reset_token");
    let token: ResetToken = serde_json::from_value(v).expect("decode ResetToken");
    assert!(!token.0.is_empty(), "minted token must be non-empty");

    // For reset_device, the MockBackend has no canned response queued, so
    // the hid_io drain helper will exhaust its retries and surface a
    // DeviceBusy error. That is fine: the goal here is to prove the
    // token validation path consumes the slot, NOT to drive a successful
    // reset round-trip (which would require canning a parseable cmd 142
    // ack into the mock for every read attempt).
    let _ = h
        .client
        .call(
            "reset_device",
            json!({ "dev": "galahad2-lcdv2:0001", "confirm": token.0 }),
        )
        .await;

    // Re-using the same token must now fail with ConfirmationRequired:
    // single-use semantics consume the slot whether or not the reset
    // round-trip succeeded.
    let err = h
        .client
        .call(
            "reset_device",
            json!({ "dev": "galahad2-lcdv2:0001", "confirm": token.0 }),
        )
        .await
        .expect_err("re-using a token must fail");
    match err {
        IpcError::ConfirmationRequired => {}
        other => panic!("expected ConfirmationRequired, got: {other:?}"),
    }

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_device_with_unknown_token_rejected() {
    let h = make_harness(false).await;

    let err = h
        .client
        .call(
            "reset_device",
            json!({ "dev": "galahad2-lcdv2:0001", "confirm": "not-a-valid-token" }),
        )
        .await
        .expect_err("unknown token must be rejected");
    match err {
        IpcError::ConfirmationRequired => {}
        other => panic!("expected ConfirmationRequired, got: {other:?}"),
    }

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_send_disabled_by_default() {
    let h = make_harness(false).await;

    let err = h
        .client
        .call(
            "raw_send",
            json!({
                "dev": "galahad2-lcdv2:0001",
                "framing": "a",
                "bytes": [0x01, 0x81],
            }),
        )
        .await
        .expect_err("raw_send must be disabled by default");
    match err {
        IpcError::NotImplemented(msg) => {
            assert!(
                msg.to_lowercase().contains("raw_send") || msg.to_lowercase().contains("disabled"),
                "expected raw_send-disabled message, got: {msg}"
            );
        }
        other => panic!("expected NotImplemented, got: {other:?}"),
    }

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_device_id_returns_device_not_found() {
    let h = make_harness(false).await;

    let err = h
        .client
        .call(
            "set_fan_pwm",
            json!({ "dev": "galahad2-lcdv2:9999", "percent": 50 }),
        )
        .await
        .expect_err("unknown device id must be rejected");
    match err {
        // The client maps the JSON-RPC error code -32000 back to
        // DeviceNotFound, but the carried string is the wire-formatted
        // Display ("device not found: <id>"), not the bare id, since
        // that's what `IpcError::to_string()` emitted on the server side.
        IpcError::DeviceNotFound(msg) => {
            assert!(
                msg.contains("galahad2-lcdv2:9999"),
                "expected DeviceNotFound to mention the missing id, got: {msg}"
            );
        }
        other => panic!("expected DeviceNotFound, got: {other:?}"),
    }

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn is_lcd_available_returns_true_for_galahad2() {
    let h = make_harness(false).await;

    let v = h
        .client
        .call("is_lcd_available", json!({ "dev": "galahad2-lcdv2:0001" }))
        .await
        .expect("is_lcd_available");
    assert_eq!(v.as_bool(), Some(true));

    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_lighting_info_returns_static_catalog() {
    let h = make_harness(false).await;

    let v = h
        .client
        .call(
            "get_lighting_info",
            json!({ "dev": "galahad2-lcdv2:0001", "kind": "fan" }),
        )
        .await
        .expect("get_lighting_info fan");
    let max_speed = v.get("max_speed").and_then(|x| x.as_u64()).unwrap();
    let max_brightness = v.get("max_brightness").and_then(|x| x.as_u64()).unwrap();
    let modes = v.get("modes").and_then(|x| x.as_array()).unwrap();
    assert_eq!(max_speed, 100);
    assert_eq!(max_brightness, 100);
    assert!(
        modes.iter().any(|m| m == "rainbow"),
        "fan modes catalog must include rainbow, got: {modes:?}"
    );

    h.shutdown().await;
}

/// Regression test for the P1H-1 finding: clear_lcd MUST issue an
/// LCDControl{mode=LocalUI} (cmd 12) write -- not just drop the
/// keeper's frame slot. Pre-fix the daemon assumed the firmware self-
/// reverted, but live smoke proved the panel held the last frame
/// indefinitely.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clear_lcd_sends_lcd_control_localui() {
    let h = make_harness(false).await;

    let info = lcdv2_info("SN-A", "/mock/hidraw0");
    let history_before = h.mock.write_history(&info).len();

    let _ = h
        .client
        .call("clear_lcd", json!({ "dev": "galahad2-lcdv2:0001" }))
        .await
        .expect("clear_lcd");

    // The MockBackend records every write the daemon makes. clear_lcd
    // should add at least one new write whose cmd-byte (offset 1) is
    // 12 (LCDControl). The setting's mode field is at offset 6 of the
    // payload (which itself starts after the 6-byte A-class header,
    // so absolute offset 6 in the packet).
    let history_after = h.mock.write_history(&info);
    let new_writes = &history_after[history_before..];

    let lcd_control_cmd = 12u8;
    let lcd_control_writes: Vec<&Vec<u8>> = new_writes
        .iter()
        .filter(|w| w.len() >= 2 && w[1] == lcd_control_cmd)
        .collect();
    assert!(
        !lcd_control_writes.is_empty(),
        "clear_lcd must issue at least one LCDControl (cmd {lcd_control_cmd}) write; \
         got new write cmds: {:?}",
        new_writes
            .iter()
            .map(|w| w.get(1).copied())
            .collect::<Vec<_>>()
    );

    h.shutdown().await;
}

#[tokio::test]
async fn lcd_state_patch_and_stats_roundtrip_preserves_omitted_fields() {
    let h = make_harness(false).await;
    let devices: Vec<DeviceDescriptor> =
        serde_json::from_value(h.client.call("list_devices", json!({})).await.unwrap()).unwrap();
    let dev = &devices[0].id;
    let first: llcontrol_ipc::LcdState = serde_json::from_value(
        h.client
            .call(
                "patch_lcd_config",
                json!({"dev":dev,"patch":{"rotation_degrees":90,"brightness_percent":73}}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first.requested.rotation_degrees, 90);
    let second: llcontrol_ipc::LcdState = serde_json::from_value(
        h.client
            .call(
                "patch_lcd_config",
                json!({"dev":dev,"patch":{"brightness_percent":40}}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(second.requested.rotation_degrees, 90);
    assert_eq!(second.requested.brightness_percent, 40);
    let state: llcontrol_ipc::LcdState = serde_json::from_value(
        h.client
            .call("get_lcd_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(state.persistence.is_some());
    let mut display_state = state;
    display_state.persistence = None; // writer status is separate from actor transaction reply
    assert_eq!(display_state, second);
    let stats: llcontrol_ipc::LcdPlaybackStats = serde_json::from_value(
        h.client
            .call("get_lcd_playback_stats", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats.sent_frames, 0);
    let invalid = h
        .client
        .call(
            "play_lcd_clip",
            json!({"dev":dev,"clip":{"fps":24,"units":[]},"loops":1}),
        )
        .await;
    assert!(invalid.is_err());
    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_lighting_rpc_acknowledges_write_and_propagates_actor_validation() {
    let h = make_harness(false).await;
    let dev = lcdv2_info("SN-A", "/mock/hidraw0");
    let colors = json!([{"r":10,"g":20,"b":30}]);
    h.client
        .call(
            "set_led_colors",
            json!({"dev":"galahad2-lcdv2:0001","target":"screen","colors":colors}),
        )
        .await
        .unwrap();
    let count = h
        .mock
        .write_history(&dev)
        .iter()
        .filter(|w| w.get(1) == Some(&20))
        .count();
    assert!(count >= 1, "reply must follow the physical write");
    let error = h
        .client
        .call(
            "set_led_colors",
            json!({"dev":"galahad2-lcdv2:0001","target":"screen","colors":[]}),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, IpcError::InvalidArgument(_)));
    h.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thermal_rpc_configures_restores_and_persists_only_base_and_rules() {
    use llcontrol_ipc::{SensorReading, SensorStatus, SensorUnit, ThermalLayer, ThermalState};
    let cache = llcontrol_daemon::sensors::SensorCache::default();
    cache.publish(
        std::time::Instant::now(),
        vec![SensorReading {
            id: "cpu:test".into(),
            label: "CPU".into(),
            unit: SensorUnit::Celsius,
            value: Some(80.0),
            status: SensorStatus::Valid,
            observed_unix_ms: Some(1),
            age_ms: Some(0),
        }],
    );
    let h = make_harness_with_sensors(false, cache).await;
    let dev = "galahad2-lcdv2:0001";
    let config = json!({"cpu":{"sensor_id":"cpu:test","threshold_c":70,"color":{"r":255,"g":0,"b":0}},"gpu":null});
    let initial: ThermalState = serde_json::from_value(
        h.client
            .call("set_thermal_config", json!({"dev":dev,"config":config}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        initial.fan.applied,
        Some(ThermalLayer::Alert { .. })
    ));
    assert!(matches!(
        initial.pump.applied,
        Some(ThermalLayer::Alert { .. })
    ));
    for kind in ["fan", "pump"] {
        h.client.call("set_lighting",json!({"dev":dev,"kind":kind,"cfg":{"mode":"static","color":{"r":1,"g":2,"b":3},"brightness_percent":50,"speed_percent":50,"direction":"forward","scope":"outer","disabled":false}})).await.unwrap();
    }
    let s: ThermalState = serde_json::from_value(
        h.client
            .call("set_thermal_config", json!({"dev":dev,"config":config}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(s.fan.applied, Some(ThermalLayer::Alert { .. })));
    let s: ThermalState = serde_json::from_value(
        h.client
            .call("get_thermal_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(s.requested.cpu.is_some());
    tokio::time::sleep(Duration::from_millis(350)).await;
    let key = llcontrol_daemon::persistence::settings_key(&lcdv2_info("SN-A", "/mock/hidraw0"));
    let store = PersistenceStore::new_with_root(h._tmp.path().into());
    let saved = store.read(&key).unwrap();
    assert!(saved.thermal.cpu.is_some());
    assert_eq!(saved.lighting.fan.unwrap().color, "#010203");
    let s: ThermalState = serde_json::from_value(
        h.client
            .call("set_thermal_config", json!({"dev":dev,"config":{}}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(!s.requested.enabled());
    assert_eq!(s.fan.applied, Some(ThermalLayer::Base));
    assert_eq!(s.pump.applied, Some(ThermalLayer::Base));
    h.shutdown().await;
}
