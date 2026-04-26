//! End-to-end round-trip over the in-memory duplex transport.
//!
//! These tests do the same shape of round-trip as
//! `client_server_roundtrip.rs`, but pipe the bytes through a
//! `tokio::io::DuplexStream` pair instead of a real Unix socket. That
//! removes the filesystem and the connect/bind race entirely, making
//! the test seam fast, deterministic, and (more importantly) the same
//! seam the Phase 2 GUI's mock daemon attaches to.
//!
//! Coverage:
//!
//! 1. `list_devices_roundtrip` — data-bearing call: the canned
//!    `Vec<DeviceDescriptor>` survives both directions of the
//!    JSON-RPC pipe.
//! 2. `not_implemented_error_roundtrip` — error path: a method the
//!    mock returns `IpcError::NotImplemented` for surfaces as the same
//!    variant on the client (verifies the `-32003` code mapping over
//!    the in-memory transport, not just the path-based one).
//! 3. `set_fan_pwm_dispatch_shape` — a method with structured params:
//!    even though the mock returns NotImplemented, the call is only
//!    valid if the server-side dispatcher successfully decodes the
//!    `{ dev, percent }` shape. A `InvalidArgument` here would mean
//!    the wire shape is wrong.
//! 4. `subscribe_snapshots_emits_notifications` — streaming path:
//!    `subscribe_snapshots` returns immediately with `subscribed: true`
//!    while a server-side pump task emits each `SensorSnapshot` from
//!    the trait method's `BoxStream` as a `"snapshot"` notification on
//!    the same connection.

use async_trait::async_trait;
use futures::stream::BoxStream;
use lconnect_ipc::client::IpcClient;
use lconnect_ipc::server::IpcServer;
use lconnect_ipc::transport_memory::mem_pair;
use lconnect_ipc::{
    DaemonRpc, DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, IpcError,
    LcdConfig, LedTarget, LightingConfig, LightingInfo, LightingKind, Pwm, PwmSource, ResetToken,
    Result, Rgb, SensorSnapshot,
};
use serde_json::json;
use std::sync::Arc;

/// Mock impl. `list_devices` returns one canned descriptor; everything
/// else returns [`IpcError::NotImplemented`] so the tests can probe both
/// the success path and the error mapping without hand-stubbing 17
/// different return shapes.
struct MockRpc;

impl MockRpc {
    fn canned_descriptor() -> DeviceDescriptor {
        // `DeviceDescriptor` is `#[non_exhaustive]`; build via Serde
        // instead of struct-literal so external test crates don't
        // depend on its full field list.
        serde_json::from_value(json!({
            "id": "mock:0001",
            "family": "mock",
            "display_name": "Mock",
            "vid": 0xDEAD,
            "pid": 0xBEEF,
            "serial": null,
            "hid_path": "/mock",
        }))
        .expect("build mock DeviceDescriptor")
    }
}

#[async_trait]
impl DaemonRpc for MockRpc {
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        Ok(vec![Self::canned_descriptor()])
    }
    async fn get_firmware(&self, _dev: DeviceId) -> Result<FirmwareInfo> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn get_snapshot(&self, _dev: DeviceId) -> Result<SensorSnapshot> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn subscribe_snapshots(
        &self,
        _dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_fan_pwm(&self, _dev: DeviceId, _percent: Pwm) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_pump_pwm(&self, _dev: DeviceId, _percent: Pwm, _source: PwmSource) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn get_lighting_info(&self, _dev: DeviceId, _kind: LightingKind) -> Result<LightingInfo> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lighting(
        &self,
        _dev: DeviceId,
        _kind: LightingKind,
        _cfg: LightingConfig,
    ) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_led_colors(
        &self,
        _dev: DeviceId,
        _target: LedTarget,
        _colors: Vec<Rgb>,
    ) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn is_lcd_available(&self, _dev: DeviceId) -> Result<bool> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lcd_config(&self, _dev: DeviceId, _cfg: LcdConfig) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lcd_image(&self, _dev: DeviceId, _jpeg: Vec<u8>) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn clear_lcd(&self, _dev: DeviceId) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn request_reset_token(&self, _dev: DeviceId) -> Result<ResetToken> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn reset_device(&self, _dev: DeviceId, _confirm: ResetToken) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn raw_send(
        &self,
        _dev: DeviceId,
        _framing: FramingKind,
        _bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn daemon_stats(&self) -> Result<DaemonStats> {
        Err(IpcError::NotImplemented("mock".into()))
    }
}

/// Stand up a server + client over a fresh `mem_pair` and return the
/// connected client plus the server task's `JoinHandle`. Centralised
/// because every test wants the same boilerplate.
async fn connect_pair() -> (IpcClient, tokio::task::JoinHandle<()>) {
    let (client_end, server_end) = mem_pair();
    let server = IpcServer::new(Arc::new(MockRpc));
    let server_task = tokio::spawn(async move {
        // `serve_stream` returns when the peer closes the duplex, which
        // happens when the test drops the client. Errors here are
        // expected on shutdown, so we swallow them.
        let _ = server.serve_stream(server_end).await;
    });
    let client = IpcClient::from_stream(client_end)
        .await
        .expect("client from_stream");
    (client, server_task)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_devices_roundtrip() {
    let (client, server_task) = connect_pair().await;

    // Sanity-check the hello payload — same shape the path-based
    // server publishes, since both go through `handle_connection`.
    assert_eq!(client.hello.method, "hello");
    assert!(client.hello.params.get("daemon_version").is_some());
    assert!(client.hello.params.get("ipc_version").is_some());

    let v = client
        .call("list_devices", json!({}))
        .await
        .expect("list_devices");
    let descs: Vec<DeviceDescriptor> = serde_json::from_value(v).expect("decode DeviceDescriptor");
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].id.as_str(), "mock:0001");
    assert_eq!(descs[0].vid, 0xDEAD);
    assert_eq!(descs[0].pid, 0xBEEF);

    drop(client);
    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn not_implemented_error_roundtrip() {
    let (client, server_task) = connect_pair().await;

    // `get_firmware` on the mock returns `NotImplemented`. The client
    // must see the same variant back (verifies the -32003 code mapping
    // round-trips through the in-memory transport).
    let err = client
        .call("get_firmware", json!({ "dev": "mock:0001" }))
        .await
        .expect_err("get_firmware should error");
    match err {
        IpcError::NotImplemented(msg) => {
            assert!(
                msg.contains("mock"),
                "expected mock's message to round-trip, got: {msg}"
            );
        }
        other => panic!("expected NotImplemented, got: {other:?}"),
    }

    drop(client);
    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_fan_pwm_dispatch_shape() {
    let (client, server_task) = connect_pair().await;

    // `set_fan_pwm` takes structured params (`dev`, `percent`). The
    // mock returns `NotImplemented`, so a successful arrival at the
    // mock is exactly what we want to assert: it proves the
    // server-side `SetFanPwmParams` deserialiser accepted the wire
    // shape. If the dispatcher had rejected the params, the client
    // would see `IpcError::InvalidArgument` instead — which would
    // signal a wire-shape regression.
    let err = client
        .call("set_fan_pwm", json!({ "dev": "mock:0001", "percent": 50 }))
        .await
        .expect_err("set_fan_pwm should error (mock returns NotImplemented)");
    match err {
        IpcError::NotImplemented(msg) => {
            assert!(
                msg.contains("mock"),
                "expected mock's NotImplemented to round-trip, got: {msg}"
            );
        }
        IpcError::InvalidArgument(msg) => {
            panic!(
                "set_fan_pwm wire shape rejected by dispatcher: {msg} \
                 (dispatcher expected dev + percent)"
            );
        }
        other => panic!("expected NotImplemented, got: {other:?}"),
    }

    drop(client);
    server_task.abort();
    let _ = server_task.await;
}

/// Mock impl for the streaming-subscription test.
///
/// `subscribe_snapshots` returns a `BoxStream` that yields three canned
/// `SensorSnapshot` values then ends (`futures::stream::iter` over a
/// fixed array, boxed into a `BoxStream`). Every other method returns
/// `NotImplemented`; only `subscribe_snapshots` matters for this test.
struct StreamingMockRpc;

impl StreamingMockRpc {
    fn canned_snapshots() -> [SensorSnapshot; 3] {
        // `SensorSnapshot` is `#[non_exhaustive]`; build via Serde so
        // the test does not depend on its full field list.
        let mk = |fan_rpm: u16| -> SensorSnapshot {
            serde_json::from_value(json!({
                "coolant_c_x10": 305_i16,
                "fan_rpm": fan_rpm,
                "pump_rpm": 2200_u16,
                "temp_valid": true,
                "last_seen_unix_ms": 1_700_000_000_000_u64,
            }))
            .expect("build mock SensorSnapshot")
        };
        [mk(800), mk(900), mk(1000)]
    }
}

#[async_trait]
impl DaemonRpc for StreamingMockRpc {
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn get_firmware(&self, _dev: DeviceId) -> Result<FirmwareInfo> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn get_snapshot(&self, _dev: DeviceId) -> Result<SensorSnapshot> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn subscribe_snapshots(
        &self,
        _dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>> {
        let items = Self::canned_snapshots();
        let stream = futures::stream::iter(items);
        Ok(Box::pin(stream))
    }
    async fn set_fan_pwm(&self, _dev: DeviceId, _percent: Pwm) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_pump_pwm(&self, _dev: DeviceId, _percent: Pwm, _source: PwmSource) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn get_lighting_info(&self, _dev: DeviceId, _kind: LightingKind) -> Result<LightingInfo> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lighting(
        &self,
        _dev: DeviceId,
        _kind: LightingKind,
        _cfg: LightingConfig,
    ) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_led_colors(
        &self,
        _dev: DeviceId,
        _target: LedTarget,
        _colors: Vec<Rgb>,
    ) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn is_lcd_available(&self, _dev: DeviceId) -> Result<bool> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lcd_config(&self, _dev: DeviceId, _cfg: LcdConfig) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn set_lcd_image(&self, _dev: DeviceId, _jpeg: Vec<u8>) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn clear_lcd(&self, _dev: DeviceId) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn request_reset_token(&self, _dev: DeviceId) -> Result<ResetToken> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn reset_device(&self, _dev: DeviceId, _confirm: ResetToken) -> Result<()> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn raw_send(
        &self,
        _dev: DeviceId,
        _framing: FramingKind,
        _bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented("mock".into()))
    }
    async fn daemon_stats(&self) -> Result<DaemonStats> {
        Err(IpcError::NotImplemented("mock".into()))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_snapshots_emits_notifications() {
    use std::time::Duration;

    let (client_end, server_end) = mem_pair();
    let server = IpcServer::new(Arc::new(StreamingMockRpc));
    let server_task = tokio::spawn(async move {
        let _ = server.serve_stream(server_end).await;
    });
    let client = IpcClient::from_stream(client_end)
        .await
        .expect("client from_stream");

    // Register the listener BEFORE issuing the call so we don't race
    // the server: the pump task spawns inside the dispatcher and may
    // start writing notifications immediately.
    let mut sub = client.subscribe("snapshot").await;

    // The call returns immediately with `{"subscribed": true, ...}`;
    // the actual snapshot stream rides as notifications afterwards.
    let v = client
        .call("subscribe_snapshots", json!({ "dev": "mock:0001" }))
        .await
        .expect("subscribe_snapshots call");
    assert_eq!(v.get("subscribed").and_then(|b| b.as_bool()), Some(true));
    assert_eq!(
        v.get("device_id").and_then(|d| d.as_str()),
        Some("mock:0001"),
    );

    // Read three notifications. Bound the wait so a regression that
    // drops items hangs the test deterministically rather than
    // wedging the suite.
    let collect = async {
        let mut got = Vec::new();
        for _ in 0..3 {
            let n = sub.recv().await.expect("notification stream open");
            got.push(n);
        }
        got
    };
    let notifications = tokio::time::timeout(Duration::from_secs(2), collect)
        .await
        .expect("expected 3 notifications within 2s");

    assert_eq!(notifications.len(), 3);
    let canned = StreamingMockRpc::canned_snapshots();
    for (i, notif) in notifications.iter().enumerate() {
        assert_eq!(notif.method, "snapshot");
        assert_eq!(
            notif.params.get("device_id").and_then(|d| d.as_str()),
            Some("mock:0001"),
            "notification {i} device_id",
        );
        let snap_val = notif
            .params
            .get("snapshot")
            .expect("notification has snapshot field");
        let snap: SensorSnapshot =
            serde_json::from_value(snap_val.clone()).expect("decode SensorSnapshot");
        assert_eq!(snap.fan_rpm, canned[i].fan_rpm);
        assert_eq!(snap.pump_rpm, canned[i].pump_rpm);
        assert_eq!(snap.coolant_c_x10, canned[i].coolant_c_x10);
        assert!(snap.temp_valid);
    }

    drop(client);
    server_task.abort();
    let _ = server_task.await;
}
