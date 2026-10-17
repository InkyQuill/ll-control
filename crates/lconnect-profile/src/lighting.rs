//! Complete source-derived LCDV2 lighting banks, including inactive sensor settings.
//! No Linux sensor binding, extraction, publication, persistence or device I/O.
use crate::{read_selected, Error, Group, Inspection, Limits};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

const FAN: &[&str] = &[
    "Rainbow",
    "RainbowMorph",
    "StaticColor",
    "Breathing",
    "Runway",
    "Meteor",
];
const PUMP: &[&str] = &[
    "Rainbow",
    "RainbowMorph",
    "StaticColor",
    "Breathing",
    "Runway",
    "Meteor",
    "TickerTape",
    "Fluctuation",
    "Transmit",
    "ColorfulStarryNight",
    "StaticStarryNight",
    "Voice",
    "BigBang",
    "Burst",
    "ColorsMorph",
    "Bounce",
];
const SENSORS: &[&str] = &[
    "CPULoad",
    "CPUTemperature",
    "GPULoad",
    "GPUTemperature",
    "PumpRPM",
    "CoolantTemp",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Parameters {
    /// Normalized byte RGB, matching the runtime converter; storage may use WPF objects.
    pub colors: Vec<[u8; 3]>,
    /// int.MinValue means not applicable; retain it rather than invent a value.
    pub speed: i32,
    pub brightness: i32,
    pub direction: u8,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct FanProfile {
    mode: u8,
    colors: Vec<[u8; 3]>,
    speed: i32,
    brightness: i32,
    direction: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct SensorSettings {
    pub high: Parameters,
    pub low: Parameters,
    pub high_value: i32,
    pub low_value: i32,
    pub max_value: i32,
    pub min_value: i32,
    /// Retained as source presentation; not an inferred Linux sensor unit.
    pub unit_text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct PumpProfile {
    pub mode: u8,
    pub is_dynamic_mode: bool,
    pub r#static: Parameters,
    pub dynamic_settings: BTreeMap<String, SensorSettings>,
    pub sensor_type: u8,
}
#[derive(Debug, Serialize)]
pub struct LightingInspection {
    pub archive: Inspection,
    pub record_index: usize,
    pub selected_fan_mode: String,
    pub selected_pump_mode: String,
    pub fan_sync_to_pump: bool,
    /// The saved loop count and size are both retained; sync selects size280/360 (16/24 LEDs)
    /// in the Windows controller; otherwise the saved loop count is used.
    pub fan_loop_number: u8,
    pub fan_size_mm: u16,
    pub effective_fan_led_count: u8,
    pub fan: BTreeMap<String, Parameters>,
    pub pump: BTreeMap<String, PumpProfile>,
    pub unconverted_controller_fields: Vec<String>,
    pub group_import_supported: bool,
    pub sensor_binding_verified: bool,
    /// Original default-serialized WPF Color objects, keyed by source JSON pointer.
    /// Runtime policies use byte RGB; this metadata is retained but never executed.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub source_color_objects: BTreeMap<String, Value>,
}

/// Decode all lighting profiles from one explicit LCDV2 record in a2.1.28 ZIP.
/// Other controller fields are listed, never silently counted as converted.
pub fn inspect_windows_lighting<R: Read + Seek>(
    reader: R,
    record_index: usize,
) -> Result<LightingInspection, Error> {
    let (archive, selected) = read_selected(reader, Limits::default(), Some(record_index))?;
    if archive.group != Group::FanAndLighting {
        return Err(Error::Invalid(
            "lighting conversion requires a FanAndLighting archive",
        ));
    }
    let mut selected =
        selected.ok_or(Error::Invalid("selected record is not an LCDV2 controller"))?;
    let mut source_color_objects = BTreeMap::new();
    for key in ["FanLEDProfiles", "ScreenLEDProfiles"] {
        if let Some(value) = selected.get_mut(key) {
            for (path, object) in crate::wpf_color::normalize(value)? {
                source_color_objects.insert(format!("/{key}{path}"), object);
            }
        }
    }
    let object = selected
        .as_object()
        .ok_or(Error::Invalid("controller profile must be an object"))?;
    let field = |key: &str| {
        object
            .get(key)
            .cloned()
            .ok_or(Error::Invalid("missing LCDV2 lighting field"))
    };
    let fan_source: BTreeMap<String, FanProfile> =
        serde_json::from_value(field("FanLEDProfiles")?)?;
    let pump: BTreeMap<String, PumpProfile> = serde_json::from_value(field("ScreenLEDProfiles")?)?;
    complete(&fan_source, FAN)?;
    complete(&pump, PUMP)?;
    let selected_fan_mode = selected_mode(&field("FanLEDMode")?, FAN)?;
    let selected_pump_mode = selected_mode(&field("ScreenLEDMode")?, PUMP)?;
    let fan_sync_to_pump: bool = serde_json::from_value(field("FanLEDSyncToScreenLED")?)?;
    let fan_loop_number: u8 = serde_json::from_value(field("FanLEDLoopNumber")?)?;
    let size: u8 = serde_json::from_value(field("FanLEDSize")?)?;
    let (fan_size_mm, size_count) = match size {
        0 => (280, 16),
        1 => (360, 24),
        _ => return Err(Error::Invalid("unsupported fan LED size")),
    };
    let mut fan = BTreeMap::new();
    for (index, name) in FAN.iter().enumerate() {
        let profile = &fan_source[*name];
        if usize::from(profile.mode) != index + 1 {
            return Err(Error::Invalid("fan lighting mode/key mismatch"));
        }
        let parameters = Parameters {
            colors: profile.colors.clone(),
            speed: profile.speed,
            brightness: profile.brightness,
            direction: profile.direction,
        };
        validate_parameters(&parameters, index)?;
        fan.insert((*name).into(), parameters);
    }
    for (index, name) in PUMP.iter().enumerate() {
        let profile = &pump[*name];
        if usize::from(profile.mode) != index + 1 || !(1..=6).contains(&profile.sensor_type) {
            return Err(Error::Invalid(
                "pump lighting mode/key or sensor selection mismatch",
            ));
        }
        validate_parameters(&profile.r#static, index)?;
        complete(&profile.dynamic_settings, SENSORS)?;
        for sensor in profile.dynamic_settings.values() {
            validate_parameters(&sensor.high, index)?;
            validate_parameters(&sensor.low, index)?;
            if sensor.min_value > sensor.low_value
                || sensor.low_value > sensor.high_value
                || sensor.high_value > sensor.max_value
                || sensor.unit_text.len() > 32
                || sensor.unit_text.chars().any(char::is_control)
            {
                return Err(Error::Invalid(
                    "invalid dynamic lighting thresholds or unit text",
                ));
            }
        }
    }
    let converted = [
        "FanLEDProfiles",
        "ScreenLEDProfiles",
        "FanLEDMode",
        "ScreenLEDMode",
        "FanLEDSyncToScreenLED",
        "FanLEDLoopNumber",
        "FanLEDSize",
    ];
    Ok(LightingInspection {
        archive,
        record_index,
        selected_fan_mode,
        selected_pump_mode,
        fan_sync_to_pump,
        fan_loop_number,
        fan_size_mm,
        effective_fan_led_count: if fan_sync_to_pump {
            size_count
        } else {
            fan_loop_number
        },
        fan,
        pump,
        unconverted_controller_fields: object
            .keys()
            .filter(|key| !converted.contains(&key.as_str()))
            .cloned()
            .collect(),
        group_import_supported: false,
        sensor_binding_verified: false,
        source_color_objects,
    })
}
fn complete<T>(bank: &BTreeMap<String, T>, expected: &[&str]) -> Result<(), Error> {
    if bank.len() != expected.len() || expected.iter().any(|key| !bank.contains_key(*key)) {
        return Err(Error::Invalid(
            "lighting bank must contain all known modes/sensors and no unknown keys",
        ));
    }
    Ok(())
}
fn selected_mode(value: &Value, modes: &[&str]) -> Result<String, Error> {
    let id: u8 = serde_json::from_value(value.clone())?;
    modes
        .get(usize::from(id).wrapping_sub(1))
        .map(|name| (*name).into())
        .ok_or(Error::Invalid("unsupported selected lighting mode"))
}
fn validate_parameters(parameters: &Parameters, index: usize) -> Result<(), Error> {
    // Palette sizes from Galahad2VisionProfile default banks/ViewSettings.
    let count = [0, 0, 1, 1, 2, 4, 4, 4, 2, 0, 1, 1, 4, 1, 0, 4][index];
    if parameters.colors.len() != count
        || ![parameters.speed, parameters.brightness]
            .iter()
            .all(|v| *v == i32::MIN || (0..=100).contains(v))
        || parameters.direction > 5
    {
        return Err(Error::Invalid(
            "lighting palette, speed, brightness or direction is outside source limits",
        ));
    }
    Ok(())
}
