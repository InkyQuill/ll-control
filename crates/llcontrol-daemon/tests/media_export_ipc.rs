use llcontrol_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use llcontrol_hid::MockBackend;
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, H264AccessUnit,
    MediaExportEvent,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn unit(index: usize) -> H264AccessUnit {
    let mut bytes = vec![255; 4096];
    bytes[..15].copy_from_slice(&[0, 0, 1, 7, 1, 0, 0, 1, 8, 1, 0, 0, 1, 5, 1]);
    bytes[4090..].copy_from_slice(format!("{index:06}").as_bytes());
    H264AccessUnit {
        bytes,
        keyframe: true,
    }
}

#[tokio::test]
async fn long_export_and_disconnected_slow_consumer_release_source() {
    let dir = tempfile::tempdir().unwrap();
    let persistence = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
    let store = persistence.media_assets();
    let saved = tokio::task::spawn_blocking({
        let store = store.clone();
        move || {
            let mut writer = store.create_file("Editable".into()).unwrap();
            for index in 0..586 {
                writer.append(&unit(index)).unwrap();
            }
            store.publish_file(writer).unwrap()
        }
    })
    .await
    .unwrap();
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
    let rpc = Arc::new(IpcServerImpl::new(
        manager.clone(),
        stats,
        config,
        persistence,
    ));
    let (a, b) = mem_pair();
    let server = tokio::spawn({
        let rpc = rpc.clone();
        async move { IpcServer::new(rpc).serve_stream(b).await }
    });
    let client = IpcClient::from_stream(a).await.unwrap();
    let mut events = client.subscribe_reliable("media_export").await;
    let id = "a".repeat(32);
    assert!(client
        .call(
            "open_media_export",
            json!({"key":saved.key,"stream_id":"bad"})
        )
        .await
        .is_err());
    let metadata = client
        .call("open_media_export", json!({"key":saved.key,"stream_id":id}))
        .await
        .unwrap();
    assert_eq!(metadata["info"]["frames"], 586);
    assert_eq!(metadata["info"]["encoded_bytes"], 586 * 4096);
    for index in 0..586 {
        let notification = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        match serde_json::from_value::<MediaExportEvent>(notification.params).unwrap() {
            MediaExportEvent::Unit {
                stream_id,
                index: actual,
                unit: actual_unit,
            } => {
                assert_eq!(stream_id, id);
                assert_eq!(actual, index);
                assert_eq!(actual_unit, unit(index));
            }
            other => panic!("unexpected event {other:?}"),
        }
    }
    assert!(
        matches!(serde_json::from_value::<MediaExportEvent>(events.recv().await.unwrap().params).unwrap(), MediaExportEvent::Complete {stream_id} if stream_id==id)
    );
    assert!(client
        .call("open_media_export", json!({"key":saved.key,"stream_id":id}))
        .await
        .is_err());
    drop(client);
    server.abort();
    let _ = server.await;

    store.delete_unreferenced(&saved.key, || Ok(false)).unwrap();
    for corrupt in [true, false] {
        let saved = tokio::task::spawn_blocking({
            let store = store.clone();
            move || {
                let mut writer = store.create_file("Editable".into()).unwrap();
                for index in 0..586 {
                    writer.append(&unit(index)).unwrap();
                }
                store.publish_file(writer).unwrap()
            }
        })
        .await
        .unwrap();
        // Tiny wire buffer holds a real source pin under backpressure.
        let (a, b) = tokio::io::duplex(1024);
        let server = tokio::spawn({
            let rpc = rpc.clone();
            async move { IpcServer::new(rpc).serve_stream(b).await }
        });
        let mut peer = BufReader::new(a);
        let mut line = String::new();
        peer.read_line(&mut line).await.unwrap();
        let request = json!({"jsonrpc":"2.0","id":1,"method":"open_media_export","params":{"key":saved.key,"stream_id":id}});
        peer.write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();
        line.clear();
        peer.read_line(&mut line).await.unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&line)
            .unwrap()
            .get("result")
            .is_some());
        assert!(store.delete_unreferenced(&saved.key, || Ok(false)).is_err());
        if corrupt {
            // Source opened successfully; damage the last payload while pumping.
            let path = dir
                .path()
                .join("prepared-clips")
                .join(format!("{}.lcm", saved.key));
            tokio::task::spawn_blocking(move || {
                let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
                file.set_len(file.metadata().unwrap().len() - 1).unwrap();
            })
            .await
            .unwrap();
            let mut count = 0;
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    line.clear();
                    assert!(peer.read_line(&mut line).await.unwrap() > 0);
                    let notification: llcontrol_ipc::jsonrpc::Notification =
                        serde_json::from_str(&line).unwrap();
                    match serde_json::from_value::<MediaExportEvent>(notification.params).unwrap() {
                        MediaExportEvent::Unit { index, .. } => {
                            assert_eq!(index, count);
                            count += 1;
                        }
                        MediaExportEvent::Failed { stream_id, .. } => {
                            assert_eq!(stream_id, id);
                            break;
                        }
                        MediaExportEvent::Complete { .. } => {
                            panic!("corrupt prefix reported complete")
                        }
                    }
                }
            })
            .await
            .unwrap();
            assert!(count < 586);
        }
        drop(peer);
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if store.delete_unreferenced(&saved.key, || Ok(false)).is_ok() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    drop(rpc);
    manager.shutdown().await;
}
