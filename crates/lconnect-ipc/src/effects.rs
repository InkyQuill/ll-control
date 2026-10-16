//! Parameters for owned procedural LCD weather scenes; no vendor assets.
use crate::{valid_sensor_value, IpcError, Result, SensorReading, SensorUnit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherKind {
    Rain,
    Snow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectSensor {
    pub sensor_id: String,
    pub unit: SensorUnit,
    pub low: i32,
    pub high: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherPreset {
    pub kind: WeatherKind,
    /// Windows static selector has three positions, retained in dynamic mode.
    pub intensity: u8,
    pub sensor: Option<EffectSensor>,
    /// Ranges for inactive sources, keyed by identity and native unit. The active
    /// binding is stored only in `sensor`, so edits cannot leave a stale duplicate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_sensors: Vec<EffectSensor>,
    #[serde(default)]
    pub dynamic: bool,
    /// Linux scene seed makes preview, restart and frame tests reproducible.
    pub seed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherEffect {
    pub kind: WeatherKind,
    /// Independent settings of the inactive weather page; never rendered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<WeatherPreset>,
    /// Windows static selector has three positions, retained in dynamic mode.
    pub intensity: u8,
    pub sensor: Option<EffectSensor>,
    /// Ranges for inactive sources, keyed by identity and native unit. The active
    /// binding is stored only in `sensor`, so edits cannot leave a stale duplicate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_sensors: Vec<EffectSensor>,
    #[serde(default)]
    pub dynamic: bool,
    /// Linux scene seed makes preview, restart and frame tests reproducible.
    pub seed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherParameters {
    /// Rain droplets rate or Snow speed, in the Windows effect's native scale.
    pub amount: u8,
    /// Dynamic source missing, stale or mismatched; static setting is the fallback.
    pub unavailable: bool,
}
impl WeatherEffect {
    pub fn validate(&self) -> Result<()> {
        let invalid = |message: &str| IpcError::InvalidArgument(message.into());
        if self.presets.len() > 1 {
            return Err(invalid("weather supports only one inactive page"));
        }
        for preset in &self.presets {
            if preset.kind == self.kind {
                return Err(invalid("active weather page cannot also be retained"));
            }
            validate_settings(
                preset.intensity,
                preset.dynamic,
                preset.sensor.as_ref(),
                &preset.retained_sensors,
            )?;
        }
        validate_settings(
            self.intensity,
            self.dynamic,
            self.sensor.as_ref(),
            &self.retained_sensors,
        )
    }
    /// Resolve native scale after validation, never replacing a missing source with zero.
    pub fn parameters(&self, readings: &[SensorReading]) -> Result<WeatherParameters> {
        self.validate()?;
        let static_amount = match self.kind {
            WeatherKind::Rain => [1, 2, 4][usize::from(self.intensity - 1)],
            WeatherKind::Snow => [10, 30, 60][usize::from(self.intensity - 1)],
        };
        if !self.dynamic {
            return Ok(WeatherParameters {
                amount: static_amount,
                unavailable: false,
            });
        }
        let Some(sensor) = &self.sensor else {
            return Ok(WeatherParameters {
                amount: static_amount,
                unavailable: false,
            });
        };
        let Some(value) = valid_sensor_value(readings, &sensor.sensor_id, sensor.unit) else {
            return Ok(WeatherParameters {
                amount: static_amount,
                unavailable: true,
            });
        };
        let amount = match self.kind {
            // Rain3D dynamic thresholds are strict; equality is the middle state.
            WeatherKind::Rain => {
                if value > f64::from(sensor.high) {
                    6
                } else if value < f64::from(sensor.low) {
                    1
                } else {
                    2
                }
            }
            WeatherKind::Snow => {
                // Five ceil-sized bands, truncation toward zero; native JS caps
                // its output at50 (the static high preset independently uses60).
                // Native JS yields NaN at value==low==high and poisons particle
                // velocity. Retain its finite branches and resolve that defect
                // to minimum speed; never pass non-finite motion to the renderer.
                let band = if sensor.low == sensor.high {
                    if value > f64::from(sensor.high) {
                        5.0
                    } else {
                        0.0
                    }
                } else {
                    let step = ((f64::from(sensor.high) - f64::from(sensor.low)) / 5.0).ceil();
                    ((value - f64::from(sensor.low)) / step).trunc()
                };
                if band <= 0.0 {
                    10
                } else if band >= 5.0 {
                    50
                } else {
                    10 * band as u8
                }
            }
        };
        Ok(WeatherParameters {
            amount,
            unavailable: false,
        })
    }
}

fn validate_settings(
    intensity: u8,
    dynamic: bool,
    sensor: Option<&EffectSensor>,
    retained_sensors: &[EffectSensor],
) -> Result<()> {
    let invalid = |message: &str| IpcError::InvalidArgument(message.into());
    if !(1..=3).contains(&intensity) {
        return Err(invalid("weather intensity must be 1..3"));
    }
    if dynamic && sensor.is_none() {
        return Err(invalid("dynamic weather requires a sensor binding"));
    }
    if retained_sensors.len() > 63 {
        return Err(invalid("weather supports at most 64 sensor ranges"));
    }
    let mut identities = Vec::new();
    for sensor in sensor.into_iter().chain(retained_sensors) {
        let identity = (&sensor.sensor_id, sensor.unit);
        if identities.contains(&identity) {
            return Err(invalid("duplicate weather sensor range"));
        }
        identities.push(identity);
        validate_effect_sensor(sensor)?;
    }
    Ok(())
}

// Shared identity/range contract for native temperature/load/RPM effects.
pub(crate) fn validate_effect_sensor(sensor: &EffectSensor) -> Result<()> {
    let invalid = |message: &str| IpcError::InvalidArgument(message.into());
    if sensor.sensor_id.trim().is_empty()
        || sensor.sensor_id.len() > 1024
        || sensor.sensor_id.contains('\0')
    {
        return Err(invalid("invalid weather sensor identity"));
    }
    if !matches!(
        sensor.unit,
        SensorUnit::Celsius | SensorUnit::Percent | SensorUnit::Rpm
    ) {
        return Err(invalid("weather sensor must be temperature, load or RPM"));
    }
    if sensor.low > sensor.high {
        return Err(invalid("weather low threshold must not exceed high"));
    }
    Ok(())
}
