//! Static Snow3D page parameters from Galahad II LCDV2 Windows2.1.28.
use crate::{IpcError, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snow3dEffect {
    pub intensity: u8,
    pub seed: u32,
}
impl Default for Snow3dEffect {
    fn default() -> Self {
        Self {
            intensity: 1,
            seed: 42,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snow3dParameters {
    pub exported_speed: u8,
    pub speed: u8,
    pub particle_level: u16,
    pub particles: u16,
}
impl Snow3dEffect {
    pub fn validate(&self) -> Result<()> {
        if !(1..=3).contains(&self.intensity) {
            return Err(IpcError::InvalidArgument(
                "Snow3D intensity must be 1..3".into(),
            ));
        }
        Ok(())
    }
    pub fn parameters(&self) -> Result<Snow3dParameters> {
        self.validate()?;
        let (speed, particle_level, particles) = match self.intensity {
            1 => (45, 300, 692),
            2 => (55, 450, 1037),
            _ => (75, 650, 1498),
        };
        Ok(Snow3dParameters {
            exported_speed: self.intensity * 10 + 5,
            speed,
            particle_level,
            particles,
        })
    }
}
