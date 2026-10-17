use lconnect_daemon::{
    persistence::{DeviceSettings, PersistenceStore},
    profile_group::{GroupKind, GroupProfile, GroupSettings},
};
use lconnect_ipc::{NamedProfile, ProfileContent, ProfileEdit, ProfileGroup};

fn save_slot(store: &PersistenceStore, group: ProfileGroup, slot: u8) {
    let revision = store.profiles().list().unwrap().revision;
    store
        .profiles()
        .edit(
            revision,
            ProfileEdit::Save {
                profile: NamedProfile::defaults(
                    group,
                    slot,
                    (group != ProfileGroup::Screen).then_some("cpu:temperature"),
                )
                .unwrap(),
            },
        )
        .unwrap();
}
fn json(value: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn screen_group_roundtrip_replaces_all_screen_slots_and_preserves_cooling_lighting_and_globals() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = PersistenceStore::new_with_root(source_dir.path().into());
    save_slot(&source, ProfileGroup::Screen, 0);
    save_slot(&source, ProfileGroup::Screen, 7);
    save_slot(&source, ProfileGroup::Fan, 3);
    let mut original = DeviceSettings::default();
    original.lcd.brightness = 76;
    original.lcd.rotation = "270".into();
    original.lcd.control_mode = "Application".into();
    original.screen_collections = Some(lconnect_ipc::ScreenCollections::defaults(900));
    let ProfileContent::Screen(mut theme) = NamedProfile::defaults(ProfileGroup::Screen, 0, None)
        .unwrap()
        .content
    else {
        unreachable!()
    };
    theme.name = "Current draft with CPU data".into();
    original.theme = Some(theme);
    let snapshot = source
        .snapshot_profile_group("device", &original, GroupKind::Screen)
        .unwrap();
    assert_eq!(
        snapshot
            .profile
            .profiles
            .iter()
            .map(|p| p.slot)
            .collect::<Vec<_>>(),
        vec![0, 7]
    );
    let serialized = serde_json::to_vec(&snapshot.profile).unwrap();
    assert!(!String::from_utf8_lossy(&serialized).contains("receipt"));
    let imported: GroupProfile = serde_json::from_slice(&serialized).unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let target = PersistenceStore::new_with_root(target_dir.path().into());
    save_slot(&target, ProfileGroup::Screen, 3);
    save_slot(&target, ProfileGroup::Pump, 2);
    let mut latest = DeviceSettings {
        screen_collections: Some(lconnect_ipc::ScreenCollections::defaults(4)),
        ..Default::default()
    };
    latest.fan.pwm_percent = 61;
    latest.pump.pwm_percent = 82;
    latest.cooling.pump = Some(lconnect_ipc::CoolingPolicy::FixedRpm { rpm: 2400 });
    latest.thermal.cpu = Some(lconnect_ipc::ThermalRule {
        sensor_id: "thermal:cpu".into(),
        threshold_c: 85.,
        color: lconnect_ipc::Rgb { r: 255, g: 0, b: 0 },
    });
    target.write("device", &latest).unwrap();
    let before_library = target.profiles().list().unwrap();
    let expected = target
        .snapshot_profile_group("device", &latest, GroupKind::Screen)
        .unwrap()
        .revision;
    let (saved, library) = target
        .commit_profile_group("device", &latest, &expected, &imported)
        .unwrap();
    assert_eq!(json(&saved.lcd), json(&original.lcd));
    assert_eq!(json(&saved.theme), json(&original.theme));
    assert_eq!(
        saved.screen_collections.as_ref().unwrap().revision,
        5,
        "source revision must never replace target revision"
    );
    assert_eq!(json(&saved.cooling), json(&latest.cooling));
    assert_eq!(json(&saved.lighting), json(&latest.lighting));
    assert_eq!(json(&saved.thermal), json(&latest.thermal));
    assert_eq!(saved.fan.pwm_percent, 61);
    assert_eq!(saved.pump.pwm_percent, 82);
    assert_eq!(library.revision, before_library.revision + 1);
    assert_eq!(
        library
            .profiles
            .iter()
            .filter(|p| p.content.group() == ProfileGroup::Screen)
            .map(|p| p.slot)
            .collect::<Vec<_>>(),
        vec![0, 7]
    );
    assert_eq!(
        library
            .profiles
            .iter()
            .find(|p| p.content.group() == ProfileGroup::Pump),
        before_library
            .profiles
            .iter()
            .find(|p| p.content.group() == ProfileGroup::Pump)
    );
    let reopened = PersistenceStore::new_with_root(target_dir.path().into());
    assert_eq!(json(&reopened.read("device").unwrap()), json(&saved));
    assert_eq!(reopened.profiles().list().unwrap(), library);
    // Same old import cannot undo a later edit or create a second revision.
    assert!(reopened
        .commit_profile_group("device", &saved, &expected, &imported)
        .is_err());
    assert_eq!(reopened.profiles().list().unwrap(), library);
}

#[test]
fn fan_and_lighting_roundtrip_includes_inactive_modes_both_slot_groups_and_exact_bindings() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    save_slot(&store, ProfileGroup::Fan, 1);
    save_slot(&store, ProfileGroup::Pump, 3);
    let mut source = DeviceSettings::default();
    source.fan.pwm_percent = 55;
    source.pump.pwm_percent = 80;
    source.pump.source = "application".into();
    source.cooling.fan =
        Some(lconnect_ipc::FanPreset::High.policy("source:exact-temperature-id".into(), 2., 100));
    source.cooling.pump = Some(lconnect_ipc::CoolingPolicy::Motherboard { percent: 80 });
    source.lighting.fan_topology.sync_to_pump = true;
    source.lighting.pump_modes.insert(
        "meteor".into(),
        lconnect_daemon::persistence::PumpLightingState {
            scope: "all".into(),
            mode: "meteor".into(),
            brightness: 75,
            speed: 0,
            direction: "forward".into(),
            color: "#ff0000".into(),
            additional_colors: vec!["#0000ff".into(), "#00ff00".into(), "#ffff00".into()],
            disabled: false,
        },
    );
    source.lighting.pump = source.lighting.pump_modes.get("meteor").cloned();
    let colors = vec![lconnect_ipc::Rgb { r: 255, g: 0, b: 0 }; 4];
    let binding = lconnect_ipc::DynamicRgbConfig {
        brightness_percent: Some(75),
        direction: Some("reverse".into()),
        sensor_id: "source:exact-load-id".into(),
        unit: lconnect_ipc::SensorUnit::Percent,
        low_value: 20,
        high_value: 80,
        low_colors: colors.clone(),
        high_colors: colors,
        low_speed_percent: Some(25),
        high_speed_percent: Some(100),
    };
    source.dynamic_rgb = Some(binding.clone());
    source
        .dynamic_rgb_profiles
        .remember("meteor", binding)
        .unwrap();
    let incoming = store
        .snapshot_profile_group("source", &source, GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let incoming: GroupProfile = serde_json::from_value(json(&incoming)).unwrap();
    let otherdir = tempfile::tempdir().unwrap();
    let target = PersistenceStore::new_with_root(otherdir.path().into());
    save_slot(&target, ProfileGroup::Screen, 7);
    save_slot(&target, ProfileGroup::Fan, 0);
    let mut latest = DeviceSettings::default();
    latest.lcd.brightness = 89;
    latest.media_clip = Some("f".repeat(32));
    latest.screen_collections = Some(lconnect_ipc::ScreenCollections::defaults(7));
    let before = target.profiles().list().unwrap();
    let token = target
        .snapshot_profile_group("target", &latest, GroupKind::FanAndLighting)
        .unwrap()
        .revision;
    let (saved, library) = target
        .commit_profile_group("target", &latest, &token, &incoming)
        .unwrap();
    assert_eq!(json(&saved.lighting), json(&source.lighting));
    assert_eq!(json(&saved.cooling), json(&source.cooling));
    assert_eq!(saved.dynamic_rgb, source.dynamic_rgb);
    assert_eq!(saved.dynamic_rgb_profiles, source.dynamic_rgb_profiles);
    assert_eq!(
        saved.cooling.fan.as_ref().unwrap().sensor_id(),
        Some("source:exact-temperature-id")
    );
    assert_eq!(json(&saved.lcd), json(&latest.lcd));
    assert_eq!(saved.media_clip, latest.media_clip);
    assert_eq!(saved.screen_collections, latest.screen_collections);
    assert_eq!(
        library
            .profiles
            .iter()
            .find(|p| p.content.group() == ProfileGroup::Screen),
        before
            .profiles
            .iter()
            .find(|p| p.content.group() == ProfileGroup::Screen)
    );
    assert!(!library
        .profiles
        .iter()
        .any(|p| p.content.group() == ProfileGroup::Fan && p.slot == 0));
    assert_eq!(library.profiles.len(), 3);
}

#[test]
fn target_changes_invalid_source_and_corrupt_files_never_publish_partial_group() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut latest = DeviceSettings::default();
    store.write("device", &latest).unwrap();
    let snapshot = store
        .snapshot_profile_group("device", &latest, GroupKind::Screen)
        .unwrap();
    // Queued owner delta has not hit disk, but still invalidates the old target token.
    latest.lcd.brightness = 77;
    assert!(store
        .commit_profile_group("device", &latest, &snapshot.revision, &snapshot.profile)
        .is_err());
    save_slot(&store, ProfileGroup::Fan, 0);
    assert!(store
        .commit_profile_group(
            "device",
            &DeviceSettings::default(),
            &snapshot.revision,
            &snapshot.profile
        )
        .is_err());
    let current = store
        .snapshot_profile_group("device", &latest, GroupKind::Screen)
        .unwrap();
    let mut bad = current.profile.clone();
    bad.profiles
        .push(NamedProfile::defaults(ProfileGroup::Fan, 1, Some("cpu:temp")).unwrap());
    let before = std::fs::read(dir.path().join("profiles.json")).unwrap();
    assert!(store
        .commit_profile_group("device", &latest, &current.revision, &bad)
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("profiles.json")).unwrap(),
        before
    );
    std::fs::write(dir.path().join("devices/device.toml"), "not = valid = toml").unwrap();
    assert!(store
        .commit_profile_group("device", &latest, &current.revision, &current.profile)
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("profiles.json")).unwrap(),
        before
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("devices/device.toml")).unwrap(),
        "not = valid = toml"
    );
}

#[test]
fn unknown_versions_fields_mixed_slots_and_bad_controls_are_rejected() {
    let latest = DeviceSettings::default();
    let captured = GroupProfile::capture(&latest, &Default::default(), GroupKind::Screen)
        .unwrap()
        .profile;
    let mut value = json(&captured);
    value["settings_from_another_version"] = true.into();
    assert!(serde_json::from_value::<GroupProfile>(value).is_err());
    let mut bad = captured.clone();
    bad.version = 2;
    assert!(bad.validate().is_err());
    let mut bad = captured.clone();
    let p = NamedProfile::defaults(ProfileGroup::Screen, 3, None).unwrap();
    bad.profiles = vec![p.clone(), p];
    assert!(bad.validate().is_err());
    let mut bad = captured;
    let GroupSettings::Screen(ref mut g) = bad.content else {
        unreachable!()
    };
    g.lcd.control_mode = "LCDTest".into();
    assert!(bad.validate().is_err());
    let mut bad = GroupProfile::capture(&latest, &Default::default(), GroupKind::FanAndLighting)
        .unwrap()
        .profile;
    let GroupSettings::FanAndLighting(ref mut g) = bad.content else {
        unreachable!()
    };
    g.cooling.pump = Some(lconnect_ipc::CoolingPolicy::Fixed { percent: 0 });
    assert!(bad.validate().is_err());
}

#[test]
fn legacy_collection_import_does_not_drop_revision_protection_or_reset_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let initial = DeviceSettings::default();
    let legacy = store
        .snapshot_profile_group("device", &initial, GroupKind::Screen)
        .unwrap()
        .profile;
    let (latest, _) = store
        .commit_screen_reset("device", &initial, 0, 0, &"a".repeat(32))
        .unwrap();
    let current = store
        .snapshot_profile_group("device", &latest, GroupKind::Screen)
        .unwrap();
    assert!(
        store
            .commit_profile_group("device", &latest, &current.revision, &legacy)
            .is_err(),
        "caller must materialize the legacy catalog before import"
    );
    let mut imported = current.profile;
    let GroupSettings::Screen(ref mut screen) = imported.content else {
        unreachable!()
    };
    screen.lcd.brightness = 88;
    let (next, profiles) = store
        .commit_profile_group("device", &latest, &current.revision, &imported)
        .unwrap();
    assert!(next.screen_reset_receipt.is_none());
    assert_eq!(next.screen_collections.unwrap().revision, 2);
    assert_eq!(profiles.revision, 2);
}

#[test]
fn queued_neighbor_edits_are_kept_while_same_group_changes_reject_stale_token() {
    let dir = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(dir.path().into());
    let mut latest = DeviceSettings::default();
    let screen = store
        .snapshot_profile_group("device", &latest, GroupKind::Screen)
        .unwrap();
    latest.pump.pwm_percent = 85;
    latest.cooling.pump = Some(lconnect_ipc::CoolingPolicy::FixedRpm { rpm: 2800 });
    let (saved, library) = store
        .commit_profile_group("device", &latest, &screen.revision, &screen.profile)
        .unwrap();
    assert_eq!(saved.pump.pwm_percent, 85);
    assert_eq!(saved.cooling, latest.cooling);
    let group = store
        .snapshot_profile_group("device", &saved, GroupKind::FanAndLighting)
        .unwrap();
    let mut changed = saved.clone();
    changed.cooling.pump = Some(lconnect_ipc::CoolingPolicy::FixedRpm { rpm: 3000 });
    assert!(store
        .commit_profile_group("device", &changed, &group.revision, &group.profile)
        .is_err());
    assert_eq!(store.profiles().list().unwrap(), library);
}

#[test]
fn target_mode_epoch_changes_group_cas_but_never_enters_portable_group() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let library = store.profiles().list().unwrap();
    let mut settings = DeviceSettings::default();
    let before = GroupProfile::capture(&settings, &library, GroupKind::FanAndLighting).unwrap();
    let screen = GroupProfile::capture(&settings, &library, GroupKind::Screen).unwrap();
    settings.cooling_modes_revision = 1;
    let after = GroupProfile::capture(&settings, &library, GroupKind::FanAndLighting).unwrap();
    assert_eq!(
        json(&before.profile),
        json(&after.profile),
        "target intent is never portable"
    );
    assert_ne!(
        before.revision, after.revision,
        "no-op bank intent invalidates old group CAS/receipts"
    );
    assert_eq!(
        screen.revision,
        GroupProfile::capture(&settings, &library, GroupKind::Screen)
            .unwrap()
            .revision,
        "screen tokens ignore cooling-only changes"
    );
    settings.cooling_modes_revision = i64::MAX as u64 + 1;
    assert!(store.write("range", &settings).is_err());
    assert!(!root.path().join("devices/range.toml").exists());
}
