use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, DaemonRpc, LcdSourceKind,
    LcdState,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
#[tokio::test]
async fn preview_does_not_apply_and_failed_theme_preserves_active_composition() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("theme-test".into()),
        PathBuf::from("/mock/theme"),
    );
    let key = settings_key(&info);
    mock.add_device(info);
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                break device.id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone());
    let (a, b) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let image_key = store
        .theme_assets()
        .import(include_bytes!("../../../desktop/src-tauri/icons/icon.png"))
        .unwrap();
    assert_eq!(
        client.call("list_theme_assets", json!({})).await.unwrap(),
        json!(["builtin-orbit-mark-v1", "builtin-midnight-v1", image_key])
    );
    for builtin in lconnect_image::defaults::IMAGES {
        assert!(client
            .call(
                "delete_theme_asset",
                json!({"key":builtin.key,"expected_revision":0})
            )
            .await
            .is_err());
        assert!(client
            .call(
                "rename_theme_asset",
                json!({"key":builtin.key,"name":"Changed","expected_revision":0})
            )
            .await
            .is_err());
        let preview: Vec<u8> = serde_json::from_value(
            client
                .call("thumbnail_theme_asset", json!({"key":builtin.key}))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            lconnect_image::assets::decode_asset(&preview)
                .unwrap()
                .width(),
            144
        );
    }
    let thumbnail: Vec<u8> = serde_json::from_value(
        client
            .call("thumbnail_theme_asset", json!({"key":image_key}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        lconnect_image::assets::decode_asset(&thumbnail)
            .unwrap()
            .width()
            <= 144
    );
    assert!(client
        .call("thumbnail_theme_asset", json!({"key":"../escape"}))
        .await
        .is_err());
    assert!(client
        .call(
            "thumbnail_theme_asset",
            json!({"key":image_key,"extra":true})
        )
        .await
        .is_err());
    let named = client.call("import_theme_asset", json!({"bytes":include_bytes!("../../../desktop/src-tauri/icons/icon.png").to_vec(),"name":"Original.png"})).await.unwrap();
    let renamed = client
        .call(
            "rename_theme_asset",
            json!({"key":named,"name":"Renamed","expected_revision":1}),
        )
        .await
        .unwrap();
    assert_eq!(renamed["revision"], 2);
    assert!(client
        .call(
            "rename_theme_asset",
            json!({"key":named,"name":"Stale","expected_revision":1})
        )
        .await
        .is_err());
    assert!(client
        .call("list_named_theme_assets", json!({}))
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["key"] == named && a["name"] == "Renamed"));
    let dynamic = client
        .call("get_dynamic_rgb", json!({"dev":dev}))
        .await
        .unwrap();
    assert!(dynamic["requested"].is_null());
    assert!(client
        .call("set_dynamic_rgb", json!({"dev":dev}))
        .await
        .is_err());
    let disabled = client
        .call("set_dynamic_rgb", json!({"dev":dev,"config":null}))
        .await
        .unwrap();
    assert!(disabled["requested"].is_null());
    let fonts = lconnect_daemon::theme_fonts::ThemeFontStore::new(store.theme_fonts_root());
    let font_bytes = include_bytes!("../../lconnect-image/assets/DejaVuSerif.ttf");
    let font_key = fonts.import(font_bytes).unwrap();
    let font_path = store.theme_fonts_root().join(format!("{font_key}.font"));
    let theme = json!({"version":1,"name":"Test","background":[20,30,40],"layers":[{
        "kind":"sensor_cycle","interval_seconds":10,"style":{
            "font_asset":font_key,"x":240,"y":200,"size":32,"color":[255,255,255],"align":"center"
        },"sensors":[
            {"sensor_id":"cpu:test","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":1},
            {"sensor_id":"power:test","label":"Power","unit":"watts","temperature_unit":"celsius","decimals":1}
        ]
    }]});
    let profiles = client.call("list_profiles", json!({})).await.unwrap();
    assert_eq!(profiles["revision"], 0);
    let saved = client.call("edit_profile", json!({"expected_revision":0,"edit":{
        "action":"save","profile":{"slot":0,"name":"My screen","content":{"kind":"screen","value":theme}}
    }})).await.unwrap();
    assert_eq!(saved["profiles"][0]["name"], "My screen");
    assert!(client
        .call(
            "edit_profile",
            json!({"expected_revision":0,"edit":{
                "action":"delete","group":"screen","slot":0
            }})
        )
        .await
        .is_err());
    assert!(client.call("edit_profile", json!({"expected_revision":1,"edit":{
        "action":"save","profile":{"slot":1,"name":"Missing","content":{"kind":"screen","value":{
            "version":1,"name":"Missing","background":[0,0,0],"layers":[{"kind":"image","asset":"missing","x":0,"y":0,"width":480,"height":480,"fit":"crop","opacity":255}]
        }}}
    }})).await.is_err());
    assert_eq!(
        client.call("list_profiles", json!({})).await.unwrap(),
        saved
    );
    let preview = client
        .call("preview_lcd_theme", json!({"theme":theme}))
        .await
        .unwrap();
    let later = client
        .call(
            "preview_lcd_theme",
            json!({"theme":theme,"elapsed_seconds":10}),
        )
        .await
        .unwrap();
    assert_ne!(preview, later, "timed preview must advance the cycle");
    let editor: lconnect_ipc::LcdEditorPreview = serde_json::from_value(
        client
            .call(
                "preview_lcd_editor",
                json!({"theme":theme,"elapsed_seconds":10}),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    let later_bytes: Vec<u8> = serde_json::from_value(later).unwrap();
    assert_eq!(
        editor.jpeg, later_bytes,
        "editor uses the same cycle and renderer"
    );
    assert_eq!(editor.layers.len(), 1);
    let bounds = editor.layers[0].expect("sensor cycle has selectable text");
    assert!(bounds.width > 0.0 && bounds.height > 0.0);
    assert!(client
        .call("preview_lcd_editor", json!({"theme":theme,"apply":true}))
        .await
        .is_err());
    let bytes: Vec<u8> = serde_json::from_value(preview).unwrap();
    assert_eq!(&bytes[..2], &[255, 216]);
    let state: LcdState = serde_json::from_value(
        client
            .call("get_lcd_state", json!({"dev":dev}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert!(state.persistence.is_some());
    let applied: LcdState = serde_json::from_value(
        client
            .call("set_lcd_theme", json!({"dev":dev,"theme":theme}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(applied.source, LcdSourceKind::Theme);
    // Rejected mutations must leave the active producer generation and intent intact.
    for (pointer, value) in [
        ("/version", json!(2)),
        ("/background/0", json!(256)),
        ("/layers/0/style/size", json!(-1)),
        ("/layers/0/style/font_asset", json!("../outside")),
        (
            "/layers/0/style/font_asset",
            json!("00000000000000000000000000000001"),
        ),
    ] {
        let mut invalid = theme.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(
            client
                .call("set_lcd_theme", json!({"dev":dev,"theme":invalid}))
                .await
                .is_err(),
            "accepted {pointer}"
        );
        let state = client
            .call("get_lcd_state", json!({"dev":dev}))
            .await
            .unwrap();
        assert_eq!(state["generation"], applied.generation);
        assert_eq!(
            client
                .call("get_lcd_theme", json!({"dev":dev}))
                .await
                .unwrap(),
            theme
        );
    }
    let mut unknown = theme.clone();
    unknown["unrecognized_setting"] = json!(true);
    assert!(client
        .call("set_lcd_theme", json!({"dev":dev,"theme":unknown}))
        .await
        .is_err());

    for (asset, width) in [("../outside", 480), ("missing", 480), ("missing", 0)] {
        let mut invalid = theme.clone();
        invalid["layers"] = json!([{
            "kind":"image", "asset":asset, "x":0, "y":0,
            "width":width, "height":480, "fit":"crop", "opacity":255
        }]);
        let error = client
            .call("set_lcd_theme", json!({"dev":dev,"theme":invalid}))
            .await
            .unwrap_err();
        if width == 0 {
            assert!(error.to_string().contains("image rectangle"), "{error}");
        }
        let state = client
            .call("get_lcd_state", json!({"dev":dev}))
            .await
            .unwrap();
        assert_eq!(state["generation"], applied.generation);
        assert_eq!(
            client
                .call("get_lcd_theme", json!({"dev":dev}))
                .await
                .unwrap(),
            theme
        );
    }

    // Exercise actual server framing while a different connection owns a live theme.
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    for payload in [
        b"{broken json\n".to_vec(),
        vec![b'x'; lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES + 1],
    ] {
        let oversized = payload.len() > lconnect_ipc::framing::MAX_JSON_RPC_LINE_BYTES;
        let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone());
        let (raw, peer) = mem_pair();
        let task =
            tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(peer).await });
        let (reader, mut writer) = tokio::io::split(raw);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap(); // hello
        tokio::time::timeout(Duration::from_secs(3), writer.write_all(&payload))
            .await
            .unwrap()
            .unwrap();
        if oversized {
            let result = tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        } else {
            line.clear();
            tokio::time::timeout(Duration::from_secs(3), reader.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(reply["error"]["code"], -32700);
            drop(writer);
            drop(reader);
            tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
        let state = client
            .call("get_lcd_state", json!({"dev":dev}))
            .await
            .unwrap();
        assert_eq!(state["generation"], applied.generation);
        assert_eq!(
            client
                .call("get_lcd_theme", json!({"dev":dev}))
                .await
                .unwrap(),
            theme
        );
    }
    // A broken dependency must not replace the active runtime or saved profile.
    std::fs::write(&font_path, b"broken font").unwrap();
    assert!(client
        .call("preview_lcd_theme", json!({"theme":theme}))
        .await
        .is_err());
    assert!(client
        .call("set_lcd_theme", json!({"dev":dev,"theme":theme}))
        .await
        .is_err());
    assert!(client.call("edit_profile", json!({"expected_revision":1,"edit":{
        "action":"save","profile":{"slot":1,"name":"Broken font","content":{"kind":"screen","value":theme}}
    }})).await.is_err());
    assert_eq!(
        client.call("list_profiles", json!({})).await.unwrap(),
        saved
    );
    assert_eq!(
        client
            .call("get_lcd_theme", json!({"dev":dev}))
            .await
            .unwrap(),
        theme
    );
    std::fs::write(&font_path, font_bytes).unwrap();
    let mut bad = theme.clone();
    bad["version"] = 2.into();
    assert!(client
        .call("set_lcd_theme", json!({"dev":dev,"theme":bad}))
        .await
        .is_err());
    assert_eq!(
        client
            .call("get_lcd_theme", json!({"dev":dev}))
            .await
            .unwrap(),
        theme
    );
    assert!(client
        .call("import_theme_asset", json!({"bytes":[1,2,3]}))
        .await
        .is_err());
    manager.shutdown().await;
    drop(client);
    server.abort();
    let _ = server.await;
    assert_eq!(
        serde_json::to_value(store.read(&key).unwrap().theme.unwrap()).unwrap(),
        theme
    );
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = wait_device(&manager).await;
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone());
    assert_eq!(
        rpc.get_lcd_state(dev.clone()).await.unwrap().source,
        LcdSourceKind::Theme
    );
    assert_eq!(
        serde_json::to_value(rpc.get_lcd_theme(dev.clone()).await.unwrap().unwrap()).unwrap(),
        theme
    );
    manager.shutdown().await;
    std::fs::remove_file(&font_path).unwrap();
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = wait_device(&manager).await;
    let rpc = IpcServerImpl::new(manager.clone(), stats.clone(), cfg.clone(), store.clone());
    let failed = rpc.get_lcd_state(dev.clone()).await.unwrap();
    assert_eq!(failed.source, LcdSourceKind::Idle);
    assert!(failed.last_error.unwrap().contains("theme restore"));
    assert_eq!(
        serde_json::to_value(store.read(&key).unwrap().theme.unwrap()).unwrap(),
        theme
    );
    std::fs::write(&font_path, font_bytes).unwrap();
    rpc.set_lcd_theme(dev.clone(), serde_json::from_value(theme.clone()).unwrap())
        .await
        .unwrap();
    rpc.set_lcd_image(dev, vec![255, 216, 255, 217])
        .await
        .unwrap();
    manager.shutdown().await;
    assert!(
        store.read(&key).unwrap().theme.is_none(),
        "manual image must cancel saved theme replay"
    );

    let mut saved = store.read(&key).unwrap();
    let mut missing = theme;
    missing["layers"] = json!([{"kind":"image","asset":"lost","x":0,"y":0,"width":480,"height":480,"fit":"crop","opacity":255}]);
    saved.theme = Some(serde_json::from_value(missing).unwrap());
    store.write(&key, &saved).unwrap();
    let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
        .spawn()
        .await;
    let dev = wait_device(&manager).await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let state = rpc.get_lcd_state(dev).await.unwrap();
    assert_eq!(state.source, LcdSourceKind::Idle);
    assert!(state.last_error.unwrap().contains("theme restore"));
    manager.shutdown().await;
    assert_eq!(
        store.read(&key).unwrap().theme,
        saved.theme,
        "restore failure retains repairable intent"
    );
}

async fn wait_device(manager: &lconnect_daemon::manager::ManagerHandle) -> lconnect_ipc::DeviceId {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(device) = manager.list().await.first() {
                break device.id.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn cycle_continues_after_ipc_disconnect_and_replays_after_manager_restart() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(directory.path().into()));
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("cycle-disconnect".into()),
        PathBuf::from("/mock/cycle-disconnect"),
    );
    mock.add_device(info.clone());
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = wait_device(&manager).await;
    let rpc = Arc::new(IpcServerImpl::new(
        manager.clone(),
        stats.clone(),
        cfg.clone(),
        store.clone(),
    ));
    let (a, b) = mem_pair();
    let server_rpc = rpc.clone();
    let server = tokio::spawn(async move { IpcServer::new(server_rpc).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let theme = json!({"version":1,"name":"Independent Cycle","background":[0,0,0],"layers":[{
        "kind":"sensor_cycle","interval_seconds":1,
        "style":{"x":240,"y":200,"size":32,"color":[255,255,255],"align":"center"},
        "sensors":[
            {"sensor_id":"missing:cpu","label":"CPU","unit":"celsius","temperature_unit":"celsius","decimals":1},
            {"sensor_id":"missing:power","label":"Power","unit":"watts","temperature_unit":"celsius","decimals":0}
        ]
    }]});
    let mut expected = Vec::new();
    for elapsed in [0, 1] {
        let bytes: Vec<u8> = serde_json::from_value(
            client
                .call(
                    "preview_lcd_theme",
                    json!({"theme":theme,"elapsed_seconds":elapsed}),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        expected.push(
            lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(&bytes)
                .unwrap()
                .into_iter()
                .map(|packet| packet.to_vec())
                .collect::<Vec<_>>(),
        );
    }
    assert_ne!(expected[0], expected[1]);
    client
        .call("set_lcd_theme", json!({"dev":dev,"theme":theme}))
        .await
        .unwrap();
    drop(client);
    server.abort();
    let _ = server.await;
    let before = mock.write_history(&info).len();
    wait_for_cycle_packets(&mock, &info, before, &expected).await;
    assert_eq!(
        serde_json::to_value(rpc.get_lcd_theme(dev).await.unwrap().unwrap()).unwrap(),
        theme
    );
    manager.shutdown().await;
    drop(rpc);
    let before_restart = mock.write_history(&info).len();
    let manager = DeviceManager::new(
        Arc::new(mock.clone()),
        store.clone(),
        cfg.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let dev = wait_device(&manager).await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store);
    assert_eq!(
        serde_json::to_value(rpc.get_lcd_theme(dev.clone()).await.unwrap().unwrap()).unwrap(),
        theme
    );
    wait_for_cycle_packets(&mock, &info, before_restart, &expected).await;
    assert_eq!(
        rpc.get_lcd_state(dev).await.unwrap().source,
        LcdSourceKind::Theme
    );
    manager.shutdown().await;
}

async fn wait_for_cycle_packets(
    mock: &MockBackend,
    info: &DeviceInfo,
    after: usize,
    expected: &[Vec<Vec<u8>>],
) {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let history = mock.write_history(info);
            if expected.iter().all(|frame| {
                history[after..]
                    .windows(frame.len())
                    .any(|batch| batch == frame)
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("both complete cycle frames must reach the LCD owner without an IPC client");
}

#[tokio::test]
async fn corrupt_image_or_font_on_restart_preserves_intent_and_recovers_without_apply() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_path_buf();
    let store = Arc::new(PersistenceStore::new_with_root(root.clone()));
    let image = store
        .theme_assets()
        .import(include_bytes!("../../../desktop/src-tauri/icons/icon.png"))
        .unwrap();
    let font = lconnect_daemon::theme_fonts::ThemeFontStore::new(store.theme_fonts_root())
        .import(include_bytes!(
            "../../lconnect-image/assets/DejaVuSerif.ttf"
        ))
        .unwrap();
    let paths = [
        store.theme_assets_root().join(format!("{image}.png")),
        store.theme_fonts_root().join(format!("{font}.font")),
    ];
    let theme: lconnect_ipc::LcdTheme = serde_json::from_value(json!({
        "version":1,"name":"Repairable theme","background":[20,30,40],"layers":[
            {"kind":"image","asset":image,"x":0,"y":0,"width":480,"height":480,"fit":"contain","opacity":255},
            {"kind":"sensor","sensor_id":"fixture:original-cpu","label":"CPU","unit":"celsius","temperature_unit":"fahrenheit","decimals":1,
             "style":{"font_asset":font,"x":240,"y":200,"size":32,"color":[255,255,255],"align":"center"}}
        ]
    })).unwrap();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("repairable-theme".into()),
        PathBuf::from("/mock/repairable-theme"),
    );
    let key = settings_key(&info);
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let mock = MockBackend::new();
    mock.add_device(info.clone());
    let stats = Arc::new(Stats::new());
    let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
        .spawn()
        .await;
    let dev = wait_device(&manager).await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg.clone(), store.clone());
    let jpeg = rpc.preview_lcd_theme(None, theme.clone()).await.unwrap();
    let expected: Vec<Vec<u8>> =
        lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(&jpeg)
            .unwrap()
            .into_iter()
            .map(|p| p.to_vec())
            .collect();
    rpc.set_lcd_theme(dev, theme.clone()).await.unwrap();
    manager.shutdown().await;
    let saved = serde_json::to_value(store.read(&key).unwrap()).unwrap();

    for path in paths {
        // Restore normalized managed bytes, not the original pre-import image.
        let bytes = std::fs::read(&path).unwrap();
        for damaged in [true, false] {
            std::fs::write(
                &path,
                if damaged {
                    b"corrupt dependency".as_slice()
                } else {
                    &bytes
                },
            )
            .unwrap();
            // Fresh stores and backends cannot mask disk corruption with session caches/history.
            let store = Arc::new(PersistenceStore::new_with_root(root.clone()));
            let mock = MockBackend::new();
            mock.add_device(info.clone());
            let stats = Arc::new(Stats::new());
            let manager = DeviceManager::new(
                Arc::new(mock.clone()),
                store.clone(),
                cfg.clone(),
                stats.clone(),
            )
            .spawn()
            .await;
            let dev = wait_device(&manager).await;
            let rpc = IpcServerImpl::new(manager.clone(), stats, cfg.clone(), store.clone());
            let state = rpc.get_lcd_state(dev.clone()).await.unwrap();
            let active = rpc.get_lcd_theme(dev).await.unwrap();
            if damaged {
                assert_eq!(state.source, LcdSourceKind::Idle);
                assert!(state
                    .last_error
                    .as_deref()
                    .unwrap()
                    .contains("theme restore"));
                assert!(active.is_none());
            } else {
                assert_eq!(state.source, LcdSourceKind::Theme);
                assert!(state.last_error.is_none());
                assert_eq!(active, Some(theme.clone()));
            }
            manager.shutdown().await;
            let history = mock.write_history(&info);
            if damaged {
                assert!(
                    !history.iter().any(|packet| packet.get(1) == Some(&14)),
                    "corrupt dependency must not start a JPEG"
                );
            } else {
                assert!(
                    history
                        .windows(expected.len())
                        .any(|batch| batch == expected),
                    "repaired theme must send the original full JPEG batch without Apply"
                );
            }
            assert_eq!(serde_json::to_value(store.read(&key).unwrap()).unwrap(), saved,
                "failed or successful restore must preserve theme, bindings and neighboring settings");
        }
    }
}
