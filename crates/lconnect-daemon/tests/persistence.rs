//! Integration tests for the per-device persistence layer (P1D-3).

use lconnect_daemon::persistence::{DeviceSettings, PersistenceStore};
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
fn unknown_schema_version_warns_falls_back_to_defaults() {
    let tmp = TempDir::new().unwrap();
    let store = PersistenceStore::new_with_root(tmp.path().to_path_buf());
    // Hand-write an invalid version so we can confirm the fallback path.
    let path = tmp.path().join("devices/k.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "schema_version = 999\n").unwrap();
    let r = store.read("k").unwrap();
    assert_eq!(r.schema_version, 1); // fell back
}
