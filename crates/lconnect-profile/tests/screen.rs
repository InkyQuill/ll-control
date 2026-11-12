use base64::{engine::general_purpose::STANDARD, Engine};
use lconnect_profile::screen::{inspect_windows_screen, inspect_with_limits, ScreenLimits};
use serde_json::{json, Value};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
const FIXTURE: &[u8] = include_bytes!("fixtures/ScreenBanks.zip");
fn rewritten(edit: impl FnOnce(&mut Value), extra: Option<(&str, &[u8])>) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for i in 0..input.len() {
        let mut f = input.by_index(i).unwrap();
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).unwrap();
        if f.name().unwrap() == "backup" {
            let mut backup: Value = serde_json::from_slice(&bytes).unwrap();
            let mut profile: Value = serde_json::from_slice(
                &STANDARD
                    .decode(backup["Datas"][0]["Data"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
            edit.take().unwrap()(&mut profile);
            backup["Datas"][0]["Data"] =
                json!(STANDARD.encode(serde_json::to_vec(&profile).unwrap()));
            bytes = serde_json::to_vec(&backup).unwrap();
        }
        output
            .start_file(
                f.name().unwrap(),
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    if let Some((name, bytes)) = extra {
        output
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        output.write_all(bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}
#[test]
fn source_runtime_and_all_eight_editor_banks_remain_distinct_and_complete() {
    let report = inspect_windows_screen(Cursor::new(FIXTURE), 0).unwrap();
    assert_eq!(
        (report.brightness_percent, report.rotation_degrees),
        (64, 270)
    );
    assert_eq!(report.slots.len(), 8);
    assert_eq!(report.slots[1].name, None);
    assert_eq!(report.slots[2].name.as_deref(), Some(""));
    assert_eq!(report.slots[3].name.as_deref(), Some("Своя тема"));
    assert_eq!(report.slots[4].selected_effect, "Time");
    for (i, slot) in report.slots.iter().enumerate() {
        assert_eq!(slot.editor_settings.len(), 3);
        assert!(slot.editor_settings["Rain"][0].uses_default);
        let setting: Value =
            serde_json::from_str(&slot.editor_settings["ImageVideo"][0].source_json).unwrap();
        assert_eq!(setting["TextCustom"]["Text"], format!("Owned editor {i}"));
    }
    assert!(report
        .runtime_config
        .source_json
        .contains("Owned runtime text"));
    assert!(!report.runtime_config.source_json.contains("Owned editor"));
    assert_eq!(report.unconverted_controller_fields, vec!["Fan"]);
    let encoded = serde_json::to_string(&report).unwrap();
    assert!(!encoded.contains("DO-NOT-LEAK-SOURCE-IDENTITY"));
    assert!(
        !report.group_import_supported
            && !report.apply_supported
            && !report.media_decoded
            && !report.theme_semantics_validated
    );
    assert!(!report.archive.asset_payloads_validated);
}
#[test]
fn galleries_match_only_uploaded_basenames_and_keep_default_and_missing_explicit() {
    let report = inspect_windows_screen(Cursor::new(FIXTURE), 0).unwrap();
    assert_eq!(report.images[0].zip_entry.as_deref(), Some("wmsq/art.png"));
    assert_eq!(report.images[0].resolution, "archive_payload");
    assert_eq!(report.images[1].resolution, "external_default");
    assert_eq!(report.images[1].zip_entry, None); // Matching basename cannot impersonate a vendor default.
    assert_eq!(report.images[2].resolution, "missing_payload");
    assert_eq!(
        report.image_video[0].zip_entry.as_deref(),
        Some("wmsq/motion.mp4")
    );
    assert_eq!(
        report.watermarks[0].zip_entry.as_deref(),
        Some("wmsq/logo.png")
    );
    assert_eq!(report.payloads.len(), 5);
    assert!(report.payloads.iter().any(|p| p.entry == "wmsq/unused.bin"));
    let payload = report
        .payloads
        .iter()
        .find(|p| p.entry == "wmsq/art.png")
        .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        payload.sha256,
        format!("{:x}", Sha256::digest(b"owned image placeholder"))
    );
    assert_eq!(payload.bytes, 23);
}
#[test]
fn rejects_inactive_malformed_banks_selection_arrays_and_duplicate_nested_json() {
    for (field, value) in [
        ("ScreenBrightness", json!(101)),
        ("ScreenRotation", json!(90)),
        ("ScreenProfileName", json!(["only one"])),
        ("ScreenEffectType", json!([1])),
        ("StyleExpressScreenEffectType", json!([19])),
        ("ScreenProfiles", json!([{}])),
        ("ScreenConfigJson", json!("[]")),
        ("ScreenConfigJson", json!("{\"a\":1,\"a\":2}")),
    ] {
        let bytes = rewritten(|p| p[field] = value, None);
        assert!(
            inspect_windows_screen(Cursor::new(bytes), 0).is_err(),
            "{field}"
        );
    }
    for (key, value) in [
        ("ImageVideo", json!([])),
        ("ImageVideo", json!(["not JSON"])),
        ("NotAnEffect", json!(["{}"])),
        ("ImageVideo", json!(["{\"x\":1,\"x\":2}"])),
    ] {
        let bytes = rewritten(|p| p["ScreenProfiles"][7][key] = value, None);
        assert!(inspect_windows_screen(Cursor::new(bytes), 0).is_err());
    }
    let bytes = rewritten(|p| p["ScreenEffectType"][7] = json!(255), None);
    assert!(inspect_windows_screen(Cursor::new(bytes), 0).is_err());
}
#[test]
fn unsafe_paths_nested_payloads_aliases_and_budgets_fail_without_extraction() {
    for path in [r"C:\owned\..\art.png", r"C:\owned\", "", "C:/owned/a\0.png"] {
        let bytes = rewritten(
            |p| p["ScreenImageFileInfos"][0]["FilePath"] = json!(path),
            None,
        );
        assert!(
            inspect_windows_screen(Cursor::new(bytes), 0).is_err(),
            "{path}"
        );
    }
    for name in ["wmsq/sub/art.png", "wmsq/ART.PNG"] {
        let bytes = rewritten(|_| {}, Some((name, b"other")));
        assert!(inspect_windows_screen(Cursor::new(bytes), 0).is_err());
    }
    for limits in [
        ScreenLimits {
            json_bytes: 4,
            ..ScreenLimits::default()
        },
        ScreenLimits {
            gallery_entries: 1,
            ..ScreenLimits::default()
        },
        ScreenLimits {
            payload_bytes: 8,
            ..ScreenLimits::default()
        },
        ScreenLimits {
            total_payload_bytes: 30,
            ..ScreenLimits::default()
        },
    ] {
        assert!(inspect_with_limits(Cursor::new(FIXTURE), 0, limits).is_err());
    }
    assert!(inspect_windows_screen(Cursor::new(FIXTURE), 99).is_err());
    assert!(
        inspect_windows_screen(Cursor::new(include_bytes!("fixtures/Control.zip")), 1).is_err()
    );
}
#[test]
fn payload_crc_corruption_is_not_hidden_by_successful_backup_inspection() {
    let mut bytes = rewritten(|_| {}, None); // Stored members make the owned payload easy to mutate.
    let needle = b"owned unused upload";
    let offset = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap();
    bytes[offset] ^= 1;
    assert!(lconnect_profile::inspect_windows(Cursor::new(&bytes)).is_ok());
    assert!(inspect_windows_screen(Cursor::new(bytes), 0).is_err());
}
#[test]
fn empty_runtime_and_empty_banks_represent_defaults_without_synthesizing_config() {
    let bytes = rewritten(
        |p| {
            p["ScreenConfigJson"] = json!("");
            p["ScreenProfiles"] = json!([{}, {}, {}, {}, {}, {}, {}, {}]);
        },
        None,
    );
    let report = inspect_windows_screen(Cursor::new(bytes), 0).unwrap();
    assert!(report.runtime_config.uses_default);
    assert_eq!(report.runtime_config.source_json, "");
    assert!(report
        .slots
        .iter()
        .all(|slot| slot.editor_settings.is_empty()));
}
