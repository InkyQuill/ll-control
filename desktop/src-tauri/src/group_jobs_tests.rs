use super::*;
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::PersistenceStore,
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::server::IpcServer;
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
                "../../../crates/llcontrol-image/assets/NotoSansTC-Regular.otf"
            ))
            .unwrap();
        let theme:llcontrol_ipc::LcdTheme=serde_json::from_value(json!({"version":1,"name":"Native full-group source","background":[12,31,60],"layers":[{"kind":"text","text":"温度","style":{"font_asset":font,"x":20,"y":20,"size":36,"color":[255,255,255],"align":"left"}}]})).unwrap();
        let mut desired = llcontrol_daemon::persistence::DeviceSettings::default();
        desired.lcd.brightness = 41;
        desired.theme = Some(theme.clone());
        desired.lcd.control_mode = "Application".into();
        desired.fan.manual_zero = true;
        desired.pump.pwm_percent = 55;
        desired.pump.source = "mcu".into();
        store
            .write(
                &llcontrol_daemon::persistence::settings_key(&info),
                &desired,
            )
            .unwrap();
        store
            .profiles()
            .edit(
                0,
                llcontrol_ipc::ProfileEdit::Save {
                    profile: llcontrol_ipc::NamedProfile {
                        slot: 7,
                        name: "Native slot".into(),
                        content: llcontrol_ipc::ProfileContent::Screen(theme),
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
    idle_with_timeout(jobs, Duration::from_secs(30)).await
}

async fn idle_with_timeout(jobs: &Groups, timeout: Duration) -> Status {
    tokio::time::timeout(timeout, async {
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
                llcontrol_ipc::InterfaceLanguage::English
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
    let current: llcontrol_ipc::LcdState = call(&client, "get_lcd_state", json!({"dev":h.dev}))
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
        llcontrol_ipc::InterfaceLanguage::English,
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
        .any(|p| p.content.group() == llcontrol_ipc::ProfileGroup::Screen && p.slot == 7));
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
        llcontrol_ipc::InterfaceLanguage::English,
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
        llcontrol_ipc::InterfaceLanguage::English,
    )
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    h.store
        .profiles()
        .edit(
            before.revision,
            llcontrol_ipc::ProfileEdit::Rename {
                group: llcontrol_ipc::ProfileGroup::Screen,
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
async fn control_explicit_retry_after_real_unix_reply_loss_reads_receipt_without_a_second_apply() {
    control_reply_loss_cases(&[Mode::Reset, Mode::Windows], false).await;
}

#[tokio::test]
#[ignore = "waits for the real 60-second group worker deadline; run for release qualification"]
async fn control_reset_deadline_reads_receipt_without_a_second_apply() {
    control_reply_loss_cases(&[Mode::Reset], true).await;
}

async fn control_reply_loss_cases(modes: &[Mode], withhold: bool) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    for &mode in modes {
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
                                // Lose the downstream ACK; the deadline case holds the socket open.
                                if matches!(method,"reset_fan_and_lighting"|"import_windows_control")&&result.is_ok()&&lose.swap(false,Ordering::SeqCst){
                                    if withhold { tokio::time::sleep(Duration::from_secs(65)).await; }
                                    break;
                                }
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
        let mut request = h.request(84, mode, Group::FanAndLighting);
        if mode == Mode::Reset {
            request.sensor_id = Some("cpu:test".into());
            jobs.start(
                proxy_path.clone(),
                request.clone(),
                llcontrol_ipc::InterfaceLanguage::English,
            )
            .unwrap();
        } else {
            let path = windows_file(root.path());
            jobs.start_with(proxy_path.clone(), request.clone(), move |_| async move {
                Ok(Some(path))
            })
            .unwrap();
            assert_eq!(idle(&jobs).await.phase, Phase::Bindings);
            jobs.prepare_windows(proxy_path.clone(), request.clone(), windows_options())
                .unwrap();
        }
        assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
        let started = std::time::Instant::now();
        jobs.action(proxy_path.clone(), request.clone(), Action::Finish, None)
            .unwrap();
        let lost = idle_with_timeout(&jobs, Duration::from_secs(70)).await;
        if withhold {
            assert!(started.elapsed() >= Duration::from_secs(59));
            assert_eq!(
                lost.message.as_deref(),
                Some("Group request timed out; read operation ID")
            );
        }
        assert_eq!(lost.phase, Phase::Uncertain);
        if mode == Mode::Windows {
            assert!(jobs
                .prepare_windows(proxy_path.clone(), request.clone(), windows_options())
                .is_err());
            std::fs::write(
                root.path().join("windows.zip"),
                b"must not be reread during retry",
            )
            .unwrap();
        }
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
}

fn windows_file(root: &Path) -> PathBuf {
    let file = root.join("windows.zip");
    std::fs::write(
        &file,
        include_bytes!("../../../crates/llcontrol-profile/tests/fixtures/Control.zip"),
    )
    .unwrap();
    file
}
fn windows_options() -> WindowsOptions {
    let mut bindings = serde_json::Map::new();
    for (role, unit) in [
        ("CPULoad", "percent"),
        ("CPUTemperature", "celsius"),
        ("GPULoad", "percent"),
        ("GPUTemperature", "celsius"),
        ("PumpRPM", "rpm"),
        ("CoolantTemp", "celsius"),
    ] {
        bindings.insert(
            role.into(),
            json!({"sensor_id":format!("fixture:{role}"),"unit":unit}),
        );
    }
    serde_json::from_value(json!({"record":1,"bindings":bindings})).unwrap()
}
#[tokio::test]
async fn windows_file_grant_selection_preparation_and_finish_use_one_native_owned_draft() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = h.request(100, Mode::Windows, Group::FanAndLighting);
    let before = h.store.profiles().list().unwrap();
    let path = windows_file(root.path());
    let choice = path.clone();
    jobs.start_with(h.socket.clone(), request.clone(), move |_| async move {
        Ok(Some(choice))
    })
    .unwrap();
    let source = idle(&jobs).await;
    assert_eq!(source.phase, Phase::Bindings);
    assert_eq!(
        source.windows_source.unwrap().controllers[0].record_index,
        1
    );
    assert!(jobs
        .action(h.socket.clone(), request.clone(), Action::Finish, None)
        .is_err());
    // A pathname replacement cannot substitute a different file for the grant.
    std::fs::rename(&path, root.path().join("original.zip")).unwrap();
    std::fs::write(&path, b"not the chosen archive").unwrap();
    jobs.prepare_windows(h.socket.clone(), request.clone(), windows_options())
        .unwrap();
    let prepared = idle(&jobs).await;
    assert_eq!(prepared.phase, Phase::Prepared, "{prepared:?}");
    assert_eq!(
        prepared.windows_lighting.as_ref().unwrap().pump_modes.len(),
        16
    );
    assert_eq!(prepared.summary.as_ref().unwrap().slots.len(), 0);
    assert_eq!(h.store.profiles().list().unwrap(), before);
    // Finish must not reread even the original descriptor after preparation.
    std::fs::write(root.path().join("original.zip"), b"changed after preview").unwrap();
    jobs.action(h.socket.clone(), request.clone(), Action::Finish, None)
        .unwrap();
    jobs.shutdown().await.unwrap();
    let done = jobs.status().unwrap().unwrap();
    assert_eq!(done.phase, Phase::Complete, "{done:?}");
    assert!(done.readback.unwrap().selected_content_current);
    assert!(jobs.lock().unwrap().windows.is_none());
    let after = h.store.profiles().list().unwrap();
    assert_eq!(
        after.profiles, before.profiles,
        "Linux extension slots are retained"
    );
    assert_eq!(after.revision, before.revision + 1);
    let recovered = Groups::default();
    recovered
        .action(h.socket.clone(), request.clone(), Action::Recover, None)
        .unwrap();
    assert_eq!(idle(&recovered).await.phase, Phase::Complete);
    assert!(recovered
        .prepare_windows(h.socket.clone(), request, windows_options())
        .is_err());
    recovered.shutdown().await.unwrap();
    h.stop().await;
}
#[tokio::test]
async fn windows_changed_source_bad_record_and_missing_roles_fail_before_apply_and_can_be_reprepared(
) {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = h.request(101, Mode::Windows, Group::FanAndLighting);
    let path = windows_file(root.path());
    let original = std::fs::read(&path).unwrap();
    let choice = path.clone();
    let before = h.store.profiles().list().unwrap();
    jobs.start_with(h.socket.clone(), request.clone(), move |_| async move {
        Ok(Some(choice))
    })
    .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Bindings);
    let mut missing = windows_options();
    missing.bindings.remove("CoolantTemp");
    jobs.prepare_windows(h.socket.clone(), request.clone(), missing)
        .unwrap();
    let failed = idle(&jobs).await;
    assert_eq!(failed.phase, Phase::Failed);
    assert_eq!(
        failed.message_code,
        Some(ArchiveNotice::WindowsPreparationFailed)
    );
    assert!(jobs
        .lock()
        .unwrap()
        .windows
        .as_ref()
        .unwrap()
        .draft
        .lock()
        .unwrap()
        .is_none());
    let mut wrong = windows_options();
    wrong.record = 0;
    jobs.prepare_windows(h.socket.clone(), request.clone(), wrong)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Failed);
    std::fs::write(&path, b"corrupt source").unwrap();
    jobs.prepare_windows(h.socket.clone(), request.clone(), windows_options())
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Failed);
    assert_eq!(h.store.profiles().list().unwrap(), before);
    std::fs::write(&path, original).unwrap();
    jobs.prepare_windows(h.socket.clone(), request.clone(), windows_options())
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    let mut stale = request.clone();
    stale.dev = DeviceId::new("other:device");
    assert!(jobs
        .prepare_windows(h.socket.clone(), stale, windows_options())
        .is_err());
    jobs.action(h.socket.clone(), request, Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    assert!(jobs.lock().unwrap().windows.is_none());
    assert_eq!(h.store.profiles().list().unwrap(), before);
    jobs.shutdown().await.unwrap();
    h.stop().await;
}
#[tokio::test]
async fn windows_prepared_close_and_offline_cancel_release_grant_without_daemon_transfer_cleanup() {
    for cancel in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let h = Harness::start(root.path()).await;
        let jobs = Groups::default();
        let request = h.request(102, Mode::Windows, Group::FanAndLighting);
        let path = windows_file(root.path());
        let before = h.store.profiles().list().unwrap();
        jobs.start_with(h.socket.clone(), request.clone(), move |_| async move {
            Ok(Some(path))
        })
        .unwrap();
        assert_eq!(idle(&jobs).await.phase, Phase::Bindings);
        jobs.prepare_windows(h.socket.clone(), request.clone(), windows_options())
            .unwrap();
        assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
        let socket = h.socket.clone();
        let store = h.store.clone();
        h.stop().await;
        if cancel {
            jobs.action(socket, request, Action::Cancel, None).unwrap();
            assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
        }
        jobs.shutdown().await.unwrap();
        assert!(jobs.lock().unwrap().windows.is_none());
        assert_eq!(store.profiles().list().unwrap(), before);
    }
}

#[path = "../../../crates/llcontrol-profile/tests/support/screen_plan.rs"]
mod screen_fixture;
fn screen_options() -> llcontrol_ipc::WindowsScreenSource {
    llcontrol_ipc::WindowsScreenSource {
        record: 0,
        bindings_json: screen_fixture::bindings().to_string(),
        resources_json: screen_fixture::mapping().to_string(),
    }
}
async fn choose_screen(h: &Harness, root: &Path, jobs: &Groups, index: u8) -> Start {
    let request = h.request(index, Mode::WindowsScreen, Group::Screen);
    let path = root.join(format!("screen-{index}.zip"));
    std::fs::write(&path, screen_fixture::archive(|_| {}, &[])).unwrap();
    let choice = path.clone();
    jobs.start_with(h.socket.clone(), request.clone(), move |_| async move {
        Ok(Some(choice))
    })
    .unwrap();
    let status = idle(jobs).await;
    assert_eq!(status.phase, Phase::Bindings, "{status:?}");
    // The selected Screen source is an immutable private snapshot, even if the
    // original regular file is overwritten in place rather than renamed.
    std::fs::write(path, b"changed original after selection").unwrap();
    request
}
#[tokio::test]
async fn windows_screen_private_selection_preview_finish_and_recovery_preserve_editor_and_library()
{
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = choose_screen(&h, root.path(), &jobs, 200).await;
    let before = h.store.profiles().list().unwrap();
    let catalog = h.store.theme_assets().list().unwrap();
    jobs.prepare_windows_request(
        h.socket.clone(),
        request.clone(),
        WindowsPreparation::Screen(screen_options()),
    )
    .unwrap();
    let prepared = idle(&jobs).await;
    assert_eq!(prepared.phase, Phase::Prepared, "{prepared:?}");
    assert_eq!(h.store.profiles().list().unwrap(), before);
    assert_eq!(h.store.theme_assets().list().unwrap(), catalog);
    assert!(
        jobs.prepare_windows_screen(h.socket.clone(), request.clone(), screen_options())
            .is_err(),
        "prepared options cannot change"
    );
    jobs.action(h.socket.clone(), request.clone(), Action::Finish, None)
        .unwrap();
    jobs.shutdown().await.unwrap();
    let done = jobs.status().unwrap().unwrap();
    assert_eq!(done.phase, Phase::Complete, "{done:?}");
    assert!(done.readback.unwrap().selected_content_current);
    assert_eq!(h.store.profiles().list().unwrap().profiles, before.profiles);
    let client = IpcClient::connect(&h.socket).await.unwrap();
    let imported = client
        .call("get_imported_lcd_editor", json!({"dev":h.dev}))
        .await
        .unwrap();
    let active = client
        .call("get_lcd_theme", json!({"dev":h.dev}))
        .await
        .unwrap();
    assert!(!imported.is_null());
    assert_ne!(active, imported);
    let recovered = Groups::default();
    recovered
        .action(h.socket.clone(), request, Action::Recover, None)
        .unwrap();
    assert_eq!(idle(&recovered).await.phase, Phase::Complete);
    recovered.shutdown().await.unwrap();
    drop(client);
    h.stop().await;
}
#[tokio::test]
async fn windows_screen_native_cancel_and_shutdown_clear_remote_private_preparation() {
    for shutdown in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let h = Harness::start(root.path()).await;
        let jobs = Groups::default();
        let request = choose_screen(&h, root.path(), &jobs, 201).await;
        let catalog = h.store.theme_assets().list().unwrap();
        jobs.prepare_windows_screen(h.socket.clone(), request.clone(), screen_options())
            .unwrap();
        assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
        if !shutdown {
            jobs.action(h.socket.clone(), request.clone(), Action::Cancel, None)
                .unwrap();
            assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
        }
        jobs.shutdown().await.unwrap();
        let client = IpcClient::connect(&h.socket).await.unwrap();
        assert!(
            upload_rpc(
                &client,
                &request,
                U::Status {
                    id: request.id.clone()
                }
            )
            .await
            .is_err(),
            "remote session released"
        );
        assert_eq!(h.store.theme_assets().list().unwrap(), catalog);
        assert!(client
            .call("get_imported_lcd_editor", json!({"dev":h.dev}))
            .await
            .unwrap()
            .is_null());
        drop(client);
        h.stop().await;
    }
}
#[tokio::test]
async fn windows_screen_cancel_before_preparation_only_drops_local_grant() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = choose_screen(&h, root.path(), &jobs, 202).await;
    jobs.action(h.socket.clone(), request, Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    assert!(jobs.lock().unwrap().windows.is_none());
    jobs.shutdown().await.unwrap();
    h.stop().await;
}

#[test]
fn windows_prepare_bridge_decodes_existing_control_and_new_screen_options_without_ambiguity() {
    let control: WindowsPreparation =
        serde_json::from_value(serde_json::to_value(windows_options()).unwrap()).unwrap();
    assert!(matches!(control, WindowsPreparation::Control(_)));
    let screen: WindowsPreparation =
        serde_json::from_value(serde_json::to_value(screen_options()).unwrap()).unwrap();
    assert!(matches!(screen, WindowsPreparation::Screen(_)));
    let mut ambiguous = serde_json::to_value(screen_options()).unwrap();
    ambiguous["bindings"] = json!({});
    assert!(serde_json::from_value::<WindowsPreparation>(ambiguous).is_err());
}
#[tokio::test]
async fn windows_screen_missing_source_record_fails_without_apply_and_requires_a_new_id() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = choose_screen(&h, root.path(), &jobs, 203).await;
    let before = h.store.profiles().list().unwrap();
    let catalog = h.store.theme_assets().list().unwrap();
    let mut options = screen_options();
    options.record = 999;
    jobs.prepare_windows_screen(h.socket.clone(), request.clone(), options)
        .unwrap();
    let failed = idle(&jobs).await;
    assert_eq!(failed.phase, Phase::Failed, "{failed:?}");
    assert_eq!(h.store.profiles().list().unwrap(), before);
    assert_eq!(h.store.theme_assets().list().unwrap(), catalog);
    assert!(jobs
        .action(h.socket.clone(), request.clone(), Action::Finish, None)
        .is_err());
    assert!(jobs
        .prepare_windows_screen(h.socket.clone(), request, screen_options())
        .is_err());
    let fresh = choose_screen(&h, root.path(), &jobs, 204).await;
    jobs.action(h.socket.clone(), fresh, Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    jobs.shutdown().await.unwrap();
    h.stop().await;
}

#[tokio::test]
async fn windows_screen_requirements_use_the_owned_source_and_do_not_publish_or_prepare_apply() {
    let root = tempfile::tempdir().unwrap();
    let h = Harness::start(root.path()).await;
    let jobs = Groups::default();
    let request = choose_screen(&h, root.path(), &jobs, 205).await;
    let before = h.store.profiles().list().unwrap();
    let catalog = h.store.theme_assets().list().unwrap();
    let options: WindowsPreparation =
        serde_json::from_value(json!({"record":0,"inspect_only":true})).unwrap();
    jobs.prepare_windows_request(h.socket.clone(), request.clone(), options)
        .unwrap();
    let status = idle(&jobs).await;
    assert_eq!(status.phase, Phase::Bindings, "{status:?}");
    let required = status.windows_screen_requirements.unwrap();
    assert_eq!(required.record_index, 0);
    assert_eq!(required.bindings.sensor_roles.len(), 6);
    assert_eq!(required.bindings.resources.len(), 2);
    assert!(required.deferred.is_empty());
    assert!(!required.payloads_decoded && !required.apply_supported);
    assert!(status.snapshot.is_none());
    assert!(jobs
        .action(h.socket.clone(), request.clone(), Action::Finish, None)
        .is_err());
    assert_eq!(h.store.profiles().list().unwrap(), before);
    assert_eq!(h.store.theme_assets().list().unwrap(), catalog);
    let wrong = Start {
        dev: DeviceId::new("foreign"),
        ..request.clone()
    };
    assert!(jobs
        .inspect_windows_screen(h.socket.clone(), wrong, 0)
        .is_err());
    jobs.prepare_windows_screen(h.socket.clone(), request.clone(), screen_options())
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Prepared);
    jobs.action(h.socket.clone(), request, Action::Cancel, None)
        .unwrap();
    assert_eq!(idle(&jobs).await.phase, Phase::Cancelled);
    jobs.shutdown().await.unwrap();
    h.stop().await;
}
