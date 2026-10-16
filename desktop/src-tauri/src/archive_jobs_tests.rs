use super::*;
use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::PersistenceStore,
    stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{server::IpcServer, LcdTheme, NamedProfile, ProfileContent, ProfileEdit};
use std::sync::atomic::Ordering;
async fn daemon(
    root: &std::path::Path,
) -> (
    Arc<PersistenceStore>,
    Arc<Stats>,
    ManagerHandle,
    tokio::task::JoinHandle<std::io::Result<()>>,
    PathBuf,
) {
    let store = Arc::new(PersistenceStore::new_with_root(root.join("settings")));
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), config, store.clone());
    let path = root.join("archive.sock");
    let server_path = path.clone();
    let server =
        tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&server_path).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(client) = IpcClient::connect(&path).await {
                drop(client);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    (store, stats, manager, server, path)
}
async fn terminal(jobs: &Archives) -> Status {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(s) = jobs.status().unwrap() {
                if s.phase.terminal() {
                    return s;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
fn request(n: u8, mode: Mode, revision: u64, slot: u8) -> Start {
    Start {
        id: format!("{n:032x}"),
        mode,
        group: ProfileGroup::Screen,
        slot,
        expected_revision: revision,
        suggested_name: "Native.lcp".into(),
    }
}
#[tokio::test]
async fn native_files_roundtrip_large_font_without_payload_in_status_and_recover_after_owner_restart(
) {
    let root = tempfile::tempdir().unwrap();
    let (store, stats, manager, server, socket) = daemon(root.path()).await;
    let font = store
        .theme_fonts()
        .import(include_bytes!(
            "../../../crates/lconnect-image/assets/NotoSansTC-Regular.otf"
        ))
        .unwrap();
    let theme:LcdTheme=serde_json::from_value(serde_json::json!({"version":1,"name":"Native CJK","background":[0,0,0],"layers":[{"kind":"text","text":"日本語 繁體中文","style":{"font_asset":font,"x":20,"y":20,"size":36,"color":[255,255,255],"align":"left"}}]})).unwrap();
    store
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: NamedProfile {
                    slot: 0,
                    name: "Native source".into(),
                    content: ProfileContent::Screen(theme),
                },
            },
        )
        .unwrap();
    let jobs = Archives::default();
    let file = root.path().join("portable.lcp");
    let selected = file.clone();
    jobs.start_with(
        socket.clone(),
        request(1, Mode::Export, 1, 0),
        move |_| async move { Ok(Some(selected)) },
    )
    .unwrap();
    let exported = terminal(&jobs).await;
    assert_eq!(exported.phase, Phase::Complete, "{exported:?}");
    let length = std::fs::metadata(&file).unwrap().len();
    assert!(length > 1024 * 1024);
    assert_eq!(exported.transferred, length);
    assert!(
        serde_json::to_vec(&exported).unwrap().len() < 32768,
        "native status contains metadata, not archive payload"
    );
    store
        .profiles()
        .edit(
            1,
            ProfileEdit::Delete {
                group: ProfileGroup::Screen,
                slot: 0,
            },
        )
        .unwrap();
    let selected = file.clone();
    jobs.start_with(
        socket.clone(),
        request(2, Mode::Import, 2, 7),
        move |_| async move { Ok(Some(selected)) },
    )
    .unwrap();
    let imported = terminal(&jobs).await;
    assert_eq!(imported.phase, Phase::Complete, "{imported:?}");
    let result = imported.result.unwrap();
    let ProfileContent::Screen(theme) = &result.state.profile.as_ref().unwrap().content else {
        panic!()
    };
    assert!(!theme.font_assets().contains(font.as_str()));
    store.theme_fonts().load_theme(theme).unwrap();
    assert_eq!(store.profiles().list().unwrap().revision, 3);
    jobs.shutdown().await.unwrap();
    let reopened = Archives::default();
    reopened
        .recover(
            socket.clone(),
            format!("{:032x}", 2),
            Recovery::Status,
            ProfileGroup::Screen,
            7,
        )
        .unwrap();
    let recovered = terminal(&reopened).await;
    assert_eq!(recovered.phase, Phase::Complete);
    assert_eq!(recovered.result.unwrap().state.committed_revision, Some(3));
    assert_eq!(store.profiles().list().unwrap().revision, 3);
    reopened.shutdown().await.unwrap();
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 0);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
#[tokio::test]
async fn existing_destination_and_invalid_input_preserve_files_and_never_commit() {
    let root = tempfile::tempdir().unwrap();
    let (store, stats, manager, server, socket) = daemon(root.path()).await;
    store
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap(),
            },
        )
        .unwrap();
    let file = root.path().join("existing.lcp");
    std::fs::write(&file, b"keep me").unwrap();
    let jobs = Archives::default();
    let selected = file.clone();
    jobs.start_with(
        socket.clone(),
        request(1, Mode::Export, 1, 0),
        move |_| async move { Ok(Some(selected)) },
    )
    .unwrap();
    let failed = terminal(&jobs).await;
    assert_eq!(failed.phase, Phase::Failed);
    assert_eq!(failed.message_code, Some(ArchiveNotice::DestinationExists));
    assert_eq!(
        serde_json::to_value(&failed).unwrap()["message_code"],
        "destination_exists"
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"keep me");
    let selected = file.clone();
    jobs.start_with(
        socket,
        request(2, Mode::Import, 1, 0),
        move |_| async move { Ok(Some(selected)) },
    )
    .unwrap();
    let failed = terminal(&jobs).await;
    assert_eq!(failed.phase, Phase::Failed);
    assert_eq!(failed.message_code, Some(ArchiveNotice::InvalidArchiveFile));
    assert_eq!(store.profiles().list().unwrap().revision, 1);
    assert_eq!(stats.hid_write_count.load(Ordering::Relaxed), 0);
    jobs.shutdown().await.unwrap();
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
#[test]
fn cancellation_and_commit_transition_share_one_atomic_decision() {
    let make = || Status {
        id: "a".repeat(32),
        mode: Mode::Import,
        group: ProfileGroup::Screen,
        slot: 0,
        phase: Phase::Uploading,
        transferred: 100,
        total: 100,
        file_name: None,
        message: None,
        message_code: None,
        result: None,
    };
    let first = Archives::default();
    let (_s, signal, _done) = first.admit(make()).unwrap();
    first.cancel(&"a".repeat(32)).unwrap();
    assert!(*signal.borrow());
    assert!(first.begin_commit(&"a".repeat(32)).is_err());
    let second = Archives::default();
    let (_s, signal, _done) = second.admit(make()).unwrap();
    second.begin_commit(&"a".repeat(32)).unwrap();
    assert!(second.cancel(&"a".repeat(32)).is_err());
    assert!(!*signal.borrow());
}
#[tokio::test]
async fn cancellation_and_shutdown_keep_admission_until_owned_chooser_really_finishes() {
    let jobs = Archives::default();
    let (release, blocked) = tokio::sync::oneshot::channel();
    let (started, entered) = tokio::sync::oneshot::channel();
    let req = request(1, Mode::Import, 0, 0);
    let id = req.id.clone();
    jobs.start_with(PathBuf::from("/never-connect"), req, move |_| async move {
        let _ = started.send(());
        blocked.await?;
        Ok(None)
    })
    .unwrap();
    entered.await.unwrap();
    jobs.cancel(&id).unwrap();
    assert_eq!(jobs.status().unwrap().unwrap().phase, Phase::Cancelling);
    assert!(jobs
        .start_with(
            PathBuf::from("/never-connect"),
            request(2, Mode::Import, 0, 0),
            move |_| async move { Ok(None) }
        )
        .is_err());
    let owner = jobs.clone();
    let shutdown = tokio::spawn(async move { owner.shutdown().await });
    tokio::task::yield_now().await;
    assert!(!shutdown.is_finished());
    release.send(()).unwrap();
    shutdown.await.unwrap().unwrap();
    assert_eq!(terminal(&jobs).await.phase, Phase::Cancelled);
    assert!(jobs
        .start_with(
            PathBuf::from("/never-connect"),
            request(3, Mode::Import, 0, 0),
            move |_| async move { Ok(None) }
        )
        .is_err());
}

#[test]
fn start_and_recover_without_runtime_fail_before_reserving_job() {
    let jobs = Archives::default();
    assert!(jobs
        .start_with(
            PathBuf::from("/never-connect"),
            request(1, Mode::Import, 0, 0),
            move |_| async move { Ok(None) }
        )
        .is_err());
    assert!(jobs.status().unwrap().is_none());
    assert!(jobs
        .recover(
            PathBuf::from("/never-connect"),
            format!("{:032x}", 1),
            Recovery::Status,
            ProfileGroup::Screen,
            0
        )
        .is_err());
    assert!(jobs.status().unwrap().is_none());
}
