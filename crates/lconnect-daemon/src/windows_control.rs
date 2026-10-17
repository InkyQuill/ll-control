//! Map a complete normalized Windows control draft onto a target snapshot.
//! No IO; publication/admission/commit remain with the existing group coordinator.
use crate::{
    persistence::{LightingSection, LightingState, PumpLightingState},
    profile_group::{GroupKind, GroupProfile, GroupSettings},
};
use anyhow::ensure;
use lconnect_ipc::{lighting_catalog, LightingKind};
use lconnect_profile::control::PreparedControl;

pub fn profile(draft: &PreparedControl, target: &GroupProfile) -> anyhow::Result<GroupProfile> {
    ensure!(
        draft.version == 1,
        "unsupported Windows control draft version"
    );
    for hash in [&draft.source_backup_sha256, &draft.source_profile_sha256] {
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid Windows source fingerprint"
        );
    }
    target.validate()?;
    ensure!(
        target.kind() == GroupKind::FanAndLighting,
        "target must be FanAndLighting"
    );
    draft.cooling_banks.validate()?;
    let defaults = lconnect_ipc::lcdv2_default_cooling_modes("validation:binding")?;
    for (bank, expected) in [
        (&draft.cooling_banks.fan, &defaults.fan),
        (&draft.cooling_banks.pump, &defaults.pump),
    ] {
        ensure!(
            bank.len() == expected.len()
                && expected.iter().all(|e| bank.iter().any(|m| m.id == e.id)),
            "incomplete Windows cooling bank"
        );
    }
    ensure!(
        draft.cooling.fan.is_some()
            && draft.cooling.pump.is_some()
            && draft.cooling.fan_mode.is_some()
            && draft.cooling.pump_mode.is_some(),
        "missing Windows cooling selection"
    );
    lconnect_ipc::validate_cooling_selection(&draft.cooling, Some(&draft.cooling_banks))?;
    for (policy, id, bank) in [
        (
            &draft.cooling.fan,
            &draft.cooling.fan_mode,
            &draft.cooling_banks.fan,
        ),
        (
            &draft.cooling.pump,
            &draft.cooling.pump_mode,
            &draft.cooling_banks.pump,
        ),
    ] {
        ensure!(
            bank.iter()
                .find(|mode| Some(&mode.id) == id.as_ref())
                .map(|mode| &mode.policy)
                == policy.as_ref(),
            "selected Windows cooling policy differs from stored mode"
        );
    }
    let lighting = &draft.lighting;
    lighting.fan_topology.validate()?;
    lighting.dynamic_rgb_profiles.validate()?;
    let mut section = LightingSection {
        fan_topology: lighting.fan_topology.clone(),
        windows_source: Some(draft.lighting_source_json.clone()),
        ..Default::default()
    };
    for (kind, modes) in [
        (LightingKind::Fan, &lighting.fan_modes),
        (LightingKind::Pump, &lighting.pump_modes),
    ] {
        let catalog = lighting_catalog(kind);
        ensure!(
            modes.len() == catalog.len() && catalog.iter().all(|c| modes.contains_key(&c.mode)),
            "incomplete Windows lighting bank"
        );
        for (key, cfg) in modes {
            ensure!(key == &cfg.mode, "lighting key/mode mismatch");
            if kind == LightingKind::Fan {
                ensure!(
                    cfg.fan_topology.as_ref() == Some(&lighting.fan_topology),
                    "per-mode fan topology disagrees with global topology"
                );
                section.fan_modes.insert(
                    key.clone(),
                    LightingState {
                        mode: key.clone(),
                        brightness: cfg.brightness_percent,
                        speed: cfg.speed_percent,
                        direction: cfg.direction.clone(),
                        color: color(cfg.color),
                        additional_colors: cfg
                            .additional_colors
                            .iter()
                            .copied()
                            .map(color)
                            .collect(),
                        disabled: cfg.disabled,
                        ..Default::default()
                    },
                );
            } else {
                section.pump_modes.insert(
                    key.clone(),
                    PumpLightingState {
                        scope: cfg.scope.clone().unwrap_or_else(|| "inner".into()),
                        mode: key.clone(),
                        brightness: cfg.brightness_percent,
                        speed: cfg.speed_percent,
                        direction: cfg.direction.clone(),
                        color: color(cfg.color),
                        additional_colors: cfg
                            .additional_colors
                            .iter()
                            .copied()
                            .map(color)
                            .collect(),
                        disabled: cfg.disabled,
                    },
                );
            }
        }
    }
    ensure!(
        lighting.dynamic_rgb_profiles.modes.len() == 16,
        "incomplete dynamic mode registry"
    );
    for (mode, cfg) in &lighting.pump_modes {
        let profile = lighting
            .dynamic_rgb_profiles
            .selected(mode)
            .ok_or_else(|| anyhow::anyhow!("missing dynamic mode"))?;
        ensure!(
            profile.bindings.len() == 6 && cfg.dynamic_mode == Some(profile.enabled),
            "incomplete dynamic source registry or enabled state mismatch"
        );
    }
    section.fan = Some(
        section
            .fan_modes
            .get(&lighting.selected_fan_mode)
            .ok_or_else(|| anyhow::anyhow!("missing selected fan lighting mode"))?
            .clone(),
    );
    section.pump = Some(
        section
            .pump_modes
            .get(&lighting.selected_pump_mode)
            .ok_or_else(|| anyhow::anyhow!("missing selected pump lighting mode"))?
            .clone(),
    );
    let selected = lighting
        .dynamic_rgb_profiles
        .selected(&lighting.selected_pump_mode)
        .ok_or_else(|| anyhow::anyhow!("missing selected dynamic mode"))?;
    let expected = selected
        .enabled
        .then(|| selected.bindings.last().cloned())
        .flatten();
    ensure!(
        expected == lighting.dynamic_rgb,
        "active dynamic policy differs from selected saved binding"
    );
    let mut result = target.clone();
    let GroupSettings::FanAndLighting(group) = &mut result.content else {
        unreachable!()
    };
    // Named Linux extension slots come from the target, not an invented Windows
    // 8-to-4 mapping. Adjacent Screen/globals are outside this group altogether.
    group.fan = Default::default();
    group.pump = Default::default();
    group.cooling = draft.cooling.clone();
    group.cooling_banks = Some(draft.cooling_banks.clone());
    group.lighting = section;
    group.dynamic_rgb = lighting.dynamic_rgb.clone();
    group.dynamic_rgb_profiles = lighting.dynamic_rgb_profiles.clone();
    result.validate()?;
    Ok(result)
}
fn color(c: lconnect_ipc::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}
