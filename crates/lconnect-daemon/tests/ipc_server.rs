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

use lconnect_daemon::config::Config;
use lconnect_daemon::ipc_server::IpcServerImpl;
use lconnect_daemon::manager::DeviceManager;
use lconnect_daemon::persistence::PersistenceStore;
use lconnect_daemon::stats::Stats;
use lconnect_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend};
use lconnect_ipc::client::IpcClient;
use lconnect_ipc::server::IpcServer;
use lconnect_ipc::transport_memory::mem_pair;
use lconnect_ipc::{DaemonStats, DeviceDescriptor, DeviceId, IpcError, ResetToken};
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
    let tmp = TempDir::new().expect("tempdir");
    let mock = MockBackend::new();
    mock.add_device(lcdv2_info("SN-A", "/mock/hidraw0"));
    let backend: Arc<dyn HidBackend> = Arc::new(mock);

    let store = Arc::new(PersistenceStore::new_with_root(tmp.path().to_path_buf()));
    let manager = DeviceManager::new(backend, store.clone(), fast_poll_config());
    let manager_handle = manager.spawn().await;

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
    let rpc = Arc::new(IpcServerImpl::new(manager_handle, stats, config, store));

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
