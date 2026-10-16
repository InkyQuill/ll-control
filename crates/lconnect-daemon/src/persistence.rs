//! Per-device persistence (Hybrid C from spec section 6).
//!
//! Small device settings (fan/pump PWM, lighting state, LCD config) are
//! persisted as TOML under `$XDG_CONFIG_HOME/lconnectd/devices/<key>.toml`.
//! LCD JPEG bytes are intentionally **not** persisted — they're regenerated
//! per frame upload.
//!
//! Writes are atomic: write to a `.toml.tmp` sibling, fsync, then `rename`
//! over the live file. Unsupported or malformed settings are rejected on read
//! and protected from replacement on write; downgrade never resets them silently.
//!
//! The debounce wrapper lives at the daemon level and lands in P1D-12;
//! replay-on-spawn lands in P1D-15.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

const CURRENT_SCHEMA: u32 = 1;
const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSettings {
    /// Durable fail-closed anchor; ordinary persistence cannot create or clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware_update_guard: Option<crate::firmware_operation::SettingsGuard>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_reset_receipt: Option<ScreenResetReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_imports: Vec<crate::profile_group::GroupImportReceipt>,
    /// None is the pre-collections shared catalog, not an empty collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_collections: Option<lconnect_ipc::ScreenCollections>,
    #[serde(default)]
    pub fan: FanSection,
    #[serde(default)]
    pub pump: PumpSection,
    #[serde(default)]
    pub lighting: LightingSection,
    #[serde(default)]
    pub lcd: LcdSection,
    #[serde(default)]
    pub cooling: lconnect_ipc::CoolingConfig,
    #[serde(default)]
    pub thermal: lconnect_ipc::ThermalConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_rgb: Option<lconnect_ipc::DynamicRgbConfig>,
    #[serde(
        default,
        skip_serializing_if = "lconnect_ipc::DynamicRgbProfiles::is_empty"
    )]
    pub dynamic_rgb_profiles: lconnect_ipc::DynamicRgbProfiles,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<lconnect_ipc::LcdTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_clip: Option<String>,
}

pub use lconnect_ipc::ScreenResetReceipt;

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            firmware_update_guard: None,
            schema_version: CURRENT_SCHEMA,
            screen_reset_receipt: None,
            group_imports: Vec::new(),
            screen_collections: None,
            fan: FanSection::default(),
            pump: PumpSection::default(),
            lighting: LightingSection::default(),
            lcd: LcdSection::default(),
            cooling: Default::default(),
            thermal: Default::default(),
            dynamic_rgb: None,
            dynamic_rgb_profiles: Default::default(),
            theme: None,
            media_clip: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FanSection {
    pub pwm_percent: u8,
    /// Legacy0 without this marker means no saved manual preference.
    #[serde(skip_serializing_if = "is_false")]
    pub manual_zero: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
impl FanSection {
    pub fn has_manual_setting(&self) -> bool {
        self.pwm_percent != 0 || self.manual_zero
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PumpSection {
    pub pwm_percent: u8,
    /// "mcu" | "application" — empty means "no preference yet".
    pub source: String,
}

impl PumpSection {
    pub fn has_manual_setting(&self) -> bool {
        self.pwm_percent != 0
            || matches!(
                self.source.as_str(),
                "mcu" | "Mcu" | "application" | "Application"
            )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightingSection {
    pub fan_topology: lconnect_ipc::FanLightingTopology,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub fan_modes: std::collections::BTreeMap<String, LightingState>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub pump_modes: std::collections::BTreeMap<String, PumpLightingState>,
    pub fan: Option<LightingState>,
    pub pump: Option<PumpLightingState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightingState {
    /// Actor packet snapshot only; persisted once in LightingSection, never per mode.
    #[serde(skip)]
    pub fan_topology: Option<lconnect_ipc::FanLightingTopology>,
    pub mode: String,
    pub brightness: u8,
    pub speed: u8,
    pub direction: String,
    pub color: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub additional_colors: Vec<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PumpLightingState {
    pub scope: String,
    pub mode: String,
    pub brightness: u8,
    pub speed: u8,
    pub direction: String,
    pub color: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub additional_colors: Vec<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LcdSection {
    pub brightness: u8,
    pub rotation: String,
    pub control_mode: String,
}

enum ReferenceKind {
    Image,
    Font,
    Media,
}

#[derive(Clone)]
pub struct PersistenceStore {
    admission: crate::admission::Admission,
    display: crate::display_settings::DisplaySettingsStore,
    fps: crate::fps_source::FpsSourceStore,
    state: crate::state_transaction::StateFiles,
    root: PathBuf, // typically $XDG_CONFIG_HOME/lconnectd
    assets: crate::theme_assets::ThemeAssetStore,
    media: crate::media_assets::MediaAssetStore,
    fonts: crate::theme_fonts::ThemeFontStore,
}

impl PersistenceStore {
    /// Production constructor: roots at `$XDG_CONFIG_HOME/lconnectd`.
    ///
    /// # Errors
    /// Returns Err when the XDG base-directory lookup fails.
    pub fn new() -> anyhow::Result<Self> {
        let dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        Self::open_with_root(dirs.get_config_home())
    }

    /// Open a store and reconcile abandoned imports before returning it.
    /// Recovery errors preserve the original files and prevent startup.
    pub fn open_with_root(root: PathBuf) -> anyhow::Result<Self> {
        let store = Self::new_with_root(root);
        store.recover_import_publications()?;
        Ok(store)
    }
    /// Low-level test/staging constructor without startup IO or recovery.
    /// Use open_with_root for a ready store, or gate catalogs/replay through
    /// recover_import_publications / DeviceManager before exposing this store.
    pub fn new_with_root(root: PathBuf) -> Self {
        let admission = crate::admission::Admission::default();
        Self {
            admission: admission.clone(),
            fps: crate::fps_source::FpsSourceStore::new(root.clone())
                .with_admission(admission.clone()),
            display: crate::display_settings::DisplaySettingsStore::new(root.clone())
                .with_admission(admission),
            state: crate::state_transaction::StateFiles::new(root.clone()),
            assets: crate::theme_assets::ThemeAssetStore::new(root.join("theme-assets")),
            fonts: crate::theme_fonts::ThemeFontStore::new(root.join("theme-fonts")),
            media: crate::media_assets::MediaAssetStore::new(root.join("prepared-clips")),
            root,
        }
    }

    /// Shared process-local admission for ordinary work; no product close route.
    pub fn admission(&self) -> crate::admission::Admission {
        self.admission.clone()
    }

    /// Configure the legacy environment fallback before cloning/starting services.
    /// A persisted record always takes precedence, including explicit disable.
    pub fn with_legacy_fps_log(mut self, path: Option<PathBuf>) -> Self {
        self.fps = self.fps.with_legacy_log(path);
        self
    }
    pub fn fps_source(&self) -> crate::fps_source::FpsSourceStore {
        self.fps.clone()
    }
    pub fn display_settings(&self) -> crate::display_settings::DisplaySettingsStore {
        self.display.clone()
    }

    pub fn firmware_operations(&self) -> crate::firmware_operation::Journal {
        crate::firmware_operation::Journal::for_store(self.root.clone(), self.clone())
    }

    pub(crate) fn publication_root(&self) -> PathBuf {
        self.root.join("import-publications")
    }
    /// Recover abandoned imports before replay/catalog access. Embedded callers
    /// should invoke this or start through DeviceManager, which gates replay.
    pub fn recover_import_publications(&self) -> anyhow::Result<Vec<ImportStartupNotice>> {
        let notices = crate::asset_publication::journal::recover(&Arc::new(self.clone()))?;
        for notice in &notices {
            tracing::warn!(?notice, "Import publication recovery retained files");
        }
        Ok(notices)
    }
    /// Read the durable history of conservative file-retention decisions.
    /// Current commit status remains authoritative in the owner's receipt.
    pub fn import_recovery_notices(&self) -> anyhow::Result<Vec<ImportRecoveryNotice>> {
        crate::asset_publication::journal::read_notices(&self.publication_root())
    }
    /// Retention history for global named-slot archive imports.
    pub fn slot_import_recovery_notices(&self) -> anyhow::Result<Vec<SlotImportRecoveryNotice>> {
        crate::asset_publication::journal::read_slot_notices(&self.publication_root())
    }
    /// Share prepared media admission between IPC clients.
    pub fn media_assets(&self) -> crate::media_assets::MediaAssetStore {
        self.media.clone()
    }

    /// Manager replay and IPC share the same asset lifetime protection.
    pub fn theme_assets(&self) -> crate::theme_assets::ThemeAssetStore {
        self.assets.clone()
    }

    /// Share font session retention between uploads, preview, profiles and replay.
    pub fn theme_fonts(&self) -> crate::theme_fonts::ThemeFontStore {
        self.fonts.clone()
    }

    /// Strict scan for font references, including offline device settings.
    pub fn referenced_theme_fonts(&self) -> anyhow::Result<std::collections::BTreeSet<String>> {
        self.referenced_keys(ReferenceKind::Font)
    }

    /// Strict scan for cleanup: never interpret unreadable/newer settings as empty.
    pub fn referenced_theme_assets(&self) -> anyhow::Result<std::collections::BTreeSet<String>> {
        self.referenced_keys(ReferenceKind::Image)
    }

    /// Strict media references including disconnected devices; unreadable state is an error.
    pub fn referenced_media_clips(&self) -> anyhow::Result<std::collections::BTreeSet<String>> {
        self.referenced_keys(ReferenceKind::Media)
    }

    fn referenced_keys(
        &self,
        kind: ReferenceKind,
    ) -> anyhow::Result<std::collections::BTreeSet<String>> {
        let _guard = self.state.access()?;
        use std::io::Read;
        let mut keys = std::collections::BTreeSet::new();
        let directory = self.root.join("devices");
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(keys),
            Err(e) => return Err(e.into()),
        };
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "settings root must be a regular directory"
        );
        for (index, entry) in std::fs::read_dir(directory)?.enumerate() {
            anyhow::ensure!(index < 4096, "too many settings files for cleanup");
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            anyhow::ensure!(
                entry.file_type()?.is_file(),
                "settings must be regular files for cleanup"
            );
            let mut text = String::new();
            std::fs::File::open(entry.path())?
                .take(1024 * 1024 + 1)
                .read_to_string(&mut text)?;
            anyhow::ensure!(
                text.len() <= 1024 * 1024,
                "settings exceed cleanup size limit"
            );
            let settings: DeviceSettings = toml::from_str(&text)?;
            anyhow::ensure!(
                settings.schema_version == CURRENT_SCHEMA,
                "unknown settings schema; cleanup refused"
            );
            if let Some(collections) = &settings.screen_collections {
                collections.validate()?;
                if matches!(kind, ReferenceKind::Media) {
                    keys.extend(collections.video_keys().into_iter().map(str::to_owned));
                } else if matches!(kind, ReferenceKind::Image) {
                    keys.extend(collections.image_keys().into_iter().map(str::to_owned));
                }
            }
            if matches!(kind, ReferenceKind::Media) {
                validate_schema(&settings)?;
                keys.extend(settings.media_clip);
                if let Some(theme) = settings.theme {
                    keys.extend(theme.video_assets().into_iter().map(str::to_owned));
                }
                continue;
            }
            if let Some(theme) = settings.theme {
                theme.validate()?;
                if matches!(kind, ReferenceKind::Font) {
                    keys.extend(theme.font_assets().into_iter().map(str::to_owned));
                } else {
                    keys.extend(theme.image_assets().into_iter().map(str::to_owned));
                }
            }
        }
        Ok(keys)
    }

    /// Asset directory shares the configured per-user persistence root.
    pub fn theme_assets_root(&self) -> PathBuf {
        self.root.join("theme-assets")
    }

    /// Shared named profiles live beside device settings and managed assets.
    pub fn profiles(&self) -> crate::profiles::ProfileStore {
        crate::profiles::ProfileStore::with_state(self.root.clone(), self.state.clone())
    }

    pub fn profiles_root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Font assets live separately from PNG assets and image cleanup.
    pub fn theme_fonts_root(&self) -> PathBuf {
        self.root.join("theme-fonts")
    }

    /// Global source choices share the configured settings root.
    pub fn sensor_defaults_root(&self) -> PathBuf {
        self.root.clone()
    }

    fn key_path(&self, key: &str) -> PathBuf {
        self.root.join("devices").join(format!("{key}.toml"))
    }

    /// Read v1 settings, including legacy files without an explicit version.
    /// Only an absent file supplies defaults; unsupported data is preserved.
    ///
    /// # Errors
    /// Unreadable, nonregular, oversized, malformed or unsupported settings.
    pub fn read(&self, key: &str) -> anyhow::Result<DeviceSettings> {
        let _guard = self.state.access()?;
        self.read_unlocked(key)
    }

    fn read_unlocked(&self, key: &str) -> anyhow::Result<DeviceSettings> {
        use std::io::Read;
        let path = self.key_path(key);
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DeviceSettings::default());
            }
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "settings must be a regular file"
        );
        anyhow::ensure!(
            metadata.len() <= MAX_SETTINGS_BYTES,
            "settings exceed size limit"
        );
        let mut text = String::new();
        std::fs::File::open(&path)?
            .take(MAX_SETTINGS_BYTES + 1)
            .read_to_string(&mut text)?;
        anyhow::ensure!(
            text.len() as u64 <= MAX_SETTINGS_BYTES,
            "settings exceed size limit"
        );
        let parsed: DeviceSettings = toml::from_str(&text)?;
        validate_schema(&parsed)?;
        self.validate_group_import_revisions(&parsed)?;
        Ok(parsed)
    }

    /// Capture the selected group and slots under the shared state gate.
    /// `latest` must come from the serialized settings owner, including queued deltas.
    pub fn snapshot_profile_group(
        &self,
        key: &str,
        latest: &DeviceSettings,
        kind: crate::profile_group::GroupKind,
    ) -> anyhow::Result<crate::profile_group::GroupSnapshot> {
        crate::state_transaction::validate_key(key)?;
        let _guard = self.state.access()?;
        self.read_unlocked(key)?;
        crate::profile_group::GroupProfile::capture(latest, &self.profiles().read_locked()?, kind)
    }

    /// Storage primitive for full-group import. Call only from the settings
    /// owner, after preparing/pinning referenced assets; reconcile live actors
    /// separately. The token belongs to the target, never the source archive.
    /// A post-journal error is ambiguous: recover/read back before retrying.
    pub fn commit_profile_group(
        &self,
        key: &str,
        latest: &DeviceSettings,
        expected: &crate::profile_group::GroupRevision,
        incoming: &crate::profile_group::GroupProfile,
    ) -> anyhow::Result<(DeviceSettings, lconnect_ipc::ProfileLibrary)> {
        crate::state_transaction::validate_key(key)?;
        let _guard = self.state.access()?;
        self.read_unlocked(key)?;
        let library = self.profiles().read_locked()?;
        let (next, profiles) = incoming.restore(latest, &library, expected)?;
        self.state.commit(key, &next, &profiles)?;
        Ok((next, profiles))
    }

    /// Commit the prepared group and its receipt in the same two-file journal.
    /// Only the serialized owner supplies latest accepted settings here.
    pub fn commit_group_import(
        &self,
        key: &str,
        latest: &DeviceSettings,
        expected: &crate::profile_group::GroupRevision,
        operation_id: &str,
        prepared: &crate::profile_archive::PreparedGroup,
    ) -> anyhow::Result<crate::profile_group::GroupImportCommit> {
        use crate::profile_group::{
            GroupImportCommit, GroupImportIntent, GroupImportReadback, GroupImportReceipt,
            GroupProfile, MAX_GROUP_IMPORT_RECEIPTS,
        };
        let intent = GroupImportIntent::new(operation_id.into(), expected.clone(), prepared)?;
        crate::state_transaction::validate_key(key)?;
        let _guard = self.state.access()?;
        let persisted = self.read_unlocked(key)?;
        let library = self.profiles().read_locked()?;
        validate_schema(latest)?;
        if let Some(receipt) = latest.group_imports.iter().find(|r| r.id == operation_id) {
            anyhow::ensure!(
                receipt.matches(&intent),
                "group import id already has different parameters"
            );
            anyhow::ensure!(
                persisted.group_imports.iter().any(|r| r == receipt),
                "group import receipt is not durable; recover before retrying"
            );
            self.sync_group_import(key)?;
            return Ok(GroupImportCommit {
                readback: GroupImportReadback::new(receipt.clone(), latest.clone(), library)?,
                repeated: true,
            });
        }
        anyhow::ensure!(
            !persisted.group_imports.iter().any(|r| r.id == operation_id),
            "group import receipt requires recovery before commit"
        );
        let (mut next, profiles) = prepared.profile().restore(latest, &library, expected)?;
        let snapshot = GroupProfile::capture(&next, &profiles, intent.kind)?;
        let receipt = GroupImportReceipt {
            id: intent.id,
            kind: intent.kind,
            source_sha256: intent.source_sha256,
            expected: intent.expected,
            profile_revision: profiles.revision,
            collection_revision: if intent.kind == crate::profile_group::GroupKind::Screen {
                next.screen_collections.as_ref().map(|c| c.revision)
            } else {
                None
            },
            content_sha256: snapshot.revision.settings_sha256,
        };
        receipt.validate()?;
        next.group_imports.push(receipt.clone());
        if next.group_imports.len() > MAX_GROUP_IMPORT_RECEIPTS {
            next.group_imports.remove(0);
        }
        validate_schema(&next)?;
        self.state.commit(key, &next, &profiles)?;
        Ok(GroupImportCommit {
            readback: GroupImportReadback::new(receipt, next, profiles)?,
            repeated: false,
        })
    }

    /// Readback never replays import/Apply. Keep latest queued deltas in the
    /// result, and confirm that its receipt exists in synced persistent files.
    pub fn read_group_import(
        &self,
        key: &str,
        latest: &DeviceSettings,
        operation_id: &str,
    ) -> anyhow::Result<Option<crate::profile_group::GroupImportReadback>> {
        crate::profile_group::validate_operation_id(operation_id)?;
        crate::state_transaction::validate_key(key)?;
        let _guard = self.state.access()?;
        let persisted = self.read_unlocked(key)?;
        anyhow::ensure!(
            persisted
                .group_imports
                .iter()
                .find(|r| r.id == operation_id)
                == latest.group_imports.iter().find(|r| r.id == operation_id),
            "group import owner requires recovery before readback"
        );
        let Some(receipt) = latest
            .group_imports
            .iter()
            .find(|r| r.id == operation_id)
            .cloned()
        else {
            return Ok(None);
        };
        anyhow::ensure!(
            persisted.group_imports.iter().any(|r| r == &receipt),
            "group import receipt is not durable; recover before readback"
        );
        let profiles = self.profiles().read_locked()?;
        self.sync_group_import(key)?;
        Ok(Some(crate::profile_group::GroupImportReadback::new(
            receipt,
            latest.clone(),
            profiles,
        )?))
    }

    fn validate_group_import_revisions(&self, settings: &DeviceSettings) -> anyhow::Result<()> {
        if !settings.group_imports.is_empty() {
            let profiles = self.profiles().read_locked()?;
            anyhow::ensure!(
                settings
                    .group_imports
                    .iter()
                    .all(|r| r.profile_revision <= profiles.revision),
                "group import receipt refers to a missing profile revision"
            );
        }
        Ok(())
    }

    fn sync_group_import(&self, key: &str) -> anyhow::Result<()> {
        for path in [
            self.key_path(key),
            self.root.join("profiles.json"),
            self.root.join("devices"),
            self.root.clone(),
        ] {
            std::fs::File::open(path)?.sync_all()?;
        }
        Ok(())
    }

    /// Commit reset settings and all eight shared screen slots together.
    /// Only the settings owner may pass its latest snapshot here: direct IPC
    /// callers must not bypass queued deltas. A failure after journal publication
    /// is committed intent; call read() before accepting further mutations.
    pub fn commit_screen_reset(
        &self,
        key: &str,
        latest: &DeviceSettings,
        expected_profile_revision: u64,
        expected_collection_revision: u64,
        operation_id: &str,
    ) -> anyhow::Result<(DeviceSettings, lconnect_ipc::ProfileLibrary)> {
        use lconnect_ipc::{NamedProfile, ProfileContent, ProfileGroup};
        crate::state_transaction::validate_key(key)?;
        let _guard = self.state.access()?;
        self.read_unlocked(key)?;
        validate_schema(latest)?;
        validate_reset_id(operation_id)?;
        let mut library = self.profiles().read_locked()?;
        if latest
            .screen_reset_receipt
            .as_ref()
            .is_some_and(|receipt| receipt.operation_id == operation_id)
        {
            return Ok((latest.clone(), library));
        }
        anyhow::ensure!(
            library.revision == expected_profile_revision,
            "profile library changed; reload before editing"
        );
        let collection_revision = latest
            .screen_collections
            .as_ref()
            .map_or(0, |collections| collections.revision);
        anyhow::ensure!(
            collection_revision == expected_collection_revision,
            "screen collections changed; reload before reset"
        );
        let next_collection_revision = collection_revision
            .checked_add(1)
            .context("collection revision exhausted")?;
        let defaults = (0..8)
            .map(|slot| NamedProfile::defaults(ProfileGroup::Screen, slot, None))
            .collect::<lconnect_ipc::Result<Vec<_>>>()?;
        let ProfileContent::Screen(theme) = &defaults[0].content else {
            unreachable!("screen default")
        };
        let mut next = latest.clone();
        next.lcd = LcdSection {
            brightness: 50,
            rotation: "0".into(),
            control_mode: "Application".into(),
        };
        next.theme = Some(theme.clone());
        next.media_clip = None;
        next.screen_collections = Some(lconnect_ipc::ScreenCollections::defaults(
            next_collection_revision,
        ));
        library
            .profiles
            .retain(|profile| profile.content.group() != ProfileGroup::Screen);
        library.profiles.extend(defaults);
        library.revision = library
            .revision
            .checked_add(1)
            .context("profile revision exhausted")?;
        next.screen_reset_receipt = Some(ScreenResetReceipt {
            operation_id: operation_id.into(),
            profile_revision: library.revision,
            collection_revision: next_collection_revision,
        });
        validate_schema(&next)?;
        library.validate()?;
        self.state.commit(key, &next, &library)?;
        Ok((next, library))
    }

    /// Probe-only bounded read: no state.access(), journal redo, or directory creation.
    pub(crate) fn firmware_guard_readonly(
        &self,
        key: &str,
    ) -> anyhow::Result<crate::firmware_operation::SettingsGuard> {
        crate::state_transaction::validate_key(key)?;
        self.state.ensure_no_pending_transaction()?;
        crate::state_transaction::check_directory(&self.root.join("devices"))?;
        let text =
            crate::state_transaction::read_optional(&self.key_path(key), MAX_SETTINGS_BYTES)?
                .context("firmware settings missing")?;
        let saved: DeviceSettings = toml::from_str(&text)?;
        validate_schema(&saved)?;
        saved
            .firmware_update_guard
            .context("firmware settings guard missing")
    }

    /// Bounded recovery inspection: never creates state, runs redo or touches HID.
    pub(crate) fn firmware_recovery_snapshot(
        &self,
    ) -> anyhow::Result<(
        Option<crate::firmware_operation::Record>,
        Vec<lconnect_ipc::FirmwareRecoveryGuard>,
    )> {
        use crate::state_transaction::{check_directory, read_optional, validate_key};
        let _lock = self.state.readonly_access()?;
        let journal = self.firmware_operations();
        let before = journal.read_snapshot()?;
        let directory = self.root.join("devices");
        check_directory(&directory)?;
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => Some(entries),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let mut guards = Vec::new();
        for (index, entry) in entries.into_iter().flatten().enumerate() {
            anyhow::ensure!(
                index < 256,
                "firmware inspection device directory exceeds 256 entries"
            );
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|s| s != "toml") {
                continue;
            }
            let key = path
                .file_stem()
                .and_then(|s| s.to_str())
                .context("invalid device settings key")?;
            validate_key(key)?;
            let text = read_optional(&path, MAX_SETTINGS_BYTES)?
                .context("device settings disappeared during firmware inspection")?;
            let saved: DeviceSettings = toml::from_str(&text)?;
            validate_schema(&saved)?;
            if let Some(guard) = saved.firmware_update_guard {
                guards.push(lconnect_ipc::FirmwareRecoveryGuard {
                    settings_key: key.into(),
                    operation_id: guard.operation_id,
                    image_sha256: guard.image_sha256,
                });
            }
        }
        guards.sort_by(|a, b| a.settings_key.cmp(&b.settings_key));
        let after = journal.read_snapshot()?;
        anyhow::ensure!(
            before.as_ref().map(|(_, raw)| raw) == after.as_ref().map(|(_, raw)| raw),
            "firmware journal changed during recovery inspection; inspect again"
        );
        Ok((before.map(|(record, _)| record), guards))
    }

    /// Internal update owner only. Must finish before any hold or flash write.
    /// Failure after publication intentionally leaves the device quarantined.
    pub(crate) fn anchor_firmware(
        &self,
        key: &str,
        guard: crate::firmware_operation::SettingsGuard,
        expected: Option<&DeviceSettings>,
    ) -> anyhow::Result<()> {
        crate::state_transaction::validate_key(key)?;
        let _lock = self.state.access()?;
        let mut saved = self.read_unlocked(key)?;
        anyhow::ensure!(
            saved.firmware_update_guard.is_none(),
            "firmware settings already guarded"
        );
        if let Some(expected) = expected {
            anyhow::ensure!(
                serde_json::to_value(&saved)? == serde_json::to_value(expected)?,
                "saved intent changed since firmware snapshot"
            );
        }
        saved.firmware_update_guard = Some(guard);
        validate_schema(&saved)?;
        let text = toml::to_string_pretty(&saved)?;
        anyhow::ensure!(
            text.len() as u64 <= MAX_SETTINGS_BYTES,
            "settings exceed size limit"
        );
        let directory = self.root.join("devices");
        crate::state_transaction::check_directory(&directory)?;
        std::fs::create_dir_all(&directory)?;
        std::fs::File::open(&self.root)?.sync_all()?;
        crate::state_transaction::atomic_write(&self.key_path(key), text.as_bytes())
    }

    /// A changed serial/path cannot evade an unresolved guard for this SKU.
    pub(crate) fn ensure_no_firmware_guard(&self) -> anyhow::Result<()> {
        let _lock = self.state.access()?;
        let directory = self.root.join("devices");
        crate::state_transaction::check_directory(&directory)?;
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|s| s == "toml") {
                let key = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .context("invalid device settings key")?;
                crate::state_transaction::validate_key(key)?;
                anyhow::ensure!(
                    self.read_unlocked(key)?.firmware_update_guard.is_none(),
                    "unresolved firmware settings guard; ordinary attachment withheld"
                );
            }
        }
        Ok(())
    }

    /// Atomic write: serialize, write to `<key>.toml.tmp`, fsync, then
    /// `rename` over `<key>.toml`. The parent `devices/` directory is
    /// created on demand.
    ///
    /// # Errors
    /// I/O errors creating the directory, writing/fsyncing the temp file,
    /// or renaming it; TOML serialization errors.
    pub fn write(&self, key: &str, settings: &DeviceSettings) -> anyhow::Result<()> {
        let _guard = self.state.access()?;
        use std::io::Write;
        validate_schema(settings)?;
        self.validate_group_import_revisions(settings)?;
        // Re-read before replacing: protect files upgraded or damaged since attach.
        let current = self.read_unlocked(key)?;
        anyhow::ensure!(
            current.firmware_update_guard.is_none() && settings.firmware_update_guard.is_none(),
            "firmware settings guard forbids ordinary persistence"
        );
        let path = self.key_path(key);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("toml.tmp");
        let text = toml::to_string_pretty(settings)?;
        anyhow::ensure!(
            text.len() as u64 <= MAX_SETTINGS_BYTES,
            "settings exceed size limit"
        );
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(tmp, &path)?;
        std::fs::File::open(path.parent().unwrap())?.sync_all()?;
        Ok(())
    }
}

pub(crate) fn validate_schema(settings: &DeviceSettings) -> anyhow::Result<()> {
    if let Some(guard) = &settings.firmware_update_guard {
        guard.validate()?;
    }
    anyhow::ensure!(
        settings.schema_version == CURRENT_SCHEMA,
        "unsupported settings schema {}; expected {}; original file preserved",
        settings.schema_version,
        CURRENT_SCHEMA
    );
    anyhow::ensure!(
        !settings.fan.manual_zero || settings.fan.pwm_percent == 0,
        "manual_zero marker requires saved fan PWM0"
    );
    anyhow::ensure!(
        settings.theme.is_none() || settings.media_clip.is_none(),
        "theme and media selection are mutually exclusive"
    );
    settings.lighting.fan_topology.validate()?;
    settings.dynamic_rgb_profiles.validate()?;
    anyhow::ensure!(
        settings.lighting.fan_modes.len() <= 6 && settings.lighting.pump_modes.len() <= 16,
        "too many saved lighting modes"
    );
    for (kind, key, mode) in settings
        .lighting
        .fan_modes
        .iter()
        .map(|(k, s)| (lconnect_ipc::LightingKind::Fan, k, &s.mode))
        .chain(
            settings
                .lighting
                .pump_modes
                .iter()
                .map(|(k, s)| (lconnect_ipc::LightingKind::Pump, k, &s.mode)),
        )
    {
        anyhow::ensure!(
            key == lconnect_ipc::canonical_lighting_mode(mode)
                && lconnect_ipc::lighting_color_count(kind, key).is_some(),
            "invalid lighting profile key or mode"
        );
    }
    for (kind, mode, colors) in settings
        .lighting
        .fan
        .as_ref()
        .map(|s| {
            (
                lconnect_ipc::LightingKind::Fan,
                &s.mode,
                &s.additional_colors,
            )
        })
        .into_iter()
        .chain(settings.lighting.pump.as_ref().map(|s| {
            (
                lconnect_ipc::LightingKind::Pump,
                &s.mode,
                &s.additional_colors,
            )
        }))
        .chain(settings.lighting.fan_modes.values().map(|s| {
            (
                lconnect_ipc::LightingKind::Fan,
                &s.mode,
                &s.additional_colors,
            )
        }))
        .chain(settings.lighting.pump_modes.values().map(|s| {
            (
                lconnect_ipc::LightingKind::Pump,
                &s.mode,
                &s.additional_colors,
            )
        }))
    {
        if !colors.is_empty() {
            anyhow::ensure!(
                colors.len() <= 3
                    && lconnect_ipc::lighting_color_count(kind, mode) == Some(colors.len() + 1),
                "invalid saved lighting palette size"
            );
            anyhow::ensure!(
                colors.iter().all(|c| {
                    let c = c.strip_prefix('#').unwrap_or(c);
                    c.len() == 6 && c.bytes().all(|b| b.is_ascii_hexdigit())
                }),
                "invalid saved lighting palette color"
            );
        }
    }
    if let Some(key) = &settings.media_clip {
        anyhow::ensure!(
            key.len() == 32
                && key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid saved media key"
        );
    }
    if let Some(theme) = &settings.theme {
        theme.validate()?;
    }
    if let Some(collections) = &settings.screen_collections {
        collections.validate()?;
    }
    anyhow::ensure!(
        settings.group_imports.len() <= crate::profile_group::MAX_GROUP_IMPORT_RECEIPTS,
        "too many group import receipts"
    );
    let mut group_ids = std::collections::BTreeSet::new();
    for receipt in &settings.group_imports {
        receipt.validate()?;
        anyhow::ensure!(
            group_ids.insert(&receipt.id),
            "duplicate group import receipt"
        );
        if let Some(revision) = receipt.collection_revision {
            anyhow::ensure!(
                settings
                    .screen_collections
                    .as_ref()
                    .is_some_and(|c| c.revision >= revision),
                "group import receipt refers to a missing collection revision"
            );
        }
    }
    if let Some(receipt) = &settings.screen_reset_receipt {
        validate_reset_id(&receipt.operation_id)?;
        anyhow::ensure!(
            settings
                .screen_collections
                .as_ref()
                .map_or(0, |c| c.revision)
                >= receipt.collection_revision,
            "reset receipt refers to a missing collection revision"
        );
        anyhow::ensure!(
            receipt.profile_revision > 0,
            "invalid reset profile revision"
        );
    }
    Ok(())
}

fn validate_reset_id(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        id.len() == 32
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "reset operation id must be 32 lowercase hexadecimal digits"
    );
    Ok(())
}

/// Stable settings identity independent of attachment order and hidraw path.
///
/// Serial bytes are hex encoded to keep filenames unambiguous and path safe.
/// A serial-less singleton key is only usable when the manager has established
/// that exactly one matching VID/PID device is present. Replacing that physical
/// unit will reuse its singleton settings; legacy runtime-ID files are not read.
#[must_use]
pub fn settings_key(info: &lconnect_hid::DeviceInfo) -> String {
    use std::fmt::Write;
    let mut key = format!("{:04x}-{:04x}-", info.vid, info.pid);
    match info.serial.as_deref().filter(|serial| !serial.is_empty()) {
        Some(serial) => {
            key.push_str("serial-");
            for byte in serial.as_bytes() {
                let _ = write!(key, "{byte:02x}");
            }
        }
        None => key.push_str("singleton"),
    }
    key
}

#[cfg(test)]
mod transaction_scheduling_tests {
    use super::*;
    use crate::actor::command_dispatch::{spawn_updates, SettingsUpdate};
    use std::{sync::Arc, time::Duration};
    use tokio::sync::{mpsc, oneshot, watch};

    #[tokio::test(flavor = "current_thread")]
    async fn contended_reset_and_ordinary_flush_leave_the_async_executor_running() {
        for reset in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let store = Arc::new(PersistenceStore::new_with_root(dir.path().into()));
            let locked = store.clone();
            let (ready, ready_rx) = oneshot::channel();
            let (release, release_rx) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _guard = locked.state.access().unwrap();
                ready.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).is_ok()
            });
            ready_rx.await.unwrap();
            let (tx, rx) = mpsc::channel(8);
            let (_shutdown, stop) = watch::channel(false);
            let task = spawn_updates(
                store.clone(),
                "device".into(),
                DeviceSettings::default(),
                rx,
                stop,
            );
            let (reply, response) = oneshot::channel();
            if reset {
                tx.send(SettingsUpdate::ResetScreen {
                    expected_profile_revision: 0,
                    expected_collection_revision: 0,
                    operation_id: "f".repeat(32),
                    reply,
                })
                .await
                .unwrap();
            } else {
                drop(reply);
                tx.send(SettingsUpdate::Fan(FanSection {
                    pwm_percent: 60,
                    ..Default::default()
                }))
                .await
                .unwrap();
            }
            // Reset starts immediately; ordinary flush starts after its 250ms debounce.
            tokio::time::sleep(Duration::from_millis(350)).await;
            let _ = release.send(());
            assert!(
                blocker.await.unwrap(),
                "async timer could not release the contended gate"
            );
            if reset {
                assert!(response.await.unwrap().is_ok());
            }
            drop(tx);
            task.await.unwrap();
            assert!(store.read("device").is_ok());
        }
    }
}

#[cfg(test)]
#[path = "persistence_group_import_tests.rs"]
mod group_import_tests;

pub use crate::asset_publication::journal::RecoveryNotice as ImportRecoveryNotice;

pub use crate::asset_publication::journal::{
    SlotRecoveryNotice as SlotImportRecoveryNotice, StartupNotice as ImportStartupNotice,
};
