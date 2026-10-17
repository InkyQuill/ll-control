#[path = "support/screen_editor.rs"]
mod editor_fixture;
#[path = "support/screen_bindings.rs"]
mod runtime_fixture;
use lconnect_ipc::{BackgroundMode, ThemeLayer};
use lconnect_profile::{
    linux_screen::parse_bindings,
    screen::{inspect_windows_screen, GalleryFile, ScreenInspection, SourceJson},
    screen_editor::prepare_basic_banks,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Cursor};
fn document(value: Value) -> SourceJson {
    SourceJson {
        source_json: value.to_string(),
        sha256: String::new(),
        uses_default: false,
    }
}
fn source() -> ScreenInspection {
    let mut source =
        inspect_windows_screen(Cursor::new(include_bytes!("fixtures/ScreenBanks.zip")), 0).unwrap();
    for (i, slot) in source.slots.iter_mut().enumerate() {
        slot.editor_settings = BTreeMap::from([
            (
                "ImageVideo".into(),
                vec![document(editor_fixture::editor(true, i))],
            ),
            (
                "BackgroundColor".into(),
                vec![document(editor_fixture::editor(false, i))],
            ),
            ("Rain".into(), vec![document(json!({"Owned":"Deferred"}))]),
        ]);
    }
    source
}
fn bindings() -> lconnect_profile::linux_screen::Bindings {
    parse_bindings(runtime_fixture::bindings().to_string().as_bytes()).unwrap()
}
#[test]
fn all_eight_basic_banks_keep_every_inactive_overlay_and_original_slot_identity() {
    let source = self::source();
    let result = prepare_basic_banks(&source, &bindings()).unwrap();
    assert_eq!(
        (
            result.slots.len(),
            result.basic_bank_count,
            result.overlay_count
        ),
        (8, 16, 112)
    );
    assert!(!result.complete_screen_group_supported && !result.apply_supported);
    for (i, (slot, original)) in result.slots.iter().zip(&source.slots).enumerate() {
        assert_eq!(slot.index, i);
        assert_eq!(slot.source_name, original.name);
        assert_eq!(slot.selected_effect, original.selected_effect);
        assert_eq!(slot.style_express_effect, original.style_express_effect);
        assert_eq!(slot.deferred_effects, vec!["Rain"]);
        for kind in ["ImageVideo", "BackgroundColor"] {
            let editor = &slot.basic[kind];
            assert_eq!(editor.selected_overlay, "Custom");
            assert!(!editor.source_default_materialized);
            assert_eq!(editor.overlays.len(), 7);
            let ThemeLayer::Text { text, style } = &editor.overlays["Custom"].theme.layers[0]
            else {
                panic!("custom")
            };
            assert_eq!(text, &format!("Owned editor slot{i}"));
            assert_eq!(style.size, 65);
            assert_eq!(style.color, [12, 23, 34]);
            let ThemeLayer::SensorCycle {
                sensors,
                interval_seconds,
                ..
            } = &editor.overlays["SliderSensor"].theme.layers[0]
            else {
                panic!("slider")
            };
            assert_eq!(*interval_seconds, 13);
            assert_eq!(
                sensors
                    .iter()
                    .map(|s| s.sensor_id.as_str())
                    .collect::<Vec<_>>(),
                [
                    "explicit:CoolantTemp",
                    "explicit:CPUTemperature",
                    "explicit:GPULoad"
                ]
            );
            assert!(editor.overlays["None"].theme.layers.is_empty());
            let ThemeLayer::Sensor { sensor_id, .. } =
                &editor.overlays["DuoSensor"].theme.layers[0]
            else {
                panic!("duo")
            };
            assert_eq!(sensor_id, "explicit:PumpRPM");
        }
    }
}
#[test]
fn inactive_gradient_colors_flip_and_solid_survive_all_overlay_proposals() {
    let result = prepare_basic_banks(&source(), &bindings()).unwrap();
    for (i, slot) in result.slots.iter().enumerate() {
        for overlay in slot.basic["BackgroundColor"].overlays.values() {
            assert_eq!(overlay.theme.background, [i as u8, 1, 2]);
            let fill = overlay.theme.background_fill.as_ref().unwrap();
            assert_eq!(fill.mode, BackgroundMode::Vertical);
            assert_eq!(fill.horizontal.colors, [[3, 4, 5], [6, 7, 8]]);
            assert!(fill.horizontal.flip);
            assert_eq!(fill.vertical.colors, [[9, 10, 11], [12, 13, 14]]);
            assert!(!fill.vertical.flip);
            assert_eq!(fill.radial.colors, [[15, 16, 17], [18, 19, 20]]);
            assert!(fill.radial.flip);
        }
    }
}
#[test]
fn malformed_inactive_setting_or_missing_inactive_binding_rejects_the_entire_bank() {
    for path in ["TextTime", "TextFourSensor", "TextCustom"] {
        let mut source = self::source();
        let mut bad = editor_fixture::editor(true, 7);
        bad[path]["Unknown"] = json!(1);
        source.slots[7]
            .editor_settings
            .get_mut("ImageVideo")
            .unwrap()[0] = document(bad);
        assert!(prepare_basic_banks(&source, &bindings()).is_err(), "{path}");
    }
    let mut source = self::source();
    let mut bad = editor_fixture::editor(false, 7);
    bad["BackgroundColor"]["ColorSettings"]["RadialGradient"]["Colors"] = json!([]);
    source.slots[7]
        .editor_settings
        .get_mut("BackgroundColor")
        .unwrap()[0] = document(bad);
    assert!(prepare_basic_banks(&source, &bindings()).is_err());
    let mut mapping = bindings();
    mapping.sensors.remove("GPUTemperature");
    assert!(prepare_basic_banks(&self::source(), &mapping).is_err());
    let mut source = self::source();
    source.slots[0]
        .editor_settings
        .get_mut("ImageVideo")
        .unwrap()
        .push(document(editor_fixture::editor(true, 0)));
    assert!(prepare_basic_banks(&source, &bindings()).is_err());
}
#[test]
fn empty_settings_materialize_scoped_defaults_and_require_explicit_default_asset_binding() {
    let mut source = self::source();
    for slot in &mut source.slots {
        for kind in ["ImageVideo", "BackgroundColor"] {
            slot.editor_settings.get_mut(kind).unwrap()[0]
                .source_json
                .clear();
        }
    }
    let result = prepare_basic_banks(&source, &bindings()).unwrap();
    let image = &result.slots[0].basic["ImageVideo"];
    let color = &result.slots[0].basic["BackgroundColor"];
    assert!(image.source_default_materialized && color.source_default_materialized);
    assert_eq!(image.selected_overlay, "SliderSensor");
    let ThemeLayer::SensorCycle {
        style,
        interval_seconds,
        sensors,
        ..
    } = &image.overlays["SliderSensor"].theme.layers[0]
    else {
        panic!("slider")
    };
    assert_eq!(*interval_seconds, 10);
    assert_eq!(style.shadow.as_ref().unwrap().opacity, 60);
    assert_eq!(sensors[0].sensor_id, "explicit:CPUTemperature");
    let ThemeLayer::SensorCycle { style, .. } = &color.overlays["SliderSensor"].theme.layers[0]
    else {
        panic!("slider")
    };
    assert_eq!(style.shadow.as_ref().unwrap().opacity, 50);
    source.image_video.insert(
        0,
        GalleryFile {
            source_path: "C:\\owned\\a_default_video.mp4".into(),
            is_default_file: true,
            zip_entry: None,
            resolution: "external_default",
        },
    );
    assert!(prepare_basic_banks(&source, &bindings()).is_err());
    let mut mapping = bindings();
    mapping.resources.insert(
        "C:\\owned\\a_default_video.mp4".into(),
        lconnect_profile::linux_screen::ResourceBinding::Video {
            key: "55555555555555555555555555555555".into(),
        },
    );
    let result = prepare_basic_banks(&source, &mapping).unwrap();
    assert_eq!(
        result.slots[0].basic["ImageVideo"].overlays["None"]
            .theme
            .background_video
            .as_deref(),
        Some("55555555555555555555555555555555")
    );
}
#[test]
fn absent_basic_banks_and_deferred_selection_do_not_create_or_apply_replacements() {
    let mut source = self::source();
    source.slots[0].editor_settings.remove("ImageVideo");
    source.slots[0].editor_settings.remove("BackgroundColor");
    source.slots[0].selected_effect = "Rain".into();
    let result = prepare_basic_banks(&source, &bindings()).unwrap();
    assert!(result.slots[0].basic.is_empty());
    assert_eq!(result.slots[0].selected_effect, "Rain");
    assert_eq!(result.basic_bank_count, 14);
    assert!(!result.complete_screen_group_supported);
}
