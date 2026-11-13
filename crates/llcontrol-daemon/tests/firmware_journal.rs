//! Private-file journal and real manager/MockBackend quarantine, no device I/O.
use llcontrol_daemon::{
    actor::firmware_update::{CoolingHold, Outcome, Phase, Progress, Step},
    config::Config,
    firmware_operation::{Journal, Stage},
    manager::DeviceManager,
    persistence::{DeviceSettings, PersistenceStore},
    stats::Stats,
};
use llcontrol_hid::{DeviceFamilyId, DeviceInfo, HidBackend, MockBackend, OpenHandle};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Barrier,
    },
    time::Duration,
};
fn info(serial: &str, path: &str) -> DeviceInfo {
    DeviceInfo::new(
        DeviceFamilyId::new("galahad2-lcdv2"),
        0x0416,
        0x7395,
        Some(serial.into()),
        path.into(),
    )
}
fn target() -> DeviceInfo {
    info("SN-A", "/mock/old")
}
fn snapshot() -> DeviceSettings {
    let mut s = DeviceSettings::default();
    s.lcd.brightness = 47;
    s.cooling.pump = Some(llcontrol_ipc::CoolingPolicy::Motherboard { percent: 80 });
    s
}
fn outcome(phase: Phase) -> Outcome {
    let mut p = Progress {
        phase,
        total_chunks: 619,
        cooling_hold: Some(CoolingHold::PumpOnly),
        ..Progress::default()
    };
    if phase != Phase::NotStarted {
        p.query_written = true;
        p.attempted = Some(Step::PumpHold);
    }
    if phase == Phase::AwaitingReconnect {
        p.pump_hold_written = true;
        p.chunk_writes = 619;
        p.chunk_echoes = 619;
        p.activation_written = true;
        p.attempted = Some(Step::Activate);
    }
    Outcome {
        progress: p,
        diagnostic: None,
    }
}
fn file(root: &Path) -> PathBuf {
    root.join("firmware-operation.json")
}
#[test]
fn published_intent_survives_drop_and_restart_without_modifying_saved_device_settings() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let key = llcontrol_daemon::persistence::settings_key(&target());
    store.write(&key, &snapshot()).unwrap();
    let before = std::fs::read(root.path().join("devices").join(format!("{key}.toml"))).unwrap();
    let job = store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    let id = job.record().operation_id.clone();
    drop(job);
    let reopened = PersistenceStore::new_with_root(root.path().into()).firmware_operations();
    let record = reopened.load().unwrap().unwrap();
    assert_eq!(record.stage, Stage::Armed);
    assert_eq!(record.operation_id, id);
    assert_eq!(record.restore_settings.cooling, snapshot().cooling);
    assert!(reopened.ensure_replay_allowed(&target()).is_err());
    assert_eq!(
        std::fs::read(root.path().join("devices").join(format!("{key}.toml"))).unwrap(),
        before
    );
}
#[test]
fn effects_marker_and_every_terminal_outcome_remain_quarantined_without_automatic_retry() {
    for phase in [Phase::NotStarted, Phase::Unknown, Phase::AwaitingReconnect] {
        let root = tempfile::tempdir().unwrap();
        let journal = Journal::new(root.path().into());
        let mut job = journal
            .arm(&target(), CoolingHold::PumpOnly, snapshot())
            .unwrap();
        if phase != Phase::NotStarted {
            job.effects_possible().unwrap();
            assert_eq!(
                Journal::new(root.path().into())
                    .load()
                    .unwrap()
                    .unwrap()
                    .stage,
                Stage::EffectsPossible
            );
        }
        job.finish(&outcome(phase)).unwrap();
        let record = Journal::new(root.path().into()).load().unwrap().unwrap();
        assert_eq!(record.progress.unwrap().phase, phase);
        assert!(journal
            .ensure_replay_allowed(&info("SN-B", "/mock/new"))
            .is_err());
        let unrelated = DeviceInfo::new(
            DeviceFamilyId::new("other"),
            0x0416,
            0x7398,
            None,
            "/mock/other".into(),
        );
        assert!(journal.ensure_replay_allowed(&unrelated).is_ok());
    }
}
#[test]
fn atomic_first_publication_has_one_winner_across_independent_store_instances() {
    let root = tempfile::tempdir().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let mut jobs = vec![];
        for _ in 0..2 {
            let b = barrier.clone();
            let path = root.path().to_path_buf();
            jobs.push(scope.spawn(move || {
                b.wait();
                Journal::new(path)
                    .arm(&target(), CoolingHold::PumpOnly, snapshot())
                    .is_ok()
            }));
        }
        jobs.into_iter()
            .map(|j| j.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|v| **v).count(), 1);
    assert!(Journal::new(root.path().into()).load().unwrap().is_some());
}
#[test]
fn malformed_future_and_inconsistent_records_are_preserved_and_never_allow_replay() {
    let root = tempfile::tempdir().unwrap();
    let journal = Journal::new(root.path().into());
    let job = journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    let original = serde_json::to_value(job.record()).unwrap();
    drop(job);
    let mut future = original.clone();
    future["version"] = 2.into();
    let mut binding = original.clone();
    binding["target"]["settings_key"] = "wrong".into();
    let mut unknown = original.clone();
    unknown["future-field"] = true.into();
    let mut invalid_progress = original.clone();
    invalid_progress["stage"] = "AwaitingReconnect".into();
    invalid_progress["progress"] = serde_json::to_value(Progress::default()).unwrap();
    for bytes in [
        b"broken json".to_vec(),
        serde_json::to_vec(&future).unwrap(),
        serde_json::to_vec(&binding).unwrap(),
        serde_json::to_vec(&unknown).unwrap(),
        serde_json::to_vec(&invalid_progress).unwrap(),
        vec![b'x'; 2 * 1024 * 1024 + 1],
    ] {
        std::fs::write(file(root.path()), &bytes).unwrap();
        assert!(journal.ensure_replay_allowed(&target()).is_err());
        assert!(journal
            .arm(&target(), CoolingHold::PumpOnly, snapshot())
            .is_err());
        assert_eq!(std::fs::read(file(root.path())).unwrap(), bytes);
    }
}
#[test]
fn redirected_file_or_root_and_external_edits_are_not_followed_or_overwritten() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let journal = Journal::new(root.path().into());
    let mut job = journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    std::fs::write(file(root.path()), b"external edit").unwrap();
    assert!(job.effects_possible().is_err());
    assert_eq!(std::fs::read(file(root.path())).unwrap(), b"external edit");
    let foreign = root.path().join("foreign");
    std::fs::write(&foreign, b"keep").unwrap();
    std::fs::remove_file(file(root.path())).unwrap();
    symlink(&foreign, file(root.path())).unwrap();
    assert!(journal.load().is_err());
    assert!(journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .is_err());
    assert_eq!(std::fs::read(&foreign).unwrap(), b"keep");
    let link = root.path().join("redirect");
    symlink(root.path(), &link).unwrap();
    assert!(Journal::new(link).load().is_err());
}
#[test]
fn invalid_target_missing_effects_marker_and_nonterminal_or_duplicate_start_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let journal = Journal::new(root.path().into());
    let invalid = DeviceInfo::new(
        DeviceFamilyId::new("other"),
        1,
        2,
        None,
        "/mock/other".into(),
    );
    assert!(journal
        .arm(&invalid, CoolingHold::PumpOnly, snapshot())
        .is_err());
    assert!(!file(root.path()).exists());
    let job = journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    assert!(job.finish(&outcome(Phase::AwaitingReconnect)).is_err());
    assert_eq!(journal.load().unwrap().unwrap().stage, Stage::Armed);
    std::fs::remove_file(file(root.path())).unwrap(); // fixture cleanup, not recovery API
    let mut job = journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    job.effects_possible().unwrap();
    assert!(job.effects_possible().is_err());
    assert!(job.finish(&outcome(Phase::Uploading)).is_err());
    assert_eq!(
        journal.load().unwrap().unwrap().stage,
        Stage::EffectsPossible
    );
    let other = tempfile::tempdir().unwrap();
    let journal = Journal::new(other.path().into());
    let mut job = journal
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    job.effects_possible().unwrap();
    assert!(job.finish(&outcome(Phase::NotStarted)).is_err());
    assert_eq!(
        journal.load().unwrap().unwrap().stage,
        Stage::EffectsPossible
    );
}
#[derive(Clone)]
struct Counting {
    mock: MockBackend,
    opens: Arc<AtomicUsize>,
    enumerations: Arc<AtomicUsize>,
}
impl HidBackend for Counting {
    fn enumerate(&self) -> llcontrol_hid::Result<Vec<DeviceInfo>> {
        self.enumerations.fetch_add(1, Ordering::SeqCst);
        self.mock.enumerate()
    }
    fn open(&self, info: &DeviceInfo) -> llcontrol_hid::Result<Box<dyn OpenHandle>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        self.mock.open(info)
    }
}
fn config() -> Config {
    let mut c = Config::default();
    c.poll.hotplug_ms = 20;
    c
}
async fn until(mut test: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !test() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn manager_never_opens_or_replays_for_unresolved_or_corrupt_journal_even_with_other_serial() {
    for phase in [
        None,
        Some(Phase::Unknown),
        Some(Phase::AwaitingReconnect),
        Some(Phase::NotStarted),
    ] {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
        let mut job = store
            .firmware_operations()
            .arm(&target(), CoolingHold::PumpOnly, snapshot())
            .unwrap();
        if let Some(phase) = phase {
            if phase != Phase::NotStarted {
                job.effects_possible().unwrap();
            }
            job.finish(&outcome(phase)).unwrap();
        }
        let mock = MockBackend::new();
        let changed = info("SN-B", "/mock/replug");
        mock.add_device(changed.clone());
        store
            .write(
                &llcontrol_daemon::persistence::settings_key(&changed),
                &snapshot(),
            )
            .unwrap();
        let backend = Counting {
            mock,
            opens: Arc::default(),
            enumerations: Arc::default(),
        };
        let handle = DeviceManager::new(
            Arc::new(backend.clone()),
            store,
            config(),
            Arc::new(Stats::new()),
        )
        .spawn()
        .await;
        until(|| backend.enumerations.load(Ordering::SeqCst) >= 2).await;
        assert!(handle.list().await.is_empty());
        assert_eq!(backend.opens.load(Ordering::SeqCst), 0);
        assert!(backend.mock.write_history(&changed).is_empty());
        handle.shutdown().await;
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::write(file(root.path()), b"malformed").unwrap();
    let backend = Counting {
        mock: MockBackend::new(),
        opens: Arc::default(),
        enumerations: Arc::default(),
    };
    backend.mock.add_device(target());
    let handle = DeviceManager::new(
        Arc::new(backend.clone()),
        Arc::new(PersistenceStore::new_with_root(root.path().into())),
        config(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    until(|| backend.enumerations.load(Ordering::SeqCst) >= 2).await;
    assert!(handle.list().await.is_empty());
    assert_eq!(backend.opens.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(file(root.path())).unwrap(), b"malformed");
    handle.shutdown().await;
}
#[tokio::test]
async fn publication_keeps_live_owner_but_blocks_replug_and_fresh_manager_restart() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let backend = Counting {
        mock: MockBackend::new(),
        opens: Arc::default(),
        enumerations: Arc::default(),
    };
    backend.mock.add_device(target());
    let handle = DeviceManager::new(
        Arc::new(backend.clone()),
        store.clone(),
        config(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    until(|| backend.opens.load(Ordering::SeqCst) == 1).await;
    let before = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let devices = handle.list().await;
            if devices.len() == 1 {
                break devices;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let _job = store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    let polls = backend.enumerations.load(Ordering::SeqCst);
    until(|| backend.enumerations.load(Ordering::SeqCst) >= polls + 2).await;
    assert_eq!(handle.list().await[0].id, before[0].id);
    backend.mock.remove_device(&target());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let replacement = info("SN-A", "/mock/newpath");
    backend.mock.add_device(replacement.clone());
    let polls = backend.enumerations.load(Ordering::SeqCst);
    until(|| backend.enumerations.load(Ordering::SeqCst) >= polls + 2).await;
    assert!(handle.list().await.is_empty());
    assert_eq!(backend.opens.load(Ordering::SeqCst), 1);
    assert!(backend.mock.write_history(&replacement).is_empty());
    handle.shutdown().await;
    let polls = backend.enumerations.load(Ordering::SeqCst);
    let restarted = DeviceManager::new(
        Arc::new(backend.clone()),
        Arc::new(PersistenceStore::new_with_root(root.path().into())),
        config(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    until(|| backend.enumerations.load(Ordering::SeqCst) >= polls + 2).await;
    assert!(restarted.list().await.is_empty());
    assert_eq!(backend.opens.load(Ordering::SeqCst), 1);
    restarted.shutdown().await;
}

#[test]
fn effects_anchor_preserves_saved_intent_and_blocks_normal_writes_after_journal_loss() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let key = llcontrol_daemon::persistence::settings_key(&target());
    store.write(&key, &snapshot()).unwrap();
    let mut job = store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    job.effects_possible().unwrap();
    let saved = store.read(&key).unwrap();
    assert_eq!(saved.lcd.brightness, 47);
    assert_eq!(saved.cooling, snapshot().cooling);
    assert_eq!(
        saved.firmware_update_guard.as_ref().unwrap().operation_id,
        job.record().operation_id
    );
    let path = root.path().join("devices").join(format!("{key}.toml"));
    let bytes = std::fs::read(&path).unwrap();
    assert!(store.write(&key, &snapshot()).is_err());
    assert!(store.write(&key, &saved).is_err());
    let mut forged = snapshot();
    forged.firmware_update_guard = saved.firmware_update_guard.clone();
    assert!(store.write("other", &forged).is_err());
    std::fs::remove_file(file(root.path())).unwrap();
    drop(job);
    let reopened = PersistenceStore::new_with_root(root.path().into());
    assert!(reopened
        .firmware_operations()
        .ensure_replay_allowed(&info("CHANGED", "/mock/new"))
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(reopened.write(&key, &snapshot()).is_err());
    let latest = snapshot();
    let group = reopened
        .snapshot_profile_group(
            &key,
            &latest,
            llcontrol_daemon::profile_group::GroupKind::Screen,
        )
        .unwrap();
    assert!(reopened
        .commit_profile_group(&key, &latest, &group.revision, &group.profile)
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn anchor_failure_leaves_armed_and_prevents_effects_marker() {
    let root = tempfile::tempdir().unwrap();
    let mut job = Journal::new(root.path().into())
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    std::fs::write(root.path().join("devices"), b"not a directory").unwrap();
    assert!(job.effects_possible().is_err());
    assert_eq!(
        Journal::new(root.path().into())
            .load()
            .unwrap()
            .unwrap()
            .stage,
        Stage::Armed
    );
}

#[tokio::test]
async fn missing_journal_with_guard_still_never_opens_hid_after_restart_or_serial_change() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(PersistenceStore::new_with_root(root.path().into()));
    let mut job = store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    job.effects_possible().unwrap();
    std::fs::remove_file(file(root.path())).unwrap();
    drop(job);
    let changed = info("DIFFERENT", "/mock/changed");
    let mock = MockBackend::new();
    mock.add_device(changed.clone());
    let backend = Counting {
        mock,
        opens: Arc::default(),
        enumerations: Arc::default(),
    };
    let handle = DeviceManager::new(
        Arc::new(backend.clone()),
        store,
        config(),
        Arc::new(Stats::new()),
    )
    .spawn()
    .await;
    until(|| backend.enumerations.load(Ordering::SeqCst) >= 2).await;
    assert!(handle.list().await.is_empty());
    assert_eq!(backend.opens.load(Ordering::SeqCst), 0);
    assert!(backend.mock.write_history(&changed).is_empty());
    handle.shutdown().await;
}

// Pre-guard DeviceSettings schema, retained to exercise downgrade rejection.
fn legacy_schema_version() -> u32 {
    1
}
#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDeviceSettings {
    #[serde(default = "legacy_schema_version")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_reset_receipt: Option<llcontrol_ipc::ScreenResetReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_imports: Vec<llcontrol_daemon::profile_group::GroupImportReceipt>,
    /// None is the pre-collections shared catalog, not an empty collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_collections: Option<llcontrol_ipc::ScreenCollections>,
    #[serde(default)]
    pub fan: llcontrol_daemon::persistence::FanSection,
    #[serde(default)]
    pub pump: llcontrol_daemon::persistence::PumpSection,
    #[serde(default)]
    pub lighting: llcontrol_daemon::persistence::LightingSection,
    #[serde(default)]
    pub lcd: llcontrol_daemon::persistence::LcdSection,
    #[serde(default)]
    pub cooling: llcontrol_ipc::CoolingConfig,
    #[serde(default)]
    pub thermal: llcontrol_ipc::ThermalConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_rgb: Option<llcontrol_ipc::DynamicRgbConfig>,
    #[serde(
        default,
        skip_serializing_if = "llcontrol_ipc::DynamicRgbProfiles::is_empty"
    )]
    pub dynamic_rgb_profiles: llcontrol_ipc::DynamicRgbProfiles,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<llcontrol_ipc::LcdTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_clip: Option<String>,
}
#[test]
fn previous_settings_reader_accepts_unguarded_but_rejects_guarded_device_file() {
    let root = tempfile::tempdir().unwrap();
    let store = PersistenceStore::new_with_root(root.path().into());
    let key = llcontrol_daemon::persistence::settings_key(&target());
    store.write(&key, &snapshot()).unwrap();
    let path = root.path().join("devices").join(format!("{key}.toml"));
    let text = std::fs::read_to_string(&path).unwrap();
    toml::from_str::<LegacyDeviceSettings>(&text).unwrap();
    let mut job = store
        .firmware_operations()
        .arm(&target(), CoolingHold::PumpOnly, snapshot())
        .unwrap();
    job.effects_possible().unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let err = toml::from_str::<LegacyDeviceSettings>(&text)
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("unknown field `firmware_update_guard`"),
        "{err}"
    );
}
