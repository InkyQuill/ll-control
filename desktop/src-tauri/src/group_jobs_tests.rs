use super::*;
use lconnect_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::PersistenceStore,
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::server::IpcServer;
struct Harness {
    manager: ManagerHandle,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
    socket: PathBuf,
    dev: DeviceId,
    store: Arc<PersistenceStore>,
}
impl Harness {
    async fn start(root: &Path) -> Self {
        let store = Arc::new(PersistenceStore::new_with_root(root.join("settings")));
        let mock = MockBackend::new();
        let info = DeviceInfo::new(
            DeviceFamilyId::new("galahad2-lcdv2"),
            0x0416,
            0x7395,
            Some("native-group".into()),
            PathBuf::from("/mock/native-group"),
        );
        let font = store
            .theme_fonts()
            .import(include_bytes!(
                "../../../crates/lconnect-image/assets/NotoSansTC-Regular.otf"
            ))
            .unwrap();
        let theme:lconnect_ipc::LcdTheme=serde_json::from_value(json!({"version":1,"name":"Native full-group source","background":[12,31,60],"layers":[{"kind":"text","text":"温度","style":{"font_asset":font,"x":20,"y":20,"size":36,"color":[255,255,255],"align":"left"}}]})).unwrap();
        let mut desired = lconnect_daemon::persistence::DeviceSettings::default();
        desired.lcd.brightness = 41;
        desired.theme = Some(theme.clone());
        desired.lcd.control_mode = "Application".into();
        desired.fan.manual_zero = true;
        desired.pump.pwm_percent = 55;
        desired.pump.source = "mcu".into();
        store
            .write(&lconnect_daemon::persistence::settings_key(&info), &desired)
            .unwrap();
        store
            .profiles()
            .edit(
                0,
                lconnect_ipc::ProfileEdit::Save {
                    profile: lconnect_ipc::NamedProfile {
                        slot: 7,
                        name: "Native slot".into(),
                        content: lconnect_ipc::ProfileContent::Screen(theme),
                    },
                },
            )
            .unwrap();
        mock.add_device(info);
        let stats = Arc::new(Stats::new());
        let cfg = Config::default();
        let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
            .spawn()
            .await;
        let dev = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(d) = manager.list().await.first() {
                    break d.id.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let socket = root.join("group.sock");
        let path = socket.clone();
        let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
        let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
        while !socket.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Self {
            manager,
            server,
            socket,
            dev,
            store,
        }
    }
    async fn stop(self) {
        self.manager.shutdown().await;
        self.server.abort();
        let _ = self.server.await;
    }
    fn request(&self, n: u8, mode: Mode, group: Group) -> Start {
        Start {
            id: format!("{n:032x}"),
            dev: self.dev.clone(),
            group,
            mode,
            sensor_id: None,
        }
    }
}
async fn idle(jobs: &Groups) -> Status {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let finished = {
                let state = jobs.lock().unwrap();
                state
                    .job
                    .as_ref()
                    .filter(|j| *j.done.borrow())
                    .map(|j| j.status.clone())
            };
            if let Some(status) = finished {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn full_groups_native_export_inspect_explicit_apply_recover_reapply_and_noclobber() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    for (i, group) in [Group::Screen, Group::FanAndLighting]
        .into_iter()
        .enumerate()
    {
        let jobs = Groups::default();
        let file = root.path().join(format!("group-{i}.lcg"));
        let selected = file.clone();
        jobs.start_with(
            h.socket.clone(),
            h.request((i * 8) as u8, Mode::Export, group),
            move |_| async move { Ok(Some(selected)) },
        )
        .unwrap();
        let exported = idle(&jobs).await;
        assert_eq!(exported.phase, Phase::Complete, "{exported:?}");
        assert_eq!(&std::fs::read(&file).unwrap()[..8], b"LCGROUP1");
        let bytes = std::fs::read(&file).unwrap();
        let selected = file.clone();
        jobs.start_with(
            h.socket.clone(),
            h.request((i * 8 + 1) as u8, Mode::Export, group),
            move |_| async move { Ok(Some(selected)) },
        )
        .unwrap();
        assert_ne!(idle(&jobs).await.phase, Phase::Complete);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        // Failed export has no effect on device settings. A separate native owner
        // models reopening the desktop window with a durable file grant.
        let jobs = Groups::default();
        let r = h.request((i * 8 + 2) as u8, Mode::Import, group);
        let selected = file.clone();
        jobs.start_with(h.socket.clone(), r.clone(), move |_| async move {
            Ok(Some(selected))
        })
        .unwrap();
        let before = h.store.profiles().list().unwrap();
        let prepared = idle(&jobs).await;
        assert_eq!(prepared.phase, Phase::Prepared, "{prepared:?}");
        assert!(prepared.readback.is_none());
        assert_eq!(
            serde_json::to_value(h.store.profiles().list().unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        let serialized = serde_json::to_string(&prepared).unwrap();
        assert!(!serialized.contains("\"bytes\""));
        assert!(!serialized.contains(root.path().to_str().unwrap()));
        assert!(jobs
            .start(
                h.socket.clone(),
                h.request(99, Mode::Import, group),
                lconnect_ipc::InterfaceLanguage::English
            )
            .is_err());
        jobs.action(h.socket.clone(), r.clone(), Action::Finish, None)
            .unwrap();
        let applied = idle(&jobs).await;
        assert_eq!(applied.phase, Phase::Complete, "{applied:?}");
        let client = connect(&h.socket).await.unwrap();
        assert!(
            upload_rpc(&client, &r, U::Status { id: r.id.clone() })
                .await
                .is_err(),
            "Known imported session must release its staging quota; receipt remains durable"
        );
        let receipt = applied.readback.unwrap();
        assert!(receipt.selected_content_current);
        jobs.shutdown().await.unwrap();
        let reopened = Groups::default();
        reopened
            .action(h.socket.clone(), r.clone(), Action::Recover, None)
            .unwrap();
        let recovered = idle(&reopened).await;
        assert_eq!(recovered.phase, Phase::Complete, "{recovered:?}");
        assert_eq!(
            recovered.readback.as_ref().unwrap().receipt,
            receipt.receipt
        );
        let expected = recovered.readback.unwrap().runtime.revision;
        reopened
            .action(h.socket.clone(), r.clone(), Action::Reapply, expected)
            .unwrap();
        assert_eq!(idle(&reopened).await.phase, Phase::Complete);
        reopened.shutdown().await.unwrap();
    }
    // Mock-only root, no production daemon, USB or sensor polling fixtures.
    assert!(h.store.theme_assets_root().starts_with(root.path()));
    h.stop().await;
}

#[tokio::test]
async fn prepared_group_cancellation_and_shutdown_release_session_without_owner_commit() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let file = root.path().join("cancel.lcg");
    let export = Groups::default();
    let chosen = file.clone();
    export
        .start_with(
            h.socket.clone(),
            h.request(50, Mode::Export, Group::Screen),
            move |_| async move { Ok(Some(chosen)) },
        )
        .unwrap();
    assert_eq!(idle(&export).await.phase, Phase::Complete);
    let before = serde_json::to_value(h.store.profiles().list().unwrap()).unwrap();
    for shutdown in [false, true] {
        let jobs = Groups::default();
        let r = h.request(if shutdown { 52 } else { 51 }, Mode::Import, Group::Screen);
        let chosen = file.clone();
        jobs.start_with(h.socket.clone(), r.clone(), move |_| async move {
            Ok(Some(chosen))
        })
        .unwrap();
        assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
        if shutdown {
            jobs.shutdown().await.unwrap();
        } else {
            jobs.action(h.socket.clone(), r.clone(), Action::Cancel, None)
                .unwrap();
            assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
        }
        let client = connect(&h.socket).await.unwrap();
        assert!(upload_rpc(&client, &r, U::Status { id: r.id.clone() })
            .await
            .is_err());
        assert!(call::<FullGroupReadback>(
            &client,
            "read_group_import",
            json!({"dev":r.dev,"group":r.group,"operation_id":r.id})
        )
        .await
        .is_err());
        assert_eq!(
            serde_json::to_value(h.store.profiles().list().unwrap()).unwrap(),
            before
        );
    }
    let jobs = Groups::default();
    let r = h.request(53, Mode::Import, Group::Screen);
    jobs.start_with(h.socket.clone(), r.clone(), move |mut cancel| async move {
        cancel.wait_for(|v| *v).await?;
        Ok(None)
    })
    .unwrap();
    jobs.action(h.socket.clone(), r, Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    jobs.shutdown().await.unwrap();
    h.stop().await;
}
#[tokio::test]
async fn stale_inspected_group_is_not_applied_over_a_newer_lcd_intent() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let file = root.path().join("stale.lcg");
    let export = Groups::default();
    let chosen = file.clone();
    export
        .start_with(
            h.socket.clone(),
            h.request(60, Mode::Export, Group::Screen),
            move |_| async move { Ok(Some(chosen)) },
        )
        .unwrap();
    assert_eq!(idle(&export).await.phase, Phase::Complete);
    let jobs = Groups::default();
    let r = h.request(61, Mode::Import, Group::Screen);
    jobs.start_with(h.socket.clone(), r.clone(), move |_| async move {
        Ok(Some(file))
    })
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    let client = connect(&h.socket).await.unwrap();
    let _: serde_json::Value = call(
        &client,
        "patch_lcd_config",
        json!({"dev":h.dev,"patch":{"brightness_percent":73}}),
    )
    .await
    .unwrap();
    jobs.action(h.socket.clone(), r.clone(), Action::Finish, None)
        .unwrap();
    let result = idle(&jobs).await;
    assert_eq!(result.phase, Phase::Failed, "{result:?}");
    assert!(result.readback.is_none());
    let current: lconnect_ipc::LcdState = call(&client, "get_lcd_state", json!({"dev":h.dev}))
        .await
        .unwrap();
    assert_eq!(current.requested.brightness_percent, 73);
    assert!(call::<FullGroupReadback>(
        &client,
        "read_group_import",
        json!({"dev":h.dev,"group":r.group,"operation_id":r.id})
    )
    .await
    .is_err());
    jobs.shutdown().await.unwrap();
    h.stop().await;
}

#[tokio::test]
async fn corrupt_group_inspection_releases_failed_staging_and_cleanup_accepts_terminal_failure() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let bytes = [b"LCGROUP1".as_slice(), &[0u8; 36]].concat();
    let path = root.path().join("corrupt.lcg");
    std::fs::write(&path, &bytes).unwrap();
    let r = h.request(70, Mode::Import, Group::Screen);
    let jobs = Groups::default();
    jobs.start_with(h.socket.clone(), r.clone(), move |_| async move {
        Ok(Some(path))
    })
    .unwrap();
    let failed = idle(&jobs).await;
    assert_eq!(failed.phase, Phase::Failed, "{failed:?}");
    let client = connect(&h.socket).await.unwrap();
    assert!(
        upload_rpc(&client, &r, U::Status { id: r.id.clone() })
            .await
            .is_err(),
        "Failed inspected transfer leaked a session slot"
    );
    // Independently fail a public upload, then qualify cleanup's terminal path.
    let r = h.request(71, Mode::Import, Group::Screen);
    let ER::Snapshot { snapshot } = export_rpc(&client, &r, E::Snapshot).await.unwrap() else {
        panic!()
    };
    upload_rpc(
        &client,
        &r,
        U::Begin {
            id: r.id.clone(),
            size: bytes.len() as u64,
            expected: snapshot.expected,
        },
    )
    .await
    .unwrap();
    upload_rpc(
        &client,
        &r,
        U::Chunk {
            id: r.id.clone(),
            offset: 0,
            bytes,
        },
    )
    .await
    .unwrap();
    let mut s = upload_rpc(&client, &r, U::Inspect { id: r.id.clone() })
        .await
        .unwrap();
    while s.phase == UP::Inspecting {
        tokio::time::sleep(Duration::from_millis(10)).await;
        s = upload_rpc(&client, &r, U::Status { id: r.id.clone() })
            .await
            .unwrap();
    }
    assert_eq!(s.phase, UP::Failed);
    tokio::time::timeout(Duration::from_secs(1), cleanup(&client, &r))
        .await
        .expect("Failed transfer cleanup must not poll Failed forever")
        .unwrap();
    assert!(upload_rpc(&client, &r, U::Status { id: r.id.clone() })
        .await
        .is_err());
    jobs.shutdown().await.unwrap();
    h.stop().await;
}

#[tokio::test]
async fn reset_preview_cancel_finish_shutdown_and_receipt_recovery_are_owned_without_a_file_chooser(
) {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let before = h.store.profiles().list().unwrap();
    let mut request = h.request(80, Mode::Reset, Group::FanAndLighting);
    request.sensor_id = Some("cpu:test".into());
    let jobs = Groups::default();
    jobs.start_with(h.socket.clone(), request.clone(), |_| async {
        anyhow::bail!("Reset must not open a file chooser")
    })
    .unwrap();
    let preview = idle(&jobs).await;
    assert_eq!(preview.phase, Phase::Prepared);
    assert_eq!(preview.summary.as_ref().unwrap().slots.len(), 8);
    assert_eq!(
        preview.summary.as_ref().unwrap().fan_led_mode.as_deref(),
        Some("rainbow")
    );
    assert_eq!(h.store.profiles().list().unwrap(), before);
    jobs.action(h.socket.clone(), request.clone(), Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    assert_eq!(h.store.profiles().list().unwrap(), before);
    jobs.action(h.socket.clone(), request.clone(), Action::Dismiss, None)
        .unwrap();
    request.id = format!("{:032x}", 81);
    jobs.start(
        h.socket.clone(),
        request.clone(),
        lconnect_ipc::InterfaceLanguage::English,
    )
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    jobs.action(h.socket.clone(), request.clone(), Action::Finish, None)
        .unwrap();
    jobs.shutdown().await.unwrap(); // Join admitted Apply; do not cancel it.
    let done = jobs.status().unwrap().unwrap();
    assert_eq!(done.phase, Phase::Complete);
    assert!(done.readback.unwrap().selected_content_current);
    assert_eq!(
        h.store.profiles().list().unwrap().revision,
        before.revision + 1
    );
    assert!(h
        .store
        .profiles()
        .list()
        .unwrap()
        .profiles
        .iter()
        .any(|p| p.content.group() == lconnect_ipc::ProfileGroup::Screen && p.slot == 7));
    let recovered = Groups::default();
    let mut identity = request.clone();
    identity.sensor_id = None;
    recovered
        .action(h.socket.clone(), identity.clone(), Action::Recover, None)
        .unwrap();
    let read = idle(&recovered).await;
    assert_eq!(read.mode, Mode::Reset);
    assert_eq!(read.phase, Phase::Complete);
    let expected = read.readback.unwrap().runtime.revision.unwrap();
    recovered
        .action(h.socket.clone(), identity, Action::Reapply, Some(expected))
        .unwrap();
    assert_eq!(idle(&recovered).await.phase, Phase::Complete);
    assert_eq!(
        h.store.profiles().list().unwrap().revision,
        before.revision + 1
    );
    recovered.shutdown().await.unwrap();
    h.stop().await;
}

#[tokio::test]
async fn prepared_reset_close_and_stale_target_never_claim_an_unconfirmed_reset_was_cancelled() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let before = h.store.profiles().list().unwrap();
    let mut request = h.request(82, Mode::Reset, Group::FanAndLighting);
    request.sensor_id = Some("cpu:test".into());
    let jobs = Groups::default();
    jobs.start(
        h.socket.clone(),
        request.clone(),
        lconnect_ipc::InterfaceLanguage::English,
    )
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    jobs.shutdown().await.unwrap();
    assert_eq!(h.store.profiles().list().unwrap(), before);
    let jobs = Groups::default();
    request.id = format!("{:032x}", 83);
    jobs.start(
        h.socket.clone(),
        request.clone(),
        lconnect_ipc::InterfaceLanguage::English,
    )
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    h.store
        .profiles()
        .edit(
            before.revision,
            lconnect_ipc::ProfileEdit::Rename {
                group: lconnect_ipc::ProfileGroup::Screen,
                slot: 7,
                name: "New screen name".into(),
            },
        )
        .unwrap();
    let newer = h.store.profiles().list().unwrap();
    jobs.action(h.socket.clone(), request.clone(), Action::Finish, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Uncertain);
    assert!(jobs
        .action(h.socket.clone(), request.clone(), Action::Cancel, None)
        .is_err());
    jobs.action(h.socket.clone(), request.clone(), Action::Recover, None)
        .unwrap();
    let unknown = idle(&jobs).await;
    assert_eq!(unknown.phase, Phase::Uncertain);
    assert!(unknown.message.unwrap().contains("No reset was replayed"));
    assert_eq!(h.store.profiles().list().unwrap(), newer);
    jobs.shutdown().await.unwrap();
    h.stop().await;
}

#[tokio::test]
async fn reset_explicit_retry_after_real_unix_reply_loss_reads_receipt_without_a_second_apply() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let before = h.store.profiles().list().unwrap();
    let proxy_path = root.path().join("reply-loss.sock");
    let listener = tokio::net::UnixListener::bind(&proxy_path).unwrap();
    let upstream = h.socket.clone();
    let lose = Arc::new(AtomicBool::new(true));
    let proxy = tokio::spawn(async move {
        let mut workers = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                connection=listener.accept()=>{
                    let (connection,_)=connection.unwrap();let socket=upstream.clone();let lose=lose.clone();
                    workers.spawn(async move {
                        let real=IpcClient::connect(&socket).await.unwrap();
                        let (input,mut output)=connection.into_split();
                        output.write_all(format!("{}\n",serde_json::to_string(&real.hello).unwrap()).as_bytes()).await.unwrap();
                        let mut lines=BufReader::new(input).lines();
                        while let Some(line)=lines.next_line().await.unwrap() {
                            let request:serde_json::Value=serde_json::from_str(&line).unwrap();
                            let method=request["method"].as_str().unwrap();
                            let result=real.call(method,request["params"].clone()).await;
                            // The production daemon has already completed the reset.
                            // Drop only the downstream connection before its ACK.
                            if method=="reset_fan_and_lighting"&&result.is_ok()&&lose.swap(false,Ordering::SeqCst){break;}
                            let response=match result {
                                Ok(value)=>json!({"jsonrpc":"2.0","id":request["id"],"result":value}),
                                Err(e)=>json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":e.jsonrpc_code(),"message":e.to_string()}}),
                            };
                            if output.write_all(format!("{response}\n").as_bytes()).await.is_err(){break;}
                        }
                    });
                },
                _=workers.join_next(),if !workers.is_empty()=>{},
            }
        }
    });
    let jobs = Groups::default();
    let mut request = h.request(84, Mode::Reset, Group::FanAndLighting);
    request.sensor_id = Some("cpu:test".into());
    jobs.start(
        proxy_path.clone(),
        request.clone(),
        lconnect_ipc::InterfaceLanguage::English,
    )
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    jobs.action(proxy_path.clone(), request.clone(), Action::Finish, None)
        .unwrap();
    let lost = idle(&jobs).await;
    assert_eq!(lost.phase, Phase::Uncertain);
    let committed = h.store.profiles().list().unwrap();
    assert_eq!(committed.revision, before.revision + 1);
    let real = IpcClient::connect(&h.socket).await.unwrap();
    let read: FullGroupReadback = call(
        &real,
        "read_group_import",
        json!({"dev":request.dev,"group":request.group,"operation_id":request.id}),
    )
    .await
    .unwrap();
    jobs.action(proxy_path.clone(), request, Action::Finish, None)
        .unwrap();
    let retry = idle(&jobs).await;
    assert_eq!(retry.phase, Phase::Complete);
    assert_eq!(
        retry.readback.unwrap().runtime.revision,
        read.runtime.revision
    );
    assert_eq!(h.store.profiles().list().unwrap(), committed);
    jobs.shutdown().await.unwrap();
    drop(real);
    proxy.abort();
    let _ = proxy.await;
    h.stop().await;
}
