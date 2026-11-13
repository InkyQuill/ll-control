#[path = "support/screen_plan.rs"]
mod fixture;
use llcontrol_profile::{screen::inspect_windows_screen, screen_requirements};
use serde_json::json;
use std::io::Cursor;

#[test]
fn requirements_match_real_preparation_without_bindings_or_payload_decoding() {
    let bytes = fixture::archive(
        |p| {
            p["ScreenProfiles"][1] = json!({"ImageVideo":["{\"UnknownUnassignedDTO\":true}"]});
        },
        &[],
    );
    let source = inspect_windows_screen(Cursor::new(&bytes), 0).unwrap();
    let required = screen_requirements::inspect(&source).unwrap();
    assert_eq!(required.bindings.sensor_roles.len(), 6);
    assert_eq!(required.bindings.fonts.len(), 2);
    assert_eq!(required.bindings.resources.get(fixture::PATH), Some(&true));
    assert_eq!(required.bindings.resources.get(fixture::URI), Some(&false));
    assert_eq!(required.payloads.len(), 1);
    assert!(required.deferred.is_empty());
    assert!(!required.apply_supported && !required.payloads_decoded);
    let bindings = llcontrol_profile::linux_screen::parse_bindings(
        &serde_json::to_vec(&fixture::bindings()).unwrap(),
    )
    .unwrap();
    let aliases = llcontrol_profile::screen_plan::parse_resource_map(
        &serde_json::to_vec(&fixture::mapping()).unwrap(),
    )
    .unwrap();
    let (_, plan) = llcontrol_profile::screen_plan::inspect_and_prepare(
        Cursor::new(bytes),
        0,
        &bindings,
        &aliases,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    let themes = plan.runtime.iter().chain(
        plan.editor_banks
            .slots
            .iter()
            .flat_map(|s| s.basic.values())
            .flat_map(|b| b.overlays.values()),
    );
    let mut roles = std::collections::BTreeSet::new();
    let mut fonts = std::collections::BTreeSet::new();
    for theme in themes {
        roles.extend(theme.used_sensor_roles.iter().cloned());
        fonts.extend(theme.used_fonts.iter().cloned());
    }
    assert_eq!(roles, required.bindings.sensor_roles);
    assert_eq!(fonts, required.bindings.fonts);
    let report = serde_json::to_value(required).unwrap();
    assert!(report.get("theme").is_none());
    assert!(!report.to_string().contains("inspection:"));
}
#[test]
fn empty_runtime_and_unsupported_current_effect_remain_explicitly_deferred() {
    let bytes = fixture::archive(
        |p| {
            p["ScreenConfigJson"] = json!("");
            p["ScreenEffectType"][0] = json!(3);
        },
        &[("wmsq/unknown.dat", b"opaque")],
    );
    let source = inspect_windows_screen(Cursor::new(bytes), 0).unwrap();
    let required = screen_requirements::inspect(&source).unwrap();
    assert!(required.deferred.contains(&"empty_runtime_default_request"));
    assert!(required.deferred.contains(&"unsupported_current_editor"));
    assert!(required.deferred.contains(&"unsupported_payload"));
    assert!(!required.apply_supported);
}
#[test]
fn malformed_current_editor_and_invalid_resource_references_never_produce_partial_requirements() {
    for resource in [false, true] {
        let bytes = fixture::archive(
            |p| {
                let mut editor: serde_json::Value =
                    serde_json::from_str(p["ScreenProfiles"][0]["ImageVideo"][0].as_str().unwrap())
                        .unwrap();
                if resource {
                    editor["ImageVideoPath"] = json!("invalid\npath");
                } else {
                    editor["TextCustom"]["FontFamily"] = json!(99);
                }
                p["ScreenProfiles"][0]["ImageVideo"][0] = json!(editor.to_string());
            },
            &[],
        );
        let source = inspect_windows_screen(Cursor::new(bytes), 0).unwrap();
        assert!(screen_requirements::inspect(&source).is_err());
    }
}

#[test]
fn absent_current_color_bank_uses_only_its_scoped_defaults_and_not_inactive_source_paths() {
    let bytes = fixture::archive(
        |p| {
            p["ScreenConfigJson"] = json!("");
            p["ScreenEffectType"][0] = json!(2);
            p["ScreenProfiles"][0] = json!({});
            for index in 1..8 {
                p["ScreenProfiles"][index] =
                    json!({"ImageVideo":["{\"ImageVideoPath\":\"never-required-private-path\"}"]});
            }
        },
        &[],
    );
    let source = inspect_windows_screen(Cursor::new(bytes), 0).unwrap();
    let required = screen_requirements::inspect(&source).unwrap();
    assert_eq!(required.bindings.sensor_roles.len(), 6);
    assert_eq!(required.bindings.resources.len(), 1);
    assert!(required.bindings.resources.contains_key(fixture::PATH));
    assert!(!required.bindings.resources.contains_key(fixture::URI));
    assert!(!serde_json::to_string(&required)
        .unwrap()
        .contains("never-required"));
    assert_eq!(required.deferred, vec!["empty_runtime_default_request"]);
}
