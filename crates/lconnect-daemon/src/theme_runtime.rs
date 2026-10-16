//! Prepared composition and current sensor snapshots for the sole LCD owner.
use crate::sensors::SensorCache;
use chrono::{Datelike, Timelike};
use lconnect_image::{
    prepare_jpeg_from_image,
    theme::{
        render_theme_over_frame, render_theme_with_fonts, ClockTime, RgbImage, ThemeAssets,
        ThemeFonts,
    },
    FitMode, Quality,
};
use lconnect_ipc::{IpcError, LcdTheme, Result};
use std::sync::{
    atomic::{AtomicU16, Ordering},
    Arc,
};

/// Validated first frame paired with the exact runtime that rendered it.
/// Construct with [`ThemeRuntime::prepare`] before enqueueing a display mutation.
#[derive(Debug)]
pub struct PreparedTheme {
    pub(crate) runtime: Arc<ThemeRuntime>,
    pub(crate) jpeg: bytes::Bytes,
}

/// Immutable assets shared across frames; sensor caches remain live.
pub struct ThemeRuntime {
    output_rotation: AtomicU16,
    theme: LcdTheme,
    temperature_variants: [LcdTheme; 2],
    display_unit: Arc<std::sync::atomic::AtomicBool>,
    assets: ThemeAssets,
    media_pin: Option<crate::media_assets::MediaPin>,
    fonts: ThemeFonts,
    sensors: Vec<SensorCache>,
    started: std::time::Instant,
    weather: Option<std::sync::Mutex<lconnect_image::effects::WeatherScene>>,
    meteor: Option<std::sync::Mutex<lconnect_image::meteor::MeteorScene>>,
    fireworks: Option<std::sync::Mutex<lconnect_image::fireworks::FireworksScene>>,
}
impl std::fmt::Debug for ThemeRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeRuntime")
            .field("theme", &self.theme)
            .field("output_rotation", &self.output_rotation)
            .finish_non_exhaustive()
    }
}
impl ThemeRuntime {
    pub fn with_display_unit(mut self, unit: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.display_unit = unit;
        self
    }
    /// Call after loading bounded assets. No device I/O occurs here.
    pub fn new(theme: LcdTheme, assets: ThemeAssets, sensors: Vec<SensorCache>) -> Result<Self> {
        Self::with_fonts(theme, assets, ThemeFonts::new(), sensors)
    }
    /// Own prepared fonts for the lifetime of this runtime, including after GUI exit.
    pub fn with_fonts(
        theme: LcdTheme,
        assets: ThemeAssets,
        fonts: ThemeFonts,
        sensors: Vec<SensorCache>,
    ) -> Result<Self> {
        theme.validate()?;
        for key in theme.font_assets() {
            if !fonts.contains_key(key) {
                return Err(IpcError::InvalidArgument(format!(
                    "font asset not loaded: {key}"
                )));
            }
        }
        let weather = theme
            .weather_effect()
            .cloned()
            .map(lconnect_image::effects::WeatherScene::new)
            .transpose()
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?
            .map(std::sync::Mutex::new);
        let meteor = theme
            .meteor_effect()
            .cloned()
            .map(lconnect_image::meteor::MeteorScene::new)
            .transpose()
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?
            .map(std::sync::Mutex::new);
        let fireworks = theme
            .fireworks_effect()
            .cloned()
            .map(lconnect_image::fireworks::FireworksScene::new)
            .transpose()
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?
            .map(std::sync::Mutex::new);
        Ok(Self {
            output_rotation: Default::default(),
            fireworks,
            meteor,
            weather,
            temperature_variants: [
                theme.resolve_temperature_units(lconnect_ipc::DisplayTemperatureUnit::Celsius),
                theme.resolve_temperature_units(lconnect_ipc::DisplayTemperatureUnit::Fahrenheit),
            ],
            display_unit: Arc::default(),
            theme,
            assets,
            media_pin: None,
            fonts,
            sensors,
            started: std::time::Instant::now(),
        })
    }
    pub(crate) fn with_media_pin(mut self, pin: Option<crate::media_assets::MediaPin>) -> Self {
        self.media_pin = pin;
        self
    }
    pub(crate) fn media_pin(&self) -> Option<crate::media_assets::MediaPin> {
        self.media_pin.clone()
    }
    /// Retain decoded image leases while accepted settings await persistence.
    pub(crate) fn image_leases(&self) -> ThemeAssets {
        self.assets.clone()
    }

    /// Requested composition, suitable for state/persistence at the caller boundary.
    pub fn theme(&self) -> &LcdTheme {
        &self.theme
    }

    /// Render and validate the first frame without holding the display actor.
    pub async fn prepare(self: &Arc<Self>) -> Result<PreparedTheme> {
        let jpeg = self.frame().await?;
        lconnect_protocol::galahad2_lcdv2::jpeg::build_send_jpeg_request(&jpeg)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?;
        Ok(PreparedTheme {
            runtime: self.clone(),
            jpeg,
        })
    }

    /// One joined blocking job per frame: never perform font/image work on Tokio.
    /// H264 path rotates the final composition, including live layers, before encoding.
    pub fn with_output_rotation(mut self, rotation: lconnect_image::FrameRotation) -> Self {
        self.output_rotation = AtomicU16::new(rotation.degrees());
        self
    }
    pub fn output_rotation(&self) -> lconnect_image::FrameRotation {
        lconnect_image::FrameRotation::try_from(self.output_rotation.load(Ordering::Acquire))
            .expect("private rotation accepts only typed values")
    }
    pub(crate) fn set_output_rotation(&self, rotation: lconnect_image::FrameRotation) {
        self.output_rotation
            .store(rotation.degrees(), Ordering::Release);
    }

    pub async fn frame(self: &Arc<Self>) -> Result<bytes::Bytes> {
        self.frame_at(self.started.elapsed().as_secs()).await
    }

    /// Preview supplies its own monotonic cycle position without changing device state.
    pub async fn frame_at(self: &Arc<Self>, elapsed_seconds: u64) -> Result<bytes::Bytes> {
        let runtime = self.clone();
        tokio::task::spawn_blocking(move || {
            let frame = runtime.render(None, elapsed_seconds)?;
            prepare_jpeg_from_image(frame.into(), FitMode::Stretch, Quality::default())
                .map(|jpeg| bytes::Bytes::from(jpeg.bytes))
                .map_err(|e| IpcError::InvalidArgument(e.to_string()))
        })
        .await
        .map_err(|e| IpcError::DeviceBusy(format!("theme renderer: {e}")))?
    }

    /// Compose live layers over one decoded frame on the caller's blocking worker.
    /// Sensor snapshots are cache-only; no hardware/provider work runs per frame.
    /// Elapsed time belongs to the composition session and need not reset on video loop.
    pub fn compose_video_frame(&self, frame: RgbImage, elapsed_seconds: u64) -> Result<RgbImage> {
        self.render(Some(frame), elapsed_seconds)
    }

    /// H264 producer supplies its frame timeline; sensor reads remain cache-only.
    pub fn compose_frame_at_millis(
        &self,
        frame: Option<RgbImage>,
        elapsed_millis: u64,
    ) -> Result<RgbImage> {
        self.render_inner(frame, elapsed_millis / 1000, elapsed_millis, true)
            .map(|frame| self.output_rotation().apply(frame))
    }
    fn render(&self, frame: Option<RgbImage>, elapsed_seconds: u64) -> Result<RgbImage> {
        self.render_inner(
            frame,
            elapsed_seconds,
            elapsed_seconds.saturating_mul(1000),
            false,
        )
        .map(|frame| self.output_rotation().apply(frame))
    }
    fn render_inner(
        &self,
        frame: Option<RgbImage>,
        elapsed_seconds: u64,
        elapsed_millis: u64,
        continuous: bool,
    ) -> Result<RgbImage> {
        let theme =
            &self.temperature_variants[usize::from(self.display_unit.load(Ordering::Acquire))];
        let now = chrono::Local::now();
        let readings = self
            .sensors
            .iter()
            .flat_map(SensorCache::readings)
            .collect::<Vec<_>>();
        let clock = ClockTime {
            year: now.year(),
            month: now.month() as u8,
            day: now.day() as u8,
            hour: now.hour() as u8,
            minute: now.minute() as u8,
            second: now.second() as u8,
        };
        if continuous {
            if let Some(fireworks) = &self.fireworks {
                let mut scene = fireworks
                    .lock()
                    .map_err(|_| IpcError::DeviceBusy("fireworks scene poisoned".into()))?;
                return lconnect_image::theme::render_theme_fireworks_frame(
                    theme,
                    &readings,
                    clock,
                    &self.assets,
                    &self.fonts,
                    lconnect_image::theme::FireworksFrame {
                        elapsed_millis,
                        scene: &mut scene,
                    },
                    frame.unwrap_or_else(|| lconnect_image::theme::render_background(theme)),
                )
                .map_err(|e| IpcError::InvalidArgument(e.to_string()));
            }
            if self.theme.snow3d_effect().is_some() {
                return lconnect_image::theme::render_theme_snow3d_frame(
                    theme,
                    &readings,
                    clock,
                    &self.assets,
                    &self.fonts,
                    elapsed_millis,
                    frame.unwrap_or_else(|| lconnect_image::theme::render_background(theme)),
                )
                .map_err(|e| IpcError::InvalidArgument(e.to_string()));
            }
            if let Some(meteor) = &self.meteor {
                let mut scene = meteor
                    .lock()
                    .map_err(|_| IpcError::DeviceBusy("meteor scene poisoned".into()))?;
                return lconnect_image::theme::render_theme_meteor_frame(
                    theme,
                    &readings,
                    clock,
                    &self.assets,
                    &self.fonts,
                    lconnect_image::theme::MeteorFrame {
                        elapsed_millis,
                        scene: &mut scene,
                    },
                    frame.unwrap_or_else(|| lconnect_image::theme::render_background(theme)),
                )
                .map_err(|e| IpcError::InvalidArgument(e.to_string()));
            }
            if let Some(weather) = &self.weather {
                let mut scene = weather
                    .lock()
                    .map_err(|_| IpcError::DeviceBusy("weather scene poisoned".into()))?;
                return lconnect_image::theme::render_theme_weather_frame(
                    theme,
                    &readings,
                    clock,
                    &self.assets,
                    &self.fonts,
                    lconnect_image::theme::WeatherFrame {
                        elapsed_millis,
                        scene: &mut scene,
                    },
                    frame.unwrap_or_else(|| lconnect_image::theme::render_background(theme)),
                )
                .map_err(|e| IpcError::InvalidArgument(e.to_string()));
            }
        }
        match frame {
            Some(frame) => render_theme_over_frame(
                theme,
                &readings,
                clock,
                &self.assets,
                &self.fonts,
                elapsed_seconds,
                frame,
            ),
            None => render_theme_with_fonts(
                theme,
                &readings,
                clock,
                &self.assets,
                &self.fonts,
                elapsed_seconds,
            ),
        }
        .map_err(|e| IpcError::InvalidArgument(e.to_string()))
    }
}
