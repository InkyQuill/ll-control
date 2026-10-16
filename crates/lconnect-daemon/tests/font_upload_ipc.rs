use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::MockBackend;
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, FontUploadState,
    FONT_CHUNK_BYTES,
};
use serde_json::json;
use std::sync::Arc;
#[tokio::test]
async fn full_cjk_font_crosses_real_rpc_in_bounded_chunks_and_finish_is_retryable() {
    let dir = tempfile::tempdir().unwrap();
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
    let rpc = IpcServerImpl::new(
        manager.clone(),
        stats.clone(),
        config.clone(),
        persistence.clone(),
    );
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();

    let bytes = include_bytes!("../../lconnect-image/assets/NotoSansTC-Regular.otf");
    assert!(
        serde_json::to_vec(&bytes.as_slice()).unwrap().len()
            > lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES
    );
    assert!(client
        .call(
            "font_upload",
            json!({"request":{"action":"begin","size":1,"unknown":true}})
        )
        .await
        .is_err());
    let state: FontUploadState = serde_json::from_value(
        client
            .call(
                "font_upload",
                json!({"request":{"action":"begin","size":bytes.len()}}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    for (i, chunk) in bytes.chunks(FONT_CHUNK_BYTES).enumerate() {
        let params = json!({"request":{"action":"chunk","id":state.id,"offset":i*FONT_CHUNK_BYTES,"bytes":chunk}});
        assert!(
            serde_json::to_vec(&params).unwrap().len()
                < lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES - 1024
        );
        client.call("font_upload", params).await.unwrap();
    }
    let params = json!({"request":{"action":"finish","id":state.id}});
    let first = client.call("font_upload", params.clone()).await.unwrap();
    assert_eq!(first, client.call("font_upload", params).await.unwrap());
    let key = first["asset"].as_str().unwrap();
    let stored = std::fs::read(dir.path().join("theme-fonts").join(format!("{key}.font"))).unwrap();
    assert_eq!(stored, bytes);
    assert_eq!(
        std::fs::read_dir(dir.path().join("theme-fonts"))
            .unwrap()
            .count(),
        1
    );
    let theme = json!({"version":1,"name":"Uploaded CJK","background":[0,0,0],"layers":[{
        "kind":"text","text":"繁體中文 測試", "style":{"font_asset":key,"x":20,"y":20,"size":40,"color":[255,255,255],"align":"left"}
    }]});
    let actual = client
        .call("preview_lcd_theme", json!({"theme":theme}))
        .await
        .unwrap();
    let mut bundled = theme.clone();
    bundled["layers"][0]["style"]
        .as_object_mut()
        .unwrap()
        .remove("font_asset");
    bundled["layers"][0]["style"]["font"] = "noto_sans_tc_regular".into();
    assert_eq!(
        actual,
        client
            .call("preview_lcd_theme", json!({"theme":bundled}))
            .await
            .unwrap()
    );
    let saved = client.call("edit_profile", json!({"expected_revision":0,"edit":{
        "action":"save","profile":{"slot":0,"name":"CJK font","content":{"kind":"screen","value":theme}}
    }})).await.unwrap();
    assert_eq!(saved["revision"], 1);
    assert_eq!(
        client.call("list_profiles", json!({})).await.unwrap(),
        saved
    );
    let cleanup = client
        .call("cleanup_theme_fonts", json!({"delete":true,"keep":[]}))
        .await
        .unwrap();
    assert_eq!(cleanup["deleted"], json!([]));
    assert_eq!(cleanup["retained_referenced"], 1);
    assert!(client
        .call("cleanup_theme_fonts", json!({"keep":["../escape"]}))
        .await
        .is_err());
    drop(client);
    server.abort();
    manager.shutdown().await;
}
