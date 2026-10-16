//! Execute the real CLI through a private Unix socket and validate complete backups.
use lconnect_daemon::{
    builtin_media,
    config::Config,
    ipc_server::IpcServerImpl,
    manager::DeviceManager,
    persistence::{settings_key, DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::GroupSettings,
    stats::Stats,
};
use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
use lconnect_ipc::{server::IpcServer, NamedProfile, ProfileContent, ProfileEdit};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
fn command(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lconnectctl"))
        .args(args)
        .env("XDG_RUNTIME_DIR", root)
        .env_remove("LCONNECTCTL_SOCKET")
        .output()
        .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_cli_full_group_backups_keep_large_gallery_only_payloads_manual_zero_and_do_not_overwrite(
) {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(
        root.path().join("settings"),
    ));
    let mut writer = store
        .media_assets()
        .create_file("Gallery only long video".into())
        .unwrap();
    let first = builtin_media::read_unit(0).unwrap();
    let frames = 1_100_000 / first.bytes.len() + 1;
    for _ in 0..frames {
        writer.append(&first).unwrap();
    }
    let media = store.media_assets().publish_file(writer).unwrap();
    store
        .profiles()
        .edit(
            0,
            ProfileEdit::Save {
                profile: NamedProfile {
                    slot: 2,
                    name: "Portable video".into(),
                    content: ProfileContent::ScreenMedia(media.key),
                },
            },
        )
        .unwrap();
    let image = store
        .theme_assets()
        .import(lconnect_image::defaults::IMAGES[0].png)
        .unwrap();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("group-cli".into()),
        PathBuf::from("/mock/group-cli"),
    );
    let device_key = settings_key(&info);
    let mut settings = DeviceSettings::default();
    settings.fan.manual_zero = true;
    settings.pump.source = "Application".into();
    store.write(&device_key, &settings).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info);
    let stats = Arc::new(Stats::new());
    let mut cfg = Config::default();
    cfg.poll.hotplug_ms = 20;
    let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
        .spawn()
        .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.list().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let socket = root.path().join("lconnectd.sock");
    let server_socket = socket.clone();
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let server =
        tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&server_socket).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !socket.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let out = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "snapshot",
            "fan-and-lighting",
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(snapshot["snapshot"]["summary"]["fan_manual_percent"], 0);
    assert_eq!(
        snapshot["snapshot"]["summary"]["pump_manual"]["source"],
        "application"
    );
    let screen = root.path().join("complete screen.lcg");
    let file = screen.to_str().unwrap();
    let out = command(
        root.path(),
        &["--json", "profiles", "group", "export", "screen", file],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&screen).unwrap();
    assert!(bytes.starts_with(b"LCGROUP1"));
    assert!(bytes.len() > 1_048_576);
    let target_root = tempfile::tempdir().unwrap();
    let target = PersistenceStore::new_with_root(target_root.path().into());
    let archive = ProfileArchives::new(
        target.theme_assets(),
        target.theme_fonts(),
        target.media_assets(),
    );
    let imported = archive.prepare_group(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(imported.uploaded_images().len(), 1);
    assert_eq!(imported.uploaded_media().len(), 1);
    assert!(store
        .theme_assets()
        .list()
        .unwrap()
        .iter()
        .any(|i| i == &image));
    let ProfileContent::ScreenMedia(key) = &imported.profile().profiles[0].content else {
        panic!()
    };
    let mut source = target.media_assets().acquire_source(key).unwrap();
    assert_eq!(source.frames(), frames);
    assert_eq!(source.read_unit(frames - 1).unwrap().bytes, first.bytes);
    let out = command(
        root.path(),
        &["profiles", "group", "export", "screen", file],
    );
    assert!(!out.status.success());
    assert_eq!(std::fs::read(&screen).unwrap(), bytes);
    let fan = root.path().join("fan.lcg");
    let out = command(
        root.path(),
        &[
            "profiles",
            "group",
            "export",
            "fan-and-lighting",
            fan.to_str().unwrap(),
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(&fan).unwrap();
    let imported = archive.prepare_group(&mut Cursor::new(bytes)).unwrap();
    let GroupSettings::FanAndLighting(group) = &imported.profile().content else {
        panic!()
    };
    assert!(group.fan.manual_zero);
    assert_eq!(group.pump.pwm_percent, 0);
    assert_eq!(
        group.pump.source, "application",
        "manager canonicalizes legacy source on replay"
    );
    let before = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "snapshot",
            "fan-and-lighting",
        ],
    );
    let out = command(
        root.path(),
        &["--json", "profiles", "group", "inspect", "screen", file],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(inspected["phase"], "prepared");
    assert_eq!(
        inspected["received"].as_u64(),
        Some(std::fs::metadata(&screen).unwrap().len())
    );
    assert_eq!(inspected["summary"]["slots"][0]["slot"], 2);
    let wrong = command(
        root.path(),
        &["profiles", "group", "inspect", "fan-and-lighting", file],
    );
    assert!(!wrong.status.success());
    let after = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "snapshot",
            "fan-and-lighting",
        ],
    );
    assert_eq!(
        before.stdout, after.stdout,
        "inspection cannot change target settings"
    );
    // Backup does not create an import receipt or apply a restored configuration.
    assert!(store.read(&device_key).unwrap().group_imports.is_empty());
    let applied = command(
        root.path(),
        &["--json", "profiles", "group", "import", "screen", file],
    );
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(applied["phase"], "imported");
    assert_eq!(applied["committed"]["repeated"], false);
    let operation = applied["committed"]["readback"]["operation_id"]
        .as_str()
        .unwrap();
    let read = command(
        root.path(),
        &[
            "--json", "profiles", "group", "readback", "screen", operation,
        ],
    );
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(read["selected_content_current"], true);
    assert_eq!(read["summary"]["slots"][0]["slot"], 2);
    let runtime_token = read["runtime"]["revision"].as_str().unwrap();
    let retry = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "reapply",
            "screen",
            operation,
            "--expected",
            runtime_token,
        ],
    );
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    let retried: serde_json::Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(retried["receipt"], read["receipt"]);
    assert_ne!(retried["runtime"]["revision"], read["runtime"]["revision"]);
    let retry_old = command(
        root.path(),
        &[
            "profiles",
            "group",
            "reapply",
            "screen",
            operation,
            "--expected",
            runtime_token,
        ],
    );
    assert!(!retry_old.status.success());
    let applied = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "import",
            "fan-and-lighting",
            fan.to_str().unwrap(),
        ],
    );
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(applied["phase"], "imported");
    assert_eq!(
        applied["committed"]["readback"]["summary"]["fan_manual_percent"],
        0
    );
    assert_eq!(
        applied["committed"]["readback"]["summary"]["pump_manual"]["percent"],
        0
    );
    let fan_operation = applied["committed"]["readback"]["operation_id"]
        .as_str()
        .unwrap();
    let retry = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "reapply",
            "fan-and-lighting",
            fan_operation,
        ],
    );
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    let retried: serde_json::Value = serde_json::from_slice(&retry.stdout).unwrap();
    assert_eq!(
        retried["receipt"],
        applied["committed"]["readback"]["receipt"]
    );
    assert_eq!(store.read(&device_key).unwrap().group_imports.len(), 2);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_cli_full_control_reset_snapshot_retry_readback_and_reapply_do_not_duplicate_commit()
{
    use lconnect_daemon::sensors::SensorCache;
    use lconnect_ipc::{SensorReading, SensorStatus, SensorUnit};
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(
        root.path().join("settings"),
    ));
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("reset-cli".into()),
        PathBuf::from("/mock/reset-cli"),
    );
    let key = settings_key(&info);
    let mut initial = DeviceSettings::default();
    initial.lcd.brightness = 67;
    initial.lcd.control_mode = "LocalUI".into();
    store.write(&key, &initial).unwrap();
    let mock = MockBackend::new();
    mock.add_device(info);
    let sensors = SensorCache::default();
    let stats = Arc::new(Stats::new());
    let cfg = Config::default();
    let manager = DeviceManager::new(Arc::new(mock), store.clone(), cfg.clone(), stats.clone())
        .with_sensors(sensors.clone())
        .spawn()
        .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while manager.list().await.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    sensors.publish(
        std::time::Instant::now(),
        vec![SensorReading {
            id: "cpu:test".into(),
            label: "CPU test temperature".into(),
            unit: SensorUnit::Celsius,
            value: Some(35.0),
            status: SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: Some(0),
        }],
    );
    let rpc = IpcServerImpl::new(manager.clone(), stats, cfg, store.clone()).with_sensors(sensors);
    let socket = root.path().join("lconnectd.sock");
    let endpoint = socket.clone();
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&endpoint).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        while !socket.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let snapshot = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "snapshot",
            "fan-and-lighting",
        ],
    );
    assert!(
        snapshot.status.success(),
        "{}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&snapshot.stdout).unwrap();
    let expected = snapshot["snapshot"]["expected"].as_str().unwrap();
    let id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
    let args = [
        "--json",
        "profiles",
        "group",
        "reset-fan-and-lighting",
        "--sensor",
        "cpu:test",
        "--id",
        id,
        "--expected",
        expected,
    ];
    let reset = command(root.path(), &args);
    assert!(
        reset.status.success(),
        "{}",
        String::from_utf8_lossy(&reset.stderr)
    );
    assert!(String::from_utf8_lossy(&reset.stderr).contains(expected));
    let result: serde_json::Value = serde_json::from_slice(&reset.stdout).unwrap();
    assert_eq!(result["repeated"], false);
    assert_eq!(
        result["readback"]["summary"]["slots"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        result["readback"]["runtime"]["cooling"]["fan"]["fallback_active"],
        false
    );
    assert_eq!(
        result["readback"]["runtime"]["cooling"]["pump"]["requested"]["mode"],
        "curve_rpm"
    );
    let library = store.profiles().list().unwrap();
    assert_eq!(library.revision, 1);
    let again = command(root.path(), &args);
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&again.stdout).unwrap()["repeated"],
        true
    );
    assert_eq!(store.profiles().list().unwrap(), library);
    let reapply = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "reapply",
            "fan-and-lighting",
            id,
        ],
    );
    assert!(
        reapply.status.success(),
        "{}",
        String::from_utf8_lossy(&reapply.stderr)
    );
    assert_eq!(store.profiles().list().unwrap(), library);
    let readback = command(
        root.path(),
        &[
            "--json",
            "profiles",
            "group",
            "readback",
            "fan-and-lighting",
            id,
        ],
    );
    assert!(readback.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&readback.stdout).unwrap()["operation_id"],
        id
    );
    let stale = command(
        root.path(),
        &[
            "profiles",
            "group",
            "reset-fan-and-lighting",
            "--sensor",
            "cpu:test",
            "--id",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2",
            "--expected",
            expected,
        ],
    );
    assert!(!stale.status.success());
    assert_eq!(store.profiles().list().unwrap(), library);
    assert_eq!(store.read(&key).unwrap().lcd.brightness, 67);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}
