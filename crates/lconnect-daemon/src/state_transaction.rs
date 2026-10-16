//! Shared serialization and redo recovery for settings + profile transactions.
//! A published journal is committed intent. Readers finish it before exposing
//! either file; external edits that match neither snapshot fail closed.
use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

const JOURNAL: &str = "screen-reset-transaction.json";
const MAX_FILE: u64 = 1024 * 1024;
const MAX_JOURNAL: u64 = 8 * MAX_FILE;

#[derive(Clone)]
pub(crate) struct StateFiles {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transaction {
    version: u32,
    device_key: String,
    device_before: Option<String>,
    device_after: String,
    profiles_before: Option<String>,
    profiles_after: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Journal,
    Device,
    Profiles,
}

impl StateFiles {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::default(),
        }
    }

    /// Read-only verification must never trigger redo recovery implicitly.
    pub(crate) fn ensure_no_pending_transaction(&self) -> anyhow::Result<()> {
        check_directory(&self.root)?;
        ensure!(
            read_optional(&self.root.join(JOURNAL), MAX_JOURNAL)?.is_none(),
            "pending settings/profile transaction; read-only firmware probe withheld"
        );
        Ok(())
    }

    /// Inspection shares the state lock but never performs redo recovery.
    pub(crate) fn readonly_access(&self) -> anyhow::Result<MutexGuard<'_, ()>> {
        let guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("state store lock poisoned"))?;
        self.ensure_no_pending_transaction()?;
        Ok(guard)
    }

    /// Hold this guard through reads/writes. Never nest access on the same store.
    pub(crate) fn access(&self) -> anyhow::Result<MutexGuard<'_, ()>> {
        let guard = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("state store lock poisoned"))?;
        check_directory(&self.root)?;
        self.recover(|_| Ok(()))?;
        Ok(guard)
    }

    /// Caller owns access guard and has validated both complete new snapshots.
    pub(crate) fn commit(
        &self,
        key: &str,
        settings: &crate::persistence::DeviceSettings,
        profiles: &lconnect_ipc::ProfileLibrary,
    ) -> anyhow::Result<()> {
        self.commit_observed(key, settings, profiles, |_| Ok(()))
    }

    fn commit_observed(
        &self,
        key: &str,
        settings: &crate::persistence::DeviceSettings,
        profiles: &lconnect_ipc::ProfileLibrary,
        mut observe: impl FnMut(Stage) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        validate_key(key)?;
        ensure!(
            settings.firmware_update_guard.is_none(),
            "firmware guard forbids profile transaction"
        );
        if let Some(text) = read_optional(&self.device_path(key), MAX_FILE)? {
            let saved: crate::persistence::DeviceSettings = toml::from_str(&text)?;
            ensure!(
                saved.firmware_update_guard.is_none(),
                "firmware guard forbids profile transaction"
            );
        }
        let transaction = Transaction {
            version: 1,
            device_key: key.into(),
            device_before: read_optional(&self.device_path(key), MAX_FILE)?,
            device_after: toml::to_string_pretty(settings)?,
            profiles_before: read_optional(&self.root.join("profiles.json"), MAX_FILE)?,
            profiles_after: serde_json::to_string_pretty(profiles)?,
        };
        transaction.validate()?;
        let journal = serde_json::to_vec_pretty(&transaction)?;
        ensure!(
            journal.len() as u64 <= MAX_JOURNAL,
            "screen reset journal exceeds size limit"
        );
        atomic_write(&self.root.join(JOURNAL), &journal)?;
        observe(Stage::Journal)?;
        self.recover(observe)
    }

    fn device_path(&self, key: &str) -> PathBuf {
        self.root.join("devices").join(format!("{key}.toml"))
    }

    fn recover(&self, mut observe: impl FnMut(Stage) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let Some(journal) = read_optional(&self.root.join(JOURNAL), MAX_JOURNAL)? else {
            return Ok(());
        };
        let transaction: Transaction = serde_json::from_str(&journal)
            .context("invalid screen reset journal; original preserved")?;
        transaction.validate()?;
        let device = self.device_path(&transaction.device_key);
        let profiles = self.root.join("profiles.json");
        // Check both before writing either; a concurrent external edit is never
        // silently replaced, including after one file has already committed.
        check_snapshot(
            &device,
            &transaction.device_before,
            &transaction.device_after,
        )?;
        check_snapshot(
            &profiles,
            &transaction.profiles_before,
            &transaction.profiles_after,
        )?;
        atomic_write(&device, transaction.device_after.as_bytes())?;
        observe(Stage::Device)?;
        atomic_write(&profiles, transaction.profiles_after.as_bytes())?;
        observe(Stage::Profiles)?;
        fs::remove_file(self.root.join(JOURNAL))?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }
}

impl Transaction {
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.version == 1,
            "unsupported screen reset journal version"
        );
        validate_key(&self.device_key)?;
        for text in [&self.device_after, &self.profiles_after]
            .into_iter()
            .chain(self.device_before.iter())
            .chain(self.profiles_before.iter())
        {
            ensure!(
                text.len() as u64 <= MAX_FILE,
                "screen reset snapshot exceeds size limit"
            );
        }
        let settings = toml::from_str(&self.device_after)?;
        crate::persistence::validate_schema(&settings)?;
        ensure!(
            settings.firmware_update_guard.is_none(),
            "profile recovery cannot write firmware guard"
        );
        if let Some(before) = &self.device_before {
            let saved: crate::persistence::DeviceSettings = toml::from_str(before)?;
            ensure!(
                saved.firmware_update_guard.is_none(),
                "profile recovery cannot replace firmware guard"
            );
        }
        let profiles: lconnect_ipc::ProfileLibrary = serde_json::from_str(&self.profiles_after)?;
        profiles.validate()?;
        ensure!(
            settings
                .group_imports
                .iter()
                .all(|r| r.profile_revision <= profiles.revision),
            "group import receipt refers to a missing profile revision"
        );
        Ok(())
    }
}

pub(crate) fn validate_key(key: &str) -> anyhow::Result<()> {
    ensure!(
        !key.is_empty()
            && key.len() <= 256
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b)),
        "invalid device settings key"
    );
    Ok(())
}

fn check_snapshot(path: &Path, before: &Option<String>, after: &str) -> anyhow::Result<()> {
    let current = read_optional(path, MAX_FILE)?;
    ensure!(
        current == *before || current.as_deref() == Some(after),
        "screen reset recovery conflict at {}; file preserved",
        path.display()
    );
    Ok(())
}

pub(crate) fn read_optional(path: &Path, max: u64) -> anyhow::Result<Option<String>> {
    if let Some(parent) = path.parent() {
        check_directory(parent)?;
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "state file must be regular"
    );
    ensure!(metadata.len() <= max, "state file exceeds size limit");
    let mut text = String::new();
    fs::File::open(path)?
        .take(max + 1)
        .read_to_string(&mut text)?;
    ensure!(text.len() as u64 <= max, "state file exceeds size limit");
    Ok(Some(text))
}

pub(crate) fn check_directory(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "state directory must be regular"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("missing state parent")?;
    // Reject a redirected state directory. Ancestors are the trusted config root.
    check_directory(parent)?;
    fs::create_dir_all(parent)?;
    if let Some(ancestor) = parent.parent() {
        fs::File::open(ancestor)?.sync_all()?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{DeviceSettings, PersistenceStore};
    use lconnect_ipc::{NamedProfile, ProfileEdit, ProfileGroup};

    fn snapshots(root: &Path) -> (DeviceSettings, lconnect_ipc::ProfileLibrary) {
        let store = PersistenceStore::new_with_root(root.into());
        let mut settings = DeviceSettings::default();
        settings.fan.pwm_percent = 63;
        store.write("device", &settings).unwrap();
        let mut library = store
            .profiles()
            .edit(
                0,
                ProfileEdit::Reset {
                    group: ProfileGroup::Fan,
                    slot: 1,
                    sensor_id: Some("cpu:temp".into()),
                },
            )
            .unwrap();
        settings.lcd.brightness = 50;
        settings.lcd.rotation = "0".into();
        library.revision += 1;
        library.profiles.extend(
            (0..8).map(|slot| NamedProfile::defaults(ProfileGroup::Screen, slot, None).unwrap()),
        );
        (settings, library)
    }

    #[test]
    fn every_committed_interruption_recovers_both_files_before_readers_proceed() {
        for stage in [Stage::Journal, Stage::Device, Stage::Profiles] {
            for profiles_first in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let (settings, library) = snapshots(dir.path());
                let files = StateFiles::new(dir.path().into());
                let guard = files.access().unwrap();
                let failure = files.commit_observed("device", &settings, &library, |at| {
                    ensure!(at != stage, "injected interruption");
                    Ok(())
                });
                assert!(failure.is_err());
                assert!(dir.path().join(JOURNAL).exists());
                drop(guard);
                // Fresh store/gate models daemon restart, not merely an in-memory retry.
                let reopened = PersistenceStore::new_with_root(dir.path().into());
                if profiles_first {
                    assert_eq!(reopened.profiles().list().unwrap(), library);
                }
                let recovered = reopened.read("device").unwrap();
                assert_eq!(recovered.lcd.brightness, 50);
                assert_eq!(recovered.fan.pwm_percent, 63);
                assert_eq!(reopened.profiles().list().unwrap(), library);
                assert!(!dir.path().join(JOURNAL).exists());
                assert_eq!(reopened.read("device").unwrap().lcd.brightness, 50);
            }
        }
    }

    #[test]
    fn external_edit_after_interruption_blocks_recovery_and_preserves_all_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, library) = snapshots(dir.path());
        let files = StateFiles::new(dir.path().into());
        let guard = files.access().unwrap();
        assert!(files
            .commit_observed("device", &settings, &library, |at| {
                ensure!(at != Stage::Device, "interruption");
                Ok(())
            })
            .is_err());
        drop(guard);
        let profiles_path = dir.path().join("profiles.json");
        fs::write(&profiles_path, "external edit").unwrap();
        let device_path = dir.path().join("devices/device.toml");
        let before = fs::read(&device_path).unwrap();
        let reopened = PersistenceStore::new_with_root(dir.path().into());
        assert!(reopened
            .read("device")
            .unwrap_err()
            .to_string()
            .contains("recovery conflict"));
        assert!(reopened.profiles().list().is_err());
        assert_eq!(fs::read_to_string(profiles_path).unwrap(), "external edit");
        assert_eq!(fs::read(device_path).unwrap(), before);
        assert!(dir.path().join(JOURNAL).exists());
    }

    #[test]
    fn malformed_or_future_journal_is_never_discarded() {
        let dir = tempfile::tempdir().unwrap();
        for text in ["broken", r#"{"version":999}"#] {
            fs::write(dir.path().join(JOURNAL), text).unwrap();
            let reopened = PersistenceStore::new_with_root(dir.path().into());
            assert!(reopened.read("device").is_err());
            assert!(reopened.profiles().list().is_err());
            assert_eq!(fs::read_to_string(dir.path().join(JOURNAL)).unwrap(), text);
            assert!(!dir.path().join("profiles.json").exists());
        }
    }
    #[test]
    fn profile_group_import_recovers_whole_group_at_each_journal_boundary() {
        use crate::profile_group::{GroupKind, GroupProfile};
        for kind in [GroupKind::Screen, GroupKind::FanAndLighting] {
            for stage in [Stage::Journal, Stage::Device, Stage::Profiles] {
                let dir = tempfile::tempdir().unwrap();
                let store = PersistenceStore::new_with_root(dir.path().into());
                let mut target = DeviceSettings::default();
                target.fan.pwm_percent = 41;
                target.lcd.brightness = 22;
                store.write("device", &target).unwrap();
                let mut source = DeviceSettings::default();
                source.fan.pwm_percent = 79;
                source.lcd.brightness = 83;
                if kind == GroupKind::Screen {
                    source.screen_collections = Some(lconnect_ipc::ScreenCollections::defaults(1));
                }
                let group = if kind == GroupKind::Screen {
                    ProfileGroup::Screen
                } else {
                    ProfileGroup::Fan
                };
                let library = lconnect_ipc::ProfileLibrary {
                    profiles: vec![NamedProfile::defaults(
                        group,
                        3,
                        (group != ProfileGroup::Screen).then_some("cpu:temp"),
                    )
                    .unwrap()],
                    ..Default::default()
                };
                let imported = GroupProfile::capture(&source, &library, kind)
                    .unwrap()
                    .profile;
                let current = store
                    .snapshot_profile_group("device", &target, kind)
                    .unwrap();
                let (mut settings, profiles) = imported
                    .restore(
                        &target,
                        &store.profiles().list().unwrap(),
                        &current.revision,
                    )
                    .unwrap();
                let content = GroupProfile::capture(&settings, &profiles, kind).unwrap();
                settings
                    .group_imports
                    .push(crate::profile_group::GroupImportReceipt {
                        id: "e".repeat(32),
                        kind,
                        source_sha256: "a".repeat(64),
                        expected: current.revision,
                        profile_revision: profiles.revision,
                        collection_revision: if kind == GroupKind::Screen {
                            settings.screen_collections.as_ref().map(|c| c.revision)
                        } else {
                            None
                        },
                        content_sha256: content.revision.settings_sha256,
                    });
                let files = StateFiles::new(dir.path().into());
                let guard = files.access().unwrap();
                assert!(files
                    .commit_observed("device", &settings, &profiles, |at| {
                        ensure!(at != stage, "group import interruption");
                        Ok(())
                    })
                    .is_err());
                drop(guard);
                let restarted = PersistenceStore::new_with_root(dir.path().into());
                let readback = restarted.read("device").unwrap();
                assert_eq!(
                    serde_json::to_value(&readback).unwrap(),
                    serde_json::to_value(&settings).unwrap()
                );
                assert_eq!(restarted.profiles().list().unwrap(), profiles);
                let acknowledged = restarted
                    .read_group_import("device", &readback, &"e".repeat(32))
                    .unwrap()
                    .unwrap();
                assert!(acknowledged.selected_content_current);
                assert_eq!(acknowledged.receipt.profile_revision, profiles.revision);
                assert_eq!(
                    readback.fan.pwm_percent,
                    if kind == GroupKind::Screen { 41 } else { 79 }
                );
                assert_eq!(
                    readback.lcd.brightness,
                    if kind == GroupKind::Screen { 83 } else { 22 }
                );
                assert!(!dir.path().join(JOURNAL).exists());
            }
        }
    }
}
