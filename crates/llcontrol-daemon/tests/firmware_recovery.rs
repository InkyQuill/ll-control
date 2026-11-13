//! Durable, read-only recovery inspection. Private files only, no HID backend.
use llcontrol_daemon::{
    actor::firmware_update::{CoolingHold, Outcome, Phase, Progress, Step},
    persistence::{settings_key, DeviceSettings, PersistenceStore},
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo};
use llcontrol_ipc::{FirmwareRecoveryState as State, FirmwareSavedStage};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn target() -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some("RECOVERY".into()),
        "/mock/previous-path".into(),
    )
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        if !dir.exists() {
            return;
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(root, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().into(),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn arm(store: &PersistenceStore) -> llcontrol_daemon::firmware_operation::ArmedOperation {
    let settings = DeviceSettings::default();
    store.write(&settings_key(&target()), &settings).unwrap();
    store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, settings)
        .unwrap()
}
#[test]
fn absent_store_inspection_never_creates_state_or_invents_a_job() {
    let root = tempfile::tempdir().unwrap();
    let absent = root.path().join("not-created");
    let status = PersistenceStore::new_with_root(absent.clone())
        .firmware_operations()
        .recovery_status();
    assert_eq!(status.state, State::NoSavedOperation);
    assert!(status.operation.is_none());
    assert!(status.guards.is_empty());
    assert!(!absent.exists());
}
#[test]
fn all_saved_stages_survive_restart_inspection_with_raw_files_unchanged() {
    for stage in [
        FirmwareSavedStage::Armed,
        FirmwareSavedStage::NotStarted,
        FirmwareSavedStage::EffectsPossible,
        FirmwareSavedStage::Unknown,
        FirmwareSavedStage::AwaitingReconnect,
    ] {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let mut operation = arm(&store);
        let id = operation.record().operation_id.clone();
        match stage {
            FirmwareSavedStage::Armed => drop(operation),
            FirmwareSavedStage::NotStarted => operation
                .finish(&Outcome {
                    progress: Progress {
                        phase: Phase::NotStarted,
                        ..Progress::default()
                    },
                    diagnostic: Some("cancelled before effects".into()),
                })
                .unwrap(),
            FirmwareSavedStage::EffectsPossible => {
                operation.effects_possible().unwrap();
                drop(operation);
            }
            FirmwareSavedStage::Unknown | FirmwareSavedStage::AwaitingReconnect => {
                operation.effects_possible().unwrap();
                let complete = stage == FirmwareSavedStage::AwaitingReconnect;
                operation
                    .finish(&Outcome {
                        progress: Progress {
                            phase: if complete {
                                Phase::AwaitingReconnect
                            } else {
                                Phase::Unknown
                            },
                            total_chunks: 619,
                            cooling_hold: Some(CoolingHold::PumpOnly),
                            query_written: true,
                            pump_hold_written: complete,
                            chunk_writes: if complete { 619 } else { 0 },
                            chunk_echoes: if complete { 619 } else { 0 },
                            activation_written: complete,
                            attempted: Some(if complete {
                                Step::Activate
                            } else {
                                Step::PumpHold
                            }),
                            ..Progress::default()
                        },
                        diagnostic: None,
                    })
                    .unwrap();
            }
        }
        let before = snapshot(root.path());
        let reopened = PersistenceStore::new_with_root(root.path().into());
        let status = reopened.firmware_operations().recovery_status();
        assert_eq!(
            status.state,
            if matches!(
                stage,
                FirmwareSavedStage::Armed | FirmwareSavedStage::NotStarted
            ) {
                State::JournalOnly
            } else {
                State::Guarded
            }
        );
        let saved = status.operation.unwrap();
        assert_eq!(saved.stage, stage);
        assert_eq!(saved.operation_id, id);
        assert_eq!(saved.previous_hid_path, "/mock/previous-path");
        if stage == FirmwareSavedStage::AwaitingReconnect {
            assert_eq!(saved.progress.unwrap().chunk_echoes, 619);
            assert!(status.diagnostic.unwrap().contains("verification"));
        }
        assert_eq!(snapshot(root.path()), before);
        assert!(reopened
            .firmware_operations()
            .ensure_replay_allowed(&target())
            .is_err());
    }
}
#[test]
fn missing_and_mismatched_markers_are_reported_without_repair_or_retargeting() {
    for mode in 0..4 {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        let mut operation = arm(&store);
        operation.effects_possible().unwrap();
        drop(operation);
        let journal = root.path().join("firmware-operation.json");
        let settings = root
            .path()
            .join("devices")
            .join(format!("{}.toml", settings_key(&target())));
        let expected = match mode {
            0 => {
                std::fs::remove_file(journal).unwrap();
                State::GuardOnly
            }
            1 => {
                std::fs::remove_file(settings).unwrap();
                State::Inconsistent
            }
            2 => {
                let text = std::fs::read_to_string(&settings).unwrap();
                let id = store
                    .firmware_operations()
                    .load()
                    .unwrap()
                    .unwrap()
                    .operation_id;
                std::fs::write(
                    settings,
                    text.replace(&id, "11111111111111111111111111111111"),
                )
                .unwrap();
                State::Inconsistent
            }
            _ => {
                let extra = root.path().join("devices/another.toml");
                std::fs::copy(settings, extra).unwrap();
                State::Inconsistent
            }
        };
        let before = snapshot(root.path());
        let status = store.firmware_operations().recovery_status();
        assert_eq!(status.state, expected, "{status:?}");
        assert_eq!(snapshot(root.path()), before);
        assert!(store
            .firmware_operations()
            .ensure_replay_allowed(&target())
            .is_err());
    }
}
#[test]
fn corrupt_oversized_symlink_and_pending_transaction_are_unreadable_without_writes() {
    for mode in 0..5 {
        let root = tempfile::tempdir().unwrap();
        let store = PersistenceStore::new_with_root(root.path().into());
        drop(arm(&store));
        let journal = root.path().join("firmware-operation.json");
        match mode {
            0 => std::fs::write(journal, b"broken json").unwrap(),
            1 => std::fs::write(journal, vec![b' '; 2 * 1024 * 1024 + 1]).unwrap(),
            2 => {
                std::fs::remove_file(&journal).unwrap();
                std::os::unix::fs::symlink("devices/missing.toml", journal).unwrap();
            }
            3 => std::fs::write(root.path().join("devices/broken.toml"), b"invalid = [").unwrap(),
            _ => {
                let key = settings_key(&target());
                let before =
                    std::fs::read_to_string(root.path().join(format!("devices/{key}.toml")))
                        .unwrap();
                let mut changed = DeviceSettings::default();
                changed.fan.pwm_percent = 73;
                let transaction = serde_json::json!({
                    "version":1,"device_key":key,"device_before":before,
                    "device_after":toml::to_string_pretty(&changed).unwrap(),"profiles_before":null,
                    "profiles_after":serde_json::to_string_pretty(&llcontrol_ipc::ProfileLibrary::default()).unwrap()
                });
                std::fs::write(
                    root.path().join("screen-reset-transaction.json"),
                    serde_json::to_vec_pretty(&transaction).unwrap(),
                )
                .unwrap();
            }
        };
        let before = if mode == 2 {
            None
        } else {
            Some(snapshot(root.path()))
        };
        let status = store.firmware_operations().recovery_status();
        assert_eq!(status.state, State::Unreadable, "{status:?}");
        assert!(status
            .diagnostic
            .unwrap()
            .contains("original state preserved"));
        if let Some(before) = before {
            assert_eq!(snapshot(root.path()), before);
        } else {
            assert_eq!(
                std::fs::read_link(root.path().join("firmware-operation.json")).unwrap(),
                PathBuf::from("devices/missing.toml")
            );
        }
    }
}
#[test]
fn excessive_directory_entries_refuse_inspection_with_bounded_diagnostic() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    std::fs::create_dir(root.path().join("devices")).unwrap();
    for i in 0..257 {
        std::fs::write(root.path().join(format!("devices/{i}.ignored")), b"").unwrap();
    }
    let status = store.firmware_operations().recovery_status();
    assert_eq!(status.state, State::Unreadable);
    assert!(status.diagnostic.unwrap().len() <= 2048);
}
