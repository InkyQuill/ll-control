#[path = "support/screen_plan.rs"]
mod fixture;
#[path = "support/screen_media.rs"]
mod media;
use llcontrol_profile::{
    linux_screen::{parse_bindings, ResourceBinding},
    screen_plan::{self, ScreenPlan},
};
use serde_json::{json, Value};
use std::{io::Cursor, sync::atomic::AtomicBool};
fn plan(bytes: &[u8], mapping: Value) -> Result<ScreenPlan, llcontrol_profile::Error> {
    let (_, plan) = screen_plan::inspect_and_prepare(
        Cursor::new(bytes),
        0,
        &parse_bindings(&serde_json::to_vec(&fixture::bindings()).unwrap())?,
        &screen_plan::parse_resource_map(&serde_json::to_vec(&mapping).unwrap())?,
        &AtomicBool::new(false),
    )?;
    Ok(plan)
}
#[test]
fn complete_plan_resolves_uri_and_editor_aliases_without_publishing_or_losing_unused_payloads() {
    let source = fixture::archive(
        |_| {},
        &[
            (
                "wmsq/unused.png",
                include_bytes!("fixtures/ScreenImage.png"),
            ),
            ("wmsq/history.txt", b"opaque"),
        ],
    );
    let p = plan(&source, fixture::mapping()).unwrap();
    assert_eq!(p.images.images.len(), 2);
    let ResourceBinding::Image { asset: key } = &p.staged_resources["wmsq/art.png"] else {
        panic!()
    };
    let ResourceBinding::Image { asset: unused } = &p.staged_resources["wmsq/unused.png"] else {
        panic!()
    };
    assert_ne!(key, unused);
    for alias in [fixture::PATH, fixture::URI] {
        assert_eq!(
            serde_json::to_value(&p.resolved_references[alias]).unwrap(),
            json!({"kind":"image","asset":key})
        );
    }
    assert!(p
        .runtime
        .as_ref()
        .unwrap()
        .theme
        .image_assets()
        .contains(&key.as_str()));
    assert_eq!(p.editor_banks.basic_bank_count, 2);
    for slot in &p.editor_banks.slots {
        for overlay in slot.basic["ImageVideo"].overlays.values() {
            assert!(overlay.theme.image_assets().contains(&key.as_str()));
        }
    }
    p.collections.validate().unwrap();
    assert!(p.collections.images.contains(key));
    assert!(!p.existing_images_to_verify.contains(key));
    assert_eq!(p.unprepared_entries, vec!["wmsq/history.txt"]);
    assert!(
        p.provisional_keys
            && !p.assets_published
            && !p.complete_screen_group_supported
            && !p.runtime_deferred
    );
    let report = serde_json::to_value(&p).unwrap();
    assert!(report.get("images").is_none() && report.get("media").is_none());
    let second = plan(&source, fixture::mapping()).unwrap();
    assert_eq!(
        serde_json::to_value(second.staged_resources).unwrap(),
        serde_json::to_value(p.staged_resources).unwrap()
    );
}
#[test]
fn missing_wrong_kind_duplicate_and_unprepared_aliases_fail_the_whole_plan() {
    let source = fixture::archive(|_| {}, &[("wmsq/history.txt", b"opaque")]);
    let mut map = fixture::mapping();
    map.as_object_mut().unwrap().remove(fixture::URI);
    assert!(plan(&source, map).is_err());
    let mut map = fixture::mapping();
    map[fixture::PATH] = json!({"kind":"archive","entry":"wmsq/history.txt"});
    assert!(plan(&source, map).is_err());
    let mut map = fixture::mapping();
    map[fixture::PATH] = json!({"kind":"existing_video","key":"22222222222222222222222222222222"});
    assert!(plan(&source, map).is_err());
    let duplicated = fixture::archive(
        |p| {
            p["ScreenImageFileInfos"]
                .as_array_mut()
                .unwrap()
                .push(json!({"FilePath":fixture::URI,"IsDefaultFile":false}));
        },
        &[],
    );
    assert!(plan(&duplicated, fixture::mapping()).is_err());
    let p = plan(&source, fixture::mapping()).unwrap();
    let ResourceBinding::Image { asset: key } = &p.staged_resources["wmsq/art.png"] else {
        panic!()
    };
    let mut map = fixture::mapping();
    map["unused-alias"] = json!({"kind":"existing_image","key":key});
    assert!(plan(&source, map).is_err());
}
#[test]
fn existing_resources_are_tracked_for_later_pins_and_cancellation_and_stale_sources_fail() {
    let source = fixture::archive(|_| {}, &[]);
    let key = "11111111111111111111111111111111";
    let mut map = fixture::mapping();
    map[fixture::URI] = json!({"kind":"existing_image","key":key});
    let p = plan(&source, map).unwrap();
    assert!(p.existing_images_to_verify.contains(key));
    let bindings = parse_bindings(&serde_json::to_vec(&fixture::bindings()).unwrap()).unwrap();
    let map =
        screen_plan::parse_resource_map(&serde_json::to_vec(&fixture::mapping()).unwrap()).unwrap();
    assert!(screen_plan::inspect_and_prepare(
        Cursor::new(&source),
        0,
        &bindings,
        &map,
        &AtomicBool::new(true)
    )
    .is_err());
    let inspection =
        llcontrol_profile::screen::inspect_windows_screen(Cursor::new(&source), 0).unwrap();
    let changed = fixture::archive(|p| p["ScreenBrightness"] = json!(42), &[]);
    assert!(screen_plan::prepare(
        Cursor::new(changed),
        &inspection,
        &bindings,
        &map,
        &AtomicBool::new(false)
    )
    .is_err());
}
#[test]
fn movie_remap_retains_owned_encoded_file_until_plan_drop() {
    let clip = media::motion("mp4");
    let source = fixture::archive(
        |p| {
            p["ScreenImageVideoFileInfos"] = json!([{"FilePath":"movie","IsDefaultFile":false}]);
        },
        &[("wmsq/movie.mp4", &clip)],
    );
    let mut map = fixture::mapping();
    map["movie"] = json!({"kind":"archive","entry":"wmsq/movie.mp4"});
    let mut p = plan(&source, map).unwrap();
    assert_eq!(p.media.items.len(), 1);
    let path = p.media.items[0].file.staging_path().to_owned();
    assert!(path.exists());
    let first = p.media.items[0].file.read_unit(0).unwrap();
    llcontrol_ipc::validate_h264_access_unit(&first, true).unwrap();
    let ResourceBinding::Video { key } = &p.staged_resources["wmsq/movie.mp4"] else {
        panic!()
    };
    assert!(p.collections.video_keys().contains(key.as_str()));
    assert!(!p.existing_videos_to_verify.contains(key));
    drop(p);
    assert!(!path.exists());
}
#[test]
fn map_admission_is_strict_bounded_and_never_accepts_host_paths_as_archive_members() {
    for bytes in [br#"{"x":{"kind":"archive","entry":"wmsq/../a.png"}}"#.as_slice(),br#"{"x":{"kind":"existing_image","key":"bad"}}"#,br#"{"x":{"kind":"archive","entry":"wmsq/a.png"},"x":{"kind":"archive","entry":"wmsq/b.png"}}"#,br#"{"x":{"kind":"archive","entry":"/etc/passwd"}}"#,br#"{"x":{"kind":"archive","entry":"wmsq/a.png","unexpected":true}}"#] {assert!(screen_plan::parse_resource_map(bytes).is_err());}
    assert!(screen_plan::parse_resource_map(&vec![b' '; 65537]).is_err());
}

#[test]
fn native_defaults_are_explicit_and_overlapping_binding_aliases_are_rejected() {
    let source = fixture::archive(|_| {}, &[]);
    let mut map = fixture::mapping();
    map[fixture::URI] = json!({"kind":"existing_image","key":llcontrol_ipc::BUILTIN_IMAGE_KEY});
    let prepared = plan(&source, map).unwrap();
    assert!(prepared
        .existing_images_to_verify
        .contains(llcontrol_ipc::BUILTIN_IMAGE_KEY));
    assert_eq!(
        prepared.collections.images[0],
        llcontrol_ipc::ScreenCollections::defaults(1).images[0]
    );
    let mut bindings = fixture::bindings();
    bindings["resources"][fixture::URI] =
        json!({"kind":"image","asset":"11111111111111111111111111111111"});
    let bindings = parse_bindings(&serde_json::to_vec(&bindings).unwrap()).unwrap();
    let map =
        screen_plan::parse_resource_map(&serde_json::to_vec(&fixture::mapping()).unwrap()).unwrap();
    assert!(screen_plan::inspect_and_prepare(
        Cursor::new(source),
        0,
        &bindings,
        &map,
        &AtomicBool::new(false)
    )
    .is_err());
}

#[test]
fn inactive_editor_fonts_are_pending_pins_and_empty_runtime_is_not_fabricated() {
    let source = fixture::archive(|p| p["ScreenConfigJson"] = json!(""), &[]);
    let mut bindings = fixture::bindings();
    let key = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    bindings["fonts"]["HyperspaceRace_BoldItalic"]["font_asset"] = json!(key);
    let bindings = parse_bindings(&serde_json::to_vec(&bindings).unwrap()).unwrap();
    let map =
        screen_plan::parse_resource_map(&serde_json::to_vec(&fixture::mapping()).unwrap()).unwrap();
    let (_, prepared) = screen_plan::inspect_and_prepare(
        Cursor::new(&source),
        0,
        &bindings,
        &map,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(prepared.runtime.is_none() && prepared.runtime_deferred);
    assert!(prepared.existing_fonts_to_verify.contains(key));
    assert_eq!(prepared.editor_banks.overlay_count, 14);
    let malformed = fixture::archive(|p| p["ScreenConfigJson"] = json!("{}"), &[]);
    assert!(screen_plan::inspect_and_prepare(
        Cursor::new(malformed),
        0,
        &bindings,
        &map,
        &AtomicBool::new(false)
    )
    .is_err());
    assert!(
        screen_plan::parse_resource_map(br#"{" ":{"kind":"archive","entry":"wmsq/art.png"}}"#)
            .is_err()
    );
}

#[test]
fn native_basic_theme_retains_both_backgrounds_all_overlays_and_refuses_selected_deferred_effects()
{
    let source = fixture::archive(|_| {}, &[]);
    let prepared = plan(&source, fixture::mapping()).unwrap();
    let theme = prepared.editor_banks.slots[0].native_theme().unwrap();
    theme.validate().unwrap();
    let cache = theme.basic_editor.as_ref().unwrap();
    assert_eq!(cache.banks.len(), 2);
    assert_eq!(
        cache.banks.iter().map(|b| b.overlays.len()).sum::<usize>(),
        14
    );
    assert_eq!(
        cache.selected_background,
        llcontrol_ipc::BasicBackground::ImageVideo
    );
    assert!(theme.image_assets().is_superset(
        &prepared.editor_banks.slots[0].basic["ImageVideo"].overlays["None"]
            .theme
            .image_assets()
    ));
    // Unassigned source entry2 is retained, not turned into a named profile.
    assert_eq!(prepared.editor_banks.slots.len(), 1);
    assert!(prepared.inactive_editor_indices.contains(&2));
    let color = theme
        .select_basic_editor(
            llcontrol_ipc::BasicBackground::BackgroundColor,
            llcontrol_ipc::BasicOverlay::Custom,
        )
        .unwrap();
    assert!(color.background_image.is_none());
    assert_eq!(
        color.background_fill.as_ref().unwrap().radial.colors,
        [[15, 16, 17], [18, 19, 20]]
    );
}

#[test]
fn constructor_shaped_empty_reserve_entries_do_not_create_named_profiles_or_require_editors() {
    let bytes = fixture::archive(
        |profile| {
            profile["ScreenProfiles"] = json!([{}, {}, {}, {}, {}, {}, {}, {}]);
            profile["ScreenEffectType"] = json!([1, 1, 1, 1, 1, 1, 1, 1]);
            profile["ScreenProfileName"] =
                json!(["Full Screen", null, null, null, null, null, null, null]);
            profile["ScreenConfigJson"] = json!("");
            for key in [
                "ScreenImageFileInfos",
                "ScreenImageVideoFileInfos",
                "ScreenWatermarkFileInfos",
            ] {
                profile[key] = json!([]);
            }
        },
        &[],
    );
    let prepared = plan(&bytes, fixture::mapping()).unwrap();
    assert_eq!(prepared.inactive_editor_indices, vec![1, 2, 3, 4, 5, 6, 7]);
    assert!(!prepared.current_editor_deferred);
    let current = prepared.current_editor_theme.as_ref().unwrap();
    current.validate().unwrap();
    assert_eq!(current.basic_editor.as_ref().unwrap().banks.len(), 1);
    assert_eq!(prepared.editor_banks.basic_bank_count, 1);
    assert_eq!(prepared.editor_banks.overlay_count, 7);
    let default = &prepared.editor_banks.slots[0].basic["ImageVideo"];
    assert!(default.source_absent_materialized && default.source_default_materialized);
    assert_eq!(default.selected_overlay, "SliderSensor");
    assert_eq!(prepared.editor_banks.slots.len(), 1);
    // Empty serialized config invokes a Windows UI default request, not proven LocalUI.
    assert!(prepared.runtime.is_none() && prepared.runtime_deferred);
    let value = serde_json::to_value(&prepared).unwrap();
    assert!(value.get("native_editor_themes").is_none());
}
#[test]
fn missing_selected_current_color_materializes_only_it_and_never_replaces_independent_runtime() {
    let bytes = fixture::archive(
        |profile| {
            profile["ScreenProfiles"] = json!([{}, {}, {}, {}, {}, {}, {}, {}]);
            profile["ScreenEffectType"] = json!([2, 1, 1, 1, 1, 1, 1, 1]);
        },
        &[],
    );
    let prepared = plan(&bytes, fixture::mapping()).unwrap();
    let current = prepared.current_editor_theme.as_ref().unwrap();
    assert!(current.background_image.is_none());
    assert_eq!(
        current.basic_editor.as_ref().unwrap().selected_background,
        llcontrol_ipc::BasicBackground::BackgroundColor
    );
    assert!(prepared
        .runtime
        .as_ref()
        .unwrap()
        .theme
        .background_image
        .is_some());
    assert!(prepared.editor_banks.slots[0].basic["BackgroundColor"].source_absent_materialized);
    assert!(!prepared.editor_banks.slots[0]
        .basic
        .contains_key("ImageVideo"));
    assert_eq!(prepared.editor_banks.basic_bank_count, 1);
}

#[test]
fn unassigned_source_entries_remain_exact_opaque_data_without_requiring_runtime_bindings() {
    let opaque = "{\"UnusedSourceEditor\":\"preserve exactly\"}";
    let bytes = fixture::archive(
        |profile| {
            profile["ScreenProfiles"][1] = json!({"ImageVideo":[opaque]});
        },
        &[],
    );
    let bindings = parse_bindings(&serde_json::to_vec(&fixture::bindings()).unwrap()).unwrap();
    let map =
        screen_plan::parse_resource_map(&serde_json::to_vec(&fixture::mapping()).unwrap()).unwrap();
    let (source, prepared) = screen_plan::inspect_and_prepare(
        Cursor::new(bytes),
        0,
        &bindings,
        &map,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        source.slots[1].editor_settings["ImageVideo"][0].source_json,
        opaque
    );
    assert_eq!(
        prepared.editor_banks.scope,
        llcontrol_profile::screen_editor::PreparationScope::CurrentEditor
    );
    assert_eq!(prepared.editor_banks.slots.len(), 1);
    assert!(prepared.current_editor_theme.is_some());
    let mut resolved = bindings;
    resolved.resources = prepared.resolved_references;
    // The explicit research mode still validates all source entries when requested.
    let error =
        llcontrol_profile::screen_editor::prepare_basic_banks(&source, &resolved).unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error}");
}
#[test]
fn unsupported_current_effect_is_explicit_without_substituting_a_basic_editor() {
    let bytes = fixture::archive(|profile| profile["ScreenEffectType"][0] = json!(3), &[]);
    let prepared = plan(&bytes, fixture::mapping()).unwrap();
    assert!(prepared.current_editor_deferred && prepared.current_editor_theme.is_none());
    assert!(
        prepared.runtime.is_some(),
        "source runtime is independently serialized"
    );
    assert_eq!(prepared.inactive_editor_indices, vec![1, 2, 3, 4, 5, 6, 7]);
}
