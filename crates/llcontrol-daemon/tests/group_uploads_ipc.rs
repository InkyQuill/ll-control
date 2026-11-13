//! Public upload/inspection against actual private RPC and settings owner.
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::{GroupKind, GroupProfile},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, transport_memory::mem_pair, DeviceId,
    FullProfileGroup as G, GroupArchiveReply, GroupUploadPhase as P, GroupUploadRequest as U,
    GroupUploadState as S,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};
async fn call(c: &IpcClient, d: &DeviceId, g: G, u: U) -> llcontrol_ipc::Result<S> {
    serde_json::from_value(
        c.call("group_upload", json!({"dev":d,"group":g,"request":u}))
            .await?,
    )
    .map_err(|e| llcontrol_ipc::IpcError::InvalidArgument(e.to_string()))
}
async fn snapshot(c: &IpcClient, d: &DeviceId, g: G) -> llcontrol_ipc::GroupArchiveSnapshot {
    let r: GroupArchiveReply = serde_json::from_value(
        c.call(
            "group_archive",
            json!({"dev":d,"group":g,"request":{"action":"snapshot"}}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let GroupArchiveReply::Snapshot { snapshot } = r else {
        panic!()
    };
    snapshot
}
async fn wait(c: &IpcClient, d: &DeviceId, g: G, id: &str, phase: P) -> S {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let s = call(c, d, g, U::Status { id: id.into() }).await.unwrap();
            if s.phase == phase {
                return s;
            }
            assert!(!matches!(s.phase, P::Failed | P::Cancelled), "{s:?}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn actual_rpc_group_upload_validates_payload_binding_retries_and_cleanup_without_commit_or_pwm(
) {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("upload".into()),
        PathBuf::from("/mock/upload"),
    );
    let key = settings_key(&info);
    let mut initial = DeviceSettings::default();
    initial.fan.manual_zero = true;
    initial.pump.source = "mcu".into();
    store.write(&key, &initial).unwrap();
    let profile = GroupProfile::capture(&initial, &Default::default(), GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let archives = ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let mut bytes = Vec::new();
    archives.export_group(&profile, &mut bytes).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info.clone());
    let cfg = Config::default();
    let stats = Arc::new(Stats::new());
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
    let (a, b) = mem_pair();
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve_stream(b).await });
    let c = IpcClient::from_stream(a).await.unwrap();
    let snap = snapshot(&c, &dev, G::FanAndLighting).await;
    let id = "11111111111111111111111111111111";
    let begin = U::Begin {
        id: id.into(),
        size: bytes.len() as u64,
        expected: snap.expected.clone(),
    };
    let pwm = || {
        mock.write_history(&info)
            .iter()
            .filter(|b| matches!(b.get(1), Some(138 | 139)))
            .count()
    };
    let count = pwm();
    call(&c, &dev, G::FanAndLighting, begin.clone())
        .await
        .unwrap();
    assert!(
        call(&c, &dev, G::FanAndLighting, U::Inspect { id: id.into() })
            .await
            .is_err()
    );
    assert!(call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Chunk {
            id: id.into(),
            offset: 1,
            bytes: bytes.clone()
        }
    )
    .await
    .is_err());
    let chunk = U::Chunk {
        id: id.into(),
        offset: 0,
        bytes: bytes.clone(),
    };
    let first = call(&c, &dev, G::FanAndLighting, chunk.clone())
        .await
        .unwrap();
    assert_eq!(first.received, bytes.len() as u64);
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(call(&c, &dev, G::FanAndLighting, chunk).await.unwrap()).unwrap()
    );
    assert!(call(
        &c,
        &DeviceId::new("other"),
        G::FanAndLighting,
        U::Inspect { id: id.into() }
    )
    .await
    .is_err());
    assert!(call(&c, &dev, G::Screen, U::Inspect { id: id.into() })
        .await
        .is_err());
    call(&c, &dev, G::FanAndLighting, U::Inspect { id: id.into() })
        .await
        .unwrap();
    let prepared = wait(&c, &dev, G::FanAndLighting, id, P::Prepared).await;
    assert_eq!(
        prepared.summary.as_ref().unwrap().fan_manual_percent,
        Some(0)
    );
    assert!(prepared
        .source_sha256
        .as_ref()
        .is_some_and(|s| s.len() == 64));
    assert_eq!(
        serde_json::to_value(prepared).unwrap(),
        serde_json::to_value(call(&c, &dev, G::FanAndLighting, begin).await.unwrap()).unwrap()
    );
    assert_eq!(snap, snapshot(&c, &dev, G::FanAndLighting).await);
    assert_eq!(pwm(), count);
    assert!(store.read(&key).unwrap().group_imports.is_empty());
    call(&c, &dev, G::FanAndLighting, U::Cancel { id: id.into() })
        .await
        .unwrap();
    wait(&c, &dev, G::FanAndLighting, id, P::Cancelled).await;
    // Cleanup tail can still be dropping its active guard; bounded retry.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if call(&c, &dev, G::FanAndLighting, U::Release { id: id.into() })
                .await
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        call(&c, &dev, G::FanAndLighting, U::Status { id: id.into() })
            .await
            .is_err()
    );
    for (id, group, mut damaged) in [
        ("22222222222222222222222222222222", G::Screen, bytes.clone()),
        (
            "33333333333333333333333333333333",
            G::FanAndLighting,
            bytes.clone(),
        ),
    ] {
        if group == G::FanAndLighting {
            let last = damaged.len() - 1;
            damaged[last] ^= 1;
        }
        let target = snapshot(&c, &dev, group).await;
        call(
            &c,
            &dev,
            group,
            U::Begin {
                id: id.into(),
                size: damaged.len() as u64,
                expected: target.expected,
            },
        )
        .await
        .unwrap();
        call(
            &c,
            &dev,
            group,
            U::Chunk {
                id: id.into(),
                offset: 0,
                bytes: damaged,
            },
        )
        .await
        .unwrap();
        // Invalid type/checksum may fail before the initiating RPC reads Status.
        let _ = call(&c, &dev, group, U::Inspect { id: id.into() }).await;
        wait(&c, &dev, group, id, P::Failed).await;
        call(&c, &dev, group, U::Release { id: id.into() })
            .await
            .unwrap();
    }
    let changed_id = "55555555555555555555555555555555";
    call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Begin {
            id: changed_id.into(),
            size: bytes.len() as u64,
            expected: snap.expected.clone(),
        },
    )
    .await
    .unwrap();
    call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Chunk {
            id: changed_id.into(),
            offset: 0,
            bytes: bytes[..32].to_vec(),
        },
    )
    .await
    .unwrap();
    assert!(call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Chunk {
            id: changed_id.into(),
            offset: 0,
            bytes: vec![bytes[0] ^ 1]
        }
    )
    .await
    .is_err());
    let failed = call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Status {
            id: changed_id.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(failed.phase, P::Failed);
    assert!(failed
        .last_error
        .as_ref()
        .is_some_and(|e| e.contains("differs")));
    call(
        &c,
        &dev,
        G::FanAndLighting,
        U::Release {
            id: changed_id.into(),
        },
    )
    .await
    .unwrap();
    // A valid Screen archive with a gallery-only user upload must validate
    // without publishing that upload into the target's shared legacy catalog.
    let source_root = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_root.path().into());
    source
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    let source_archives = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let screen_profile = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::Screen,
    )
    .unwrap()
    .profile;
    let mut screen_bytes = Vec::new();
    source_archives
        .export_group(&screen_profile, &mut screen_bytes)
        .unwrap();
    let inventory = store.theme_assets().list().unwrap();
    let screen_snapshot = snapshot(&c, &dev, G::Screen).await;
    let screen_id = "44444444444444444444444444444444";
    call(
        &c,
        &dev,
        G::Screen,
        U::Begin {
            id: screen_id.into(),
            size: screen_bytes.len() as u64,
            expected: screen_snapshot.expected.clone(),
        },
    )
    .await
    .unwrap();
    for (index, bytes) in screen_bytes
        .chunks(llcontrol_ipc::PROFILE_ARCHIVE_CHUNK_BYTES)
        .enumerate()
    {
        call(
            &c,
            &dev,
            G::Screen,
            U::Chunk {
                id: screen_id.into(),
                offset: (index * llcontrol_ipc::PROFILE_ARCHIVE_CHUNK_BYTES) as u64,
                bytes: bytes.to_vec(),
            },
        )
        .await
        .unwrap();
    }
    call(
        &c,
        &dev,
        G::Screen,
        U::Inspect {
            id: screen_id.into(),
        },
    )
    .await
    .unwrap();
    wait(&c, &dev, G::Screen, screen_id, P::Prepared).await;
    assert_eq!(
        inventory,
        store.theme_assets().list().unwrap(),
        "Inspect must not import gallery uploads"
    );
    assert_eq!(screen_snapshot, snapshot(&c, &dev, G::Screen).await);
    call(
        &c,
        &dev,
        G::Screen,
        U::Cancel {
            id: screen_id.into(),
        },
    )
    .await
    .unwrap();
    wait(&c, &dev, G::Screen, screen_id, P::Cancelled).await;
    call(
        &c,
        &dev,
        G::Screen,
        U::Release {
            id: screen_id.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(pwm(), count);
    assert_eq!(snap, snapshot(&c, &dev, G::FanAndLighting).await);
    drop(c);
    server.await.unwrap().unwrap();
    manager.shutdown().await;
}
