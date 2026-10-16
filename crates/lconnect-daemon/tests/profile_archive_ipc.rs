use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::PersistenceStore,
    stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{client::IpcClient, framing::MAX_JSON_RPC_LINE_BYTES, server::IpcServer, *};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::io::AsyncWriteExt;
async fn launch(
    root: &Path,
) -> (
    Arc<PersistenceStore>,
    ManagerHandle,
    tokio::task::JoinHandle<std::io::Result<()>>,
    PathBuf,
    IpcClient,
) {
    let persistence = Arc::new(PersistenceStore::new_with_root(root.into()));
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(MockBackend::new()),
        persistence.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, persistence.clone());
    let path = root.join("archive-test.sock");
    let server_path = path.clone();
    let server =
        tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&server_path).await });
    let client = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match IpcClient::connect(&path).await {
                Ok(client) => break client,
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .unwrap();
    (persistence, manager, server, path, client)
}
async fn call(client: &IpcClient, request: Value) -> ProfileArchiveReply {
    let params = json!({"request":request});
    assert!(serde_json::to_vec(&params).unwrap().len() < MAX_JSON_RPC_LINE_BYTES);
    serde_json::from_value(client.call("profile_archive", params).await.unwrap()).unwrap()
}
#[tokio::test]
async fn unix_archive_roundtrip_lost_finish_and_daemon_restart_keep_one_import_and_do_not_resurrect_reset_slot(
) {
    let source = tempfile::tempdir().unwrap();
    let (store, manager, server, _, client) = launch(source.path()).await;
    let image = store
        .theme_assets()
        .import_named(
            lconnect_image::defaults::IMAGES[0].png,
            Some("Portable image"),
        )
        .unwrap();
    let font = store
        .theme_fonts()
        .import(include_bytes!(
            "../../lconnect-image/assets/NotoSansTC-Regular.otf"
        ))
        .unwrap();
    let clip = H264Clip {
        fps: 24,
        units: vec![H264AccessUnit {
            bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
            keyframe: true,
        }],
    };
    let media = store
        .media_assets()
        .import("Portable video".into(), clip)
        .unwrap()
        .key;
    let theme:LcdTheme=serde_json::from_value(json!({"version":1,"name":"Archive full CJK","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[{"kind":"text","text":"日本語 繁體中文","style":{"font_asset":font,"x":20,"y":20,"size":40,"color":[255,255,255],"align":"left"}}],"page_cache":[{"page":"rain","theme":{"version":3,"name":"Cached Rain","background":[0,0,0],"background_video":media,"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":1}}]}}]})).unwrap();
    store
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: NamedProfile {
                    slot: 0,
                    name: "Portable source".into(),
                    content: ProfileContent::Screen(theme),
                },
            },
        )
        .unwrap();
    let export = "e".repeat(32);
    let begin = json!({"action":"begin_export","id":export,"expected_revision":1,"group":"screen","slot":0});
    let state = call(&client, begin.clone()).await;
    assert_eq!(state, call(&client, begin).await);
    assert_eq!(state.state.phase, ProfileArchivePhase::Ready);
    let mut bytes: Vec<u8> = Vec::new();
    let mut offset = 0;
    loop {
        let req = json!({"action":"read","id":export,"offset":offset,"length":PROFILE_ARCHIVE_CHUNK_BYTES});
        let r = call(&client, req.clone()).await;
        assert_eq!(r.offset, Some(offset));
        assert!(r.bytes.len() <= PROFILE_ARCHIVE_CHUNK_BYTES);
        if offset == 0 {
            assert_eq!(r, call(&client, req).await);
        }
        offset += r.bytes.len() as u64;
        bytes.extend(&r.bytes);
        if r.eof {
            break;
        }
        assert!(!r.bytes.is_empty());
    }
    assert_eq!(offset, state.state.size);
    assert!(
        serde_json::to_vec(&bytes).unwrap().len() > MAX_JSON_RPC_LINE_BYTES,
        "full archive cannot fit a single JSON RPC frame"
    );
    assert_eq!(
        call(&client, json!({"action":"cancel","id":export}))
            .await
            .state
            .phase,
        ProfileArchivePhase::Cancelled
    );
    assert!(client
        .call(
            "profile_archive",
            json!({"request":{"action":"read","id":export,"offset":0,"length":1}})
        )
        .await
        .is_err());
    call(&client, json!({"action":"release","id":export})).await;
    assert_eq!(
        client.call("daemon_stats", json!({})).await.unwrap()["hid_write_count"],
        0
    );
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;

    let dest = tempfile::tempdir().unwrap();
    let (store, manager, server, path, client) = launch(dest.path()).await;
    let id = "a".repeat(32);
    let begin = json!({"action":"begin_import","id":id,"size":bytes.len(),"expected_revision":0,"group":"screen","slot":7});
    let initial = call(&client, begin.clone()).await;
    assert_eq!(initial, call(&client, begin.clone()).await);
    assert!(client
        .call(
            "profile_archive",
            json!({"request":{"action":"status","id":id,"unknown":true}})
        )
        .await
        .is_err());
    for (index, chunk) in bytes.chunks(PROFILE_ARCHIVE_CHUNK_BYTES).enumerate() {
        call(&client,json!({"action":"chunk","id":id,"offset":index*PROFILE_ARCHIVE_CHUNK_BYTES,"bytes":chunk})).await;
    }
    let inspected = call(&client, json!({"action":"inspect","id":id})).await;
    assert_eq!(inspected.state.profile.unwrap().name, "Portable source");
    // Send Finish on a separate real Unix connection and intentionally never consume its reply.
    let mut lost = tokio::net::UnixStream::connect(&path).await.unwrap();
    let mut line=serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":"profile_archive","params":{"request":{"action":"finish","id":id}}})).unwrap();
    line.push(b'\n');
    lost.write_all(&line).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if store.profiles().list().unwrap().revision == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(lost);
    let first = call(&client, json!({"action":"finish","id":id})).await;
    assert_eq!(first.state.phase, ProfileArchivePhase::Imported);
    assert_eq!(first.state.committed_revision, Some(1));
    assert_eq!(
        first,
        call(&client, json!({"action":"finish","id":id})).await
    );
    let actual = first.state.profile.as_ref().unwrap();
    let ProfileContent::Screen(imported) = &actual.content else {
        panic!()
    };
    assert_eq!(actual.slot, 7);
    assert_eq!(imported.pages().count(), 2);
    assert_ne!(imported.background_image.as_ref().unwrap().asset, image);
    assert!(!imported.font_assets().contains(font.as_str()));
    store.theme_assets().load(imported).unwrap();
    store.theme_fonts().load_theme(imported).unwrap();
    store.media_assets().pin_theme(imported).unwrap();
    let saved = store.profiles().list().unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.archive_imports.len(), 1);
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    drop(store);

    let (store, manager, server, _, client) = launch(dest.path()).await;
    for req in [
        json!({"action":"status","id":id}),
        json!({"action":"finish","id":id}),
        begin.clone(),
        json!({"action":"cancel","id":id}),
    ] {
        assert_eq!(first, call(&client, req).await);
    }
    assert_eq!(store.profiles().list().unwrap().revision, 1);
    let lib = client
        .call(
            "edit_profile",
            json!({"expected_revision":1,"edit":{"action":"delete","group":"screen","slot":7}}),
        )
        .await
        .unwrap();
    assert_eq!(lib["revision"], 2);
    let current = call(&client, json!({"action":"status","id":id})).await;
    assert_eq!(current.state.library.unwrap().revision, 2);
    assert!(current.state.profile.is_none());
    assert_eq!(current.state.committed_revision, Some(1));
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
    drop(store);

    let (store, manager, server, _, client) = launch(dest.path()).await;
    let replay = call(&client, begin).await;
    assert_eq!(replay.state.phase, ProfileArchivePhase::Imported);
    assert_eq!(replay.state.committed_revision, Some(1));
    assert!(replay.state.profile.is_none());
    assert_eq!(replay.state.library.unwrap().revision, 2);
    assert!(
        store.profiles().list().unwrap().profiles.is_empty(),
        "old Finish must not resurrect a subsequently removed slot"
    );
    assert_eq!(
        client.call("daemon_stats", json!({})).await.unwrap()["hid_write_count"],
        0
    );
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
