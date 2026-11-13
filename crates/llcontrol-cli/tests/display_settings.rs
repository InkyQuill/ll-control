use llcontrol_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use llcontrol_hid::MockBackend;
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, DisplaySettings, DisplayTemperatureUnit,
};
use serde_json::json;
use std::{process::Command, sync::Arc};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_units_real_unix_cli_cas_invalid_requests_and_startup_restore_without_device() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().join("state")));
    let stats = Arc::new(Stats::new());
    let cfg = Config::default();
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let cache = llcontrol_daemon::sensors::SensorCache::default();
    cache.publish(
        std::time::Instant::now(),
        vec![llcontrol_ipc::SensorReading {
            id: "cpu".into(),
            label: "CPU".into(),
            unit: llcontrol_ipc::SensorUnit::Celsius,
            value: Some(40.0),
            status: llcontrol_ipc::SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: None,
        }],
    );
    let rpc = Arc::new(
        IpcServerImpl::new(manager.clone(), stats, cfg, store.clone()).with_sensors(cache.clone()),
    );
    let socket = root.path().join("llcontrold.sock");
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
    let client = IpcClient::connect(&socket).await.unwrap();
    assert_eq!(
        client.call("list_devices", json!({})).await.unwrap(),
        json!([])
    );
    let initial: DisplaySettings = serde_json::from_value(
        client
            .call("get_display_settings", json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(initial, DisplaySettings::default());
    for params in [
        json!({"settings":{"version":1,"revision":0,"temperature_unit":"kelvin"}}),
        json!({"settings":{"version":1,"revision":0,"temperature_unit":"fahrenheit","extra":true}}),
        json!({"settings":{"version":2,"revision":0,"temperature_unit":"fahrenheit"}}),
    ] {
        assert!(client.call("set_display_settings", params).await.is_err());
    }
    assert!(client
        .call("get_display_settings", json!({"dev":"other"}))
        .await
        .is_err());
    let runtime = root.path().to_path_buf();
    let out = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .args([
                "--json",
                "display-settings",
                "set",
                "--temperature-unit",
                "fahrenheit",
            ])
            .env("XDG_RUNTIME_DIR", runtime)
            .env_remove("LLCONTROL_SOCKET")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let saved: DisplaySettings = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.temperature_unit, DisplayTemperatureUnit::Fahrenheit);
    assert!(client
        .call("set_display_settings", json!({"settings":initial}))
        .await
        .is_err());
    assert_eq!(
        client
            .call("get_display_settings", json!({}))
            .await
            .unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    let theme = json!({"version":1,"name":"Global unit","background":[0,0,0],"layers":[{"kind":"sensor","sensor_id":"cpu","label":"CPU","unit":"celsius","temperature_unit":"system","decimals":1,"style":{"x":240,"y":240,"size":40,"color":[255,255,255],"align":"center"}}]});
    let system_preview = client
        .call("preview_lcd_theme", json!({"dev":null,"theme":theme}))
        .await
        .unwrap();
    let mut fixed = theme.clone();
    fixed["layers"][0]["temperature_unit"] = json!("fahrenheit");
    assert_eq!(
        system_preview,
        client
            .call("preview_lcd_theme", json!({"dev":null,"theme":fixed}))
            .await
            .unwrap()
    );
    fixed["layers"][0]["temperature_unit"] = json!("celsius");
    assert_ne!(
        system_preview,
        client
            .call("preview_lcd_theme", json!({"dev":null,"theme":fixed}))
            .await
            .unwrap()
    );
    assert_eq!(cache.readings()[0].unit, llcontrol_ipc::SensorUnit::Celsius);
    assert_eq!(cache.readings()[0].value, Some(40.0));
    let runtime = root.path().to_path_buf();
    let out = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_llcontrol"))
            .args(["--json", "display-settings", "set", "--language", "english"])
            .env("XDG_RUNTIME_DIR", runtime)
            .env_remove("LLCONTROL_SOCKET")
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let saved: DisplaySettings = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        saved.interface_language,
        llcontrol_ipc::InterfaceLanguage::English
    );
    assert_eq!(saved.temperature_unit, DisplayTemperatureUnit::Fahrenheit);
    assert_eq!(saved.revision, 2);
    let bad = json!({"settings":{"version":1,"revision":2,"temperature_unit":"celsius","interface_language":"../de-DE"}});
    assert!(client.call("set_display_settings", bad).await.is_err());
    let file = root.path().join("state/display-settings.json");
    assert_eq!(
        serde_json::from_slice::<DisplaySettings>(&std::fs::read(&file).unwrap()).unwrap(),
        saved
    );
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    let fresh = Arc::new(PersistenceStore::new_with_root(root.path().join("state")));
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        fresh.clone(),
        Config::default(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    assert!(fresh
        .display_settings()
        .live_unit()
        .load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(fresh.display_settings().get().await.unwrap(), saved);
    manager.shutdown().await;
}
