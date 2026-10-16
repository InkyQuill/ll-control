use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, H264AccessUnit,
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
async fn long_upload_exceeds_one_rpc_frame_and_lost_finish_reply_recovers_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let id = "c".repeat(32);
    let begin = json!({"request":{"action":"begin","id":id,"name":"Long upload","frames":586,"encoded_bytes":586*4096}});
    assert!(
        serde_json::to_vec(&unit(0)).unwrap().len() * 586
            > lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES
    );
    for restart in [false, true] {
        let persistence = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
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
            persistence.clone(),
        ));
        let (a, b) = mem_pair();
        let serving = rpc.clone();
        let server = tokio::spawn(async move { IpcServer::new(serving).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if !restart {
            assert!(client.call("media_upload", json!({"request":{"action":"begin","id":id,"name":"Bad","frames":1,"encoded_bytes":15,"unknown":true}})).await.is_err());
            assert_eq!(
                client.call("media_upload", begin.clone()).await.unwrap()["received_frames"],
                0
            );
            for index in 0..586 {
                let params =
                    json!({"request":{"action":"chunk","id":id,"index":index,"unit":unit(index)}});
                assert!(
                    serde_json::to_vec(&params).unwrap().len()
                        < lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES - 1024
                );
                let state = client.call("media_upload", params.clone()).await.unwrap();
                assert_eq!(state["received_frames"], index + 1);
                if index == 100 {
                    assert_eq!(state, client.call("media_upload", params).await.unwrap());
                }
            }
            assert!(persistence.media_assets().user_list().unwrap().is_empty());
            // Drop a real RPC connection after delivering Finish but before
            // reading its reply. The blocking commit must still complete.
            let (mut lost, b) = mem_pair();
            let serving = rpc.clone();
            let finishing =
                tokio::spawn(async move { IpcServer::new(serving).serve_stream(b).await });
            // The server writes hello before accepting requests. Wait for it so
            // disconnect tests loss of Finish's reply, not failure of the handshake.
            let mut hello = String::new();
            BufReader::new(&mut lost)
                .read_line(&mut hello)
                .await
                .unwrap();
            assert!(hello.contains("hello"));
            let mut line = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":"media_upload","params":{"request":{"action":"finish","id":id}}})).unwrap();
            line.push(b'\n');
            lost.write_all(&line).await.unwrap();
            drop(lost);
            let result = tokio::time::timeout(Duration::from_secs(5), finishing)
                .await
                .unwrap()
                .unwrap();
            assert!(result.is_err(), "peer closed before publication reply");
        } else {
            let frame = client
                .call("load_media_first_frame", json!({"key":id}))
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_value::<H264AccessUnit>(frame.clone()).unwrap(),
                unit(0)
            );
            assert!(serde_json::to_vec(&frame).unwrap().len() < 128 * 1024 * 4 + 1024);
            assert!(
                client
                    .call("load_prepared_clip", json!({"key":id}))
                    .await
                    .is_err(),
                "v2 must not materialize as a diagnostic clip"
            );
            assert!(client
                .call("load_media_first_frame", json!({"key":"../escape"}))
                .await
                .is_err());
            assert!(client
                .call("load_media_first_frame", json!({"key":id,"extra":true}))
                .await
                .is_err());
            for params in [
                json!({"request":{"action":"status","id":id}}),
                begin.clone(),
                json!({"request":{"action":"finish","id":id}}),
                json!({"request":{"action":"cancel","id":id}}),
            ] {
                let state = client.call("media_upload", params).await.unwrap();
                assert_eq!(state["phase"], "published");
                assert_eq!(state["asset"]["key"], id);
                assert_eq!(state["received_frames"], 586);
            }
            let store = persistence.media_assets();
            let key = id.clone();
            tokio::task::spawn_blocking(move || {
                assert_eq!(store.user_list().unwrap().len(), 1);
                let mut source = store.acquire_source(&key).unwrap();
                for index in 0..586 {
                    assert_eq!(source.read_unit(index).unwrap(), unit(index));
                }
            })
            .await
            .unwrap();
        }
        drop(client);
        server.abort();
        let _ = server.await;
        drop(rpc);
        manager.shutdown().await;
    }
}

// These lifecycle/quota assertions count user publications, excluding the immutable catalog.
trait UserMediaList {
    fn user_list(&self) -> anyhow::Result<Vec<lconnect_ipc::PreparedClipInfo>>;
}
impl UserMediaList for lconnect_daemon::media_assets::MediaAssetStore {
    fn user_list(&self) -> anyhow::Result<Vec<lconnect_ipc::PreparedClipInfo>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|clip| !clip.is_default)
            .collect())
    }
}
