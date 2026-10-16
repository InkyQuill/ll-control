//! Serialized, durable profile edits. Call blocking methods off the async executor.
use anyhow::{bail, ensure, Context};
use lconnect_ipc::{ProfileEdit, ProfileLibrary};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_LIBRARY_BYTES: u64 = 1024 * 1024;

/// One shared store per daemon; clones share the edit lock even after RPC cancellation.
#[derive(Clone)]
pub struct ProfileStore {
    root: PathBuf,
    state: crate::state_transaction::StateFiles,
}
impl ProfileStore {
    pub fn new(root: PathBuf) -> Self {
        let state = crate::state_transaction::StateFiles::new(root.clone());
        Self { root, state }
    }

    pub(crate) fn with_state(root: PathBuf, state: crate::state_transaction::StateFiles) -> Self {
        Self { root, state }
    }

    pub(crate) fn read_locked(&self) -> anyhow::Result<ProfileLibrary> {
        let path = self.root.join("profiles.json");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProfileLibrary::default())
            }
            Err(e) => return Err(e.into()),
        };
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "profile library must be a regular file"
        );
        ensure!(
            metadata.len() <= MAX_LIBRARY_BYTES,
            "profile library exceeds size limit"
        );
        let file = fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(MAX_LIBRARY_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_LIBRARY_BYTES,
            "profile library exceeds size limit"
        );
        let library: ProfileLibrary = serde_json::from_slice(&bytes)?;
        library.validate()?;
        Ok(library)
    }

    pub fn list(&self) -> anyhow::Result<ProfileLibrary> {
        let _guard = self.state.access()?;
        self.read_locked()
    }

    /// Finish an ambiguous rename's durability before acknowledging its receipt.
    pub(crate) fn recover_import(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<(lconnect_ipc::ArchiveImportReceipt, ProfileLibrary)>> {
        let _guard = self.state.access()?;
        let library = self.read_locked()?;
        let Some(receipt) = library.archive_imports.iter().find(|r| r.id == id).cloned() else {
            return Ok(None);
        };
        self.sync_library()?;
        Ok(Some((receipt, library)))
    }
    fn sync_library(&self) -> anyhow::Result<()> {
        fs::File::open(self.root.join("profiles.json"))?.sync_all()?;
        fs::File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    /// A stale editor must reload instead of silently overwriting a newer edit.
    /// Invalid/corrupt/unknown-version files are never replaced with empty defaults.
    pub fn edit(
        &self,
        expected_revision: u64,
        edit: ProfileEdit,
    ) -> anyhow::Result<ProfileLibrary> {
        self.edit_with_receipt(expected_revision, edit, None)
    }

    pub(crate) fn edit_import(
        &self,
        expected_revision: u64,
        profile: lconnect_ipc::NamedProfile,
        receipt: lconnect_ipc::ArchiveImportReceipt,
    ) -> anyhow::Result<ProfileLibrary> {
        ensure!(
            receipt.expected_revision == expected_revision
                && receipt.group == profile.content.group()
                && receipt.slot == profile.slot,
            "archive receipt differs from imported slot"
        );
        self.edit_with_receipt(
            expected_revision,
            ProfileEdit::Save { profile },
            Some(receipt),
        )
    }
    fn edit_with_receipt(
        &self,
        expected_revision: u64,
        edit: ProfileEdit,
        receipt: Option<lconnect_ipc::ArchiveImportReceipt>,
    ) -> anyhow::Result<ProfileLibrary> {
        edit.validate()?;
        let _guard = self.state.access()?;
        let mut library = self.read_locked()?;
        if let Some(receipt) = &receipt {
            if let Some(old) = library.archive_imports.iter().find(|r| r.id == receipt.id) {
                ensure!(
                    old == receipt,
                    "archive operation id already has different parameters"
                );
                self.sync_library()?;
                return Ok(library);
            }
        }
        ensure!(
            library.revision == expected_revision,
            "profile library changed; reload before editing"
        );
        let edit = match edit {
            ProfileEdit::Reset {
                group,
                slot,
                sensor_id,
            } => ProfileEdit::Save {
                profile: lconnect_ipc::NamedProfile::defaults(group, slot, sensor_id.as_deref())?,
            },
            other => other,
        };
        match edit {
            ProfileEdit::ResetGroup { group, sensor_id } => {
                // Build and validate the complete replacement before removing anything.
                let defaults = (0..group.slot_count())
                    .map(|slot| {
                        lconnect_ipc::NamedProfile::defaults(group, slot, sensor_id.as_deref())
                    })
                    .collect::<lconnect_ipc::Result<Vec<_>>>()?;
                library
                    .profiles
                    .retain(|profile| profile.content.group() != group);
                library.profiles.extend(defaults);
            }
            ProfileEdit::Reset { .. } => unreachable!("reset was normalized to save"),
            ProfileEdit::Save { profile } => {
                if let Some(old) = library.profiles.iter_mut().find(|p| {
                    p.slot == profile.slot && p.content.group() == profile.content.group()
                }) {
                    *old = profile;
                } else {
                    library.profiles.push(profile);
                }
            }
            ProfileEdit::Rename { group, slot, name } => {
                let profile = library
                    .profiles
                    .iter_mut()
                    .find(|p| p.slot == slot && p.content.group() == group)
                    .context("profile slot is empty")?;
                profile.name = name;
            }
            ProfileEdit::Delete { group, slot } => {
                let Some(index) = library
                    .profiles
                    .iter()
                    .position(|p| p.slot == slot && p.content.group() == group)
                else {
                    bail!("profile slot is empty")
                };
                library.profiles.remove(index);
            }
        }
        library.revision = library
            .revision
            .checked_add(1)
            .context("profile revision exhausted")?;
        if let Some(receipt) = receipt {
            ensure!(
                receipt.committed_revision == library.revision,
                "archive receipt revision mismatch"
            );
            if library.archive_imports.len() == 64 {
                library.archive_imports.remove(0);
            }
            library.archive_imports.push(receipt);
        }
        library.validate()?;
        let bytes = serde_json::to_vec_pretty(&library)?;
        ensure!(
            bytes.len() as u64 <= MAX_LIBRARY_BYTES,
            "profile library exceeds size limit"
        );
        persist_library(&self.root, &bytes, |_| Ok(()))?;
        Ok(library)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteStage {
    Written,
    FileSynced,
    Renamed,
}

// The observer permits deterministic I/O-failure tests without changing process
// permissions or racing filesystem operations. Production always uses a no-op.
fn persist_library(
    root: &Path,
    bytes: &[u8],
    mut observe: impl FnMut(WriteStage) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    file.write_all(bytes)?;
    observe(WriteStage::Written)?;
    file.as_file().sync_all()?;
    observe(WriteStage::FileSynced)?;
    file.persist(root.join("profiles.json"))?;
    observe(WriteStage::Renamed)?;
    // Failure here is ambiguous: rename has already committed. Read back before retry.
    fs::File::open(root)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lconnect_ipc::{NamedProfile, ProfileGroup};

    #[test]
    fn interrupted_atomic_write_preserves_old_or_committed_library_and_recovers() {
        for stage in [
            WriteStage::Written,
            WriteStage::FileSynced,
            WriteStage::Renamed,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let store = ProfileStore::new(dir.path().into());
            let before = store
                .edit(
                    0,
                    ProfileEdit::Save {
                        profile: NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap(),
                    },
                )
                .unwrap();
            let mut committed = before.clone();
            committed.revision += 1;
            committed.profiles[0].name = "Committed rename".into();
            let failure =
                persist_library(dir.path(), &serde_json::to_vec(&committed).unwrap(), |at| {
                    anyhow::ensure!(at != stage, "injected failure at {at:?}");
                    Ok(())
                });
            assert!(failure.is_err(), "{stage:?}");
            let reopened = ProfileStore::new(dir.path().into());
            let expected = if stage == WriteStage::Renamed {
                &committed
            } else {
                &before
            };
            assert_eq!(&reopened.list().unwrap(), expected, "{stage:?}");
            assert_eq!(
                fs::read_dir(dir.path()).unwrap().count(),
                1,
                "temporary file leaked at {stage:?}"
            );
            if stage == WriteStage::Renamed {
                assert!(reopened
                    .edit(
                        before.revision,
                        ProfileEdit::Delete {
                            group: ProfileGroup::Screen,
                            slot: 0
                        }
                    )
                    .is_err());
                assert_eq!(reopened.list().unwrap(), committed);
            }
            let recovered = reopened
                .edit(
                    expected.revision,
                    ProfileEdit::Rename {
                        group: ProfileGroup::Screen,
                        slot: 0,
                        name: "Explicit recovery".into(),
                    },
                )
                .unwrap();
            assert_eq!(recovered.revision, expected.revision + 1);
            assert_eq!(recovered.profiles[0].name, "Explicit recovery");
            assert_eq!(
                ProfileStore::new(dir.path().into()).list().unwrap(),
                recovered
            );
        }
    }
    #[test]
    fn import_receipt_and_slot_commit_together_through_all_atomic_write_failures() {
        for stage in [
            WriteStage::Written,
            WriteStage::FileSynced,
            WriteStage::Renamed,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let store = ProfileStore::new(dir.path().into());
            let before = store
                .edit(
                    0,
                    ProfileEdit::Save {
                        profile: NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap(),
                    },
                )
                .unwrap();
            let mut after = before.clone();
            after.revision += 1;
            after.profiles[0].name = "Imported".into();
            let receipt = lconnect_ipc::ArchiveImportReceipt {
                source_sha256: None,
                id: "a".repeat(32),
                size: 100,
                expected_revision: before.revision,
                committed_revision: after.revision,
                group: ProfileGroup::Screen,
                slot: 0,
            };
            after.archive_imports.push(receipt.clone());
            after.validate().unwrap();
            assert!(
                persist_library(dir.path(), &serde_json::to_vec(&after).unwrap(), |at| {
                    anyhow::ensure!(at != stage, "injected {at:?}");
                    Ok(())
                })
                .is_err()
            );
            let reopened = ProfileStore::new(dir.path().into());
            assert_eq!(
                reopened.list().unwrap(),
                if stage == WriteStage::Renamed {
                    after.clone()
                } else {
                    before
                }
            );
            let acknowledged = reopened.recover_import(&receipt.id).unwrap();
            assert_eq!(
                acknowledged.map(|(_, library)| library),
                if stage == WriteStage::Renamed {
                    Some(after.clone())
                } else {
                    None
                }
            );
            let actual = reopened
                .edit_import(
                    receipt.expected_revision,
                    after.profiles[0].clone(),
                    receipt.clone(),
                )
                .unwrap();
            assert_eq!(actual, after);
            assert_eq!(
                actual,
                reopened
                    .edit_import(
                        receipt.expected_revision,
                        after.profiles[0].clone(),
                        receipt
                    )
                    .unwrap()
            );
        }
    }
    #[test]
    fn import_receipt_window_is_bounded_and_retries_do_not_resurrect_deleted_slots() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().into());
        let profile = NamedProfile::defaults(ProfileGroup::Screen, 0, None).unwrap();
        let receipt = |n: u64| lconnect_ipc::ArchiveImportReceipt {
            source_sha256: None,
            id: format!("{:032x}", n + 1),
            size: 100,
            expected_revision: n,
            committed_revision: n + 1,
            group: ProfileGroup::Screen,
            slot: 0,
        };
        for n in 0..65 {
            store.edit_import(n, profile.clone(), receipt(n)).unwrap();
        }
        let library = store.list().unwrap();
        assert_eq!(library.archive_imports.len(), 64);
        assert_eq!(library.archive_imports[0].id, format!("{:032x}", 2));
        assert!(
            store.edit_import(0, profile.clone(), receipt(0)).is_err(),
            "forgotten old intent still fails stale revision"
        );
        let removed = store
            .edit(
                65,
                ProfileEdit::Delete {
                    group: ProfileGroup::Screen,
                    slot: 0,
                },
            )
            .unwrap();
        assert!(removed.profiles.is_empty());
        assert_eq!(
            removed,
            store.edit_import(64, profile, receipt(64)).unwrap()
        );
    }
}
