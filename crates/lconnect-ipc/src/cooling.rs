//! Cooling policies and LCDV2 pump presets. RPM targets use a reference mapping,
//! not closed-loop feedback from measured pump RPM.
use crate::{IpcError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoolingTarget {
    Fan,
    Pump,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurvePoint {
    pub celsius: f64,
    pub percent: u8,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpmCurvePoint {
    pub celsius: i16,
    pub rpm: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PumpPreset {
    Quiet,
    Performance,
}
impl PumpPreset {
    /// LCDV2 2.1.28 Galahad2VisionProfile.cs:782-836. Windows binds CPU;
    /// Linux requires an explicit stable temperature binding from the caller.
    pub fn policy(
        self,
        sensor_id: String,
        hysteresis_c: f64,
        fallback_percent: u8,
    ) -> CoolingPolicy {
        let pairs = match self {
            Self::Quiet => [
                (30, 2000),
                (45, 2200),
                (60, 2400),
                (70, 2600),
                (80, 2800),
                (85, 3000),
            ],
            Self::Performance => [
                (30, 2200),
                (40, 2500),
                (47, 2750),
                (53, 2900),
                (60, 3000),
                (70, 3600),
            ],
        };
        CoolingPolicy::CurveRpm {
            sensor_id,
            points: pairs
                .into_iter()
                .map(|(celsius, rpm)| RpmCurvePoint { celsius, rpm })
                .collect(),
            hysteresis_c,
            fallback_percent,
        }
    }
}

/// LCDV2 Windows2.1.28 Galahad2VisionProfile.cs:206-239.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanPreset {
    Quiet,
    Standard,
    High,
    Full,
}
impl FanPreset {
    pub fn policy(
        self,
        sensor_id: String,
        hysteresis_c: f64,
        fallback_percent: u8,
    ) -> CoolingPolicy {
        let pairs = match self {
            Self::Quiet => [(25, 20), (45, 45), (65, 52), (80, 75), (90, 100)],
            Self::Standard => [(25, 20), (40, 50), (55, 62), (70, 75), (90, 100)],
            Self::High => [(25, 45), (35, 55), (50, 70), (70, 85), (85, 100)],
            Self::Full => [(25, 100), (40, 100), (55, 100), (70, 100), (90, 100)],
        };
        CoolingPolicy::FanCurve {
            sensor_id,
            // Windows RPMPhaseInfo.ToRPMConfigPhases(false) surrounds the five
            // editable knots with 0C/MinSpeed and 100C/MaxSpeed. In particular,
            // low CPU temperatures interpolate from 10%, even in Full mode.
            points: std::iter::once((0, 10))
                .chain(pairs)
                .chain(std::iter::once((100, 100)))
                .map(|(c, p)| CurvePoint {
                    celsius: f64::from(c),
                    percent: p,
                })
                .collect(),
            hysteresis_c,
            fallback_percent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoolingPolicy {
    /// Fan-only reference arithmetic: integer Celsius, truncated interpolated duty.
    FanCurve {
        sensor_id: String,
        points: Vec<CurvePoint>,
        hysteresis_c: f64,
        fallback_percent: u8,
    },
    FixedRpm {
        rpm: u16,
    },
    CurveRpm {
        sensor_id: String,
        points: Vec<RpmCurvePoint>,
        hysteresis_c: f64,
        fallback_percent: u8,
    },
    Fixed {
        percent: u8,
    },
    Motherboard {
        percent: u8,
    },
    Curve {
        sensor_id: String,
        points: Vec<CurvePoint>,
        hysteresis_c: f64,
        fallback_percent: u8,
    },
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoolingConfig {
    pub fan: Option<CoolingPolicy>,
    pub pump: Option<CoolingPolicy>,
    /// Stored-mode identity of the requested snapshot, not measured hardware state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fan_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pump_mode: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoolingChannelState {
    /// Absent for manual PWM or a policy applied independently of a stored mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_id: Option<String>,
    pub requested: Option<CoolingPolicy>,
    pub applied_percent: Option<u8>,
    pub motherboard: bool,
    pub fallback_active: bool,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoolingState {
    pub fan: CoolingChannelState,
    pub pump: CoolingChannelState,
}

impl CoolingPolicy {
    pub fn sensor_id(&self) -> Option<&str> {
        match self {
            Self::Curve { sensor_id, .. }
            | Self::CurveRpm { sensor_id, .. }
            | Self::FanCurve { sensor_id, .. } => Some(sensor_id),
            _ => None,
        }
    }
    pub fn validate(&self, target: CoolingTarget) -> Result<()> {
        // LCDV2 Windows profile fan minimum10%; pump conversion starts800RPM→20%.
        // Reference 2.1.28: Galahad2VisionProfile.cs:189; service controller:143.
        let min = if target == CoolingTarget::Pump {
            20
        } else {
            10
        };
        let duty = |p: u8| {
            if (min..=100).contains(&p) {
                Ok(())
            } else {
                Err(IpcError::InvalidArgument(format!(
                    "duty must be {min}..=100"
                )))
            }
        };
        let rpm = |rpm: u16| {
            if target == CoolingTarget::Pump && (800..=3600).contains(&rpm) {
                Ok(())
            } else {
                Err(IpcError::InvalidArgument(
                    "RPM policies are pump-only; RPM must be 800..=3600".into(),
                ))
            }
        };
        match self {
            Self::FixedRpm { rpm: value } => rpm(*value),
            Self::CurveRpm {
                sensor_id,
                points,
                hysteresis_c,
                fallback_percent,
            } => {
                if sensor_id.is_empty()
                    || sensor_id.len() > 4096
                    || !(2..=16).contains(&points.len())
                    || !hysteresis_c.is_finite()
                    || !(0.0..=10.0).contains(hysteresis_c)
                {
                    return Err(IpcError::InvalidArgument(
                        "invalid RPM curve binding, points or hysteresis".into(),
                    ));
                }
                duty(*fallback_percent)?;
                for point in points {
                    rpm(point.rpm)?;
                    if !(-50..=150).contains(&point.celsius) {
                        return Err(IpcError::InvalidArgument(
                            "curve temperature out of range".into(),
                        ));
                    }
                }
                if points
                    .windows(2)
                    .any(|p| p[0].celsius >= p[1].celsius || p[0].rpm > p[1].rpm)
                {
                    return Err(IpcError::InvalidArgument(
                        "curve temperatures must increase and RPM must not decrease".into(),
                    ));
                }
                Ok(())
            }
            Self::Fixed { percent } => duty(*percent),
            Self::Motherboard { percent } => {
                if target != CoolingTarget::Pump {
                    return Err(IpcError::InvalidArgument(
                        "motherboard mode is pump-only".into(),
                    ));
                }
                duty(*percent)
            }
            Self::Curve {
                sensor_id,
                points,
                hysteresis_c,
                fallback_percent,
            }
            | Self::FanCurve {
                sensor_id,
                points,
                hysteresis_c,
                fallback_percent,
            } => {
                if matches!(self, Self::FanCurve { .. })
                    && (target != CoolingTarget::Fan
                        || points.iter().any(|p| p.celsius.fract() != 0.0))
                {
                    return Err(IpcError::InvalidArgument(
                        "fan_curve requires fan target and integer Celsius points".into(),
                    ));
                }
                if sensor_id.is_empty()
                    || sensor_id.len() > 4096
                    || !(2..=16).contains(&points.len())
                    || !hysteresis_c.is_finite()
                    || !(0.0..=10.0).contains(hysteresis_c)
                {
                    return Err(IpcError::InvalidArgument(
                        "invalid curve binding, points or hysteresis".into(),
                    ));
                }
                duty(*fallback_percent)?;
                for point in points {
                    duty(point.percent)?;
                    if !point.celsius.is_finite() || !(-50.0..=150.0).contains(&point.celsius) {
                        return Err(IpcError::InvalidArgument(
                            "curve temperature out of range".into(),
                        ));
                    }
                }
                if points
                    .windows(2)
                    .any(|p| p[0].celsius >= p[1].celsius || p[0].percent > p[1].percent)
                {
                    return Err(IpcError::InvalidArgument(
                        "curve temperatures must increase and duty must not decrease".into(),
                    ));
                }
                Ok(())
            }
        }
    }
}
