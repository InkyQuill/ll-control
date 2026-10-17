//! Full LCDV2 profile groups, separate from individual named-slot archives.
//!
//! These are storage snapshots, not live commands or Windows ZIP decoders.
//! Only the serialized settings owner may commit one; it must first prepare
//! and retain every referenced asset, then reconcile the affected runtime.
use crate::persistence::{DeviceSettings, FanSection, LcdSection, LightingSection, PumpSection};
use anyhow::{ensure, Context};
use lconnect_ipc::{NamedProfile, ProfileGroup, ProfileLibrary};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    Screen,
    FanAndLighting,
}
impl GroupKind {
    pub(crate) fn includes(self, group: ProfileGroup) -> bool {
        match self {
            Self::Screen => group == ProfileGroup::Screen,
            Self::FanAndLighting => matches!(group, ProfileGroup::Fan | ProfileGroup::Pump),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenGroup {
    pub lcd: LcdSection,
    pub theme: Option<lconnect_ipc::LcdTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<crate::screen_editor::StoredScreenEditor>,
    pub media_clip: Option<String>,
    /// None retains legacy shared-catalog semantics; archive export must
    /// materialize that catalog before collecting its file dependencies.
    pub collections: Option<lconnect_ipc::ScreenCollections>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanAndLightingGroup {
    pub fan: FanSection,
    pub pump: PumpSection,
    pub cooling: lconnect_ipc::CoolingConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooling_banks: Option<lconnect_ipc::CoolingModeBanks>,
    pub lighting: LightingSection,
    pub dynamic_rgb: Option<lconnect_ipc::DynamicRgbConfig>,
    pub dynamic_rgb_profiles: lconnect_ipc::DynamicRgbProfiles,
}

impl FanAndLightingGroup {
    pub(crate) fn validate_runtime(&self) -> lconnect_ipc::Result<()> {
        GroupProfile {
            version: 1,
            content: GroupSettings::FanAndLighting(Box::new(self.clone())),
            profiles: vec![],
        }
        .validate()
        .map_err(|error| lconnect_ipc::IpcError::InvalidArgument(error.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "group",
    content = "settings",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GroupSettings {
    Screen(Box<ScreenGroup>),
    FanAndLighting(Box<FanAndLightingGroup>),
}

/// No source device identity, library revision, or transaction receipts travel
/// in the portable content. Slots keep their original indices, including gaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupProfile {
    pub version: u32,
    pub content: GroupSettings,
    pub profiles: Vec<NamedProfile>,
}

/// Target-side optimistic token. Never substitute the source archive's token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRevision {
    pub profile_revision: u64,
    pub settings_sha256: String,
}

#[derive(Debug, Clone)]
pub struct GroupSnapshot {
    pub profile: GroupProfile,
    pub revision: GroupRevision,
}

impl GroupProfile {
    pub fn kind(&self) -> GroupKind {
        match &self.content {
            GroupSettings::Screen(_) => GroupKind::Screen,
            GroupSettings::FanAndLighting(_) => GroupKind::FanAndLighting,
        }
    }

    pub fn capture(
        latest: &DeviceSettings,
        library: &ProfileLibrary,
        kind: GroupKind,
    ) -> anyhow::Result<GroupSnapshot> {
        crate::persistence::validate_schema(latest)?;
        library.validate()?;
        let content = match kind {
            GroupKind::Screen => GroupSettings::Screen(Box::new(ScreenGroup {
                lcd: latest.lcd.clone(),
                theme: latest.theme.clone(),
                editor: latest.screen_editor.clone(),
                media_clip: latest.media_clip.clone(),
                collections: latest.screen_collections.clone(),
            })),
            GroupKind::FanAndLighting => {
                GroupSettings::FanAndLighting(Box::new(FanAndLightingGroup {
                    fan: latest.fan.clone(),
                    pump: latest.pump.clone(),
                    cooling: latest.cooling.clone(),
                    cooling_banks: latest.cooling_banks.clone(),
                    lighting: latest.lighting.clone(),
                    dynamic_rgb: latest.dynamic_rgb.clone(),
                    dynamic_rgb_profiles: latest.dynamic_rgb_profiles.clone(),
                }))
            }
        };
        let mut profiles: Vec<_> = library
            .profiles
            .iter()
            .filter(|p| kind.includes(p.content.group()))
            .cloned()
            .collect();
        profiles.sort_by_key(|p| {
            (
                if p.content.group() == ProfileGroup::Pump {
                    1
                } else {
                    0
                },
                p.slot,
            )
        });
        let profile = Self {
            version: 1,
            content,
            profiles,
        };
        profile.validate()?;
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(&profile)?);
        if kind == GroupKind::FanAndLighting && latest.cooling_modes_revision > 0 {
            hash.update(b"cooling-mode-intent-v1");
            hash.update(latest.cooling_modes_revision.to_be_bytes());
        }
        let revision = GroupRevision {
            profile_revision: library.revision,
            settings_sha256: format!("{:x}", hash.finalize()),
        };
        Ok(GroupSnapshot { profile, revision })
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.version == 1, "unsupported profile group version");
        ensure!(self.profiles.len() <= 8, "too many profile group slots");
        for (index, p) in self.profiles.iter().enumerate() {
            p.validate()?;
            ensure!(
                self.kind().includes(p.content.group()),
                "profile belongs to another group"
            );
            ensure!(
                !self.profiles[..index]
                    .iter()
                    .any(|old| old.slot == p.slot && old.content.group() == p.content.group()),
                "duplicate profile group slot"
            );
        }
        let mut settings = DeviceSettings::default();
        self.copy_settings(&mut settings);
        crate::persistence::validate_schema(&settings)?;
        if let GroupSettings::FanAndLighting(group) = &self.content {
            ensure!(
                group.fan.pwm_percent <= 100 && group.pump.pwm_percent <= 100,
                "invalid saved PWM"
            );
            ensure!(
                matches!(group.pump.source.as_str(), "" | "mcu" | "application"),
                "invalid pump source"
            );
            lconnect_ipc::validate_cooling_selection(&group.cooling, group.cooling_banks.as_ref())?;
            for (policy, target) in [
                (&group.cooling.fan, lconnect_ipc::CoolingTarget::Fan),
                (&group.cooling.pump, lconnect_ipc::CoolingTarget::Pump),
            ] {
                if let Some(policy) = policy {
                    policy.validate(target)?;
                }
            }
            for state in group
                .lighting
                .fan
                .iter()
                .chain(group.lighting.fan_modes.values())
            {
                validate_lighting(
                    lconnect_ipc::LightingKind::Fan,
                    &state.mode,
                    state.brightness,
                    state.speed,
                    &state.direction,
                    &state.color,
                )?;
            }
            for state in group
                .lighting
                .pump
                .iter()
                .chain(group.lighting.pump_modes.values())
            {
                validate_lighting(
                    lconnect_ipc::LightingKind::Pump,
                    &state.mode,
                    state.brightness,
                    state.speed,
                    &state.direction,
                    &state.color,
                )?;
                ensure!(
                    matches!(state.scope.as_str(), "" | "inner" | "outer" | "all"),
                    "invalid pump lighting scope"
                );
            }
            if let Some(config) = &group.dynamic_rgb {
                let pump = group
                    .lighting
                    .pump
                    .as_ref()
                    .context("dynamic RGB requires a selected pump mode")?;
                config.validate_mode(&pump.mode)?;
            }
        }
        if let GroupSettings::Screen(group) = &self.content {
            ensure!(group.lcd.brightness <= 100, "invalid screen brightness");
            ensure!(
                matches!(group.lcd.rotation.as_str(), "" | "0" | "90" | "180" | "270"),
                "invalid screen rotation"
            );
            ensure!(
                matches!(
                    group.lcd.control_mode.as_str(),
                    "" | "LocalUI" | "Application" | "LocalH264" | "LocalAVI"
                ),
                "invalid screen control mode"
            );
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 1024 * 1024,
            "profile group exceeds manifest budget"
        );
        Ok(())
    }

    fn copy_settings(&self, next: &mut DeviceSettings) {
        match &self.content {
            GroupSettings::Screen(group) => {
                next.lcd = group.lcd.clone();
                next.theme = group.theme.clone();
                next.screen_editor = group.editor.clone();
                next.media_clip = group.media_clip.clone();
                next.screen_collections = group.collections.clone();
            }
            GroupSettings::FanAndLighting(group) => {
                next.fan = group.fan.clone();
                next.pump = group.pump.clone();
                next.cooling = group.cooling.clone();
                next.cooling_banks = group.cooling_banks.clone();
                next.lighting = group.lighting.clone();
                next.dynamic_rgb = group.dynamic_rgb.clone();
                next.dynamic_rgb_profiles = group.dynamic_rgb_profiles.clone();
            }
        }
    }

    /// Pure preparation for the shared two-file journal. Preserves the other
    /// group and global thermal alert policy; never applies a hardware setting.
    pub(crate) fn restore(
        &self,
        latest: &DeviceSettings,
        library: &ProfileLibrary,
        expected: &GroupRevision,
    ) -> anyhow::Result<(DeviceSettings, ProfileLibrary)> {
        self.validate()?;
        let current = Self::capture(latest, library, self.kind())?;
        ensure!(
            current.revision == *expected,
            "profile group changed; reload before importing"
        );
        let mut next = latest.clone();
        self.copy_settings(&mut next);
        if self.kind() == GroupKind::FanAndLighting {
            next.cooling_modes_revision = latest
                .cooling_modes_revision
                .checked_add(1)
                .filter(|v| *v <= i64::MAX as u64)
                .context("cooling mode revision exhausted")?;
        }
        if self.kind() == GroupKind::Screen {
            // Source counters and receipts must not become target concurrency
            // tokens or revive an old reset. Advance even when source is legacy.
            if let Some(collections) = &mut next.screen_collections {
                collections.revision = latest
                    .screen_collections
                    .as_ref()
                    .map_or(0, |c| c.revision)
                    .checked_add(1)
                    .context("collection revision exhausted")?;
            } else {
                ensure!(
                    latest.screen_collections.is_none(),
                    "materialize legacy source collections before replacing a versioned catalog"
                );
            }
            next.screen_reset_receipt = None;
        }
        let mut profiles = library.clone();
        profiles
            .profiles
            .retain(|p| !self.kind().includes(p.content.group()));
        profiles.profiles.extend(self.profiles.iter().cloned());
        profiles.revision = profiles
            .revision
            .checked_add(1)
            .context("profile revision exhausted")?;
        crate::persistence::validate_schema(&next)?;
        profiles.validate()?;
        Ok((next, profiles))
    }
}

fn validate_lighting(
    kind: lconnect_ipc::LightingKind,
    mode: &str,
    brightness: u8,
    speed: u8,
    direction: &str,
    color: &str,
) -> anyhow::Result<()> {
    ensure!(
        lconnect_ipc::lighting_color_count(kind, mode).is_some(),
        "invalid lighting mode"
    );
    ensure!(
        brightness <= 100 && speed <= 100,
        "invalid lighting percentage"
    );
    ensure!(
        matches!(
            direction,
            "" | "forward"
                | "reverse"
                | "right"
                | "clockwise"
                | "right_or_clockwise"
                | "left"
                | "counter_clockwise"
                | "counterclockwise"
                | "left_or_counter_clockwise"
                | "up"
                | "down"
                | "spreaded"
                | "spread"
                | "gathered"
                | "gather"
        ),
        "invalid lighting direction"
    );
    let color = color.strip_prefix('#').unwrap_or(color);
    ensure!(
        color.len() == 6 && color.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid lighting color"
    );
    Ok(())
}

mod import;
pub(crate) use import::{validate_operation_id, GroupImportIntent};
pub use import::{
    GroupImportCommit, GroupImportError, GroupImportReadback, GroupImportReceipt,
    MAX_GROUP_IMPORT_RECEIPTS,
};
