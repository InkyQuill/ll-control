//! LCDV2 Windows 2.1.28 FanAndLighting reset defaults. No file or hardware IO.
use crate::{
    persistence::{DeviceSettings, LightingState, PumpLightingState},
    profile_group::{GroupKind, GroupProfile},
};
use lconnect_ipc::{
    lighting_catalog, CoolingConfig, FanPreset, LightingKind, NamedProfile, ProfileGroup,
    ProfileLibrary, PumpPreset,
};

pub(crate) fn profile(sensor: &str) -> anyhow::Result<GroupProfile> {
    anyhow::ensure!(
        !sensor.trim().is_empty() && sensor.len() <= 1024 && !sensor.chars().any(char::is_control),
        "reset requires an explicit temperature sensor identity"
    );
    let mut settings = DeviceSettings {
        cooling: CoolingConfig {
            fan: Some(FanPreset::Standard.policy(sensor.into(), 2.0, 100)),
            pump: Some(PumpPreset::Quiet.policy(sensor.into(), 2.0, 100)),
        },
        ..Default::default()
    };
    let color = |c: lconnect_ipc::Rgb| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    for mode in lighting_catalog(LightingKind::Fan) {
        let c = mode.defaults;
        settings.lighting.fan_modes.insert(
            mode.mode,
            LightingState {
                mode: c.mode,
                brightness: c.brightness_percent,
                speed: c.speed_percent,
                direction: c.direction,
                color: color(c.color),
                additional_colors: c.additional_colors.into_iter().map(color).collect(),
                disabled: false,
                fan_topology: None,
            },
        );
    }
    for mode in lighting_catalog(LightingKind::Pump) {
        let c = mode.defaults;
        settings.lighting.pump_modes.insert(
            mode.mode,
            PumpLightingState {
                scope: c.scope.unwrap_or_else(|| "inner".into()),
                mode: c.mode,
                brightness: c.brightness_percent,
                speed: c.speed_percent,
                direction: c.direction,
                color: color(c.color),
                additional_colors: c.additional_colors.into_iter().map(color).collect(),
                disabled: false,
            },
        );
    }
    settings.lighting.fan = settings.lighting.fan_modes.get("rainbow").cloned();
    settings.lighting.pump = settings.lighting.pump_modes.get("rainbow").cloned();
    // Clearing the dynamic registry restores lazy source/unit defaults on next
    // selection. No guessed Linux binding is materialized for CPU/GPU metrics.
    let mut library = ProfileLibrary::default();
    for group in [ProfileGroup::Fan, ProfileGroup::Pump] {
        for slot in 0..4 {
            library
                .profiles
                .push(NamedProfile::defaults(group, slot, Some(sensor))?);
        }
    }
    Ok(GroupProfile::capture(&settings, &library, GroupKind::FanAndLighting)?.profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_group::GroupSettings;
    #[test]
    fn reset_restores_active_and_inactive_defaults_and_requires_explicit_binding() {
        for invalid in ["", " ", "cpu\0temp", "cpu\ntemp"] {
            assert!(profile(invalid).is_err());
        }
        let p = profile("hwmon:cpu:Tctl").unwrap();
        let GroupSettings::FanAndLighting(g) = &p.content else {
            panic!()
        };
        assert_eq!(
            g.cooling.fan,
            Some(FanPreset::Standard.policy("hwmon:cpu:Tctl".into(), 2.0, 100))
        );
        assert_eq!(
            g.cooling.pump,
            Some(PumpPreset::Quiet.policy("hwmon:cpu:Tctl".into(), 2.0, 100))
        );
        let fan = g.lighting.fan.as_ref().unwrap();
        let pump = g.lighting.pump.as_ref().unwrap();
        assert_eq!(
            (&*fan.mode, fan.brightness, fan.speed),
            ("rainbow", 100, 75)
        );
        assert_eq!(
            (&*pump.mode, &*pump.scope, pump.brightness, pump.speed),
            ("rainbow", "inner", 100, 75)
        );
        assert_eq!(
            g.lighting.fan_topology,
            lconnect_ipc::FanLightingTopology::default()
        );
        assert_eq!(g.lighting.fan_modes.len(), 6);
        assert_eq!(g.lighting.pump_modes.len(), 16);
        assert!(g.dynamic_rgb.is_none() && g.dynamic_rgb_profiles.is_empty());
        assert!(!g.fan.has_manual_setting() && !g.pump.has_manual_setting());
        assert_eq!(p.profiles.len(), 8);
        for slot in &p.profiles {
            assert_eq!(
                slot,
                &NamedProfile::defaults(slot.content.group(), slot.slot, Some("hwmon:cpu:Tctl"))
                    .unwrap()
            );
        }
    }
}
