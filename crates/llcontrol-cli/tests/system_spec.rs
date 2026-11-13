use llcontrol_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats, system_spec::LinuxSpec,
};
use llcontrol_hid::MockBackend;
use llcontrol_ipc::{client::IpcClient, server::IpcServer, SystemSpec};
use serde_json::json;
use std::{process::Command, sync::Arc, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_unix_inventory_cli_and_rpc_work_without_a_pump_and_reject_external_paths() {
    let root = tempfile::tempdir().unwrap();
    let proc_root = root.path().join("proc");
    let sys_root = root.path().join("sys");
    std::fs::create_dir_all(&proc_root).unwrap();
    std::fs::create_dir_all(sys_root.join("class/dmi/id")).unwrap();
    std::fs::write(
        proc_root.join("cpuinfo"),
        "processor : 0\nmodel name : Fixture CPU\n\n",
    )
    .unwrap();
    std::fs::write(sys_root.join("class/dmi/id/board_name"), "Fixture Board").unwrap();
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
    let rpc = Arc::new(
        IpcServerImpl::new(manager.clone(), stats, cfg, store).with_system_spec(LinuxSpec {
            proc_root,
            sys_root: sys_root.clone(),
            pci_ids: vec![],
        }),
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
    let before: SystemSpec =
        serde_json::from_value(client.call("get_system_spec", json!({})).await.unwrap()).unwrap();
    before.validate().unwrap();
    assert_eq!(before.groups[4].items[0].title, "Fixture Board");
    for bad in [
        json!({"refresh":"yes"}),
        json!({"sys_root":"/sys"}),
        json!({"dev":"fake"}),
    ] {
        assert!(client.call("get_system_spec", bad).await.is_err());
    }
    std::fs::write(sys_root.join("class/dmi/id/board_name"), "Updated Board").unwrap();
    let run = |args: Vec<&'static str>| {
        let runtime = root.path().to_path_buf();
        tokio::task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_llcontrol"))
                .args(args)
                .env("XDG_RUNTIME_DIR", runtime)
                .env_remove("LLCONTROL_SOCKET")
                .output()
                .unwrap()
        })
    };
    let out = tokio::time::timeout(Duration::from_secs(15), run(vec!["--json", "spec"]))
        .await
        .unwrap()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<SystemSpec>(&out.stdout).unwrap(),
        before
    );
    let out = run(vec!["spec", "--refresh"]).await.unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let human = String::from_utf8(out.stdout).unwrap();
    assert!(human.contains("Updated Board"));
    assert!(human.contains("Unavailable:"));
    assert!(human.contains("sysfs/dmi"));
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
