//! `IpcServerImpl` -- concrete `DaemonRpc` backed by `ManagerHandle` + `Stats`.
//!
//! Each method:
//!
//! 1. Bumps `Stats::ipc_request_count`.
//! 2. Looks up the live `ManagedDevice` via `manager.with_device(id, ...)`,
//!    cloning out the `DeviceHandles` so the read-lock guard does not span
//!    any await points.
//! 3. Builds the wire bytes via the `llcontrol-protocol` codecs and ships
//!    them through the appropriate per-device channel (`hid_io_tx` for one-
//!    shot writes, `lcd_frame_tx` for LCD frames, `lighting_state_tx` for
//!    keeper-state replacement, `settings_tx` for debounced disk writes).
//!
//! ## Reset-token registry
//!
//! `request_reset_token` mints a 32-hex-char token, stores `(token, expires_at)`
//! in a `Mutex<HashMap<DeviceId, ...>>`, and returns it. `reset_device`
//! looks the entry up, verifies the token matches and is unexpired, and
//! removes the entry **before** issuing the reset (single-use, even on
//! failure -- the token is consumed by the attempt). Expiry is 30s.
//!
//! ## raw_send gating
//!
//! `raw_send` is gated by `config.daemon.allow_raw_send`. The default is
//! `false`; opting in requires editing `daemon.toml`. When disabled, the
//! method returns `IpcError::NotImplemented`. Cmd 15 (`UploadJpeg`) is
//! deliberately not exposed via `set_lcd_image` -- only cmd 14 (`SendJpeg`)
//! drives the lcd_keeper. See the Phase 0e finding in
//! `docs/development/protocol/captures/2026-04-25-jpeg-upload-live.txt`.
//!
//! Settings mutations are independent deltas applied by command_dispatch.

mod firmware_coordinator;
mod group_import;
pub use firmware_coordinator::{
    FirmwareJob, FirmwareJobReport, FirmwareJobStage, FirmwareJobStatus, FirmwareReconnectIdentity,
    FirmwareReconnectJob, FirmwareReconnectReport, FirmwareReconnectStage, FirmwareReconnectStatus,
};

use crate::actor::command_dispatch::SettingsUpdate;
use crate::actor::{DeviceHandles, HidCommand, LcdCommand, LightingStateMsg};
use crate::codec_str::rgb_to_hex;
use crate::config::Config;
use crate::manager::ManagerHandle;
use crate::persistence::{LightingState, PersistenceStore, PumpLightingState};
use crate::stats::Stats;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::StreamExt;
use llcontrol_ipc::{
    DaemonRpc, DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, H264Clip,
    IpcError, LcdConfig, LcdConfigPatch, LcdPlaybackStats, LcdState, LedTarget, LightingConfig,
    LightingInfo, LightingKind, Pwm, PwmSource, ResetToken, Result, Rgb, SensorSnapshot,
};
use llcontrol_protocol::galahad2_lcdv2::commands::Command;
use llcontrol_protocol::galahad2_lcdv2::firmware::FirmwareVersion;
use llcontrol_protocol::galahad2_lcdv2::reset::{
    build_request as build_reset_request, parse_response as parse_reset_response,
};
use rand::RngCore;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};
use tokio_stream::wrappers::WatchStream;

/// Single-use reset-token expiry.
const RESET_TOKEN_TTL: Duration = Duration::from_secs(30);

/// Concrete `DaemonRpc` implementation for the production daemon.
pub struct IpcServerImpl {
    firmware_jobs: std::sync::Mutex<firmware_coordinator::FirmwareJobs>,
    firmware_inspections: Arc<tokio::sync::Semaphore>,
    persistence: Arc<PersistenceStore>,
    profiles: crate::profiles::ProfileStore,
    archive_transfers: crate::archive_transfer::ArchiveTransfers,
    group_exports: crate::group_exports::GroupExports,
    group_uploads: crate::group_uploads::GroupUploads,
    sensor_defaults: crate::sensor_defaults::SensorDefaultsStore,
    system_spec: crate::system_spec::SpecService,
    theme_assets: crate::theme_assets::ThemeAssetStore,
    font_uploads: crate::font_upload::FontUploads,
    media_uploads: crate::media_upload::MediaUploads,
    media_exports: crate::media_export::MediaExports,
    theme_jobs: crate::admission::WorkSlots,
    editor_cache: Arc<crate::editor_cache::EditorCache>,
    editor_video_jobs: crate::admission::WorkSlots,
    sensors: crate::sensors::SensorCache,
    manager: ManagerHandle,
    stats: Arc<Stats>,
    config: Config,
    /// device_id -> (token, expires_at). One-shot 30s TTL.
    reset_tokens: Mutex<HashMap<DeviceId, (ResetToken, Instant)>>,
}

impl IpcServerImpl {
    /// Construct a new server. The daemon's `main.rs` (P1D-14) will wire
    /// this up to a `Arc<dyn DaemonRpc>` and feed it to `IpcServer::new`.
    #[must_use]
    pub fn new(
        manager: ManagerHandle,
        stats: Arc<Stats>,
        config: Config,
        persistence: Arc<PersistenceStore>,
    ) -> Self {
        let admission = persistence.admission();
        Self {
            firmware_jobs: std::sync::Mutex::default(),
            firmware_inspections: Arc::new(tokio::sync::Semaphore::new(2)),
            group_uploads: crate::group_uploads::GroupUploads::new()
                .with_admission(admission.clone()),
            group_exports: crate::group_exports::GroupExports::new(
                crate::profile_archive::ProfileArchives::new(
                    persistence.theme_assets(),
                    persistence.theme_fonts(),
                    persistence.media_assets(),
                ),
            )
            .with_admission(admission.clone()),
            archive_transfers: crate::archive_transfer::ArchiveTransfers::new(
                crate::profile_archive::ProfileArchives::new(
                    persistence.theme_assets(),
                    persistence.theme_fonts(),
                    persistence.media_assets(),
                ),
                persistence.clone(),
            )
            .with_admission(admission.clone()),
            profiles: persistence.profiles(),
            sensor_defaults: crate::sensor_defaults::SensorDefaultsStore::new(
                persistence.sensor_defaults_root(),
            ),
            theme_assets: persistence.theme_assets(),
            font_uploads: crate::font_upload::FontUploads::new(persistence.theme_fonts()),
            media_uploads: crate::media_upload::MediaUploads::new(persistence.media_assets())
                .with_admission(admission.clone()),
            media_exports: crate::media_export::MediaExports::new(persistence.media_assets())
                .with_admission(admission.clone()),
            persistence,
            theme_jobs: crate::admission::WorkSlots::new(2, admission.clone()),
            editor_cache: Arc::default(),
            editor_video_jobs: crate::admission::WorkSlots::new(1, admission.clone()),
            sensors: crate::sensors::SensorCache::default(),
            system_spec: crate::system_spec::SpecService::new(Default::default())
                .with_admission(admission.clone()),
            manager,
            stats,
            config,
            reset_tokens: Mutex::default(),
        }
    }

    /// Internal maintenance preparation only; no IPC/product Apply route.
    /// Caller owns the maintenance job and must still quiesce/join actors and
    /// wait_checked before taking the restore snapshot or starting transfer.
    pub async fn close_staging(
        &self,
        maintenance: &crate::admission::Maintenance,
    ) -> anyhow::Result<()> {
        let gate = self.persistence.admission();
        let lease = maintenance.child(&gate)?;
        self.media_exports.close_staging();
        self.font_uploads.close_staging(lease.child());
        self.media_uploads.close_staging(lease);
        self.archive_transfers.close_staging(maintenance)?;
        self.group_uploads.close_staging(maintenance).await?;
        self.group_exports.close_staging(maintenance).await?;
        Ok(())
    }

    fn admit(&self) -> Result<crate::admission::Lease> {
        self.persistence
            .admission()
            .enter()
            .map_err(|error| IpcError::DeviceBusy(error.to_string()))
    }

    async fn prepare_theme(
        store: crate::theme_assets::ThemeAssetStore,
        fonts: crate::theme_fonts::ThemeFontStore,
        caches: Vec<crate::sensors::SensorCache>,
        media: crate::media_assets::MediaAssetStore,
        theme: llcontrol_ipc::LcdTheme,
        display_unit: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Arc<crate::theme_runtime::ThemeRuntime>> {
        Self::prepare_theme_oriented(
            store,
            fonts,
            caches,
            media,
            theme,
            llcontrol_image::FrameRotation::None,
            display_unit,
        )
        .await
    }
    async fn prepare_theme_oriented(
        store: crate::theme_assets::ThemeAssetStore,
        fonts: crate::theme_fonts::ThemeFontStore,
        caches: Vec<crate::sensors::SensorCache>,
        media: crate::media_assets::MediaAssetStore,
        theme: llcontrol_ipc::LcdTheme,
        rotation: llcontrol_image::FrameRotation,
        display_unit: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Arc<crate::theme_runtime::ThemeRuntime>> {
        theme.validate()?;
        tokio::task::spawn_blocking(move || {
            let assets = store
                .load(&theme)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
            let fonts = fonts
                .load_theme(&theme)
                .map_err(|e| IpcError::InvalidArgument(format!("theme fonts: {e}")))?;
            let pin = media
                .pin_theme(&theme)
                .map_err(|e| IpcError::InvalidArgument(format!("theme media: {e}")))?;
            Ok(Arc::new(
                crate::theme_runtime::ThemeRuntime::with_fonts(theme, assets, fonts, caches)?
                    .with_display_unit(display_unit)
                    .with_media_pin(pin)
                    .with_output_rotation(rotation),
            ))
        })
        .await
        .map_err(lcd_closed)?
    }

    async fn manual_cooling(
        &self,
        dev: DeviceId,
        target: llcontrol_ipc::CoolingTarget,
        percent: u8,
        motherboard: bool,
    ) -> Result<()> {
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .cooling_tx
            .send(crate::actor::cooling::CoolingCommand::Manual {
                target,
                percent,
                motherboard,
                reply,
            })
            .await
            .map_err(|_| IpcError::DeviceBusy("cooling actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("cooling reply dropped".into()))?
    }
    /// Attach the process-wide host sensor cache.
    pub fn with_system_spec(mut self, provider: crate::system_spec::LinuxSpec) -> Self {
        self.system_spec = crate::system_spec::SpecService::new(provider)
            .with_admission(self.persistence.admission());
        self
    }
    pub fn with_sensors(mut self, sensors: crate::sensors::SensorCache) -> Self {
        self.sensors = sensors;
        self
    }

    /// Look up the `DeviceHandles` for `id`, returning
    /// `IpcError::DeviceNotFound` if the device is not currently attached.
    ///
    /// Cloning the handles inside the `with_device` closure releases the
    /// manager's read lock before we suspend on any HID I/O — the
    /// channels themselves are `Arc`-backed so cloning is cheap.
    async fn group_snapshot(
        &self,
        dev: DeviceId,
        group: llcontrol_ipc::FullProfileGroup,
    ) -> Result<(
        llcontrol_ipc::GroupArchiveSnapshot,
        crate::profile_group::GroupProfile,
    )> {
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::ReadProfileGroup {
                kind: match group {
                    llcontrol_ipc::FullProfileGroup::Screen => {
                        crate::profile_group::GroupKind::Screen
                    }
                    llcontrol_ipc::FullProfileGroup::FanAndLighting => {
                        crate::profile_group::GroupKind::FanAndLighting
                    }
                },
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        let source = rx
            .await
            .map_err(lcd_closed)?
            .map_err(IpcError::DeviceBusy)?;
        let expected = serde_json::to_string(&source.revision)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let summary = crate::group_exports::summary(&source.profile)?;
        Ok((
            llcontrol_ipc::GroupArchiveSnapshot {
                dev,
                group,
                expected,
                summary,
            },
            source.profile,
        ))
    }
    async fn handles(&self, id: &DeviceId) -> Result<DeviceHandles> {
        self.manager
            .with_device(id, |dev| dev.handles.clone())
            .await
            .ok_or_else(|| IpcError::DeviceNotFound(id.as_str().to_string()))
    }
}

/// Issue an `HidCommand::WriteRead64`, decoding any I/O failure to
/// `IpcError::DeviceBusy`. The hid_io actor bumps
/// `Stats::hid_write_count` on the success path.
async fn write_read_64(
    handles: &DeviceHandles,
    bytes: Vec<u8>,
    expected_cmd: u8,
) -> Result<[u8; 64]> {
    let (reply_tx, reply_rx) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::WriteRead64 {
            bytes,
            expected_cmd,
            reply: reply_tx,
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid_io channel closed: {e}")))?;
    reply_rx
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid reply dropped: {e}")))?
        .map_err(|e| IpcError::DeviceBusy(format!("hid round-trip failed: {e}")))
}

/// Issue an `HidCommand::WriteRead512`, decoding any I/O failure to
/// `IpcError::DeviceBusy`. The hid_io actor bumps
/// `Stats::hid_write_count` on the success path.
///
/// `expected_report_id` selects between B-class (`0x02`) and C-class
/// (`0x03`) framings — the drain loop in `hid_io` matches against this
/// byte to identify the real response among any stale ones still in
/// the device's USB read buffer.
async fn write_read_512(
    handles: &DeviceHandles,
    bytes: Vec<u8>,
    expected_report_id: u8,
    expected_cmd: u8,
) -> Result<[u8; 512]> {
    let (reply_tx, reply_rx) = oneshot::channel();
    handles
        .hid_io_tx
        .send(HidCommand::WriteRead512 {
            bytes,
            expected_report_id,
            expected_cmd,
            reply: reply_tx,
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid_io channel closed: {e}")))?;
    reply_rx
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("hid reply dropped: {e}")))?
        .map_err(|e| IpcError::DeviceBusy(format!("hid round-trip failed: {e}")))
}

/// Generate a 16-byte cryptographically-random hex token.
fn mint_reset_token() -> ResetToken {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let s = bytes.iter().fold(String::with_capacity(32), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    ResetToken(s)
}

#[async_trait]
impl DaemonRpc for IpcServerImpl {
    async fn cleanup_theme_assets(
        &self,
        delete: bool,
        keep: Vec<String>,
    ) -> Result<llcontrol_ipc::AssetCleanupReport> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let persistence = self.persistence.clone();
        let profiles = self.profiles.clone();
        let assets = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut referenced = persistence.referenced_theme_assets()?;
            for profile in profiles.list()?.profiles {
                if let llcontrol_ipc::ProfileContent::Screen(theme) = profile.content {
                    referenced.extend(theme.image_assets().into_iter().map(str::to_owned));
                }
            }
            assets.cleanup(&referenced, &keep, delete)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("asset cleanup: {e}")))
    }
    async fn cleanup_theme_fonts(
        &self,
        delete: bool,
        keep: Vec<String>,
    ) -> Result<llcontrol_ipc::AssetCleanupReport> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let persistence = self.persistence.clone();
        let profiles = self.profiles.clone();
        let assets = self.persistence.theme_fonts();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut referenced = persistence.referenced_theme_fonts()?;
            for profile in profiles.list()?.profiles {
                if let llcontrol_ipc::ProfileContent::Screen(theme) = profile.content {
                    referenced.extend(theme.font_assets().into_iter().map(str::to_owned));
                }
            }
            assets.cleanup(&referenced, &keep, delete)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("font cleanup: {e}")))
    }
    async fn reapply_screen_reset(
        &self,
        dev: DeviceId,
        operation_id: String,
        expected_generation: u64,
    ) -> Result<llcontrol_ipc::LcdState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let _permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let (reply, response) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::PrepareScreenResetReapply {
                operation_id: operation_id.clone(),
                expected_generation,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        let (runtime, preparation) = response.await.map_err(lcd_closed)??;
        let prepared = runtime.prepare().await?;
        let (reply, response) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::ReapplyScreenReset {
                operation_id,
                expected_generation,
                preparation,
                prepared,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        response.await.map_err(lcd_closed)?
    }

    async fn get_screen_reset(&self, dev: DeviceId) -> Result<llcontrol_ipc::ScreenResetReadback> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, response) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::ReadScreenReset { reply })
            .await
            .map_err(lcd_closed)?;
        response.await.map_err(lcd_closed)?
    }

    async fn reset_screen(
        &self,
        dev: DeviceId,
        operation_id: String,
        expected_profile_revision: u64,
        expected_collection_revision: u64,
    ) -> Result<llcontrol_ipc::ScreenResetResult> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        if operation_id.len() != 32
            || !operation_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(IpcError::InvalidArgument(
                "reset operation id must be 32 lowercase hex characters".into(),
            ));
        }
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let (reply, response) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::BeginScreenReset {
                operation_id: operation_id.clone(),
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        let preparation = match response.await.map_err(lcd_closed)?? {
            crate::actor::ScreenResetAdmission::Repeated(result) => return Ok(result),
            crate::actor::ScreenResetAdmission::Prepare(ticket) => ticket,
        };
        let default =
            llcontrol_ipc::NamedProfile::defaults(llcontrol_ipc::ProfileGroup::Screen, 0, None)?;
        let llcontrol_ipc::ProfileContent::Screen(theme) = default.content else {
            unreachable!("screen default")
        };
        let runtime = Self::prepare_theme(
            self.theme_assets.clone(),
            self.persistence.theme_fonts(),
            vec![self.sensors.clone(), handles.sensor_cache],
            self.persistence.media_assets(),
            theme,
            self.persistence.display_settings().live_unit(),
        )
        .await?;
        let prepared = runtime.prepare().await?;
        let (reply, response) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::ResetScreen {
                preparation: Some(preparation),
                operation_id,
                expected_profile_revision,
                expected_collection_revision,
                prepared,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        let result = response.await.map_err(lcd_closed)?;
        drop(permit);
        result
    }

    async fn migrate_screen_collections(
        &self,
        dev: DeviceId,
    ) -> Result<llcontrol_ipc::ScreenCollections> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let images = self.theme_assets.clone();
        let media = self.persistence.media_assets();
        let migration = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::screen_collections::PreparedMigration::prepare(&images, &media)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|error| IpcError::InvalidArgument(format!("migration: {error}")))?;
        let (reply, response) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::MigrateCollections { migration, reply })
            .await
            .map_err(lcd_closed)?;
        response
            .await
            .map_err(lcd_closed)?
            .map_err(IpcError::InvalidArgument)
    }

    async fn get_screen_collections(
        &self,
        dev: DeviceId,
    ) -> Result<llcontrol_ipc::ScreenCollectionsState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, response) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::ReadCollections { reply })
            .await
            .map_err(lcd_closed)?;
        response.await.map_err(lcd_closed)
    }

    async fn set_screen_collections(
        &self,
        dev: DeviceId,
        expected_revision: u64,
        collections: llcontrol_ipc::ScreenCollections,
    ) -> Result<llcontrol_ipc::ScreenCollections> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        collections.validate()?;
        if expected_revision.checked_add(1) != Some(collections.revision) {
            return Err(IpcError::InvalidArgument(
                "expected next collection revision".into(),
            ));
        }
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let assets = self.theme_assets.clone();
        let media = self.persistence.media_assets();
        let prepared = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::screen_collections::PreparedCollections::prepare(collections, &assets, &media)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|error| IpcError::InvalidArgument(format!("screen collections: {error}")))?;
        let (reply, response) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::SetCollections {
                expected_revision,
                prepared,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        response
            .await
            .map_err(lcd_closed)?
            .map_err(IpcError::InvalidArgument)
    }

    async fn list_profiles(&self) -> Result<llcontrol_ipc::ProfileLibrary> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.profiles.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.list()
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("profile library: {e}")))
    }
    async fn edit_profile(
        &self,
        expected_revision: u64,
        edit: llcontrol_ipc::ProfileEdit,
    ) -> Result<llcontrol_ipc::ProfileLibrary> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        edit.validate()?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.profiles.clone();
        let assets = self.theme_assets.clone();
        let fonts = self.persistence.theme_fonts();
        let media = self.persistence.media_assets();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut media_pin = None;
            let mut image_leases = None;
            if let llcontrol_ipc::ProfileEdit::Save { profile } = &edit {
                if let llcontrol_ipc::ProfileContent::Screen(theme) = &profile.content {
                    image_leases = Some(assets.load(theme)?);
                    fonts.load_theme(theme)?;
                    media_pin = media.pin_theme(theme)?;
                }
                if let llcontrol_ipc::ProfileContent::ScreenMedia(key) = &profile.content {
                    media_pin = Some(media.acquire_source(key)?.pin());
                }
            }
            let result = store.edit(expected_revision, edit);
            drop(media_pin);
            drop(image_leases);
            result
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("profile library: {e}")))
    }
    async fn get_dynamic_rgb(&self, dev: DeviceId) -> Result<llcontrol_ipc::DynamicRgbState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::DynamicRgbState(reply))
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)
    }
    async fn set_dynamic_rgb(
        &self,
        dev: DeviceId,
        config: Option<llcontrol_ipc::DynamicRgbConfig>,
    ) -> Result<llcontrol_ipc::DynamicRgbState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        if let Some(config) = &config {
            config.validate()?;
        }
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::SetDynamicRgb { config, reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?
    }
    async fn get_thermal_state(&self, dev: DeviceId) -> Result<llcontrol_ipc::ThermalState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::ThermalState(reply))
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("lighting reply dropped".into()))
    }
    async fn set_thermal_config(
        &self,
        dev: DeviceId,
        config: llcontrol_ipc::ThermalConfig,
    ) -> Result<llcontrol_ipc::ThermalState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::SetThermal { config, reply })
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("lighting reply dropped".into()))?
    }

    async fn get_sensor_defaults(&self) -> Result<llcontrol_ipc::SensorDefaultsState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.sensor_defaults.clone();
        let config = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.load()
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("sensor defaults: {e}")))?;
        // Read freshness after disk I/O, not before a potentially slow load.
        let readings = self.sensors.readings();
        let candidates = crate::sensor_selection::candidates(&readings);
        let selected_gpu = config.gpu.as_ref().map(|gpu| gpu.device_id.as_str());
        let values = llcontrol_ipc::DefaultSensorRole::ALL
            .into_iter()
            .map(|role| llcontrol_ipc::DefaultSensorValue {
                role,
                sensor_id: config.binding(role).map(str::to_owned),
                reading: config
                    .resolve(role, &readings)
                    .filter(|reading| {
                        candidates.iter().any(|candidate| {
                            candidate.role == role
                                && candidate.reading.id == reading.id
                                && (candidate.gpu_id.is_none()
                                    || candidate.gpu_id.as_deref() == selected_gpu)
                        })
                    })
                    .cloned(),
            })
            .collect();
        Ok(llcontrol_ipc::SensorDefaultsState {
            recommended: crate::sensor_selection::recommended(&readings, config.revision),
            candidates,
            config,
            values,
        })
    }

    async fn set_sensor_defaults(
        &self,
        config: llcontrol_ipc::SensorDefaults,
    ) -> Result<llcontrol_ipc::SensorDefaults> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        config.validate()?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.sensor_defaults.clone();
        let sensors = self.sensors.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let current = store.load()?;
            crate::sensor_selection::validate_selection(&config, &current, &sensors.readings())?;
            // save rechecks revision under its write lock: concurrent editors cannot
            // overwrite a change committed after validation of this snapshot.
            store.save(config)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(format!("sensor defaults: {e}")))
    }

    async fn get_display_settings(&self) -> Result<llcontrol_ipc::DisplaySettings> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.persistence.display_settings().get().await
    }
    async fn set_display_settings(
        &self,
        settings: llcontrol_ipc::DisplaySettings,
    ) -> Result<llcontrol_ipc::DisplaySettings> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.persistence.display_settings().save(settings).await
    }
    async fn get_system_spec(&self, refresh: bool) -> Result<llcontrol_ipc::SystemSpec> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.system_spec.get(refresh).await
    }
    async fn list_sensors(&self) -> Result<Vec<llcontrol_ipc::SensorReading>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let mut rows = self.sensors.readings();
        for dev in self.manager.list().await {
            if let Some(mut device_rows) = self
                .manager
                .with_device(&dev.id, |d| d.handles.sensor_cache.readings())
                .await
            {
                rows.append(&mut device_rows);
            }
        }
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(rows)
    }
    // -------------------------------------------------------------------
    // Discovery
    // -------------------------------------------------------------------

    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        Ok(self.manager.list().await)
    }

    /// Global source intent; ordinary maintenance admission still applies.
    async fn get_fps_source(&self) -> Result<llcontrol_ipc::FpsSourceSettings> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.persistence
            .fps_source()
            .get(self.sensors.clone())
            .await
    }
    async fn set_fps_source(
        &self,
        settings: llcontrol_ipc::FpsSourceSettings,
    ) -> Result<llcontrol_ipc::FpsSourceSettings> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.persistence
            .fps_source()
            .save(settings, self.sensors.clone())
            .await
    }

    /// Available with zero ordinary devices and while maintenance admission is
    /// sealed. Uses private read-only files only, never journal redo or HID.
    async fn get_firmware_recovery(&self) -> Result<llcontrol_ipc::FirmwareRecoveryStatus> {
        self.stats.inc_ipc_request();
        let permit = self
            .firmware_inspections
            .clone()
            .try_acquire_owned()
            .map_err(|_| IpcError::DeviceBusy("firmware recovery inspection busy".into()))?;
        let store = self.persistence.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.firmware_operations().recovery_status()
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("firmware recovery inspection worker lost: {e}")))
    }

    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // One actor-owned request. PN1 is optional; another PN0 cannot be a build date.
        let (reply, rx) = oneshot::channel();
        handles
            .hid_io_tx
            .send(HidCommand::Firmware { reply })
            .await
            .map_err(|error| IpcError::DeviceBusy(format!("firmware actor closed: {error}")))?;
        let responses = rx
            .await
            .map_err(|error| IpcError::DeviceBusy(format!("firmware reply dropped: {error}")))?
            .map_err(|error| IpcError::DeviceBusy(format!("firmware read: {error}")))?;
        let parsed = FirmwareVersion::from_optional_responses(
            &responses.version,
            responses.datetime.as_ref().map(|bytes| bytes.as_slice()),
        )
        .map_err(|error| IpcError::ProtocolMismatch(format!("firmware decode: {error}")))?;
        let version = parsed.user_version.clone();
        let raw_token = format!(
            "{},{},{},{},{},{},{}|{}",
            parsed.chip_code,
            parsed.boot_code,
            parsed.kind,
            parsed.model_a,
            parsed.model_b,
            parsed.software_version,
            parsed.user_version,
            parsed.raw_datetime,
        );
        let value = json!({
            "version": version,
            "raw_token": raw_token,
            "build_datetime": (!parsed.raw_datetime.is_empty()).then_some(parsed.raw_datetime),
        });
        serde_json::from_value::<FirmwareInfo>(value)
            .map_err(|e| IpcError::Serialization(e.to_string()))
    }

    // -------------------------------------------------------------------
    // Sensors
    // -------------------------------------------------------------------

    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // `borrow()` is a `Ref`; clone the inner `Copy` value out before
        // dropping the borrow (Ref is not Send).
        let snap = *handles.snapshot_rx.borrow();
        Ok(snap)
    }

    async fn subscribe_snapshots(
        &self,
        dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        // `WatchStream` yields the current value once on first poll,
        // then every subsequent change. The IPC server wraps the items
        // we yield here into `"snapshot"` notifications on the same
        // connection (see `llcontrol_ipc::server::spawn_snapshot_pump`).
        let stream = WatchStream::new(handles.snapshot_rx);
        Ok(Box::pin(stream))
    }

    // -------------------------------------------------------------------
    // PWM
    // -------------------------------------------------------------------

    async fn set_fan_pwm(&self, dev: DeviceId, percent: Pwm) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.manual_cooling(
            dev,
            llcontrol_ipc::CoolingTarget::Fan,
            percent.percent(),
            false,
        )
        .await
    }
    async fn set_pump_pwm(&self, dev: DeviceId, percent: Pwm, source: PwmSource) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let motherboard = match source {
            PwmSource::Mcu => false,
            PwmSource::Application => true,
            _ => return Err(IpcError::InvalidArgument("unsupported pump source".into())),
        };
        self.manual_cooling(
            dev,
            llcontrol_ipc::CoolingTarget::Pump,
            percent.percent(),
            motherboard,
        )
        .await
    }
    async fn get_cooling_state(&self, dev: DeviceId) -> Result<llcontrol_ipc::CoolingState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .cooling_tx
            .send(crate::actor::cooling::CoolingCommand::State(reply))
            .await
            .map_err(|_| IpcError::DeviceBusy("cooling actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("cooling reply dropped".into()))
    }
    async fn get_cooling_modes(
        &self,
        dev: DeviceId,
    ) -> Result<Option<llcontrol_ipc::CoolingModeLibrary>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::ReadCoolingModes { reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?.map_err(IpcError::DeviceBusy)
    }
    async fn save_cooling_mode(
        &self,
        dev: DeviceId,
        target: llcontrol_ipc::CoolingTarget,
        expected_revision: String,
        profile: llcontrol_ipc::CoolingModeProfile,
    ) -> Result<llcontrol_ipc::CoolingModeLibrary> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::SaveCoolingMode {
                target,
                expected_revision,
                profile,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?.map_err(IpcError::DeviceBusy)
    }
    async fn initialize_cooling_modes(
        &self,
        dev: DeviceId,
        sensor: String,
    ) -> Result<llcontrol_ipc::CoolingModeLibrary> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let (supported, handles) = self
            .manager
            .with_device(&dev, |device| {
                (
                    device.descriptor.family.as_str() == "galahad2-lcdv2",
                    device.handles.clone(),
                )
            })
            .await
            .ok_or_else(|| IpcError::DeviceNotFound(dev.as_str().into()))?;
        if !supported {
            return Err(IpcError::NotImplemented(
                "standard cooling modes are only implemented for LCDV2".into(),
            ));
        }
        let mut rows = self.sensors.readings();
        rows.extend(handles.sensor_cache.readings());
        if !rows
            .iter()
            .any(|r| r.id == sensor && r.unit == llcontrol_ipc::SensorUnit::Celsius)
        {
            return Err(IpcError::InvalidArgument(
                "initialization requires a known host or this device temperature sensor".into(),
            ));
        }
        let (reply, rx) = oneshot::channel();
        handles
            .settings_tx
            .send(SettingsUpdate::InitializeCoolingModes { sensor, reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?.map_err(IpcError::DeviceBusy)
    }
    async fn apply_cooling_mode(
        &self,
        dev: DeviceId,
        target: llcontrol_ipc::CoolingTarget,
        expected_revision: String,
        id: String,
    ) -> Result<llcontrol_ipc::CoolingState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .cooling_tx
            .send(crate::actor::cooling::CoolingCommand::Stored {
                target,
                expected_revision,
                id,
                reply,
            })
            .await
            .map_err(|_| IpcError::DeviceBusy("cooling actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("cooling reply dropped".into()))?
    }
    async fn set_cooling_policy(
        &self,
        dev: DeviceId,
        target: llcontrol_ipc::CoolingTarget,
        policy: llcontrol_ipc::CoolingPolicy,
    ) -> Result<llcontrol_ipc::CoolingState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .cooling_tx
            .send(crate::actor::cooling::CoolingCommand::Set {
                target,
                policy,
                reply,
            })
            .await
            .map_err(|_| IpcError::DeviceBusy("cooling actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("cooling reply dropped".into()))?
    }

    // -------------------------------------------------------------------
    // Lighting
    // -------------------------------------------------------------------

    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind) -> Result<LightingInfo> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        // Catalog is static; read the committed manual base from the sole RGB
        // owner. This does not issue a HID query or return alert/dynamic pixels.
        let handles = self.handles(&dev).await?;
        let modes: Vec<String> = match kind {
            LightingKind::Fan => llcontrol_ipc::FAN_LIGHTING_MODES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            LightingKind::Pump => llcontrol_ipc::PUMP_LIGHTING_MODES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported LightingKind: {other:?}"
                )))
            }
        };
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::Base { kind, reply })
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting actor closed".into()))?;
        let base = rx
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting base reply dropped".into()))?;
        Ok(LightingInfo {
            dynamic_mode_supported: kind == LightingKind::Pump,
            fan_topology: base.fan_topology,
            base: base.current,
            profiles: base.profiles,
            catalog: llcontrol_ipc::lighting_catalog(kind),
            max_speed: 100,
            max_brightness: 100,
            modes,
        })
    }

    async fn set_lighting(
        &self,
        dev: DeviceId,
        kind: LightingKind,
        cfg: LightingConfig,
    ) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        if cfg.dynamic_mode.is_some() && kind != LightingKind::Pump {
            return Err(IpcError::InvalidArgument(
                "dynamic mode is pump-only".into(),
            ));
        }
        if let Some(topology) = &cfg.fan_topology {
            if kind != LightingKind::Fan {
                return Err(IpcError::InvalidArgument("fan topology is fan-only".into()));
            }
            topology.validate()?;
        }
        if !cfg.additional_colors.is_empty()
            && (cfg.additional_colors.len() > 3
                || llcontrol_ipc::lighting_color_count(kind, &cfg.mode)
                    != Some(cfg.additional_colors.len() + 1))
        {
            return Err(IpcError::InvalidArgument(
                "palette does not match lighting mode".into(),
            ));
        }
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        let msg = match kind {
            LightingKind::Fan => LightingStateMsg::SetFan {
                state: LightingState {
                    mode: cfg.mode,
                    brightness: cfg.brightness_percent,
                    speed: cfg.speed_percent,
                    direction: cfg.direction,
                    color: rgb_to_hex(cfg.color),
                    additional_colors: cfg.additional_colors.into_iter().map(rgb_to_hex).collect(),
                    fan_topology: cfg.fan_topology,
                    disabled: cfg.disabled,
                },
                reply,
            },
            LightingKind::Pump => LightingStateMsg::SetPumpWithDynamic {
                restore_dynamic: cfg.dynamic_mode.unwrap_or(false),
                state: PumpLightingState {
                    scope: cfg.scope.unwrap_or_else(|| "inner".into()),
                    mode: cfg.mode,
                    brightness: cfg.brightness_percent,
                    speed: cfg.speed_percent,
                    direction: cfg.direction,
                    color: rgb_to_hex(cfg.color),
                    additional_colors: cfg.additional_colors.into_iter().map(rgb_to_hex).collect(),
                    disabled: cfg.disabled,
                },
                reply,
            },
            _ => {
                return Err(IpcError::InvalidArgument(
                    "unsupported lighting kind".into(),
                ))
            }
        };
        handles
            .lighting_state_tx
            .send(msg)
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("lighting reply dropped".into()))?
    }

    async fn set_led_colors(
        &self,
        dev: DeviceId,
        target: LedTarget,
        colors: Vec<Rgb>,
    ) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lighting_state_tx
            .send(LightingStateMsg::Direct {
                target,
                colors,
                reply,
            })
            .await
            .map_err(|_| IpcError::DeviceBusy("lighting actor closed".into()))?;
        rx.await
            .map_err(|_| IpcError::DeviceBusy("lighting reply dropped".into()))?
    }

    // -------------------------------------------------------------------
    // LCD
    // -------------------------------------------------------------------

    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        // Lookup validates the device is attached. galahad2-lcdv2
        // always has an LCD; future families will read this from a
        // family-registry capability bit.
        let _ = self.handles(&dev).await?;
        Ok(true)
    }

    async fn set_lcd_config(&self, dev: DeviceId, cfg: LcdConfig) -> Result<()> {
        self.patch_lcd_config(
            dev,
            LcdConfigPatch {
                mode: Some(cfg.mode),
                brightness_percent: Some(cfg.brightness_percent),
                rotation_degrees: Some(cfg.rotation_degrees),
            },
        )
        .await
        .map(|_| ())
    }
    async fn patch_lcd_config(&self, dev: DeviceId, patch: LcdConfigPatch) -> Result<LcdState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::Patch { patch, reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?
    }
    async fn media_upload(
        &self,
        request: llcontrol_ipc::MediaUploadRequest,
    ) -> Result<llcontrol_ipc::MediaUploadState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let uploads = self.media_uploads.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            uploads.handle(request)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn reapply_group_import(
        &self,
        dev: DeviceId,
        group: llcontrol_ipc::FullProfileGroup,
        operation_id: String,
        expected: String,
    ) -> Result<llcontrol_ipc::FullGroupReadback> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.group_import_reapply(dev, group, operation_id, expected)
            .await
    }
    async fn preview_windows_control_import(
        &self,
        dev: DeviceId,
        draft_json: String,
    ) -> Result<llcontrol_ipc::WindowsControlPreview> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let supported = self
            .manager
            .with_device(&dev, |d| d.descriptor.family.as_str() == "galahad2-lcdv2")
            .await
            .ok_or_else(|| IpcError::DeviceNotFound(dev.as_str().into()))?;
        if !supported {
            return Err(IpcError::InvalidArgument(
                "Windows control import requires LCDV2".into(),
            ));
        }
        let draft = llcontrol_profile::control::parse(draft_json.as_bytes())
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let (target, profile) = self
            .group_snapshot(dev, llcontrol_ipc::FullProfileGroup::FanAndLighting)
            .await?;
        let proposed = crate::windows_control::profile(&draft, &profile)
            .and_then(|p| Ok(crate::group_exports::summary(&p)?))
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        Ok(llcontrol_ipc::WindowsControlPreview {
            target,
            proposed,
            lighting: llcontrol_ipc::WindowsControlLightingReview {
                fan_topology: draft.lighting.fan_topology.clone(),
                fan_modes: draft.lighting.fan_modes.values().cloned().collect(),
                pump_modes: draft.lighting.pump_modes.values().cloned().collect(),
                dynamic_rgb_profiles: draft.lighting.dynamic_rgb_profiles.clone(),
            },
        })
    }
    async fn import_windows_control(
        &self,
        dev: DeviceId,
        operation_id: String,
        expected: String,
        draft_json: String,
    ) -> Result<llcontrol_ipc::FullGroupCommit> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        crate::profile_group::validate_operation_id(&operation_id)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        if expected.len() > 1024 {
            return Err(IpcError::InvalidArgument(
                "group revision token too long".into(),
            ));
        }
        let expected: crate::profile_group::GroupRevision = serde_json::from_str(&expected)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let supported = self
            .manager
            .with_device(&dev, |d| d.descriptor.family.as_str() == "galahad2-lcdv2")
            .await
            .ok_or_else(|| IpcError::DeviceNotFound(dev.as_str().into()))?;
        if !supported {
            return Err(IpcError::InvalidArgument(
                "Windows control import requires LCDV2".into(),
            ));
        }
        let draft = llcontrol_profile::control::parse(draft_json.as_bytes())
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let (_, target) = self
            .group_snapshot(dev.clone(), llcontrol_ipc::FullProfileGroup::FanAndLighting)
            .await?;
        let prepared = Arc::new(
            crate::profile_archive::PreparedGroup::windows_control(&draft, &target)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?,
        );
        self.commit_control_profile(dev, operation_id, expected, prepared)
            .await
    }
    async fn preview_fan_and_lighting_reset(
        &self,
        dev: DeviceId,
        sensor_id: String,
    ) -> Result<llcontrol_ipc::ControlResetPreview> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let defaults = crate::control_reset::profile(&sensor_id)
            .and_then(|p| Ok(crate::group_exports::summary(&p)?))
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        let (target, _) = self
            .group_snapshot(dev, llcontrol_ipc::FullProfileGroup::FanAndLighting)
            .await?;
        Ok(llcontrol_ipc::ControlResetPreview { target, defaults })
    }
    async fn reset_fan_and_lighting(
        &self,
        dev: DeviceId,
        operation_id: String,
        expected: String,
        sensor_id: String,
    ) -> Result<llcontrol_ipc::FullGroupCommit> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.control_group_reset(dev, operation_id, expected, sensor_id)
            .await
    }
    async fn read_group_import(
        &self,
        dev: DeviceId,
        group: llcontrol_ipc::FullProfileGroup,
        operation_id: String,
    ) -> Result<llcontrol_ipc::FullGroupReadback> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.group_import_readback(dev, group, operation_id).await
    }
    async fn preview_windows_screen_upload(
        &self,
        dev: DeviceId,
        id: String,
        source: llcontrol_ipc::WindowsScreenPreviewSource,
    ) -> Result<Vec<u8>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        self.group_uploads
            .preview_windows(
                &dev,
                &id,
                source,
                crate::group_uploads::preview::PreviewContext {
                    archives: crate::profile_archive::ProfileArchives::new(
                        self.persistence.theme_assets(),
                        self.persistence.theme_fonts(),
                        self.persistence.media_assets(),
                    ),
                    sensors: vec![self.sensors.clone(), handles.sensor_cache],
                    display_unit: self.persistence.display_settings().live_unit(),
                    permit,
                },
            )
            .await
    }
    async fn group_upload(
        &self,
        dev: DeviceId,
        group: llcontrol_ipc::FullProfileGroup,
        request: llcontrol_ipc::GroupUploadRequest,
    ) -> Result<llcontrol_ipc::GroupUploadState> {
        use llcontrol_ipc::GroupUploadRequest as Request;
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        match request {
            Request::Begin { id, size, expected } => {
                if expected.len() > 1024 {
                    return Err(IpcError::InvalidArgument(
                        "group revision token too long".into(),
                    ));
                }
                let revision: crate::profile_group::GroupRevision = serde_json::from_str(&expected)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                let expected = serde_json::to_string(&revision)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                if let Some(state) = self
                    .group_uploads
                    .existing(&dev, group, &id, size, &expected)
                    .await?
                {
                    return Ok(state);
                }
                let (snapshot, _) = self.group_snapshot(dev, group).await?;
                if snapshot.expected != expected {
                    return Err(IpcError::DeviceBusy(
                        "target changed; read snapshot again".into(),
                    ));
                }
                self.group_uploads.begin(id, snapshot, size).await
            }
            Request::BeginWindowsScreen {
                id,
                size,
                expected,
                source,
            } => {
                if group != llcontrol_ipc::FullProfileGroup::Screen || expected.len() > 1024 {
                    return Err(IpcError::InvalidArgument(
                        "Windows Screen requires a Screen target and bounded revision token".into(),
                    ));
                }
                let revision: crate::profile_group::GroupRevision = serde_json::from_str(&expected)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                let expected = serde_json::to_string(&revision).map_err(lcd_closed)?;
                // Bound and parse before retaining any source options in the registry.
                llcontrol_profile::linux_screen::parse_bindings(source.bindings_json.as_bytes())
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                llcontrol_profile::screen_plan::parse_resource_map(
                    source.resources_json.as_bytes(),
                )
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                if let Some(state) = self
                    .group_uploads
                    .existing_source(&dev, group, &id, size, &expected, Some(&source))
                    .await?
                {
                    return Ok(state);
                }
                let supported = self
                    .manager
                    .with_device(&dev, |d| d.descriptor.family.as_str() == "galahad2-lcdv2")
                    .await
                    .ok_or_else(|| IpcError::DeviceNotFound(dev.as_str().into()))?;
                if !supported {
                    return Err(IpcError::InvalidArgument(
                        "Windows Screen import requires LCDV2".into(),
                    ));
                }
                let (snapshot, target) = self.group_snapshot(dev, group).await?;
                if snapshot.expected != expected {
                    return Err(IpcError::DeviceBusy(
                        "target changed; read snapshot again".into(),
                    ));
                }
                self.group_uploads
                    .begin_source(id, snapshot, size, Some((source, target)))
                    .await
            }
            Request::Chunk { id, offset, bytes } => {
                self.group_uploads
                    .chunk(&dev, group, &id, offset, bytes)
                    .await
            }
            Request::Inspect { id } => {
                self.group_uploads
                    .inspect_source(
                        &dev,
                        group,
                        &id,
                        Some(crate::profile_archive::ProfileArchives::new(
                            self.persistence.theme_assets(),
                            self.persistence.theme_fonts(),
                            self.persistence.media_assets(),
                        )),
                    )
                    .await
            }
            Request::Finish { id } => self.finish_group_upload(dev, group, id).await,
            Request::Status { id } => self.group_uploads.status(&dev, group, &id).await,
            Request::Cancel { id } => self.group_uploads.cancel(&dev, group, &id).await,
            Request::Release { id } => self.group_uploads.release(&dev, group, &id).await,
        }
    }

    async fn group_archive(
        &self,
        dev: DeviceId,
        group: llcontrol_ipc::FullProfileGroup,
        request: llcontrol_ipc::GroupArchiveRequest,
    ) -> Result<llcontrol_ipc::GroupArchiveReply> {
        use llcontrol_ipc::{GroupArchiveReply as Reply, GroupArchiveRequest as Request};
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        match request {
            Request::Snapshot => {
                let (snapshot, _) = self.group_snapshot(dev, group).await?;
                Ok(Reply::Snapshot { snapshot })
            }
            Request::BeginExport { id, expected } => {
                if expected.len() > 1024 {
                    return Err(IpcError::InvalidArgument(
                        "group revision token too long".into(),
                    ));
                }
                let revision: crate::profile_group::GroupRevision = serde_json::from_str(&expected)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                let canonical = serde_json::to_string(&revision)
                    .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
                if let Some(reply) = self
                    .group_exports
                    .existing(&dev, group, &id, &canonical)
                    .await?
                {
                    return Ok(reply);
                }
                let (snapshot, profile) = self.group_snapshot(dev, group).await?;
                if snapshot.expected != canonical {
                    return Err(IpcError::DeviceBusy(
                        "source group revision changed; read snapshot again".into(),
                    ));
                }
                self.group_exports.begin(id, snapshot, profile).await
            }
            Request::Status { id } => self.group_exports.status(&dev, group, &id).await,
            Request::Read { id, offset, length } => {
                self.group_exports
                    .read(&dev, group, &id, offset, length)
                    .await
            }
            Request::Cancel { id } => self.group_exports.cancel(&dev, group, &id).await,
            Request::Release { id } => self.group_exports.release(&dev, group, &id).await,
        }
    }
    async fn profile_archive(
        &self,
        request: llcontrol_ipc::ProfileArchiveRequest,
    ) -> Result<llcontrol_ipc::ProfileArchiveReply> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        self.archive_transfers.handle(request).await
    }
    async fn font_upload(
        &self,
        request: llcontrol_ipc::FontUploadRequest,
    ) -> Result<llcontrol_ipc::FontUploadState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let uploads = self.font_uploads.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            uploads.handle(request)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn list_named_theme_assets(&self) -> Result<Vec<llcontrol_ipc::ThemeAssetInfo>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.list_named()
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn delete_theme_asset(
        &self,
        key: String,
        expected_revision: u64,
        keep: Vec<String>,
    ) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let assets = self.theme_assets.clone();
        let persistence = self.persistence.clone();
        let profiles = self.profiles.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            assets.delete_unreferenced(&key, expected_revision, &keep, || {
                let referenced = persistence.referenced_theme_assets()?;
                // Read both sources even when one references the key: malformed
                // durable state must never be treated as permission to delete.
                let library = profiles.list()?;
                Ok(referenced.contains(&key) || library.profiles.iter().any(|profile| {
                    matches!(&profile.content, llcontrol_ipc::ProfileContent::Screen(theme) if theme.image_assets().contains(key.as_str()))
                }))
            })
        }).await.map_err(lcd_closed)?.map_err(|e| IpcError::InvalidArgument(format!("image deletion: {e}")))
    }
    async fn rename_theme_asset(
        &self,
        key: String,
        name: String,
        expected_revision: u64,
    ) -> Result<llcontrol_ipc::ThemeAssetInfo> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.rename(&key, &name, expected_revision)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn list_theme_assets(&self) -> Result<Vec<String>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.list()
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn read_theme_asset_for_edit(&self, key: String) -> Result<Vec<u8>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.read_for_edit(&key)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn thumbnail_theme_asset(&self, key: String) -> Result<Vec<u8>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.thumbnail(&key)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn import_theme_asset(&self, bytes: Vec<u8>) -> Result<String> {
        self.import_named_theme_asset(bytes, None).await
    }
    async fn import_named_theme_asset(
        &self,
        bytes: Vec<u8>,
        name: Option<String>,
    ) -> Result<String> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        if bytes.len() > 1024 * 1024 {
            return Err(IpcError::InvalidArgument(
                "theme upload exceeds 1 MiB IPC limit".into(),
            ));
        }
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.theme_assets.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.import_named(&bytes, name.as_deref())
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn preview_lcd_theme(
        &self,
        dev: Option<DeviceId>,
        theme: llcontrol_ipc::LcdTheme,
    ) -> Result<Vec<u8>> {
        self.preview_lcd_theme_at(dev, theme, 0).await
    }
    async fn preview_lcd_theme_at(
        &self,
        dev: Option<DeviceId>,
        theme: llcontrol_ipc::LcdTheme,
        elapsed_seconds: u64,
    ) -> Result<Vec<u8>> {
        self.begin_lcd_theme_preview(dev, theme, elapsed_seconds)
            .await?
            .await
    }
    async fn begin_lcd_theme_preview(
        &self,
        dev: Option<DeviceId>,
        theme: llcontrol_ipc::LcdTheme,
        elapsed_seconds: u64,
    ) -> Result<futures::future::BoxFuture<'static, Result<Vec<u8>>>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let mut caches = vec![self.sensors.clone()];
        if let Some(dev) = dev {
            caches.push(self.handles(&dev).await?.sensor_cache);
        }
        let store = self.theme_assets.clone();
        let fonts = self.persistence.theme_fonts();
        let media = self.persistence.media_assets();
        let display_unit = self.persistence.display_settings().live_unit();
        Ok(Box::pin(async move {
            let (bytes, _permit) = join_theme_job(permit, async move {
                let runtime =
                    Self::prepare_theme(store, fonts, caches, media.clone(), theme, display_unit)
                        .await?;
                if runtime.theme().background_video.is_some() {
                    crate::media_composition::preview(media, runtime, elapsed_seconds)
                        .await
                        .map_err(lcd_closed)
                } else {
                    Ok(runtime.frame_at(elapsed_seconds).await?.to_vec())
                }
            })
            .await?;
            Ok(bytes)
        }))
    }
    async fn begin_lcd_editor_preview(
        &self,
        dev: Option<DeviceId>,
        theme: llcontrol_ipc::LcdTheme,
        elapsed_millis: u64,
        layer_index: Option<u16>,
    ) -> Result<futures::future::BoxFuture<'static, Result<llcontrol_ipc::LcdEditorPreview>>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let mut caches = vec![self.sensors.clone()];
        if let Some(dev) = &dev {
            caches.push(self.handles(dev).await?.sensor_cache);
        }
        let store = self.theme_assets.clone();
        let fonts = self.persistence.theme_fonts();
        let media = self.persistence.media_assets();
        let display_unit = self.persistence.display_settings().live_unit();
        let cached = self.editor_cache.get(&dev, &theme);
        let editor_cache = self.editor_cache.clone();
        let video_jobs = self.editor_video_jobs.clone();
        Ok(Box::pin(async move {
            let (preview, _permit) = join_theme_job(permit, async move {
                let runtime = if let Some(runtime) = cached {
                    runtime
                } else {
                    let runtime = Self::prepare_theme(
                        store,
                        fonts,
                        caches,
                        media.clone(),
                        theme.clone(),
                        display_unit,
                    )
                    .await?;
                    editor_cache.store(dev, theme, runtime.clone());
                    runtime
                };
                let frame = if let Some(key) = runtime.theme().background_video.clone() {
                    Some(
                        editor_cache
                            .video_frame(key, elapsed_millis, media, video_jobs)
                            .await
                            .map_err(lcd_closed)?,
                    )
                } else {
                    None
                };
                tokio::task::spawn_blocking(move || {
                    runtime.editor_preview(frame, elapsed_millis, layer_index)
                })
                .await
                .map_err(lcd_closed)?
            })
            .await?;
            Ok(preview)
        }))
    }
    async fn set_lcd_theme(
        &self,
        dev: DeviceId,
        theme: llcontrol_ipc::LcdTheme,
    ) -> Result<LcdState> {
        self.begin_lcd_theme(dev, theme).await?.await
    }
    async fn begin_lcd_theme(
        &self,
        dev: DeviceId,
        theme: llcontrol_ipc::LcdTheme,
    ) -> Result<futures::future::BoxFuture<'static, Result<LcdState>>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let handles = self.handles(&dev).await?;
        // Reserve source intent on the owner before slow asset/font preparation.
        // A later Clear or replacement must not be undone by this completed job.
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::BeginThemePreparation { reply })
            .await
            .map_err(lcd_closed)?;
        let preparation = Some(rx.await.map_err(lcd_closed)?);
        let rotation = if theme.requires_streaming() {
            let (reply, state) = oneshot::channel();
            handles
                .lcd_tx
                .send(LcdCommand::State { reply })
                .await
                .map_err(lcd_closed)?;
            llcontrol_image::FrameRotation::try_from(
                state.await.map_err(lcd_closed)?.requested.rotation_degrees,
            )
            .map_err(lcd_closed)?
        } else {
            llcontrol_image::FrameRotation::None
        };
        let caches = vec![self.sensors.clone(), handles.sensor_cache];
        let store = self.theme_assets.clone();
        let fonts = self.persistence.theme_fonts();
        let media = self.persistence.media_assets();
        let display_unit = self.persistence.display_settings().live_unit();
        Ok(Box::pin(async move {
            if theme.requires_streaming() {
                let preparation_media = media.clone();
                let (runtime, permit) = join_theme_job(permit, async move {
                    Self::prepare_theme_oriented(
                        store,
                        fonts,
                        caches,
                        preparation_media,
                        theme,
                        rotation,
                        display_unit,
                    )
                    .await
                })
                .await?;
                let prepared = crate::media_composition::PreparedComposition::prepare_with_permit(
                    media,
                    runtime,
                    Some(permit),
                )
                .await
                .map_err(lcd_closed)?;
                let (reply, rx) = oneshot::channel();
                handles
                    .lcd_tx
                    .send(LcdCommand::SetComposition {
                        prepared,
                        preparation,
                        reply,
                    })
                    .await
                    .map_err(lcd_closed)?;
                // Accepted worker owns the lease until first-frame commit or
                // joined rejection/failure, independently of this RPC waiter.
                return rx.await.map_err(lcd_closed)?;
            }
            // If the caller task is cancelled during preparation, the finished runtime is
            // dropped with its permit. No deferred device command is sent.
            let (prepared, permit) = join_theme_job(permit, async move {
                let runtime =
                    Self::prepare_theme(store, fonts, caches, media.clone(), theme, display_unit)
                        .await?;
                runtime.prepare().await
            })
            .await?;
            let (reply, rx) = oneshot::channel();
            handles
                .lcd_tx
                .send(LcdCommand::SetTheme {
                    prepared,
                    preparation,
                    reply,
                })
                .await
                .map_err(lcd_closed)?;
            // Once enqueued, only the LCD owner may finish the accepted mutation.
            // Keep its slot occupied even if the caller no longer waits for the reply.
            let (state, _permit) =
                join_theme_job(permit, async move { rx.await.map_err(lcd_closed)? }).await?;
            Ok(state)
        }))
    }
    async fn get_imported_lcd_editor(
        &self,
        dev: DeviceId,
    ) -> Result<Option<llcontrol_ipc::LcdTheme>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let (_, profile) = self
            .group_snapshot(dev, llcontrol_ipc::FullProfileGroup::Screen)
            .await?;
        let crate::profile_group::GroupSettings::Screen(screen) = profile.content else {
            return Err(lcd_closed("expected Screen snapshot"));
        };
        Ok(screen.editor.and_then(|editor| editor.theme))
    }
    async fn get_lcd_theme(&self, dev: DeviceId) -> Result<Option<llcontrol_ipc::LcdTheme>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::Theme { reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)
    }
    async fn get_lcd_state(&self, dev: DeviceId) -> Result<LcdState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::State { reply })
            .await
            .map_err(lcd_closed)?;
        let mut state = rx.await.map_err(lcd_closed)?;
        state.persistence = Some(handles.save_state.borrow().clone());
        Ok(state)
    }
    async fn set_lcd_image(&self, dev: DeviceId, jpeg: Vec<u8>) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::SetImage {
                jpeg: Bytes::from(jpeg),
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?.map(|_| ())
    }
    async fn clear_lcd(&self, dev: DeviceId) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::Clear { reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?.map(|_| ())
    }
    async fn get_lcd_media(&self, dev: DeviceId) -> Result<Option<String>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::MediaClip { reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)
    }
    async fn set_lcd_media(&self, dev: DeviceId, key: String) -> Result<LcdState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::BeginThemePreparation { reply })
            .await
            .map_err(lcd_closed)?;
        let preparation = Some(rx.await.map_err(lcd_closed)?);
        let (reply, state) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::State { reply })
            .await
            .map_err(lcd_closed)?;
        let rotation = llcontrol_image::FrameRotation::try_from(
            state.await.map_err(lcd_closed)?.requested.rotation_degrees,
        )
        .map_err(lcd_closed)?;
        if rotation != llcontrol_image::FrameRotation::None {
            let theme = crate::actor::screen_group::media_theme(key.clone());
            let images = self.theme_assets.clone();
            let fonts = self.persistence.theme_fonts();
            let caches = vec![self.sensors.clone(), handles.sensor_cache.clone()];
            let media = self.persistence.media_assets();
            let prepare_media = media.clone();
            let display_unit = self.persistence.display_settings().live_unit();
            let (runtime, permit) = join_theme_job(permit, async move {
                Self::prepare_theme_oriented(
                    images,
                    fonts,
                    caches,
                    prepare_media,
                    theme,
                    rotation,
                    display_unit,
                )
                .await
            })
            .await?;
            let prepared = crate::media_composition::PreparedComposition::prepare_with_permit(
                media,
                runtime,
                Some(permit),
            )
            .await
            .map_err(lcd_closed)?;
            let (reply, response) = oneshot::channel();
            handles
                .lcd_tx
                .send(LcdCommand::SetMediaComposition {
                    key,
                    prepared,
                    preparation,
                    reply,
                })
                .await
                .map_err(lcd_closed)?;
            return response.await.map_err(lcd_closed)?;
        }
        let store = self.persistence.media_assets();
        let saved_key = key.clone();
        let (clip, permit) = tokio::task::spawn_blocking(move || {
            let clip = store
                .acquire_source(&saved_key)
                .map_err(|e| IpcError::InvalidArgument(e.to_string()));
            (clip, permit)
        })
        .await
        .map_err(lcd_closed)?;
        let source = clip?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::SetMediaClip {
                key,
                source,
                preparation,
                reply,
            })
            .await
            .map_err(lcd_closed)?;
        let (state, _permit) =
            join_theme_job(permit, async move { rx.await.map_err(lcd_closed)? }).await?;
        Ok(state)
    }
    async fn import_prepared_clip(
        &self,
        name: String,
        clip: H264Clip,
    ) -> Result<llcontrol_ipc::PreparedClipInfo> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.media_assets();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.import(name, clip)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn list_prepared_clips(&self) -> Result<Vec<llcontrol_ipc::PreparedClipInfo>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.media_assets();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.list()
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn delete_prepared_clip(&self, key: String) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let persistence = self.persistence.clone();
        let store = persistence.media_assets();
        let profiles = self.profiles.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.delete_unreferenced(&key, || {
                let device_reference = persistence.referenced_media_clips()?.contains(&key);
                let library = profiles.list()?;
                Ok(device_reference
                    || library
                        .profiles
                        .iter()
                        .any(|profile| match &profile.content {
                            llcontrol_ipc::ProfileContent::ScreenMedia(saved) => saved == &key,
                            llcontrol_ipc::ProfileContent::Screen(theme) => {
                                theme.video_assets().contains(key.as_str())
                            }
                            _ => false,
                        }))
            })
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn open_media_export(&self, key: String) -> Result<llcontrol_ipc::MediaExport> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let source = self
            .media_exports
            .open(key)
            .await
            .map_err(|error| IpcError::InvalidArgument(error.to_string()))?;
        Ok(llcontrol_ipc::MediaExport {
            info: llcontrol_ipc::MediaExportInfo {
                fps: source.fps,
                frames: source.frames,
                encoded_bytes: source.encoded_bytes,
            },
            units: source
                .units
                .map(|unit| unit.map_err(|error| IpcError::InvalidArgument(error.to_string())))
                .boxed(),
        })
    }
    async fn load_media_first_frame(&self, key: String) -> Result<llcontrol_ipc::H264AccessUnit> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.media_assets();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            // Keep the descriptor's pin through the read. The returned frame
            // owns its bytes and must not retain a library deletion reference.
            store.acquire_source(&key)?.read_unit(0)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn load_prepared_clip(&self, key: String) -> Result<H264Clip> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let permit = self
            .theme_jobs
            .clone()
            .try_acquire_owned()
            .map_err(lcd_closed)?;
        let store = self.persistence.media_assets();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store.load(&key)
        })
        .await
        .map_err(lcd_closed)?
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
    async fn play_lcd_clip(&self, dev: DeviceId, clip: H264Clip, loops: u16) -> Result<LcdState> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::PlayClip { clip, loops, reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)?
    }
    async fn get_lcd_playback_stats(&self, dev: DeviceId) -> Result<LcdPlaybackStats> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let handles = self.handles(&dev).await?;
        let (reply, rx) = oneshot::channel();
        handles
            .lcd_tx
            .send(LcdCommand::Stats { reply })
            .await
            .map_err(lcd_closed)?;
        rx.await.map_err(lcd_closed)
    }

    // -------------------------------------------------------------------
    // Maintenance
    // -------------------------------------------------------------------

    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        // Validate the device exists -- minting a token for an absent
        // device is meaningless and would just expire unused.
        let _ = self.handles(&dev).await?;
        let token = mint_reset_token();
        let expires_at = Instant::now() + RESET_TOKEN_TTL;
        self.reset_tokens
            .lock()
            .await
            .insert(dev, (token.clone(), expires_at));
        Ok(token)
    }

    async fn reset_device(&self, dev: DeviceId, confirm: ResetToken) -> Result<()> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();

        // Pop the entry first so the token is single-use even on failure
        // -- a botched confirm should not leave the slot live for a
        // retry attempt with a different token guess.
        let entry = self.reset_tokens.lock().await.remove(&dev);
        let (expected, expires_at) = entry.ok_or(IpcError::ConfirmationRequired)?;
        if expected.0 != confirm.0 || Instant::now() >= expires_at {
            return Err(IpcError::ConfirmationRequired);
        }

        let handles = self.handles(&dev).await?;
        // Reset replies with one A-class packet; use WriteRead64 so we
        // observe the firmware's success ack before returning.
        let cmd = Command::ResetDevice as u8;
        let req = build_reset_request().to_vec();
        let buf = write_read_64(&handles, req, cmd).await?;
        parse_reset_response(&buf)
            .map_err(|e| IpcError::ProtocolMismatch(format!("reset ack: {e}")))?;
        Ok(())
    }

    async fn raw_send(
        &self,
        dev: DeviceId,
        framing: FramingKind,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        if !self.config.daemon.allow_raw_send {
            return Err(IpcError::NotImplemented(
                "raw_send disabled; set daemon.allow_raw_send=true in daemon.toml to enable".into(),
            ));
        }
        if bytes.len() < 2 {
            return Err(IpcError::InvalidArgument(
                "raw_send: bytes must include at least [report_id, cmd]".into(),
            ));
        }
        let expected_cmd = bytes[1];
        let handles = self.handles(&dev).await?;
        // FramingKind::A => 64-byte read with report_id 0x01.
        // FramingKind::B => 512-byte read with report_id 0x02.
        // FramingKind::C => 512-byte read with report_id 0x03.
        // The drain loop in hid_io matches the report_id to discriminate
        // the real response from stale frames; passing the wrong id
        // poisons the USB buffer for subsequent reads.
        let buf: Vec<u8> = match framing {
            FramingKind::A => write_read_64(&handles, bytes, expected_cmd).await?.to_vec(),
            FramingKind::B => write_read_512(&handles, bytes, 0x02, expected_cmd)
                .await?
                .to_vec(),
            FramingKind::C => write_read_512(&handles, bytes, 0x03, expected_cmd)
                .await?
                .to_vec(),
            other => {
                return Err(IpcError::InvalidArgument(format!(
                    "unsupported FramingKind: {other:?}"
                )))
            }
        };
        Ok(buf)
    }

    async fn daemon_stats(&self) -> Result<DaemonStats> {
        let _admission = self.admit()?;
        self.stats.inc_ipc_request();
        let n = u32::try_from(self.manager.list().await.len()).unwrap_or(u32::MAX);
        Ok(self.stats.snapshot(n))
    }
}

fn lcd_closed(error: impl std::fmt::Display) -> IpcError {
    IpcError::DeviceBusy(format!("display actor unavailable: {error}"))
}

/// Cancellation drops the waiter, not the work owning the concurrency slot.
/// Return the permit with the result so multi-stage jobs do not open a gap.
async fn join_theme_job<T: Send + 'static, P: Send + 'static>(
    permit: P,
    work: impl std::future::Future<Output = Result<T>> + Send + 'static,
) -> Result<(T, P)> {
    tokio::spawn(async move {
        let value = work.await?;
        Ok((value, permit))
    })
    .await
    .map_err(lcd_closed)?
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cancelled_theme_waiter_retains_capacity_until_background_work_finishes() {
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let (started, start) = tokio::sync::oneshot::channel();
        let (finish, done) = tokio::sync::oneshot::channel();
        let waiter = tokio::spawn(super::join_theme_job(permit, async move {
            started.send(()).unwrap();
            done.await.unwrap();
            Ok(())
        }));
        start.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(semaphore.clone().try_acquire_owned().is_err());
        finish.send(()).unwrap();
        let permit = tokio::time::timeout(std::time::Duration::from_secs(1), semaphore.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn preparation_hands_capacity_to_next_stage_and_failure_releases_it() {
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let (value, permit) = super::join_theme_job(permit, async { Ok(42) })
            .await
            .unwrap();
        assert_eq!(value, 42);
        assert!(semaphore.clone().try_acquire_owned().is_err());
        let result = super::join_theme_job::<(), _>(permit, async {
            Err(llcontrol_ipc::IpcError::InvalidArgument("injected".into()))
        })
        .await;
        assert!(result.is_err());
        assert!(semaphore.try_acquire().is_ok());
    }

    #[tokio::test]
    async fn sealed_admission_waits_for_cancelled_ipc_preparation_until_owned_job_finishes() {
        let gate = crate::admission::Admission::default();
        let slots = crate::admission::WorkSlots::new(1, gate.clone());
        let permit = slots.try_acquire_owned().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, release) = tokio::sync::oneshot::channel();
        let caller = tokio::spawn(super::join_theme_job(permit, async move {
            started.send(()).unwrap();
            release.await.unwrap();
            Ok(())
        }));
        ready.await.unwrap();
        caller.abort();
        let _ = caller.await;
        let drain = tokio::spawn(gate.close().wait());
        tokio::task::yield_now().await;
        assert!(!drain.is_finished());
        assert_eq!(gate.active(), 1);
        finish.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), drain)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(gate.active(), 0);
        assert!(gate.is_closed());
    }

    use crate::codec_str::parse_lcd_mode;
    use llcontrol_protocol::galahad2_lcdv2::lcd::LCDControlMode;

    /// Sanity check that the public LCD modes still round-trip through the
    /// IPC parser. If LocalUI/Application/LocalH264/LocalAVI ever stop
    /// matching, this test fails before we ship a daemon that can't
    /// honour valid IPC requests.
    #[test]
    fn parse_lcd_mode_accepts_safe_modes() {
        assert!(matches!(parse_lcd_mode("LocalUI"), LCDControlMode::LocalUI));
        assert!(matches!(
            parse_lcd_mode("Application"),
            LCDControlMode::Application
        ));
        assert!(matches!(
            parse_lcd_mode("LocalH264"),
            LCDControlMode::LocalH264
        ));
        assert!(matches!(
            parse_lcd_mode("LocalAVI"),
            LCDControlMode::LocalAVI
        ));
    }

    /// LCDTest and LCDSetting are firmware-internal modes that lock the
    /// panel until explicitly cleared; they MUST NOT be reachable through
    /// the IPC parser, regardless of casing/snake-casing. If this test
    /// ever flips, a direct IPC caller can persist a mode that re-bricks
    /// the panel on every hot-plug.
    #[test]
    fn parse_lcd_mode_rejects_unsafe_modes() {
        for unsafe_mode in [
            "LCDTest",
            "lcd_test",
            "LCDSetting",
            "lcd_setting",
            "garbage",
            "",
        ] {
            assert!(
                matches!(parse_lcd_mode(unsafe_mode), LCDControlMode::LocalUI),
                "unsafe/unknown mode {unsafe_mode:?} must fall back to LocalUI"
            );
        }
    }
}
