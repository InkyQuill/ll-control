//! CLI → private Unix IPC → real daemon library/LCD actor with MockBackend only.
use lconnect_daemon::{
    config::Config, ipc_server::IpcServerImpl, manager::DeviceManager,
    persistence::PersistenceStore, stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::server::IpcServer;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

// The real transport drains stale reads before a command. Queue the firmware
// reply only when that command is written, exactly as a device would answer.
struct FirmwareMock(MockBackend, bool);
struct FirmwareHandle {
    inner: Box<dyn lconnect_hid::OpenHandle>,
    mock: MockBackend,
    info: DeviceInfo,
    date: bool,
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
            date: self.1,
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
            if self.date {
                let mut packet =
                    lconnect_protocol::galahad2_lcdv2::packets::ACommandPacket::new_request(
                        134,
                        b"Apr 12 2024 14:33:21",
                    )
                    .as_bytes()
                    .to_vec();
                packet[4] = 1;
                self.mock.queue_response(&self.info, packet);
            }
        }
        Ok(())
    }
    fn read_timeout(&self, bytes: &mut [u8], timeout_ms: i32) -> lconnect_hid::Result<usize> {
        self.inner.read_timeout(bytes, timeout_ms)
    }
}

async fn cli(socket: &Path, args: &[&str]) -> std::process::Output {
    tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
            .args(args)
            .env("LCONNECTCTL_SOCKET", socket)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap()
}
fn success(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[tokio::test]
async fn cli_import_offline_restart_play_saved_and_clear_via_mock_actor() {
    library_lifecycle(false).await;
}
#[tokio::test]
async fn indexed_media_select_profile_restart_sleep_clear_and_delete_via_mock_actor() {
    library_lifecycle(true).await;
}
async fn library_lifecycle(indexed: bool) {
    let temp = tempfile::tempdir().unwrap();
    let manifest =
        json!({"fps":24,"units":[{"bytes":[0,0,1,7,1,0,0,1,8,1,0,0,1,5,1],"keyframe":true}]});
    let path = temp.path().join("input.json");
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let path = path.to_str().unwrap();
    let mut saved = Value::Null;
    for stage in 0..5 {
        if stage == 2 || stage == 3 {
            let record = temp.path().join("settings/prepared-clips").join(format!(
                "{}.{}",
                saved["key"].as_str().unwrap(),
                if indexed { "lcm" } else { "json" }
            ));
            let backup = record.with_extension("saved");
            if stage == 2 {
                std::fs::rename(record, backup).unwrap();
            } else {
                std::fs::rename(backup, record).unwrap();
            }
        }
        let connected = stage != 0;
        let socket = temp.path().join("fixture.sock");
        let _ = std::fs::remove_file(&socket);
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(
            temp.path().join("settings"),
        ));
        let mock = MockBackend::new();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("saved-clip".into()),
            PathBuf::from("/mock/saved-clip"),
        );
        if connected {
            mock.add_device(info.clone());
        }
        let stats = Arc::new(Stats::new());
        let mut cfg = Config::default();
        cfg.poll.sensor_hz = 0;
        let manager = DeviceManager::new(
            Arc::new(FirmwareMock(mock.clone(), false)),
            store.clone(),
            cfg.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        let rpc = Arc::new(IpcServerImpl::new(manager.clone(), stats, cfg, store));
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                // One command at a time; all client connections close after reply.
                IpcServer::new(rpc.clone())
                    .serve_stream(stream)
                    .await
                    .unwrap();
            }
        });
        if !connected {
            assert_eq!(
                success(cli(&socket, &["lcd", "clips"]).await),
                json!([lconnect_daemon::builtin_media::info()])
            );
            saved = if indexed {
                let media =
                    PersistenceStore::new_with_root(temp.path().join("settings")).media_assets();
                let first: lconnect_ipc::H264Clip =
                    serde_json::from_value(manifest.clone()).unwrap();
                serde_json::to_value(
                    tokio::task::spawn_blocking(move || {
                        let mut writer = media.create_file("Мой ролик".into()).unwrap();
                        for _ in 0..586 {
                            writer.append(&first.units[0]).unwrap();
                        }
                        media.publish_file(writer).unwrap()
                    })
                    .await
                    .unwrap(),
                )
                .unwrap()
            } else {
                success(
                    cli(
                        &socket,
                        &["lcd", "import-clip", path, "--name", "Мой ролик"],
                    )
                    .await,
                )
            };
            assert_eq!(saved["name"], "Мой ролик");
            let profile_path = temp.path().join("profile.json");
            std::fs::write(&profile_path, serde_json::to_vec(&json!({"slot":7,"name":"Cinema","content":{"kind":"screen_media","value":saved["key"]}})).unwrap()).unwrap();
            let result = cli(
                &socket,
                &["profiles", "save", profile_path.to_str().unwrap()],
            )
            .await;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        if stage != 2 {
            assert_eq!(
                success(cli(&socket, &["lcd", "clips"]).await),
                json!([lconnect_daemon::builtin_media::info(), saved])
            );
        }
        if connected {
            let mut dev = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Some(d) = manager.list().await.first() {
                        break d.id.0.clone();
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            if stage == 2 {
                assert_eq!(
                    success(cli(&socket, &["lcd", "selected-clip", "--device", &dev]).await),
                    saved["key"]
                );
                let state = success(cli(&socket, &["lcd", "state", "--device", &dev]).await);
                assert_eq!(state["source"], "Idle");
                assert!(state["last_error"]
                    .as_str()
                    .unwrap()
                    .contains("media restore"));
                assert!(!mock
                    .write_history(&info)
                    .iter()
                    .any(|p| p.get(1) == Some(&13)));
                server.abort();
                let _ = server.await;
                manager.shutdown().await;
                continue;
            }
            if stage == 3 {
                assert_eq!(
                    success(cli(&socket, &["lcd", "selected-clip", "--device", &dev]).await),
                    saved["key"]
                );
                let state = success(cli(&socket, &["lcd", "state", "--device", &dev]).await);
                assert_eq!(state["source"], "DiagnosticClip");
                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        if mock
                            .write_history(&info)
                            .iter()
                            .filter(|p| p.get(1) == Some(&13))
                            .count()
                            >= 4
                        {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
            }
            if stage == 3 {
                tokio::time::timeout(Duration::from_secs(3), manager.prepare_for_sleep(true))
                    .await
                    .unwrap()
                    .unwrap();
                assert!(manager.list().await.is_empty());
                let sleeping_history = mock.write_history(&info);
                tokio::time::sleep(Duration::from_millis(150)).await;
                assert_eq!(
                    mock.write_history(&info),
                    sleeping_history,
                    "frames must stop before sleep acknowledgement"
                );
                let old_device = dev.clone();
                let stale = cli(
                    &socket,
                    &[
                        "lcd",
                        "select-clip",
                        saved["key"].as_str().unwrap(),
                        "--device",
                        &old_device,
                    ],
                )
                .await;
                assert!(!stale.status.success());
                assert_eq!(mock.write_history(&info), sleeping_history);
                let deletion = cli(
                    &socket,
                    &["lcd", "delete-clip", saved["key"].as_str().unwrap()],
                )
                .await;
                assert!(
                    !deletion.status.success(),
                    "sleeping device still references the clip"
                );
                tokio::time::timeout(Duration::from_secs(3), manager.prepare_for_sleep(false))
                    .await
                    .unwrap()
                    .unwrap();
                dev = manager.list().await.first().unwrap().id.0.clone();
                assert_ne!(
                    dev, old_device,
                    "wake must allocate a new attachment identity"
                );
                assert_eq!(
                    success(cli(&socket, &["lcd", "selected-clip", "--device", &dev]).await),
                    saved["key"]
                );
                let clip: lconnect_ipc::H264Clip =
                    serde_json::from_value(manifest.clone()).unwrap();
                let packets: Vec<Vec<u8>> =
                    lconnect_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(
                        &clip.units[0].bytes,
                    )
                    .unwrap()
                    .into_iter()
                    .map(|p| p.to_vec())
                    .collect();
                assert!(
                    mock.write_history(&info)[sleeping_history.len()..]
                        .windows(packets.len())
                        .any(|batch| batch == packets),
                    "wake must resend complete stored frame on new actor"
                );
            }
            if stage == 4 {
                let referenced = cli(
                    &socket,
                    &["lcd", "delete-clip", saved["key"].as_str().unwrap()],
                )
                .await;
                assert!(
                    !referenced.status.success(),
                    "profile survives device Clear and restart"
                );
                let removed = cli(&socket, &["profiles", "delete", "screen", "7"]).await;
                assert!(removed.status.success());
                let deletion = cli(
                    &socket,
                    &["lcd", "delete-clip", saved["key"].as_str().unwrap()],
                )
                .await;
                assert!(
                    deletion.status.success(),
                    "{}",
                    String::from_utf8_lossy(&deletion.stderr)
                );
                assert_eq!(
                    success(cli(&socket, &["lcd", "clips"]).await),
                    json!([lconnect_daemon::builtin_media::info()])
                );
                assert_eq!(
                    success(cli(&socket, &["lcd", "selected-clip", "--device", &dev]).await),
                    Value::Null
                );
                assert!(!mock
                    .write_history(&info)
                    .iter()
                    .any(|p| p.get(1) == Some(&13)));
                server.abort();
                let _ = server.await;
                manager.shutdown().await;
                continue;
            }
            let state = if stage == 3 {
                success(cli(&socket, &["lcd", "state", "--device", &dev]).await)
            } else if indexed {
                success(
                    cli(
                        &socket,
                        &[
                            "lcd",
                            "select-clip",
                            saved["key"].as_str().unwrap(),
                            "--device",
                            &dev,
                        ],
                    )
                    .await,
                )
            } else {
                success(
                    cli(
                        &socket,
                        &[
                            "lcd",
                            "play-saved-clip",
                            saved["key"].as_str().unwrap(),
                            "--loops",
                            "2",
                            "--device",
                            &dev,
                        ],
                    )
                    .await,
                )
            };
            assert_eq!(state["source"], "DiagnosticClip");
            let prepared: lconnect_ipc::H264Clip =
                serde_json::from_value(manifest.clone()).unwrap();
            let expected: Vec<Vec<u8>> =
                lconnect_protocol::galahad2_lcdv2::h264::build_send_h264_frame_512_request(
                    &prepared.units[0].bytes,
                )
                .unwrap()
                .into_iter()
                .map(|p| p.to_vec())
                .collect();
            assert!(mock
                .write_history(&info)
                .windows(expected.len())
                .any(|batch| batch == expected));
            let before_clear = mock.write_history(&info).len();
            let cleared = cli(&socket, &["lcd", "clear", "--device", &dev]).await;
            assert!(cleared.status.success(), "{:?}", cleared);
            use lconnect_protocol::galahad2_lcdv2::lcd::{
                build_lcd_control_request, Brightness, LCDControlSetting,
            };
            let mut setting = LCDControlSetting::default();
            setting.brightness =
                Brightness::new(state["requested"]["brightness_percent"].as_u64().unwrap() as u8)
                    .unwrap();
            setting.video_fps = 24;
            let control = build_lcd_control_request(setting);
            assert!(mock.write_history(&info)[before_clear..]
                .iter()
                .any(|p| p.as_slice() == control));
            let stopped = success(cli(&socket, &["lcd", "state", "--device", &dev]).await);
            assert_eq!(stopped["source"], "Idle");
            assert_eq!(stopped["applied"]["mode"], "LocalUI");
            let before = mock.write_history(&info).len();
            let bad = cli(
                &socket,
                &["lcd", "play-saved-clip", "../../escape", "--device", &dev],
            )
            .await;
            assert!(!bad.status.success());
            assert!(String::from_utf8_lossy(&bad.stderr).contains("invalid prepared clip key"));
            assert_eq!(mock.write_history(&info).len(), before);
            assert_eq!(
                success(cli(&socket, &["lcd", "clips"]).await),
                json!([lconnect_daemon::builtin_media::info(), saved])
            );
            if stage == 1 {
                let selected = success(
                    cli(
                        &socket,
                        &[
                            "--json", "profiles", "apply", "screen", "7", "--device", &dev,
                        ],
                    )
                    .await,
                );
                assert_eq!(selected["source"], "DiagnosticClip");
                let deletion = cli(
                    &socket,
                    &["lcd", "delete-clip", saved["key"].as_str().unwrap()],
                )
                .await;
                assert!(
                    !deletion.status.success(),
                    "playing/queued selection must retain its file"
                );
                assert_eq!(
                    success(cli(&socket, &["lcd", "selected-clip", "--device", &dev]).await),
                    saved["key"]
                );
            }
        }
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
    }
}
#[tokio::test]
async fn bad_import_and_invalid_loop_fail_before_connecting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.json");
    std::fs::write(&path, b"{bad json").unwrap();
    let socket = temp.path().join("nonexistent.sock");
    for args in [
        vec![
            "lcd",
            "import-clip",
            path.to_str().unwrap(),
            "--name",
            "Name",
        ],
        vec![
            "lcd",
            "play-saved-clip",
            "0123456789abcdef0123456789abcdef",
            "--loops",
            "0",
        ],
    ] {
        let output = cli(&socket, &args).await;
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("connect"));
    }
}

#[tokio::test]
async fn firmware_cli_uses_one_request_and_reports_real_or_unavailable_date() {
    for with_date in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("firmware.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(
            temp.path().join("settings"),
        ));
        let mock = MockBackend::new();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("firmware".into()),
            PathBuf::from("/mock/firmware"),
        );
        mock.add_device(info.clone());
        let mut cfg = Config::default();
        cfg.poll.sensor_hz = 0;
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(FirmwareMock(mock.clone(), with_date)),
            store.clone(),
            cfg.clone(),
            stats.clone(),
        )
        .spawn()
        .await;
        for _ in 0..100 {
            if !manager.list().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!manager.list().await.is_empty());
        let rpc = Arc::new(IpcServerImpl::new(manager.clone(), stats, cfg, store));
        let server = tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                IpcServer::new(rpc.clone())
                    .serve_stream(stream)
                    .await
                    .unwrap();
            }
        });
        let value = success(cli(&socket, &["--json", "firmware"]).await);
        assert_eq!(value["version"], "1.6");
        if with_date {
            assert_eq!(value["build_datetime"], "Apr 12 2024 14:33:21");
        } else {
            assert!(value.get("build_datetime").is_none());
            assert!(value["raw_token"].as_str().unwrap().ends_with('|'));
        }
        assert_eq!(
            mock.write_history(&info)
                .iter()
                .filter(|bytes| bytes.starts_with(&[1, 134]))
                .count(),
            1
        );
        let output = cli(&socket, &["firmware"]).await;
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(if with_date {
            "Build date: Apr 12 2024 14:33:21"
        } else {
            "Build date: unavailable"
        }));
        assert_eq!(
            mock.write_history(&info)
                .iter()
                .filter(|bytes| bytes.starts_with(&[1, 134]))
                .count(),
            2
        );
        assert!(!mock
            .write_history(&info)
            .iter()
            .any(|bytes| bytes.first() == Some(&2) && matches!(bytes.get(1), Some(10 | 11))));
        server.abort();
        let _ = server.await;
        manager.shutdown().await;
    }
}
