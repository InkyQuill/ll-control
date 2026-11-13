use base64::{engine::general_purpose::STANDARD, Engine};
use llcontrol_profile::{inspect_windows, inspect_with_limits, Group, Limits, LCDV2_TYPE_ID};
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn record(kind: u32, profile: &[u8]) -> Value {
    json!({"MainType":0,"SubType":kind,"Metadata":"PRIVATE-WINDOWS-DEVICE-PATH", "Data":STANDARD.encode(profile)})
}
fn backup(kind: u32, records: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"Version":"2.1.28.0","CreatedAt":"2026-10-17T00:00:00Z","Type":kind,"Datas":records})).unwrap()
}
fn archive(entries: &[(&str, &[u8])], compression: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn valid() -> Vec<u8> {
    backup(
        1,
        vec![record(
            LCDV2_TYPE_ID,
            br#"{"ScreenBrightness":50,"ScreenConfigJson":"private user content"}"#,
        )],
    )
}
#[test]
fn source_derived_screen_and_fan_envelopes_keep_record_selection_and_hide_private_data() {
    for (kind, expected) in [(0, Group::FanAndLighting), (1, Group::Screen)] {
        for compression in [CompressionMethod::Stored, CompressionMethod::Deflated] {
            let bytes = backup(
                kind,
                vec![
                    record(LCDV2_TYPE_ID - 1, br#"{}"#),
                    record(LCDV2_TYPE_ID, br#"{"Fan":{},"Pump":{}}"#),
                    record(LCDV2_TYPE_ID, br#"{"ScreenBrightness":50}"#),
                ],
            );
            let zip = archive(
                &[
                    ("backup", &bytes),
                    ("wmsq/кадр.png", b"not decoded"),
                    ("other/file", b"other"),
                ],
                compression,
            );
            let report = inspect_windows(Cursor::new(&zip)).unwrap();
            assert_eq!(report.group, expected);
            assert_eq!(report.total_records, 3);
            assert_eq!(
                report
                    .controllers
                    .iter()
                    .map(|c| c.record_index)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert_eq!(report.wmsq_files, 1);
            assert_eq!(report.zip_entries, 3);
            assert!(report.inspection_only);
            assert!(!report.import_supported);
            assert!(!report.asset_payloads_validated);
            let output = serde_json::to_string(&report).unwrap();
            assert!(!output.contains("PRIVATE-WINDOWS-DEVICE-PATH"));
            assert_eq!(report.controllers[0].profile_fields, vec!["Fan", "Pump"]);
            assert_eq!(report.controllers[0].profile_sha256.len(), 64);
        }
    }
}
#[test]
fn foreign_versions_groups_and_device_families_never_become_lcdv2() {
    let original: Value = serde_json::from_slice(&valid()).unwrap();
    for (key, value) in [
        ("Version", json!("2.1.29.0")),
        ("Type", json!(2)),
        ("Type", json!(true)),
        ("Datas", json!([])),
        ("Future", json!(1)),
    ] {
        let mut envelope = original.clone();
        envelope[key] = value;
        assert!(
            inspect_windows(Cursor::new(archive(
                &[("backup", &serde_json::to_vec(&envelope).unwrap())],
                CompressionMethod::Stored
            )))
            .is_err(),
            "{key}"
        );
    }
    let bytes = backup(1, vec![record(LCDV2_TYPE_ID - 1, br#"{"Fan":{}}"#)]);
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &bytes)],
        CompressionMethod::Stored
    )))
    .is_err());
}
#[test]
fn ambiguous_keys_malformed_base64_and_non_object_profiles_are_rejected() {
    let bytes = br#"{"Version":"2.1.28.0","Version":"2.1.28.0","Type":1,"Datas":[]}"#;
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", bytes)],
        CompressionMethod::Deflated
    )))
    .is_err());
    for profile in [
        br#"{"Fan":1,"Fan":2}"#.as_slice(),
        br#"{"Fan":{"Mode":1,"Mode":2}}"#,
        br#"[]"#,
        br#"{}"#,
        b"not json",
    ] {
        let bytes = backup(1, vec![record(LCDV2_TYPE_ID, profile)]);
        assert!(inspect_windows(Cursor::new(archive(
            &[("backup", &bytes)],
            CompressionMethod::Stored
        )))
        .is_err());
    }
    let mut r = record(LCDV2_TYPE_ID, b"{}");
    r["Data"] = json!("%%%not-base64");
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &backup(1, vec![r]))],
        CompressionMethod::Stored
    )))
    .is_err());
    let nested = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    let bytes = backup(1, vec![record(LCDV2_TYPE_ID, nested.as_bytes())]);
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &bytes)],
        CompressionMethod::Deflated
    )))
    .is_err());
}
#[test]
fn path_collisions_unsafe_names_links_and_missing_backup_are_rejected_without_extraction() {
    let bytes = valid();
    for name in [
        "../backup",
        "/absolute",
        "wmsq/../../escape",
        "C:/file",
        "wmsq\\file",
        "wmsq/./file",
        "wmsq//file",
        "wmsq/name:stream",
    ] {
        let zip = archive(
            &[("backup", &bytes), (name, b"x")],
            CompressionMethod::Stored,
        );
        assert!(inspect_windows(Cursor::new(zip)).is_err(), "{name}");
    }
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &bytes), ("BACKUP", &bytes)],
        CompressionMethod::Stored
    )))
    .is_err());
    assert!(inspect_windows(Cursor::new(archive(
        &[("other", b"not a Windows export")],
        CompressionMethod::Stored
    )))
    .is_err());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("backup", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&bytes).unwrap();
    writer
        .add_symlink("wmsq/link", "../../escape", SimpleFileOptions::default())
        .unwrap();
    assert!(inspect_windows(Cursor::new(writer.finish().unwrap().into_inner())).is_err());
}
#[test]
fn metadata_and_decompressed_budgets_are_enforced_before_any_asset_read() {
    let bytes = valid();
    let zip = archive(
        &[("backup", &bytes), ("wmsq/large", &vec![0; 8192])],
        CompressionMethod::Deflated,
    );
    for limits in [
        Limits {
            archive_bytes: 100,
            ..Limits::default()
        },
        Limits {
            entries: 1,
            ..Limits::default()
        },
        Limits {
            directory_bytes: 10,
            ..Limits::default()
        },
        Limits {
            backup_bytes: 10,
            ..Limits::default()
        },
        Limits {
            profile_bytes: 10,
            ..Limits::default()
        },
        Limits {
            archive_bytes: 4096,
            ..Limits::default()
        },
    ] {
        assert!(inspect_with_limits(Cursor::new(&zip), limits).is_err());
    }
    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
    envelope["CreatedAt"] = json!("x".repeat(65));
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &serde_json::to_vec(&envelope).unwrap())],
        CompressionMethod::Stored
    )))
    .is_err());
    let mut large_fields = serde_json::Map::new();
    large_fields.insert("x".repeat(129), json!(0));
    let bytes = backup(
        1,
        vec![record(
            LCDV2_TYPE_ID,
            &serde_json::to_vec(&large_fields).unwrap(),
        )],
    );
    assert!(inspect_windows(Cursor::new(archive(
        &[("backup", &bytes)],
        CompressionMethod::Stored
    )))
    .is_err());
    // Asset contents are not decoded: the report explicitly preserves that boundary.
    assert!(
        !inspect_windows(Cursor::new(zip))
            .unwrap()
            .asset_payloads_validated
    );
}
#[test]
fn directory_forgery_crc_damage_comments_and_trailing_data_are_rejected() {
    let bytes = valid();
    let zip = archive(&[("backup", &bytes)], CompressionMethod::Stored);
    let mut corrupt = zip.clone();
    let end = corrupt.len() - 22;
    corrupt[end + 8..end + 10].copy_from_slice(&5000u16.to_le_bytes());
    corrupt[end + 10..end + 12].copy_from_slice(&5000u16.to_le_bytes());
    assert!(inspect_windows(Cursor::new(corrupt)).is_err());
    let center = zip
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .unwrap();
    let mut encrypted = zip.clone();
    encrypted[6..8].copy_from_slice(&1u16.to_le_bytes());
    encrypted[center + 8..center + 10].copy_from_slice(&1u16.to_le_bytes());
    assert!(inspect_windows(Cursor::new(encrypted)).is_err());
    let mut unsupported = zip.clone();
    unsupported[8..10].copy_from_slice(&99u16.to_le_bytes());
    unsupported[center + 10..center + 12].copy_from_slice(&99u16.to_le_bytes());
    assert!(inspect_windows(Cursor::new(unsupported)).is_err());
    let mut multi = zip.clone();
    multi[end + 4..end + 6].copy_from_slice(&1u16.to_le_bytes());
    assert!(inspect_windows(Cursor::new(multi)).is_err());
    let mut corrupt = zip.clone();
    let at = corrupt
        .windows(bytes.len())
        .position(|window| window == bytes)
        .unwrap();
    corrupt[at] ^= 1;
    assert!(inspect_windows(Cursor::new(corrupt)).is_err());
    let mut corrupt = zip.clone();
    corrupt.push(0);
    assert!(inspect_windows(Cursor::new(corrupt)).is_err());
    let mut corrupt = zip;
    let end = corrupt.len() - 22;
    corrupt[end + 20..end + 22].copy_from_slice(&1u16.to_le_bytes());
    corrupt.push(b'x');
    assert!(inspect_windows(Cursor::new(corrupt)).is_err());
}
#[test]
fn zip64_directory_is_bounded_and_inspected_without_large_payload_allocation() {
    let bytes = valid();
    let mut zip = archive(&[("backup", &bytes)], CompressionMethod::Stored);
    let end = zip.split_off(zip.len() - 22);
    let directory_offset = u32::from_le_bytes(end[16..20].try_into().unwrap()) as u64;
    let directory_size = u32::from_le_bytes(end[12..16].try_into().unwrap()) as u64;
    let position = zip.len() as u64;
    zip.extend_from_slice(b"PK\x06\x06");
    zip.extend_from_slice(&44u64.to_le_bytes());
    zip.extend_from_slice(&45u16.to_le_bytes());
    zip.extend_from_slice(&45u16.to_le_bytes());
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&1u64.to_le_bytes());
    zip.extend_from_slice(&1u64.to_le_bytes());
    zip.extend_from_slice(&directory_size.to_le_bytes());
    zip.extend_from_slice(&directory_offset.to_le_bytes());
    zip.extend_from_slice(b"PK\x06\x07");
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&position.to_le_bytes());
    zip.extend_from_slice(&1u32.to_le_bytes());
    let mut end = end;
    end[8..12].fill(255);
    end[12..20].fill(255);
    zip.extend_from_slice(&end);
    assert_eq!(
        inspect_windows(Cursor::new(&zip))
            .unwrap()
            .controllers
            .len(),
        1
    );
    let at = position as usize + 32;
    zip[at..at + 8].copy_from_slice(&5000u64.to_le_bytes());
    let other = position as usize + 24;
    zip[other..other + 8].copy_from_slice(&5000u64.to_le_bytes());
    assert!(inspect_windows(Cursor::new(zip)).is_err());
}

#[test]
fn dotnet_default_json_and_optimal_zip_producer_match_the_source_derived_envelope() {
    for (bytes, group) in [
        (
            include_bytes!("fixtures/FanAndLighting.zip").as_slice(),
            Group::FanAndLighting,
        ),
        (
            include_bytes!("fixtures/Screen.zip").as_slice(),
            Group::Screen,
        ),
    ] {
        let report = inspect_windows(Cursor::new(bytes)).unwrap();
        assert_eq!(report.group, group);
        assert_eq!(report.source_version, "2.1.28.0");
        assert_eq!(report.controllers.len(), 1);
        assert_eq!(report.controllers[0].profile_fields.len(), 5);
        assert_eq!(report.created_at.as_deref(), Some("2026-10-17T00:00:00Z"));
        assert_eq!(report.wmsq_files, 1);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("OWNED-SYNTHETIC-DEVICE-PATH"));
        assert!(!report.import_supported);
        assert!(!report.asset_payloads_validated);
    }
}
