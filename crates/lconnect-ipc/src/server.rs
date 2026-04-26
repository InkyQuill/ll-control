//! JSON-RPC 2.0 server over `UnixListener`.
//!
//! [`IpcServer::serve`] binds the socket, accepts long-lived connections,
//! reads line-delimited JSON-RPC requests, dispatches them to a
//! [`DaemonRpc`] impl, and writes responses on the same connection.
//! Server-pushed notifications (e.g. `hello`, `snapshot`) ride the same
//! connection.
//!
//! # Concurrency model
//!
//! Connections are driven concurrently inside a single task using
//! [`futures::stream::FuturesUnordered`]. The crate's tokio dependency
//! does not enable the `rt` feature, so `tokio::spawn` is unavailable
//! here; callers that want to handle accept errors or co-locate the
//! server with other tasks should drive [`IpcServer::serve`] from the
//! daemon's runtime.
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
//! task. The full streaming wiring lands in P1D-13 alongside the
//! daemon-side `IpcServerImpl`. For now the dispatch arm short-circuits
//! with [`IpcError::NotImplemented`].

use crate::{
    jsonrpc::{ErrorObj, Notification, Request, Response},
    DaemonRpc, DeviceId, FramingKind, IpcError, LcdConfig, LedTarget, LightingConfig, LightingKind,
    Pwm, PwmSource, ResetToken,
};
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tracing::{debug, error, info, warn};

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
    /// Each accepted connection is driven to completion concurrently
    /// alongside the accept loop via [`FuturesUnordered`].
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

        let mut connections: FuturesUnordered<_> = FuturesUnordered::new();
        loop {
            // Drive the accept loop and any in-flight connections
            // concurrently. We do not depend on the tokio `macros`
            // feature here, so we use `futures::future::select` over
            // hand-fused futures rather than `tokio::select!`.
            let accept = listener.accept().fuse();
            futures::pin_mut!(accept);

            // If there are no in-flight connections, just wait for the
            // next accept; otherwise race accept against the next
            // completion from the connection set.
            if connections.is_empty() {
                let (stream, _addr) = accept.await?;
                connections.push(spawn_connection(stream, self.rpc.clone()));
            } else {
                let next = connections.next();
                futures::pin_mut!(next);
                match futures::future::select(accept, next).await {
                    futures::future::Either::Left((accept_res, _next)) => {
                        let (stream, _addr) = accept_res?;
                        connections.push(spawn_connection(stream, self.rpc.clone()));
                    }
                    futures::future::Either::Right((_done, _accept)) => {
                        // A connection finished; loop and re-arm.
                    }
                }
            }
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

/// Wrap [`handle_connection`] so it returns `()` (suitable for stuffing
/// into a [`FuturesUnordered`]) and logs any per-connection error.
async fn spawn_connection<S, R>(stream: S, rpc: Arc<R>)
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    R: DaemonRpc,
{
    if let Err(e) = handle_connection(stream, rpc).await {
        warn!("IPC connection ended with error: {e}");
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
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);

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
    write_line(&mut write_half, &hello).await?;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            // Peer closed the connection.
            return Ok(());
        }

        let response = dispatch(&rpc, &line).await;
        write_line(&mut write_half, &response).await?;
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

/// Parse one line, dispatch to the trait, and build the response
/// envelope. A parse failure produces a `-32700` error response with
/// `id: null` because the id could not be recovered from the malformed
/// frame.
async fn dispatch<R: DaemonRpc>(rpc: &Arc<R>, line: &str) -> Response {
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

    let result = call_method(rpc, method, req.params).await;

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
async fn call_method<R: DaemonRpc>(
    rpc: &Arc<R>,
    method: &str,
    params: Value,
) -> crate::Result<Value> {
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
        // TODO(P1D-13): wire streaming subscription. Returning a stream
        // requires sharing the connection's write half with a pump task,
        // which is built out alongside the daemon's `IpcServerImpl`.
        "subscribe_snapshots" => Err(IpcError::NotImplemented(
            "subscribe_snapshots wired in P1D-13".into(),
        )),

        // ---- PWM ----
        "set_fan_pwm" => {
            let p: SetFanPwmParams = from_value(params)?;
            rpc.set_fan_pwm(p.dev, p.percent).await?;
            Ok(Value::Null)
        }
        "set_pump_pwm" => {
            let p: SetPumpPwmParams = from_value(params)?;
            rpc.set_pump_pwm(p.dev, p.percent, p.source).await?;
            Ok(Value::Null)
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
            Ok(Value::Null)
        }
        "set_led_colors" => {
            let p: SetLedColorsParams = from_value(params)?;
            rpc.set_led_colors(p.dev, p.target, p.colors).await?;
            Ok(Value::Null)
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
            Ok(Value::Null)
        }
        "set_lcd_image" => {
            let p: SetLcdImageParams = from_value(params)?;
            rpc.set_lcd_image(p.dev, p.jpeg).await?;
            Ok(Value::Null)
        }
        "clear_lcd" => {
            let p: DeviceParams = from_value(params)?;
            rpc.clear_lcd(p.dev).await?;
            Ok(Value::Null)
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
            Ok(Value::Null)
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
