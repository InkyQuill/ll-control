//! `DaemonRpc` trait — the IPC contract.
//!
//! Implementations: the daemon's `IpcServerImpl` (production) and any
//! mocks the test harness defines. The trait is the seam for a future
//! DBus migration: only the transport changes; the trait stays put.

use crate::*;
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait DaemonRpc: Send + Sync + 'static {
    async fn get_fps_source(&self) -> Result<FpsSourceSettings> {
        Err(IpcError::NotImplemented("get_fps_source".into()))
    }
    async fn set_fps_source(&self, _settings: FpsSourceSettings) -> Result<FpsSourceSettings> {
        Err(IpcError::NotImplemented("set_fps_source".into()))
    }

    async fn get_firmware_recovery(&self) -> Result<FirmwareRecoveryStatus> {
        Err(IpcError::NotImplemented("get_firmware_recovery".into()))
    }

    async fn get_display_settings(&self) -> Result<crate::DisplaySettings> {
        Err(IpcError::NotImplemented("display settings".into()))
    }
    async fn set_display_settings(
        &self,
        _settings: crate::DisplaySettings,
    ) -> Result<crate::DisplaySettings> {
        Err(IpcError::NotImplemented("display settings".into()))
    }
    async fn get_system_spec(&self, _refresh: bool) -> Result<crate::SystemSpec> {
        Err(IpcError::NotImplemented("get_system_spec".into()))
    }
    async fn open_media_export(&self, _key: String) -> Result<MediaExport> {
        Err(IpcError::NotImplemented("open_media_export".into()))
    }

    async fn cleanup_theme_assets(
        &self,
        _delete: bool,
        _keep: Vec<String>,
    ) -> Result<AssetCleanupReport> {
        Err(IpcError::NotImplemented("cleanup_theme_assets".into()))
    }
    async fn cleanup_theme_fonts(
        &self,
        _delete: bool,
        _keep: Vec<String>,
    ) -> Result<AssetCleanupReport> {
        Err(IpcError::NotImplemented("cleanup_theme_fonts".into()))
    }
    async fn list_profiles(&self) -> Result<ProfileLibrary> {
        Err(IpcError::NotImplemented("list_profiles".into()))
    }
    async fn edit_profile(
        &self,
        _expected_revision: u64,
        _edit: ProfileEdit,
    ) -> Result<ProfileLibrary> {
        Err(IpcError::NotImplemented("edit_profile".into()))
    }

    async fn migrate_screen_collections(&self, _dev: DeviceId) -> Result<ScreenCollections> {
        Err(IpcError::NotImplemented(
            "migrate_screen_collections".into(),
        ))
    }
    async fn get_screen_collections(&self, _dev: DeviceId) -> Result<ScreenCollectionsState> {
        Err(IpcError::NotImplemented("get_screen_collections".into()))
    }
    async fn set_screen_collections(
        &self,
        _dev: DeviceId,
        _expected_revision: u64,
        _collections: ScreenCollections,
    ) -> Result<ScreenCollections> {
        Err(IpcError::NotImplemented("set_screen_collections".into()))
    }

    async fn reset_screen(
        &self,
        _dev: DeviceId,
        _operation_id: String,
        _expected_profile_revision: u64,
        _expected_collection_revision: u64,
    ) -> Result<ScreenResetResult> {
        Err(IpcError::NotImplemented("reset_screen".into()))
    }
    async fn reapply_screen_reset(
        &self,
        _dev: DeviceId,
        _operation_id: String,
        _expected_generation: u64,
    ) -> Result<LcdState> {
        Err(IpcError::NotImplemented("reapply_screen_reset".into()))
    }
    async fn get_screen_reset(&self, _dev: DeviceId) -> Result<ScreenResetReadback> {
        Err(IpcError::NotImplemented("get_screen_reset".into()))
    }

    // Discovery
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>>;
    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo>;

    // Sensors
    async fn get_sensor_defaults(&self) -> Result<SensorDefaultsState> {
        Err(IpcError::NotImplemented("get_sensor_defaults".into()))
    }
    async fn set_sensor_defaults(&self, _config: SensorDefaults) -> Result<SensorDefaults> {
        Err(IpcError::NotImplemented("set_sensor_defaults".into()))
    }
    async fn list_sensors(&self) -> Result<Vec<SensorReading>> {
        Err(IpcError::NotImplemented("list_sensors".into()))
    }
    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot>;
    async fn subscribe_snapshots(
        &self,
        dev: DeviceId,
    ) -> Result<BoxStream<'static, SensorSnapshot>>;

    async fn get_cooling_state(&self, _dev: DeviceId) -> Result<CoolingState> {
        Err(IpcError::NotImplemented("get_cooling_state".into()))
    }
    /// Stored policies only; source provenance is omitted and nothing is applied.
    async fn get_cooling_modes(&self, _dev: DeviceId) -> Result<Option<CoolingModeLibrary>> {
        Err(IpcError::NotImplemented("get_cooling_modes".into()))
    }
    /// Save an existing stored mode with CAS; never changes the running policy.
    async fn save_cooling_mode(
        &self,
        _dev: DeviceId,
        _target: CoolingTarget,
        _expected_revision: String,
        _profile: CoolingModeProfile,
    ) -> Result<CoolingModeLibrary> {
        Err(IpcError::NotImplemented("save_cooling_mode".into()))
    }
    /// Create the fixed LCDV2 bank only when none exists. Never applies it.
    async fn initialize_cooling_modes(
        &self,
        _dev: DeviceId,
        _sensor: String,
    ) -> Result<CoolingModeLibrary> {
        Err(IpcError::NotImplemented("initialize_cooling_modes".into()))
    }
    /// Apply a CAS-resolved stored snapshot and retain its identity with intent.
    async fn apply_cooling_mode(
        &self,
        _dev: DeviceId,
        _target: CoolingTarget,
        _expected_revision: String,
        _id: String,
    ) -> Result<CoolingState> {
        Err(IpcError::NotImplemented("apply_cooling_mode".into()))
    }
    async fn set_cooling_policy(
        &self,
        _dev: DeviceId,
        _target: CoolingTarget,
        _policy: CoolingPolicy,
    ) -> Result<CoolingState> {
        Err(IpcError::NotImplemented("set_cooling_policy".into()))
    }
    async fn get_dynamic_rgb(&self, _dev: DeviceId) -> Result<DynamicRgbState> {
        Err(IpcError::NotImplemented("get_dynamic_rgb".into()))
    }
    async fn set_dynamic_rgb(
        &self,
        _dev: DeviceId,
        _config: Option<DynamicRgbConfig>,
    ) -> Result<DynamicRgbState> {
        Err(IpcError::NotImplemented("set_dynamic_rgb".into()))
    }
    async fn get_thermal_state(&self, _dev: DeviceId) -> Result<ThermalState> {
        Err(IpcError::NotImplemented("get_thermal_state".into()))
    }
    async fn set_thermal_config(
        &self,
        _dev: DeviceId,
        _config: ThermalConfig,
    ) -> Result<ThermalState> {
        Err(IpcError::NotImplemented("set_thermal_config".into()))
    }
    // PWM
    async fn set_fan_pwm(&self, dev: DeviceId, percent: Pwm) -> Result<()>;
    async fn set_pump_pwm(&self, dev: DeviceId, percent: Pwm, source: PwmSource) -> Result<()>;

    // Lighting
    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind) -> Result<LightingInfo>;
    async fn set_lighting(
        &self,
        dev: DeviceId,
        kind: LightingKind,
        cfg: LightingConfig,
    ) -> Result<()>;
    async fn set_led_colors(
        &self,
        dev: DeviceId,
        target: LedTarget,
        colors: Vec<Rgb>,
    ) -> Result<()>;

    async fn preview_windows_control_import(
        &self,
        _dev: DeviceId,
        _draft_json: String,
    ) -> Result<crate::WindowsControlPreview> {
        Err(IpcError::NotImplemented(
            "preview_windows_control_import".into(),
        ))
    }
    async fn import_windows_control(
        &self,
        _dev: DeviceId,
        _operation_id: String,
        _expected: String,
        _draft_json: String,
    ) -> Result<crate::FullGroupCommit> {
        Err(IpcError::NotImplemented("import_windows_control".into()))
    }

    async fn preview_fan_and_lighting_reset(
        &self,
        _dev: DeviceId,
        _sensor_id: String,
    ) -> Result<crate::ControlResetPreview> {
        Err(IpcError::NotImplemented(
            "preview_fan_and_lighting_reset".into(),
        ))
    }

    /// Reset all FanAndLighting preferences/slots to versioned LCDV2 defaults.
    /// The caller supplies the target token, operation ID and temperature source.
    async fn reset_fan_and_lighting(
        &self,
        _dev: DeviceId,
        _operation_id: String,
        _expected: String,
        _sensor_id: String,
    ) -> Result<crate::FullGroupCommit> {
        Err(IpcError::NotImplemented("reset_fan_and_lighting".into()))
    }

    async fn reapply_group_import(
        &self,
        _dev: DeviceId,
        _group: crate::FullProfileGroup,
        _operation_id: String,
        _expected: String,
    ) -> Result<crate::FullGroupReadback> {
        Err(IpcError::NotImplemented("reapply_group_import".into()))
    }
    async fn read_group_import(
        &self,
        _dev: DeviceId,
        _group: crate::FullProfileGroup,
        _operation_id: String,
    ) -> Result<crate::FullGroupReadback> {
        Err(IpcError::NotImplemented("read_group_import".into()))
    }

    async fn preview_windows_screen_upload(
        &self,
        _dev: DeviceId,
        _id: String,
        _source: crate::WindowsScreenPreviewSource,
    ) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented(
            "preview_windows_screen_upload".into(),
        ))
    }
    async fn group_upload(
        &self,
        _dev: DeviceId,
        _group: crate::FullProfileGroup,
        _request: crate::GroupUploadRequest,
    ) -> Result<crate::GroupUploadState> {
        Err(IpcError::NotImplemented("group_upload".into()))
    }

    async fn group_archive(
        &self,
        _dev: DeviceId,
        _group: crate::FullProfileGroup,
        _request: crate::GroupArchiveRequest,
    ) -> Result<crate::GroupArchiveReply> {
        Err(IpcError::NotImplemented("group_archive".into()))
    }

    async fn profile_archive(
        &self,
        _request: crate::ProfileArchiveRequest,
    ) -> Result<crate::ProfileArchiveReply> {
        Err(IpcError::NotImplemented("profile_archive".into()))
    }

    async fn font_upload(
        &self,
        _request: crate::FontUploadRequest,
    ) -> Result<crate::FontUploadState> {
        Err(IpcError::NotImplemented("font_upload".into()))
    }

    async fn media_upload(
        &self,
        _request: crate::MediaUploadRequest,
    ) -> Result<crate::MediaUploadState> {
        Err(IpcError::NotImplemented("media_upload".into()))
    }

    // LCD
    async fn list_named_theme_assets(&self) -> Result<Vec<crate::ThemeAssetInfo>> {
        Err(IpcError::NotImplemented("list_named_theme_assets".into()))
    }
    async fn delete_theme_asset(
        &self,
        _key: String,
        _expected_revision: u64,
        _keep: Vec<String>,
    ) -> Result<()> {
        Err(IpcError::NotImplemented("delete_theme_asset".into()))
    }
    async fn rename_theme_asset(
        &self,
        _key: String,
        _name: String,
        _expected_revision: u64,
    ) -> Result<crate::ThemeAssetInfo> {
        Err(IpcError::NotImplemented("rename_theme_asset".into()))
    }
    async fn import_named_theme_asset(
        &self,
        bytes: Vec<u8>,
        name: Option<String>,
    ) -> Result<String> {
        if name.is_some() {
            return Err(IpcError::NotImplemented("named image import".into()));
        }
        self.import_theme_asset(bytes).await
    }
    async fn list_theme_assets(&self) -> Result<Vec<String>> {
        Err(IpcError::NotImplemented("list_theme_assets".into()))
    }
    async fn read_theme_asset_for_edit(&self, _key: String) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented("read_theme_asset_for_edit".into()))
    }
    async fn thumbnail_theme_asset(&self, _key: String) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented("thumbnail_theme_asset".into()))
    }
    async fn import_theme_asset(&self, _bytes: Vec<u8>) -> Result<String> {
        Err(IpcError::NotImplemented("import_theme_asset".into()))
    }
    async fn preview_lcd_theme(
        &self,
        _dev: Option<DeviceId>,
        _theme: crate::LcdTheme,
    ) -> Result<Vec<u8>> {
        Err(IpcError::NotImplemented("preview_lcd_theme".into()))
    }
    /// Timed preview for rotating layers; older implementations retain one-shot support.
    async fn preview_lcd_theme_at(
        &self,
        dev: Option<DeviceId>,
        theme: crate::LcdTheme,
        elapsed_seconds: u64,
    ) -> Result<Vec<u8>> {
        if elapsed_seconds == 0 {
            self.preview_lcd_theme(dev, theme).await
        } else {
            Err(IpcError::NotImplemented("timed theme preview".into()))
        }
    }
    /// Admit in wire order before deferring expensive work. Overrides must reserve
    /// mutation intent here; the returned future may run alongside later requests.
    /// The default completes inline to preserve ordering for existing implementors.
    async fn begin_lcd_theme(
        &self,
        dev: DeviceId,
        theme: crate::LcdTheme,
    ) -> Result<futures::future::BoxFuture<'static, Result<LcdState>>> {
        let result = self.set_lcd_theme(dev, theme).await;
        Ok(Box::pin(async move { result }))
    }
    /// Admit a bounded preview before returning work to the transport.
    async fn begin_lcd_theme_preview(
        &self,
        dev: Option<DeviceId>,
        theme: crate::LcdTheme,
        elapsed_seconds: u64,
    ) -> Result<futures::future::BoxFuture<'static, Result<Vec<u8>>>> {
        let result = self.preview_lcd_theme_at(dev, theme, elapsed_seconds).await;
        Ok(Box::pin(async move { result }))
    }
    /// Read-only editor preview, admitted with the same bounds as ordinary preview.
    async fn begin_lcd_editor_preview(
        &self,
        _dev: Option<DeviceId>,
        _theme: crate::LcdTheme,
        _elapsed_millis: u64,
        _layer_index: Option<u16>,
    ) -> Result<futures::future::BoxFuture<'static, Result<crate::LcdEditorPreview>>> {
        Err(IpcError::NotImplemented("preview_lcd_editor".into()))
    }
    async fn set_lcd_theme(&self, _dev: DeviceId, _theme: crate::LcdTheme) -> Result<LcdState> {
        Err(IpcError::NotImplemented("set_lcd_theme".into()))
    }
    async fn get_lcd_theme(&self, _dev: DeviceId) -> Result<Option<crate::LcdTheme>> {
        Err(IpcError::NotImplemented("get_lcd_theme".into()))
    }
    /// Stored Windows basic editor only; does not read private source history or Apply.
    async fn get_imported_lcd_editor(&self, _dev: DeviceId) -> Result<Option<crate::LcdTheme>> {
        Err(IpcError::NotImplemented("get_imported_lcd_editor".into()))
    }
    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool>;
    async fn set_lcd_config(&self, dev: DeviceId, cfg: LcdConfig) -> Result<()>;
    async fn set_lcd_image(&self, dev: DeviceId, jpeg: Vec<u8>) -> Result<()>;
    async fn clear_lcd(&self, dev: DeviceId) -> Result<()>;

    async fn get_lcd_state(&self, _dev: DeviceId) -> Result<LcdState> {
        Err(IpcError::NotImplemented("get_lcd_state".into()))
    }
    async fn patch_lcd_config(&self, _dev: DeviceId, _patch: LcdConfigPatch) -> Result<LcdState> {
        Err(IpcError::NotImplemented("patch_lcd_config".into()))
    }
    async fn set_lcd_media(&self, _dev: DeviceId, _key: String) -> Result<LcdState> {
        Err(IpcError::NotImplemented("set_lcd_media".into()))
    }
    async fn get_lcd_media(&self, _dev: DeviceId) -> Result<Option<String>> {
        Err(IpcError::NotImplemented("get_lcd_media".into()))
    }
    async fn import_prepared_clip(
        &self,
        _name: String,
        _clip: H264Clip,
    ) -> Result<crate::PreparedClipInfo> {
        Err(IpcError::NotImplemented("import_prepared_clip".into()))
    }
    async fn list_prepared_clips(&self) -> Result<Vec<crate::PreparedClipInfo>> {
        Err(IpcError::NotImplemented("list_prepared_clips".into()))
    }
    async fn delete_prepared_clip(&self, _key: String) -> Result<()> {
        Err(IpcError::NotImplemented("delete_prepared_clip".into()))
    }
    /// Load only the independently decodable first AU for a native thumbnail.
    async fn load_media_first_frame(&self, _key: String) -> Result<crate::H264AccessUnit> {
        Err(IpcError::NotImplemented("load_media_first_frame".into()))
    }
    async fn load_prepared_clip(&self, _key: String) -> Result<H264Clip> {
        Err(IpcError::NotImplemented("load_prepared_clip".into()))
    }
    async fn play_lcd_clip(
        &self,
        _dev: DeviceId,
        _clip: H264Clip,
        _loops: u16,
    ) -> Result<LcdState> {
        Err(IpcError::NotImplemented("play_lcd_clip".into()))
    }
    async fn get_lcd_playback_stats(&self, _dev: DeviceId) -> Result<LcdPlaybackStats> {
        Err(IpcError::NotImplemented("get_lcd_playback_stats".into()))
    }

    // Maintenance
    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken>;
    async fn reset_device(&self, dev: DeviceId, confirm: ResetToken) -> Result<()>;
    async fn raw_send(
        &self,
        dev: DeviceId,
        framing: FramingKind,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>>;
    async fn daemon_stats(&self) -> Result<DaemonStats>;
}
