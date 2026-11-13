//! Real codec → private IPC → LCD actor, using MockBackend only.
use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, PersistenceStore},
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{client::IpcClient, server::IpcServer, transport_memory::mem_pair};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};

// The real transport drains stale reads before a command. Queue the firmware
// reply only when that command is written, exactly as a device would answer.
struct FirmwareMock(MockBackend);
struct FirmwareHandle {
    inner: Box<dyn lconnect_hid::OpenHandle>,
    mock: MockBackend,
    info: DeviceInfo,
}
impl lconnect_hid::HidBackend for FirmwareMock {
    fn enumerate(&self) -> lconnect_hid::Result<Vec<DeviceInfo>> {
        self.0.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> lconnect_hid::Result<Box<dyn lconnect_hid::OpenHandle>> {
        Ok(Box::new(FirmwareHandle {
            inner: self.0.open(info)?,
            mock: self.0.clone(),
            info: info.clone(),
        }))
    }
}
impl lconnect_hid::OpenHandle for FirmwareHandle {
    fn write(&self, bytes: &[u8]) -> lconnect_hid::Result<()> {
        self.inner.write(bytes)?;
        if bytes.starts_with(&[1, 134]) {
            let packet = lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                134,
                b"N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
            );
            self.mock
                .queue_response(&self.info, packet.as_bytes().to_vec());
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout_ms: i32) -> lconnect_hid::Result<usize> {
        self.inner.read_timeout(bytes, timeout_ms)
    }
}

// Fail only response writes after the hello handshake. The request still reaches
// the real dispatcher; the test observes a failed reply, not a fake RPC error.
struct LostReplyStream {
    inner: tokio::io::DuplexStream,
    fail: Arc<std::sync::atomic::AtomicBool>,
    failures: Arc<std::sync::atomic::AtomicUsize>,
}
impl tokio::io::AsyncRead for LostReplyStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}
impl tokio::io::AsyncWrite for LostReplyStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        if this.fail.load(std::sync::atomic::Ordering::SeqCst) {
            this.failures
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return std::task::Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()));
        }
        std::pin::Pin::new(&mut this.inner).poll_write(cx, buf)
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

async fn apply_with_lost_reply(
    rpc: Arc<IpcServerImpl>,
    device: &lconnect_ipc::DeviceId,
    theme: &serde_json::Value,
) {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (peer, inner) = mem_pair();
    let fail = Arc::new(AtomicBool::new(false));
    let failures = Arc::new(AtomicUsize::new(0));
    let stream = LostReplyStream {
        inner,
        fail: fail.clone(),
        failures: failures.clone(),
    };
    let server = tokio::spawn(async move { IpcServer::new(rpc).serve_stream(stream).await });
    let mut peer = BufReader::new(peer);
    let mut hello = String::new();
    peer.read_line(&mut hello).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&hello).unwrap()["method"],
        "hello"
    );
    fail.store(true, Ordering::SeqCst);
    let mut request = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":"set_lcd_theme","params":{"dev":device,"theme":theme}})).unwrap();
    request.push(b'\n');
    peer.get_mut().write_all(&request).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while failures.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Apply response must hit injected write failure");
    drop(peer);
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

async fn apply_then_close_input(
    rpc: Arc<IpcServerImpl>,
    device: &lconnect_ipc::DeviceId,
    theme: &serde_json::Value,
) {
    use tokio::io::AsyncWriteExt;
    let (mut peer, inner) = mem_pair();
    let server = tokio::spawn(async move { IpcServer::new(rpc).serve_stream(inner).await });
    let mut request = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":2,"method":"set_lcd_theme","params":{"dev":device,"theme":theme}})).unwrap();
    request.push(b'\n');
    peer.write_all(&request).await.unwrap();
    // EOF is already buffered when dispatch returns its deferred preparation.
    // The connection owns that task and must cancel it rather than later Apply.
    peer.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn composition_apply_restart_sleep_missing_media_recovery_and_clear() {
    // Reset's reap assertion observes every child of this process. Run this
    // test alone in a child harness so parallel codec tests cannot contribute
    // unrelated PIDs to that observation; retain the exact reap assertion.
    const CHILD_ENV: &str = "LCONNECT_COMPOSITION_LIFECYCLE_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "composition_apply_restart_sleep_missing_media_recovery_and_clear",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .contains("test result: ok. 1 passed; 0 failed;"),
            "isolated lifecycle test failed or did not run exactly one test ({}):\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let media = store.media_assets();
    let prepared_media = media.clone();
    let key = tokio::task::spawn_blocking(move || {
        let encoded = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=480x480:r=24",
                "-frames:v",
                "1",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-profile:v",
                "baseline",
                "-pix_fmt",
                "yuv420p",
                "-threads",
                "1",
                "-f",
                "h264",
                "-",
            ])
            .output()
            .unwrap();
        assert!(encoded.status.success());
        prepared_media
            .import(
                "Background".into(),
                lconnect_ipc::H264Clip {
                    fps: 24,
                    units: vec![lconnect_ipc::H264AccessUnit {
                        bytes: encoded.stdout,
                        keyframe: true,
                    }],
                },
            )
            .unwrap()
            .key
    })
    .await
    .unwrap();
    let theme = json!({"version":2,"name":"Live video","background":[0,0,0],"background_video":key,
        "layers":[{"kind":"text","text":"CPU overlay","style":{"x":240,"y":200,"size":40,"color":[255,255,255],"align":"center"}}]});
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("composition".into()),
        PathBuf::from("/mock/composition"),
    );
    let device_key = settings_key(&info);
    let record = root
        .path()
        .join("prepared-clips")
        .join(format!("{key}.json"));
    for stage in 0..4 {
        if stage == 2 {
            std::fs::rename(&record, record.with_extension("saved")).unwrap();
        }
        if stage == 3 {
            std::fs::rename(record.with_extension("saved"), &record).unwrap();
        }
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        let stats = Arc::new(Stats::new());
        let mut config = Config::default();
        config.poll.hotplug_ms = 20;
        let manager = DeviceManager::new(
            Arc::new(FirmwareMock(mock.clone())),
            store.clone(),
            config.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let mut device = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(dev) = manager.list().await.first() {
                    break dev.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let rpc = Arc::new(IpcServerImpl::new(
            manager.clone(),
            stats,
            config,
            store.clone(),
        ));
        let (a, b) = mem_pair();
        let connection_rpc = rpc.clone();
        let server =
            tokio::spawn(async move { IpcServer::new(connection_rpc).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if stage == 0 {
            let preview_key = media
                .import("Preview only".into(), media.load(&key).unwrap())
                .unwrap()
                .key;
            let mut preview_theme = theme.clone();
            preview_theme["background_video"] = json!(preview_key);
            let before = client
                .call("get_lcd_state", json!({"dev":device}))
                .await
                .unwrap();
            let lcd_writes = || {
                mock.write_history(&info)
                    .into_iter()
                    .filter(|p| p.get(1).is_some_and(|cmd| matches!(cmd, 12..=14)))
                    .count()
            };
            let writes = lcd_writes();
            for dev in [serde_json::Value::Null, json!(device)] {
                let jpeg: Vec<u8> = serde_json::from_value(
                    client
                        .call(
                            "preview_lcd_theme",
                            json!({"dev":dev,"theme":preview_theme,"elapsed_seconds":5}),
                        )
                        .await
                        .unwrap(),
                )
                .unwrap();
                let image = lconnect_image::assets::decode_asset(&jpeg)
                    .unwrap()
                    .to_rgb8();
                assert_eq!(image.dimensions(), (480, 480));
                let corner = image.get_pixel(0, 0).0;
                assert!(
                    corner[2] > 220 && corner[0] < 20 && corner[1] < 20,
                    "preview lost video background"
                );
                assert!(
                    image
                        .pixels()
                        .any(|pixel| pixel.0.iter().all(|channel| *channel > 200)),
                    "overlay missing from video preview"
                );
            }
            assert_eq!(
                client
                    .call("get_lcd_state", json!({"dev":device}))
                    .await
                    .unwrap(),
                before
            );
            assert_eq!(lcd_writes(), writes, "preview mutated LCD");
            client
                .call("delete_prepared_clip", json!({"key":preview_key}))
                .await
                .unwrap();
            assert!(client
                .call(
                    "preview_lcd_theme",
                    json!({"dev":device,"theme":preview_theme,"elapsed_seconds":0})
                )
                .await
                .is_err());
            let applied = client
                .call("set_lcd_theme", json!({"dev":device,"theme":theme}))
                .await
                .unwrap();
            assert_eq!(applied["source"], "Theme");
            let mut abandoned = theme.clone();
            abandoned["name"] = json!("Disconnected draft must not apply");
            for _ in 0..3 {
                apply_then_close_input(rpc.clone(), &device, &abandoned).await;
            }
            assert_eq!(
                client
                    .call("get_lcd_theme", json!({"dev":device}))
                    .await
                    .unwrap(),
                theme
            );
            // The response is lost after actor commit. A second connection must
            // see the applied theme and continued playback without resending it.
            let mut lost_reply_theme = theme.clone();
            lost_reply_theme["name"] = json!("Committed without response");
            apply_with_lost_reply(rpc.clone(), &device, &lost_reply_theme).await;
            assert_eq!(
                client
                    .call("get_lcd_theme", json!({"dev":device}))
                    .await
                    .unwrap(),
                lost_reply_theme
            );
            let before = client
                .call("get_lcd_playback_stats", json!({"dev":device}))
                .await
                .unwrap()["sent_frames"]
                .as_u64()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let after = client
                        .call("get_lcd_playback_stats", json!({"dev":device}))
                        .await
                        .unwrap()["sent_frames"]
                        .as_u64()
                        .unwrap();
                    if after >= before + 3 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("playback stopped when reply connection closed");
            // Successful commit must release both preparation slots for later Apply.
            for _ in 0..3 {
                client
                    .call("set_lcd_theme", json!({"dev":device,"theme":theme}))
                    .await
                    .unwrap();
            }
            let mut missing = theme.clone();
            missing["background_video"] = json!("f".repeat(32));
            assert!(client
                .call("set_lcd_theme", json!({"dev":device,"theme":missing}))
                .await
                .is_err());
        }
        let state = client
            .call("get_lcd_state", json!({"dev":device}))
            .await
            .unwrap();
        if stage == 2 {
            assert_eq!(state["source"], "Idle");
            assert!(state["last_error"].as_str().unwrap().contains("restore"));
            assert_eq!(
                serde_json::to_value(store.read(&device_key).unwrap().theme.unwrap()).unwrap(),
                theme
            );
        } else {
            assert_eq!(state["source"], "Theme");
            assert_eq!(
                client
                    .call("get_lcd_theme", json!({"dev":device}))
                    .await
                    .unwrap(),
                theme
            );
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let stats = client
                        .call("get_lcd_playback_stats", json!({"dev":device}))
                        .await
                        .unwrap();
                    if stats["sent_frames"].as_u64().unwrap() >= 3 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert!(client
                .call("delete_prepared_clip", json!({"key":key}))
                .await
                .is_err());
            if stage == 1 {
                manager.prepare_for_sleep(true).await.unwrap();
                let history = mock.write_history(&info);
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(mock.write_history(&info), history);
                assert!(client
                    .call("delete_prepared_clip", json!({"key":key}))
                    .await
                    .is_err());
                manager.prepare_for_sleep(false).await.unwrap();
                let new_device = manager.list().await.first().unwrap().id.clone();
                assert_ne!(new_device, device);
                device = new_device;
                assert_eq!(
                    client
                        .call("get_lcd_theme", json!({"dev":device}))
                        .await
                        .unwrap(),
                    theme
                );
            }
            if stage == 3 {
                let children: std::collections::BTreeSet<u32> =
                    std::fs::read_dir("/proc/self/task")
                        .unwrap()
                        .flat_map(|task| {
                            std::fs::read_to_string(task.unwrap().path().join("children"))
                                .unwrap_or_default()
                                .split_whitespace()
                                .map(|id| id.parse::<u32>().unwrap())
                                .collect::<Vec<_>>()
                        })
                        .collect();
                assert!(
                    children.len() >= 2,
                    "live decoder and encoder must be present before reset"
                );
                let lconnect_ipc::ProfileContent::Screen(default) =
                    lconnect_ipc::NamedProfile::defaults(
                        lconnect_ipc::ProfileGroup::Screen,
                        0,
                        None,
                    )
                    .unwrap()
                    .content
                else {
                    panic!("default screen");
                };
                let runtime = Arc::new(
                    lconnect_daemon::theme_runtime::ThemeRuntime::new(
                        default.clone(),
                        Default::default(),
                        vec![],
                    )
                    .unwrap(),
                );
                let handles = manager
                    .with_device(&device, |d| d.handles.clone())
                    .await
                    .unwrap();
                let (reply, response) = tokio::sync::oneshot::channel();
                handles
                    .lcd_tx
                    .send(lconnect_daemon::actor::LcdCommand::ResetScreen {
                        preparation: None,
                        operation_id: "a".repeat(32),
                        expected_profile_revision: store.profiles().list().unwrap().revision,
                        expected_collection_revision: 0,
                        prepared: runtime.prepare().await.unwrap(),
                        reply,
                    })
                    .await
                    .unwrap();
                let reset = response.await.unwrap().unwrap();
                assert!(reset.lcd.last_error.is_none());
                assert_eq!(reset.lcd.requested.mode, "Application");
                for pid in children {
                    assert!(
                        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                        "reset must reap codec child {pid} before replying"
                    );
                }
                assert_eq!(
                    client
                        .call("get_lcd_theme", json!({"dev":device}))
                        .await
                        .unwrap(),
                    serde_json::to_value(default).unwrap()
                );
                let before = client
                    .call("get_lcd_playback_stats", json!({"dev":device}))
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
                let after = client
                    .call("get_lcd_playback_stats", json!({"dev":device}))
                    .await
                    .unwrap();
                assert_eq!(
                    before["sent_frames"], after["sent_frames"],
                    "old video output stopped"
                );
                // Deletion while the manager is still alive proves old source and
                // pending settings pins were released by reset, not by shutdown.
                client
                    .call("delete_prepared_clip", json!({"key":key}))
                    .await
                    .unwrap();
                client
                    .call("clear_lcd", json!({"dev":device}))
                    .await
                    .unwrap();
            }
        }
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        if stage != 3 {
            assert_eq!(
                serde_json::to_value(store.read(&device_key).unwrap().theme.unwrap()).unwrap(),
                theme
            );
        }
    }
    assert!(store.read(&device_key).unwrap().theme.is_none());
    assert!(media.list().unwrap().iter().all(|clip| clip.key != key));
}

#[tokio::test]
async fn procedural_weather_apply_restart_sleep_and_clear_use_the_same_owner() {
    use lconnect_ipc::LcdTheme;
    let theme: LcdTheme=serde_json::from_value(json!({"version":3,"name":"Rain","background":[12,18,30],"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":2,"seed":42,"dynamic":false,"sensor":null,"retained_sensors":[{"sensor_id":"cpu/load","unit":"percent","low":30,"high":60},{"sensor_id":"pump/rpm","unit":"rpm","low":800,"high":1200}]}},{"kind":"text","text":"CPU overlay","style":{"x":240,"y":200,"size":40,"color":[255,255,255],"align":"center"}}]})).unwrap();
    procedural_effect_owner(theme).await;
}
#[tokio::test]
async fn procedural_meteor_apply_restart_sleep_and_clear_use_the_same_owner() {
    let theme:lconnect_ipc::LcdTheme=serde_json::from_value(json!({"version":3,"name":"Meteor","background":[12,18,30],"layers":[{"kind":"meteor","effect":{"speed":2,"colors":[[0,255,0],[0,0,255]],"dynamic":false,"sensor":null,"seed":42}},{"kind":"text","text":"CPU overlay","style":{"x":240,"y":200,"size":40,"color":[255,255,255],"align":"center"}}]})).unwrap();
    procedural_effect_owner(theme).await;
}
#[tokio::test]
async fn procedural_snow3d_apply_restart_sleep_and_clear_use_the_same_owner() {
    let theme:lconnect_ipc::LcdTheme=serde_json::from_value(json!({"version":3,"name":"Snow3D","background":[12,18,30],"layers":[{"kind":"snow3d","effect":{"intensity":3,"seed":42}},{"kind":"text","text":"CPU overlay","style":{"x":240,"y":200,"size":40,"color":[255,255,255],"align":"center"}}]})).unwrap();
    procedural_effect_owner(theme).await;
}
#[tokio::test]
async fn procedural_fireworks_apply_restart_sleep_and_clear_use_the_same_owner() {
    let theme:lconnect_ipc::LcdTheme=serde_json::from_value(json!({"version":3,"name":"Fireworks","background":[12,18,30],"layers":[{"kind":"fireworks","effect":{"intensity":2,"colors":lconnect_ipc::FIREWORKS_COLORS,"dynamic":false,"sensor":null,"seed":42}},{"kind":"text","text":"CPU overlay","style":{"x":240,"y":200,"size":40,"color":[255,255,255],"align":"center"}}]})).unwrap();
    procedural_effect_owner(theme).await;
}
async fn procedural_effect_owner(theme: lconnect_ipc::LcdTheme) {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("weather".into()),
        PathBuf::from("/mock/weather"),
    );
    let device_key = settings_key(&info);
    for stage in 0..2 {
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        let stats = Arc::new(Stats::new());
        let mut config = Config::default();
        config.poll.hotplug_ms = 20;
        let manager = DeviceManager::new(
            Arc::new(FirmwareMock(mock.clone())),
            store.clone(),
            config.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let mut device = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(dev) = manager.list().await.first() {
                    break dev.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let rpc = Arc::new(IpcServerImpl::new(
            manager.clone(),
            stats,
            config,
            store.clone(),
        ));
        let (a, b) = mem_pair();
        let server = tokio::spawn(async move { IpcServer::new(rpc).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        if stage == 0 {
            let before = mock.write_history(&info);
            let preview: Vec<u8> = serde_json::from_value(
                client
                    .call(
                        "preview_lcd_theme",
                        json!({"dev":device,"theme":theme,"elapsed_seconds":2}),
                    )
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert!(preview.starts_with(&[0xff, 0xd8]));
            assert_eq!(
                mock.write_history(&info)
                    .into_iter()
                    .filter(|p| p.get(1).is_some_and(|v| matches!(v, 12..=14)))
                    .count(),
                before
                    .into_iter()
                    .filter(|p| p.get(1).is_some_and(|v| matches!(v, 12..=14)))
                    .count(),
                "Preview mutated LCD"
            );
            client
                .call("set_lcd_theme", json!({"dev":device,"theme":theme}))
                .await
                .unwrap();
        }
        wait_restored_theme(&client, &device, &serde_json::to_value(&theme).unwrap()).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frames = client
                    .call("get_lcd_playback_stats", json!({"dev":device}))
                    .await
                    .unwrap()["sent_frames"]
                    .as_u64()
                    .unwrap();
                if frames >= 5 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            mock.write_history(&info)
                .iter()
                .any(|p| p.get(1) == Some(&13)),
            "Weather never reached H264 transport"
        );
        if stage == 1 {
            manager.prepare_for_sleep(true).await.unwrap();
            let writes = mock.write_history(&info);
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(mock.write_history(&info), writes);
            manager.prepare_for_sleep(false).await.unwrap();
            device = manager.list().await.first().unwrap().id.clone();
            wait_restored_theme(&client, &device, &serde_json::to_value(&theme).unwrap()).await;
            client
                .call("clear_lcd", json!({"dev":device}))
                .await
                .unwrap();
        }
        drop(client);
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
        if stage == 0 {
            assert_eq!(
                store.read(&device_key).unwrap().theme.as_ref(),
                Some(&theme)
            );
        }
    }
    assert!(store.read(&device_key).unwrap().theme.is_none());
}

async fn wait_restored_theme(
    client: &IpcClient,
    device: &lconnect_ipc::DeviceId,
    theme: &serde_json::Value,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let restored = client
                .call("get_lcd_theme", json!({"dev":device}))
                .await
                .unwrap();
            let state = client
                .call("get_lcd_state", json!({"dev":device}))
                .await
                .unwrap();
            if &restored == theme && state["source"] == "Theme" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Weather theme was not restored");
}

#[tokio::test]
async fn rotated_media_and_theme_start_and_manager_replay_preserve_identity_and_wire_zero_rotation()
{
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("rotation".into()),
        PathBuf::from("/mock/rotation"),
    );
    let device_key = settings_key(&info);
    let mut initial = lconnect_daemon::persistence::DeviceSettings::default();
    initial.lcd.brightness = 73;
    initial.lcd.rotation = "0".into();
    initial.lcd.control_mode = "Application".into();
    initial.media_clip = Some(lconnect_ipc::BUILTIN_VIDEO_KEY.into());
    store.write(&device_key, &initial).unwrap();
    let theme = json!({"version":3,"name":"Rotated rain","background":[0,0,0],"layers":[{"kind":"weather","effect":{"kind":"rain","intensity":1,"dynamic":false,"sensor":null,"seed":42}}]});
    for stage in 0..3 {
        let mock = MockBackend::new();
        mock.add_device(info.clone());
        let stats = Arc::new(Stats::new());
        let mut config = Config::default();
        config.poll.hotplug_ms = 20;
        let manager = DeviceManager::new(
            Arc::new(FirmwareMock(mock.clone())),
            store.clone(),
            config.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let device = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if let Some(dev) = manager.list().await.first() {
                    break dev.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let rpc = Arc::new(IpcServerImpl::new(
            manager.clone(),
            stats,
            config,
            store.clone(),
        ));
        let (a, b) = mem_pair();
        let server = tokio::spawn(async move { IpcServer::new(rpc).serve_stream(b).await });
        let client = IpcClient::from_stream(a).await.unwrap();
        let state = client
            .call("get_lcd_state", json!({"dev":device}))
            .await
            .unwrap();
        assert!(state["last_error"].is_null(), "{state}");
        assert_eq!(
            state["requested"]["rotation_degrees"],
            if stage == 0 { 0 } else { 270 }
        );
        if stage < 2 {
            assert_eq!(
                client
                    .call("get_lcd_media", json!({"dev":device}))
                    .await
                    .unwrap(),
                json!(lconnect_ipc::BUILTIN_VIDEO_KEY)
            );
            assert!(client
                .call("get_lcd_theme", json!({"dev":device}))
                .await
                .unwrap()
                .is_null());
        } else {
            assert_eq!(
                client
                    .call("get_lcd_theme", json!({"dev":device}))
                    .await
                    .unwrap(),
                theme
            );
        }
        let streaming: Vec<_> = mock
            .write_history(&info)
            .into_iter()
            .filter(|p| p.len() == 1024 && p[1] == 12)
            .filter_map(|p| {
                let data =
                    lconnect_protocol::galahad2_lcdv2::bpacket::BCommandPacket::from_response(
                        &p, 12,
                    )
                    .ok()?
                    .data()
                    .to_vec();
                (data[0]
                    == lconnect_protocol::galahad2_lcdv2::lcd::LCDControlMode::LCDSetting as u8)
                    .then_some(data)
            })
            .collect();
        assert!(
            !streaming.is_empty(),
            "stage={stage}, state={state}, writes={:?}",
            mock.write_history(&info)
                .iter()
                .map(|p| (p.len(), p[..p.len().min(12)].to_vec()))
                .collect::<Vec<_>>()
        );
        assert!(streaming.iter().all(|data| data[2] == 0));
        // Live updates keep this worker/source and advance its existing counters.
        let before = client
            .call("get_lcd_state", json!({"dev":device}))
            .await
            .unwrap();
        let configs_before = mock
            .write_history(&info)
            .iter()
            .filter(|p| p.len() == 1024 && p[1] == 12)
            .count();
        let counters_before = client
            .call("get_lcd_playback_stats", json!({"dev":device}))
            .await
            .unwrap();
        for angle in [0, 90, 180, 270] {
            client
                .call(
                    "patch_lcd_config",
                    json!({"dev":device,"patch":{"rotation_degrees":angle}}),
                )
                .await
                .unwrap();
            let after = client
                .call("get_lcd_state", json!({"dev":device}))
                .await
                .unwrap();
            assert_eq!(after["generation"], before["generation"]);
            assert_eq!(after["requested"]["rotation_degrees"], angle);
            assert_eq!(after["applied"]["rotation_degrees"], angle);
            assert!(after["last_error"].is_null(), "{after}");
            assert_eq!(after["source"], before["source"]);
            let counters = client
                .call("get_lcd_playback_stats", json!({"dev":device}))
                .await
                .unwrap();
            assert!(
                counters["sent_frames"].as_u64().unwrap()
                    >= counters_before["sent_frames"].as_u64().unwrap(),
                "live transition must not reset counters"
            );
            if stage < 2 {
                assert_eq!(
                    client
                        .call("get_lcd_media", json!({"dev":device}))
                        .await
                        .unwrap(),
                    json!(lconnect_ipc::BUILTIN_VIDEO_KEY)
                );
                assert!(client
                    .call("get_lcd_theme", json!({"dev":device}))
                    .await
                    .unwrap()
                    .is_null());
            }
        }
        assert_eq!(
            mock.write_history(&info)
                .iter()
                .filter(|p| p.len() == 1024 && p[1] == 12)
                .count(),
            configs_before,
            "angle-only change must not reconfigure HID"
        );
        if stage == 0 {
            client
                .call(
                    "patch_lcd_config",
                    json!({"dev":device,"patch":{"brightness_percent":85}}),
                )
                .await
                .unwrap();
            client
                .call("clear_lcd", json!({"dev":device}))
                .await
                .unwrap();
            client
                .call(
                    "patch_lcd_config",
                    json!({"dev":device,"patch":{"rotation_degrees":270}}),
                )
                .await
                .unwrap();
            client
                .call(
                    "set_lcd_media",
                    json!({"dev":device,"key":lconnect_ipc::BUILTIN_VIDEO_KEY}),
                )
                .await
                .unwrap();
            let still = json!({"version":1,"name":"Still","background":[20,30,40],"layers":[]});
            client
                .call("set_lcd_theme", json!({"dev":device,"theme":still}))
                .await
                .unwrap();
            let configs: Vec<_> = mock
                .write_history(&info)
                .into_iter()
                .filter(|p| p.len() == 1024 && p[1] == 12)
                .collect();
            let last = lconnect_protocol::galahad2_lcdv2::bpacket::BCommandPacket::from_response(
                configs.last().unwrap(),
                12,
            )
            .unwrap();
            assert_eq!(
                last.data()[2],
                3,
                "JPEG after rotated H264 must restore hardware270 orientation"
            );
            client
                .call(
                    "set_lcd_media",
                    json!({"dev":device,"key":lconnect_ipc::BUILTIN_VIDEO_KEY}),
                )
                .await
                .unwrap();
        } else if stage == 1 {
            client
                .call("set_lcd_theme", json!({"dev":device,"theme":theme}))
                .await
                .unwrap();
        }
        drop(client);
        server.await.unwrap().unwrap();
        manager.shutdown().await;
        let saved = store.read(&device_key).unwrap();
        assert_eq!(saved.lcd.rotation, "270");
        if stage == 0 {
            assert!(saved.theme.is_none());
            assert_eq!(
                saved.media_clip.as_deref(),
                Some(lconnect_ipc::BUILTIN_VIDEO_KEY)
            );
        } else {
            assert_eq!(serde_json::to_value(saved.theme.unwrap()).unwrap(), theme);
            assert!(saved.media_clip.is_none());
        }
    }
}
