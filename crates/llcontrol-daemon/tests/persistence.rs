//! Integration tests for the per-device persistence layer (P1D-3).

use llcontrol_daemon::persistence::{DeviceSettings, PersistenceStore};
use tempfile::TempDir;

#[test]
fn schema_version_default_is_1() {
    let s = DeviceSettings::default();
    assert_eq!(s.schema_version, 1);
}

#[test]
fn atomic_write_then_read_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    let key = "galahad2-lcdv2_0416-7395_default";
    let mut s = DeviceSettings::default();
    s.fan.pwm_percent = 75;
    store.write(key, &s).unwrap();
    let r = store.read(key).unwrap();
    assert_eq!(r.fan.pwm_percent, 75);
}

#[test]
fn missing_file_returns_default() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    let r = store.read("never-written").unwrap();
    assert_eq!(r.schema_version, 1);
}

#[test]
fn unsupported_or_corrupt_settings_cannot_be_read_or_overwritten() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let path = tmp.path().join("devices/k.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for original in [
        "schema_version = 999\n",
        "invalid toml",
        "[cooling]\nfuture_option=true\n",
        "[thermal.cpu]\nsensor_id='cpu'\nthreshold_c=80\n[thermal.cpu.color]\nr=255\ng=0\nb=0\nfuture_option=true\n",
        "schema_version=1\nfuture_option=true\n",
        "[fan]\npwm_percent=70\nfuture_option=true\n",
        "[theme]\nversion=2\nname='Future'\nbackground=[0,0,0]\nlayers=[]\n",
    ] {
        std::fs::write(&path, original).unwrap();
        assert!(store.read("k").is_err(), "{original}");
        assert!(store.write("k", &DeviceSettings::default()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert!(!path.with_extension("toml.tmp").exists());
    }
}

#[test]
fn legacy_v1_defaults_upgrade_only_on_explicit_write() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let path = tmp.path().join("devices/k.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let legacy = "[fan]\npwm_percent=73\n";
    std::fs::write(&path, legacy).unwrap();
    let mut settings = store.read("k").unwrap();
    assert_eq!(settings.schema_version, 1);
    assert_eq!(settings.fan.pwm_percent, 73);
    assert!(
        settings.theme.is_none() && settings.cooling.fan.is_none() && settings.media_clip.is_none()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy);
    settings.lcd.brightness = 65;
    store.write("k", &settings).unwrap();
    let restored = store.read("k").unwrap();
    assert_eq!(restored.fan.pwm_percent, 73);
    assert_eq!(restored.lcd.brightness, 65);
}

#[test]
fn invalid_outgoing_schema_cannot_replace_valid_settings() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    store.write("k", &DeviceSettings::default()).unwrap();
    let path = tmp.path().join("devices/k.toml");
    let original = std::fs::read(&path).unwrap();
    let settings = DeviceSettings {
        schema_version: 2,
        ..Default::default()
    };
    assert!(store.write("k", &settings).is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn nonregular_and_oversized_settings_are_not_replaced() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let path = tmp.path().join("devices/k.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    symlink(tmp.path().join("absent"), &path).unwrap();
    assert!(store.read("k").is_err());
    assert!(store.write("k", &DeviceSettings::default()).is_err());
    assert!(std::fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    std::fs::remove_file(&path).unwrap();
    let bytes = vec![b' '; 1024 * 1024 + 1];
    std::fs::write(&path, &bytes).unwrap();
    assert!(store.read("k").is_err());
    assert!(store.write("k", &DeviceSettings::default()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn saved_media_key_roundtrips_and_conflicting_theme_never_replaces_settings() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let mut settings = DeviceSettings::default();
    settings.fan.pwm_percent = 73;
    settings.media_clip = Some("a".repeat(32));
    store.write("k", &settings).unwrap();
    let saved = store.read("k").unwrap();
    assert_eq!(saved.media_clip, settings.media_clip);
    assert_eq!(saved.fan.pwm_percent, 73);
    let path = tmp.path().join("devices/k.toml");
    let original = std::fs::read(&path).unwrap();
    settings.theme = Some(
        serde_json::from_value(
            serde_json::json!({"version":1,"name":"Conflict","background":[0,0,0],"layers":[]}),
        )
        .unwrap(),
    );
    assert!(store.write("k", &settings).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    settings.theme = None;
    for bad in ["../outside".into(), "A".repeat(32), "a".repeat(31)] {
        settings.media_clip = Some(bad);
        assert!(store.write("k", &settings).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

#[test]
fn media_reference_scan_keeps_offline_devices_and_refuses_unknown_or_invalid_state() {
    let tmp = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    let a = "a".repeat(32);
    let b = "b".repeat(32);
    let mut settings = DeviceSettings {
        media_clip: Some(a.clone()),
        ..Default::default()
    };
    store.write("offline-a", &settings).unwrap();
    settings.media_clip = Some(b.clone());
    store.write("offline-b", &settings).unwrap();
    assert_eq!(
        store.referenced_media_clips().unwrap(),
        [a, b].into_iter().collect()
    );
    let path = tmp.path().join("devices/offline-b.toml");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, "schema_version = 99").unwrap();
    assert!(store.referenced_media_clips().is_err());
    std::fs::write(&path, "media_clip = '../bad-key'").unwrap();
    assert!(store.referenced_media_clips().is_err());
    std::fs::write(&path, &original).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("missing"), &path).unwrap();
    assert!(store.referenced_media_clips().is_err());
}

#[test]
fn video_theme_roundtrip_protects_media_and_rejects_split_source() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().into());
    let key = "a".repeat(32);
    let theme: llcontrol_ipc::LcdTheme = serde_json::from_value(serde_json::json!({
        "version":2,"name":"Video and layers","background":[0,0,0],
        "background_video":key,"layers":[]
    }))
    .unwrap();
    let mut settings = DeviceSettings {
        theme: Some(theme.clone()),
        ..Default::default()
    };
    store.write("offline", &settings).unwrap();
    let restarted = PersistenceStore::new_with_root(tmp.path().into());
    assert_eq!(
        restarted.read("offline").unwrap().theme,
        Some(theme.clone())
    );
    assert!(restarted.referenced_media_clips().unwrap().contains(&key));
    settings.media_clip = Some("b".repeat(32));
    assert!(restarted.write("offline", &settings).is_err());
    assert_eq!(restarted.read("offline").unwrap().theme, Some(theme));
    restarted
        .write("offline", &DeviceSettings::default())
        .unwrap();
    assert!(restarted.referenced_media_clips().unwrap().is_empty());
}
