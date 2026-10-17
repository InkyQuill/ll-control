//! Typed Linux lighting preparation; no daemon connection or persistent writes.
use crate::{
    lighting::{LightingInspection, Parameters},
    Error,
};
use lconnect_ipc::{
    lighting_catalog, DynamicRgbConfig, DynamicRgbProfile, DynamicRgbProfiles, FanLedSize,
    FanLightingTopology, LightingConfig, LightingKind, Rgb, SensorUnit,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const ROLES: &[(&str, SensorUnit)] = &[
    ("CPULoad", SensorUnit::Percent),
    ("CPUTemperature", SensorUnit::Celsius),
    ("GPULoad", SensorUnit::Percent),
    ("GPUTemperature", SensorUnit::Celsius),
    ("PumpRPM", SensorUnit::Rpm),
    ("CoolantTemp", SensorUnit::Celsius),
];
const MODES: &[(&str, &str)] = &[
    ("Rainbow", "rainbow"),
    ("RainbowMorph", "rainbow_morph"),
    ("StaticColor", "static"),
    ("Breathing", "breathing"),
    ("Runway", "runway"),
    ("Meteor", "meteor"),
    ("TickerTape", "ticker_tape"),
    ("Fluctuation", "fluctuation"),
    ("Transmit", "transmit"),
    ("ColorfulStarryNight", "colorful_starry_night"),
    ("StaticStarryNight", "static_starry_night"),
    ("Voice", "voice"),
    ("BigBang", "big_bang"),
    ("Burst", "burst"),
    ("ColorsMorph", "colors_morph"),
    ("Bounce", "bounce"),
];
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorBinding {
    pub sensor_id: String,
    pub unit: SensorUnit,
}
pub type SensorBindings = BTreeMap<String, SensorBinding>;

/// Parse bounded explicit bindings without accepting duplicate JSON keys.
pub fn parse_bindings(bytes: &[u8]) -> Result<SensorBindings, Error> {
    if bytes.len() > 16 * 1024 {
        return Err(Error::Invalid("lighting binding map exceeds16KiB"));
    }
    Ok(serde_json::from_value(crate::strict_json::parse(bytes)?)?)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedLighting {
    pub fan_topology: FanLightingTopology,
    pub selected_fan_mode: String,
    pub selected_pump_mode: String,
    pub fan_modes: BTreeMap<String, LightingConfig>,
    pub pump_modes: BTreeMap<String, LightingConfig>,
    pub dynamic_rgb: Option<DynamicRgbConfig>,
    pub dynamic_rgb_profiles: DynamicRgbProfiles,
}

/// Map every mode and every source. Incomplete/ambiguous binding maps fail before
/// any result is returned. Host discovery and eventual target admission are separate.
pub fn prepare(
    source: &LightingInspection,
    bindings: &SensorBindings,
) -> Result<PreparedLighting, Error> {
    if bindings.len() != ROLES.len() {
        return Err(Error::Invalid(
            "all six lighting sensor roles require explicit bindings",
        ));
    }
    if source.fan.len() != 6
        || source.pump.len() != 16
        || !source.fan.contains_key(&source.selected_fan_mode)
        || !source.pump.contains_key(&source.selected_pump_mode)
    {
        return Err(Error::Invalid("incomplete lighting banks or selection"));
    }
    let mut identities = BTreeSet::new();
    for &(role, unit) in ROLES {
        let binding = bindings
            .get(role)
            .ok_or(Error::Invalid("missing lighting sensor role binding"))?;
        if binding.unit != unit
            || binding.sensor_id.trim().is_empty()
            || binding.sensor_id.len() > 1024
            || binding.sensor_id.chars().any(char::is_control)
            || !identities.insert(&binding.sensor_id)
        {
            return Err(Error::Invalid(
                "lighting sensor binding has wrong units, invalid ID or duplicate role identity",
            ));
        }
    }
    let topology = FanLightingTopology {
        sync_to_pump: source.fan_sync_to_pump,
        size: match source.fan_size_mm {
            280 => FanLedSize::Mm280,
            360 => FanLedSize::Mm360,
            _ => return Err(Error::Invalid("unsupported fan size")),
        },
        manual_led_count: source.fan_loop_number,
    };
    topology.validate()?;
    let selected_fan_mode = linux_mode(&source.selected_fan_mode)?.to_owned();
    let selected_pump_mode = linux_mode(&source.selected_pump_mode)?.to_owned();
    let fan_modes = source
        .fan
        .iter()
        .map(|(name, parameters)| {
            let mut config = static_config(name, parameters, LightingKind::Fan)?;
            config.fan_topology = Some(topology.clone());
            Ok((config.mode.clone(), config))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let mut pump_modes = BTreeMap::new();
    let mut profiles = DynamicRgbProfiles::default();
    let mut active = None;
    for (name, profile) in &source.pump {
        let mode = linux_mode(name)?;
        let mut base = static_config(name, &profile.r#static, LightingKind::Pump)?;
        base.dynamic_mode = Some(profile.is_dynamic_mode);
        pump_modes.insert(mode.into(), base);
        let selected_role = ROLES
            .get(usize::from(profile.sensor_type).wrapping_sub(1))
            .ok_or(Error::Invalid("unknown selected lighting sensor role"))?
            .0;
        if profile.dynamic_settings.len() != 6 {
            return Err(Error::Invalid("incomplete dynamic source settings"));
        }
        let (_, speed_supported) =
            lconnect_ipc::dynamic_rgb_controls(mode).ok_or(Error::Invalid("unknown pump mode"))?;
        let mut configs = Vec::new();
        let mut selected_config = None;
        for (role, settings) in &profile.dynamic_settings {
            let binding = bindings
                .get(role)
                .ok_or(Error::Invalid("unknown lighting sensor role"))?;
            let speed_pair = match (settings.low.speed, settings.high.speed) {
                (i32::MIN, i32::MIN) if !speed_supported => (None, None),
                (low, high)
                    if speed_supported && (0..=100).contains(&low) && (0..=100).contains(&high) =>
                {
                    (Some(low as u8), Some(high as u8))
                }
                _ if !speed_supported => (None, None),
                _ => {
                    return Err(Error::Invalid(
                        "dynamic speed sentinels cannot be represented losslessly",
                    ))
                }
            };
            let config = DynamicRgbConfig {
                brightness_percent: Some(percent(settings.high.brightness)?),
                direction: Some(direction(settings.high.direction)?.into()),
                sensor_id: binding.sensor_id.clone(),
                unit: binding.unit,
                low_value: u16::try_from(settings.low_value)
                    .map_err(|_| Error::Invalid("negative dynamic lighting threshold"))?,
                high_value: u16::try_from(settings.high_value).map_err(|_| {
                    Error::Invalid("dynamic lighting threshold exceeds Linux limits")
                })?,
                low_colors: colors(&settings.low.colors),
                high_colors: colors(&settings.high.colors),
                low_speed_percent: speed_pair.0,
                high_speed_percent: speed_pair.1,
            };
            config.validate_mode(mode)?;
            if role == selected_role {
                if mode == selected_pump_mode && profile.is_dynamic_mode {
                    active = Some(config.clone());
                }
                selected_config = Some(config);
            } else {
                configs.push(config);
            }
        }
        // The final binding is the existing Linux registry's selected source.
        configs.push(selected_config.ok_or(Error::Invalid("missing selected lighting source"))?);
        profiles.modes.push(DynamicRgbProfile {
            enabled: profile.is_dynamic_mode,
            mode: mode.into(),
            bindings: configs,
        });
    }
    profiles.validate()?;
    Ok(PreparedLighting {
        fan_topology: topology,
        selected_fan_mode,
        selected_pump_mode,
        fan_modes,
        pump_modes,
        dynamic_rgb: active,
        dynamic_rgb_profiles: profiles,
    })
}
fn linux_mode(name: &str) -> Result<&'static str, Error> {
    MODES
        .iter()
        .find(|(source, _)| *source == name)
        .map(|(_, mode)| *mode)
        .ok_or(Error::Invalid("unknown lighting mode"))
}
fn percent(value: i32) -> Result<u8, Error> {
    match value {
        i32::MIN => Ok(0),
        0..=100 => Ok(value as u8),
        _ => Err(Error::Invalid("invalid lighting percentage")),
    }
}
fn direction(value: u8) -> Result<&'static str, Error> {
    match value {
        0 => Ok("forward"),
        1 => Ok("reverse"),
        _ => Err(Error::Invalid(
            "source lighting direction cannot be represented by Linux",
        )),
    }
}
fn colors(values: &[[u8; 3]]) -> Vec<Rgb> {
    values.iter().map(|&[r, g, b]| Rgb { r, g, b }).collect()
}
fn static_config(
    name: &str,
    parameters: &Parameters,
    kind: LightingKind,
) -> Result<LightingConfig, Error> {
    let mode = linux_mode(name)?;
    let controls = lighting_catalog(kind)
        .into_iter()
        .find(|c| c.mode == mode)
        .ok_or(Error::Invalid("mode does not belong to lighting channel"))?;
    if parameters.colors.len() != controls.color_count {
        return Err(Error::Invalid("lighting palette size mismatch"));
    }
    let mut palette = colors(&parameters.colors).into_iter();
    let mut config = LightingConfig::new(mode, palette.next().unwrap_or(Rgb { r: 0, g: 0, b: 0 }));
    config.additional_colors = palette.collect();
    config.speed_percent = percent(parameters.speed)?;
    config.brightness_percent = percent(parameters.brightness)?;
    config.direction = direction(parameters.direction)?.into();
    if !controls.direction && config.direction != "forward" {
        return Err(Error::Invalid(
            "direction is unsupported for this Linux mode",
        ));
    }
    if kind == LightingKind::Pump {
        config.scope = Some("inner".into());
    }
    Ok(config)
}
