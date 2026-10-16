use super::*;
use crate::{
    persistence::DeviceSettings,
    profile_archive::ProfileArchives,
    profile_group::{GroupProfile, GroupSettings},
};
use sha2::{Digest, Sha256};
use std::{
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const OP: &str = "77777777777777777777777777777777";
#[test]
#[ignore = "subprocess fixture invoked explicitly by SIGKILL tests"]
fn crash_child() {
    let Some(root) = std::env::var_os("LCONNECT_IMPORT_CRASH_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(root.is_absolute() && root.join("fixture-marker").is_file());
    let phase = std::env::var("LCONNECT_IMPORT_CRASH_PHASE").unwrap();
    let device_key =
        std::env::var("LCONNECT_IMPORT_CRASH_DEVICE").unwrap_or_else(|_| "device".into());
    let source = PersistenceStore::new_with_root(root.join("source"));
    let image = source
        .theme_assets()
        .import_named(lconnect_image::defaults::IMAGES[0].png, Some("Owned image"))
        .unwrap();
    let font = source
        .theme_fonts()
        .import(include_bytes!(
            "../../../lconnect-image/assets/DejaVuSans.ttf"
        ))
        .unwrap();
    let mut media = source
        .media_assets()
        .create_file("Owned video".into())
        .unwrap();
    media
        .append(&crate::builtin_media::read_unit(0).unwrap())
        .unwrap();
    let media_key = source.media_assets().publish_file(media).unwrap().key;
    let mut settings=DeviceSettings {theme:Some(serde_json::from_value(serde_json::json!({"version":1,"name":"Crash source","background":[0,0,0],"background_image":{"asset":image,"fit":"crop"},"layers":[{"kind":"text","text":"Owned","style":{"font_asset":font,"x":20,"y":20,"size":24,"color":[255,255,255],"align":"left"}}]})).unwrap()),..Default::default()};
    settings.lcd = crate::persistence::LcdSection {
        brightness: 55,
        rotation: "90".into(),
        control_mode: "Application".into(),
    };
    if phase.starts_with("slot_") {
        slot_child(
            &root,
            &phase,
            &source,
            settings.theme.clone().unwrap(),
            media_key,
        );
    }
    let profile = GroupProfile::capture(&settings, &Default::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let source_archives = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let mut bytes = Vec::new();
    source_archives.export_group(&profile, &mut bytes).unwrap();
    let header = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let store = Arc::new(PersistenceStore::new_with_root(root.clone()));
    let expected = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::Screen,
    )
    .unwrap()
    .revision;
    let binding = Binding {
        device_key: device_key.clone(),
        operation: OP.into(),
        group: GroupKind::Screen,
        expected: expected.clone(),
        source_sha256: format!("{:x}", Sha256::digest(&bytes[44..44 + header])),
    };
    let scope = Publication::begin(store.clone(), binding).unwrap();
    if phase == "candidate" {
        let mut candidate =
            tempfile::NamedTempFile::new_in(scope.staging_directory().unwrap()).unwrap();
        candidate.write_all(b"unregistered private bytes").unwrap();
        candidate.as_file().sync_all().unwrap();
        write_json(
            &root.join("ready.json"),
            &serde_json::json!({"entries":[],"directory":scope.staging_directory()}),
        )
        .unwrap();
        wait_for_kill();
    }
    let archives = ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let prepared = archives
        .prepare_group_scoped(&mut std::io::Cursor::new(bytes), Some(scope.clone()))
        .unwrap();
    if phase != "preparing" {
        scope.arm().unwrap();
    }
    if phase == "discard" {
        scope.journal.as_ref().unwrap().discard().unwrap();
    }
    if phase == "keep" {
        scope.journal.as_ref().unwrap().keep().unwrap();
    }
    if phase == "accepted" {
        store
            .commit_group_import(
                &device_key,
                &DeviceSettings::default(),
                &expected,
                OP,
                &prepared,
            )
            .unwrap();
    }
    let entries = lock(&scope.records)
        .iter()
        .map(|e| serde_json::json!({"kind":e.kind,"key":e.key}))
        .collect::<Vec<_>>();
    let GroupSettings::Screen(screen) = &prepared.profile().content else {
        panic!()
    };
    write_json(&root.join("ready.json"),&serde_json::json!({"entries":entries,"theme":screen.theme,"directory":scope.staging_directory()})).unwrap();
    // SIGKILL from the parent deliberately skips Publication, TempFile, Arc and
    // advisory-lock Drop; recovery must rely exclusively on durable files.
    wait_for_kill();
}
fn slot_child(
    root: &Path,
    phase: &str,
    source: &PersistenceStore,
    mut theme: lconnect_ipc::LcdTheme,
    media: String,
) -> ! {
    let image = theme.background_image.take().unwrap().asset;
    theme.version = 2;
    theme.background_video = Some(media);
    theme.layers.push(serde_json::from_value(serde_json::json!({"kind":"image","asset":image,"x":10,"y":10,"width":100,"height":100,"fit":"contain","opacity":128})).unwrap());
    let profile = lconnect_ipc::NamedProfile {
        slot: 0,
        name: "Crash slot".into(),
        content: lconnect_ipc::ProfileContent::Screen(theme),
    };
    let archives = ProfileArchives::new(
        source.theme_assets(),
        source.theme_fonts(),
        source.media_assets(),
    );
    let mut bytes = Vec::new();
    archives.export(&profile, &mut bytes).unwrap();
    let hash = bytes[12..44]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let receipt = lconnect_ipc::ArchiveImportReceipt {
        id: OP.into(),
        size: bytes.len() as u64,
        expected_revision: 0,
        committed_revision: 1,
        group: lconnect_ipc::ProfileGroup::Screen,
        slot: 7,
        source_sha256: Some(hash.clone()),
    };
    let store = Arc::new(PersistenceStore::new_with_root(root.into()));
    let binding = SlotBinding {
        operation: OP.into(),
        group: receipt.group,
        slot: receipt.slot,
        expected_revision: 0,
        size: receipt.size,
        source_sha256: hash,
    };
    let scope = Publication::begin_slot(store.clone(), binding).unwrap();
    let target = ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let prepared = target
        .prepare_slot_scoped(
            &mut std::io::Cursor::new(bytes),
            7,
            Some(&receipt),
            Some(scope.clone()),
        )
        .unwrap();
    if phase != "slot_preparing" && phase != "slot_legacy_preparing" {
        scope.arm().unwrap();
    }
    if phase == "slot_accepted" || phase == "slot_legacy_preparing" {
        let mut receipt = receipt.clone();
        if phase == "slot_legacy_preparing" {
            receipt.source_sha256 = None;
        }
        store
            .profiles()
            .edit_import(0, prepared.profile.clone(), receipt)
            .unwrap();
    }
    let entries = lock(&scope.records)
        .iter()
        .map(|e| serde_json::json!({"kind":e.kind,"key":e.key}))
        .collect::<Vec<_>>();
    write_json(&root.join("ready.json"),&serde_json::json!({"entries":entries,"directory":scope.staging_directory(),"receipt":receipt})).unwrap();
    wait_for_kill()
}
fn wait_for_kill() -> ! {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
    }
    // Bound orphan lifetime if the parent test runner is itself interrupted.
    std::process::exit(74)
}
struct Crash {
    root: tempfile::TempDir,
    state: serde_json::Value,
}
impl Crash {
    fn at(phase: &str) -> Self {
        Self::at_key(phase, "device")
    }
    fn at_key(phase: &str, device_key: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("fixture-marker"),
            b"isolated crash fixture",
        )
        .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "asset_publication::journal::tests::crash_child",
                "--nocapture",
                "--ignored",
            ])
            .env("LCONNECT_IMPORT_CRASH_ROOT", root.path())
            .env("LCONNECT_IMPORT_CRASH_PHASE", phase)
            .env("LCONNECT_IMPORT_CRASH_DEVICE", device_key)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.path().join("ready.json").exists() {
            if let Some(status) = child.try_wait().unwrap() {
                let output = child.wait_with_output().unwrap();
                panic!(
                    "crash child {status}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            if Instant::now() > deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("crash fixture setup timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        assert_eq!(child.wait().unwrap().signal(), Some(9));
        let state =
            serde_json::from_slice(&fs::read(root.path().join("ready.json")).unwrap()).unwrap();
        Self { root, state }
    }
    fn paths(&self) -> Vec<PathBuf> {
        self.state["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let key = e["key"].as_str().unwrap();
                match e["kind"].as_str().unwrap() {
                    "image" => self
                        .root
                        .path()
                        .join("theme-assets")
                        .join(format!("{key}.png")),
                    "image_name" => self
                        .root
                        .path()
                        .join("theme-assets")
                        .join(format!("{key}.name.json")),
                    "font" => self
                        .root
                        .path()
                        .join("theme-fonts")
                        .join(format!("{key}.font")),
                    "media" => self
                        .root
                        .path()
                        .join("prepared-clips")
                        .join(format!("{key}.lcm")),
                    other => panic!("{other}"),
                }
            })
            .collect()
    }
}
#[test]
fn sigkill_before_owner_admission_recovers_private_candidates_and_unreferenced_assets_idempotently()
{
    for phase in ["candidate", "preparing", "discard"] {
        let crash = Crash::at(phase);
        let paths = crash.paths();
        assert!(paths.iter().all(|p| p.exists()));
        let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
        assert!(paths.iter().all(|p| !p.exists()));
        assert!(!Path::new(crash.state["directory"].as_str().unwrap()).exists());
        assert!(store.import_recovery_notices().unwrap().is_empty());
        assert!(store.recover_import_publications().unwrap().is_empty());
        assert_eq!(store.profiles().list().unwrap().revision, 0);
    }
}
#[test]
fn sigkill_armed_without_receipt_retains_files_and_durable_diagnostic_without_apply_or_commit() {
    let crash = Crash::at("armed");
    let before = crash
        .paths()
        .iter()
        .map(|p| fs::read(p).unwrap())
        .collect::<Vec<_>>();
    let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    let notices = store.import_recovery_notices().unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].operation, OP);
    assert_eq!(
        crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    assert!(store.read("device").unwrap().group_imports.is_empty());
    assert!(store.recover_import_publications().unwrap().is_empty());
    assert_eq!(store.import_recovery_notices().unwrap().len(), 1);
}
#[test]
fn sigkill_after_real_owner_commit_keeps_every_asset_and_receipt_without_another_commit() {
    let crash = Crash::at("accepted");
    let store = PersistenceStore::new_with_root(crash.root.path().into());
    let before_settings = serde_json::to_value(store.read("device").unwrap()).unwrap();
    let before_profiles = serde_json::to_value(store.profiles().list().unwrap()).unwrap();
    let bytes = crash
        .paths()
        .iter()
        .map(|p| fs::read(p).unwrap())
        .collect::<Vec<_>>();
    let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    assert!(store.import_recovery_notices().unwrap().is_empty());
    assert_eq!(
        serde_json::to_value(store.read("device").unwrap()).unwrap(),
        before_settings
    );
    assert_eq!(
        serde_json::to_value(store.profiles().list().unwrap()).unwrap(),
        before_profiles
    );
    assert_eq!(
        crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        bytes
    );
    assert!(store.recover_import_publications().unwrap().is_empty());
}
#[test]
fn sigkill_preparing_preserves_independent_offline_references_and_foreign_replacements() {
    let crash = Crash::at("preparing");
    let store = PersistenceStore::new_with_root(crash.root.path().into());
    let theme: lconnect_ipc::LcdTheme =
        serde_json::from_value(crash.state["theme"].clone()).unwrap();
    let image = theme.background_image.as_ref().unwrap().asset.clone();
    let offline = DeviceSettings {
        theme: Some(theme),
        ..Default::default()
    };
    store.write("offline", &offline).unwrap();
    let media = crash
        .paths()
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "lcm"))
        .unwrap();
    let foreign = crash.root.path().join("foreign");
    fs::write(&foreign, b"independent replacement").unwrap();
    fs::rename(foreign, &media).unwrap();
    let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    assert!(store
        .theme_assets_root()
        .join(format!("{image}.png"))
        .exists());
    assert_eq!(fs::read(media).unwrap(), b"independent replacement");
    assert_eq!(
        store
            .read("offline")
            .unwrap()
            .theme
            .unwrap()
            .background_image
            .unwrap()
            .asset,
        image
    );
    assert_eq!(store.profiles().list().unwrap().revision, 0);
}
#[test]
fn live_scope_cannot_be_recovered_and_unknown_manifest_preserves_everything() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let binding = Binding {
        device_key: "device".into(),
        operation: OP.into(),
        group: GroupKind::Screen,
        expected: GroupRevision {
            profile_revision: 0,
            settings_sha256: "a".repeat(64),
        },
        source_sha256: "b".repeat(64),
    };
    let scope = Publication::begin(store.clone(), binding).unwrap();
    assert!(store.recover_import_publications().is_err());
    assert!(scope.staging_directory().unwrap().exists());
    scope.rollback().unwrap();
    let directory = store.publication_root().join("8".repeat(32));
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("manifest.json"), b"{\"version\":99}").unwrap();
    fs::write(directory.join("private-candidate"), b"preserve").unwrap();
    assert!(PersistenceStore::open_with_root(root.path().into()).is_err());
    assert_eq!(
        fs::read(directory.join("private-candidate")).unwrap(),
        b"preserve"
    );
}

#[test]
fn sigkill_keep_decision_without_receipt_is_reported_and_never_rolled_back() {
    let crash = Crash::at("keep");
    let bytes = crash
        .paths()
        .iter()
        .map(|p| fs::read(p).unwrap())
        .collect::<Vec<_>>();
    let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    assert_eq!(store.import_recovery_notices().unwrap().len(), 1);
    assert_eq!(
        crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        bytes
    );
}
#[test]
fn replaced_ownership_link_and_redirected_journal_root_fail_closed() {
    let crash = Crash::at("preparing");
    let directory = Path::new(crash.state["directory"].as_str().unwrap());
    let replacement = crash.root.path().join("replacement");
    fs::write(&replacement, b"foreign link target").unwrap();
    fs::rename(&replacement, directory.join("0.owned")).unwrap();
    assert!(PersistenceStore::open_with_root(crash.root.path().into()).is_err());
    assert!(crash.paths().iter().all(|p| p.exists()));
    let separate = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("precious"), b"preserve").unwrap();
    std::os::unix::fs::symlink(external.path(), separate.path().join("import-publications"))
        .unwrap();
    assert!(PersistenceStore::open_with_root(separate.path().into()).is_err());
    assert_eq!(
        fs::read(external.path().join("precious")).unwrap(),
        b"preserve"
    );
}
#[test]
fn real_receipt_eviction_after_64_commits_keeps_old_assets_and_reports_unknown_outcome() {
    let crash = Crash::at("accepted");
    let store = PersistenceStore::new_with_root(crash.root.path().into());
    let archives = ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    );
    let incoming = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::FanAndLighting,
    )
    .unwrap()
    .profile;
    let mut bytes = Vec::new();
    archives.export_group(&incoming, &mut bytes).unwrap();
    let prepared = archives
        .prepare_group(&mut std::io::Cursor::new(bytes))
        .unwrap();
    for n in 0..64 {
        let latest = store.read("device").unwrap();
        let library = store.profiles().list().unwrap();
        let revision = GroupProfile::capture(&latest, &library, GroupKind::FanAndLighting)
            .unwrap()
            .revision;
        store
            .commit_group_import(
                "device",
                &latest,
                &revision,
                &format!("{:032x}", 1000 + n),
                &prepared,
            )
            .unwrap();
    }
    let latest = store.read("device").unwrap();
    assert_eq!(latest.group_imports.len(), 64);
    assert!(!latest.group_imports.iter().any(|r| r.id == OP));
    let before = serde_json::to_value(&latest).unwrap();
    let revision = store.profiles().list().unwrap().revision;
    let bytes = crash
        .paths()
        .iter()
        .map(|p| fs::read(p).unwrap())
        .collect::<Vec<_>>();
    let recovered = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    assert_eq!(recovered.import_recovery_notices().unwrap().len(), 1);
    assert_eq!(
        serde_json::to_value(recovered.read("device").unwrap()).unwrap(),
        before
    );
    assert_eq!(recovered.profiles().list().unwrap().revision, revision);
    assert_eq!(
        crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        bytes
    );
}
#[tokio::test]
async fn embedded_manager_never_writes_hid_when_publication_recovery_fails() {
    use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let directory = store.publication_root().join("8".repeat(32));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("manifest.json"), b"unknown journal").unwrap();
    let mock = MockBackend::new();
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("blocked".into()),
        PathBuf::from("/mock/blocked"),
    );
    mock.add_device(info.clone());
    let manager = crate::manager::DeviceManager::new(
        Arc::new(mock.clone()),
        store,
        crate::config::Config::default(),
        Arc::new(crate::stats::Stats::new()),
    )
    .spawn()
    .await;
    assert!(manager.list().await.is_empty());
    assert!(mock.write_history(&info).is_empty());
    manager.shutdown().await;
    assert_eq!(
        fs::read(directory.join("manifest.json")).unwrap(),
        b"unknown journal"
    );
}
#[test]
fn interrupted_scope_creation_report_write_and_retirement_are_idempotently_collected() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let base = store.publication_root();
    fs::create_dir_all(&base).unwrap();
    let creating = base.join("1".repeat(32));
    fs::create_dir(&creating).unwrap();
    fs::write(creating.join(".tmpAb1234"), b"partial initial manifest").unwrap();
    let retiring = base.join(format!("done-{}", "2".repeat(32)));
    fs::create_dir(&retiring).unwrap();
    fs::write(retiring.join("partial"), b"resolved staging").unwrap();
    fs::write(base.join(".tmpXy9876"), b"partial report").unwrap();
    store.recover_import_publications().unwrap();
    assert_eq!(fs::read_dir(&base).unwrap().count(), 0);
    assert!(store.recover_import_publications().unwrap().is_empty());
}

#[tokio::test]
async fn manager_startup_and_actual_ipc_report_unconfirmed_import_without_replaying_or_committing_it(
) {
    use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    use lconnect_ipc::{client::IpcClient, server::IpcServer};
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("journal-rpc".into()),
        PathBuf::from("/mock/journal-rpc"),
    );
    let key = crate::persistence::settings_key(&info);
    let crash = Crash::at_key("armed", &key);
    let store = Arc::new(PersistenceStore::new_with_root(crash.root.path().into()));
    let cfg = crate::config::Config::default();
    let stats = Arc::new(crate::stats::Stats::new());
    let mock = MockBackend::new();
    mock.add_device(info);
    let manager = crate::manager::DeviceManager::new(
        Arc::new(mock),
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
    assert_eq!(store.import_recovery_notices().unwrap().len(), 1);
    assert!(crash.paths().iter().all(|p| p.exists()));
    let socket = crash.root.path().join("recovery.sock");
    let path = socket.clone();
    let rpc = crate::ipc_server::IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
    let client = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(c) = IpcClient::connect(&socket).await {
                break c;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    for _ in 0..2 {
        let error = client
            .call(
                "read_group_import",
                serde_json::json!({"dev":dev,"group":"screen","operation_id":OP}),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("files were retained"), "{error}");
    }
    let theme = client
        .call("get_lcd_theme", serde_json::json!({"dev":dev}))
        .await
        .unwrap();
    assert!(theme.is_null());
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    assert!(store.read(&key).unwrap().group_imports.is_empty());
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}

#[test]
fn live_unknown_outcome_and_drop_keep_durable_notice_even_after_journal_retirement() {
    for fallback in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let binding = Binding {
            device_key: "device".into(),
            operation: OP.into(),
            group: GroupKind::Screen,
            expected: GroupRevision {
                profile_revision: 0,
                settings_sha256: "a".repeat(64),
            },
            source_sha256: "b".repeat(64),
        };
        let scope = Publication::begin(store.clone(), binding).unwrap();
        let directory = scope.staging_directory().unwrap().to_owned();
        let key = store
            .theme_assets()
            .import_named_in(
                lconnect_image::defaults::IMAGES[0].png,
                None,
                scope.staging_directory(),
                |key, _, file| scope.created(CreatedKind::Image, key, file),
            )
            .unwrap();
        scope.arm().unwrap();
        if fallback {
            drop(scope);
        } else {
            scope.retain_unknown().unwrap();
            scope.retain_unknown().unwrap();
            drop(scope);
        }
        assert!(!directory.exists());
        assert_eq!(store.import_recovery_notices().unwrap().len(), 1);
        let reopened = PersistenceStore::open_with_root(root.path().into()).unwrap();
        assert!(reopened
            .theme_assets_root()
            .join(format!("{key}.png"))
            .exists());
        assert_eq!(reopened.import_recovery_notices().unwrap().len(), 1);
        assert_eq!(reopened.profiles().list().unwrap().revision, 0);
    }
}
#[test]
fn dangling_recovery_report_symlink_is_an_error_without_replacing_it() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    fs::create_dir_all(store.publication_root()).unwrap();
    let path = store.publication_root().join("recovery-report.json");
    std::os::unix::fs::symlink(root.path().join("missing"), &path).unwrap();
    assert!(store.import_recovery_notices().is_err());
    assert!(PersistenceStore::open_with_root(root.path().into()).is_err());
    assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
}

#[test]
fn ownership_manifest_write_failure_after_link_creation_vetoes_the_final_asset() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let binding = Binding {
        device_key: "device".into(),
        operation: OP.into(),
        group: GroupKind::Screen,
        expected: GroupRevision {
            profile_revision: 0,
            settings_sha256: "a".repeat(64),
        },
        source_sha256: "b".repeat(64),
    };
    let scope = Publication::begin(store.clone(), binding).unwrap();
    let directory = scope.staging_directory().unwrap().to_owned();
    fs::remove_file(directory.join("manifest.json")).unwrap();
    fs::create_dir(directory.join("manifest.json")).unwrap();
    let mut claimed = String::new();
    let result = store.theme_assets().import_named_in(
        lconnect_image::defaults::IMAGES[0].png,
        None,
        scope.staging_directory(),
        |key, _, file| {
            claimed = key.into();
            scope.created(CreatedKind::Image, key, file)
        },
    );
    assert!(result.is_err());
    assert!(directory.join("0.owned").exists());
    assert!(!store
        .theme_assets_root()
        .join(format!("{claimed}.png"))
        .exists());
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    drop(scope);
    assert!(PersistenceStore::open_with_root(root.path().into()).is_err());
    assert!(directory.join("0.owned").exists());
    assert!(!store
        .theme_assets_root()
        .join(format!("{claimed}.png"))
        .exists());
}

#[tokio::test]
async fn accepted_journal_is_reconciled_before_normal_manager_theme_replay_and_readback_is_readonly(
) {
    use lconnect_hid::{DeviceFamilyId, DeviceInfo, MockBackend};
    use lconnect_ipc::{client::IpcClient, server::IpcServer, LcdSourceKind, LcdState};
    let info = DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("journal-accepted".into()),
        PathBuf::from("/mock/journal-accepted"),
    );
    let key = crate::persistence::settings_key(&info);
    let crash = Crash::at_key("accepted", &key);
    let store = Arc::new(PersistenceStore::new_with_root(crash.root.path().into()));
    let cfg = crate::config::Config::default();
    let stats = Arc::new(crate::stats::Stats::new());
    let mock = MockBackend::new();
    mock.add_device(info);
    let manager = crate::manager::DeviceManager::new(
        Arc::new(mock),
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
    assert!(!Path::new(crash.state["directory"].as_str().unwrap()).exists());
    assert!(store.import_recovery_notices().unwrap().is_empty());
    let socket = crash.root.path().join("accepted.sock");
    let path = socket.clone();
    let rpc = crate::ipc_server::IpcServerImpl::new(manager.clone(), stats, cfg, store.clone());
    let server = tokio::spawn(async move { IpcServer::new(Arc::new(rpc)).serve(&path).await });
    let client = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(c) = IpcClient::connect(&socket).await {
                break c;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let lcd = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let lcd: LcdState = serde_json::from_value(
                client
                    .call("get_lcd_state", serde_json::json!({"dev":dev}))
                    .await
                    .unwrap(),
            )
            .unwrap();
            if lcd.source == LcdSourceKind::Theme {
                break lcd;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(lcd.requested.brightness_percent, 55);
    assert_eq!(lcd.requested.rotation_degrees, 90);
    let before = serde_json::to_value(store.read(&key).unwrap()).unwrap();
    let revision = store.profiles().list().unwrap().revision;
    for _ in 0..2 {
        let receipt = client
            .call(
                "read_group_import",
                serde_json::json!({"dev":dev,"group":"screen","operation_id":OP}),
            )
            .await
            .unwrap();
        assert_eq!(receipt["selected_content_current"], true);
        assert_eq!(receipt["runtime"]["lcd"]["generation"], lcd.generation);
    }
    assert_eq!(
        serde_json::to_value(store.read(&key).unwrap()).unwrap(),
        before
    );
    assert_eq!(store.profiles().list().unwrap().revision, revision);
    drop(client);
    server.abort();
    let _ = server.await;
    manager.shutdown().await;
}

#[test]
fn named_slot_sigkill_preparing_cleans_owned_payloads_without_changing_the_library() {
    let crash = Crash::at("slot_preparing");
    let directory = Path::new(crash.state["directory"].as_str().unwrap());
    assert!(
        directory.join("0").is_file(),
        "raw archive decoding must also use durable owned staging"
    );
    let manifest: serde_json::Value = read_json(&directory.join("manifest.json")).unwrap();
    assert_eq!(manifest["version"], 2);
    assert!(manifest["binding"].get("device_key").is_none());
    assert!(manifest["binding"].get("expected_revision").is_some());
    assert!(crash.paths().iter().all(|p| p.exists()));
    let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
    assert!(crash.paths().iter().all(|p| !p.exists()));
    assert!(!directory.exists());
    assert_eq!(store.profiles().list().unwrap().revision, 0);
    assert!(store.slot_import_recovery_notices().unwrap().is_empty());
}
#[test]
fn named_slot_sigkill_commit_and_legacy_receipt_preserve_assets_without_another_slot_edit() {
    for phase in ["slot_accepted", "slot_legacy_preparing"] {
        let crash = Crash::at(phase);
        let store = PersistenceStore::new_with_root(crash.root.path().into());
        let before = serde_json::to_value(store.profiles().list().unwrap()).unwrap();
        let data = crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>();
        let store = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
        assert_eq!(
            serde_json::to_value(store.profiles().list().unwrap()).unwrap(),
            before
        );
        assert_eq!(
            crash
                .paths()
                .iter()
                .map(|p| fs::read(p).unwrap())
                .collect::<Vec<_>>(),
            data
        );
        assert_eq!(
            store.slot_import_recovery_notices().unwrap().len(),
            usize::from(phase == "slot_legacy_preparing")
        );
        assert!(store.recover_import_publications().unwrap().is_empty());
    }
}
#[test]
fn named_slot_armed_without_receipt_and_evicted_receipt_keep_files_with_a_slot_diagnostic() {
    for phase in ["slot_armed", "slot_accepted"] {
        let crash = Crash::at(phase);
        let store = PersistenceStore::new_with_root(crash.root.path().into());
        if phase == "slot_accepted" {
            for n in 0..64 {
                let lib = store.profiles().list().unwrap();
                let receipt = lconnect_ipc::ArchiveImportReceipt {
                    id: format!("{:032x}", 2000 + n),
                    size: 100,
                    expected_revision: lib.revision,
                    committed_revision: lib.revision + 1,
                    group: lconnect_ipc::ProfileGroup::Fan,
                    slot: 0,
                    source_sha256: Some("a".repeat(64)),
                };
                let profile = lconnect_ipc::NamedProfile {
                    slot: 0,
                    name: "Other".into(),
                    content: lconnect_ipc::ProfileContent::Fan(
                        lconnect_ipc::CoolingPolicy::Fixed { percent: 40 },
                    ),
                };
                store
                    .profiles()
                    .edit_import(lib.revision, profile, receipt)
                    .unwrap();
            }
            assert!(!store
                .profiles()
                .list()
                .unwrap()
                .archive_imports
                .iter()
                .any(|r| r.id == OP));
        }
        let before = serde_json::to_value(store.profiles().list().unwrap()).unwrap();
        let bytes = crash
            .paths()
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>();
        let recovered = PersistenceStore::open_with_root(crash.root.path().into()).unwrap();
        assert_eq!(recovered.slot_import_recovery_notices().unwrap().len(), 1);
        assert_eq!(recovered.slot_import_recovery_notices().unwrap()[0].slot, 7);
        let recovered = Arc::new(recovered);
        let service = crate::archive_transfer::ArchiveTransfers::new(
            ProfileArchives::new(
                recovered.theme_assets(),
                recovered.theme_fonts(),
                recovered.media_assets(),
            ),
            recovered.clone(),
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(service.handle(lconnect_ipc::ProfileArchiveRequest::Status { id: OP.into() }))
            .unwrap_err()
            .to_string();
        assert!(error.contains("files were retained"), "{error}");
        assert!(error.contains("new operation ID"), "{error}");
        assert!(
            error.contains("No slot or hardware Apply was replayed"),
            "{error}"
        );
        assert_eq!(
            serde_json::to_value(recovered.profiles().list().unwrap()).unwrap(),
            before
        );
        assert_eq!(
            crash
                .paths()
                .iter()
                .map(|p| fs::read(p).unwrap())
                .collect::<Vec<_>>(),
            bytes
        );
        assert!(recovered.recover_import_publications().unwrap().is_empty());
    }
}
