//! Actual CLI and strict Unix RPC for global FPS settings, zero devices/private files.
use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, sensors::SensorCache, stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{client::IpcClient, server::IpcServer, FpsSourceSettings};
use serde_json::json;
use std::{process::Command, sync::Arc, time::Duration};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_configures_global_fps_without_device_and_stale_or_bad_intent_never_overwrites() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    let store = Arc::new(PersistenceStore::new_with_root(state.clone()));
    let cache = SensorCache::default();
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
    let rpc = Arc::new(
        IpcServerImpl::new(manager.clone(), stats, config, store.clone())
            .with_sensors(cache.clone()),
    );
    let socket = root.path().join("lconnectd.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let server = tokio::spawn(async move {
        let mut jobs = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
             accepted=listener.accept()=>{let (stream,_)=accepted.unwrap();let rpc=rpc.clone();jobs.spawn(async move{IpcServer::new(rpc).serve_stream(stream).await});},
             _=jobs.join_next(),if !jobs.is_empty()=>{}
            }
        }
    });
    let client = IpcClient::connect(&socket).await.unwrap();
    assert_eq!(
        client.call("list_devices", json!({})).await.unwrap(),
        json!([])
    );
    let run = |args: Vec<String>| {
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
    let output = run(vec!["--json".into(), "fps".into(), "show".into()])
        .await
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<FpsSourceSettings>(&output.stdout).unwrap(),
        FpsSourceSettings::default()
    );
    let path = root.path().join("game.csv").to_str().unwrap().to_owned();
    let args = vec![
        "--json".into(),
        "fps".into(),
        "set".into(),
        "--revision".into(),
        "0".into(),
        "--enabled".into(),
        "true".into(),
        "--log-path".into(),
        path.clone(),
    ];
    let output = tokio::time::timeout(Duration::from_secs(10), run(args.clone()))
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let saved: FpsSourceSettings = serde_json::from_slice(&output.stdout).unwrap();
    assert!(saved.enabled);
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.log_path.as_ref(), Some(&path));
    assert_eq!(
        store.fps_source().subscribe().borrow().as_ref(),
        Some(&saved)
    );
    let before = std::fs::read(state.join("fps-source.json")).unwrap();
    assert!(!run(args).await.unwrap().status.success());
    assert_eq!(
        std::fs::read(state.join("fps-source.json")).unwrap(),
        before
    );
    for bad in [json!({"root":"/tmp"}), json!([])] {
        assert!(client.call("get_fps_source", bad).await.is_err());
    }
    for settings in [
        json!({"version":1,"revision":1,"enabled":true,"log_path":"relative.csv"}),
        json!({"version":2,"revision":1,"enabled":false,"log_path":null}),
        json!({"version":1,"revision":1,"enabled":false,"log_path":null,"clear":true}),
    ] {
        assert!(client
            .call("set_fps_source", json!({"settings":settings}))
            .await
            .is_err());
    }
    assert_eq!(
        std::fs::read(state.join("fps-source.json")).unwrap(),
        before
    );
    let output = run(vec![
        "fps".into(),
        "set".into(),
        "--revision".into(),
        "1".into(),
        "--enabled".into(),
        "false".into(),
        "--log-path".into(),
        path,
    ])
    .await
    .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("disabled"));
    let restarted = PersistenceStore::new_with_root(state);
    assert!(
        !restarted
            .fps_source()
            .get(SensorCache::default())
            .await
            .unwrap()
            .enabled
    );
    store.admission().close().wait_checked().await.unwrap();
    assert!(client.call("get_fps_source", json!({})).await.is_err());
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
