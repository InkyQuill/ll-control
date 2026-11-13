//! Per-device thermal RGB alert configuration and observable state.
use crate::{IpcError, Result, Rgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalRule {
    pub sensor_id: String,
    pub threshold_c: f64,
    pub color: Rgb,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThermalConfig {
    pub cpu: Option<ThermalRule>,
    pub gpu: Option<ThermalRule>,
}
impl ThermalConfig {
    pub fn enabled(&self) -> bool {
        self.cpu.is_some() || self.gpu.is_some()
    }
    pub fn validate(&self) -> Result<()> {
        for r in [&self.cpu, &self.gpu].into_iter().flatten() {
            if r.sensor_id.is_empty()
                || r.sensor_id.len() > 4096
                || !r.threshold_c.is_finite()
                || !(0.0..=150.0).contains(&r.threshold_c)
            {
                return Err(IpcError::InvalidArgument(
                    "thermal rule requires a sensor ID and finite threshold in 0..=150 Celsius"
                        .into(),
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalSource {
    Cpu,
    Gpu,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThermalLayer {
    Base,
    Alert { color: Rgb },
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThermalZoneState {
    /// Known prefix covered by a direct-color base/alert; None for mode bases.
    pub direct_led_count: Option<usize>,
    pub applied: Option<ThermalLayer>,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThermalState {
    pub requested: ThermalConfig,
    pub active_source: Option<ThermalSource>,
    pub degraded_sources: Vec<ThermalSource>,
    pub grace_remaining_ms: u64,
    pub blocked_reason: Option<String>,
    pub fan: ThermalZoneState,
    pub pump: ThermalZoneState,
}
