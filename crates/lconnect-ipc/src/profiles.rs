//! Named LCDV2 slots. Saving a slot never applies it to a device.
use crate::{CoolingPolicy, CoolingTarget, IpcError, LcdTheme, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileGroup {
    Screen,
    Fan,
    Pump,
}
impl ProfileGroup {
    pub fn slot_count(self) -> u8 {
        if self == Self::Screen {
            8
        } else {
            4
        }
    }

    pub fn validate_slot(self, slot: u8) -> Result<()> {
        if slot >= self.slot_count() {
            return Err(IpcError::InvalidArgument(
                "profile slot out of range".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProfileContent {
    Screen(LcdTheme),
    /// A prepared clip occupying one of the same eight screen slots.
    ScreenMedia(String),
    Fan(CoolingPolicy),
    Pump(CoolingPolicy),
}
impl ProfileContent {
    pub fn group(&self) -> ProfileGroup {
        match self {
            Self::Screen(_) | Self::ScreenMedia(_) => ProfileGroup::Screen,
            Self::Fan(_) => ProfileGroup::Fan,
            Self::Pump(_) => ProfileGroup::Pump,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Screen(theme) => theme.validate(),
            Self::ScreenMedia(key) => {
                if key.len() != 32
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(IpcError::InvalidArgument(
                        "invalid prepared clip key".into(),
                    ));
                }
                Ok(())
            }
            Self::Fan(policy) => policy.validate(CoolingTarget::Fan),
            Self::Pump(policy) => policy.validate(CoolingTarget::Pump),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedProfile {
    pub slot: u8,
    pub name: String,
    pub content: ProfileContent,
}
impl NamedProfile {
    /// Reset one named slot. Cooling uses reference Quiet with an explicit binding.
    /// Screen defaults are original Linux artwork, not Windows profile/assets import.
    pub fn defaults(group: ProfileGroup, slot: u8, sensor_id: Option<&str>) -> Result<Self> {
        group.validate_slot(slot)?;
        let name = match group {
            ProfileGroup::Screen => format!("Screen {}", slot + 1),
            ProfileGroup::Fan => format!("UserDefine{}", slot + 1),
            ProfileGroup::Pump => format!("Pump {}", slot + 1),
        };
        let content = match group {
            ProfileGroup::Screen => {
                if sensor_id.is_some() {
                    return Err(IpcError::InvalidArgument(
                        "screen reset does not accept a sensor binding".into(),
                    ));
                }
                let style = |y| crate::TextStyle {
                    font_asset: None,
                    line_box: false,
                    max_width: None,
                    line_height: None,
                    vertical_align: Default::default(),
                    font: Default::default(),
                    opacity: 100,
                    shadow: None,
                    x: 240,
                    y,
                    size: 36,
                    color: [235, 245, 255],
                    align: crate::TextAlign::Center,
                };
                ProfileContent::Screen(LcdTheme {
                    page_cache: Vec::new(),
                    basic_editor: None,
                    version: 1,
                    name: name.clone(),
                    background: [12, 18, 30],
                    background_image: None,
                    background_video: None,
                    background_fill: None,
                    layers: vec![
                        crate::ThemeLayer::Text {
                            text: "L-Connect Linux".into(),
                            style: style(110),
                        },
                        crate::ThemeLayer::Clock {
                            format: Default::default(),
                            twenty_four_hour: true,
                            show_seconds: true,
                            style: style(300),
                        },
                    ],
                })
            }
            ProfileGroup::Fan | ProfileGroup::Pump => {
                let sensor = sensor_id.filter(|s| !s.trim().is_empty()).ok_or_else(|| {
                    IpcError::InvalidArgument(
                        "cooling reset requires an explicit temperature sensor".into(),
                    )
                })?;
                if group == ProfileGroup::Fan {
                    ProfileContent::Fan(crate::FanPreset::Quiet.policy(sensor.into(), 2.0, 100))
                } else {
                    ProfileContent::Pump(crate::PumpPreset::Quiet.policy(sensor.into(), 2.0, 100))
                }
            }
        };
        let profile = Self {
            slot,
            name,
            content,
        };
        profile.validate()?;
        Ok(profile)
    }
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        self.content.group().validate_slot(self.slot)?;
        self.content.validate()
    }
}
fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(IpcError::InvalidArgument(
            "profile name must be 1–128 bytes without control characters".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileLibrary {
    pub version: u32,
    /// Optimistic concurrency token for all edits, retained across daemon restarts.
    pub revision: u64,
    pub profiles: Vec<NamedProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archive_imports: Vec<crate::ArchiveImportReceipt>,
}
impl Default for ProfileLibrary {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            profiles: Vec::new(),
            archive_imports: Vec::new(),
        }
    }
}
impl ProfileLibrary {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.profiles.len() > 16 {
            return Err(IpcError::InvalidArgument(
                "unsupported profile library version or size".into(),
            ));
        }
        if self.archive_imports.len() > 64 {
            return Err(IpcError::InvalidArgument(
                "too many archive import receipts".into(),
            ));
        }
        let mut receipts = std::collections::HashSet::new();
        for receipt in &self.archive_imports {
            receipt.group.validate_slot(receipt.slot)?;
            if receipt.id.len() != 32
                || !receipt
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || !receipts.insert(&receipt.id)
                || !(1..=crate::MAX_PROFILE_ARCHIVE_BYTES).contains(&receipt.size)
                || receipt.expected_revision.checked_add(1) != Some(receipt.committed_revision)
                || receipt.committed_revision > self.revision
                || receipt.source_sha256.as_ref().is_some_and(|s| {
                    s.len() != 64
                        || !s
                            .bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                })
            {
                return Err(IpcError::InvalidArgument(
                    "invalid archive import receipt".into(),
                ));
            }
        }
        for (i, profile) in self.profiles.iter().enumerate() {
            profile.validate()?;
            if self.profiles[..i]
                .iter()
                .any(|p| p.slot == profile.slot && p.content.group() == profile.content.group())
            {
                return Err(IpcError::InvalidArgument("duplicate profile slot".into()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileEdit {
    /// Replace every slot in one group in one library revision. Does not apply
    /// device settings or remove files; not the full device Screen reset.
    ResetGroup {
        group: ProfileGroup,
        sensor_id: Option<String>,
    },
    Reset {
        group: ProfileGroup,
        slot: u8,
        sensor_id: Option<String>,
    },
    Save {
        profile: NamedProfile,
    },
    Rename {
        group: ProfileGroup,
        slot: u8,
        name: String,
    },
    Delete {
        group: ProfileGroup,
        slot: u8,
    },
}
impl ProfileEdit {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ResetGroup { group, sensor_id } => {
                NamedProfile::defaults(*group, 0, sensor_id.as_deref()).map(|_| ())
            }
            Self::Reset {
                group,
                slot,
                sensor_id,
            } => NamedProfile::defaults(*group, *slot, sensor_id.as_deref()).map(|_| ()),
            Self::Save { profile } => profile.validate(),
            Self::Rename { group, slot, name } => {
                group.validate_slot(*slot)?;
                validate_name(name)
            }
            Self::Delete { group, slot } => group.validate_slot(*slot),
        }
    }
}
