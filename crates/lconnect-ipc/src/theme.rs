//! Versioned, daemon-owned LCD composition. Coordinates are in the 480px canvas.
use crate::{DisplayTemperatureUnit, IpcError, Result, SensorReading, SensorStatus, SensorUnit};
use serde::{Deserialize, Serialize};

/// Read-only canvas preview. Bounds and JPEG share one sensor/time snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LcdEditorPreview {
    pub jpeg: Vec<u8>,
    /// Independent PNG rasters in document order. Empty entries are hidden or procedural.
    #[serde(default)]
    pub layer_images: Vec<Vec<u8>>,
    /// Background plus procedural effects, without editable foreground layers.
    #[serde(default)]
    pub background: Vec<u8>,
    pub layers: Vec<Option<LayerBounds>>,
}

/// Unrotated foreground bounds in native LCD pixels, before canvas clipping.
/// Shadows are excluded. A layer without editable foreground has no bounds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayerBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayerBounds {
    /// Smallest rectangle containing both foreground regions.
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: (self.x + self.width).max(other.x + other.width) - x,
            height: (self.y + self.height).max(other.y + other.height) - y,
        }
    }
}

/// A portable theme stores explicit sensor identities; missing identities never remap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LcdTheme {
    pub version: u32,
    pub name: String,
    pub background: [u8; 3],
    /// Version 2 composition: managed media key; layers share its saved identity.
    /// Version 1 has no video background and retains the legacy renderer semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_video: Option<String>,
    /// Static image under all overlay layers; exclusive with video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image: Option<BackgroundImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_fill: Option<BackgroundFill>,
    pub layers: Vec<ThemeLayer>,
    /// Inactive editor pages. Children cannot contain another page cache.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_cache: Vec<SavedLcdPage>,
    /// Independent basic background/text editors; inactive templates never render.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basic_editor: Option<Box<BasicEditorState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LcdPageKind {
    Basic,
    Rain,
    Snow,
    Meteor,
    Fireworks,
    #[serde(rename = "snow3d")]
    Snow3d,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedLcdPage {
    pub page: LcdPageKind,
    pub theme: Box<LcdTheme>,
}

/// Separate Windows-compatible basic editor identities, also usable by native drafts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicBackground {
    ImageVideo,
    BackgroundColor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasicOverlay {
    None,
    SliderSensor,
    DuoSensor,
    ThreeSensor,
    FourSensor,
    Time,
    Custom,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicOverlayTheme {
    pub overlay: BasicOverlay,
    pub theme: Box<LcdTheme>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicEditorBank {
    pub background: BasicBackground,
    pub selected_overlay: BasicOverlay,
    pub overlays: Vec<BasicOverlayTheme>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicEditorState {
    pub selected_background: BasicBackground,
    pub banks: Vec<BasicEditorBank>,
}
impl BasicEditorState {
    pub fn themes(&self) -> impl Iterator<Item = &LcdTheme> {
        self.banks
            .iter()
            .flat_map(|bank| bank.overlays.iter().map(|item| item.theme.as_ref()))
    }
    pub fn validate(&self) -> Result<()> {
        let invalid = |s: &str| IpcError::InvalidArgument(s.into());
        if self.banks.is_empty() || self.banks.len() > 2 {
            return Err(invalid("basic editor requires one or two background banks"));
        }
        let mut backgrounds = Vec::new();
        for bank in &self.banks {
            if backgrounds.contains(&bank.background) || bank.overlays.len() != 7 {
                return Err(invalid(
                    "duplicate basic background or incomplete overlay bank",
                ));
            }
            backgrounds.push(bank.background);
            let mut overlays = Vec::new();
            for item in &bank.overlays {
                if overlays.contains(&item.overlay)
                    || !item.theme.page_cache.is_empty()
                    || item.theme.basic_editor.is_some()
                    || item.theme.page_kind() != LcdPageKind::Basic
                {
                    return Err(invalid(
                        "duplicate overlay, nested cache or non-basic editor template",
                    ));
                }
                overlays.push(item.overlay);
                item.theme.validate()?;
            }
            if !overlays.contains(&bank.selected_overlay) {
                return Err(invalid("selected basic overlay is absent"));
            }
        }
        if !backgrounds.contains(&self.selected_background) {
            return Err(invalid("selected basic background is absent"));
        }
        Ok(())
    }
}

/// Full-canvas image background; alpha reveals the saved color/gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundImage {
    pub asset: String,
    pub fit: ImageFit,
}

/// Independent per-mode settings; selecting solid retains gradient edits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundFill {
    pub mode: BackgroundMode,
    pub horizontal: GradientColors,
    pub vertical: GradientColors,
    pub radial: GradientColors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    Solid,
    Horizontal,
    Vertical,
    Radial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientColors {
    pub colors: [[u8; 3]; 2],
    pub flip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureUnit {
    /// Follow the daemon-owned global presentation preference.
    System,
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}
fn default_vertical_align(value: &VerticalAlign) -> bool {
    *value == VerticalAlign::Top
}

/// Bundled, deterministic font choices; no client filesystem paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeFont {
    #[default]
    Sans,
    Serif,
    Mono,
    NotoSansTcRegular,
    NotoSansTcBold,
    AbrilFatface,
    Caprasimo,
    Cubic11,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextShadow {
    pub x: i16,
    pub y: i16,
    pub blur: u8,
    pub color: [u8; 3],
    pub opacity: u8,
}
impl TextShadow {
    fn valid(&self) -> bool {
        (-32..=32).contains(&self.x)
            && (-32..=32).contains(&self.y)
            && self.blur <= 16
            && self.opacity <= 100
    }
}
fn default_font(font: &ThemeFont) -> bool {
    *font == ThemeFont::Sans
}
fn default_opacity(opacity: &u8) -> bool {
    *opacity == 100
}
fn opaque() -> u8 {
    100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    /// Managed user font key; when present, takes precedence over the bundled font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_asset: Option<String>,
    /// Use CSS pixels-per-em sizing and half-leading above/below glyph metrics.
    /// False preserves legacy ascent-minus-descent sizing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub line_box: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<u16>,
    #[serde(default, skip_serializing_if = "default_vertical_align")]
    pub vertical_align: VerticalAlign,
    #[serde(default, skip_serializing_if = "default_font")]
    pub font: ThemeFont,
    #[serde(default = "opaque", skip_serializing_if = "default_opacity")]
    pub opacity: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
    pub x: u16,
    pub y: u16,
    pub size: u16,
    pub color: [u8; 3],
    pub align: TextAlign,
}

/// Independently styled parts of the LCD clock; Time preserves legacy formatting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockFormat {
    #[default]
    Time,
    TimeWithoutPeriod,
    Date,
    Period,
}
fn default_clock_format(format: &ClockFormat) -> bool {
    *format == ClockFormat::Time
}

/// Optional independently styled label and unit; layer.style controls the value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorParts {
    pub label: TextStyle,
    pub unit: TextStyle,
}

/// One explicit binding in a rotating sensor layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSensor {
    pub sensor_id: String,
    pub label: String,
    pub unit: SensorUnit,
    pub temperature_unit: TemperatureUnit,
    pub decimals: u8,
}

/// Windows watermark center: positive values are canvas percentages; zero uses 22px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkPosition {
    pub horizontal: u8,
    pub vertical: u8,
}

/// Persistent editor identity and affordances; missing metadata preserves legacy themes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerEditor {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ThemeLayer {
    /// Editable axis-aligned decoration. Fractional coordinates retain half-pixel source bounds.
    Divider {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        axis: DividerAxis,
        x: f32,
        y: f32,
        length: f32,
        thickness: f32,
        color: [u8; 3],
        opacity: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextShadow>,
    },
    Fireworks {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        effect: crate::FireworksEffect,
    },
    Snow3d {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        effect: crate::Snow3dEffect,
    },
    Meteor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        effect: crate::MeteorEffect,
    },
    Weather {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        effect: crate::WeatherEffect,
    },
    SensorCycle {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parts: Option<SensorParts>,
        sensors: Vec<ThemeSensor>,
        interval_seconds: u16,
        style: TextStyle,
    },
    Text {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        text: String,
        style: TextStyle,
    },
    Sensor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parts: Option<SensorParts>,
        sensor_id: String,
        label: String,
        /// Prevent a reused identity with a changed unit from displaying nonsense.
        unit: SensorUnit,
        temperature_unit: TemperatureUnit,
        decimals: u8,
        style: TextStyle,
    },
    Clock {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        #[serde(default, skip_serializing_if = "default_clock_format")]
        format: ClockFormat,
        twenty_four_hour: bool,
        show_seconds: bool,
        style: TextStyle,
    },
    /// Daemon-managed asset key, not an arbitrary filesystem path or URL.
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        editor: Option<LayerEditor>,
        asset: String,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        fit: ImageFit,
        opacity: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextShadow>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watermark_position: Option<WatermarkPosition>,
    },
}

impl ThemeLayer {
    pub fn editor(&self) -> Option<&LayerEditor> {
        match self {
            Self::Divider { editor, .. }
            | Self::Fireworks { editor, .. }
            | Self::Snow3d { editor, .. }
            | Self::Meteor { editor, .. }
            | Self::Weather { editor, .. }
            | Self::SensorCycle { editor, .. }
            | Self::Text { editor, .. }
            | Self::Sensor { editor, .. }
            | Self::Clock { editor, .. }
            | Self::Image { editor, .. } => editor.as_ref(),
        }
    }
    pub fn hidden(&self) -> bool {
        self.editor().is_some_and(|editor| editor.hidden)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DividerAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Crop,
    Contain,
    ContainNative,
    Stretch,
}

impl LcdTheme {
    pub fn page_kind(&self) -> LcdPageKind {
        if self.fireworks_effect().is_some() {
            return LcdPageKind::Fireworks;
        }
        if self.snow3d_effect().is_some() {
            return LcdPageKind::Snow3d;
        }
        if self.meteor_effect().is_some() {
            return LcdPageKind::Meteor;
        }
        match self.weather_effect().map(|effect| effect.kind) {
            Some(crate::WeatherKind::Rain) => LcdPageKind::Rain,
            Some(crate::WeatherKind::Snow) => LcdPageKind::Snow,
            None => LcdPageKind::Basic,
        }
    }
    /// Switch a prepared basic editor while first retaining edits to its active
    /// template. Does not publish assets or change a hardware setting.
    pub fn select_basic_editor(
        &self,
        background: BasicBackground,
        overlay: BasicOverlay,
    ) -> Result<Self> {
        self.validate()?;
        let invalid = |s: &str| IpcError::InvalidArgument(s.into());
        let mut editor = self
            .basic_editor
            .clone()
            .ok_or_else(|| invalid("theme has no basic editor banks"))?;
        let mut current = self.clone();
        current.basic_editor = None;
        current.page_cache.clear();
        let active = editor
            .banks
            .iter_mut()
            .find(|bank| bank.background == editor.selected_background)
            .ok_or_else(|| invalid("selected basic background is absent"))?;
        // Windows editor background and watermark sit outside its seven text DTOs.
        // Retain them across text selection; ordinary native image layers remain
        // local to their text/composition template.
        let marks: Vec<_> = current
            .layers
            .iter()
            .filter(|layer| {
                matches!(
                    layer,
                    ThemeLayer::Image {
                        watermark_position: Some(_),
                        ..
                    }
                )
            })
            .cloned()
            .collect();
        for item in &mut active.overlays {
            item.theme.version = current.version;
            item.theme.background = current.background;
            item.theme.background_image = current.background_image.clone();
            item.theme.background_video = current.background_video.clone();
            item.theme.background_fill = current.background_fill.clone();
            item.theme.layers.retain(|layer| {
                !matches!(
                    layer,
                    ThemeLayer::Image {
                        watermark_position: Some(_),
                        ..
                    }
                )
            });
            item.theme.layers.extend(marks.iter().cloned());
        }
        let selected = active
            .overlays
            .iter_mut()
            .find(|item| item.overlay == active.selected_overlay)
            .ok_or_else(|| invalid("selected basic overlay is absent"))?;
        *selected.theme = current;
        let target = editor
            .banks
            .iter_mut()
            .find(|bank| bank.background == background)
            .ok_or_else(|| invalid("requested basic background is absent"))?;
        let mut next = target
            .overlays
            .iter()
            .find(|item| item.overlay == overlay)
            .ok_or_else(|| invalid("requested basic overlay is absent"))?
            .theme
            .as_ref()
            .clone();
        target.selected_overlay = overlay;
        editor.selected_background = background;
        next.name = self.name.clone();
        next.basic_editor = Some(editor);
        next.page_cache = self.page_cache.clone();
        next.validate()?;
        Ok(next)
    }
    /// Active page, direct effect pages and their bounded basic templates; never recursively traverses input.
    pub fn pages(&self) -> impl Iterator<Item = &LcdTheme> {
        std::iter::once(self)
            .chain(self.page_cache.iter().map(|page| page.theme.as_ref()))
            .flat_map(|page| {
                std::iter::once(page)
                    .chain(page.basic_editor.iter().flat_map(|editor| editor.themes()))
            })
    }
    pub fn video_assets(&self) -> std::collections::BTreeSet<&str> {
        self.pages()
            .filter_map(|page| page.background_video.as_deref())
            .collect()
    }
    pub fn weather_effect(&self) -> Option<&crate::WeatherEffect> {
        self.layers.iter().find_map(|layer| match layer {
            ThemeLayer::Weather { editor: _, effect } => Some(effect),
            _ => None,
        })
    }
    pub fn fireworks_effect(&self) -> Option<&crate::FireworksEffect> {
        self.layers.iter().find_map(|layer| match layer {
            ThemeLayer::Fireworks { editor: _, effect } => Some(effect),
            _ => None,
        })
    }
    pub fn snow3d_effect(&self) -> Option<&crate::Snow3dEffect> {
        self.layers.iter().find_map(|layer| match layer {
            ThemeLayer::Snow3d { editor: _, effect } => Some(effect),
            _ => None,
        })
    }
    pub fn meteor_effect(&self) -> Option<&crate::MeteorEffect> {
        self.layers.iter().find_map(|layer| match layer {
            ThemeLayer::Meteor { editor: _, effect } => Some(effect),
            _ => None,
        })
    }
    pub fn has_procedural_effect(&self) -> bool {
        self.weather_effect().is_some()
            || self.meteor_effect().is_some()
            || self.snow3d_effect().is_some()
            || self.fireworks_effect().is_some()
    }
    /// Animated procedural scenes share the existing sole H264 display owner.
    pub fn requires_streaming(&self) -> bool {
        self.background_video.is_some() || self.has_procedural_effect()
    }

    /// Every image referenced by the theme, including the separate static background.
    pub fn image_assets(&self) -> std::collections::BTreeSet<&str> {
        self.pages()
            .flat_map(|page| {
                page.background_image
                    .iter()
                    .map(|image| image.asset.as_str())
                    .chain(page.layers.iter().filter_map(|layer| match layer {
                        ThemeLayer::Image { asset, .. } => Some(asset.as_str()),
                        _ => None,
                    }))
            })
            .collect()
    }

    /// Unique managed font references, including independently styled sensor parts.
    pub fn font_assets(&self) -> std::collections::BTreeSet<&str> {
        let mut keys = std::collections::BTreeSet::new();
        for layer in self.pages().flat_map(|page| &page.layers) {
            let style = match layer {
                ThemeLayer::Text { style, .. } | ThemeLayer::Clock { style, .. } => style,
                ThemeLayer::Sensor { style, parts, .. }
                | ThemeLayer::SensorCycle { style, parts, .. } => {
                    if let Some(parts) = parts {
                        for part in [&parts.label, &parts.unit] {
                            if let Some(key) = &part.font_asset {
                                keys.insert(key.as_str());
                            }
                        }
                    }
                    style
                }
                ThemeLayer::Divider { .. }
                | ThemeLayer::Image { .. }
                | ThemeLayer::Weather { .. }
                | ThemeLayer::Meteor { .. }
                | ThemeLayer::Snow3d { .. }
                | ThemeLayer::Fireworks { .. } => continue,
            };
            if let Some(key) = &style.font_asset {
                keys.insert(key.as_str());
            }
        }
        keys
    }

    /// Validate both client input and persisted state before allocating render work.
    pub fn validate(&self) -> Result<()> {
        let invalid = |s: &str| IpcError::InvalidArgument(s.into());
        if self.page_cache.len() > 5 {
            return Err(invalid("at most five inactive LCD pages"));
        }
        if let Some(editor) = &self.basic_editor {
            if self.page_kind() != LcdPageKind::Basic {
                return Err(invalid("basic editor state must belong to a basic page"));
            }
            editor.validate()?;
        }
        let mut kinds = vec![self.page_kind()];
        for page in &self.page_cache {
            if !page.theme.page_cache.is_empty() {
                return Err(invalid("nested LCD page caches are forbidden"));
            }
            if page.page != page.theme.page_kind() || kinds.contains(&page.page) {
                return Err(invalid("duplicate or mismatched LCD page identity"));
            }
            kinds.push(page.page);
            page.theme.validate()?;
        }
        let scene_count = self
            .layers
            .iter()
            .filter(|layer| {
                matches!(
                    layer,
                    ThemeLayer::Weather { .. }
                        | ThemeLayer::Meteor { .. }
                        | ThemeLayer::Snow3d { .. }
                        | ThemeLayer::Fireworks { .. }
                )
            })
            .count();
        if scene_count > 1
            || (scene_count == 1
                && !matches!(
                    self.layers.first(),
                    Some(
                        ThemeLayer::Weather { .. }
                            | ThemeLayer::Meteor { .. }
                            | ThemeLayer::Snow3d { .. }
                            | ThemeLayer::Fireworks { .. }
                    )
                ))
        {
            return Err(invalid(
                "one procedural scene must be the first overlay layer",
            ));
        }
        if scene_count == 1 && self.version != 3 {
            return Err(invalid("procedural scenes require theme version 3"));
        }
        match (self.version, self.background_video.as_deref()) {
            (1, None) => {}
            (3, None) if scene_count == 1 => {}
            (version, Some(key))
                if (version == 2 || (version == 3 && scene_count == 1))
                    && key.len() == 32
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => {}
            _ => return Err(invalid(
                "theme v1 has no video, v2 requires managed video, v3 requires a procedural scene",
            )),
        }
        if self.background_image.is_some() && self.background_video.is_some() {
            return Err(invalid(
                "image and video backgrounds are mutually exclusive",
            ));
        }
        if self.image_assets().iter().any(|asset| {
            asset.is_empty()
                || asset.len() > 128
                || !asset
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            return Err(invalid("invalid managed image asset key"));
        }
        if self.name.trim().is_empty() || self.name.len() > 128 || self.layers.len() > 32 {
            return Err(invalid(
                "theme needs a name up to 128 bytes and at most 32 layers",
            ));
        }
        let mut editor_ids = std::collections::HashSet::new();
        for layer in &self.layers {
            if let Some(editor) = layer.editor() {
                if editor.id.is_empty()
                    || editor.id.len() > 128
                    || editor.name.len() > 256
                    || !editor_ids.insert(&editor.id)
                {
                    return Err(invalid("invalid or duplicate layer editor identity"));
                }
            }
            if let ThemeLayer::Sensor {
                parts: Some(parts), ..
            }
            | ThemeLayer::SensorCycle {
                parts: Some(parts), ..
            } = layer
            {
                validate_style(&parts.label)?;
                validate_style(&parts.unit)?;
            }
            let style = match layer {
                ThemeLayer::Divider {
                    axis,
                    x,
                    y,
                    length,
                    thickness,
                    opacity,
                    shadow,
                    ..
                } => {
                    let (width, height) = match axis {
                        DividerAxis::Horizontal => (*length, *thickness),
                        DividerAxis::Vertical => (*thickness, *length),
                    };
                    if ![x, y, length, thickness].into_iter().all(|v| v.is_finite())
                        || *x < 0.0
                        || *y < 0.0
                        || *length <= 0.0
                        || *length > 480.0
                        || *thickness < 0.25
                        || *thickness > 16.0
                        || x + width > 480.0
                        || y + height > 480.0
                        || *opacity > 100
                        || shadow.as_ref().is_some_and(|s| !s.valid())
                    {
                        return Err(invalid("invalid divider geometry, opacity or shadow"));
                    }
                    continue;
                }
                ThemeLayer::Fireworks { editor: _, effect } => {
                    effect.validate()?;
                    continue;
                }
                ThemeLayer::Snow3d { editor: _, effect } => {
                    effect.validate()?;
                    continue;
                }
                ThemeLayer::Meteor { editor: _, effect } => {
                    effect.validate()?;
                    continue;
                }
                ThemeLayer::Weather { editor: _, effect } => {
                    effect.validate()?;
                    continue;
                }
                ThemeLayer::SensorCycle {
                    sensors,
                    interval_seconds,
                    style,
                    ..
                } => {
                    if sensors.is_empty()
                        || sensors.len() > 16
                        || !(1..=3600).contains(interval_seconds)
                    {
                        return Err(invalid(
                            "sensor cycle needs 1..16 bindings and interval 1..3600 seconds",
                        ));
                    }
                    for sensor in sensors {
                        if sensor.sensor_id.trim().is_empty()
                            || sensor.sensor_id.len() > 1024
                            || sensor.sensor_id.contains('\0')
                            || sensor.label.len() > 128
                            || sensor.label.contains('\0')
                            || sensor.decimals > 2
                        {
                            return Err(invalid(
                                "invalid cycle sensor binding, label or precision",
                            ));
                        }
                    }
                    style
                }
                ThemeLayer::Text {
                    editor: _,
                    text,
                    style,
                } => {
                    if text.len() > 2048 || text.contains('\0') {
                        return Err(invalid("text exceeds 2048 bytes or contains NUL"));
                    }
                    style
                }
                ThemeLayer::Sensor {
                    sensor_id,
                    label,
                    decimals,
                    style,
                    ..
                } => {
                    if sensor_id.trim().is_empty()
                        || sensor_id.len() > 1024
                        || sensor_id.contains('\0')
                        || label.len() > 128
                        || label.contains('\0')
                        || *decimals > 2
                    {
                        return Err(invalid("invalid sensor binding, label or precision"));
                    }
                    style
                }
                ThemeLayer::Clock { style, .. } => style,
                ThemeLayer::Image {
                    asset,
                    x,
                    y,
                    width,
                    height,
                    shadow,
                    watermark_position,
                    ..
                } => {
                    if asset.is_empty()
                        || asset.len() > 128
                        || !asset
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    {
                        return Err(invalid("invalid managed image asset key"));
                    }
                    if *width == 0
                        || *height == 0
                        || *width > 480
                        || *height > 480
                        || (watermark_position.is_none()
                            && (u32::from(*x) + u32::from(*width) > 480
                                || u32::from(*y) + u32::from(*height) > 480))
                    {
                        return Err(invalid("image rectangle must fit the 480px canvas"));
                    }
                    if watermark_position
                        .as_ref()
                        .is_some_and(|p| p.horizontal > 100 || p.vertical > 100)
                    {
                        return Err(invalid("watermark position must be in 0..100"));
                    }
                    if shadow.as_ref().is_some_and(|s| !s.valid()) {
                        return Err(invalid("invalid image shadow"));
                    }
                    continue;
                }
            };
            validate_style(style)?;
        }
        Ok(())
    }
}

fn validate_style(style: &TextStyle) -> Result<()> {
    if style.font_asset.as_ref().is_some_and(|key| {
        key.len() != 32
            || !key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err(IpcError::InvalidArgument("invalid managed font key".into()));
    }
    if style.max_width.is_some_and(|w| !(1..=480).contains(&w))
        || style.line_height.is_some_and(|h| !(8..=320).contains(&h))
    {
        return Err(IpcError::InvalidArgument(
            "invalid text width (1..480) or line height (8..320)".into(),
        ));
    }
    if style.opacity > 100 || style.shadow.as_ref().is_some_and(|s| !s.valid()) {
        return Err(IpcError::InvalidArgument(
            "invalid text opacity or shadow".into(),
        ));
    }
    if style.x >= 480 || style.y >= 480 || !(8..=160).contains(&style.size) {
        return Err(IpcError::InvalidArgument(
            "invalid text position or size (8..160)".into(),
        ));
    }
    Ok(())
}

/// Display suffix shared by combined and independently styled sensor parts.
pub fn theme_sensor_unit(unit: SensorUnit, temperature_unit: TemperatureUnit) -> &'static str {
    match unit {
        SensorUnit::Celsius if temperature_unit == TemperatureUnit::Fahrenheit => "°F",
        SensorUnit::Celsius => "°C",
        SensorUnit::Percent => "%",
        SensorUnit::Rpm => " RPM",
        SensorUnit::Watts => " W",
        SensorUnit::Volts => " V",
        SensorUnit::Hertz => " MHz",
        SensorUnit::BytesPerSecond => " B/s",
        SensorUnit::FramesPerSecond => " FPS",
        SensorUnit::Bytes => " GiB",
    }
}

/// Read the exact bound source in native units, respecting cache freshness.
/// Reused identities with changed units and invalid/stale values remain unavailable.
pub fn valid_sensor_value(
    readings: &[SensorReading],
    sensor_id: &str,
    unit: SensorUnit,
) -> Option<f64> {
    let reading = readings.iter().find(|r| r.id == sensor_id)?;
    if reading.unit != unit
        || reading.status != SensorStatus::Valid
        || !matches!(reading.age_ms, Some(0..=3000))
    {
        return None;
    }
    reading.value.filter(|v| v.is_finite())
}

/// Format a matching, fresh reading. Missing/error values remain visibly unavailable.
/// The renderer supplies the current cache snapshot each frame; it must not reuse it
/// indefinitely. The registry's three-second freshness limit also applies here.
pub fn theme_sensor_text(
    readings: &[SensorReading],
    sensor_id: &str,
    unit: SensorUnit,
    temperature_unit: TemperatureUnit,
    decimals: u8,
) -> String {
    let Some(mut value) = valid_sensor_value(readings, sensor_id, unit) else {
        return "—".into();
    };
    match unit {
        SensorUnit::Celsius if temperature_unit == TemperatureUnit::Fahrenheit => {
            value = value * 1.8 + 32.0
        }
        SensorUnit::Bytes => value /= 1024.0 * 1024.0 * 1024.0,
        SensorUnit::Hertz => value /= 1_000_000.0,
        _ => {}
    }
    let suffix = theme_sensor_unit(unit, temperature_unit);
    if !value.is_finite() {
        return "—".into();
    }
    format!("{:.*}{suffix}", usize::from(decimals.min(2)), value)
}

impl LcdTheme {
    /// Resolve only display fields; source units, ranges and stored theme stay unchanged.
    pub fn resolve_temperature_units(&self, unit: DisplayTemperatureUnit) -> Self {
        let mut theme = self.clone();
        let resolved = match unit {
            DisplayTemperatureUnit::Celsius => TemperatureUnit::Celsius,
            DisplayTemperatureUnit::Fahrenheit => TemperatureUnit::Fahrenheit,
        };
        for layer in &mut theme.layers {
            match layer {
                ThemeLayer::Sensor {
                    temperature_unit, ..
                } if *temperature_unit == TemperatureUnit::System => *temperature_unit = resolved,
                ThemeLayer::SensorCycle { sensors, .. } => {
                    for sensor in sensors {
                        if sensor.temperature_unit == TemperatureUnit::System {
                            sensor.temperature_unit = resolved;
                        }
                    }
                }
                _ => {}
            }
        }
        theme
    }
}
