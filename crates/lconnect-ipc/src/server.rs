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
//! clients. Theme Apply/preview reserve bounded work in request order, then
//! reply by ID independently; later control requests can continue on the same
//! connection. Disconnect aborts deferred waiters, not already accepted work.
//! Streaming subscriptions (e.g. `subscribe_snapshots`) further
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
//! task. The per-connection handler wraps the write half in an
//! `Arc<Mutex<...>>` so the request-response loop and any spawned
//! notification pump can serialise their writes without interleaving
//! partial JSON lines. The dispatcher's `subscribe_snapshots` arm
//! returns immediately with `{"subscribed": true, ...}` and spawns a
//! per-subscription task that consumes the
//! [`futures::stream::BoxStream`] returned from
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
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
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
    let slots = Arc::new(Semaphore::new(4));
    let mut deferred = JoinSet::new();
    let mut export_started = false;
    loop {
        while deferred.try_join_next().is_some() {}
        line.clear();
        let n = crate::framing::read_bounded_line(
            &mut reader,
            &mut line,
            crate::framing::MAX_JSON_RPC_LINE_BYTES,
        )
        .await?;
        if n == 0 {
            // Peer closed the connection.
            return Ok(());
        }

        if let Some(response) = dispatch(
            &rpc,
            &line,
            &writer,
            &mut deferred,
            &slots,
            &mut export_started,
        )
        .await
        {
            let mut w = writer.lock().await;
            write_line(&mut *w, &response).await?;
        }
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
async fn dispatch<R, W>(
    rpc: &Arc<R>,
    line: &str,
    writer: &SharedWriter<W>,
    deferred: &mut JoinSet<()>,
    slots: &Arc<Semaphore>,
    export_started: &mut bool,
) -> Option<Response>
where
    R: DaemonRpc,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Some(Response::err(
                None,
                ErrorObj {
                    code: -32700,
                    message: format!("parse error: {e}"),
                    data: None,
                },
            ));
        }
    };
    let id = req.id.clone();
    let method = req.method.as_str();
    debug!("ipc rx method={method} id={:?}", req.id);

    if method == "open_media_export" {
        if *export_started {
            return Some(response_for(
                id,
                Err(IpcError::InvalidArgument(
                    "Use a new dedicated connection for each export".into(),
                )),
            ));
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Params {
            key: String,
            stream_id: String,
        }
        let params = match serde_json::from_value::<Params>(req.params) {
            Ok(p)
                if p.stream_id.len() == 32
                    && p.stream_id
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                p
            }
            _ => {
                return Some(response_for(
                    id,
                    Err(IpcError::InvalidArgument(
                        "Expected key and 32 lowercase hex stream_id".into(),
                    )),
                ))
            }
        };
        let permit = match slots.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                return Some(response_for(
                    id,
                    Err(IpcError::DeviceBusy("Too many pending requests".into())),
                ))
            }
        };
        *export_started = true;
        let rpc = rpc.clone();
        let writer = writer.clone();
        // The connection owns both opening and pumping. EOF drops the JoinSet;
        // blocking storage work retains its own pin/admission until it exits.
        deferred.spawn(async move {
            let _permit = permit;
            let mut source = match rpc.open_media_export(params.key).await {
                Ok(source) => source,
                Err(error) => {
                    let _ =
                        write_line(&mut *writer.lock().await, &response_for(id, Err(error))).await;
                    return;
                }
            };
            let response =
                Response::ok(id, json!({"stream_id":params.stream_id,"info":source.info}));
            if write_line(&mut *writer.lock().await, &response)
                .await
                .is_err()
            {
                return;
            }
            let mut index = 0;
            let mut bytes = 0u64;
            while let Some(result) = source.units.next().await {
                let event = match result {
                    Ok(unit)
                        if index < source.info.frames
                            && bytes.saturating_add(unit.bytes.len() as u64)
                                <= source.info.encoded_bytes =>
                    {
                        bytes += unit.bytes.len() as u64;
                        let event = crate::MediaExportEvent::Unit {
                            stream_id: params.stream_id.clone(),
                            index,
                            unit,
                        };
                        index += 1;
                        event
                    }
                    other => crate::MediaExportEvent::Failed {
                        stream_id: params.stream_id.clone(),
                        message: other
                            .err()
                            .map_or_else(|| "Export exceeds metadata".into(), |e| e.to_string()),
                    },
                };
                let failed = matches!(event, crate::MediaExportEvent::Failed { .. });
                let notification = Notification::new(
                    "media_export",
                    serde_json::to_value(event).expect("export event serializes"),
                );
                if write_line(&mut *writer.lock().await, &notification)
                    .await
                    .is_err()
                    || failed
                {
                    return;
                }
            }
            let event = if index == source.info.frames && bytes == source.info.encoded_bytes {
                crate::MediaExportEvent::Complete {
                    stream_id: params.stream_id,
                }
            } else {
                crate::MediaExportEvent::Failed {
                    stream_id: params.stream_id,
                    message: "Incomplete media export".into(),
                }
            };
            let notification = Notification::new(
                "media_export",
                serde_json::to_value(event).expect("export event serializes"),
            );
            let _ = write_line(&mut *writer.lock().await, &notification).await;
        });
        return None;
    }
    if matches!(
        method,
        "set_lcd_theme" | "preview_lcd_theme" | "preview_lcd_editor"
    ) {
        let admitted = match slots.clone().try_acquire_owned() {
            Ok(permit) => admit_theme(rpc, method, req.params)
                .await
                .map(|work| (work, permit)),
            Err(_) => Err(IpcError::DeviceBusy(
                "too many pending theme requests".into(),
            )),
        };
        match admitted {
            Ok((work, permit)) => {
                let writer = writer.clone();
                deferred.spawn(async move {
                    let _permit = permit;
                    let response = response_for(id, work.await);
                    let mut writer = writer.lock().await;
                    if let Err(error) = write_line(&mut *writer, &response).await {
                        debug!(%error, "deferred theme reply failed");
                    }
                });
                return None;
            }
            Err(error) => return Some(response_for(id, Err(error))),
        }
    }
    Some(response_for(
        id,
        call_method(rpc, method, req.params, writer).await,
    ))
}

fn response_for(id: Value, result: crate::Result<Value>) -> Response {
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

// Admission is awaited in wire order. In particular Apply reserves its owner
// ticket before the reader can dispatch a later Clear or another Apply.
async fn admit_theme<R: DaemonRpc>(
    rpc: &Arc<R>,
    method: &str,
    params: Value,
) -> crate::Result<futures::future::BoxFuture<'static, crate::Result<Value>>> {
    if method == "set_lcd_theme" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Params {
            dev: DeviceId,
            theme: crate::LcdTheme,
        }
        let p: Params = from_value(params)?;
        p.theme.validate()?;
        let work = rpc.begin_lcd_theme(p.dev, p.theme).await?;
        Ok(Box::pin(async move { to_value(&work.await?) }))
    } else {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Params {
            dev: Option<DeviceId>,
            theme: crate::LcdTheme,
            #[serde(default)]
            elapsed_seconds: u64,
        }
        let p: Params = from_value(params)?;
        p.theme.validate()?;
        if method == "preview_lcd_editor" {
            let work = rpc
                .begin_lcd_editor_preview(p.dev, p.theme, p.elapsed_seconds)
                .await?;
            return Ok(Box::pin(async move { to_value(&work.await?) }));
        }
        let work = rpc
            .begin_lcd_theme_preview(p.dev, p.theme, p.elapsed_seconds)
            .await?;
        Ok(Box::pin(async move { to_value(&work.await?) }))
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
        "get_firmware_recovery" => {
            if !params.as_object().is_some_and(|p| p.is_empty()) {
                return Err(IpcError::InvalidArgument(
                    "firmware recovery expects an empty object".into(),
                ));
            }
            to_value(&rpc.get_firmware_recovery().await?)
        }
        "get_firmware" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.get_firmware(p.dev).await?;
            to_value(&v)
        }

        // ---- Sensors ----
        "get_fps_source" => {
            if !params.as_object().is_some_and(|p| p.is_empty()) {
                return Err(IpcError::InvalidArgument(
                    "FPS source expects empty object".into(),
                ));
            }
            to_value(&rpc.get_fps_source().await?)
        }
        "set_fps_source" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                settings: crate::FpsSourceSettings,
            }
            let p: Params = from_value(params)?;
            p.settings.validate()?;
            to_value(&rpc.set_fps_source(p.settings).await?)
        }
        "get_display_settings" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {}
            let _: Params = from_value(params)?;
            to_value(&rpc.get_display_settings().await?)
        }
        "set_display_settings" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                settings: crate::DisplaySettings,
            }
            let p: Params = from_value(params)?;
            p.settings.validate()?;
            to_value(&rpc.set_display_settings(p.settings).await?)
        }
        "get_system_spec" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                #[serde(default)]
                refresh: bool,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.get_system_spec(p.refresh).await?)
        }
        "list_sensors" => to_value(&rpc.list_sensors().await?),
        "get_sensor_defaults" => to_value(&rpc.get_sensor_defaults().await?),
        "set_sensor_defaults" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                config: crate::SensorDefaults,
            }
            let p: Params = from_value(params)?;
            p.config.validate()?;
            to_value(&rpc.set_sensor_defaults(p.config).await?)
        }

        "get_dynamic_rgb" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_dynamic_rgb(p.dev).await?)
        }
        "set_dynamic_rgb" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                #[serde(deserialize_with = "serde::Deserialize::deserialize")]
                config: Option<crate::DynamicRgbConfig>,
            }
            let p: Params = from_value(params)?;
            if let Some(config) = &p.config {
                config.validate()?;
            }
            to_value(&rpc.set_dynamic_rgb(p.dev, p.config).await?)
        }
        "get_thermal_state" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_thermal_state(p.dev).await?)
        }
        "set_thermal_config" => {
            #[derive(serde::Deserialize)]
            struct Params {
                dev: crate::DeviceId,
                config: crate::ThermalConfig,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.set_thermal_config(p.dev, p.config).await?)
        }
        "get_cooling_state" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_cooling_state(p.dev).await?)
        }
        "get_cooling_modes" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_cooling_modes(p.dev).await?)
        }
        "save_cooling_mode" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                target: crate::CoolingTarget,
                expected_revision: String,
                profile: crate::CoolingModeProfile,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.save_cooling_mode(p.dev, p.target, p.expected_revision, p.profile)
                    .await?,
            )
        }
        "initialize_cooling_modes" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                sensor: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.initialize_cooling_modes(p.dev, p.sensor).await?)
        }
        "apply_cooling_mode" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                target: crate::CoolingTarget,
                expected_revision: String,
                id: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.apply_cooling_mode(p.dev, p.target, p.expected_revision, p.id)
                    .await?,
            )
        }
        "set_cooling_policy" => {
            #[derive(serde::Deserialize)]
            struct Params {
                dev: crate::DeviceId,
                target: crate::CoolingTarget,
                policy: crate::CoolingPolicy,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.set_cooling_policy(p.dev, p.target, p.policy).await?)
        }
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
        "media_upload" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                request: crate::MediaUploadRequest,
            }
            let p: Params = serde_json::from_value(params)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
            to_value(&rpc.media_upload(p.request).await?)
        }
        "font_upload" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                request: crate::FontUploadRequest,
            }
            let p: Params = serde_json::from_value(params)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
            to_value(&rpc.font_upload(p.request).await?)
        }
        "list_named_theme_assets" => to_value(&rpc.list_named_theme_assets().await?),
        "delete_theme_asset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
                expected_revision: u64,
                #[serde(default)]
                keep: Vec<String>,
            }
            let p: Params = from_value(params)?;
            rpc.delete_theme_asset(p.key, p.expected_revision, p.keep)
                .await?;
            Ok(Value::Null)
        }
        "rename_theme_asset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
                name: String,
                expected_revision: u64,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.rename_theme_asset(p.key, p.name, p.expected_revision)
                    .await?,
            )
        }
        "list_theme_assets" => to_value(&rpc.list_theme_assets().await?),
        "read_theme_asset_for_edit" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.read_theme_asset_for_edit(p.key).await?)
        }
        "thumbnail_theme_asset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.thumbnail_theme_asset(p.key).await?)
        }
        "import_theme_asset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                bytes: Vec<u8>,
                #[serde(default)]
                name: Option<String>,
            }
            let p: Params = from_value(params)?;
            if p.bytes.len() > 1024 * 1024 {
                return Err(IpcError::InvalidArgument(
                    "theme upload exceeds 1 MiB IPC limit; normalize locally first".into(),
                ));
            }
            to_value(&rpc.import_named_theme_asset(p.bytes, p.name).await?)
        }
        "cleanup_theme_assets" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                #[serde(default)]
                delete: bool,
                #[serde(default)]
                keep: Vec<String>,
            }
            let p: Params = from_value(params)?;
            if p.keep.len() > 1024 {
                return Err(IpcError::InvalidArgument(
                    "too many draft asset keys".into(),
                ));
            }
            to_value(&rpc.cleanup_theme_assets(p.delete, p.keep).await?)
        }
        "cleanup_theme_fonts" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                #[serde(default)]
                delete: bool,
                #[serde(default)]
                keep: Vec<String>,
            }
            let p: Params = from_value(params)?;
            if p.keep.len() > 1024 {
                return Err(IpcError::InvalidArgument(
                    "too many draft asset keys".into(),
                ));
            }
            to_value(&rpc.cleanup_theme_fonts(p.delete, p.keep).await?)
        }
        "reapply_screen_reset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                operation_id: String,
                expected_generation: u64,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.reapply_screen_reset(p.dev, p.operation_id, p.expected_generation)
                    .await?,
            )
        }
        "get_screen_reset" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_screen_reset(p.dev).await?)
        }
        "reset_screen" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                operation_id: String,
                expected_profile_revision: u64,
                expected_collection_revision: u64,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.reset_screen(
                    p.dev,
                    p.operation_id,
                    p.expected_profile_revision,
                    p.expected_collection_revision,
                )
                .await?,
            )
        }
        "migrate_screen_collections" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.migrate_screen_collections(p.dev).await?)
        }
        "get_screen_collections" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_screen_collections(p.dev).await?)
        }
        "set_screen_collections" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                expected_revision: u64,
                collections: crate::ScreenCollections,
            }
            let p: Params = from_value(params)?;
            p.collections.validate()?;
            to_value(
                &rpc.set_screen_collections(p.dev, p.expected_revision, p.collections)
                    .await?,
            )
        }
        "preview_windows_control_import" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                draft_json: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.preview_windows_control_import(p.dev, p.draft_json)
                    .await?,
            )
        }
        "import_windows_control" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                operation_id: String,
                expected: String,
                draft_json: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.import_windows_control(p.dev, p.operation_id, p.expected, p.draft_json)
                    .await?,
            )
        }
        "preview_fan_and_lighting_reset" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                sensor_id: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.preview_fan_and_lighting_reset(p.dev, p.sensor_id)
                    .await?,
            )
        }
        "reset_fan_and_lighting" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                operation_id: String,
                expected: String,
                sensor_id: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.reset_fan_and_lighting(p.dev, p.operation_id, p.expected, p.sensor_id)
                    .await?,
            )
        }
        "reapply_group_import" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                group: crate::FullProfileGroup,
                operation_id: String,
                expected: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.reapply_group_import(p.dev, p.group, p.operation_id, p.expected)
                    .await?,
            )
        }
        "read_group_import" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                group: crate::FullProfileGroup,
                operation_id: String,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.read_group_import(p.dev, p.group, p.operation_id)
                    .await?,
            )
        }
        "preview_windows_screen_upload" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                id: String,
                source: crate::WindowsScreenPreviewSource,
            }
            let p: Params = from_value(params)?;
            to_value(
                &rpc.preview_windows_screen_upload(p.dev, p.id, p.source)
                    .await?,
            )
        }
        "group_upload" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                group: crate::FullProfileGroup,
                request: crate::GroupUploadRequest,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.group_upload(p.dev, p.group, p.request).await?)
        }
        "group_archive" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: crate::DeviceId,
                group: crate::FullProfileGroup,
                request: crate::GroupArchiveRequest,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.group_archive(p.dev, p.group, p.request).await?)
        }
        "profile_archive" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                request: crate::ProfileArchiveRequest,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.profile_archive(p.request).await?)
        }
        "list_profiles" => to_value(&rpc.list_profiles().await?),
        "edit_profile" => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                expected_revision: u64,
                edit: crate::ProfileEdit,
            }
            let p: Params = from_value(params)?;
            p.edit.validate()?;
            to_value(&rpc.edit_profile(p.expected_revision, p.edit).await?)
        }
        "get_imported_lcd_editor" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_imported_lcd_editor(p.dev).await?)
        }
        "get_lcd_theme" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_lcd_theme(p.dev).await?)
        }
        "is_lcd_available" => {
            let p: DeviceParams = from_value(params)?;
            let v = rpc.is_lcd_available(p.dev).await?;
            to_value(&v)
        }
        "get_lcd_state" => {
            let p: DeviceParams = from_value(params)?;
            serde_json::to_value(rpc.get_lcd_state(p.dev).await?)
                .map_err(|e| IpcError::Serialization(e.to_string()))
        }
        "get_lcd_playback_stats" => {
            let p: DeviceParams = from_value(params)?;
            serde_json::to_value(rpc.get_lcd_playback_stats(p.dev).await?)
                .map_err(|e| IpcError::Serialization(e.to_string()))
        }
        "patch_lcd_config" => {
            let p: PatchLcdParams = from_value(params)?;
            serde_json::to_value(rpc.patch_lcd_config(p.dev, p.patch).await?)
                .map_err(|e| IpcError::Serialization(e.to_string()))
        }
        "set_lcd_media" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                dev: DeviceId,
                key: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.set_lcd_media(p.dev, p.key).await?)
        }
        "get_lcd_media" => {
            let p: DeviceParams = from_value(params)?;
            to_value(&rpc.get_lcd_media(p.dev).await?)
        }
        "import_prepared_clip" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                name: String,
                clip: crate::H264Clip,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.import_prepared_clip(p.name, p.clip).await?)
        }
        "list_prepared_clips" => to_value(&rpc.list_prepared_clips().await?),
        "delete_prepared_clip" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
            }
            let p: Params = from_value(params)?;
            rpc.delete_prepared_clip(p.key).await?;
            Ok(Value::Null)
        }
        "load_media_first_frame" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.load_media_first_frame(p.key).await?)
        }
        "load_prepared_clip" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                key: String,
            }
            let p: Params = from_value(params)?;
            to_value(&rpc.load_prepared_clip(p.key).await?)
        }
        "play_lcd_clip" => {
            let p: PlayLcdParams = from_value(params)?;
            serde_json::to_value(rpc.play_lcd_clip(p.dev, p.clip, p.loops).await?)
                .map_err(|e| IpcError::Serialization(e.to_string()))
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

/// Success payload for trait methods returning `Result<()>`.
/// Keep the established empty-object wire contract for mutators. Optional
/// getters may return JSON null; the response decoder preserves its presence.
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

#[derive(Deserialize)]
struct PatchLcdParams {
    dev: DeviceId,
    patch: crate::LcdConfigPatch,
}
#[derive(Deserialize)]
struct PlayLcdParams {
    dev: DeviceId,
    clip: crate::H264Clip,
    loops: u16,
}
