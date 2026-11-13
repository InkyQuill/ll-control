//! Persisted default sensor choices. Explicit theme/policy bindings stay independent.
use crate::{IpcError, Result, SensorReading, SensorStatus, SensorUnit};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuSensorDefaults {
    pub temperature: Option<String>,
    pub clock: Option<String>,
    pub load: Option<String>,
    pub power: Option<String>,
    pub fan: Option<String>,
    pub voltage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuSensorDefaults {
    /// Provider-owned physical identity, never a display name or enumeration index.
    pub device_id: String,
    pub temperature: Option<String>,
    pub clock: Option<String>,
    pub load: Option<String>,
    pub power: Option<String>,
    pub fan: Option<String>,
    pub voltage: Option<String>,
    pub memory_used: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorDefaults {
    pub version: u32,
    pub revision: u64,
    pub cpu: CpuSensorDefaults,
    pub gpu: Option<GpuSensorDefaults>,
}
impl Default for SensorDefaults {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            cpu: CpuSensorDefaults::default(),
            gpu: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultSensorRole {
    CpuTemperature,
    CpuClock,
    CpuLoad,
    CpuPower,
    CpuFan,
    CpuVoltage,
    GpuTemperature,
    GpuClock,
    GpuLoad,
    GpuPower,
    GpuFan,
    GpuVoltage,
    GpuMemoryUsed,
}
impl DefaultSensorRole {
    pub const ALL: [Self; 13] = [
        Self::CpuTemperature,
        Self::CpuClock,
        Self::CpuLoad,
        Self::CpuPower,
        Self::CpuFan,
        Self::CpuVoltage,
        Self::GpuTemperature,
        Self::GpuClock,
        Self::GpuLoad,
        Self::GpuPower,
        Self::GpuFan,
        Self::GpuVoltage,
        Self::GpuMemoryUsed,
    ];
    pub fn unit(self) -> SensorUnit {
        match self {
            Self::CpuTemperature | Self::GpuTemperature => SensorUnit::Celsius,
            Self::CpuClock | Self::GpuClock => SensorUnit::Hertz,
            Self::CpuLoad | Self::GpuLoad => SensorUnit::Percent,
            Self::CpuPower | Self::GpuPower => SensorUnit::Watts,
            Self::CpuFan | Self::GpuFan => SensorUnit::Rpm,
            Self::CpuVoltage | Self::GpuVoltage => SensorUnit::Volts,
            Self::GpuMemoryUsed => SensorUnit::Bytes,
        }
    }
}
impl SensorDefaults {
    /// Validate saved structure without requiring currently connected hardware.
    /// Apply must additionally verify provider-owned CPU/GPU membership.
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(IpcError::InvalidArgument(
                "unsupported sensor defaults version".into(),
            ));
        }
        if let Some(gpu) = &self.gpu {
            validate_id(&gpu.device_id)?;
        }
        for role in DefaultSensorRole::ALL {
            if let Some(id) = self.binding(role) {
                validate_id(id)?;
            }
        }
        Ok(())
    }

    pub fn binding(&self, role: DefaultSensorRole) -> Option<&str> {
        use DefaultSensorRole::*;
        let id = match role {
            CpuTemperature => &self.cpu.temperature,
            CpuClock => &self.cpu.clock,
            CpuLoad => &self.cpu.load,
            CpuPower => &self.cpu.power,
            CpuFan => &self.cpu.fan,
            CpuVoltage => &self.cpu.voltage,
            GpuTemperature => &self.gpu.as_ref()?.temperature,
            GpuClock => &self.gpu.as_ref()?.clock,
            GpuLoad => &self.gpu.as_ref()?.load,
            GpuPower => &self.gpu.as_ref()?.power,
            GpuFan => &self.gpu.as_ref()?.fan,
            GpuVoltage => &self.gpu.as_ref()?.voltage,
            GpuMemoryUsed => &self.gpu.as_ref()?.memory_used,
        };
        id.as_deref()
    }

    /// Resolve only the saved ID with a usable sample. Never choose a replacement.
    /// Callers supply a freshness-checked catalog; missing age is also unavailable.
    pub fn resolve<'a>(
        &self,
        role: DefaultSensorRole,
        catalog: &'a [SensorReading],
    ) -> Option<&'a SensorReading> {
        if self.validate().is_err() {
            return None;
        }
        let id = self.binding(role)?;
        let mut matches = catalog.iter().filter(|reading| reading.id == id);
        let reading = matches.next()?;
        if matches.next().is_some()
            || reading.unit != role.unit()
            || reading.status != SensorStatus::Valid
            || reading.age_ms.is_none_or(|age| age > 3000)
            || !reading.value.is_some_and(f64::is_finite)
        {
            return None;
        }
        Some(reading)
    }
}
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 1024 || id.chars().any(char::is_control) {
        return Err(IpcError::InvalidArgument(
            "invalid sensor defaults identity".into(),
        ));
    }
    Ok(())
}

/// A daemon-classified choice, not metadata supplied by the editing client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorDefaultCandidate {
    pub role: DefaultSensorRole,
    /// None for CPU roles; stable physical/provider identity for GPU roles.
    pub gpu_id: Option<String>,
    pub reading: SensorReading,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultSensorValue {
    pub role: DefaultSensorRole,
    pub sensor_id: Option<String>,
    /// None when not selected, missing, ambiguous, wrong-unit, invalid or stale.
    pub reading: Option<SensorReading>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorDefaultsState {
    pub config: SensorDefaults,
    /// Read-only proposal for an explicit reset-to-defaults draft action.
    pub recommended: SensorDefaults,
    pub candidates: Vec<SensorDefaultCandidate>,
    pub values: Vec<DefaultSensorValue>,
}
