use super::*;
use crate::windows_screen_fixture as fixture;
use crate::{
    asset_publication::Publication,
    persistence::{DeviceSettings, PersistenceStore},
};
use std::io::Cursor;
#[path = "../../../../llcontrol-profile/tests/support/screen_media.rs"]
mod motion_fixture;

fn preview_plan(video: Option<&[u8]>) -> (ScreenInspection, ScreenPlan) {
    let bytes = fixture::archive(
        |profile| {
            profile["ScreenConfigJson"]=serde_json::json!({"Style":"Full","Screens":[{"Theme":{"Type":"Image","Path":fixture::URI,"BackgroundColor":[3,5,7]},"Text":{"Type":"SliderSensor","Sensors":["CPUTemperature"],"LoopSecond":10,"Layout":[{"FontFamily":"HyperspaceRace_BoldItalic","FontSize":43,"FontColors":[[255,255,255],[255,255,255],[255,255,255]],"HorizontalAlignment":"Center","VerticalAlignment":"Center","TextAlignment":"Center","Shadow":20,"Opacity":100}]},"Watermark":{"HasImage":false,"Image":"","Horizontal":50,"Vertical":50,"Shadow":50,"Opacity":50}}]}).to_string().into();
        },
        &video
            .map(|bytes| vec![("wmsq/motion.mp4", bytes)])
            .unwrap_or_default(),
    );
    let mut mapping = fixture::mapping();
    if video.is_some() {
        mapping[fixture::URI] = serde_json::json!({"kind":"archive","entry":"wmsq/motion.mp4"});
    }
    llcontrol_profile::screen_plan::inspect_and_prepare(
        Cursor::new(bytes),
        0,
        &llcontrol_profile::linux_screen::parse_bindings(
            &serde_json::to_vec(&fixture::bindings()).unwrap(),
        )
        .unwrap(),
        &llcontrol_profile::screen_plan::parse_resource_map(&serde_json::to_vec(&mapping).unwrap())
            .unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn publish_preview_sensor(cache: &crate::sensors::SensorCache, value: f64) {
    cache.publish(
        std::time::Instant::now(),
        vec![llcontrol_ipc::SensorReading {
            id: "explicit:CPUTemperature".into(),
            label: "CPU".into(),
            unit: llcontrol_ipc::SensorUnit::Celsius,
            value: Some(value),
            status: llcontrol_ipc::SensorStatus::Valid,
            observed_unix_ms: None,
            age_ms: Some(0),
        }],
    );
}
#[tokio::test]
async fn private_scene_and_editor_previews_render_live_sensors_without_publication_for_images_and_video(
) {
    for video in [false, true] {
        let motion = video.then(|| motion_fixture::motion("mp4"));
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let archives = archives(&store);
        let target = target(&store);
        let catalog = store.theme_assets().list().unwrap();
        let (source, plan) = preview_plan(motion.as_deref());
        let staged = archives
            .stage_windows_screen(&source, plan, &target, &AtomicBool::new(false))
            .unwrap();
        let cache = crate::sensors::SensorCache::default();
        publish_preview_sensor(&cache, 42.0);
        let make = |which| {
            staged
                .preview_input(
                    &archives,
                    vec![cache.clone()],
                    Arc::new(AtomicBool::new(false)),
                    which,
                    &AtomicBool::new(false),
                )
                .unwrap()
        };
        let scene = make(llcontrol_ipc::WindowsScreenPreviewSource::Scene)
            .render()
            .await
            .unwrap();
        let editor = make(llcontrol_ipc::WindowsScreenPreviewSource::Editor)
            .render()
            .await
            .unwrap();
        assert_ne!(
            scene, editor,
            "current source and stored editor must not substitute"
        );
        let image = llcontrol_image::assets::decode_asset(&scene).unwrap();
        assert_eq!((image.width(), image.height()), (480, 480));
        publish_preview_sensor(&cache, 87.0);
        let changed = make(llcontrol_ipc::WindowsScreenPreviewSource::Scene)
            .render()
            .await
            .unwrap();
        assert_ne!(scene, changed, "cache values are sampled at preview time");
        assert_eq!(store.theme_assets().list().unwrap(), catalog);
        assert!(store
            .media_assets()
            .list()
            .unwrap()
            .iter()
            .all(|clip| clip.is_default));
    }
}

fn plan() -> (ScreenInspection, ScreenPlan) {
    llcontrol_profile::screen_plan::inspect_and_prepare(
        Cursor::new(fixture::archive(|_| {}, &[])),
        0,
        &llcontrol_profile::linux_screen::parse_bindings(
            &serde_json::to_vec(&fixture::bindings()).unwrap(),
        )
        .unwrap(),
        &llcontrol_profile::screen_plan::parse_resource_map(
            &serde_json::to_vec(&fixture::mapping()).unwrap(),
        )
        .unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn target(store: &PersistenceStore) -> GroupProfile {
    let key = store
        .theme_assets()
        .import(llcontrol_image::defaults::IMAGES[1].png)
        .unwrap();
    let mut target = GroupProfile::capture(
        &DeviceSettings::default(),
        &Default::default(),
        GroupKind::Screen,
    )
    .unwrap()
    .profile;
    target.profiles.push(serde_json::from_value(serde_json::json!({"slot":3,"name":"Keep native slot","content":{"kind":"screen","value":{"version":1,"name":"Keep image identity","background":[0,0,0],"layers":[],"background_image":{"asset":key,"fit":"crop"}}}})).unwrap());
    target.validate().unwrap();
    target
}
fn archives(store: &PersistenceStore) -> ProfileArchives {
    ProfileArchives::new(
        store.theme_assets(),
        store.theme_fonts(),
        store.media_assets(),
    )
}
#[test]
fn source_runtime_and_editor_publish_separately_while_native_library_retains_identity() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let archives = archives(&store);
    let target = target(&store);
    let original = serde_json::to_value(&target.profiles).unwrap();
    let (source, draft) = plan();
    let original_runtime = draft.runtime.as_ref().unwrap().theme.clone();
    let original_editor = draft.current_editor_theme.as_ref().unwrap().clone();
    assert_ne!(
        serde_json::to_value(&original_runtime).unwrap(),
        serde_json::to_value(&original_editor).unwrap()
    );
    let provisional = resource(&draft.staged_resources, "wmsq/art.png", Kind::Image).unwrap();
    let scope = Publication::new(store.clone(), "77777777777777777777777777777777".into());
    let prepared = archives
        .prepare_windows_screen_scoped(
            &source,
            draft,
            &target,
            &AtomicBool::new(false),
            Some(scope.clone()),
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&prepared.profile.profiles).unwrap(),
        original
    );
    assert_eq!(prepared.uploaded_images().len(), 1);
    let published = prepared.uploaded_images().first().unwrap().clone();
    assert_ne!(provisional, published);
    let GroupSettings::Screen(screen) = &prepared.profile.content else {
        panic!()
    };
    assert_eq!(screen.lcd.control_mode, "Application");
    let active = screen.theme.as_ref().unwrap();
    let editor = screen.editor.as_ref().unwrap();
    assert_eq!(active.background_image.as_ref().unwrap().asset, *published);
    assert_eq!(
        editor
            .theme
            .as_ref()
            .unwrap()
            .basic_editor
            .as_ref()
            .unwrap()
            .banks
            .len(),
        2
    );
    assert!(editor.image_keys().contains(published.as_str()));
    let mut bytes = Vec::new();
    archives
        .export_group(&prepared.profile, &mut bytes)
        .unwrap();
    let roundtrip = archives.prepare_group(&mut Cursor::new(bytes)).unwrap();
    assert!(matches!(&roundtrip.profile.content, GroupSettings::Screen(s) if s.editor.is_some()));
    drop(roundtrip);
    drop(prepared);
    scope.rollback().unwrap();
    assert!(store
        .theme_assets()
        .list()
        .unwrap()
        .iter()
        .all(|key| key != &published));
}

#[test]
fn screen_identity_excludes_target_named_library_and_uploads_are_owned_until_rollback() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let archives = archives(&store);
    let mut target = target(&store);
    let mut digests = Vec::new();
    for index in 0..2 {
        let (source, draft) = plan();
        let scope = Publication::new(store.clone(), format!("{index:032x}"));
        let prepared = archives
            .prepare_windows_screen_scoped(
                &source,
                draft,
                &target,
                &AtomicBool::new(false),
                Some(scope.clone()),
            )
            .unwrap();
        digests.push(prepared.source_sha256().to_owned());
        drop(prepared);
        scope.rollback().unwrap();
        target.profiles[0].name = "Changed after an ambiguous acknowledgement".into();
    }
    assert_eq!(digests[0], digests[1]);
}

#[test]
fn prepared_windows_screen_commits_and_restarts_through_native_group_storage() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let target = target(&store);
    let latest = DeviceSettings::default();
    let revision = store
        .snapshot_profile_group("owned-target", &latest, GroupKind::Screen)
        .unwrap()
        .revision;
    let (source, draft) = plan();
    let scope = Publication::new(store.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    let prepared = archives(&store)
        .prepare_windows_screen_scoped(
            &source,
            draft,
            &target,
            &AtomicBool::new(false),
            Some(scope.clone()),
        )
        .unwrap();
    let (saved, _) = store
        .commit_profile_group("owned-target", &latest, &revision, prepared.profile())
        .unwrap();
    scope.retain().unwrap();
    drop(prepared);
    let restarted = PersistenceStore::new_with_root(root.path().into());
    let loaded = restarted.read("owned-target").unwrap();
    assert_eq!(loaded.theme, saved.theme);
    assert_eq!(loaded.screen_editor, saved.screen_editor);
    assert_eq!(
        serde_json::to_value(restarted.profiles().list().unwrap().profiles).unwrap(),
        serde_json::to_value(target.profiles).unwrap()
    );
    restarted
        .theme_assets()
        .load(loaded.theme.as_ref().unwrap())
        .unwrap();
    restarted
        .theme_assets()
        .load(
            loaded
                .screen_editor
                .as_ref()
                .unwrap()
                .theme
                .as_ref()
                .unwrap(),
        )
        .unwrap();
}

#[test]
fn provisional_key_collision_cannot_rewrite_preserved_native_profile() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let target = target(&store);
    let (source, mut draft) = plan();
    let key = dependencies(&target.profiles[0])
        .into_iter()
        .find(|(kind, _)| *kind == Kind::Image)
        .unwrap()
        .1;
    draft
        .staged_resources
        .insert("wmsq/art.png".into(), ResourceBinding::Image { asset: key });
    let before = serde_json::to_value(&target).unwrap();
    assert!(archives(&store)
        .prepare_windows_screen(&source, draft, &target, &AtomicBool::new(false))
        .unwrap_err()
        .to_string()
        .contains("collides"));
    assert_eq!(serde_json::to_value(&target).unwrap(), before);
}

#[test]
fn private_preview_files_do_not_publish_and_changed_staging_is_rejected_before_finish() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let target = target(&store);
    let archives = archives(&store);
    let before = store.theme_assets().list().unwrap();
    let (source, draft) = plan();
    let staged = archives
        .stage_windows_screen(&source, draft, &target, &AtomicBool::new(false))
        .unwrap();
    let directory = staged.stage.path().to_owned();
    assert_eq!(store.theme_assets().list().unwrap(), before);
    let path = directory.join("0");
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, bytes).unwrap();
    let scope = Publication::new(store.clone(), "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into());
    assert!(staged
        .publish(&archives, Some(scope.clone()), &AtomicBool::new(false))
        .unwrap_err()
        .to_string()
        .contains("staging changed"));
    assert!(!directory.exists());
    scope.rollback().unwrap();
    assert_eq!(store.theme_assets().list().unwrap(), before);
}

#[test]
fn changed_payload_missing_dependency_and_cancel_never_publish_any_upload() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let archives = archives(&store);
    let target = target(&store);
    let before = store.theme_assets().list().unwrap();
    for fault in 0..6 {
        let (source, mut draft) = plan();
        let cancel = AtomicBool::new(fault == 4);
        match fault {
            0 => draft.images.images[0].png[0] ^= 1,
            1 => draft.images.images[0].source_sha256 = "0".repeat(64),
            2 => {
                draft
                    .runtime
                    .as_mut()
                    .unwrap()
                    .theme
                    .background_image
                    .as_mut()
                    .unwrap()
                    .asset = "d".repeat(32);
            }
            3 => draft.source_record_index += 1,
            5 => draft.runtime = None,
            _ => {}
        }
        let scope = Publication::new(store.clone(), format!("{:032x}", fault + 100));
        assert!(
            archives
                .prepare_windows_screen_scoped(
                    &source,
                    draft,
                    &target,
                    &cancel,
                    Some(scope.clone())
                )
                .is_err(),
            "fault {fault}"
        );
        scope.rollback().unwrap();
        assert_eq!(
            store.theme_assets().list().unwrap(),
            before,
            "fault {fault}"
        );
    }
}

#[test]
fn moving_video_reaches_native_media_container_and_late_corruption_prevents_image_publication() {
    let clip = motion_fixture::motion("mp4");
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let archives = archives(&store);
    let target = target(&store);
    let make_plan = || {
        llcontrol_profile::screen_plan::inspect_and_prepare(
            Cursor::new(fixture::archive(|_| {}, &[("wmsq/motion.mp4", &clip)])),
            0,
            &llcontrol_profile::linux_screen::parse_bindings(
                &serde_json::to_vec(&fixture::bindings()).unwrap(),
            )
            .unwrap(),
            &llcontrol_profile::screen_plan::parse_resource_map(
                &serde_json::to_vec(&fixture::mapping()).unwrap(),
            )
            .unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap()
    };
    let (source, mut draft) = make_plan();
    let mut original = Vec::new();
    for frame in 0..draft.media.items[0].frames {
        original.push(draft.media.items[0].file.read_unit(frame).unwrap().bytes);
    }
    assert!(original.len() >= 6);
    assert_ne!(original.first(), original.last());
    let scope = Publication::new(store.clone(), "88888888888888888888888888888888".into());
    let prepared = archives
        .prepare_windows_screen_scoped(
            &source,
            draft,
            &target,
            &AtomicBool::new(false),
            Some(scope.clone()),
        )
        .unwrap();
    let key = prepared.uploaded_media().first().unwrap().clone();
    let mut native = store.media_assets().acquire_source(&key).unwrap();
    assert_eq!(native.frames(), original.len());
    for (frame, bytes) in original.iter().enumerate() {
        assert_eq!(&native.read_unit(frame).unwrap().bytes, bytes);
    }
    drop(native);
    drop(prepared);
    scope.rollback().unwrap();
    assert!(store.media_assets().acquire_source(&key).is_err());

    let before: BTreeSet<_> = fs::read_dir(store.theme_assets_root())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let (source, mut draft) = make_plan();
    draft.media.items[0].encoded_sha256 = "0".repeat(64);
    let scope = Publication::new(store.clone(), "99999999999999999999999999999999".into());
    assert!(archives
        .prepare_windows_screen_scoped(
            &source,
            draft,
            &target,
            &AtomicBool::new(false),
            Some(scope.clone())
        )
        .is_err());
    // Inspect actual directory entries before rollback, not the hidden catalog.
    let after: BTreeSet<_> = fs::read_dir(store.theme_assets_root())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    scope.rollback().unwrap();
}
