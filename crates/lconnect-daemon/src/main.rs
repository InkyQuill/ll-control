//! lconnectd - the LL-Control daemon.
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
use std::sync::Arc;

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next()) {
        (Some(flag), None) if flag == "--query-intel-memory" => {
            // Dispatch before runtime, config, persistence or HID initialization.
            let rows = lconnect_intel_memory::collect()
                .map_err(|e| anyhow::anyhow!("Intel memory query: {e:?}"))?;
            serde_json::to_writer(std::io::stdout().lock(), &rows)?;
            Ok(())
        }
        (Some(flag), None) if flag == "--query-intel-engines" => {
            let rows = lconnect_intel_memory::collect_engines()
                .map_err(|e| anyhow::anyhow!("Intel engine query: {e:?}"))?;
            serde_json::to_writer(std::io::stdout().lock(), &rows)?;
            Ok(())
        }
        (None, None) => run_daemon(),
        _ => anyhow::bail!("unsupported daemon arguments"),
    }
}

#[tokio::main]
async fn run_daemon() -> Result<()> {
    init_tracing();

    let config = Config::load().context("failed to load daemon configuration")?;
    tracing::info!("lconnectd starting; log_level={}", config.daemon.log_level);

    let socket_path =
        lconnect_ipc::socket::default_socket_path().context("failed to resolve IPC socket path")?;
    let query_helper = std::env::current_exe()?;
    // The loser returns here, before persistence, HID, actor spawn, or replay.
    let ownership = tokio::task::spawn_blocking(move || {
        lconnect_ipc::socket::DaemonOwnership::acquire(&socket_path)
    })
    .await
    .context("daemon ownership task failed")?
    .context(
        "refusing to start a second lconnectd; exited before persistence, HID, or settings replay",
    )?;

    run_owned(config, query_helper, ownership).await
}

async fn run_owned(
    config: Config,
    query_helper: std::path::PathBuf,
    ownership: lconnect_ipc::socket::DaemonOwnership,
) -> Result<()> {
    let socket_path = ownership.path().to_path_buf();
    tracing::info!(
        "exclusive daemon ownership acquired for {}",
        socket_path.display()
    );
    let (listener, guard) = ownership
        .into_listener()
        .context("failed to prepare the owned IPC listener")?;

    let registry = FamilyRegistry::with_default_lcdv2();
    let backend: Arc<dyn HidBackend> =
        Arc::new(HidApiBackend::new(registry).context("HID backend init failed")?);
    let persistence =
        Arc::new(PersistenceStore::new()?.with_legacy_fps_log(
            std::env::var_os("LCONNECT_MANGOHUD_PER_FRAME_LOG").map(Into::into),
        ));
    let stats = Arc::new(Stats::new());

    let sensors = lconnect_daemon::sensors::SensorCache::default();
    if let Err(error) = persistence.fps_source().get(sensors.clone()).await {
        tracing::warn!(%error, "FPS settings unavailable; FPS disabled until qualified settings load");
    }
    let manager = DeviceManager::new(
        Arc::clone(&backend),
        Arc::clone(&persistence),
        config.clone(),
        Arc::clone(&stats),
    );
    let manager_handle = manager.with_sensors(sensors.clone()).spawn().await;

    let (sensor_shutdown, sensor_stop) = tokio::sync::watch::channel(false);
    let sensor_task = lconnect_daemon::sensors::spawn(
        lconnect_daemon::sensors::LinuxSensors::default()
            .with_intel_query_helper(query_helper)
            .with_fps_settings(persistence.fps_source().subscribe()),
        sensors.clone(),
        sensor_stop,
    );

    let (power_shutdown, power_stop) = tokio::sync::watch::channel(false);
    let power_task = lconnect_daemon::power::spawn(manager_handle.clone(), power_stop);

    let rpc = Arc::new(
        IpcServerImpl::new(
            manager_handle.clone(),
            Arc::clone(&stats),
            config.clone(),
            Arc::clone(&persistence),
        )
        .with_sensors(sensors),
    );

    tracing::info!("IPC socket path: {}", socket_path.display());

    let server = IpcServer::new(rpc.clone());

    let serve_result = tokio::select! {
        r = server.serve_listener(listener) => {
            tracing::warn!("IPC server exited: {:?}", r);
            r
        }
        _ = wait_for_shutdown() => {
            tracing::info!("shutdown signal received");
            Ok(())
        }
    };

    let _ = power_shutdown.send(true);
    let _ = power_task.await;
    let _ = sensor_shutdown.send(true);
    let _ = sensor_task.await;
    if let Err(error) = rpc.shutdown_firmware().await {
        tracing::error!(%error, "firmware shutdown outcome unconfirmed");
    }
    manager_handle.shutdown().await;
    drop(guard);

    serve_result.map_err(Into::into)
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("lconnectd=info,lconnect_daemon=info,lconnect_ipc=info")
    });
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
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
