use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{server::IpcServer, CoolingPolicy, NamedProfile, ProfileContent, ProfileEdit};
use std::{process::Command, sync::Arc, time::Duration};
fn command(root: &std::path::Path, args: Vec<String>) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(args)
        .env("XDG_RUNTIME_DIR", root)
        .env_remove("LCONNECTCTL_SOCKET")
        .output()
        .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_cli_export_import_and_durable_finish_readback_keep_other_slots_and_do_not_apply() {
    let dir = tempfile::tempdir().unwrap();
    let persistence = Arc::new(PersistenceStore::new_with_root(dir.path().join("settings")));
    let config = Config::default();
    let stats = Arc::new(Stats::new());
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        persistence.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), config, persistence.clone());
    let path = dir.path().join("lconnectd.sock");
    let server_path = path.clone();
    let server =
        tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&server_path).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let saved = NamedProfile {
        slot: 0,
        name: "Portable fan".into(),
        content: ProfileContent::Fan(CoolingPolicy::Fixed { percent: 35 }),
    };
    persistence
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: saved.clone(),
            },
        )
        .unwrap();
    let file = dir.path().join("fan.lcp");
    let out = command(
        dir.path(),
        vec![
            "profiles".into(),
            "export".into(),
            "fan".into(),
            "0".into(),
            file.display().to_string(),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(file.exists());
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.starts_with(b"LCPROF01"));
    let out = command(
        dir.path(),
        vec![
            "profiles".into(),
            "export".into(),
            "fan".into(),
            "0".into(),
            file.display().to_string(),
        ],
    );
    assert!(!out.status.success());
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let out = command(
        dir.path(),
        vec![
            "--json".into(),
            "profiles".into(),
            "import".into(),
            "fan".into(),
            "3".into(),
            file.display().to_string(),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let reply: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let id = reply["state"]["id"].as_str().unwrap();
    assert_eq!(reply["state"]["phase"], "imported");
    assert_eq!(reply["state"]["committed_revision"], 2);
    let library = persistence.profiles().list().unwrap();
    assert_eq!(library.profiles.len(), 2);
    assert_eq!(library.profiles[0], saved);
    assert_eq!(library.profiles[1].slot, 3);
    assert_eq!(library.profiles[1].content, library.profiles[0].content);
    // The CLI releases the in-memory session after success; readback uses the durable receipt.
    for action in ["archive-state", "archive-finish", "archive-cancel"] {
        let out = command(
            dir.path(),
            vec!["--json".into(), "profiles".into(), action.into(), id.into()],
        );
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let current: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(current["state"]["phase"], "imported");
        assert_eq!(persistence.profiles().list().unwrap().revision, 2);
    }
    assert_eq!(
        stats
            .hid_write_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
#[test]
fn bad_local_archive_and_invalid_slot_fail_before_socket_connection() {
    let root = tempfile::tempdir().unwrap();
    let bad = root.path().join("bad.lcp");
    std::fs::write(&bad, b"not an archive").unwrap();
    let out = command(
        root.path(),
        vec![
            "profiles".into(),
            "import".into(),
            "screen".into(),
            "0".into(),
            bad.display().to_string(),
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid archive file type/length"));
    let out = command(
        root.path(),
        vec![
            "profiles".into(),
            "export".into(),
            "screen".into(),
            "8".into(),
            root.path().join("never.lcp").display().to_string(),
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("profile slot out of range"));
    assert!(!root.path().join("never.lcp").exists());
}
