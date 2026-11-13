#[path = "support/screen_bindings.rs"]
mod bindings_fixture;
#[path = "support/control.rs"]
mod control_fixture;
#[path = "support/screen_editor.rs"]
mod editor_fixture;
#[path = "support/lighting.rs"]
mod lighting_fixture;
#[path = "support/wpf_color.rs"]
mod wpf_fixture;
use base64::{engine::general_purpose::STANDARD, Engine};
use llcontrol_profile::{
    lighting::inspect_windows_lighting, linux_screen::parse_bindings,
    screen::inspect_windows_screen, screen_editor::prepare_basic_banks,
};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
fn wpf_control() -> Vec<u8> {
    let mut z = ZipArchive::new(Cursor::new(control_fixture::archive())).unwrap();
    let mut bytes = Vec::new();
    z.by_name("backup")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let mut backup: Value = serde_json::from_slice(&bytes).unwrap();
    let mut profile: Value = serde_json::from_slice(
        &STANDARD
            .decode(backup["Datas"][1]["Data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    wpf_fixture::storage_colors(&mut profile);
    backup["Datas"][1]["Data"] = json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("backup", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    zip.finish().unwrap().into_inner()
}
#[test]
fn default_color_objects_normalize_like_runtime_arrays_and_retain_private_source_values() {
    let original = lighting_fixture::profile();
    let array =
        inspect_windows_lighting(Cursor::new(lighting_fixture::archive(&original, 0)), 0).unwrap();
    let mut wpf = original.clone();
    wpf_fixture::storage_colors(&mut wpf);
    let source =
        inspect_windows_lighting(Cursor::new(lighting_fixture::archive(&wpf, 0)), 0).unwrap();
    assert_eq!(source.fan, array.fan);
    assert_eq!(source.pump, array.pump);
    assert!(!source.source_color_objects.is_empty());
    let history = &source.source_color_objects["/FanLEDProfiles/StaticColor/Colors/0"];
    assert_eq!(
        history["R"],
        original["FanLEDProfiles"]["StaticColor"]["Colors"][0][0]
    );
    assert_eq!(history["A"], 255);
    assert!(history["ColorContext"].is_null());
    assert!(history["ScR"].is_number());
    assert!(!serde_json::to_value(array)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("source_color_objects"));
}
#[test]
fn complete_control_draft_keeps_default_serialized_color_history_without_executing_it() {
    let bindings = llcontrol_profile::linux_lighting::parse_bindings(
        control_fixture::bindings().to_string().as_bytes(),
    )
    .unwrap();
    let draft =
        llcontrol_profile::control::prepare(Cursor::new(wpf_control()), 1, &bindings, 0.0, 100)
            .unwrap();
    let history: Value = serde_json::from_str(&draft.lighting_source_json).unwrap();
    assert!(!history["source_color_objects"]
        .as_object()
        .unwrap()
        .is_empty());
    let bytes = serde_json::to_vec(&draft).unwrap();
    eprintln!(
        "WPF control draft bytes={} source history={}",
        bytes.len(),
        draft.lighting_source_json.len()
    );
    assert!(llcontrol_profile::control::parse(&bytes).is_ok());
}
#[test]
fn complete_editor_color_objects_keep_all_inactive_overlay_and_gradient_values() {
    let bindings = parse_bindings(bindings_fixture::bindings().to_string().as_bytes()).unwrap();
    let mut source =
        inspect_windows_screen(Cursor::new(include_bytes!("fixtures/ScreenBanks.zip")), 0).unwrap();
    for (i, slot) in source.slots.iter_mut().enumerate() {
        for (kind, image) in [("ImageVideo", true), ("BackgroundColor", false)] {
            let mut value = editor_fixture::editor(image, i);
            wpf_fixture::storage_colors(&mut value);
            slot.editor_settings.get_mut(kind).unwrap()[0].source_json = value.to_string();
        }
    }
    let prepared = prepare_basic_banks(&source, &bindings).unwrap();
    assert_eq!(prepared.overlay_count, 112);
    assert_eq!(
        prepared.slots[0].basic["BackgroundColor"].overlays["Custom"]
            .theme
            .background_fill
            .as_ref()
            .unwrap()
            .radial
            .colors,
        [[15, 16, 17], [18, 19, 20]]
    );
}
#[test]
fn malformed_or_profiled_color_objects_fail_instead_of_falling_back_to_black() {
    let mut source = lighting_fixture::profile();
    wpf_fixture::storage_colors(&mut source);
    for field in ["A", "R", "ScR", "ColorContext"] {
        let mut source = source.clone();
        source["FanLEDProfiles"]["StaticColor"]["Colors"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            inspect_windows_lighting(Cursor::new(lighting_fixture::archive(&source, 0)), 0)
                .is_err()
        );
    }
    let mut profile = source.clone();
    profile["FanLEDProfiles"]["StaticColor"]["Colors"][0]["ColorContext"] =
        json!({"ProfileUri":"file:///not-opened.icc"});
    assert!(
        inspect_windows_lighting(Cursor::new(lighting_fixture::archive(&profile, 0)), 0).is_err()
    );
    source["FanLEDProfiles"]["StaticColor"]["Colors"][0]["Unknown"] = json!(true);
    assert!(
        inspect_windows_lighting(Cursor::new(lighting_fixture::archive(&source, 0)), 0).is_err()
    );
}
