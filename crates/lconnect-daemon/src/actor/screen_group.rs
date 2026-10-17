//! Device-free preparation for the LCD owner's full-screen group transaction.
use crate::{
    persistence::{DeviceSettings, PersistenceStore},
    profile_archive::PreparedGroup,
    profile_group::{GroupImportCommit, GroupProfile, GroupSettings, ScreenGroup},
    sensors::SensorCache,
};
use lconnect_ipc::{IpcError, LcdConfig, LcdConfigPatch, LcdState, Result};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ScreenGroupState {
    pub revision: super::group_barrier::RuntimeRevision,
    pub lcd: LcdState,
    pub recovery_required: bool,
}
#[derive(Debug)]
pub struct ScreenReapplyAdmission {
    pub readback: crate::profile_group::GroupImportReadback,
    pub revision: super::group_barrier::RuntimeRevision,
    pub generation: u64,
    pub preparation: u64,
}
#[derive(Debug)]
pub struct ScreenReapplyResult {
    pub readback: crate::profile_group::GroupImportReadback,
    pub runtime: ScreenGroupState,
}

pub(crate) enum PreparedSource {
    Idle,
    Theme(crate::theme_runtime::PreparedTheme),
    Media {
        key: String,
        source: crate::media_assets::PinnedMedia,
    },
    Composition {
        prepared: crate::media_composition::PreparedComposition,
        media: Option<String>,
    },
}
/// First frame/assets prepared before publication; live sensor caches remain live.
pub struct PreparedScreen {
    pub(crate) config: LcdConfig,
    pub(crate) source: PreparedSource,
    theme: Option<lconnect_ipc::LcdTheme>,
    media: Option<String>,
}
impl std::fmt::Debug for PreparedScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedScreen")
            .field("config", &self.config)
            .field("streaming", &self.streaming())
            .finish_non_exhaustive()
    }
}
impl PreparedScreen {
    /// Used for first import and explicit reapply from current durable readback.
    pub async fn prepare(
        settings: &DeviceSettings,
        stores: Arc<PersistenceStore>,
        caches: Vec<SensorCache>,
    ) -> Result<Self> {
        Self::prepare_exempt(settings, stores, caches, Default::default()).await
    }
    async fn prepare_exempt(
        settings: &DeviceSettings,
        stores: Arc<PersistenceStore>,
        caches: Vec<SensorCache>,
        font_exemptions: std::collections::BTreeSet<String>,
    ) -> Result<Self> {
        let group = ScreenGroup {
            lcd: settings.lcd.clone(),
            theme: settings.theme.clone(),
            editor: settings.screen_editor.clone(),
            media_clip: settings.media_clip.clone(),
            collections: settings.screen_collections.clone(),
        };
        GroupProfile {
            version: 1,
            content: GroupSettings::Screen(Box::new(group)),
            profiles: Vec::new(),
        }
        .validate()
        .map_err(invalid)?;
        let config = screen_config(settings)?;
        let streaming = settings.media_clip.is_some()
            || settings
                .theme
                .as_ref()
                .is_some_and(|theme| theme.requires_streaming());
        let rotation = lconnect_image::FrameRotation::try_from(if streaming {
            config.rotation_degrees
        } else {
            0
        })
        .map_err(invalid)?;
        let source = if let Some(theme) = settings.theme.clone() {
            let media = stores.media_assets();
            let runtime = tokio::task::spawn_blocking(move || {
                let images = stores.theme_assets().load(&theme).map_err(invalid)?;
                let fonts = stores
                    .theme_fonts()
                    .load_theme_exempt(&theme, &font_exemptions)
                    .map_err(invalid)?;
                let pin = stores.media_assets().pin_theme(&theme).map_err(invalid)?;
                Ok::<_, IpcError>(Arc::new(
                    crate::theme_runtime::ThemeRuntime::with_fonts(theme, images, fonts, caches)?
                        .with_display_unit(stores.display_settings().live_unit())
                        .with_media_pin(pin)
                        .with_output_rotation(rotation),
                ))
            })
            .await
            .map_err(invalid)??;
            if streaming {
                PreparedSource::Composition {
                    prepared: crate::media_composition::PreparedComposition::prepare(
                        media, runtime,
                    )
                    .await
                    .map_err(invalid)?,
                    media: None,
                }
            } else {
                PreparedSource::Theme(runtime.prepare().await?)
            }
        } else if let Some(key) = settings.media_clip.clone() {
            if rotation != lconnect_image::FrameRotation::None {
                let runtime = Arc::new(
                    crate::theme_runtime::ThemeRuntime::new(
                        media_theme(key.clone()),
                        Default::default(),
                        vec![],
                    )?
                    .with_output_rotation(rotation),
                );
                let prepared = crate::media_composition::PreparedComposition::prepare(
                    stores.media_assets(),
                    runtime,
                )
                .await
                .map_err(invalid)?;
                return Ok(Self {
                    config,
                    source: PreparedSource::Composition {
                        prepared,
                        media: Some(key),
                    },
                    theme: None,
                    media: settings.media_clip.clone(),
                });
            }
            let stored = key.clone();
            let source = tokio::task::spawn_blocking(move || {
                let mut source = stores.media_assets().acquire_source(&stored)?;
                source.read_unit(0)?;
                Ok::<_, anyhow::Error>(source)
            })
            .await
            .map_err(invalid)?
            .map_err(invalid)?;
            PreparedSource::Media { key, source }
        } else {
            PreparedSource::Idle
        };
        Ok(Self {
            config,
            source,
            theme: settings.theme.clone(),
            media: settings.media_clip.clone(),
        })
    }
    pub(crate) fn streaming(&self) -> bool {
        matches!(
            self.source,
            PreparedSource::Media { .. } | PreparedSource::Composition { .. }
        )
    }
    pub(crate) fn matches(&self, settings: &DeviceSettings) -> bool {
        self.theme == settings.theme
            && self.media == settings.media_clip
            && screen_config(settings).is_ok_and(|config| config == self.config)
    }
    pub(crate) async fn stop(&mut self) -> Result<()> {
        if let PreparedSource::Composition { prepared, .. } = &mut self.source {
            prepared.worker.stop().await.map_err(invalid)?;
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct PreparedScreenGroup {
    pub(crate) archive: Arc<PreparedGroup>,
    pub(crate) screen: PreparedScreen,
}
impl PreparedScreenGroup {
    pub async fn prepare(
        archive: Arc<PreparedGroup>,
        stores: Arc<PersistenceStore>,
        caches: Vec<SensorCache>,
    ) -> Result<Self> {
        let GroupSettings::Screen(group) = &archive.profile().content else {
            return Err(invalid("LCD import requires a screen group"));
        };
        let settings = DeviceSettings {
            lcd: group.lcd.clone(),
            theme: group.theme.clone(),
            media_clip: group.media_clip.clone(),
            screen_collections: group.collections.clone(),
            screen_editor: group.editor.clone(),
            ..Default::default()
        };
        let screen =
            PreparedScreen::prepare_exempt(&settings, stores, caches, archive.font_exemptions())
                .await?;
        Ok(Self { archive, screen })
    }
    pub(crate) async fn stop(&mut self) -> Result<()> {
        self.screen.stop().await
    }
}
#[derive(Debug)]
pub struct ScreenGroupResult {
    pub runtime: ScreenGroupState,
    /// Committed receipt is independent of a possible lcd.last_error.
    pub commit: GroupImportCommit,
    pub lcd: LcdState,
}
pub(crate) fn screen_config(settings: &DeviceSettings) -> Result<LcdConfig> {
    let lcd = &settings.lcd;
    let empty = lcd.control_mode.is_empty() && lcd.rotation.is_empty() && lcd.brightness == 0;
    let config = LcdConfig::new(
        if lcd.control_mode.is_empty() {
            "LocalUI"
        } else {
            &lcd.control_mode
        },
        if empty { 100 } else { lcd.brightness },
        if lcd.rotation.is_empty() {
            0
        } else {
            lcd.rotation.parse().map_err(invalid)?
        },
    );
    super::lcd_state::merge_lcd_config(
        &config,
        &LcdConfigPatch {
            mode: None,
            brightness_percent: None,
            rotation_degrees: None,
        },
    )
}
fn invalid(error: impl std::fmt::Display) -> IpcError {
    IpcError::InvalidArgument(error.to_string())
}

/// Runtime-only wrapper; selecting a media clip still persists a media identity.
pub(crate) fn media_theme(key: String) -> lconnect_ipc::LcdTheme {
    lconnect_ipc::LcdTheme {
        version: 2,
        name: "Managed media".into(),
        background: [0, 0, 0],
        background_video: Some(key),
        background_image: None,
        background_fill: None,
        layers: Vec::new(),
        page_cache: Vec::new(),
        basic_editor: None,
    }
}
