//! Real CLI -> Unix IPC -> read-only durable recovery, private store/MockBackend.
use lconnect_daemon::{
    actor::firmware_update::CoolingHold,
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, FirmwareRecoveryState, FirmwareRecoveryStatus,
};
use serde_json::json;
use std::{process::Command, sync::Arc, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovery_cli_and_strict_rpc_work_without_device_after_admission_closes() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    let store = Arc::new(PersistenceStore::new_with_root(state.clone()));
    let target = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("CLI-RECOVERY".into()),
        "/mock/previous".into(),
    );
    let saved = DeviceSettings::default();
    let key = settings_key(&target);
    store.write(&key, &saved).unwrap();
    let mut operation = store
        .firmware_operations()
        .arm(&target, CoolingHold::PumpOnly, saved)
        .unwrap();
    operation.effects_possible().unwrap();
    drop(operation);
    store.admission().close().wait_checked().await.unwrap();
    let config = Config::default();
    let stats = Arc::new(Stats::new());
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = Arc::new(IpcServerImpl::new(
        manager.clone(),
        stats,
        config,
        store.clone(),
    ));
    let socket = root.path().join("lconnectd.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept()=>{let (stream,_)=accepted.unwrap();let rpc=rpc.clone();jobs.spawn(async move{IpcServer::new(rpc).serve_stream(stream).await});},
                _=jobs.join_next(), if !jobs.is_empty()=>{}
            }
        }
    });
    let journal = state.join("firmware-operation.json");
    let settings = state.join(format!("devices/{key}.toml"));
    let before = (
        std::fs::read(&journal).unwrap(),
        std::fs::read(&settings).unwrap(),
    );
    let client = IpcClient::connect(&socket).await.unwrap();
    assert!(client.call("list_devices", json!({})).await.is_err());
    let status: FirmwareRecoveryStatus = serde_json::from_value(
        client
            .call("get_firmware_recovery", json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status.state, FirmwareRecoveryState::Guarded);
    for bad in [
        json!({"dev":"fake"}),
        json!({"root":"/tmp"}),
        json!({"clear":true}),
        json!([]),
    ] {
        assert!(client.call("get_firmware_recovery", bad).await.is_err());
    }
    let run = |args: Vec<&'static str>| {
        let runtime = root.path().to_path_buf();
        tokio::task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
                .args(args)
                .env("XDG_RUNTIME_DIR", runtime)
                .env_remove("LCONNECTCTL_SOCKET")
                .output()
                .unwrap()
        })
    };
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        run(vec!["--json", "firmware-recovery"]),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<FirmwareRecoveryStatus>(&output.stdout).unwrap(),
        status
    );
    let output = run(vec!["firmware-recovery"]).await.unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Saved firmware state: guarded"));
    assert!(text.contains("fresh target verification"));
    assert!(text.contains("not live job status"));
    assert_eq!(
        (
            std::fs::read(&journal).unwrap(),
            std::fs::read(&settings).unwrap()
        ),
        before
    );
    // Restart-style journal loss is still visible globally, without target lookup.
    std::fs::remove_file(&journal).unwrap();
    let orphan: FirmwareRecoveryStatus = serde_json::from_value(
        client
            .call("get_firmware_recovery", json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(orphan.state, FirmwareRecoveryState::GuardOnly);
    assert_eq!(orphan.guards, status.guards);
    assert_eq!(std::fs::read(settings).unwrap(), before.1);
    assert_eq!(store.admission().active(), 0);
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
