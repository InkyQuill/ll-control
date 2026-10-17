//! Basic Windows editor settings -> seven separately retained native proposals.
//! Runtime config is not used to reconstruct inactive editor settings.
use crate::{
    linux_screen::{prepare_runtime, Bindings, PreparedRuntime, FONTS},
    screen::ScreenInspection,
    strict_json, Error,
};
use lconnect_ipc::{BackgroundFill, BackgroundMode, GradientColors};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
const OVERLAYS: &[&str] = &[
    "None",
    "SliderSensor",
    "DuoSensor",
    "ThreeSensor",
    "FourSensor",
    "Time",
    "Custom",
];

const ROLES: &[&str] = &[
    "Unknown",
    "CPUTemperature",
    "GPUTemperature",
    "CPULoad",
    "GPULoad",
    "PumpRPM",
    "CoolantTemp",
];
const COLORS: &[&str] = &[
    "SolidColor",
    "HorizontalGradient",
    "VerticalGradient",
    "RadialGradient",
];
#[derive(Debug, Serialize)]
pub struct PreparedEditor {
    pub source_sha256: String,
    pub source_default_materialized: bool,
    pub source_absent_materialized: bool,
    pub selected_overlay: String,
    /// Every supported overlay, including inactive custom/time/sensor settings.
    pub overlays: BTreeMap<String, PreparedRuntime>,
}
#[derive(Debug, Serialize)]
pub struct PreparedSlot {
    pub index: usize,
    pub source_name: Option<String>,
    pub selected_effect: String,
    pub style_express_effect: String,
    pub basic: BTreeMap<String, PreparedEditor>,
    /// Still retained by the source inspector, never replaced with another effect.
    pub deferred_effects: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationScope {
    CurrentEditor,
    AllSourceEntries,
}
#[derive(Debug, Serialize)]
pub struct PreparedBanks {
    pub scope: PreparationScope,
    pub slots: Vec<PreparedSlot>,
    pub basic_editor_banks_prepared: bool,
    pub basic_bank_count: usize,
    pub overlay_count: usize,
    pub complete_screen_group_supported: bool,
    pub apply_supported: bool,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Aligned {
    label_color: Vec<[u8; 3]>,
    font_family: u8,
    vertical_alignment: u8,
    horizontal_alignment: u8,
    text_alignment: u8,
    shadow: u8,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Slider {
    sensor_types: Vec<u8>,
    loop_second: u16,
    label_color: Vec<[u8; 3]>,
    font_family: u8,
    vertical_alignment: u8,
    horizontal_alignment: u8,
    text_alignment: u8,
    shadow: u8,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Duo {
    sensor_types: Vec<u8>,
    label_color: Vec<[u8; 3]>,
    font_family: u8,
    vertical_alignment: u8,
    shadow: u8,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Grid {
    sensor_types: Vec<u8>,
    label_color: Vec<[u8; 3]>,
    font_family: u8,
    shadow: u8,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Custom {
    text: String,
    label_color: Vec<[u8; 3]>,
    font_size: u8,
    font_family: u8,
    vertical_alignment: u8,
    horizontal_alignment: u8,
    text_alignment: u8,
    shadow: u8,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Settings {
    image_video_path: Option<String>,
    background_color: Value,
    text_theme_type: u8,
    text_slider_sensor: Slider,
    text_duo_sensor: Duo,
    text_three_sensor: Grid,
    text_four_sensor: Grid,
    text_time: Aligned,
    text_custom: Custom,
    watermark: Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct ColorSettings {
    r#type: u8,
    color_settings: BTreeMap<String, Color>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Color {
    colors: Vec<[u8; 3]>,
    flip: bool,
}
fn enum_name<'a>(names: &[&'a str], id: u8) -> Result<&'a str, Error> {
    names
        .get(usize::from(id))
        .copied()
        .ok_or(Error::Invalid("unknown Windows editor enum"))
}
fn layout(
    colors: &[[u8; 3]],
    font: u8,
    horizontal: u8,
    vertical: u8,
    alignment: u8,
    shadow: u8,
    size: u16,
) -> Result<Value, Error> {
    Ok(
        json!({"FontColors":colors,"FontFamily":enum_name(FONTS,font)?,"FontSize":size,
        "HorizontalAlignment":enum_name(&["Left","Center","Right"],horizontal)?,"VerticalAlignment":enum_name(&["Top","Center","Bottom"],vertical)?,
        "TextAlignment":enum_name(&["Left","Right","Center"],alignment)?,"Shadow":shadow,"Opacity":100}),
    )
}
fn roles(ids: &[u8]) -> Result<Vec<&str>, Error> {
    ids.iter()
        .map(|id| {
            if *id == 0 {
                Err(Error::Invalid(
                    "Unknown is not an executable Screen sensor binding",
                ))
            } else {
                enum_name(ROLES, *id)
            }
        })
        .collect()
}
impl Settings {
    fn overlays(&self) -> Result<Vec<Value>, Error> {
        let s = &self.text_slider_sensor;
        let d = &self.text_duo_sensor;
        let t = &self.text_time;
        let c = &self.text_custom;
        let grid = |g: &Grid, kind: &str| -> Result<Value, Error> {
            Ok(
                json!({"Type":kind,"Sensors":roles(&g.sensor_types)?,"Layout":[layout(&g.label_color,g.font_family,1,1,2,g.shadow,43)?]}),
            )
        };
        Ok(vec![
            json!({"Type":"None"}),
            json!({"Type":"SliderSensor","Sensors":roles(&s.sensor_types)?,"LoopSecond":s.loop_second,"Layout":[layout(&s.label_color,s.font_family,s.horizontal_alignment,s.vertical_alignment,s.text_alignment,s.shadow,43)?]}),
            json!({"Type":"DuoSensor","Sensors":roles(&d.sensor_types)?,"Layout":[layout(&d.label_color,d.font_family,0,d.vertical_alignment,2,d.shadow,43)?,layout(&d.label_color,d.font_family,2,d.vertical_alignment,2,d.shadow,43)?]}),
            grid(&self.text_three_sensor, "ThreeSensor")?,
            grid(&self.text_four_sensor, "FourSensor")?,
            json!({"Type":"Time","Layout":[layout(&t.label_color,t.font_family,t.horizontal_alignment,t.vertical_alignment,t.text_alignment,t.shadow,43)?]}),
            json!({"Type":"Custom","Text":c.text,"Layout":[layout(&c.label_color,c.font_family,c.horizontal_alignment,c.vertical_alignment,c.text_alignment,c.shadow,*[25,43,65].get(usize::from(c.font_size)).ok_or(Error::Invalid("unknown custom text size enum"))? )?]}),
        ])
    }
}
/// Prepare existing basic entries. Windows lazily imports empty settings for an
/// absent selected editor at index0; only that selected bank materializes defaults.
/// Unused source positions and absent inactive banks are never manufactured.
pub fn prepare_basic_banks(
    source: &ScreenInspection,
    bindings: &Bindings,
) -> Result<PreparedBanks, Error> {
    prepare_banks(source, PreparationScope::AllSourceEntries, |runtime| {
        prepare_runtime(runtime, bindings)
    })
}
/// Current index0 only. Unassigned array entries stay opaque in the source inventory.
pub fn prepare_current_banks(
    source: &ScreenInspection,
    bindings: &Bindings,
) -> Result<PreparedBanks, Error> {
    prepare_banks(source, PreparationScope::CurrentEditor, |runtime| {
        prepare_runtime(runtime, bindings)
    })
}
fn prepare_banks(
    source: &ScreenInspection,
    scope: PreparationScope,
    mut map: impl FnMut(&str) -> Result<PreparedRuntime, Error>,
) -> Result<PreparedBanks, Error> {
    if source.archive.group != crate::Group::Screen
        || source.slots.len() != 8
        || source.slots.iter().enumerate().any(|(i, s)| s.index != i)
    {
        return Err(Error::Invalid(
            "complete ordered eight-entry Screen source required",
        ));
    }
    let mut slots = Vec::new();
    for slot in source
        .slots
        .iter()
        .filter(|slot| scope == PreparationScope::AllSourceEntries || slot.index == 0)
    {
        let mut basic = BTreeMap::new();
        let mut deferred_effects = Vec::new();
        let absent_selected = slot.index == 0
            && matches!(
                slot.selected_effect.as_str(),
                "ImageVideo" | "BackgroundColor"
            )
            && !slot.editor_settings.contains_key(&slot.selected_effect);
        let default_values = vec![crate::screen::SourceJson {
            source_json: String::new(),
            sha256: format!("{:x}", Sha256::digest([])),
            uses_default: true,
        }];
        let extra = absent_selected.then_some((&slot.selected_effect, &default_values));
        for (kind, values) in slot.editor_settings.iter().chain(extra) {
            if !matches!(kind.as_str(), "ImageVideo" | "BackgroundColor") {
                deferred_effects.push(kind.clone());
                continue;
            }
            if values.len() != 1 {
                return Err(Error::Invalid(
                    "LCDV2 basic editor bank requires exactly one settings document",
                ));
            }
            let settings = &values[0].source_json;
            if settings.len() > 512 * 1024 {
                return Err(Error::Invalid("editor document exceeds512KiB"));
            }
            let default = if settings.is_empty() {
                Some(defaults(kind, source)?)
            } else {
                None
            };
            let mut value = if let Some(default) = default {
                default
            } else {
                strict_json::parse(settings.as_bytes())?
            };
            // Editor Export() uses default WPF object serialization, while GetConfigs()
            // supplies RGB arrays. Source inspector still retains the exact original JSON.
            crate::wpf_color::normalize(&mut value)?;
            let editor: Settings = serde_json::from_value(value)?;
            let selected = enum_name(OVERLAYS, editor.text_theme_type)?.to_owned();
            let (background, full_fill) = background(kind, &editor)?;
            let mut overlays = BTreeMap::new();
            for overlay in editor.overlays()? {
                let key = overlay["Type"]
                    .as_str()
                    .ok_or(Error::Invalid("missing basic overlay type"))?
                    .to_owned();
                let runtime = json!({"Style":"Full","Screens":[{"Theme":background,"Text":overlay,"Watermark":editor.watermark}]});
                let mut mapped = map(&runtime.to_string())?;
                if let Some((solid, fill)) = &full_fill {
                    mapped.theme.background = *solid;
                    mapped.theme.background_fill = Some(fill.clone());
                    mapped.theme.validate()?;
                }
                overlays.insert(key, mapped);
            }
            basic.insert(
                kind.clone(),
                PreparedEditor {
                    source_sha256: format!("{:x}", Sha256::digest(settings.as_bytes())),
                    source_default_materialized: settings.is_empty(),
                    source_absent_materialized: absent_selected && kind == &slot.selected_effect,
                    selected_overlay: selected,
                    overlays,
                },
            );
        }
        slots.push(PreparedSlot {
            index: slot.index,
            source_name: slot.name.clone(),
            selected_effect: slot.selected_effect.clone(),
            style_express_effect: slot.style_express_effect.clone(),
            basic,
            deferred_effects,
        });
    }
    let basic_bank_count = slots.iter().map(|s| s.basic.len()).sum();
    let overlay_count = slots
        .iter()
        .flat_map(|s| s.basic.values())
        .map(|b| b.overlays.len())
        .sum();
    Ok(PreparedBanks {
        scope,
        slots,
        basic_bank_count,
        overlay_count,
        basic_editor_banks_prepared: true,
        complete_screen_group_supported: false,
        apply_supported: false,
    })
}
type Fill = Option<([u8; 3], BackgroundFill)>;
fn background(kind: &str, editor: &Settings) -> Result<(Value, Fill), Error> {
    if kind == "ImageVideo" {
        let path = editor
            .image_video_path
            .as_ref()
            .ok_or(Error::Invalid("missing editor image/video reference"))?;
        let color: [u8; 3] = serde_json::from_value(editor.background_color.clone())?;
        return Ok((
            json!({"Type":"Image","Path":path,"BackgroundColor":color}),
            None,
        ));
    }
    if editor.image_video_path.is_some() {
        return Err(Error::Invalid("unexpected image path in Color editor"));
    }
    let config: ColorSettings = serde_json::from_value(editor.background_color.clone())?;
    let name = enum_name(COLORS, config.r#type)?;
    if config.color_settings.len() != 4
        || COLORS
            .iter()
            .any(|key| !config.color_settings.contains_key(*key))
    {
        return Err(Error::Invalid(
            "complete four-mode Color editor settings required",
        ));
    }
    for (key, color) in &config.color_settings {
        if color.colors.len() != if key == "SolidColor" { 1 } else { 2 } {
            return Err(Error::Invalid("invalid inactive editor gradient palette"));
        }
    }
    let get = |key: &str| {
        config
            .color_settings
            .get(key)
            .ok_or(Error::Invalid("missing Color mode"))
    };
    let gradient = |key: &str| -> Result<GradientColors, Error> {
        let c = get(key)?;
        Ok(GradientColors {
            colors: [c.colors[0], c.colors[1]],
            flip: c.flip,
        })
    };
    let mode = match config.r#type {
        0 => BackgroundMode::Solid,
        1 => BackgroundMode::Horizontal,
        2 => BackgroundMode::Vertical,
        3 => BackgroundMode::Radial,
        _ => return Err(Error::Invalid("unknown Color mode")),
    };
    let selected = get(name)?;
    Ok((
        json!({"Type":"Color","ColorType":name,"Colors":selected.colors,"Flip":selected.flip}),
        Some((
            get("SolidColor")?.colors[0],
            BackgroundFill {
                mode,
                horizontal: gradient("HorizontalGradient")?,
                vertical: gradient("VerticalGradient")?,
                radial: gradient("RadialGradient")?,
            },
        )),
    ))
}
/// Materialize observed constructor defaults only for an explicitly empty settings
/// string. ImageVideo additionally follows Windows' scoped default gallery lookup.
fn defaults(kind: &str, source: &ScreenInspection) -> Result<Value, Error> {
    let common = |count: usize, shadow: u8| json!({"LabelColor":vec![[255u8;3];count],"FontFamily":5,"VerticalAlignment":1,"HorizontalAlignment":1,"TextAlignment":2,"Shadow":shadow});
    let mut slider = common(3, if kind == "ImageVideo" { 60 } else { 50 });
    slider["SensorTypes"] = json!([1, 2, 3, 4, 5, 6]);
    slider["LoopSecond"] = json!(10);
    let mut duo = json!({"LabelColor":[[255,255,255],[255,255,255],[255,255,255]],"FontFamily":5,"VerticalAlignment":1,"Shadow":50});
    duo["SensorTypes"] = json!([1, 2]);
    let grid = |roles: Value| json!({"LabelColor":[[255,255,255],[255,255,0]],"FontFamily":5,"Shadow":60,"SensorTypes":roles});
    let mut custom = common(1, 50);
    custom["Text"] = json!("Welcome");
    custom["FontSize"] = json!(1);
    let mut value = json!({"TextThemeType":1,"TextSliderSensor":slider,"TextDuoSensor":duo,"TextThreeSensor":grid(json!([1,2,3])),"TextFourSensor":grid(json!([1,2,3,4])),"TextTime":common(3,50),"TextCustom":custom,
        "Watermark":{"HasImage":false,"Image":"","Horizontal":50,"Vertical":50,"Shadow":50,"Opacity":50}});
    if kind == "ImageVideo" {
        let default = source.image_video.iter().find(|f| {
            let base = f.source_path.rsplit(['\\', '/']).next().unwrap_or_default();
            f.is_default_file
                && base.starts_with("a_default_")
                && base
                    .rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("mp4"))
        });
        value["ImageVideoPath"] = json!(default.map(|f| f.source_path.as_str()).unwrap_or(""));
        value["BackgroundColor"] = json!([0, 0, 0]);
    } else if kind == "BackgroundColor" {
        value["BackgroundColor"] = json!({"Type":0,"ColorSettings":{"SolidColor":{"Colors":[[52,230,209]],"Flip":false},"HorizontalGradient":{"Colors":[[52,230,209],[26,115,105]],"Flip":false},"VerticalGradient":{"Colors":[[52,230,209],[26,115,105]],"Flip":false},"RadialGradient":{"Colors":[[52,230,209],[26,115,105]],"Flip":false}}});
    } else {
        return Err(Error::Invalid("unsupported basic editor default"));
    }
    Ok(value)
}

impl PreparedSlot {
    /// Materialize one source array entry as a native basic theme. Only index0
    /// is the current Windows editor; this method never assigns a Linux named slot.
    /// Deferred selected effects cannot substitute.
    pub fn native_theme(&self) -> Result<lconnect_ipc::LcdTheme, Error> {
        use lconnect_ipc::{BasicBackground, BasicEditorBank, BasicEditorState, BasicOverlayTheme};
        let background = |name: &str| match name {
            "ImageVideo" => Ok(BasicBackground::ImageVideo),
            "BackgroundColor" => Ok(BasicBackground::BackgroundColor),
            _ => Err(Error::Invalid(
                "selected Screen effect has no basic native editor",
            )),
        };
        let selected_background = background(&self.selected_effect)?;
        let selected = self
            .basic
            .get(&self.selected_effect)
            .ok_or(Error::Invalid("selected Screen basic bank is absent"))?;
        let mut theme = selected
            .overlays
            .get(&selected.selected_overlay)
            .ok_or(Error::Invalid("selected Screen basic overlay is absent"))?
            .theme
            .clone();
        let banks = self
            .basic
            .iter()
            .map(|(kind, bank)| {
                Ok(BasicEditorBank {
                    background: background(kind)?,
                    selected_overlay: native_overlay(&bank.selected_overlay)?,
                    overlays: bank
                        .overlays
                        .iter()
                        .map(|(kind, proposal)| {
                            Ok(BasicOverlayTheme {
                                overlay: native_overlay(kind)?,
                                theme: Box::new(proposal.theme.clone()),
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        theme.basic_editor = Some(Box::new(BasicEditorState {
            selected_background,
            banks,
        }));
        theme.validate()?;
        Ok(theme)
    }
}
fn native_overlay(name: &str) -> Result<lconnect_ipc::BasicOverlay, Error> {
    use lconnect_ipc::BasicOverlay;
    match name {
        "None" => Ok(BasicOverlay::None),
        "SliderSensor" => Ok(BasicOverlay::SliderSensor),
        "DuoSensor" => Ok(BasicOverlay::DuoSensor),
        "ThreeSensor" => Ok(BasicOverlay::ThreeSensor),
        "FourSensor" => Ok(BasicOverlay::FourSensor),
        "Time" => Ok(BasicOverlay::Time),
        "Custom" => Ok(BasicOverlay::Custom),
        _ => Err(Error::Invalid("unsupported Screen basic overlay")),
    }
}

/// Required bindings for existing basic banks of current editor index0 only.
/// Source positions1..7 and inactive procedural effects stay opaque.
pub fn inspect_current_requirements(
    source: &ScreenInspection,
) -> Result<crate::linux_screen::RuntimeRequirements, Error> {
    let mut requirements = crate::linux_screen::RuntimeRequirements::default();
    prepare_banks(source, PreparationScope::CurrentEditor, |runtime| {
        let (proposal, required) = crate::linux_screen::inspect_runtime_proposal(runtime)?;
        requirements.merge(required);
        Ok(proposal)
    })?;
    Ok(requirements)
}
