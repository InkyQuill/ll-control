//! JSON-RPC 2.0 server over `UnixListener` (or any `AsyncRead+AsyncWrite` stream).
//!
//! [`IpcServer::serve`] binds the socket and accepts long-lived
//! connections, spawning a per-connection task that reads line-delimited
//! JSON-RPC requests, dispatches them to a [`DaemonRpc`] impl, and writes
//! responses on the same connection. The per-connection writer is shared
//! (`Arc<Mutex<W>>`) so request responses and server-pushed notifications
//! (e.g. `hello`, `snapshot`) serialise their writes without interleaving
//! partial JSON lines.
//!
//! # Concurrency model
//!
//! Each accepted connection runs on its own [`tokio::spawn`]ed task, so a
//! slow or stalled client cannot block the accept loop or starve other
//! clients. Streaming subscriptions (e.g. `subscribe_snapshots`) further
//! spawn a per-subscription pump task that shares the connection writer.
//!
//! # Wire format
//!
//! Line-delimited: one JSON object per `\n`. The framing rule is owned
//! by this module — [`crate::jsonrpc`] only describes the envelope.
//!
//! # `subscribe_snapshots`
//!
//! Returns a stream and therefore needs the writer half of the
//! connection to be shared between the request-loop and a stream-pump
//! task. [`handle_connection`] wraps the write half in an
//! `Arc<Mutex<...>>` so the request-response loop and any spawned
//! notification pump can serialise their writes without interleaving
//! partial JSON lines. The dispatcher's `subscribe_snapshots` arm
//! returns immediately with `{"subscribed": true, ...}` and spawns a
//! per-subscription task that consumes the [`BoxStream`] from
//! `IpcServerImpl::subscribe_snapshots` and emits each item as a
//! `"snapshot"` notification.

use crate::{
    jsonrpc::{ErrorObj, Notification, Request, Response},
    DaemonRpc, DeviceId, FramingKind, IpcError, LcdConfig, LedTarget, LightingConfig, LightingKind,
    Pwm, PwmSource, ResetToken,
};
use futures::stream::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Shared writer half for a single connection.
///
/// Held behind `Arc<Mutex<...>>` so the request-response loop and any
/// notification-pump tasks (one per active `subscribe_*` call) can all
/// write to the same socket without interleaving partial JSON lines.
type SharedWriter<W> = Arc<Mutex<W>>;

/// JSON-RPC server bound to a Unix socket.
pub struct IpcServer<R: DaemonRpc> {
    rpc: Arc<R>,
}

impl<R: DaemonRpc> IpcServer<R> {
    /// Create a new server wrapping a [`DaemonRpc`] implementation.
    #[must_use]
    pub fn new(rpc: Arc<R>) -> Self {
        Self { rpc }
    }

    /// Bind the socket and accept connections forever.
    ///
    /// Removes any stale socket file at `socket_path` before binding.
    /// Each accepted connection is handed off to a dedicated
    /// [`tokio::spawn`]ed task, so slow clients cannot stall the accept
    /// loop. The spawned [`tokio::task::JoinHandle`]s are intentionally
    /// detached: a per-connection panic ends that connection but does
    /// not propagate into the daemon's runtime.
    ///
    /// # Errors
    /// Fails if the socket path is already in use, unwritable, or the
    /// listener returns a fatal accept error.
    pub async fn serve(&self, socket_path: &Path) -> std::io::Result<()> {
        // Remove stale socket file if present. Ignore errors here
        // (path may simply not exist). `std::fs` is fine for a one-shot
        // unlink on startup; the crate's tokio dep does not pull in
        // the `fs` feature so we deliberately avoid `tokio::fs`.
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)?;
        info!("IPC server listening at {}", socket_path.display());

        loop {
            let (stream, _addr) = listener.accept().await?;
            let rpc = self.rpc.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, rpc).await {
                    warn!("IPC connection ended with error: {e}");
                }
            });
        }
    }

    /// Run the connection loop against a single duplex stream.
    ///
    /// This is the test-only seam used by the in-memory transport
    /// ([`crate::transport_memory::mem_pair`]) and by the Phase 2 GUI's
    /// mock-daemon harness. The body is identical to one iteration of
    /// [`Self::serve`]'s accept loop, just without the listener.
    ///
    /// `S` is generic so callers can pass either a
    /// [`tokio::io::DuplexStream`] or a real socket (handy for tests
    /// that want to wrap the transport in a tracer or a fault-injector).
    ///
    /// # Errors
    /// Bubbles up the first read or write error from the underlying
    /// stream.
    pub async fn serve_stream<S>(&self, stream: S) -> std::io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        handle_connection(stream, self.rpc.clone()).await
    }
}

/// Drive a single accepted connection: send the `hello` notification,
/// then loop reading line-delimited JSON-RPC requests and writing
/// responses until EOF or a write error.
///
/// Generic over any `AsyncRead + AsyncWrite + Unpin` stream so the same
/// body powers both the path-based accept loop ([`UnixStream`]) and the
/// in-memory transport ([`tokio::io::DuplexStream`]) used by tests and
/// the Phase 2 GUI's mock-daemon harness. Uses [`tokio::io::split`]
/// rather than the `UnixStream`-specific `into_split` so the function is
/// transport-agnostic.
async fn handle_connection<S, R>(stream: S, rpc: Arc<R>) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    R: DaemonRpc,
{
    let (read_half, write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    let writer: SharedWriter<_> = Arc::new(Mutex::new(write_half));

    // Send hello notification on connect so clients can verify protocol
    // and daemon versions before issuing any requests.
    let hello = Notification::new(
        "hello",
        json!({
            "daemon_version": env!("CARGO_PKG_VERSION"),
            "ipc_version": format!(
                "{}.{}",
                crate::IPC_VERSION_MAJOR,
                crate::IPC_VERSION_MINOR,
            ),
        }),
    );
    {
        let mut w = writer.lock().await;
        write_line(&mut *w, &hello).await?;
    }

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            // Peer closed the connection.
            return Ok(());
        }

        let response = dispatch(&rpc, &line, &writer).await;
        let mut w = writer.lock().await;
        write_line(&mut *w, &response).await?;
    }
}

/// Serialise `value` as a single JSON line (`{...}\n`) and write it to
/// `writer`. JSON serialisation of our own envelope types is
/// infallible; if it ever fails we fall back to a hand-built parse-error
/// envelope so the peer still gets a valid frame.
async fn write_line<W, T>(writer: &mut W, value: &T) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
    T: serde::Serialize,
{
    let mut out = serde_json::to_string(value).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"serialize-failed"}}"#
            .to_string()
    });
    out.push('\n');
    if let Err(e) = writer.write_all(out.as_bytes()).await {
        error!("write failed: {e}");
        return Err(e);
    }
    Ok(())
}

/// Spawn a per-subscription task that consumes `stream` and emits each
/// item as a `"snapshot"` [`Notification`] on the shared connection
/// writer.
///
/// The task ends when the stream ends or when a write fails (peer closed
/// the connection). It does **not** abort other in-flight notification
/// pumps -- each subscription is independent. Holding the writer mutex
/// only across the single `write_line` call guarantees the request loop
/// can continue handing back responses between notifications without
/// being starved by a long stream.
fn spawn_snapshot_pump<W, S>(writer: SharedWriter<W>, device_id: DeviceId, stream: S)
where
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    S: futures::stream::Stream<Item = crate::SensorSnapshot> + Send + 'static,
{
    tokio::spawn(async move {
        futures::pin_mut!(stream);
        while let Some(snap) = stream.next().await {
            let snapshot_value = match serde_json::to_value(snap) {
                Ok(v) => v,
                Err(e) => {
                    warn!("snapshot serialise failed: {e}");
                    continue;
                }
            };
            let notif = Notification::new(
                "snapshot",
                json!({
                    "device_id": &device_id,
                    "snapshot": snapshot_value,
                }),
            );
            let mut w = writer.lock().await;
            if write_line(&mut *w, &notif).await.is_err() {
                // Peer closed the connection; bail rather than spinning.
                break;
            }
        }
        debug!(
            "snapshot pump for {} ended (stream closed)",
            device_id.as_str()
        );
    });
}

/// Parse one line, dispatch to the trait, and build the response
/// envelope. A parse failure produces a `-32700` error response with
/// `id: null` because the id could not be recovered from the malformed
/// frame.
///
/// `writer` is the connection's shared write half. Most methods do not
/// touch it -- the caller writes the returned `Response` after this
/// function returns. Streaming methods (e.g. `subscribe_snapshots`)
/// take a clone so they can spawn a notification-pump task that writes
/// onto the same connection without racing the response or future
/// requests.
async fn dispatch<R, W>(rpc: &Arc<R>, line: &str, writer: &SharedWriter<W>) -> Response
where
    R: DaemonRpc,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Response::err(
                None,
                ErrorObj {
                    code: -32700,
                    message: format!("parse error: {e}"),
                    data: None,
                },
            );
        }
    };
    let id = req.id.clone();
    let method = req.method.as_str();
    debug!("ipc rx method={method} id={:?}", req.id);

    let result = call_method(rpc, method, req.params, writer).await;

    match result {
        Ok(value) => Response::ok(id, value),
        Err(e) => Response::err(
            Some(id),
            ErrorObj {
                code: e.jsonrpc_code(),
                message: e.to_string(),
                data: None,
            },
        ),
    }
}

/// Map a method name + JSON params to a [`DaemonRpc`] trait call,
/// returning the JSON-encoded success value or an [`IpcError`].
///
/// One arm per method on the trait. Each arm: deserialise params,
/// invoke the trait method, and JSON-encode the success value.
///
/// `writer` is only consulted by streaming methods that need to spawn
/// a per-subscription notification-pump task on the same connection;
/// most arms ignore it.
async fn call_method<R, W>(
    rpc: &Arc<R>,
    method: &str,
    params: Value,
    writer: &SharedWriter<W>,
) -> crate::Result<Value>
where
    R: DaemonRpc,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    match method {
        // ---- Discovery ----
        "list_devices" => {
            let v = rpc.list_devices().await?;
            to_value(&v)
        }
        "get_firmware" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.get_firmware(p.dev).await?;
            to_value(&v)
        }

        // ---- Sensors ----
        "get_snapshot" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.get_snapshot(p.dev).await?;
            to_value(&v)
        }
        // Streaming subscription. The trait method returns a
        // `BoxStream<SensorSnapshot>`; we spawn a per-subscription
        // pump task that emits each item as a `"snapshot"` notification
        // on the same connection. The call itself returns immediately
        // with `{"subscribed": true, ...}` so the client's `call`
        // future resolves without waiting for the first item.
        "subscribe_snapshots" => {
            let p: DeviceParams = from_value(params)?;
            let device_id = p.dev.clone();
            let stream = rpc.subscribe_snapshots(p.dev).await?;
            spawn_snapshot_pump(writer.clone(), device_id.clone(), stream);
            Ok(json!({
                "subscribed": true,
                "device_id": device_id,
            }))
        }

        // ---- PWM ----
        "set_fan_pwm" => {
            let p: SetFanPwmParams = from_value(params)?;
            rpc.set_fan_pwm(p.dev, p.percent).await?;
            Ok(unit_ok())
        }
        "set_pump_pwm" => {
            let p: SetPumpPwmParams = from_value(params)?;
            rpc.set_pump_pwm(p.dev, p.percent, p.source).await?;
            Ok(unit_ok())
        }

        // ---- Lighting ----
        "get_lighting_info" => {
            let p: LightingKindParams = from_value(params)?;
            let v = rpc.get_lighting_info(p.dev, p.kind).await?;
            to_value(&v)
        }
        "set_lighting" => {
            let p: SetLightingParams = from_value(params)?;
            rpc.set_lighting(p.dev, p.kind, p.cfg).await?;
            Ok(unit_ok())
        }
        "set_led_colors" => {
            let p: SetLedColorsParams = from_value(params)?;
            rpc.set_led_colors(p.dev, p.target, p.colors).await?;
            Ok(unit_ok())
        }

        // ---- LCD ----
        "is_lcd_available" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.is_lcd_available(p.dev).await?;
            to_value(&v)
        }
        "set_lcd_config" => {
            let p: SetLcdConfigParams = from_value(params)?;
            rpc.set_lcd_config(p.dev, p.cfg).await?;
            Ok(unit_ok())
        }
        "set_lcd_image" => {
            let p: SetLcdImageParams = from_value(params)?;
            rpc.set_lcd_image(p.dev, p.jpeg).await?;
            Ok(unit_ok())
        }
        "clear_lcd" => {
            let p: DeviceParams = from_value(params)?;
            rpc.clear_lcd(p.dev).await?;
            Ok(unit_ok())
        }

        // ---- Maintenance ----
        "request_reset_token" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.request_reset_token(p.dev).await?;
            to_value(&v)
        }
        "reset_device" => {
            let p: ResetDeviceParams = from_value(params)?;
            rpc.reset_device(p.dev, p.confirm).await?;
            Ok(unit_ok())
        }
        "raw_send" => {
            let p: RawSendParams = from_value(params)?;
            let v = rpc.raw_send(p.dev, p.framing, p.bytes).await?;
            to_value(&v)
        }
        "daemon_stats" => {
            let v = rpc.daemon_stats().await?;
            to_value(&v)
        }

        unknown => Err(IpcError::NotImplemented(format!(
            "unknown method: {unknown}"
        ))),
    }
}

// --- helpers -----------------------------------------------------------

/// Sentinel "unit result" payload for methods that return `()`.
///
/// We deliberately avoid `Value::Null`: `Some(Value::Null)` round-trips
/// back to `None` on the client side because serde maps JSON `null` to
/// `Option<Value>::None` by default, which the client interprets as
/// "neither result nor error set" -- a transport error. An empty object
/// is unambiguous: it survives the `Option<Value>` deserialiser as
/// `Some(Value::Object(_))` and the dispatcher reaches the success arm.
fn unit_ok() -> Value {
    json!({})
}

/// JSON-encode a serialisable value, mapping any error to
/// [`IpcError::Serialization`]. Used after a successful trait call.
fn to_value<T: serde::Serialize>(v: &T) -> crate::Result<Value> {
    serde_json::to_value(v).map_err(|e| IpcError::Serialization(e.to_string()))
}

/// Decode JSON params into the per-arm parameter struct, mapping any
/// error to [`IpcError::InvalidArgument`] (these are caller-supplied
/// bytes, so the failure mode is "bad request", not "internal error").
fn from_value<T: for<'de> Deserialize<'de>>(params: Value) -> crate::Result<T> {
    serde_json::from_value(params).map_err(|e| IpcError::InvalidArgument(e.to_string()))
}

// --- per-arm parameter structs ----------------------------------------
//
// Defined as small ad-hoc `Deserialize` types so each match arm gets a
// single typed `from_value` call rather than fishing fields out of the
// raw `Value`. Field names here are the on-the-wire param names; if a
// caller renames them, the JSON-RPC layer will reject it.

#[derive(Deserialize)]
struct DeviceParams {
    dev: DeviceId,
}

#[derive(Deserialize)]
struct SetFanPwmParams {
    dev: DeviceId,
    percent: Pwm,
}

#[derive(Deserialize)]
struct SetPumpPwmParams {
    dev: DeviceId,
    percent: Pwm,
    source: PwmSource,
}

#[derive(Deserialize)]
struct LightingKindParams {
    dev: DeviceId,
    kind: LightingKind,
}

#[derive(Deserialize)]
struct SetLightingParams {
    dev: DeviceId,
    kind: LightingKind,
    cfg: LightingConfig,
}

#[derive(Deserialize)]
struct SetLedColorsParams {
    dev: DeviceId,
    target: LedTarget,
    colors: Vec<crate::Rgb>,
}

#[derive(Deserialize)]
struct SetLcdConfigParams {
    dev: DeviceId,
    cfg: LcdConfig,
}

#[derive(Deserialize)]
struct SetLcdImageParams {
    dev: DeviceId,
    jpeg: Vec<u8>,
}

#[derive(Deserialize)]
struct ResetDeviceParams {
    dev: DeviceId,
    confirm: ResetToken,
}

#[derive(Deserialize)]
struct RawSendParams {
    dev: DeviceId,
    framing: FramingKind,
    bytes: Vec<u8>,
}
