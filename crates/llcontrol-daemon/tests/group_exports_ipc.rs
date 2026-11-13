//! Actual private RPC export contract; no physical HID and no config Apply.
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::GroupSettings,
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, DeviceId,
    FullProfileGroup as Group, GroupArchivePhase as Phase, GroupArchiveReply as Reply,
    GroupArchiveRequest as Request, PROFILE_ARCHIVE_CHUNK_BYTES as CHUNK,
};
use serde_json::json;
use std::{io::Cursor, path::PathBuf, sync::Arc, time::Duration};
async fn rpc(
    client: &IpcClient,
    dev: &DeviceId,
    group: Group,
    request: Request,
) -> llcontrol_ipc::Result<Reply> {
    serde_json::from_value(
        client
            .call(
                "group_archive",
                json!({"dev":dev,"group":group,"request":request}),
            )
            .await?,
    )
    .map_err(|e| llcontrol_ipc::IpcError::InvalidArgument(e.to_string()))
}
#[tokio::test]
async fn full_group_rpc_snapshot_binding_stale_source_idempotent_begin_and_bounded_repeated_reads()
{
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("group-export".into()),
        PathBuf::from("/mock/group-export"),
    );
    let key = settings_key(&info);
    let mut initial = DeviceSettings::default();
    initial.fan.manual_zero = true;
    initial.pump.source = "mcu".into();
    store.write(&key, &initial).unwrap();
    let mock = MockBackend::new();
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
    let server_rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let (a, b) = mem_pair();
    let server =
        tokio::spawn(async move { IpcServer::new(Arc::new(server_rpc)).serve_stream(b).await });
    let client = IpcClient::from_stream(a).await.unwrap();
    let Reply::Snapshot { snapshot } = rpc(&client, &dev, Group::FanAndLighting, Request::Snapshot)
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(snapshot.summary.fan_manual_percent, Some(0));
    assert_eq!(snapshot.summary.pump_manual.as_ref().unwrap().percent, 0);
    let id = "11223344556677889900aabbccddeeff";
    let begin = Request::BeginExport {
        id: id.into(),
        expected: snapshot.expected.clone(),
    };
    rpc(&client, &dev, Group::FanAndLighting, begin.clone())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let Reply::Transfer { state, .. } = rpc(
                &client,
                &dev,
                Group::FanAndLighting,
                Request::Status { id: id.into() },
            )
            .await
            .unwrap() else {
                panic!()
            };
            if state.phase == Phase::Ready {
                break;
            }
            assert_eq!(state.phase, Phase::Preparing);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    client
        .call("set_fan_pwm", json!({"dev":dev,"percent":67}))
        .await
        .unwrap();
    rpc(&client, &dev, Group::FanAndLighting, begin)
        .await
        .unwrap();
    assert!(rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::BeginExport {
            id: "22222222222222222222222222222222".into(),
            expected: snapshot.expected.clone()
        }
    )
    .await
    .is_err());
    assert!(rpc(
        &client,
        &dev,
        Group::Screen,
        Request::Read {
            id: id.into(),
            offset: 0,
            length: CHUNK
        }
    )
    .await
    .is_err());
    assert!(rpc(
        &client,
        &DeviceId::new("other-device"),
        Group::FanAndLighting,
        Request::Status { id: id.into() }
    )
    .await
    .is_err());
    assert!(rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Read {
            id: id.into(),
            offset: 0,
            length: CHUNK + 1
        }
    )
    .await
    .is_err());
    let Reply::Transfer {
        state,
        bytes,
        offset,
        eof,
    } = rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Read {
            id: id.into(),
            offset: 0,
            length: CHUNK,
        },
    )
    .await
    .unwrap()
    else {
        panic!()
    };
    assert_eq!(offset, Some(0));
    assert!(eof);
    assert_eq!(state.snapshot, snapshot);
    let Reply::Transfer { bytes: again, .. } = rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Read {
            id: id.into(),
            offset: 0,
            length: CHUNK,
        },
    )
    .await
    .unwrap() else {
        panic!()
    };
    assert_eq!(again, bytes);
    let archive = ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let p = archive.prepare_group(&mut Cursor::new(&bytes)).unwrap();
    let GroupSettings::FanAndLighting(group) = &p.profile().content else {
        panic!()
    };
    assert!(group.fan.manual_zero);
    assert_eq!(
        group.fan.pwm_percent, 0,
        "immutable export retains captured state before later edit"
    );
    assert!(rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Read {
            id: id.into(),
            offset: bytes.len() as u64 + 1,
            length: 1
        }
    )
    .await
    .is_err());
    rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Release { id: id.into() },
    )
    .await
    .unwrap();
    assert!(rpc(
        &client,
        &dev,
        Group::FanAndLighting,
        Request::Status { id: id.into() }
    )
    .await
    .is_err());
    assert!(store.read(&key).unwrap().group_imports.is_empty());
    assert_eq!(
        mock.write_history(&info)
            .iter()
            .filter(|p| matches!(p.get(1), Some(138 | 139)))
            .count(),
        3,
        "snapshot/export/read/release must not send PWM"
    );
    drop(client);
    server.await.unwrap().unwrap();
    manager.shutdown().await;
}
