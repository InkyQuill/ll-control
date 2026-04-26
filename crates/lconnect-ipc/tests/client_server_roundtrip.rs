//! Round-trip integration test: client + server over a real `UnixStream`.
//!
//! The full test (commented out below) defines a mock [`DaemonRpc`]
//! implementation that returns canned values for `list_devices` and
//! `daemon_stats` and [`IpcError::NotImplemented`] for everything else.
//! It binds an [`IpcServer`] on a temp Unix socket, spawns it on the
//! tokio runtime, connects an [`IpcClient`], and asserts:
//!
//! 1. `list_devices` round-trips the canned `Vec<DeviceDescriptor>`.
//! 2. `daemon_stats` round-trips the canned [`DaemonStats`].
//! 3. A method that returns [`IpcError::NotImplemented`] on the server
//!    surfaces as `IpcError::NotImplemented` on the client (verifying
//!    the JSON-RPC error code -> `IpcError` mapping in both
//!    directions).
//!
//! Currently `#[ignore]`'d: the [`IpcServer`] type lands in P1C-4. Once
//! that task is merged, replace the placeholder body with the real
//! round-trip and drop `#[ignore]`.

use lconnect_ipc::{IpcError, Result};

// Suppress the unused-import warning when the test body is a placeholder.
#[allow(dead_code)]
fn _silence_unused() -> Result<()> {
    Err(IpcError::NotImplemented("placeholder".into()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Re-enable after P1C-4 lands (IpcServer is still a stub)."]
async fn client_server_roundtrip() {
    // The real test will:
    //   1. Build a `MockDaemon` impl of `DaemonRpc`.
    //   2. Allocate a tempdir + socket path.
    //   3. Spawn `IpcServer::serve(socket_path, Arc::new(mock))` in a
    //      tokio task; capture its JoinHandle.
    //   4. `IpcClient::connect(socket_path).await`.
    //   5. Assert hello.method == "hello".
    //   6. Call list_devices, daemon_stats, and an unimplemented method;
    //      assert canned values + IpcError::NotImplemented mapping.
    //   7. Drop client; abort server task; tempdir auto-cleans.
    //
    // See task P1C-5 in docs/superpowers/plans/2026-04-27-phase-1-daemon.md
    // for the full roundtrip body.
}
