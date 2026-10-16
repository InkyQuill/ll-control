//! Fireworks settings: six independent static/low/high colors per source.
use crate::{
    effects::validate_effect_sensor,
    meteor::{blend_hsb, native_palette_weight},
    rgb_to_hsb, valid_sensor_value, EffectSensor, IpcError, Result, SensorReading,
};
use serde::{Deserialize, Serialize};
pub const FIREWORKS_COLORS: [[u8; 3]; 6] = [
    [0, 255, 0],
    [0, 0, 255],
    [128, 0, 255],
    [255, 0, 65],
    [255, 215, 0],
    [130, 173, 237],
];
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworksSensor {
    pub binding: EffectSensor,
    pub low_colors: [[u8; 3]; 6],
    pub high_colors: [[u8; 3]; 6],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FireworksEffect {
    pub intensity: u8,
    pub colors: [[u8; 3]; 6],
    pub dynamic: bool,
    pub sensor: Option<FireworksSensor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_sensors: Vec<FireworksSensor>,
    pub seed: u32,
}
impl Default for FireworksEffect {
    fn default() -> Self {
        Self {
            intensity: 1,
            colors: FIREWORKS_COLORS,
            dynamic: false,
            sensor: None,
            retained_sensors: Vec::new(),
            seed: 42,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireworksParameters {
    /// Native probability factor .1/.2/.3 (launch probability=.7×factor/frame).
    pub rate: f64,
    pub hsb: [[f64; 3]; 6],
    pub unavailable: bool,
}
impl FireworksEffect {
    pub fn validate(&self) -> Result<()> {
        let invalid = |message: &str| IpcError::InvalidArgument(message.into());
        if !(1..=3).contains(&self.intensity) {
            return Err(invalid("fireworks intensity must be 1..3"));
        }
        if self.dynamic && self.sensor.is_none() {
            return Err(invalid("dynamic fireworks requires a sensor binding"));
        }
        if self.retained_sensors.len() > 63 {
            return Err(invalid("fireworks supports at most 64 source settings"));
        }
        let mut identities = Vec::new();
        for source in self.sensor.iter().chain(&self.retained_sensors) {
            validate_effect_sensor(&source.binding)?;
            let identity = (&source.binding.sensor_id, source.binding.unit);
            if identities.contains(&identity) {
                return Err(invalid("duplicate fireworks source settings"));
            }
            identities.push(identity);
        }
        Ok(())
    }
    pub fn parameters(&self, readings: &[SensorReading]) -> Result<FireworksParameters> {
        self.validate()?;
        let fallback = |unavailable| FireworksParameters {
            rate: f64::from(self.intensity) * 0.1,
            hsb: self.colors.map(rgb_to_hsb),
            unavailable,
        };
        if !self.dynamic {
            return Ok(fallback(false));
        }
        let source = self.sensor.as_ref().expect("validated dynamic source");
        let binding = &source.binding;
        let Some(value) = valid_sensor_value(readings, &binding.sensor_id, binding.unit) else {
            return Ok(fallback(true));
        };
        let weight = native_palette_weight(binding, value);
        let rate = if weight == 0 {
            0.1
        } else if weight == 10 {
            0.3
        } else {
            0.2
        };
        Ok(FireworksParameters {
            rate,
            hsb: blend_hsb(source.low_colors, source.high_colors, weight),
            unavailable: false,
        })
    }
}
