//! CPU composition renderer. No filesystem, device access or wall clock reads.
#[path = "theme_divider.rs"]
mod divider;
use crate::text_layout::Shaper;
use ab_glyph::{Font, FontArc, FontRef, ScaleFont};
use image::{imageops::FilterType, Rgb};
pub use image::{DynamicImage, RgbImage};
use lconnect_ipc::{
    theme_sensor_text, theme_sensor_unit, ClockFormat, ImageFit, LayerBounds, LcdTheme,
    SensorParts, SensorReading, TextAlign, TextStyle, ThemeLayer,
};
use std::collections::BTreeMap;
use unicode_segmentation::UnicodeSegmentation;

/// A prepared frame can be encoded once for preview and device transmission.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Invalid composition (also validate saved themes on replay).
    #[error("{0}")]
    Theme(#[from] lconnect_ipc::IpcError),
    /// Never silently omit a missing image from an applied composition.
    #[error("image asset not loaded: {0}")]
    MissingAsset(String),
    /// A requested user font is mandatory; never silently substitute a bundled font.
    #[error("font asset not loaded: {0}")]
    MissingFont(String),
    /// Decoded assets must have nonzero dimensions.
    #[error("empty image asset: {0}")]
    EmptyAsset(String),
    /// The caller must supply a valid local time.
    #[error("invalid clock time")]
    Clock,
    /// Bundled font failed validation.
    #[error("invalid renderer font")]
    Font,
    /// Video preparation must supply the exact LCD canvas, without implicit scaling.
    #[error("composition frame must be 480x480")]
    FrameSize,
    #[error("video theme requires a decoded video frame")]
    MissingVideoFrame,
    #[error("{0}")]
    Weather(#[from] crate::effects::EffectError),
    #[error("weather motion state does not match theme settings")]
    WeatherSettings,
    #[error("{0}")]
    Meteor(#[from] crate::meteor::MeteorError),
    #[error("meteor motion state does not match theme settings")]
    MeteorSettings,
    #[error("{0}")]
    Fireworks(#[from] crate::fireworks::FireworksError),
    #[error("fireworks motion state does not match theme settings")]
    FireworksSettings,
    #[error("{0}")]
    Snow3d(#[from] crate::snow3d::Snow3dError),
}

/// Local civil time, supplied by runtime so preview and output use the same instant.
#[derive(Debug, Clone, Copy)]
pub struct ClockTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    /// 0 through 23.
    pub hour: u8,
    /// 0 through 59.
    pub minute: u8,
    /// 0 through 59.
    pub second: u8,
}

/// Immutable decoded images, prepared off the async/HID executor.
pub type ThemeAssets = BTreeMap<String, std::sync::Arc<DynamicImage>>;

/// Owned parsed fonts, loaded once during preparation and shared across frames.
pub type ThemeFonts = BTreeMap<String, FontArc>;

struct RenderFonts<'a> {
    bundled: Vec<FontArc>,
    managed: &'a ThemeFonts,
}
impl RenderFonts<'_> {
    fn get(&self, style: &TextStyle) -> Result<&FontArc, RenderError> {
        match &style.font_asset {
            Some(key) => self
                .managed
                .get(key)
                .ok_or_else(|| RenderError::MissingFont(key.clone())),
            None => Ok(&self.bundled[style.font as usize]),
        }
    }
}

/// Render in layer order with alpha blending and clipping to the square LCD canvas.
/// Asset ingestion must enforce encoded/decoded size limits before constructing assets.
pub fn render_theme(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
) -> Result<RgbImage, RenderError> {
    render_theme_elapsed(theme, readings, clock, assets, 0)
}

/// Render a cycle using monotonic elapsed seconds since this runtime was prepared.
/// Preview starts at the first binding; application/replay starts a fresh cycle.
pub fn render_theme_elapsed(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    elapsed_seconds: u64,
) -> Result<RgbImage, RenderError> {
    render_theme_with_fonts(
        theme,
        readings,
        clock,
        assets,
        &ThemeFonts::new(),
        elapsed_seconds,
    )
}

/// Render with preloaded user fonts; no filesystem work or reparsing of user fonts.
pub fn render_theme_with_fonts(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    elapsed_seconds: u64,
) -> Result<RgbImage, RenderError> {
    if theme.background_video.is_some() {
        return Err(RenderError::MissingVideoFrame);
    }
    render_theme_over_frame(
        theme,
        readings,
        clock,
        assets,
        managed,
        elapsed_seconds,
        render_background(theme),
    )
}

/// Draw the same ordered theme layers on an owned decoded video frame.
/// The video's pixels replace the theme background; layer opacity, shadows,
/// clocks and explicit sensor bindings keep their normal rendering semantics.
/// No frame clone, filesystem access, sensor collection or clock read occurs here.
pub fn render_theme_over_frame(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    elapsed_seconds: u64,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: elapsed_seconds,
            millis: elapsed_seconds.saturating_mul(1000),
            weather: None,
            meteor: None,
            fireworks: None,
        },
        canvas,
    )
}

/// Motion state supplied by the streaming owner, never by a global renderer clock.
pub struct WeatherFrame<'a> {
    pub elapsed_millis: u64,
    pub scene: &'a mut crate::effects::WeatherScene,
}
pub fn render_theme_snow3d_frame(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    millis: u64,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: millis / 1000,
            millis,
            weather: None,
            meteor: None,
            fireworks: None,
        },
        canvas,
    )
}
pub struct FireworksFrame<'a> {
    pub elapsed_millis: u64,
    pub scene: &'a mut crate::fireworks::FireworksScene,
}
pub fn render_theme_fireworks_frame(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    fireworks: FireworksFrame<'_>,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: fireworks.elapsed_millis / 1000,
            millis: fireworks.elapsed_millis,
            weather: None,
            meteor: None,
            fireworks: Some(fireworks.scene),
        },
        canvas,
    )
}
pub struct MeteorFrame<'a> {
    pub elapsed_millis: u64,
    pub scene: &'a mut crate::meteor::MeteorScene,
}
pub fn render_theme_meteor_frame(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    meteor: MeteorFrame<'_>,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: meteor.elapsed_millis / 1000,
            millis: meteor.elapsed_millis,
            weather: None,
            meteor: Some(meteor.scene),
            fireworks: None,
        },
        canvas,
    )
}
struct RenderTime<'a> {
    seconds: u64,
    millis: u64,
    weather: Option<&'a mut crate::effects::WeatherScene>,
    meteor: Option<&'a mut crate::meteor::MeteorScene>,
    fireworks: Option<&'a mut crate::fireworks::FireworksScene>,
}
/// Render animation behind ordinary ordered overlays using its owned motion history.
pub fn render_theme_weather_frame(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    weather: WeatherFrame<'_>,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: weather.elapsed_millis / 1000,
            millis: weather.elapsed_millis,
            weather: Some(weather.scene),
            meteor: None,
            fireworks: None,
        },
        canvas,
    )
}
/// A frame and its selectable foreground geometry, produced by the same layout pass.
pub struct ThemeFrame {
    pub image: RgbImage,
    /// One entry per theme layer. Backgrounds and procedural effects are fixed.
    pub layers: Vec<Option<LayerBounds>>,
}

/// Measure and render overlays on a supplied frame (including a decoded video frame).
/// Geometry uses the exact font shaping, sensor text and fitted assets in the image.
pub fn render_theme_measured(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    elapsed_seconds: u64,
    canvas: RgbImage,
) -> Result<ThemeFrame, RenderError> {
    render_layers_measured(
        theme,
        readings,
        clock,
        assets,
        managed,
        RenderTime {
            seconds: elapsed_seconds,
            millis: elapsed_seconds.saturating_mul(1000),
            weather: None,
            meteor: None,
            fireworks: None,
        },
        canvas,
    )
}

fn render_layers(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    time: RenderTime<'_>,
    canvas: RgbImage,
) -> Result<RgbImage, RenderError> {
    render_layers_measured(theme, readings, clock, assets, managed, time, canvas)
        .map(|frame| frame.image)
}

fn render_layers_measured(
    theme: &LcdTheme,
    readings: &[SensorReading],
    clock: ClockTime,
    assets: &ThemeAssets,
    managed: &ThemeFonts,
    mut time: RenderTime<'_>,
    mut canvas: RgbImage,
) -> Result<ThemeFrame, RenderError> {
    let elapsed_seconds = time.seconds;
    if canvas.dimensions() != (480, 480) {
        return Err(RenderError::FrameSize);
    }
    theme.validate()?;
    for key in theme.font_assets() {
        if !managed.contains_key(key) {
            return Err(RenderError::MissingFont(key.into()));
        }
    }
    if clock.hour > 23 || clock.minute > 59 || clock.second > 59 {
        return Err(RenderError::Clock);
    }
    let fonts = [
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")),
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSerif.ttf")),
        FontRef::try_from_slice(include_bytes!("../assets/DejaVuSansMono.ttf")),
        FontRef::try_from_slice(include_bytes!("../assets/NotoSansTC-Regular.otf")),
        FontRef::try_from_slice(include_bytes!("../assets/NotoSansTC-Bold.otf")),
        FontRef::try_from_slice(include_bytes!("../assets/AbrilFatface-Regular.ttf")),
        FontRef::try_from_slice(include_bytes!("../assets/Caprasimo-Regular.ttf")),
        FontRef::try_from_slice(include_bytes!("../assets/Cubic_11_1.013_R.ttf")),
    ]
    .into_iter()
    .map(|font| font.map(FontArc::new))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| RenderError::Font)?;
    let fonts = RenderFonts {
        bundled: fonts,
        managed,
    };
    let background_layer = theme
        .background_image
        .as_ref()
        .map(|image| ThemeLayer::Image {
            asset: image.asset.clone(),
            x: 0,
            y: 0,
            width: 480,
            height: 480,
            fit: image.fit,
            opacity: 255,
            shadow: None,
            watermark_position: None,
        });
    let mut layers = Vec::with_capacity(theme.layers.len());
    for (index, layer) in background_layer.iter().chain(&theme.layers).enumerate() {
        let mut foreground = None;
        match layer {
            ThemeLayer::Divider {
                axis,
                x,
                y,
                length,
                thickness,
                color,
                opacity,
                shadow,
            } => {
                let bounds = match axis {
                    lconnect_ipc::DividerAxis::Horizontal => [*x, *y, *length, *thickness],
                    lconnect_ipc::DividerAxis::Vertical => [*x, *y, *thickness, *length],
                };
                foreground = Some(LayerBounds {
                    x: bounds[0],
                    y: bounds[1],
                    width: bounds[2],
                    height: bounds[3],
                });
                divider::draw(&mut canvas, bounds, *color, *opacity, shadow.as_ref());
            }
            ThemeLayer::Fireworks { effect } => {
                if let Some(scene) = time.fireworks.as_deref_mut() {
                    if scene.effect() != effect {
                        return Err(RenderError::FireworksSettings);
                    }
                    scene.render(&mut canvas, readings, time.millis)?;
                } else {
                    crate::fireworks::draw_fireworks(&mut canvas, effect, readings, time.millis)?;
                }
            }
            ThemeLayer::Snow3d { effect } => {
                crate::snow3d::draw_snow3d(&mut canvas, effect, time.millis)?;
            }
            ThemeLayer::Meteor { effect } => {
                if let Some(scene) = time.meteor.as_deref_mut() {
                    if scene.effect() != effect {
                        return Err(RenderError::MeteorSettings);
                    }
                    scene.render(&mut canvas, readings, time.millis)?;
                } else {
                    crate::meteor::draw_meteor(&mut canvas, effect, readings, time.millis)?;
                }
            }
            ThemeLayer::Weather { effect } => {
                if let Some(scene) = time.weather.as_deref_mut() {
                    if scene.effect() != effect {
                        return Err(RenderError::WeatherSettings);
                    }
                    scene.render(&mut canvas, readings, time.millis)?;
                } else {
                    crate::effects::draw_weather(&mut canvas, effect, readings, time.millis)?;
                }
            }
            ThemeLayer::SensorCycle {
                parts,
                sensors,
                interval_seconds,
                style,
            } => {
                let index = (elapsed_seconds / u64::from(*interval_seconds)) % sensors.len() as u64;
                let sensor = &sensors[index as usize];
                let value = theme_sensor_text(
                    readings,
                    &sensor.sensor_id,
                    sensor.unit,
                    sensor.temperature_unit,
                    sensor.decimals,
                );
                foreground = draw_sensor(
                    &mut canvas,
                    &fonts,
                    &value,
                    &sensor.label,
                    theme_sensor_unit(sensor.unit, sensor.temperature_unit),
                    parts.as_ref(),
                    style,
                )?;
            }
            ThemeLayer::Text { text, style } => {
                foreground = draw_text(&mut canvas, fonts.get(style)?, text, style)?;
            }
            ThemeLayer::Sensor {
                parts,
                sensor_id,
                label,
                unit,
                temperature_unit,
                decimals,
                style,
            } => {
                let value =
                    theme_sensor_text(readings, sensor_id, *unit, *temperature_unit, *decimals);
                foreground = draw_sensor(
                    &mut canvas,
                    &fonts,
                    &value,
                    label,
                    theme_sensor_unit(*unit, *temperature_unit),
                    parts.as_ref(),
                    style,
                )?;
            }
            ThemeLayer::Clock {
                format,
                twenty_four_hour,
                show_seconds,
                style,
            } => {
                let hour = if *twenty_four_hour {
                    clock.hour
                } else {
                    (clock.hour + 11) % 12 + 1
                };
                let mut text = format!("{hour:02}:{:02}", clock.minute);
                if *show_seconds {
                    text.push_str(&format!(":{:02}", clock.second));
                }
                if !twenty_four_hour && *format == ClockFormat::Time {
                    text.push_str(if clock.hour < 12 { " AM" } else { " PM" });
                }
                match format {
                    ClockFormat::Date => {
                        let leap =
                            clock.year % 4 == 0 && (clock.year % 100 != 0 || clock.year % 400 == 0);
                        let days = match clock.month {
                            2 => {
                                if leap {
                                    29
                                } else {
                                    28
                                }
                            }
                            4 | 6 | 9 | 11 => 30,
                            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                            _ => 0,
                        };
                        if !(1..=9999).contains(&clock.year) || clock.day == 0 || clock.day > days {
                            return Err(RenderError::Clock);
                        }
                        text = format!("{:04}/{:02}/{:02}", clock.year, clock.month, clock.day);
                    }
                    ClockFormat::Period => {
                        text = if clock.hour < 12 { "A.M." } else { "P.M." }.into()
                    }
                    ClockFormat::Time | ClockFormat::TimeWithoutPeriod => {}
                }
                foreground = draw_text(&mut canvas, fonts.get(style)?, &text, style)?;
            }
            ThemeLayer::Image {
                asset,
                x,
                y,
                width,
                height,
                fit,
                opacity,
                shadow,
                watermark_position,
            } => {
                let source = assets
                    .get(asset)
                    .ok_or_else(|| RenderError::MissingAsset(asset.clone()))?;
                let (w, h) = (u32::from(*width), u32::from(*height));
                if source.width() == 0 || source.height() == 0 {
                    return Err(RenderError::EmptyAsset(asset.clone()));
                }
                let fitted = match fit {
                    // Crop before resizing: a panoramic source must not allocate an
                    // enormous intermediate image just to discard most of it.
                    ImageFit::Crop => {
                        let sw = u64::from(source.width());
                        let sh = u64::from(source.height());
                        let (cw, ch) = if sw * u64::from(h) > sh * u64::from(w) {
                            (
                                (sh * u64::from(w) / u64::from(h)).max(1) as u32,
                                source.height(),
                            )
                        } else {
                            (
                                source.width(),
                                (sw * u64::from(h) / u64::from(w)).max(1) as u32,
                            )
                        };
                        source
                            .crop_imm(
                                (source.width() - cw) / 2,
                                (source.height() - ch) / 2,
                                cw,
                                ch,
                            )
                            .resize_exact(w, h, FilterType::Triangle)
                    }
                    ImageFit::ContainNative => source.resize(
                        w.min(source.width()),
                        h.min(source.height()),
                        FilterType::Triangle,
                    ),
                    ImageFit::Contain => source.resize(w, h, FilterType::Triangle),
                    ImageFit::Stretch => source.resize_exact(w, h, FilterType::Triangle),
                }
                .into_rgba8();
                let (left, top) = if let Some(position) = watermark_position {
                    let center = |p: u8| {
                        if p == 0 {
                            22
                        } else {
                            (i32::from(p) * 480 + 50) / 100
                        }
                    };
                    (
                        center(position.horizontal) - fitted.width() as i32 / 2,
                        center(position.vertical) - fitted.height() as i32 / 2,
                    )
                } else {
                    (
                        i32::from(*x) + (w - fitted.width()) as i32 / 2,
                        i32::from(*y) + (h - fitted.height()) as i32 / 2,
                    )
                };
                foreground = Some(LayerBounds {
                    x: left as f32,
                    y: top as f32,
                    width: fitted.width() as f32,
                    height: fitted.height() as f32,
                });
                if let Some(shadow) = shadow {
                    if shadow.opacity > 0 && *opacity > 0 {
                        // Keep off-canvas source alpha until final blend: it can cast a visible shadow.
                        // Three sigma padding; at most 576x576 pixels.
                        let pad = u32::from(shadow.blur) * 3;
                        let mut mask = image::GrayImage::new(
                            fitted.width() + 2 * pad,
                            fitted.height() + 2 * pad,
                        );
                        for (px, py, color) in fitted.enumerate_pixels() {
                            mask.put_pixel(pad + px, pad + py, image::Luma([color[3]]));
                        }
                        let mask = if shadow.blur > 0 {
                            image::imageops::blur(&mask, f32::from(shadow.blur))
                        } else {
                            mask
                        };
                        for (px, py, alpha) in mask.enumerate_pixels() {
                            blend(
                                &mut canvas,
                                left + px as i32 - pad as i32 + i32::from(shadow.x),
                                top + py as i32 - pad as i32 + i32::from(shadow.y),
                                shadow.color,
                                f32::from(alpha[0]) / 255.0 * f32::from(*opacity) / 255.0
                                    * f32::from(shadow.opacity)
                                    / 100.0,
                            );
                        }
                    }
                }
                for (px, py, color) in fitted.enumerate_pixels() {
                    blend(
                        &mut canvas,
                        left + px as i32,
                        top + py as i32,
                        [color[0], color[1], color[2]],
                        f32::from(color[3]) * f32::from(*opacity) / 65025.0,
                    );
                }
            }
        }
        if background_layer.is_none() || index > 0 {
            layers.push(foreground);
        }
    }
    Ok(ThemeFrame {
        image: canvas,
        layers,
    })
}

fn draw_sensor(
    canvas: &mut RgbImage,
    fonts: &RenderFonts<'_>,
    value: &str,
    label: &str,
    suffix: &str,
    parts: Option<&SensorParts>,
    style: &TextStyle,
) -> Result<Option<LayerBounds>, RenderError> {
    if let Some(parts) = parts {
        let numeric = value.strip_suffix(suffix).unwrap_or(value);
        let regions = [
            draw_text(canvas, fonts.get(style)?, numeric, style)?,
            draw_text(canvas, fonts.get(&parts.label)?, label, &parts.label)?,
            draw_text(canvas, fonts.get(&parts.unit)?, suffix.trim(), &parts.unit)?,
        ];
        Ok(regions.into_iter().flatten().reduce(LayerBounds::union))
    } else {
        let text = if label.is_empty() {
            value.to_owned()
        } else {
            format!("{label} {value}")
        };
        draw_text(canvas, fonts.get(style)?, &text, style)
    }
}

fn blend(canvas: &mut RgbImage, x: i32, y: i32, color: [u8; 3], alpha: f32) {
    if alpha == 0.0 || !(0..480).contains(&x) || !(0..480).contains(&y) {
        return;
    }
    let pixel = canvas.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 {
        pixel[i] =
            (f32::from(pixel[i]) * (1.0 - alpha) + f32::from(color[i]) * alpha).round() as u8;
    }
}

fn draw_text(
    canvas: &mut RgbImage,
    font: &impl Font,
    text: &str,
    style: &TextStyle,
) -> Result<Option<LayerBounds>, RenderError> {
    let shaper = Shaper::new(font, style, text).ok_or(RenderError::Font)?;
    if let Some(shadow) = &style.shadow {
        if shadow.opacity > 0 && style.opacity > 0 {
            let mut mask = RgbImage::new(480, 480);
            let mut mask_style = style.clone();
            mask_style.color = [255; 3];
            mask_style.opacity = 100;
            draw_text_pass(&mut mask, font, &shaper, text, &mask_style);
            // White glyph coverage is identical in all RGB channels. Blur one
            // exact channel, preserving the existing raster and edge behavior.
            let mask =
                image::GrayImage::from_fn(480, 480, |x, y| image::Luma([mask.get_pixel(x, y)[0]]));
            let mask = if shadow.blur > 0 {
                image::imageops::blur(&mask, f32::from(shadow.blur))
            } else {
                mask
            };
            for (x, y, pixel) in mask.enumerate_pixels() {
                blend(
                    canvas,
                    x as i32 + i32::from(shadow.x),
                    y as i32 + i32::from(shadow.y),
                    shadow.color,
                    f32::from(pixel[0]) / 255.0 * f32::from(shadow.opacity) / 100.0
                        * f32::from(style.opacity)
                        / 100.0,
                );
            }
        }
    }
    Ok(draw_text_pass(canvas, font, &shaper, text, style))
}

// Keep source newlines and spaces; prefer word boundaries, then whole graphemes.
fn text_lines(
    shaper: &Shaper<'_, impl Font>,
    text: &str,
    style: &TextStyle,
) -> Vec<std::ops::Range<usize>> {
    let source_offset = |part: &str| part.as_ptr() as usize - text.as_ptr() as usize;
    let Some(limit) = style.max_width else {
        return text
            .lines()
            .map(|line| {
                let start = source_offset(line);
                start..start + line.len()
            })
            .collect();
    };
    if style.line_box {
        return css_text_lines(shaper, text, limit);
    }
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let offset = source_offset(paragraph);
        let mut start = 0;
        let mut last_break = None;
        for (index, grapheme) in paragraph.grapheme_indices(true) {
            let end = index + grapheme.len();
            if index > start && shaper.line(offset + start..offset + end).width > f32::from(limit) {
                let split = last_break.filter(|&b| b > start).unwrap_or(index);
                lines.push(offset + start..offset + split);
                start = split;
                last_break = None;
                // A word after the last whitespace can itself exceed the limit.
                if index > start
                    && shaper.line(offset + start..offset + end).width > f32::from(limit)
                {
                    lines.push(offset + start..offset + index);
                    start = index;
                }
            }
            if grapheme.chars().all(char::is_whitespace)
                && grapheme != "\u{a0}"
                && grapheme != "\u{202f}"
            {
                last_break = Some(end);
            }
        }
        lines.push(offset + start..offset + paragraph.len());
    }
    lines
}

// CSS pre-wrap uses normal Unicode opportunities: an unbreakable word may
// overflow. Preserved trailing spaces hang and do not force another line.
fn css_text_lines(
    shaper: &Shaper<'_, impl Font>,
    text: &str,
    limit: u16,
) -> Vec<std::ops::Range<usize>> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let offset = paragraph.as_ptr() as usize - text.as_ptr() as usize;
        let mut start = 0;
        loop {
            let mut previous = start;
            let mut wrapped = false;
            // CSS segment breaks are handled by lines() above. Chromium pre-wrap
            // does not force U+2028/NEL/standalone CR breaks merely because UAX14
            // marks them Mandatory; use these only as wrapping opportunities.
            for (end, _) in unicode_linebreak::linebreaks(paragraph) {
                if end <= start {
                    continue;
                }
                let visible = paragraph[start..end].trim_end_matches([' ', '\t']);
                if previous > start
                    && shaper
                        .line(offset + start..offset + start + visible.len())
                        .width
                        > f32::from(limit)
                {
                    lines.push(offset + start..offset + previous);
                    start = previous;
                    wrapped = true;
                    // Revisit opportunities rejected on the previous line:
                    // a hyphen that did not fit there may fit after this break.
                    break;
                }
                let candidate = offset + start..offset + end;
                if previous == start
                    || !paragraph[..end].ends_with('\u{ad}')
                    || shaper.line_break(candidate, true).width <= f32::from(limit)
                {
                    previous = end;
                }
            }
            if !wrapped {
                lines.push(offset + start..offset + paragraph.len());
                break;
            }
        }
    }
    lines
}

// pre-wrap hangs spaces after soft wraps. At explicit/end breaks they occupy
// the remaining box width and only the excess hangs outside it.
fn aligned_line(
    shaper: &Shaper<'_, impl Font>,
    text: &str,
    range: std::ops::Range<usize>,
    style: &TextStyle,
) -> crate::text_layout::Line {
    let remaining = &text[range.end..];
    let soft_wrap =
        !remaining.is_empty() && !remaining.starts_with('\n') && !remaining.starts_with("\r\n");
    let mut layout = shaper.line_break(range.clone(), style.line_box && soft_wrap);
    if !style.line_box {
        return layout;
    }
    let Some(limit) = style.max_width else {
        return layout;
    };
    let visible = text[range.clone()].trim_end_matches([' ', '\t']);
    if visible.len() == range.len() {
        return layout;
    }
    let visible_width = shaper.line(range.start..range.start + visible.len()).width;
    layout.width = if soft_wrap {
        visible_width
    } else {
        layout.width.min(f32::from(limit).max(visible_width))
    };
    layout
}

fn draw_text_pass<F: Font>(
    canvas: &mut RgbImage,
    font: &F,
    shaper: &Shaper<'_, F>,
    text: &str,
    style: &TextStyle,
) -> Option<LayerBounds> {
    let mut foreground: Option<LayerBounds> = None;
    let scale = shaper.scale();
    let scaled = font.as_scaled(scale);
    let lines = text_lines(shaper, text, style);
    let step = style
        .line_height
        .map(f32::from)
        .unwrap_or(scaled.height() + scaled.line_gap());
    let leading = if style.line_box {
        (step - scaled.height()) / 2.0
    } else {
        0.0
    };
    let height = if style.line_box {
        lines.len() as f32 * step
    } else {
        scaled.height() + lines.len().saturating_sub(1) as f32 * step
    };
    let top = f32::from(style.y)
        - match style.vertical_align {
            lconnect_ipc::VerticalAlign::Top => 0.0,
            lconnect_ipc::VerticalAlign::Center => height / 2.0,
            lconnect_ipc::VerticalAlign::Bottom => height,
        };
    for (line_index, line) in lines.into_iter().enumerate() {
        let layout = aligned_line(shaper, text, line, style);
        let x = f32::from(style.x)
            - match style.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => layout.width / 2.0,
                TextAlign::Right => layout.width,
            };
        let y = top + leading + scaled.ascent() + line_index as f32 * step;
        if y - scaled.ascent() >= 480.0 {
            break;
        }
        for placed in layout.glyphs {
            if let Some(glyph) = shaper.outline(&placed, x, y) {
                let bounds = glyph.px_bounds();
                let region = LayerBounds {
                    x: bounds.min.x,
                    y: bounds.min.y,
                    width: bounds.width(),
                    height: bounds.height(),
                };
                foreground = Some(foreground.map_or(region, |previous| previous.union(region)));
                glyph.draw(|gx, gy, coverage| {
                    blend(
                        canvas,
                        bounds.min.x as i32 + gx as i32,
                        bounds.min.y as i32 + gy as i32,
                        style.color,
                        coverage * f32::from(style.opacity) / 100.0,
                    )
                });
            }
        }
    }
    foreground
}

/// Paint the background before any overlays. Coordinates use the Windows canvas
/// extent (480), with radial distance measured to the corner, not the side.
pub fn render_background(theme: &LcdTheme) -> RgbImage {
    use lconnect_ipc::BackgroundMode;
    let Some(fill) = &theme.background_fill else {
        return RgbImage::from_pixel(480, 480, Rgb(theme.background));
    };
    let gradient = match fill.mode {
        BackgroundMode::Solid => return RgbImage::from_pixel(480, 480, Rgb(theme.background)),
        BackgroundMode::Horizontal => &fill.horizontal,
        BackgroundMode::Vertical => &fill.vertical,
        BackgroundMode::Radial => &fill.radial,
    };
    let [mut start, mut end] = gradient.colors;
    if gradient.flip {
        std::mem::swap(&mut start, &mut end);
    }
    RgbImage::from_fn(480, 480, |x, y| {
        let t = match fill.mode {
            BackgroundMode::Horizontal => f64::from(x) / 480.0,
            BackgroundMode::Vertical => f64::from(y) / 480.0,
            BackgroundMode::Radial => {
                (f64::from(x) - 240.0).hypot(f64::from(y) - 240.0) / 240.0_f64.hypot(240.0)
            }
            BackgroundMode::Solid => 0.0,
        }
        .clamp(0.0, 1.0);
        Rgb(std::array::from_fn(|i| {
            (f64::from(start[i]) * (1.0 - t) + f64::from(end[i]) * t).round() as u8
        }))
    })
}

#[cfg(test)]
mod wrapping_tests {
    use super::*;
    #[test]
    fn text_shadows_match_full_rgb_reference_on_colored_backgrounds() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        for (blur, x, y, opacity, line_box) in [
            (0, 0, 3, 100, false),
            (2, -3, 4, 65, true),
            (5, 7, -8, 100, true),
        ] {
            let mut style = test_style();
            style.x = 20;
            style.y = 10;
            style.align = TextAlign::Center;
            style.color = [32, 211, 141];
            style.opacity = opacity;
            style.line_box = line_box;
            let shadow = lconnect_ipc::TextShadow {
                x,
                y,
                blur,
                color: [17, 40, 91],
                opacity: 73,
            };
            style.shadow = Some(shadow.clone());
            let mut actual = RgbImage::from_pixel(480, 480, Rgb([91, 127, 184]));
            let mut expected = actual.clone();
            let text = "42.5°C Ж ffi";
            let mut white = style.clone();
            white.shadow = None;
            white.opacity = 100;
            white.color = [255; 3];
            let mut mask = RgbImage::new(480, 480);
            draw_text(&mut mask, &font, text, &white).unwrap();
            let mask = if blur > 0 {
                image::imageops::blur(&mask, f32::from(blur))
            } else {
                mask
            };
            for (px, py, pixel) in mask.enumerate_pixels() {
                blend(
                    &mut expected,
                    px as i32 + i32::from(x),
                    py as i32 + i32::from(y),
                    shadow.color,
                    f32::from(pixel[0]) / 255.0 * f32::from(shadow.opacity) / 100.0
                        * f32::from(opacity)
                        / 100.0,
                );
            }
            let mut foreground = style.clone();
            foreground.shadow = None;
            draw_text(&mut expected, &font, text, &foreground).unwrap();
            draw_text(&mut actual, &font, text, &style).unwrap();
            assert_eq!(
                actual, expected,
                "blur={blur}, x={x}, y={y}, opacity={opacity}"
            );
        }
    }
    fn line_width(font: &impl Font, style: &TextStyle, text: &str) -> f32 {
        Shaper::new(font, style, text)
            .unwrap()
            .line(0..text.len())
            .width
    }
    fn text_lines<'a>(font: &impl Font, text: &'a str, style: &TextStyle) -> Vec<&'a str> {
        super::text_lines(&Shaper::new(font, style, text).unwrap(), text, style)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }
    #[test]
    fn wrapping_preserves_words_spaces_and_unicode_graphemes() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSansMono.ttf")).unwrap();
        let mut style: TextStyle = test_style();
        style.max_width = Some(line_width(&font, &style, "AA ").ceil() as u16);
        assert_eq!(text_lines(&font, "AA BB", &style), ["AA ", "BB"]);
        style.max_width = Some(1);
        assert_eq!(
            text_lines(&font, "A\u{301}B\u{301}", &style),
            ["A\u{301}", "B\u{301}"]
        );
        assert_eq!(text_lines(&font, "👩‍💻X", &style), ["👩‍💻", "X"]);
        assert_eq!(text_lines(&font, "A\n\nB", &style), ["A", "", "B"]);
    }
    #[test]
    fn css_fallback_preserves_clusters_and_matches_browser_stack_advances() {
        let font =
            FontRef::try_from_slice(include_bytes!("../assets/AbrilFatface-Regular.ttf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        for (text, expected) in [
            ("Привет", 152.94921875),
            ("溫度", 80.0),
            ("CPU 溫度 42°C", 261.6397705078125),
            ("CPU سلام 42°C", 263.78826904296875),
            ("é", 20.639984130859375),
            ("e\u{301}", 20.639984130859375),
            ("Ж\u{301}", 43.0859375),
            ("אבג", 66.34765625),
            ("سلام", 82.1484375),
            ("س\u{ad}لام", 96.40625),
            ("س\u{2060}لام", 82.1484375),
        ] {
            let shaper = Shaper::new(&font, &style, text).unwrap();
            let line = shaper.line(0..text.len());
            assert!(
                (f64::from(line.width) - expected).abs() < 0.001,
                "{text}: {} vs {expected}",
                line.width
            );
            assert!(line.glyphs.iter().all(|g| g.id.0 != 0), "{text}");
        }
        let cluster = "Ж\u{301}";
        let shaper = Shaper::new(&font, &style, cluster).unwrap();
        assert!(shaper
            .line(0..cluster.len())
            .glyphs
            .iter()
            .all(|g| g.font_index == 1));
        let noto =
            FontRef::try_from_slice(include_bytes!("../assets/NotoSansTC-Regular.otf")).unwrap();
        let shaper = Shaper::new(&font, &style, "溫").unwrap();
        let line = shaper.line(0.."溫".len());
        let actual = shaper.outline(&line.glyphs[0], 0.0, 100.0).unwrap();
        let scale = f32::from(style.size) * noto.height_unscaled() / noto.units_per_em().unwrap();
        let expected = noto
            .outline_glyph(
                noto.glyph_id('溫')
                    .with_scale_and_position(scale, ab_glyph::point(0.0, 100.0)),
            )
            .unwrap();
        assert_eq!(actual.px_bounds(), expected.px_bounds());
        let mut actual_pixels = Vec::new();
        let mut expected_pixels = Vec::new();
        actual.draw(|x, y, coverage| actual_pixels.push((x, y, coverage)));
        expected.draw(|x, y, coverage| expected_pixels.push((x, y, coverage)));
        assert_eq!(actual_pixels, expected_pixels);
        style.max_width = Some(79);
        assert_eq!(text_lines(&font, "溫度", &style), ["溫", "度"]);
        style.line_box = false;
        let shaper = Shaper::new(&font, &style, "溫").unwrap();
        assert!(shaper
            .line(0.."溫".len())
            .glyphs
            .iter()
            .all(|g| g.font_index == 0 && g.id.0 == 0));
    }

    #[test]
    fn css_line_box_uses_em_size_for_cjk_advance_and_wrapping() {
        let font =
            FontRef::try_from_slice(include_bytes!("../assets/NotoSansTC-Regular.otf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        style.max_width = Some(79);
        // Each of these ideographs has a one-em advance in this pinned font.
        // Two 40px ems cannot fit into a 79px CSS box.
        assert_eq!(text_lines(&font, "溫度", &style), ["溫", "度"]);
        style.max_width = Some(80);
        assert_eq!(text_lines(&font, "溫度", &style), ["溫度"]);
        style.line_box = false;
        style.max_width = Some(79);
        assert_eq!(
            text_lines(&font, "溫度", &style),
            ["溫度"],
            "legacy metric sizing is preserved"
        );
    }

    #[test]
    fn css_shaping_matches_browser_fractional_advances_and_canonical_marks() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        let shape = |text: &str| {
            Shaper::new(&font, &style, text)
                .unwrap()
                .line(0..text.len())
        };
        // Independent Chromium canvas oracle, identical font at 40px, LTR,
        // geometricPrecision (unhinted fractional advances, not pixel raster).
        for (text, expected) in [
            ("ffi", 38.671875),
            ("office", 109.746_09),
            ("é", 24.609375),
            ("e\u{301}", 24.609375),
            ("سلام", 82.14844),
            ("(אבג)", 97.55859),
            ("CPU אבג 42°C", 271.9336),
            ("CPU سلام 42°C", 287.73438),
        ] {
            assert!((shape(text).width - expected).abs() < 0.001, "{text}");
        }
        assert_eq!(shape("ffi").glyphs.len(), 1, "OpenType ligature");
        let composed = shape("é");
        let decomposed = shape("e\u{301}");
        assert_eq!(composed.glyphs.len(), decomposed.glyphs.len());
        for (a, b) in composed.glyphs.iter().zip(&decomposed.glyphs) {
            assert_eq!((a.id, a.x, a.y), (b.id, b.x, b.y));
        }
        let hebrew = shape("(אבג)");
        assert_eq!(
            hebrew.glyphs.iter().map(|g| g.id).collect::<Vec<_>>(),
            "(גבא)"
                .chars()
                .map(|c| font.glyph_id(c))
                .collect::<Vec<_>>()
        );
        assert_eq!(shape("سلام").glyphs.len(), 3, "Arabic contextual lam-alef");
        style.max_width = Some(39);
        assert_eq!(
            text_lines(&font, "ffi", &style),
            ["ffi"],
            "wrap measures the ligature used in rendering"
        );
        style.line_box = false;
        assert_eq!(
            Shaper::new(&font, &style, "ffi")
                .unwrap()
                .line(0..3)
                .glyphs
                .len(),
            3,
            "legacy layout retained"
        );
    }

    #[test]
    fn css_canonical_mark_forms_render_identically_with_alignment() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            let mut style = test_style();
            style.line_box = true;
            style.align = align;
            let mut composed = RgbImage::new(480, 480);
            let mut decomposed = composed.clone();
            draw_text(&mut composed, &font, "é", &style).unwrap();
            draw_text(&mut decomposed, &font, "e\u{301}", &style).unwrap();
            assert_eq!(composed, decomposed);
            assert!(composed.pixels().any(|p| p.0 != [0, 0, 0]));
        }
    }

    #[test]
    fn css_soft_wrap_keeps_paragraph_bidi_override() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        style.max_width = Some(60);
        let text = "\u{202e}AB CD\u{202c}";
        let shaper = Shaper::new(&font, &style, text).unwrap();
        let lines = super::text_lines(&shaper, text, &style);
        assert_eq!(lines.len(), 2);
        assert_eq!(&text[lines[1].clone()], "CD\u{202c}");
        let second = shaper.line(lines[1].clone());
        // Default-ignorable bidi controls may occupy a zero-advance space glyph.
        let ids = second
            .glyphs
            .iter()
            .map(|g| g.id)
            .filter(|&id| id != font.glyph_id(' '))
            .collect::<Vec<_>>();
        assert_eq!(ids, ['D', 'C'].map(|c| font.glyph_id(c)));
    }

    #[test]
    fn css_wrap_matches_browser_words_nbsp_and_cjk_punctuation() {
        let font =
            FontRef::try_from_slice(include_bytes!("../assets/NotoSansTC-Regular.otf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        // Independent DOM Range oracle: same font, 40px, pre-wrap, normal breaks.
        for (text, width, expected) in [
            ("A\u{2028}B", 300, vec!["A\u{2028}B"]),
            ("A\rB", 300, vec!["A\rB"]),
            ("A\u{85}B", 300, vec!["A\u{85}B"]),
            ("溫度，正常", 80, vec!["溫", "度，", "正常"]),
            ("溫（正常）度", 120, vec!["溫（正", "常）度"]),
            ("ABCD", 40, vec!["ABCD"]),
            ("A\u{a0}B C", 60, vec!["A\u{a0}B ", "C"]),
            ("AA BB", 65, vec!["AA ", "BB"]),
            (
                "\u{202e}AB CD\u{202c}",
                65,
                vec!["\u{202e}AB ", "CD\u{202c}"],
            ),
        ] {
            style.max_width = Some(width);
            assert_eq!(text_lines(&font, text, &style), expected, "{text}");
        }
    }

    #[test]
    fn css_alignment_hangs_soft_wrap_spaces_but_preserves_explicit_spaces() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            let mut style = test_style();
            style.line_box = true;
            style.max_width = Some(100);
            style.align = align;
            let render = |text: &str| {
                let mut canvas = RgbImage::new(480, 480);
                draw_text(&mut canvas, &font, text, &style).unwrap();
                canvas
            };
            assert_eq!(render("AA BB"), render("AA\nBB"), "soft wrap {align:?}");
            if align != TextAlign::Left {
                assert_ne!(render("AA \nBB"), render("AA\nBB"), "explicit break");
                assert_ne!(render("AA "), render("AA"), "end of text");
            }
        }
        let mut style = test_style();
        style.line_box = true;
        style.max_width = Some(100);
        for text in ["AA          ", "אבג   "] {
            let shaper = Shaper::new(&font, &style, text).unwrap();
            assert!(shaper.line(0..text.len()).width > 100.0);
            assert_eq!(
                super::aligned_line(&shaper, text, 0..text.len(), &style).width,
                100.0
            );
        }
        let text = "A\u{a0}";
        let shaper = Shaper::new(&font, &style, text).unwrap();
        assert_eq!(
            super::aligned_line(&shaper, text, 0..text.len(), &style).width,
            shaper.line(0..text.len()).width
        );
    }

    #[test]
    fn css_tabs_match_browser_stops_and_do_not_draw_missing_glyphs() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        // DOM Range positions from the same font at 40px, default tab-size:8.
        // Browser geometry is quantized to 1/64px.
        for (text, last_char, expected_x) in [
            ("A\tB", 'B', 101.71875),
            ("\tA", 'A', 101.71875),
            ("A\t\tB", 'B', 203.4375),
            ("אבג\tX", 'X', 101.71875),
            ("A\tאבג", 'ג', 101.71875),
            ("AAA \tB", 'B', 203.4375),
        ] {
            let shaper = Shaper::new(&font, &style, text).unwrap();
            let layout = shaper.line(0..text.len());
            let glyph = layout
                .glyphs
                .iter()
                .find(|g| g.id == font.glyph_id(last_char))
                .unwrap();
            assert!((glyph.x - expected_x).abs() < 0.04, "{text}: {}", glyph.x);
            assert!(
                layout.glyphs.iter().all(|g| g.id.0 != 0),
                "tab is spacing, not .notdef"
            );
        }
        let text = "A\tB\nC\tD";
        let shaper = Shaper::new(&font, &style, text).unwrap();
        assert_eq!(shaper.line(0..3).glyphs[1].x, shaper.line(4..7).glyphs[1].x);
        style.max_width = Some(100);
        assert_eq!(text_lines(&font, "A\tB", &style), ["A\t", "B"]);
        let mut canvas = RgbImage::new(480, 480);
        draw_text(&mut canvas, &font, "\t", &style).unwrap();
        assert!(canvas.pixels().all(|p| p.0 == [0, 0, 0]));
    }

    #[test]
    fn css_soft_hyphen_appears_only_at_selected_discretionary_breaks() {
        let font = FontRef::try_from_slice(include_bytes!("../assets/DejaVuSans.ttf")).unwrap();
        let mut style = test_style();
        style.line_box = true;
        for (text, width, expected) in [
            ("ab\u{ad}cd", 300, vec!["ab\u{ad}cd"]),
            ("ab\u{ad}cd", 70, vec!["ab\u{ad}", "cd"]),
            ("ab\u{ad}cd", 40, vec!["ab\u{ad}", "cd"]),
            ("a b\u{ad}cdef", 80, vec!["a b\u{ad}", "cdef"]),
            ("a b\u{ad}cdef", 70, vec!["a ", "b\u{ad}", "cdef"]),
            ("a ab\u{ad}i", 100, vec!["a ab\u{ad}i"]),
        ] {
            style.max_width = Some(width);
            assert_eq!(text_lines(&font, text, &style), expected, "{text}/{width}");
        }
        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            style.align = align;
            for (text, reference, width) in [
                ("ab\u{ad}cd", "ab\u{2010}\ncd", 70),
                ("ab\u{ad}cd", "abcd", 300),
                ("ab\u{ad}\ncd", "ab\ncd", 300),
                ("ab\u{ad}", "ab", 300),
            ] {
                style.max_width = Some(width);
                let mut actual = RgbImage::new(480, 480);
                let mut expected = actual.clone();
                draw_text(&mut actual, &font, text, &style).unwrap();
                draw_text(&mut expected, &font, reference, &style).unwrap();
                assert_eq!(actual, expected, "{text}/{width}/{align:?}");
            }
        }
    }

    fn test_style() -> TextStyle {
        TextStyle {
            x: 240,
            y: 100,
            size: 40,
            color: [255; 3],
            align: TextAlign::Center,
            font: Default::default(),
            opacity: 100,
            shadow: None,
            font_asset: None,
            line_box: false,
            max_width: None,
            line_height: None,
            vertical_align: Default::default(),
        }
    }
}
