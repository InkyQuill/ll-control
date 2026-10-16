use lconnect_daemon::sensor_defaults::SensorDefaultsStore;
use lconnect_ipc::{CpuSensorDefaults, SensorDefaults};
use std::{
    fs,
    sync::{Arc, Barrier},
};
fn choice(revision: u64, id: &str) -> SensorDefaults {
    SensorDefaults {
        revision,
        cpu: CpuSensorDefaults {
            temperature: Some(id.into()),
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn save_restart_reset_and_stale_editor_preserve_explicit_choices() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("new-parent/settings");
    let store = SensorDefaultsStore::new(root.clone());
    assert_eq!(store.load().unwrap(), SensorDefaults::default());
    assert!(!root.exists());
    let saved = store.save(choice(0, "cpu:original:temp")).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(
        SensorDefaultsStore::new(root.clone()).load().unwrap(),
        saved
    );
    assert!(store.save(choice(0, "cpu:other:temp")).is_err());
    assert_eq!(store.load().unwrap(), saved);
    let reset = store
        .save(SensorDefaults {
            revision: saved.revision,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(reset.revision, 2);
    assert!(reset.cpu.temperature.is_none());
    assert_eq!(SensorDefaultsStore::new(root).load().unwrap(), reset);
}
#[test]
fn incompatible_corrupt_oversized_and_nonregular_files_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sensor-defaults.json");
    let store = SensorDefaultsStore::new(dir.path().into());
    let mut future = serde_json::to_value(choice(0, "cpu:a:temp")).unwrap();
    future["version"] = 2.into();
    let mut unknown = serde_json::to_value(choice(0, "cpu:a:temp")).unwrap();
    unknown["cpu"]["future"] = true.into();
    for bytes in [
        b"broken".to_vec(),
        serde_json::to_vec(&future).unwrap(),
        serde_json::to_vec(&unknown).unwrap(),
        vec![b' '; 65537],
    ] {
        fs::write(&path, &bytes).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(choice(0, "cpu:b:temp")).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_file(&path).unwrap();
    let target = dir.path().join("original.json");
    let bytes = serde_json::to_vec(&choice(0, "cpu:a:temp")).unwrap();
    fs::write(&target, &bytes).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(store.load().is_err());
    assert!(store.save(choice(0, "cpu:b:temp")).is_err());
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert!(fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(store.load().is_err());
    assert!(store.save(choice(0, "cpu:b:temp")).is_err());
    assert!(path.is_dir());
}
#[test]
fn concurrent_editors_have_one_winner_and_ambiguous_retry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = SensorDefaultsStore::new(dir.path().into());
    let barrier = Arc::new(Barrier::new(3));
    let workers: Vec<_> = ["cpu:a:temp", "cpu:b:temp"]
        .into_iter()
        .map(|id| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.save(choice(0, id))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let committed = store.load().unwrap();
    assert_eq!(committed.revision, 1);
    // Simulate a caller that never received the successful response.
    assert!(store
        .save(choice(0, committed.cpu.temperature.as_deref().unwrap()))
        .is_err());
    assert_eq!(store.load().unwrap(), committed);
}
#[test]
fn failure_does_not_fake_success_and_retry_requires_a_writable_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("blocked");
    fs::write(&root, "ordinary file").unwrap();
    let store = SensorDefaultsStore::new(root.clone());
    assert!(store.save(choice(0, "cpu:a:temp")).is_err());
    assert_eq!(fs::read_to_string(&root).unwrap(), "ordinary file");
    fs::remove_file(&root).unwrap();
    assert_eq!(store.save(choice(0, "cpu:a:temp")).unwrap().revision, 1);
    let before = fs::read(root.join("sensor-defaults.json")).unwrap();
    assert!(store.save(choice(1, "")).is_err());
    assert_eq!(fs::read(root.join("sensor-defaults.json")).unwrap(), before);
    let exhausted = choice(u64::MAX, "cpu:a:temp");
    let bytes = serde_json::to_vec(&exhausted).unwrap();
    fs::write(root.join("sensor-defaults.json"), &bytes).unwrap();
    assert!(store.save(exhausted).is_err());
    assert_eq!(fs::read(root.join("sensor-defaults.json")).unwrap(), bytes);
}
