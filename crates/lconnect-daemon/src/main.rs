//! lconnectd - the L-Connect Linux daemon.
//!
//! Wires the long-lived process: load config, build the HID backend,
//! spawn the device manager, and serve the JSON-RPC IPC over a Unix
//! socket until SIGTERM/SIGINT.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::{FamilyRegistry, HidApiBackend, HidBackend};
use lconnect_ipc::server::IpcServer;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = Config::load().unwrap_or_else(|e| {
        tracing::warn!("config load failed ({e}); using defaults");
        Config::default()
    });
    tracing::info!("lconnectd starting; log_level={}", config.daemon.log_level);

    let registry = FamilyRegistry::with_default_lcdv2();
    let backend: Arc<dyn HidBackend> =
        Arc::new(HidApiBackend::new(registry).context("HID backend init failed")?);
    let persistence = Arc::new(PersistenceStore::new()?);
    let stats = Arc::new(Stats::new());

    let manager = DeviceManager::new(
        Arc::clone(&backend),
        Arc::clone(&persistence),
        config.clone(),
    );
    let manager_handle = manager.spawn().await;

    let rpc = Arc::new(IpcServerImpl::new(
        manager_handle.clone(),
        Arc::clone(&stats),
        config.clone(),
        Arc::clone(&persistence),
    ));

    let socket_path = runtime_socket_path()?;
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create socket parent dir {}", parent.display()))?;
    }
    tracing::info!("IPC socket path: {}", socket_path.display());

    let server = IpcServer::new(rpc);

    tokio::select! {
        r = server.serve(&socket_path) => {
            tracing::warn!("IPC server exited: {:?}", r);
            r?;
        }
        _ = wait_for_shutdown() => {
            tracing::info!("shutdown signal received");
            // Graceful shutdown: actor trees teardown when the manager's
            // ManagedDevice entries drop on process exit. A future
            // improvement is to wire a global shutdown watch and wait
            // for the actors to flush.
        }
    }

    // Clean up the socket file so the next daemon start can bind cleanly.
    let _ = std::fs::remove_file(&socket_path);

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("lconnectd=info,lconnect_daemon=info,lconnect_ipc=info")
    });
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Resolve the IPC socket path.
///
/// Prefers `$XDG_RUNTIME_DIR/lconnectd.sock` (the standard well-known
/// location for per-user runtime sockets). Falls back to
/// `xdg::BaseDirectories::get_runtime_directory` when the env var is
/// unset; that lookup itself errors when the platform has no runtime
/// directory at all, which is the right failure mode for a daemon that
/// needs a Unix socket.
fn runtime_socket_path() -> Result<PathBuf> {
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(rt).join("lconnectd.sock"));
    }
    let dirs = xdg::BaseDirectories::with_prefix("lconnectd")
        .context("xdg base-directory lookup failed")?;
    let runtime = dirs
        .get_runtime_directory()
        .context("no runtime directory available (XDG_RUNTIME_DIR unset)")?;
    Ok(runtime.join("lconnectd.sock"))
}

async fn wait_for_shutdown() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut intr = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = term.recv() => (),
        _ = intr.recv() => (),
    }
}
