//! End-to-end full-group commit/readback via the production RPC/manager path.
use llcontrol_daemon::{
    config::Config,
    ipc_server::IpcServerImpl,
    manager::{DeviceManager, ManagerHandle},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::{GroupKind, GroupProfile},
    stats::Stats,
};
#[path = "support/display_firmware.rs"]
mod display_firmware;
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use llcontrol_ipc::{
    client::IpcClient, server::IpcServer, DeviceId, FullGroupReadback, FullProfileGroup as G,
    GroupArchiveReply, GroupUploadPhase as P, GroupUploadRequest as U, GroupUploadState as S,
};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
struct Harness {
    manager: ManagerHandle,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
    socket: PathBuf,
    dev: DeviceId,
}
impl Harness {
    async fn start(runtime: &Path, store: Arc<PersistenceStore>, mock: MockBackend) -> Self {
        std::fs::create_dir_all(runtime).unwrap();
        let socket = runtime.join("llcontrold.sock");
        let cfg = Config::default();
        let stats = Arc::new(Stats::new());
        let manager = DeviceManager::new(
            Arc::new(display_firmware::FirmwareMock(mock)),
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
        let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store);
        let path = socket.clone();
        let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
        tokio::time::timeout(Duration::from_secs(3), async {
            while !socket.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        Self {
            manager,
            server,
            socket,
            dev,
        }
    }
    async fn client(&self) -> IpcClient {
        IpcClient::connect(&self.socket).await.unwrap()
    }
    async fn stop(self) {
        self.server.abort();
        let _ = self.server.await;
        self.manager.shutdown().await;
    }
}
async fn upload(c: &IpcClient, d: &DeviceId, g: G, u: U) -> S {
    serde_json::from_value(
        c.call("group_upload", json!({"dev":d,"group":g,"request":u}))
            .await
            .unwrap(),
    )
    .unwrap()
}
async fn wait(c: &IpcClient, d: &DeviceId, g: G, id: &str, wanted: P) -> S {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let state = upload(c, d, g, U::Status { id: id.into() }).await;
            if state.phase == wanted {
                return state;
            }
            assert!(
                !matches!(state.phase, P::Failed | P::RecoveryRequired | P::Cancelled),
                "{state:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
async fn prepare(c: &IpcClient, d: &DeviceId, g: G, id: &str, bytes: &[u8]) {
    let snap: GroupArchiveReply = serde_json::from_value(
        c.call(
            "group_archive",
            json!({"dev":d,"group":g,"request":{"action":"snapshot"}}),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let GroupArchiveReply::Snapshot { snapshot } = snap else {
        panic!()
    };
    upload(
        c,
        d,
        g,
        U::Begin {
            id: id.into(),
            size: bytes.len() as u64,
            expected: snapshot.expected,
        },
    )
    .await;
    for (i, chunk) in bytes
        .chunks(llcontrol_ipc::PROFILE_ARCHIVE_CHUNK_BYTES)
        .enumerate()
    {
        upload(
            c,
            d,
            g,
            U::Chunk {
                id: id.into(),
                offset: (i * llcontrol_ipc::PROFILE_ARCHIVE_CHUNK_BYTES) as u64,
                bytes: chunk.to_vec(),
            },
        )
        .await;
    }
    upload(c, d, g, U::Inspect { id: id.into() }).await;
    wait(c, d, g, id, P::Prepared).await;
}
async fn reapply(
    c: &IpcClient,
    d: &DeviceId,
    g: G,
    id: &str,
    expected: &str,
) -> llcontrol_ipc::Result<FullGroupReadback> {
    serde_json::from_value(
        c.call(
            "reapply_group_import",
            json!({"dev":d,"group":g,"operation_id":id,"expected":expected}),
        )
        .await?,
    )
    .map_err(|e| llcontrol_ipc::IpcError::InvalidArgument(e.to_string()))
}
async fn readback(c: &IpcClient, d: &DeviceId, g: G, id: &str) -> FullGroupReadback {
    serde_json::from_value(
        c.call(
            "read_group_import",
            json!({"dev":d,"group":g,"operation_id":id}),
        )
        .await
        .unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn actual_full_group_finish_applies_both_groups_preserves_newer_intent_and_recovers_after_server_restart(
) {
    let root = tempfile::tempdir().unwrap();
    let settings = root.path().join("settings");
    let store = Arc::new(PersistenceStore::new_with_root(settings.clone()));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("finish".into()),
        PathBuf::from("/mock/finish"),
    );
    let key = settings_key(&info);
    let mut target = DeviceSettings::default();
    target.fan.pwm_percent = 38;
    target.pump.pwm_percent = 55;
    target.pump.source = "mcu".into();
    target.lcd.brightness = 41;
    store.write(&key, &target).unwrap();
    let source_root = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_root.path().into());
    let archives = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let mut desired = DeviceSettings::default();
    desired.fan.manual_zero = true;
    desired.pump.source = "application".into();
    desired.lighting.fan = Some(llcontrol_daemon::persistence::LightingState {
        mode: "static".into(),
        brightness: 80,
        speed: 50,
        direction: "forward".into(),
        color: "#ff0000".into(),
        ..Default::default()
    });
    desired.lighting.pump = Some(llcontrol_daemon::persistence::PumpLightingState {
        scope: "all".into(),
        mode: "static".into(),
        brightness: 85,
        speed: 50,
        direction: "forward".into(),
        color: "#0000ff".into(),
        ..Default::default()
    });
    let profile = GroupProfile::capture(&desired, &Default::default(), GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let mut fan_bytes = Vec::new();
    archives.export_group(&profile, &mut fan_bytes).unwrap();
    desired.lcd.control_mode = "Application".into();
    desired.lcd.rotation = "90".into();
    desired.lcd.brightness = 75;
    desired.theme=Some(serde_json::from_value(json!({"version":1,"name":"Imported sensor screen","background":[12,31,60],"layers":[]})).unwrap());
    let profile = GroupProfile::capture(&desired, &Default::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let mut screen_bytes = Vec::new();
    archives.export_group(&profile, &mut screen_bytes).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info.clone());
    let h = Harness::start(&root.path().join("runtime0"), store.clone(), mock.clone()).await;
    let c = h.client().await;
    let fan_id = "11111111111111111111111111111111";
    prepare(&c, &h.dev, G::FanAndLighting, fan_id, &fan_bytes).await;
    let baseline = mock
        .write_history(&info)
        .iter()
        .filter(|b| matches!(b.get(1), Some(138 | 139)))
        .count();
    upload(
        &c,
        &h.dev,
        G::FanAndLighting,
        U::Finish { id: fan_id.into() },
    )
    .await;
    // Lose the connection after acceptance; owned Finish must survive it.
    drop(c);
    let c = h.client().await;
    let result = wait(&c, &h.dev, G::FanAndLighting, fan_id, P::Imported).await;
    let commit = result.committed.unwrap();
    assert!(!commit.repeated);
    assert!(commit.readback.selected_content_current);
    assert_eq!(commit.readback.summary.fan_manual_percent, Some(0));
    assert!(commit.readback.runtime.last_error.is_none());
    let raw = store.read(&key).unwrap();
    assert_eq!(raw.lcd.brightness, 41);
    assert_eq!(raw.group_imports.len(), 1);
    assert_eq!(raw.lighting.fan.as_ref().unwrap().color, "#ff0000");
    assert_eq!(raw.lighting.pump.as_ref().unwrap().color, "#0000ff");
    assert_eq!(
        commit.readback.summary.fan_led_mode.as_deref(),
        Some("static")
    );
    assert_eq!(
        commit
            .readback
            .runtime
            .thermal
            .as_ref()
            .unwrap()
            .fan
            .applied,
        Some(llcontrol_ipc::ThermalLayer::Base)
    );
    assert_eq!(
        commit
            .readback
            .runtime
            .thermal
            .as_ref()
            .unwrap()
            .pump
            .applied,
        Some(llcontrol_ipc::ThermalLayer::Base)
    );
    assert_eq!(
        mock.write_history(&info)
            .iter()
            .filter(|b| matches!(b.get(1), Some(138 | 139)))
            .count(),
        baseline + 2
    );
    let device_file = settings.join("devices").join(format!("{key}.toml"));
    let profile_file = settings.join("profiles.json");
    let accepted_bytes = (
        std::fs::read(&device_file).unwrap(),
        std::fs::read(&profile_file).unwrap(),
    );
    let fan_token = commit.readback.runtime.revision.clone().unwrap();
    let retried = reapply(&c, &h.dev, G::FanAndLighting, fan_id, &fan_token)
        .await
        .unwrap();
    assert!(retried.selected_content_current);
    assert!(retried.runtime.last_error.is_none());
    assert_eq!(
        accepted_bytes,
        (
            std::fs::read(&device_file).unwrap(),
            std::fs::read(&profile_file).unwrap()
        )
    );
    assert!(reapply(&c, &h.dev, G::FanAndLighting, fan_id, &fan_token)
        .await
        .is_err());
    // Direct is runtime-only. Old token must fail while a fresh explicit retry
    // can restore imported bases without another profile commit.
    let old_token = retried.runtime.revision.unwrap();
    c.call(
        "set_led_colors",
        json!({"dev":h.dev,"target":"fan","colors":[{"r":0,"g":255,"b":0}]}),
    )
    .await
    .unwrap();
    assert!(reapply(&c, &h.dev, G::FanAndLighting, fan_id, &old_token)
        .await
        .is_err());
    let direct = readback(&c, &h.dev, G::FanAndLighting, fan_id).await;
    assert!(direct.selected_content_current);
    let retried = reapply(
        &c,
        &h.dev,
        G::FanAndLighting,
        fan_id,
        direct.runtime.revision.as_deref().unwrap(),
    )
    .await
    .unwrap();
    assert!(retried.runtime.last_error.is_none());
    assert_eq!(
        accepted_bytes,
        (
            std::fs::read(&device_file).unwrap(),
            std::fs::read(&profile_file).unwrap()
        )
    );
    c.call("set_fan_pwm", json!({"dev":h.dev,"percent":67}))
        .await
        .unwrap();
    let before = mock
        .write_history(&info)
        .iter()
        .filter(|b| matches!(b.get(1), Some(138 | 139)))
        .count();
    let repeat = upload(
        &c,
        &h.dev,
        G::FanAndLighting,
        U::Finish { id: fan_id.into() },
    )
    .await
    .committed
    .unwrap();
    assert!(repeat.repeated);
    assert!(!repeat.readback.selected_content_current);
    assert_eq!(repeat.readback.summary.fan_manual_percent, Some(67));
    assert_eq!(
        mock.write_history(&info)
            .iter()
            .filter(|b| matches!(b.get(1), Some(138 | 139)))
            .count(),
        before
    );
    assert!(c
        .call(
            "read_group_import",
            json!({"dev":h.dev,"group":"screen","operation_id":fan_id})
        )
        .await
        .is_err());
    upload(
        &c,
        &h.dev,
        G::FanAndLighting,
        U::Release { id: fan_id.into() },
    )
    .await;
    assert!(
        !readback(&c, &h.dev, G::FanAndLighting, fan_id)
            .await
            .selected_content_current
    );
    let newer = readback(&c, &h.dev, G::FanAndLighting, fan_id).await;
    assert!(reapply(
        &c,
        &h.dev,
        G::FanAndLighting,
        fan_id,
        newer.runtime.revision.as_deref().unwrap()
    )
    .await
    .is_err());
    let screen_id = "22222222222222222222222222222222";
    prepare(&c, &h.dev, G::Screen, screen_id, &screen_bytes).await;
    upload(
        &c,
        &h.dev,
        G::Screen,
        U::Finish {
            id: screen_id.into(),
        },
    )
    .await;
    let screen = wait(&c, &h.dev, G::Screen, screen_id, P::Imported)
        .await
        .committed
        .unwrap();
    assert_eq!(
        screen
            .readback
            .runtime
            .lcd
            .as_ref()
            .unwrap()
            .requested
            .brightness_percent,
        75
    );
    assert!(screen.readback.runtime.last_error.is_none());
    assert_eq!(store.read(&key).unwrap().fan.pwm_percent, 67);
    let screen_token = screen.readback.runtime.revision.clone().unwrap();
    let screen_bytes_before = (
        std::fs::read(&device_file).unwrap(),
        std::fs::read(&profile_file).unwrap(),
    );
    let applied = reapply(&c, &h.dev, G::Screen, screen_id, &screen_token)
        .await
        .unwrap();
    assert!(applied.runtime.last_error.is_none());
    assert_eq!(applied.runtime.recovery_required, Some(false));
    assert_eq!(
        screen_bytes_before,
        (
            std::fs::read(&device_file).unwrap(),
            std::fs::read(&profile_file).unwrap()
        )
    );
    assert!(reapply(&c, &h.dev, G::Screen, screen_id, &screen_token)
        .await
        .is_err());
    assert!(reapply(
        &c,
        &h.dev,
        G::FanAndLighting,
        fan_id,
        applied.runtime.revision.as_deref().unwrap()
    )
    .await
    .is_err());
    let old_token = applied.runtime.revision.unwrap();
    assert!(c
        .call(
            "patch_lcd_config",
            json!({"dev":h.dev,"patch":{"brightness_percent":101}})
        )
        .await
        .is_err());
    assert!(reapply(&c, &h.dev, G::Screen, screen_id, &old_token)
        .await
        .is_err());
    let current = readback(&c, &h.dev, G::Screen, screen_id).await;
    assert!(current.selected_content_current);
    let applied = reapply(
        &c,
        &h.dev,
        G::Screen,
        screen_id,
        current.runtime.revision.as_deref().unwrap(),
    )
    .await
    .unwrap();
    let restart_token = applied.runtime.revision.unwrap();
    let stale_id = "33333333333333333333333333333333";
    prepare(&c, &h.dev, G::Screen, stale_id, &screen_bytes).await;
    c.call(
        "patch_lcd_config",
        json!({"dev":h.dev,"patch":{"brightness_percent":52}}),
    )
    .await
    .unwrap();
    upload(
        &c,
        &h.dev,
        G::Screen,
        U::Finish {
            id: stale_id.into(),
        },
    )
    .await;
    wait(&c, &h.dev, G::Screen, stale_id, P::Failed).await;
    assert_eq!(store.read(&key).unwrap().group_imports.len(), 2);
    let current = readback(&c, &h.dev, G::Screen, screen_id).await;
    assert!(!current.selected_content_current);
    assert_eq!(
        current
            .runtime
            .lcd
            .as_ref()
            .unwrap()
            .requested
            .brightness_percent,
        52
    );
    let dev = h.dev.clone();
    drop(c);
    h.stop().await;
    drop(store);
    let store = Arc::new(PersistenceStore::new_with_root(settings));
    let h = Harness::start(&root.path().join("runtime1"), store.clone(), mock).await;
    assert_eq!(h.dev, dev);
    let c = h.client().await;
    let restored = readback(&c, &h.dev, G::FanAndLighting, fan_id).await;
    assert!(!restored.selected_content_current);
    assert_eq!(restored.summary.fan_manual_percent, Some(67));
    assert!(reapply(&c, &h.dev, G::Screen, screen_id, &restart_token)
        .await
        .is_err());
    assert_eq!(store.read(&key).unwrap().group_imports.len(), 2);
    assert!(c.call("group_upload",json!({"dev":h.dev,"group":"fan_and_lighting","request":{"action":"status","id":fan_id}})).await.is_err());
    drop(c);
    h.stop().await;
}

#[tokio::test]
async fn rejected_screen_finish_removes_only_its_private_publications_and_keeps_legacy_catalog() {
    const FONT: &[u8] = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(
        root.path().join("settings"),
    ));
    let image = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    let font = store.theme_fonts().import(FONT).unwrap();
    let unit = llcontrol_daemon::builtin_media::read_unit(0).unwrap();
    let mut writer = store
        .media_assets()
        .create_file("Existing upload".into())
        .unwrap();
    writer.append(&unit).unwrap();
    let existing_clip = store.media_assets().publish_file(writer).unwrap().key;
    let source_root = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_root.path().into());
    let source_image = source
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[1].png)
        .unwrap();
    let source_font = source.theme_fonts().import(FONT).unwrap();
    let mut writer = source
        .media_assets()
        .create_file("Unreferenced gallery video".into())
        .unwrap();
    writer.append(&unit).unwrap();
    source.media_assets().publish_file(writer).unwrap();
    let mut settings = DeviceSettings::default();
    settings.lcd.control_mode = "Application".into();
    settings.lcd.brightness = 71;
    settings.theme=Some(serde_json::from_value(json!({"version":1,"name":"Rollback candidate","background":[0,0,0],"background_image":{"asset":source_image,"fit":"crop"},"layers":[{"kind":"text","text":"Rollback","style":{"x":20,"y":20,"size":24,"color":[255,255,255],"align":"left","font_asset":source_font}}]})).unwrap());
    let profile = GroupProfile::capture(&settings, &Default::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let archive = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let mut bytes = Vec::new();
    archive.export_group(&profile, &mut bytes).unwrap();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("rollback".into()),
        PathBuf::from("/mock/rollback"),
    );
    let key = settings_key(&info);
    store.write(&key, &DeviceSettings::default()).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info);
    let h = Harness::start(&root.path().join("runtime"), store.clone(), mock).await;
    let c = h.client().await;
    let id = "44444444444444444444444444444444";
    prepare(&c, &h.dev, G::Screen, id, &bytes).await;
    // An unrelated upload arriving during this operation must survive rollback.
    let parallel = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    let images = store.theme_assets().list().unwrap();
    let media: Vec<_> = store
        .media_assets()
        .list()
        .unwrap()
        .into_iter()
        .map(|v| v.key)
        .collect();
    c.call(
        "patch_lcd_config",
        json!({"dev":h.dev,"patch":{"brightness_percent":52}}),
    )
    .await
    .unwrap();
    upload(&c, &h.dev, G::Screen, U::Finish { id: id.into() }).await;
    let rejected = wait(&c, &h.dev, G::Screen, id, P::Failed).await;
    assert!(
        rejected
            .last_error
            .as_deref()
            .is_some_and(|e| e.contains("profile group changed; reload before importing")),
        "{rejected:?}"
    );
    assert_eq!(
        store.theme_assets().list().unwrap(),
        images,
        "a rejected Finish must not add uploads to the legacy catalog"
    );
    assert_eq!(
        store
            .media_assets()
            .list()
            .unwrap()
            .into_iter()
            .map(|v| v.key)
            .collect::<Vec<_>>(),
        media
    );
    assert_eq!(
        std::fs::read_dir(store.theme_fonts_root())
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|s| s == "font"))
            .count(),
        1
    );
    store.theme_assets().read_for_edit(&image).unwrap();
    store.theme_assets().read_for_edit(&parallel).unwrap();
    assert_eq!(store.theme_fonts().load(&font).unwrap(), FONT);
    store.media_assets().acquire_source(&existing_clip).unwrap();
    let latest = store.read(&key).unwrap();
    assert!(latest.group_imports.is_empty());
    assert!(latest.screen_collections.is_none());
    let live: llcontrol_ipc::LcdState =
        serde_json::from_value(c.call("get_lcd_state", json!({"dev":h.dev})).await.unwrap())
            .unwrap();
    assert_eq!(live.requested.brightness_percent, 52);
    drop(c);
    h.stop().await;
    assert_eq!(store.read(&key).unwrap().lcd.brightness, 52);
}
