//! In-memory client/server pair for hardware-free tests.
//!
//! [`mem_pair`] returns `(client_end, server_end)`. Each end is
//! `AsyncRead + AsyncWrite` (a [`tokio::io::DuplexStream`]). The daemon's
//! actor-tree tests — and the Phase 2 GUI's mock daemon harness — can
//! stand up a full [`crate::server::IpcServer`] +
//! [`crate::client::IpcClient`] over these without touching the
//! filesystem.
//!
//! The buffer size (64 KiB) is sized so a typical single RPC fits in one
//! buffer cycle without backpressure; larger payloads (e.g. LCD JPEGs)
//! still work, they just round-trip through the duplex pipe in chunks.

use tokio::io::DuplexStream;

/// Returns `(client_end, server_end)`. Hand `client_end` to
/// [`crate::client::IpcClient::from_stream`] and `server_end` to
/// [`crate::server::IpcServer::serve_stream`].
#[must_use]
pub fn mem_pair() -> (DuplexStream, DuplexStream) {
    tokio::io::duplex(64 * 1024)
}
