//! MeteorShower settings and versioned Windows 2.1.28 speed/HSB mapping.
use crate::{
    effects::validate_effect_sensor, valid_sensor_value, EffectSensor, IpcError, Result,
    SensorReading,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeteorSensor {
    pub binding: EffectSensor,
    pub low_colors: [[u8; 3]; 2],
    pub high_colors: [[u8; 3]; 2],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeteorEffect {
    /// Windows slider 1..3; export sends slider+1 to the scene.
    pub speed: u8,
    pub colors: [[u8; 3]; 2],
    pub dynamic: bool,
    pub sensor: Option<MeteorSensor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_sensors: Vec<MeteorSensor>,
    pub seed: u32,
}
impl Default for MeteorEffect {
    fn default() -> Self {
        Self {
            speed: 2,
            colors: [[0, 255, 0], [0, 0, 255]],
            dynamic: false,
            sensor: None,
            retained_sensors: Vec::new(),
            seed: 42,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeteorParameters {
    pub speed: u8,
    /// Hue0..360, saturation/brightness0..100, rounded as in native rgbToHsb.
    pub hsb: [[f64; 3]; 2],
    pub unavailable: bool,
}
impl MeteorEffect {
    pub fn validate(&self) -> Result<()> {
        let invalid = |message: &str| IpcError::InvalidArgument(message.into());
        if !(1..=3).contains(&self.speed) {
            return Err(invalid("meteor speed must be 1..3"));
        }
        if self.dynamic && self.sensor.is_none() {
            return Err(invalid("dynamic meteor requires a sensor binding"));
        }
        if self.retained_sensors.len() > 63 {
            return Err(invalid("meteor supports at most 64 source settings"));
        }
        let mut identities = Vec::new();
        for sensor in self.sensor.iter().chain(&self.retained_sensors) {
            validate_effect_sensor(&sensor.binding)?;
            let identity = (&sensor.binding.sensor_id, sensor.binding.unit);
            if identities.contains(&identity) {
                return Err(invalid("duplicate meteor sensor setting"));
            }
            identities.push(identity);
        }
        Ok(())
    }
    pub fn parameters(&self, readings: &[SensorReading]) -> Result<MeteorParameters> {
        self.validate()?;
        let fallback = |unavailable| MeteorParameters {
            speed: self.speed + 1,
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
        let speed = if weight == 0 {
            2
        } else if weight == 10 {
            4
        } else {
            3
        };
        let hsb = blend_hsb(source.low_colors, source.high_colors, weight);
        Ok(MeteorParameters {
            speed,
            hsb,
            unavailable: false,
        })
    }
}
/// Native uses ceil on all channels and a direct hue blend (no shortest-arc wrap).
pub fn rgb_to_hsb(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|channel| f64::from(channel) / 255.0);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let sector = if delta == 0.0 {
        0.0
    } else if max == r {
        (g - b) / delta
    } else if max == g {
        2.0 + (b - r) / delta
    } else {
        4.0 + (r - g) / delta
    };
    let hue = 60.0 * if sector < 0.0 { sector + 6.0 } else { sector };
    [
        hue.ceil(),
        if max == 0.0 {
            0.0
        } else {
            (delta / max * 100.0).ceil()
        },
        (max * 100.0).ceil(),
    ]
}

// These two native effects share nine ceil-sized bands and direct HSB blending.
// Equal bounds poison native colors at equality; retain finite branches and
// choose the low palette at equality, never passing NaN into a scene.
pub(crate) fn native_palette_weight(binding: &EffectSensor, value: f64) -> u8 {
    let band = if binding.low == binding.high {
        if value > f64::from(binding.high) {
            9.0
        } else {
            0.0
        }
    } else {
        let step = ((f64::from(binding.high) - f64::from(binding.low)) / 9.0).ceil();
        ((value - f64::from(binding.low)) / step).trunc()
    };
    if band <= 3.0 {
        0
    } else if band >= 7.0 {
        10
    } else {
        band as u8
    }
}
pub(crate) fn blend_hsb<const N: usize>(
    low: [[u8; 3]; N],
    high: [[u8; 3]; N],
    weight: u8,
) -> [[f64; 3]; N] {
    let low = low.map(rgb_to_hsb);
    let high = high.map(rgb_to_hsb);
    if weight == 0 {
        return low;
    }
    if weight == 10 {
        return high;
    }
    let mut colors = [[0.0; 3]; N];
    let weight = f64::from(weight);
    for (index, color) in colors.iter_mut().enumerate() {
        for (channel, value) in color.iter_mut().enumerate() {
            *value = (weight * high[index][channel] + (10.0 - weight) * low[index][channel]) / 10.0;
        }
    }
    colors
}
