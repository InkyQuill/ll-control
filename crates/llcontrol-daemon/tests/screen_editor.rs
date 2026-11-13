#[path = "../../llcontrol-profile/tests/support/screen_plan.rs"]
mod source_fixture;
use llcontrol_daemon::{
    persistence::{DeviceSettings, PersistenceStore},
    profile_archive::ProfileArchives,
    profile_group::{GroupKind, GroupProfile, GroupSettings},
    screen_editor::StoredScreenEditor,
};
use llcontrol_ipc::*;
use serde_json::json;
use std::{collections::BTreeMap, io::Cursor, sync::atomic::AtomicBool};
const FONT: &[u8] = include_bytes!("../../llcontrol-image/assets/DejaVuSans.ttf");
fn history(opaque: &str) -> String {
    json!({"version":1,"format":"windows_lconnect3_2_1_28","source":{"archive":{"backup_sha256":"a".repeat(64)},"unassigned":{"1":opaque}}}).to_string()
}
fn editor(theme: Option<LcdTheme>) -> StoredScreenEditor {
    StoredScreenEditor {
        version: 1,
        theme,
        source_json: history("private Windows path"),
        resources: BTreeMap::new(),
    }
}
fn plain() -> LcdTheme {
    serde_json::from_value(
        json!({"version":1,"name":"Active unchanged","background":[12,18,30],"layers":[]}),
    )
    .unwrap()
}
fn archives(store: &PersistenceStore) -> ProfileArchives {
    ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    )
}
fn stored_fixture(store: &PersistenceStore) -> StoredScreenEditor {
    let image = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    let font = store.theme_fonts().import(FONT).unwrap();
    let video = store
        .media_assets()
        .import(
            "Editor-only clip".into(),
            H264Clip {
                fps: 24,
                units: vec![H264AccessUnit {
                    bytes: vec![0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 1, 0, 0, 1, 0x65, 1],
                    keyframe: true,
                }],
            },
        )
        .unwrap()
        .key;
    let mut value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../desktop/tests/fixtures/basic-editor-theme.json"
    ))
    .unwrap();
    fn replace(value: &mut serde_json::Value, image: &str, font: &str) {
        match value {
            serde_json::Value::Object(map) => {
                if map.contains_key("asset") {
                    map.insert("asset".into(), json!(image));
                }
                if let Some(style) = map.get_mut("style").and_then(|v| v.as_object_mut()) {
                    style.insert("font_asset".into(), json!(font));
                }
                for child in map.values_mut() {
                    replace(child, image, font);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    replace(child, image, font);
                }
            }
            _ => {}
        }
    }
    replace(&mut value, &image, &font);
    value.as_object_mut().unwrap().remove("background_image");
    value["background_video"] = json!(video);
    value["version"] = json!(2);
    let mut stored = editor(Some(serde_json::from_value(value).unwrap()));
    let unlisted = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[1].png)
        .unwrap();
    stored.resources.insert(
        "archive:wmsq/unlisted.png".into(),
        ScreenBackgroundAsset::Image(unlisted),
    );
    stored.validate().unwrap();
    stored
}
#[test]
fn source_plan_preserves_raw_unassigned_data_and_resource_aliases_in_private_editor_history() {
    let raw = "{\"Unused\":\"C:\\\\private\\\\source.png\"}";
    let archive = source_fixture::archive(
        |p| p["ScreenProfiles"][1] = json!({"ImageVideo":[raw]}),
        &[],
    );
    let bindings = llcontrol_profile::linux_screen::parse_bindings(
        &source_fixture::bindings().to_string().into_bytes(),
    )
    .unwrap();
    let map = llcontrol_profile::screen_plan::parse_resource_map(
        &source_fixture::mapping().to_string().into_bytes(),
    )
    .unwrap();
    let (mut source, plan) = llcontrol_profile::screen_plan::inspect_and_prepare(
        Cursor::new(archive),
        0,
        &bindings,
        &map,
        &AtomicBool::new(false),
    )
    .unwrap();
    let stored = StoredScreenEditor::from_prepared(&source, &plan).unwrap();
    let history: serde_json::Value = serde_json::from_str(&stored.source_json).unwrap();
    assert_eq!(
        history["source"]["slots"][1]["editor_settings"]["ImageVideo"][0]["source_json"],
        raw
    );
    assert!(stored.resources.contains_key("archive:wmsq/art.png"));
    assert!(stored
        .resources
        .contains_key(&format!("reference:{}", source_fixture::URI)));
    source.record_index += 1;
    assert!(StoredScreenEditor::from_prepared(&source, &plan).is_err());
    source.record_index -= 1;
    source.archive.backup_sha256 = "b".repeat(64);
    assert!(StoredScreenEditor::from_prepared(&source, &plan).is_err());
}
#[test]
fn offline_restart_and_cleanup_scan_retain_editor_resources_but_never_interpret_opaque_history() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let mut stored = stored_fixture(&store);
    let orphan = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[1].png)
        .unwrap();
    stored.source_json = history(&orphan);
    let settings = DeviceSettings {
        theme: Some(plain()),
        screen_editor: Some(stored.clone()),
        ..Default::default()
    };
    store.write("offline", &settings).unwrap();
    let restarted = PersistenceStore::new_with_root(root.path().into());
    let read = restarted.read("offline").unwrap();
    assert_eq!(read.theme, settings.theme);
    assert_eq!(read.screen_editor, Some(stored.clone()));
    assert_eq!(
        restarted.referenced_theme_assets().unwrap(),
        stored.image_keys().into_iter().map(str::to_owned).collect()
    );
    assert!(!restarted
        .referenced_theme_assets()
        .unwrap()
        .contains(&orphan));
    assert_eq!(
        restarted.referenced_media_clips().unwrap(),
        stored.video_keys().into_iter().map(str::to_owned).collect()
    );
    assert_eq!(
        restarted.referenced_theme_fonts().unwrap(),
        stored.font_keys().into_iter().map(str::to_owned).collect()
    );
    let before = std::fs::read(root.path().join("devices/offline.toml")).unwrap();
    let mut invalid = read.clone();
    invalid.screen_editor.as_mut().unwrap().source_json = "[]".into();
    assert!(restarted.write("offline", &invalid).is_err());
    assert_eq!(
        std::fs::read(root.path().join("devices/offline.toml")).unwrap(),
        before
    );
}
#[test]
fn portable_group_remaps_editor_only_assets_and_retains_private_history_without_replacing_active_scene(
) {
    let dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(dir.path().into());
    let stored = stored_fixture(&source);
    let settings = DeviceSettings {
        theme: Some(plain()),
        screen_collections: Some(ScreenCollections::defaults(1)),
        screen_editor: Some(stored.clone()),
        ..Default::default()
    };
    let group = GroupProfile::capture(&settings, &ProfileLibrary::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let mut bytes = Vec::new();
    archives(&source).export_group(&group, &mut bytes).unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let target = PersistenceStore::new_with_root(target_dir.path().into());
    let latest = DeviceSettings::default();
    let expected = target
        .snapshot_profile_group("target", &latest, GroupKind::Screen)
        .unwrap()
        .revision;
    let prepared = archives(&target)
        .prepare_group(&mut Cursor::new(bytes))
        .unwrap();
    let (saved, _) = target
        .commit_profile_group("target", &latest, &expected, prepared.profile())
        .unwrap();
    assert_eq!(saved.theme, settings.theme);
    let imported = saved.screen_editor.as_ref().unwrap();
    assert_eq!(imported.source_json, stored.source_json);
    assert!(imported.image_keys().is_disjoint(&stored.image_keys()));
    assert!(imported.video_keys().is_disjoint(&stored.video_keys()));
    assert!(imported.font_keys().is_disjoint(&stored.font_keys()));
    target
        .theme_assets()
        .load(imported.theme.as_ref().unwrap())
        .unwrap();
    target
        .theme_fonts()
        .load_theme(imported.theme.as_ref().unwrap())
        .unwrap();
    for key in imported.video_keys() {
        target.media_assets().acquire_source(key).unwrap();
    }
    let reloaded = PersistenceStore::new_with_root(target_dir.path().into())
        .read("target")
        .unwrap();
    assert_eq!(reloaded.screen_editor, saved.screen_editor);
    assert_eq!(reloaded.theme, settings.theme);
    let mut exported = Vec::new();
    archives(&target)
        .export_group(
            &GroupProfile::capture(
                &reloaded,
                &target.profiles().list().unwrap(),
                GroupKind::Screen,
            )
            .unwrap()
            .profile,
            &mut exported,
        )
        .unwrap();
    assert!(exported.len() > FONT.len());
    let mut changed = reloaded.clone();
    changed.screen_editor.as_mut().unwrap().source_json = history("changed metadata");
    assert_ne!(
        target
            .snapshot_profile_group("target", &changed, GroupKind::Screen)
            .unwrap()
            .revision,
        target
            .snapshot_profile_group("target", &reloaded, GroupKind::Screen)
            .unwrap()
            .revision
    );
    let GroupSettings::Screen(native) = &group.content else {
        panic!()
    };
    assert!(native.editor.is_some());
}
#[test]
fn metadata_limits_and_legacy_absence_fail_closed() {
    let mut stored = editor(None);
    stored.validate().unwrap();
    stored.version = 2;
    assert!(stored.validate().is_err());
    stored.version = 1;
    stored.source_json = " ".repeat(512 * 1024 + 1);
    assert!(stored.validate().is_err());
    let legacy = serde_json::to_value(DeviceSettings::default()).unwrap();
    assert!(legacy.get("screen_editor").is_none());
    stored.source_json = history("opaque");
    stored.resources.insert(
        "reference:bad".into(),
        ScreenBackgroundAsset::Image("../escape".into()),
    );
    assert!(stored.validate().is_err());
}

#[test]
fn control_import_preserves_independent_editor_and_full_screen_reset_clears_it() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let settings = DeviceSettings {
        theme: Some(plain()),
        screen_editor: Some(stored_fixture(&store)),
        ..Default::default()
    };
    store.write("device", &settings).unwrap();
    let before = store
        .snapshot_profile_group("device", &settings, GroupKind::FanAndLighting)
        .unwrap();
    let control = GroupProfile::capture(
        &DeviceSettings::default(),
        &ProfileLibrary::default(),
        GroupKind::FanAndLighting,
    )
    .unwrap()
    .profile;
    let (saved, library) = store
        .commit_profile_group("device", &settings, &before.revision, &control)
        .unwrap();
    assert_eq!(saved.screen_editor, settings.screen_editor);
    assert_eq!(saved.theme, settings.theme);
    let (reset, _) = store
        .commit_screen_reset("device", &saved, library.revision, 0, &"f".repeat(32))
        .unwrap();
    assert!(reset.screen_editor.is_none());
    assert!(store.read("device").unwrap().screen_editor.is_none());
    let original = settings
        .screen_editor
        .as_ref()
        .unwrap()
        .image_keys()
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(store
        .referenced_theme_assets()
        .unwrap()
        .is_disjoint(&original));
}

#[tokio::test]
async fn actual_cleanup_deletes_only_orphan_and_preserves_editor_only_and_unlisted_resources() {
    use llcontrol_daemon::{
        config::Config, ipc_server::IpcServerImpl, manager::DeviceManager, stats::Stats,
    };
    use std::sync::Arc;
    let root = tempfile::tempdir().unwrap();
    let initial = PersistenceStore::new_with_root(root.path().into());
    let mut editor = stored_fixture(&initial);
    let orphan = initial
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[0].png)
        .unwrap();
    editor.source_json = history(&orphan);
    initial
        .write(
            "offline",
            &DeviceSettings {
                screen_editor: Some(editor.clone()),
                ..Default::default()
            },
        )
        .unwrap();
    drop(initial);
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let stats = Arc::new(Stats::new());
    let config = Config::default();
    let manager = DeviceManager::new(
        Arc::new(llcontrol_hid::MockBackend::new()),
        store.clone(),
        config.clone(),
        stats.clone(),
    )
    .spawn()
    .await;
    let rpc = IpcServerImpl::new(manager.clone(), stats, config, store.clone());
    let result = rpc.cleanup_theme_assets(true, vec![]).await.unwrap();
    assert_eq!(result.deleted, vec![orphan]);
    for key in editor.image_keys() {
        assert!(store
            .theme_assets_root()
            .join(format!("{key}.png"))
            .is_file());
    }
    assert!(
        store.read("offline").unwrap().theme.is_none(),
        "cleanup/editor data must not create a running scene"
    );
    manager.shutdown().await;
}
