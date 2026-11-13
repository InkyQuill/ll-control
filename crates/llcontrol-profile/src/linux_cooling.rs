//! Typed cooling preparation shared by CLI and complete Windows control import.
use crate::cooling::{Bank, Control, CoolingInspection, SpeedUnit};
use crate::Error;
use llcontrol_ipc::{
    CoolingBankSource, CoolingConfig, CoolingModeBanks, CoolingModeProfile, CoolingPolicy,
    CoolingTarget, CurvePoint, RpmCurvePoint,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedCooling {
    pub cooling_config: CoolingConfig,
    pub cooling_banks: CoolingModeBanks,
    pub fan: Value,
    pub pump: Value,
}
pub fn prepare(
    source: &CoolingInspection,
    sensor: &str,
    hysteresis: f64,
    fallback: u8,
) -> Result<PreparedCooling, Error> {
    if sensor.trim().is_empty()
        || sensor.len() > 4096
        || sensor.chars().any(char::is_control)
        || !hysteresis.is_finite()
        || !(0.0..=10.0).contains(&hysteresis)
        || !(20..=100).contains(&fallback)
    {
        return Err(Error::Invalid(
            "invalid explicit cooling source, hysteresis or fallback",
        ));
    }
    let (fan, fan_modes) = policies(
        &source.fan,
        CoolingTarget::Fan,
        sensor,
        hysteresis,
        fallback,
    )?;
    let (pump, pump_modes) = policies(
        &source.pump,
        CoolingTarget::Pump,
        sensor,
        hysteresis,
        fallback,
    )?;
    let banks = CoolingModeBanks {
        version: 1,
        fan: fan_modes,
        pump: pump_modes,
        source: Some(CoolingBankSource::Windows2128 {
            fan_json: serde_json::to_string(&source.fan.source_settings)?,
            pump_json: serde_json::to_string(&source.pump.source_settings)?,
        }),
    };
    banks.validate()?;
    let selected = |modes: &[CoolingModeProfile], id: &str| {
        modes
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.policy.clone())
            .ok_or(Error::Invalid("missing selected cooling mode"))
    };
    let config = CoolingConfig {
        fan: Some(selected(&banks.fan, &source.fan.selected_mode)?),
        pump: Some(selected(&banks.pump, &source.pump.selected_mode)?),
        fan_mode: Some(source.fan.selected_mode.clone()),
        pump_mode: Some(source.pump.selected_mode.clone()),
    };
    llcontrol_ipc::validate_cooling_selection(&config, Some(&banks))?;
    Ok(PreparedCooling {
        cooling_config: config,
        cooling_banks: banks,
        fan,
        pump,
    })
}
fn policies(
    bank: &Bank,
    target: CoolingTarget,
    sensor: &str,
    hysteresis_c: f64,
    fallback_percent: u8,
) -> Result<(Value, Vec<CoolingModeProfile>), Error> {
    let mut modes = Vec::new();
    let mut stored = Vec::new();
    for mode in &bank.modes {
        let policy = match (bank.speed_unit, mode.control) {
            (SpeedUnit::Rpm, Control::Fixed) => CoolingPolicy::FixedRpm {
                rpm: mode.phases[0].speed,
            },
            (SpeedUnit::Rpm, Control::Curve) => CoolingPolicy::CurveRpm {
                sensor_id: sensor.into(),
                points: mode
                    .phases
                    .iter()
                    .map(|p| RpmCurvePoint {
                        celsius: p.celsius,
                        rpm: p.speed,
                    })
                    .collect(),
                hysteresis_c,
                fallback_percent,
            },
            (SpeedUnit::Percent, Control::Curve) => CoolingPolicy::FanCurve {
                sensor_id: sensor.into(),
                points: mode
                    .phases
                    .iter()
                    .map(|p| {
                        Ok(CurvePoint {
                            celsius: f64::from(p.celsius),
                            percent: u8::try_from(p.speed).map_err(|_| {
                                crate::Error::Invalid("fan percentage out of bounds")
                            })?,
                        })
                    })
                    .collect::<Result<Vec<_>, Error>>()?,
                hysteresis_c,
                fallback_percent,
            },
            _ => {
                return Err(crate::Error::Invalid(
                    "unsupported RPM bank/channel combination",
                ))
            }
        };
        policy.validate(target)?;
        stored.push(CoolingModeProfile {
            use_default_name: mode.use_default_name,
            id: mode.windows_mode.clone(),
            name: if mode.use_default_name {
                llcontrol_ipc::lcdv2_default_mode_name(&mode.windows_mode, target)
                    .ok_or(Error::Invalid("unknown default cooling mode name"))?
                    .into()
            } else {
                mode.name.clone()
            },
            policy: policy.clone(),
        });
        modes.push(json!({"windows_mode":mode.windows_mode,"name":mode.name,"policy":policy}));
    }
    Ok((
        json!({"selected_mode":bank.selected_mode,"modes":modes,"source_settings":bank.source_settings}),
        stored,
    ))
}
