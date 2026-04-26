//! End-to-end round-trip: [`IpcClient`] and [`IpcServer`] over a real
//! `UnixStream` pair on a temp filesystem socket.
//!
//! Covers three cases:
//!
//! 1. `list_devices` round-trips the server's canned `Vec<DeviceDescriptor>`.
//! 2. `daemon_stats` round-trips the server's canned [`DaemonStats`].
//! 3. A method the mock returns [`IpcError::NotImplemented`] for surfaces
//!    as `IpcError::NotImplemented` on the client (verifies the round-trip
//!    of the `-32003` JSON-RPC error code through both sides of the
//!    mapping).
//!
//! The test exercises [`crate::jsonrpc`] (envelope types), [`crate::server`]
//! (dispatch + framing), and [`crate::client`] (multiplexing) together —
//! none of those modules has a unit test that covers the whole stack.

use async_trait::async_trait;
use futures::stream::BoxStream;
use lconnect_ipc::client::IpcClient;
use lconnect_ipc::server::IpcServer;
use lconnect_ipc::{
    DaemonRpc, DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, IpcError,
    LcdConfig, LedTarget, LightingConfig, LightingInfo, LightingKind, Pwm, PwmSource, ResetToken,
    Result, Rgb, SensorSnapshot,
};
use serde_json::json;
use std::sync::Arc;

/// Mock daemon impl. Only `list_devices` and `daemon_stats` return real
/// data; everything else returns [`IpcError::NotImplemented`] so the test
/// can verify the error round-trip without listing every method.
struct MockDaemon {
    devices: Vec<DeviceDescriptor>,
    stats: DaemonStats,
}

impl MockDaemon {
    fn new() -> Self {
        // `DeviceDescriptor` and `DaemonStats` are `#[non_exhaustive]`,
        // so external crates can't construct them with struct-literal
        // syntax. Build them via Serde from a JSON literal — same
        // shape, no future-compat hazard.
        let descriptor: DeviceDescriptor = serde_json::from_value(json!({
            "id": "galahad2-lcdv2:test-0001",
            "family": "galahad2-lcdv2",
            "display_name": "Galahad II LCDV2 (mock)",
            "vid": 0x0cf2,
            "pid": 0xa100,
            "serial": "MOCK-SERIAL-0001",
            "hid_path": "/dev/null",
        }))
        .expect("build DeviceDescriptor");
        let stats: DaemonStats = serde_json::from_value(json!({
            "uptime_seconds": 12345,
            "devices_known": 1,
            "hid_write_count": 42,
            "hid_read_errors": 0,
            "hid_decode_errors": 0,
            "ipc_request_count": 7,
        }))
        .expect("build DaemonStats");
        Self {
            devices: vec![descriptor],
            stats,
        }
    }
}

#[async_trait]
impl DaemonRpc for MockDaemon {
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        Ok(self.devices.clone())
    }
    async fn daemon_stats(&self) -> Result<DaemonStats> {
        Ok(self.stats.clone())
    }

    // --- Everything else: not implemented in the mock. -----------------

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
}

/// Spin up a server on a temp socket, connect a client, and exercise
/// both data-bearing and error paths.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_server_roundtrip() {
    // Temp dir auto-cleans on drop; the socket file inside is unlinked
    // along with the directory.
    let tmp = tempfile::tempdir().expect("tempdir");
    let socket_path = tmp.path().join("lconnect-ipc-test.sock");

    // Start the server. `IpcServer::serve` runs forever, so park it in a
    // background task and abort it at the end.
    let mock = Arc::new(MockDaemon::new());
    let server = IpcServer::new(mock);
    let server_path = socket_path.clone();
    let server_task = tokio::spawn(async move {
        let _ = server.serve(&server_path).await;
    });

    // Wait for the server to publish the socket. Bind happens inside the
    // task, so `connect` can race the bind on a cold runtime; retry a
    // handful of times with a short sleep before giving up.
    let client = {
        let mut attempt = 0;
        loop {
            match IpcClient::connect(&socket_path).await {
                Ok(c) => break c,
                Err(_) if attempt < 20 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                Err(e) => panic!("client failed to connect after retries: {e}"),
            }
        }
    };

    // Sanity-check the hello payload — server.rs publishes
    // method = "hello" with `daemon_version` + `ipc_version`.
    assert_eq!(client.hello.method, "hello");
    assert!(client.hello.params.get("daemon_version").is_some());
    assert!(client.hello.params.get("ipc_version").is_some());

    // 1. list_devices: data-bearing call, returns the canned vector.
    let v = client
        .call("list_devices", json!({}))
        .await
        .expect("list_devices");
    let devs: Vec<DeviceDescriptor> = serde_json::from_value(v).expect("decode DeviceDescriptor");
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].id.as_str(), "galahad2-lcdv2:test-0001");
    assert_eq!(devs[0].vid, 0x0cf2);
    assert_eq!(devs[0].pid, 0xa100);

    // 2. daemon_stats: another data-bearing call with different shape.
    let v = client
        .call("daemon_stats", json!({}))
        .await
        .expect("daemon_stats");
    let stats: DaemonStats = serde_json::from_value(v).expect("decode DaemonStats");
    assert_eq!(stats.uptime_seconds, 12345);
    assert_eq!(stats.devices_known, 1);
    assert_eq!(stats.hid_write_count, 42);
    assert_eq!(stats.ipc_request_count, 7);

    // 3. NotImplemented round-trip: the mock returns NotImplemented for
    //    `get_firmware`; the client must see the same variant back, with
    //    the message preserved through the JSON-RPC envelope.
    let err = client
        .call("get_firmware", json!({ "dev": "galahad2-lcdv2:test-0001" }))
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

    // Drop the client, then stop the server task. The tempdir cleans up
    // the socket file when it goes out of scope.
    drop(client);
    server_task.abort();
    let _ = server_task.await;
}
