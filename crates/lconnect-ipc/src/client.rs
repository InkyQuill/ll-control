//! JSON-RPC 2.0 client over `UnixStream`.
//!
//! [`IpcClient::connect`] opens the socket, reads the `hello`
//! notification the server pushes immediately on accept, and exposes a
//! [`call`](IpcClient::call) async method for one-shot RPCs. For Phase 1
//! the client also exposes [`subscribe`](IpcClient::subscribe), which
//! returns an mpsc receiver of incoming notifications matching a method
//! name (e.g., `"snapshot"`, `"device_attached"`).
//!
//! ## Architecture
//!
//! A single `UnixStream` is split into a read half and a write half. Two
//! background tasks manage them:
//!
//! - **Reader task** parses each newline-framed JSON object. If it's a
//!   [`Response`], the response is dispatched (by id) to a pending
//!   oneshot. If it's a [`Notification`], a clone is forwarded to every
//!   subscriber registered for that method name.
//! - **Writer / dispatch task** serialises outbound work: it inserts a
//!   pending oneshot before writing the request line (so a fast server
//!   can never race the reader), and it registers new subscriptions.
//!
//! Both tasks exit when their channels close, which happens automatically
//! when the [`IpcClient`] is dropped.
//!
//! Daemon-side errors come back as JSON-RPC error objects with codes
//! mirroring [`crate::IpcError::jsonrpc_code`]; the client reverses
//! that mapping internally so callers see the same `IpcError` variants
//! regardless of which side produced them.

use crate::jsonrpc::{Notification, Request, Response};
use crate::{IpcError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, Mutex};

/// JSON-RPC client connected to the daemon over a Unix socket.
pub struct IpcClient {
    next_id: AtomicU64,
    reader_task: tokio::task::JoinHandle<()>,
    writer_task: tokio::task::JoinHandle<()>,
    tx: mpsc::Sender<ClientMsg>,
    /// The `hello` notification the server pushed on connect (carries
    /// version + capability info). Exposed so callers can sanity-check
    /// the wire protocol version before issuing calls.
    pub hello: Notification,
}

/// Messages sent from `call`/`subscribe` into the writer task.
enum ClientMsg {
    Call {
        id: u64,
        line: String,
        reply: oneshot::Sender<Response>,
    },
    Subscribe {
        method: String,
        sink: mpsc::Sender<Notification>,
    },
    ReaderClosed,
}

impl IpcClient {
    /// Connect to the daemon at `socket_path`, read the server `hello`,
    /// and spawn the reader/writer tasks.
    ///
    /// # Errors
    /// Fails if the socket can't be opened or the `hello` line can't be
    /// read or parsed as a [`Notification`].
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| IpcError::Transport(e.to_string()))?;
        Self::from_stream(stream).await
    }

    /// Build a client from any `AsyncRead + AsyncWrite` stream.
    ///
    /// This is the test-only seam used by the in-memory transport
    /// ([`crate::transport_memory::mem_pair`]) and by the Phase 2 GUI's
    /// mock-daemon harness. The body is the same multiplexer wiring
    /// that [`Self::connect`] uses; only the transport changes.
    ///
    /// # Errors
    /// Fails if the `hello` line can't be read or parsed as a
    /// [`Notification`] — the stream itself is assumed already open.
    pub async fn from_stream<S>(stream: S) -> Result<Self>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (read_half, mut write_half) = tokio::io::split(stream);
        let mut reader = BufReader::new(read_half);

        // Read the hello line synchronously before spawning the reader
        // task — the caller gets back a fully usable client.
        let mut hello_line = String::new();
        let n = crate::framing::read_bounded_line(
            &mut reader,
            &mut hello_line,
            crate::framing::MAX_JSON_RPC_LINE_BYTES,
        )
        .await
        .map_err(|e| IpcError::Transport(e.to_string()))?;
        if n == 0 {
            return Err(IpcError::Transport("connection closed before hello".into()));
        }
        let hello: Notification = serde_json::from_str(hello_line.trim_end())
            .map_err(|e| IpcError::Serialization(e.to_string()))?;

        // Shared dispatch tables.
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let subs: Arc<Mutex<HashMap<String, Vec<mpsc::Sender<Notification>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, mut rx) = mpsc::channel::<ClientMsg>(64);

        // Reader task: distribute responses (by id) and notifications
        // (by method) to their respective consumers.
        let pending_r = pending.clone();
        let subs_r = subs.clone();
        let tx_r = tx.clone();
        let reader_task = tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match crate::framing::read_bounded_line(
                    &mut reader,
                    &mut line,
                    crate::framing::MAX_JSON_RPC_LINE_BYTES,
                )
                .await
                {
                    Ok(0) => break, // EOF
                    Ok(_) => {}
                    Err(e) => {
                        tracing::debug!("ipc client read error: {e}");
                        break;
                    }
                }
                let trimmed = line.trim_end();
                if trimmed.is_empty() {
                    continue;
                }

                // Discriminate by the presence of the `method` key:
                // notifications carry one and responses do not. This is
                // structural rather than try-Response-then-Notification,
                // which avoided ambiguity for envelopes that happen to
                // round-trip cleanly into both shapes (e.g. a response
                // with neither `result` nor `error` set, which the
                // try-Response path would have accepted as a malformed
                // response and silently dropped).
                let raw: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!("ipc client parse failed: {e}");
                        continue;
                    }
                };
                if raw.get("method").is_some() {
                    // Notification path.
                    match serde_json::from_value::<Notification>(raw) {
                        Ok(notif) => {
                            let sinks: Vec<mpsc::Sender<Notification>> = subs_r
                                .lock()
                                .await
                                .get(&notif.method)
                                .cloned()
                                .unwrap_or_default();
                            for s in sinks {
                                let _ = s.send(notif.clone()).await;
                            }
                        }
                        Err(e) => {
                            tracing::debug!("ipc client notification decode failed: {e}");
                        }
                    }
                } else {
                    // Keep the id so malformed envelopes fail their caller promptly.
                    let response_id = raw.get("id").and_then(Value::as_u64);
                    match serde_json::from_value::<Response>(raw) {
                        Ok(resp) => {
                            if let Some(id_val) = resp.id.as_ref().and_then(Value::as_u64) {
                                if let Some(tx) = pending_r.lock().await.remove(&id_val) {
                                    let _ = tx.send(resp);
                                }
                            }
                            // Response with no parseable id: drop.
                        }
                        Err(e) => {
                            tracing::debug!("ipc client response decode failed: {e}");
                            if let Some(id) = response_id {
                                pending_r.lock().await.remove(&id);
                            }
                        }
                    }
                }
            }
            // Tell the writer to stop accepting work and drop every pending
            // reply sender. This also covers calls queued concurrently with
            // the reader failure.
            let _ = tx_r.send(ClientMsg::ReaderClosed).await;
        });

        // Writer / dispatch task.
        let pending_w = pending.clone();
        let subs_w = subs.clone();
        let writer_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ClientMsg::Call { id, line, reply } => {
                        // Insert before writing so the reader can never
                        // observe a response for an unknown id.
                        pending_w.lock().await.insert(id, reply);
                        if let Err(e) = write_half.write_all(line.as_bytes()).await {
                            tracing::error!("ipc client write failed: {e}");
                            pending_w.lock().await.clear();
                            break;
                        }
                    }
                    ClientMsg::Subscribe { method, sink } => {
                        subs_w.lock().await.entry(method).or_default().push(sink);
                    }
                    ClientMsg::ReaderClosed => {
                        pending_w.lock().await.clear();
                        break;
                    }
                }
            }
        });

        Ok(IpcClient {
            next_id: AtomicU64::new(1),
            reader_task,
            writer_task,
            tx,
            hello,
        })
    }

    /// One-shot RPC call. Awaits the matching response.
    ///
    /// # Errors
    /// Returns the daemon's [`IpcError`] (mapped from JSON-RPC error
    /// codes) on failure, or [`IpcError::Transport`] /
    /// [`IpcError::Serialization`] for connection-level problems.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request::new(id, method, params);
        let mut line =
            serde_json::to_string(&req).map_err(|e| IpcError::Serialization(e.to_string()))?;
        line.push('\n');

        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ClientMsg::Call {
                id,
                line,
                reply: reply_tx,
            })
            .await
            .map_err(|_| IpcError::Transport("client multiplexer closed".into()))?;

        let resp = reply_rx
            .await
            .map_err(|_| IpcError::Transport("response dropped".into()))?;

        match (resp.result, resp.error) {
            (Some(v), None) => Ok(v),
            (None, Some(e)) => Err(map_jsonrpc_error(e.code, e.message)),
            (Some(_), Some(_)) => Err(IpcError::Transport(
                "invalid response: both result and error set".into(),
            )),
            (None, None) => Err(IpcError::Transport(
                "invalid response: neither result nor error set".into(),
            )),
        }
    }

    /// Subscribe to a notification method (e.g., `"snapshot"`,
    /// `"device_attached"`).
    ///
    /// The returned receiver yields notifications as long as the
    /// connection is alive. Multiple subscriptions to the same method
    /// are independent — each gets its own clone of every notification.
    pub async fn subscribe(&self, method: &str) -> mpsc::Receiver<Notification> {
        let (tx, rx) = mpsc::channel(32);
        let _ = self
            .tx
            .send(ClientMsg::Subscribe {
                method: method.to_string(),
                sink: tx,
            })
            .await;
        rx
    }
}

impl Drop for IpcClient {
    fn drop(&mut self) {
        // Reader retains a sender to notify writer on EOF, so dropping tx alone
        // cannot close the connection. Abort both owners, including a reader
        // blocked on a full notification queue. Accepted server work is separate.
        self.reader_task.abort();
        self.writer_task.abort();
    }
}

/// Map a JSON-RPC error code + message back to an [`IpcError`] variant.
///
/// Inverse of [`crate::IpcError::jsonrpc_code`]. Unknown codes degrade
/// to [`IpcError::Transport`] so callers always get a typed error.
fn map_jsonrpc_error(code: i32, message: String) -> IpcError {
    match code {
        -32000 => IpcError::DeviceNotFound(message),
        -32001 => IpcError::DeviceBusy(message),
        -32002 => IpcError::InvalidArgument(message),
        -32003 => IpcError::NotImplemented(message),
        -32004 => IpcError::ConfirmationRequired,
        -32005 => IpcError::ProtocolMismatch(message),
        _ => IpcError::Transport(format!("code={code}: {message}")),
    }
}
