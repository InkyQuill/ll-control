//! Sensor-driven adjustment of the selected pump lighting mode (RGB-004).
use crate::{IpcError, Result, Rgb, SensorUnit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicRgbConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness_percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub sensor_id: String,
    pub unit: SensorUnit,
    pub low_value: u16,
    pub high_value: u16,
    pub low_colors: Vec<Rgb>,
    pub high_colors: Vec<Rgb>,
    pub low_speed_percent: Option<u8>,
    pub high_speed_percent: Option<u8>,
}
impl DynamicRgbConfig {
    pub fn validate(&self) -> Result<()> {
        let max = match self.unit {
            SensorUnit::Celsius | SensorUnit::Percent => 100,
            SensorUnit::Rpm => 3600,
            _ => {
                return Err(IpcError::InvalidArgument(
                    "dynamic RGB requires Celsius, percent or pump RPM".into(),
                ))
            }
        };
        if self.brightness_percent.is_some_and(|v| v > 100)
            || self
                .direction
                .as_deref()
                .is_some_and(|v| !matches!(v, "forward" | "reverse"))
            || self.sensor_id.trim().is_empty()
            || self.sensor_id.len() > 1024
            || self.sensor_id.contains('\0')
            || self.low_value > self.high_value
            || self.high_value > max
            || self.low_colors.len() > 4
            || self.low_colors.len() != self.high_colors.len()
            || self.low_speed_percent.is_some() != self.high_speed_percent.is_some()
            || self.low_speed_percent.is_some_and(|v| v > 100)
            || self.high_speed_percent.is_some_and(|v| v > 100)
        {
            return Err(IpcError::InvalidArgument(
                "invalid dynamic RGB binding, range, colors or speed pair".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRgbSample {
    pub value: i32,
    pub colors: Vec<Rgb>,
    pub speed_percent: Option<u8>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DynamicRgbState {
    #[serde(default)]
    pub profiles: DynamicRgbProfiles,
    #[serde(default)]
    pub mode_defaults: Option<crate::LightingConfig>,
    #[serde(default)]
    pub direction_supported: bool,
    pub base_mode: Option<String>,
    pub color_count: Option<usize>,
    pub speed_supported: bool,
    pub requested: Option<DynamicRgbConfig>,
    pub applied: Option<DynamicRgbSample>,
    pub degraded: bool,
    pub suppressed_by_thermal: bool,
    pub last_error: Option<String>,
}

/// Mode-specific dynamic controls from Galahad2VisionProfile.cs:430–659.
pub fn dynamic_rgb_controls(mode: &str) -> Option<(usize, bool)> {
    let mode = crate::canonical_lighting_mode(mode);
    Some(match mode {
        "rainbow" | "rainbow_morph" | "colorful_starry_night" | "colors_morph" => (0, true),
        "static" => (1, false),
        "breathing" | "static_starry_night" | "voice" | "burst" => (1, true),
        "runway" | "transmit" => (2, true),
        "meteor" | "ticker_tape" | "fluctuation" | "big_bang" | "bounce" => (4, true),
        _ => return None,
    })
}

/// Static palette size in the versioned LCDV2 fan/pump UI catalog.
/// Legacy single-color callers can omit the remaining slots.
pub fn lighting_color_count(kind: crate::LightingKind, mode: &str) -> Option<usize> {
    let mode = if mode == "static_color" {
        "static"
    } else {
        mode
    };
    match kind {
        crate::LightingKind::Pump => dynamic_rgb_controls(mode).map(|(count, _)| count),
        crate::LightingKind::Fan
            if matches!(
                mode,
                "rainbow" | "rainbow_morph" | "static" | "breathing" | "runway" | "meteor"
            ) =>
        {
            dynamic_rgb_controls(mode).map(|(count, _)| count)
        }
        _ => None,
    }
}

/// Persisted source settings per pump mode. The last binding is last applied;
/// disabling an active policy does not delete these editing values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DynamicRgbProfiles {
    pub modes: Vec<DynamicRgbProfile>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicRgbProfile {
    /// Retained choice for this mode, independent of the currently active mode.
    #[serde(default)]
    pub enabled: bool,
    pub mode: String,
    pub bindings: Vec<DynamicRgbConfig>,
}
impl DynamicRgbConfig {
    pub fn validate_mode(&self, mode: &str) -> Result<()> {
        self.validate()?;
        let (count, speed) = dynamic_rgb_controls(mode)
            .ok_or_else(|| IpcError::InvalidArgument("unsupported dynamic pump mode".into()))?;
        if self.low_colors.len() != count || self.low_speed_percent.is_some() != speed {
            return Err(IpcError::InvalidArgument(format!(
                "mode {mode} requires {count} colors and speed={speed}"
            )));
        }
        let direction = matches!(
            crate::canonical_lighting_mode(mode),
            "rainbow" | "meteor" | "ticker_tape" | "transmit"
        );
        if !direction && self.direction.as_deref().is_some_and(|d| d != "forward") {
            return Err(IpcError::InvalidArgument(
                "direction is not supported by this pump mode".into(),
            ));
        }
        Ok(())
    }
}
impl DynamicRgbProfiles {
    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }
    pub fn validate(&self) -> Result<()> {
        if self.modes.len() > 16 {
            return Err(IpcError::InvalidArgument(
                "too many dynamic pump modes".into(),
            ));
        }
        let mut modes = std::collections::HashSet::new();
        for profile in &self.modes {
            if profile.mode != crate::canonical_lighting_mode(&profile.mode)
                || !modes.insert(&profile.mode)
                || dynamic_rgb_controls(&profile.mode).is_none()
                || profile.bindings.is_empty()
                || profile.bindings.len() > 64
            {
                return Err(IpcError::InvalidArgument(
                    "invalid dynamic mode profile".into(),
                ));
            }
            let mut sources = std::collections::HashSet::new();
            for binding in &profile.bindings {
                binding.validate_mode(&profile.mode)?;
                if !sources.insert((
                    &binding.sensor_id,
                    match binding.unit {
                        SensorUnit::Celsius => 0,
                        SensorUnit::Percent => 1,
                        SensorUnit::Rpm => 2,
                        _ => 3,
                    },
                )) {
                    return Err(IpcError::InvalidArgument(
                        "duplicate dynamic source identity/unit".into(),
                    ));
                }
            }
        }
        if serde_json::to_vec(self)
            .map_err(|e| IpcError::InvalidArgument(e.to_string()))?
            .len()
            > 256 * 1024
        {
            return Err(IpcError::InvalidArgument(
                "dynamic profiles exceed256KiB".into(),
            ));
        }
        Ok(())
    }
    pub fn selected(&self, mode: &str) -> Option<&DynamicRgbProfile> {
        let mode = crate::canonical_lighting_mode(mode);
        self.modes.iter().find(|p| p.mode == mode)
    }
    pub fn disable(&mut self, mode: &str) {
        let mode = crate::canonical_lighting_mode(mode);
        if let Some(profile) = self.modes.iter_mut().find(|p| p.mode == mode) {
            profile.enabled = false;
        }
    }
    pub fn remember(&mut self, mode: &str, config: DynamicRgbConfig) -> Result<()> {
        let mode = crate::canonical_lighting_mode(mode);
        config.validate_mode(mode)?;
        let mut next = self.clone();
        if let Some(profile) = next.modes.iter_mut().find(|p| p.mode == mode) {
            profile
                .bindings
                .retain(|p| p.sensor_id != config.sensor_id || p.unit != config.unit);
            profile.bindings.push(config);
            profile.enabled = true;
        } else {
            next.modes.push(DynamicRgbProfile {
                enabled: true,
                mode: mode.into(),
                bindings: vec![config],
            });
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}
