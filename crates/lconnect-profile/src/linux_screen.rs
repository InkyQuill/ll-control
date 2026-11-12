//! Explicit, reviewable basic runtime-theme proposal. Source editor banks are separate.
//! Resource keys and fonts are provided by the caller; no filesystem paths are opened.
use crate::{strict_json, Error};
use lconnect_ipc::{
    BackgroundFill, BackgroundImage, BackgroundMode, ClockFormat, GradientColors, ImageFit,
    LcdTheme, SensorParts, SensorUnit, TextAlign, TextShadow, TextStyle, ThemeFont, ThemeLayer,
    ThemeSensor, VerticalAlign, WatermarkPosition,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontBinding {
    pub font: ThemeFont,
    pub font_asset: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceBinding {
    Image { asset: String },
    Video { key: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    pub name: String,
    /// Windows role -> explicitly chosen Linux identity, label, unit and precision.
    pub sensors: BTreeMap<String, SensorBinding>,
    /// Windows font family -> explicit bundled/custom Linux font. No name guessing.
    pub fonts: BTreeMap<String, FontBinding>,
    /// Exact source path/URI -> explicit managed, provisional or known builtin resource key.
    pub resources: BTreeMap<String, ResourceBinding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorBinding {
    pub sensor_id: String,
    pub label: String,
    pub unit: SensorUnit,
    pub temperature_unit: lconnect_ipc::TemperatureUnit,
    pub decimals: u8,
}
impl From<&SensorBinding> for ThemeSensor {
    fn from(value: &SensorBinding) -> Self {
        Self {
            sensor_id: value.sensor_id.clone(),
            label: value.label.clone(),
            unit: value.unit,
            temperature_unit: value.temperature_unit,
            decimals: value.decimals,
        }
    }
}
pub(crate) const FONTS: &[&str] = &[
    "AbrilFatface",
    "AgencyGothicCT_Bold",
    "BakerieSmooth_WideMediumRegular",
    "Caprasimo_Regular",
    "GoodTimesRg_Italic",
    "HyperspaceRace_BoldItalic",
    "LTCFournierLeJeune_Regular",
    "ManoloMono_Regular",
    "NotoSansTC_Regular",
    "NotoSansTC_Bold",
    "Cubic11",
];
#[derive(Debug, Serialize)]
pub struct PreparedRuntime {
    pub source_sha256: String,
    pub theme: LcdTheme,
    pub used_sensor_roles: Vec<String>,
    pub used_fonts: Vec<String>,
    pub used_resources: Vec<String>,
    pub layout_review_required: bool,
    pub review: Vec<&'static str>,
    pub pixel_parity_verified: bool,
    pub resource_payloads_verified: bool,
    pub sensor_binding_verified: bool,
    pub editor_banks_converted: bool,
    pub apply_supported: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Runtime {
    style: String,
    screens: Vec<Screen>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Screen {
    theme: Background,
    text: Overlay,
    watermark: Watermark,
}
#[derive(Deserialize)]
#[serde(tag = "Type", deny_unknown_fields)]
enum Background {
    Image {
        #[serde(rename = "Path")]
        path: String,
        #[serde(rename = "BackgroundColor")]
        color: [u8; 3],
    },
    Color {
        #[serde(rename = "ColorType")]
        color_type: String,
        #[serde(rename = "Colors")]
        colors: Vec<[u8; 3]>,
        #[serde(rename = "Flip")]
        flip: bool,
    },
}
#[derive(Deserialize)]
#[serde(tag = "Type", deny_unknown_fields)]
enum Overlay {
    None,
    Custom {
        #[serde(rename = "Text")]
        text: String,
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
    Time {
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
    SliderSensor {
        #[serde(rename = "Sensors")]
        sensors: Vec<String>,
        #[serde(rename = "LoopSecond")]
        seconds: u16,
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
    DuoSensor {
        #[serde(rename = "Sensors")]
        sensors: Vec<String>,
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
    ThreeSensor {
        #[serde(rename = "Sensors")]
        sensors: Vec<String>,
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
    FourSensor {
        #[serde(rename = "Sensors")]
        sensors: Vec<String>,
        #[serde(rename = "Layout")]
        layout: Vec<Layout>,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Layout {
    font_colors: Vec<[u8; 3]>,
    font_size: u16,
    font_family: String,
    horizontal_alignment: String,
    vertical_alignment: String,
    text_alignment: String,
    shadow: u8,
    opacity: u8,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Watermark {
    has_image: bool,
    image: String,
    horizontal: u8,
    vertical: u8,
    shadow: u8,
    opacity: u8,
}

/// Strictly parse the explicit mapping document before any connection or mutation.
pub fn parse_bindings(bytes: &[u8]) -> Result<Bindings, Error> {
    if bytes.len() > 64 * 1024 {
        return Err(Error::Invalid("Screen bindings exceed64KiB"));
    }
    let bindings: Bindings = serde_json::from_value(strict_json::parse(bytes)?)?;
    validate_bindings(&bindings)?;
    Ok(bindings)
}
fn role_unit(role: &str) -> Option<SensorUnit> {
    match role {
        "CPULoad" | "GPULoad" => Some(SensorUnit::Percent),
        "CPUTemperature" | "GPUTemperature" | "CoolantTemp" => Some(SensorUnit::Celsius),
        "PumpRPM" => Some(SensorUnit::Rpm),
        _ => None,
    }
}
pub(crate) fn key_valid(key: &str) -> bool {
    key.len() == 32
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_bindings(bindings: &Bindings) -> Result<(), Error> {
    if bindings.name.trim().is_empty() || bindings.name.len() > 128 || bindings.name.contains('\0')
    {
        return Err(Error::Invalid(
            "explicit Screen theme name required within128 bytes",
        ));
    }
    if bindings.sensors.len() > 6 || bindings.fonts.len() > 32 || bindings.resources.len() > 256 {
        return Err(Error::Invalid("too many Screen source bindings"));
    }
    let mut ids = BTreeSet::new();
    for (role, sensor) in &bindings.sensors {
        if role_unit(role) != Some(sensor.unit)
            || sensor.sensor_id.trim().is_empty()
            || sensor.sensor_id.len() > 1024
            || sensor.sensor_id.chars().any(char::is_control)
            || sensor.label.len() > 128
            || sensor.label.contains('\0')
            || sensor.decimals > 2
            || !ids.insert(&sensor.sensor_id)
        {
            return Err(Error::Invalid(
                "invalid or duplicate Screen sensor role/identity/unit",
            ));
        }
    }
    if bindings.fonts.iter().any(|(name, f)| {
        !FONTS.contains(&name.as_str())
            || name.chars().any(char::is_control)
            || f.font_asset.as_ref().is_some_and(|k| !key_valid(k))
    }) {
        return Err(Error::Invalid("invalid Screen font mapping"));
    }
    if bindings.resources.iter().any(|(path, r)| {
        path.is_empty()
            || path.len() > 32768
            || path.chars().any(char::is_control)
            || match r {
                ResourceBinding::Image { asset } => {
                    !key_valid(asset) && lconnect_image::defaults::image(asset).is_none()
                }
                ResourceBinding::Video { key } => !key_valid(key),
            }
    }) {
        return Err(Error::Invalid("invalid Screen managed resource mapping"));
    }
    Ok(())
}
struct Context<'a> {
    bindings: Option<&'a Bindings>,
    sensors: BTreeSet<String>,
    fonts: BTreeSet<String>,
    resources: BTreeSet<String>,
    image_resources: BTreeSet<String>,
}
impl Context<'_> {
    fn font(&mut self, name: &str) -> Result<FontBinding, Error> {
        self.fonts.insert(name.to_owned());
        match self.bindings {
            Some(bindings) => bindings
                .fonts
                .get(name)
                .cloned()
                .ok_or(Error::Invalid("explicit Screen font mapping is required")),
            None => Ok(FontBinding {
                font: ThemeFont::Sans,
                font_asset: None,
            }),
        }
    }
    fn sensor(&mut self, role: &str) -> Result<ThemeSensor, Error> {
        self.sensors.insert(role.to_owned());
        match self.bindings {
            Some(bindings) => {
                bindings
                    .sensors
                    .get(role)
                    .map(ThemeSensor::from)
                    .ok_or(Error::Invalid(
                        "explicit Screen sensor role mapping is required",
                    ))
            }
            None => Ok(ThemeSensor {
                sensor_id: format!("inspection:{role}"),
                label: role.into(),
                unit: role_unit(role).ok_or(Error::Invalid("unknown Screen sensor role"))?,
                temperature_unit: lconnect_ipc::TemperatureUnit::System,
                decimals: 1,
            }),
        }
    }
    fn resource(&mut self, path: &str, image_only: bool) -> Result<ResourceBinding, Error> {
        if path.is_empty() || path.len() > 32768 || path.chars().any(char::is_control) {
            return Err(Error::Invalid("invalid Screen source resource reference"));
        }
        self.resources.insert(path.to_owned());
        if image_only {
            self.image_resources.insert(path.to_owned());
        }
        if self.resources.len() > 256 {
            return Err(Error::Invalid("too many Screen source references"));
        }
        match self.bindings {
            Some(bindings) => bindings.resources.get(path).cloned().ok_or(Error::Invalid(
                "exact Screen resource path/URI mapping is required",
            )),
            None => Ok(ResourceBinding::Image {
                asset: "0".repeat(32),
            }),
        }
    }
}
/// Names/types required by validated source semantics, never executable bindings.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeRequirements {
    pub sensor_roles: BTreeSet<String>,
    pub fonts: BTreeSet<String>,
    /// True means a watermark/image-gallery use requires an image, not a video.
    pub resources: BTreeMap<String, bool>,
}
impl RuntimeRequirements {
    pub(crate) fn merge(&mut self, other: Self) {
        self.sensor_roles.extend(other.sensor_roles);
        self.fonts.extend(other.fonts);
        for (reference, image_only) in other.resources {
            *self.resources.entry(reference).or_default() |= image_only;
        }
    }
}
/// Inspect with the same bounded parser/layout validation as actual conversion.
/// Private placeholder values are discarded and cannot leave this API as a theme.
pub fn inspect_runtime_requirements(source: &str) -> Result<RuntimeRequirements, Error> {
    Ok(inspect_runtime_proposal(source)?.1)
}
pub(crate) fn inspect_runtime_proposal(
    source: &str,
) -> Result<(PreparedRuntime, RuntimeRequirements), Error> {
    prepare_inner(source, None)
}

/// Convert one Full basic runtime config into a native proposal. The result must
/// be reviewed: CSS/font geometry, decorative dividers and Windows raster remain
/// separate qualification, and no group/editor bank commit is provided here.
pub fn prepare_runtime(source: &str, bindings: &Bindings) -> Result<PreparedRuntime, Error> {
    Ok(prepare_inner(source, Some(bindings))?.0)
}
fn prepare_inner(
    source: &str,
    bindings: Option<&Bindings>,
) -> Result<(PreparedRuntime, RuntimeRequirements), Error> {
    if let Some(bindings) = bindings {
        validate_bindings(bindings)?;
    }
    if source.is_empty() || source.len() > 512 * 1024 {
        return Err(Error::Invalid(
            "explicit nonempty Screen runtime JSON required within512KiB",
        ));
    }
    let runtime: Runtime = serde_json::from_value(strict_json::parse(source.as_bytes())?)?;
    if runtime.style != "Full" || runtime.screens.len() != 1 {
        return Err(Error::Invalid(
            "only one Full LCDV2 runtime screen is supported",
        ));
    }
    let mut context = Context {
        bindings,
        sensors: BTreeSet::new(),
        fonts: BTreeSet::new(),
        resources: BTreeSet::new(),
        image_resources: BTreeSet::new(),
    };
    let mut theme = LcdTheme {
        version: 1,
        name: bindings.map_or_else(|| "Inspection only".into(), |b| b.name.clone()),
        background: [0; 3],
        background_video: None,
        background_image: None,
        background_fill: None,
        layers: Vec::new(),
        page_cache: Vec::new(),
        basic_editor: None,
    };
    let mut review=vec!["Native proposal requires layout/font review; Windows CSS metrics and raster are not qualified.","Only runtime config is converted; all source editor banks must be preserved separately before group import."];
    let screen = runtime
        .screens
        .into_iter()
        .next()
        .ok_or(Error::Invalid("missing runtime screen"))?;
    match screen.theme {
        Background::Image { path, color } => {
            theme.background = color;
            if !path.is_empty() {
                match context.resource(&path, false)? {
                    ResourceBinding::Image { asset } => {
                        theme.background_image = Some(BackgroundImage {
                            asset,
                            fit: ImageFit::Stretch,
                        })
                    }
                    ResourceBinding::Video { key } => {
                        theme.version = 2;
                        theme.background_video = Some(key);
                    }
                }
            }
        }
        Background::Color {
            color_type,
            colors,
            flip,
        } => {
            let mode = match color_type.as_str() {
                "SolidColor" => BackgroundMode::Solid,
                "HorizontalGradient" => BackgroundMode::Horizontal,
                "VerticalGradient" => BackgroundMode::Vertical,
                "RadialGradient" => BackgroundMode::Radial,
                _ => return Err(Error::Invalid("unknown basic Screen background type")),
            };
            let count = if mode == BackgroundMode::Solid { 1 } else { 2 };
            if colors.len() != count {
                return Err(Error::Invalid("invalid basic Screen background palette"));
            }
            theme.background = colors[0];
            let pair = if count == 2 {
                [colors[0], colors[1]]
            } else {
                [colors[0]; 2]
            };
            let gradient = GradientColors { colors: pair, flip };
            theme.background_fill = Some(BackgroundFill {
                mode,
                horizontal: gradient.clone(),
                vertical: gradient.clone(),
                radial: gradient,
            });
            review.push("Runtime contains only the selected background mode; inactive gradient edits remain in source editor settings.");
        }
    }
    overlay(screen.text, &mut theme, &mut context, &mut review)?;
    let mark = screen.watermark;
    if mark.has_image == mark.image.is_empty()
        || mark.horizontal > 100
        || mark.vertical > 100
        || mark.shadow > 100
        || mark.opacity > 100
    {
        return Err(Error::Invalid("invalid Screen watermark fields"));
    }
    if mark.has_image {
        let ResourceBinding::Image { asset } = context.resource(&mark.image, true)? else {
            return Err(Error::Invalid(
                "Screen watermark requires an image resource",
            ));
        };
        theme.layers.push(ThemeLayer::Image {
            asset,
            x: 0,
            y: 0,
            width: 480,
            height: 480,
            fit: ImageFit::ContainNative,
            opacity: ((u16::from(mark.opacity) * 255 + 50) / 100) as u8,
            shadow: Some(shadow(mark.shadow, 5, 5, 5)),
            watermark_position: Some(WatermarkPosition {
                horizontal: mark.horizontal,
                vertical: mark.vertical,
            }),
        });
    }
    theme.validate()?;
    let requirements = RuntimeRequirements {
        sensor_roles: context.sensors.clone(),
        fonts: context.fonts.clone(),
        resources: context
            .resources
            .iter()
            .map(|path| (path.clone(), context.image_resources.contains(path)))
            .collect(),
    };
    Ok((
        PreparedRuntime {
            source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
            theme,
            used_sensor_roles: context.sensors.into_iter().collect(),
            used_fonts: context.fonts.into_iter().collect(),
            used_resources: context.resources.into_iter().collect(),
            layout_review_required: true,
            review,
            pixel_parity_verified: false,
            resource_payloads_verified: false,
            sensor_binding_verified: false,
            editor_banks_converted: false,
            apply_supported: false,
        },
        requirements,
    ))
}
fn shadow(opacity: u8, x: i16, y: i16, blur: u8) -> TextShadow {
    TextShadow {
        x,
        y,
        blur,
        color: [0; 3],
        opacity,
    }
}
fn validate_layout(layout: &Layout, colors: usize) -> Result<(), Error> {
    if (!layout.font_family.is_empty() && !FONTS.contains(&layout.font_family.as_str()))
        || layout.font_colors.len() != colors
        || !(8..=160).contains(&layout.font_size)
        || layout.shadow > 100
        || layout.opacity > 100
        || !matches!(
            layout.horizontal_alignment.as_str(),
            "Left" | "Center" | "Right"
        )
        || !matches!(
            layout.vertical_alignment.as_str(),
            "Top" | "Center" | "Bottom"
        )
        || !matches!(layout.text_alignment.as_str(), "Left" | "Center" | "Right")
    {
        return Err(Error::Invalid("invalid Screen text layout/color count"));
    }
    Ok(())
}
fn one(layout: Vec<Layout>, colors: usize) -> Result<Layout, Error> {
    if layout.len() != 1 {
        return Err(Error::Invalid("basic Screen overlay requires one layout"));
    }
    let layout = layout
        .into_iter()
        .next()
        .ok_or(Error::Invalid("missing text layout"))?;
    validate_layout(&layout, colors)?;
    Ok(layout)
}
fn style(
    layout: &Layout,
    font: &FontBinding,
    color: usize,
    x: u16,
    y: u16,
    size: u16,
    align: TextAlign,
) -> TextStyle {
    TextStyle {
        font_asset: font.font_asset.clone(),
        font: font.font,
        line_box: true,
        max_width: None,
        line_height: Some(size),
        vertical_align: VerticalAlign::Top,
        opacity: layout.opacity,
        shadow: Some(shadow(layout.shadow, 3, 3, 10)),
        x,
        y,
        size,
        color: layout.font_colors[color],
        align,
    }
}
fn font_name(layout: &Layout) -> &str {
    if layout.font_family.is_empty() {
        "HyperspaceRace_BoldItalic"
    } else {
        &layout.font_family
    }
}
fn sizes(font: &str, kind: &str) -> (u16, u16) {
    match kind {
        "Time" => (
            match font {
                "Caprasimo_Regular" | "GoodTimesRg_Italic" => 84,
                "LTCFournierLeJeune_Regular" => 112,
                "ManoloMono_Regular" => 116,
                _ => 100,
            },
            32,
        ),
        "Grid" => match font {
            "Caprasimo_Regular" => (100, 40),
            "GoodTimesRg_Italic" | "ManoloMono_Regular" | "NotoSansTC_Bold" => (92, 35),
            "LTCFournierLeJeune_Regular" => (114, 35),
            _ => (114, 46),
        },
        _ => (
            match font {
                "Caprasimo_Regular" | "GoodTimesRg_Italic" => 112,
                "LTCFournierLeJeune_Regular" => 152,
                "ManoloMono_Regular" => 154,
                _ => 135,
            },
            32,
        ),
    }
}
fn block_top(layout: &Layout, height: u16) -> u16 {
    match layout.vertical_alignment.as_str() {
        "Top" => 22,
        "Bottom" => 458 - height,
        _ => (480 - height) / 2,
    }
}
fn parts(
    layout: &Layout,
    main: &FontBinding,
    sub: &FontBinding,
    position: (u16, u16),
    sizes: (u16, u16),
    align: TextAlign,
) -> (TextStyle, SensorParts) {
    let (x, y) = position;
    let (size, small) = sizes;
    (
        style(layout, main, 1, x, y + small, size, align),
        SensorParts {
            label: style(layout, sub, 0, x, y, small, align),
            unit: style(layout, sub, 2, x, y + small + size, small, align),
        },
    )
}
fn sensor_layer(sensor: ThemeSensor, style: TextStyle, parts: SensorParts) -> ThemeLayer {
    ThemeLayer::Sensor {
        parts: Some(parts),
        sensor_id: sensor.sensor_id,
        label: sensor.label,
        unit: sensor.unit,
        temperature_unit: sensor.temperature_unit,
        decimals: sensor.decimals,
        style,
    }
}
fn overlay(
    overlay: Overlay,
    theme: &mut LcdTheme,
    context: &mut Context<'_>,
    review: &mut Vec<&'static str>,
) -> Result<(), Error> {
    match overlay {
        Overlay::None => {}
        Overlay::Custom { text, layout } => {
            let layout = one(layout, 1)?;
            let font = context.font(font_name(&layout))?;
            let left = match layout.horizontal_alignment.as_str() {
                "Left" => 22,
                "Right" => 144,
                _ => 83,
            };
            let (offset, align) = match layout.text_alignment.as_str() {
                "Left" => (0, TextAlign::Left),
                "Right" => (314, TextAlign::Right),
                _ => (157, TextAlign::Center),
            };
            let (y, vertical) = match layout.vertical_alignment.as_str() {
                "Top" => (22, VerticalAlign::Top),
                "Bottom" => (458, VerticalAlign::Bottom),
                _ => (240, VerticalAlign::Center),
            };
            let mut s = style(&layout, &font, 0, left + offset, y, layout.font_size, align);
            s.max_width = Some(314);
            s.vertical_align = vertical;
            s.line_height = Some(match layout.font_size {
                25 => 29,
                43 => 58,
                65 => 77,
                size => size,
            });
            // Windows explicitly removes empty newline entries in TextCustomShowMode.
            let text = text
                .split('\n')
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            theme.layers.push(ThemeLayer::Text { text, style: s });
        }
        Overlay::Time { layout } => {
            let layout = one(layout, 3)?;
            let font = context.font(font_name(&layout))?;
            let sub = context.font("NotoSansTC_Bold")?;
            let (size, small) = sizes(font_name(&layout), "Time");
            let y = block_top(&layout, size + 2 * small);
            let x = match layout.horizontal_alignment.as_str() {
                "Left" => 140,
                "Right" => 340,
                _ => 240,
            };
            for (format, color, offset, font, size) in [
                (ClockFormat::Date, 0, 0, &sub, small),
                (ClockFormat::TimeWithoutPeriod, 1, small, &font, size),
                (ClockFormat::Period, 2, small + size, &sub, small),
            ] {
                theme.layers.push(ThemeLayer::Clock {
                    format,
                    twenty_four_hour: false,
                    show_seconds: false,
                    style: style(&layout, font, color, x, y + offset, size, TextAlign::Center),
                });
            }
        }
        Overlay::SliderSensor {
            sensors,
            seconds,
            layout,
        } => {
            if sensors.is_empty() || sensors.len() > 16 || !(1..=3600).contains(&seconds) {
                return Err(Error::Invalid("invalid Screen slider roles or interval"));
            }
            let layout = one(layout, 3)?;
            let font = context.font(font_name(&layout))?;
            let sub = context.font("NotoSansTC_Bold")?;
            let (size, small) = sizes(font_name(&layout), "Slider");
            let y = block_top(&layout, size + 2 * small);
            let (x, align) = match layout.horizontal_alignment.as_str() {
                "Left" => (22, TextAlign::Left),
                "Right" => (458, TextAlign::Right),
                _ => (240, TextAlign::Center),
            };
            let (style, parts) = parts(&layout, &font, &sub, (x, y), (size, small), align);
            let sensors = sensors
                .iter()
                .map(|role| context.sensor(role))
                .collect::<Result<_, _>>()?;
            theme.layers.push(ThemeLayer::SensorCycle {
                parts: Some(parts),
                sensors,
                interval_seconds: seconds,
                style,
            });
        }
        Overlay::DuoSensor { sensors, layout } => {
            if sensors.len() != 2 || layout.len() != 2 {
                return Err(Error::Invalid("DuoSensor requires two roles/layouts"));
            }
            let left = &layout[0];
            let right = &layout[1];
            if left.horizontal_alignment != "Left"
                || right.horizontal_alignment != "Right"
                || left.vertical_alignment != right.vertical_alignment
                || left.font_family != right.font_family
                || left.shadow != right.shadow
                || left.opacity != right.opacity
                || left.text_alignment != "Center"
                || right.text_alignment != "Center"
            {
                return Err(Error::Invalid(
                    "DuoSensor requires the paired layout emitted by the Windows editor",
                ));
            }
            for (i, (role, layout)) in sensors.iter().zip(layout).enumerate() {
                validate_layout(&layout, 3)?;
                let sensor = context.sensor(role)?;
                let font = context.font(font_name(&layout))?;
                let sub = context.font("NotoSansTC_Bold")?;
                let (size, small) = sizes(font_name(&layout), "Duo");
                let y = block_top(&layout, size + 2 * small);
                let (style, parts) = parts(
                    &layout,
                    &font,
                    &sub,
                    (if i == 0 { 125 } else { 355 }, y),
                    (size, small),
                    TextAlign::Center,
                );
                theme.layers.push(sensor_layer(sensor, style, parts));
            }
            review.push("DuoSensor uses native half-canvas columns; Windows intrinsic-width/family margin adjustments require comparison.");
        }
        Overlay::ThreeSensor { sensors, layout } => {
            grid(sensors, layout, 3, theme, context, review)?
        }
        Overlay::FourSensor { sensors, layout } => {
            grid(sensors, layout, 4, theme, context, review)?
        }
    }
    Ok(())
}
fn grid(
    sensors: Vec<String>,
    layout: Vec<Layout>,
    count: usize,
    theme: &mut LcdTheme,
    context: &mut Context<'_>,
    review: &mut Vec<&'static str>,
) -> Result<(), Error> {
    if sensors.len() != count {
        return Err(Error::Invalid("invalid basic Screen grid role count"));
    }
    let layout = one(layout, 2)?;
    if layout.opacity != 100 {
        return Err(Error::Invalid(
            "Windows grid renderer requires the emitted100-percent opacity",
        ));
    }
    let font = context.font(font_name(&layout))?;
    let (big, small) = sizes(font_name(&layout), "Grid");
    for (i, role) in sensors.iter().enumerate() {
        let sensor = context.sensor(role)?;
        let (x, y) = if count == 3 && i == 0 {
            (240, 34)
        } else {
            let index = if count == 3 { i - 1 } else { i };
            (
                if index.is_multiple_of(2) { 133 } else { 347 },
                if count == 3 || index >= 2 { 270 } else { 34 },
            )
        };
        let mut value_style = style(&layout, &font, 0, x, y, big, TextAlign::Center);
        let mut label_style = style(&layout, &font, 1, x - 8, y + big, small, TextAlign::Right);
        let mut unit_style = style(&layout, &font, 1, x + 8, y + big, small, TextAlign::Left);
        for style in [&mut value_style, &mut label_style, &mut unit_style] {
            style.shadow = Some(shadow(layout.shadow, 0, 3, 2));
        }
        let style = value_style;
        let parts = SensorParts {
            label: label_style,
            unit: unit_style,
        };
        theme.layers.push(sensor_layer(sensor, style, parts));
    }
    // Bounds observed from the frozen Full-mode component/CSS. Native explicit
    // geometry remains editable; font/value-dependent flex reflow is not implied.
    let line = |axis, x, y, length| ThemeLayer::Divider {
        axis,
        x,
        y,
        length,
        thickness: 2.0,
        color: layout.font_colors[1],
        opacity: 100,
        shadow: Some(TextShadow {
            x: 0,
            y: 0,
            blur: 5,
            color: [0; 3],
            opacity: 100,
        }),
    };
    if count == 3 {
        theme.layers.push(line(
            lconnect_ipc::DividerAxis::Horizontal,
            34.0,
            240.0,
            412.0,
        ));
        theme.layers.push(line(
            lconnect_ipc::DividerAxis::Vertical,
            241.0,
            262.0,
            166.0,
        ));
    } else {
        theme.layers.push(line(
            lconnect_ipc::DividerAxis::Vertical,
            239.0,
            53.0,
            169.0,
        ));
        theme.layers.push(line(
            lconnect_ipc::DividerAxis::Horizontal,
            34.0,
            242.0,
            412.0,
        ));
        theme.layers.push(line(
            lconnect_ipc::DividerAxis::Vertical,
            239.0,
            264.0,
            166.0,
        ));
    }
    review.push("Three/FourSensor include editable native dividers at explicit Full-mode source bounds. Font/value-dependent flex reflow, intrinsic-width spacing and Windows raster/shadow equivalence remain unqualified.");
    Ok(())
}
