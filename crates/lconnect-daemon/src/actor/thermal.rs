//! Source-derived priority: TemperatureAlertController.cs:250-324 (2.1.28).
//! Missing samples retain an active warning, instead of fabricating zero Celsius.
use lconnect_ipc::{Rgb, SensorReading, SensorStatus, SensorUnit, ThermalConfig, ThermalSource};
#[derive(Default, Clone)]
pub struct Evaluator {
    stack: Vec<ThermalSource>,
    pub degraded: Vec<ThermalSource>,
}
impl Evaluator {
    /// Retain warning history for the same bindings; disabled/rebound sources
    /// start fresh. Editing a color or reapplying a profile is not a trigger.
    pub fn reconfigure(&mut self, old: &ThermalConfig, new: &ThermalConfig) {
        self.stack.retain(|source| {
            let (a, b) = match source {
                ThermalSource::Cpu => (&old.cpu, &new.cpu),
                ThermalSource::Gpu => (&old.gpu, &new.gpu),
            };
            matches!((a,b), (Some(a),Some(b)) if a.sensor_id == b.sensor_id)
        });
    }

    pub fn evaluate(
        &mut self,
        config: &ThermalConfig,
        rows: &[SensorReading],
    ) -> Option<(ThermalSource, Rgb)> {
        self.degraded.clear();
        // CPU first means GPU wins simultaneous new triggers, as in Windows.
        for (source, rule) in [
            (ThermalSource::Cpu, &config.cpu),
            (ThermalSource::Gpu, &config.gpu),
        ] {
            let triggered = match rule {
                None => Some(false),
                Some(rule) => rows
                    .iter()
                    .find(|r| {
                        r.id == rule.sensor_id
                            && r.unit == SensorUnit::Celsius
                            && r.status == SensorStatus::Valid
                    })
                    .and_then(|r| r.value)
                    .filter(|v| v.is_finite())
                    .map(|v| v >= rule.threshold_c),
            };
            match triggered {
                Some(true) if !self.stack.contains(&source) => self.stack.push(source),
                Some(false) => self.stack.retain(|s| *s != source),
                None => self.degraded.push(source),
                _ => {}
            }
        }
        let source = *self.stack.last()?;
        let rule = match source {
            ThermalSource::Cpu => config.cpu.as_ref(),
            ThermalSource::Gpu => config.gpu.as_ref(),
        }?;
        Some((source, rule.color))
    }
}
