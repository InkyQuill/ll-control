//! Stored cooling modes, independent of active policies and named user slots.
use crate::{CoolingPolicy, CoolingTarget, IpcError, Result, SettingsSaveState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoolingModeProfile {
    /// Translate the known factory label; never infer this from user name text.
    #[serde(default, skip_serializing_if = "is_false")]
    pub use_default_name: bool,
    pub id: String,
    pub name: String,
    pub policy: CoolingPolicy,
}

/// Read/edit view. It cannot replace the full bank or its private provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoolingModeLibrary {
    pub version: u32,
    pub revision: String,
    pub fan: Vec<CoolingModeProfile>,
    pub pump: Vec<CoolingModeProfile>,
    pub persistence: SettingsSaveState,
}

/// Historical presentation metadata only. Never interpreted as runtime intent,
/// sensor identity, device capabilities, file paths or executable configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoolingBankSource {
    #[serde(rename = "windows_llcontrol3_2_1_28")]
    Windows2128 { fan_json: String, pump_json: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoolingModeBanks {
    pub version: u32,
    pub fan: Vec<CoolingModeProfile>,
    pub pump: Vec<CoolingModeProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<CoolingBankSource>,
}

impl CoolingModeBanks {
    pub fn validate(&self) -> Result<()> {
        let invalid = |message: &str| IpcError::InvalidArgument(message.into());
        if self.version != 1 {
            return Err(invalid("unsupported cooling mode bank version"));
        }
        for (modes, target) in [
            (&self.fan, CoolingTarget::Fan),
            (&self.pump, CoolingTarget::Pump),
        ] {
            if modes.len() > 16 {
                return Err(invalid("too many stored cooling modes"));
            }
            for (index, mode) in modes.iter().enumerate() {
                if mode.id.is_empty()
                    || mode.id.len() > 32
                    || !mode
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                    || modes[..index].iter().any(|old| old.id == mode.id)
                    || mode.name.trim().is_empty()
                    || mode.name.len() > 128
                    || mode.name.chars().any(char::is_control)
                {
                    return Err(invalid(
                        "invalid or duplicate stored cooling mode identity/name",
                    ));
                }
                if mode.use_default_name
                    && lcdv2_default_mode_name(&mode.id, target) != Some(mode.name.as_str())
                {
                    return Err(invalid("invalid default cooling mode name"));
                }
                mode.policy.validate(target)?;
            }
        }
        if let Some(CoolingBankSource::Windows2128 {
            fan_json,
            pump_json,
        }) = &self.source
        {
            for value in [fan_json, pump_json] {
                // JSON is stored as text so nullable Windows fields survive TOML.
                // Parsing proves bounded object syntax, not Windows semantics.
                if value.len() > 64 * 1024
                    || !serde_json::from_str::<serde_json::Value>(value)
                        .is_ok_and(|v| v.is_object())
                {
                    return Err(invalid("invalid or oversized cooling source metadata"));
                }
            }
        }
        Ok(())
    }
}

/// A stored mode can be edited after Apply; the active policy remains its old
/// snapshot. Validate identity membership, not equality to the edited bank.
pub fn validate_cooling_selection(
    config: &crate::CoolingConfig,
    banks: Option<&CoolingModeBanks>,
) -> Result<()> {
    for (id, policy, target) in [
        (&config.fan_mode, &config.fan, CoolingTarget::Fan),
        (&config.pump_mode, &config.pump, CoolingTarget::Pump),
    ] {
        if let Some(id) = id {
            let modes = banks.map(|banks| match target {
                CoolingTarget::Fan => &banks.fan,
                CoolingTarget::Pump => &banks.pump,
            });
            if policy.is_none()
                || !modes.is_some_and(|modes| modes.iter().any(|mode| &mode.id == id))
            {
                return Err(IpcError::InvalidArgument(
                    "selected cooling mode requires a requested policy and matching stored ID"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Canonical English factory labels. User-defined names never use this lookup.
pub fn lcdv2_default_mode_name(id: &str, target: CoolingTarget) -> Option<&'static str> {
    match (target, id) {
        (CoolingTarget::Fan, "Quiet") => Some("Quiet"),
        (CoolingTarget::Fan, "StandardSpeed") => Some("Standard"),
        (CoolingTarget::Fan, "HighSpeed") => Some("High speed"),
        (CoolingTarget::Fan, "FullSpeed") => Some("Full speed"),
        (CoolingTarget::Fan, "UserDefine1") => Some("Custom 1"),
        (CoolingTarget::Fan, "UserDefine2") => Some("Custom 2"),
        (CoolingTarget::Fan, "UserDefine3") => Some("Custom 3"),
        (CoolingTarget::Fan, "UserDefine4") => Some("Custom 4"),
        (CoolingTarget::Pump, "FixRPM") => Some("Fixed RPM"),
        (CoolingTarget::Pump, "PWMPerformance") => Some("Performance"),
        (CoolingTarget::Pump, "PWMQuiet") => Some("Quiet"),
        _ => None,
    }
}

/// Pure per-mode restoration. Fixed RPM needs no temperature binding; curves do.
pub fn lcdv2_default_cooling_mode(
    target: CoolingTarget,
    id: &str,
    sensor: &str,
) -> Result<CoolingModeProfile> {
    let name = lcdv2_default_mode_name(id, target).ok_or_else(|| {
        IpcError::InvalidArgument("no factory defaults for this cooling mode".into())
    })?;
    let policy = if target == CoolingTarget::Pump && id == "FixRPM" {
        CoolingPolicy::FixedRpm { rpm: 2200 }
    } else {
        validate_factory_sensor(sensor)?;
        match target {
            CoolingTarget::Fan => match id {
                "StandardSpeed" => crate::FanPreset::Standard,
                "HighSpeed" => crate::FanPreset::High,
                "FullSpeed" => crate::FanPreset::Full,
                _ => crate::FanPreset::Quiet,
            }
            .policy(sensor.into(), 0.0, 100),
            CoolingTarget::Pump => match id {
                "PWMPerformance" => crate::PumpPreset::Performance,
                _ => crate::PumpPreset::Quiet,
            }
            .policy(sensor.into(), 0.0, 100),
        }
    };
    policy.validate(target)?;
    Ok(CoolingModeProfile {
        id: id.into(),
        name: name.into(),
        use_default_name: true,
        policy,
    })
}
fn validate_factory_sensor(sensor: &str) -> Result<()> {
    if sensor.trim().is_empty() || sensor.len() > 4096 || sensor.chars().any(char::is_control) {
        return Err(IpcError::InvalidArgument(
            "an explicit temperature sensor is required".into(),
        ));
    }
    Ok(())
}

/// Pure LCDV2 Windows2.1.28 defaults; creates no active policies or device IO.
/// Temperature binding is explicit; runtime fallback is100%, with no hysteresis.
pub fn lcdv2_default_cooling_modes(sensor: &str) -> Result<CoolingModeBanks> {
    validate_factory_sensor(sensor)?;
    let fan = [
        "Quiet",
        "StandardSpeed",
        "HighSpeed",
        "FullSpeed",
        "UserDefine1",
        "UserDefine2",
        "UserDefine3",
        "UserDefine4",
    ]
    .into_iter()
    .map(|id| lcdv2_default_cooling_mode(CoolingTarget::Fan, id, sensor))
    .collect::<Result<Vec<_>>>()?;
    let pump = ["FixRPM", "PWMPerformance", "PWMQuiet"]
        .into_iter()
        .map(|id| lcdv2_default_cooling_mode(CoolingTarget::Pump, id, sensor))
        .collect::<Result<Vec<_>>>()?;
    let banks = CoolingModeBanks {
        version: 1,
        fan,
        pump,
        source: None,
    };
    banks.validate()?;
    Ok(banks)
}
