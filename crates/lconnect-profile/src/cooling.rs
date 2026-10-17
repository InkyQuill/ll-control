//! Source-derived LCDV2 RPM banks. No Linux binding, persistence or device I/O.
//! Every mode must validate, including inactive modes. Source settings are kept
//! alongside normalized runtime phases so presentation metadata is not lost.
use crate::{read_selected, Error, Group, Inspection, Limits};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedUnit {
    Percent,
    Rpm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Fixed,
    Curve,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimePhase {
    pub celsius: i16,
    pub speed: u16,
}

#[derive(Debug, Serialize)]
pub struct Mode {
    pub windows_mode: String,
    pub name: String,
    /// Empty/null Windows Name uses a localized factory resource, not a literal.
    pub use_default_name: bool,
    pub control: Control,
    /// LCDV2's frozen Windows service only evaluates CPU temperature.
    pub reference: &'static str,
    pub phases: Vec<RuntimePhase>,
}

#[derive(Debug, Serialize)]
pub struct Bank {
    pub selected_mode: String,
    pub speed_unit: SpeedUnit,
    pub modes: Vec<Mode>,
    /// Original RPM settings only, without Windows device Metadata or Screen/RGB.
    /// Keep chart settings/names/capability metadata until full-group mapping.
    pub source_settings: Value,
}

#[derive(Debug, Serialize)]
pub struct CoolingInspection {
    pub archive: Inspection,
    pub record_index: usize,
    pub fan: Bank,
    pub pump: Bank,
    pub unconverted_controller_fields: Vec<String>,
    pub group_import_supported: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Setting {
    supported_motherboard_sync: bool,
    supported_start_stop: bool,
    supported_rename_profile_name: bool,
    supported_fan_size: Vec<i32>,
    #[serde(rename = "SupportedRPMReferenceSource")]
    supported_reference: Vec<i32>,
    speed_unit: i32,
    mode: i32,
    profiles: BTreeMap<String, Profile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Profile {
    mode: i32,
    name: Option<String>,
    is_start_stop: bool,
    fan_size: i32,
    #[serde(rename = "RPMReferenceSource")]
    reference: i32,
    control_type: i32,
    phase_infos: BTreeMap<String, PhaseInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct PhaseInfo {
    // Presentation values are retained in source_settings, never interpreted as
    // Linux device capabilities or used to infer a source binding.
    #[serde(rename = "Adjustable")]
    _adjustable: bool,
    phases: Vec<Phase>,
    min_speed: i32,
    max_speed: i32,
    #[serde(rename = "ChartMinSpeed")]
    _chart_min_speed: i32,
    #[serde(rename = "ChartMaxSpeed")]
    _chart_max_speed: i32,
    #[serde(rename = "ChartSpeedTick")]
    _chart_speed_tick: i32,
    #[serde(rename = "RPMChartXAxisConfigs")]
    _chart_axis: Option<Value>,
    #[serde(rename = "ChartSpeedLabels")]
    _chart_speed_labels: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
struct Phase {
    temperature: i16,
    speed: i32,
}

const FAN_MODES: [(&str, i32); 8] = [
    ("Quiet", 0),
    ("StandardSpeed", 1),
    ("HighSpeed", 2),
    ("FullSpeed", 3),
    ("UserDefine1", 4),
    ("UserDefine2", 5),
    ("UserDefine3", 6),
    ("UserDefine4", 7),
];
const PUMP_MODES: [(&str, i32); 3] = [("FixRPM", 9), ("PWMQuiet", 11), ("PWMPerformance", 12)];

/// Read a selected LCDV2 record and normalize its complete cooling banks.
/// Does not convert RGB/Screen/assets or produce an importable full group.
pub fn inspect_windows_cooling<R: Read + Seek>(
    reader: R,
    record_index: usize,
) -> Result<CoolingInspection, Error> {
    let (archive, selected) = read_selected(reader, Limits::default(), Some(record_index))?;
    if archive.group != Group::FanAndLighting {
        return Err(Error::Invalid(
            "cooling conversion requires a FanAndLighting archive",
        ));
    }
    let selected = selected.ok_or(Error::Invalid("selected record is not an LCDV2 controller"))?;
    let object = selected
        .as_object()
        .ok_or(Error::Invalid("controller profile must be an object"))?;
    let fan = bank(
        object
            .get("Fan")
            .ok_or(Error::Invalid("missing Fan RPM settings"))?,
        false,
    )?;
    let pump = bank(
        object
            .get("Pump")
            .ok_or(Error::Invalid("missing Pump RPM settings"))?,
        true,
    )?;
    Ok(CoolingInspection {
        archive,
        record_index,
        fan,
        pump,
        unconverted_controller_fields: object
            .keys()
            .filter(|k| !matches!(k.as_str(), "Fan" | "Pump"))
            .cloned()
            .collect(),
        group_import_supported: false,
    })
}

fn bank(source: &Value, pump: bool) -> Result<Bank, Error> {
    let setting: Setting = serde_json::from_value(source.clone())?;
    let (expected, speed_unit, minimum, maximum): (&[(&str, i32)], _, _, _) = if pump {
        (&PUMP_MODES, SpeedUnit::Rpm, 800, 3600)
    } else {
        (&FAN_MODES, SpeedUnit::Percent, 10, 100)
    };
    if setting.speed_unit != if pump { 0 } else { 1 }
        || setting.supported_start_stop
        || setting.supported_motherboard_sync != pump
        || !setting.supported_rename_profile_name
        || setting.supported_fan_size != [0]
        || setting.supported_reference != [0]
    {
        return Err(Error::Invalid(
            "unsupported LCDV2 RPM units or capabilities",
        ));
    }
    if setting.profiles.len() != expected.len()
        || expected
            .iter()
            .any(|(key, _)| !setting.profiles.contains_key(*key))
    {
        return Err(Error::Invalid(
            "RPM bank must contain all known LCDV2 modes and no unknown modes",
        ));
    }
    let selected_mode = expected
        .iter()
        .find(|(_, id)| *id == setting.mode)
        .ok_or(Error::Invalid(
            "selected RPM mode is not supported for this channel",
        ))?
        .0
        .to_owned();
    let mut modes = Vec::new();
    for &(key, id) in expected {
        let profile = &setting.profiles[key];
        let fixed = pump && id == 9;
        let phase_key = if fixed {
            "None|CPU|Fixed"
        } else {
            "None|CPU|Curve"
        };
        if profile.mode != id
            || profile.is_start_stop
            || profile.fan_size != 0
            || profile.reference != 0
            || profile.control_type != if fixed { 0 } else { 1 }
            || profile.phase_infos.len() != 1
            || !profile.phase_infos.contains_key(phase_key)
        {
            return Err(Error::Invalid(
                "unsupported or inconsistent LCDV2 RPM profile/key/reference",
            ));
        }
        let info = &profile.phase_infos[phase_key];
        if !(minimum..=maximum).contains(&info.min_speed)
            || !(info.min_speed..=maximum).contains(&info.max_speed)
            || info
                .phases
                .iter()
                .any(|p| !(info.min_speed..=info.max_speed).contains(&p.speed))
        {
            return Err(Error::Invalid(
                "RPM profile speed is outside channel or profile limits",
            ));
        }
        if (fixed && info.phases.len() != 1)
            || (!fixed
                && (!(2..=14).contains(&info.phases.len())
                    || info
                        .phases
                        .iter()
                        .any(|p| !(1..=99).contains(&p.temperature))
                    || info
                        .phases
                        .windows(2)
                        .any(|p| p[0].temperature >= p[1].temperature || p[0].speed > p[1].speed)))
        {
            return Err(Error::Invalid(
                "RPM phases cannot be represented losslessly by the current Linux policy",
            ));
        }
        let name = profile
            .name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(key);
        if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err(Error::Invalid(
                "RPM profile name cannot be represented by a Linux profile",
            ));
        }
        let mut phases: Vec<_> = info
            .phases
            .iter()
            .map(|p| RuntimePhase {
                celsius: p.temperature,
                // Bounds above establish positivity and u16 range.
                speed: p.speed as u16,
            })
            .collect();
        if !fixed {
            phases.insert(
                0,
                RuntimePhase {
                    celsius: 0,
                    speed: info.min_speed as u16,
                },
            );
            phases.push(RuntimePhase {
                celsius: 100,
                speed: info.max_speed as u16,
            });
        }
        modes.push(Mode {
            windows_mode: key.into(),
            name: name.into(),
            use_default_name: profile.name.as_deref().is_none_or(str::is_empty),
            control: if fixed {
                Control::Fixed
            } else {
                Control::Curve
            },
            reference: "cpu",
            phases,
        });
    }
    Ok(Bank {
        selected_mode,
        speed_unit,
        modes,
        source_settings: source.clone(),
    })
}
