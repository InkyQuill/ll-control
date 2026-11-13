//! Imported Windows editor state, independent of the active LCD scene.
use anyhow::{ensure, Context};
use llcontrol_ipc::ScreenBackgroundAsset;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredScreenEditor {
    pub version: u8,
    /// None preserves an unconverted selected effect without substituting basic.
    pub theme: Option<llcontrol_ipc::LcdTheme>,
    /// Bounded, private source inventory. Never executed or opened as host paths.
    pub source_json: String,
    /// Archive members and explicit reference aliases retained even outside galleries.
    pub resources: BTreeMap<String, ScreenBackgroundAsset>,
}
impl StoredScreenEditor {
    pub fn from_prepared(
        source: &llcontrol_profile::screen::ScreenInspection,
        plan: &llcontrol_profile::screen_plan::ScreenPlan,
    ) -> anyhow::Result<Self> {
        ensure!(
            source.archive.backup_sha256 == plan.source_backup_sha256
                && source.record_index == plan.source_record_index,
            "Screen editor source fingerprint mismatch"
        );
        let convert = |binding: &llcontrol_profile::linux_screen::ResourceBinding| match binding {
            llcontrol_profile::linux_screen::ResourceBinding::Image { asset } => {
                ScreenBackgroundAsset::Image(asset.clone())
            }
            llcontrol_profile::linux_screen::ResourceBinding::Video { key } => {
                ScreenBackgroundAsset::Video(key.clone())
            }
        };
        let resources =
            plan.staged_resources
                .iter()
                .map(|(entry, binding)| (format!("archive:{entry}"), convert(binding)))
                .chain(plan.resolved_references.iter().map(|(reference, binding)| {
                    (format!("reference:{reference}"), convert(binding))
                }))
                .collect();
        let value = Self {
            version: 1,
            resources,
            theme: plan.current_editor_theme.clone(),
            source_json: serde_json::to_string(
                &serde_json::json!({"version":1,"format":"windows_lconnect3_2_1_28","source":source}),
            )?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.version == 1,
            "unsupported stored Screen editor version"
        );
        ensure!(
            self.source_json.len() <= 512 * 1024,
            "historical Screen source exceeds512KiB"
        );
        let source: serde_json::Value = serde_json::from_str(&self.source_json)?;
        ensure!(
            source.is_object(),
            "historical Screen source must be an object"
        );
        ensure!(
            source["version"] == 1
                && source["format"] == "windows_lconnect3_2_1_28"
                && source["source"].is_object(),
            "invalid historical Screen source envelope"
        );
        let sha = source
            .pointer("/source/archive/backup_sha256")
            .and_then(|v| v.as_str())
            .context("missing historical Screen backup fingerprint")?;
        ensure!(
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid historical Screen backup fingerprint"
        );
        ensure!(
            self.resources.len() <= 1280,
            "too many retained Screen resources"
        );
        for (reference, asset) in &self.resources {
            ensure!(
                !reference.is_empty()
                    && reference.len() <= 32780
                    && !reference.chars().any(char::is_control),
                "invalid retained Screen resource reference"
            );
            let (key, builtin) = match asset {
                ScreenBackgroundAsset::Image(key) => {
                    (key, llcontrol_image::defaults::image(key).is_some())
                }
                ScreenBackgroundAsset::Video(key) => (key, false),
            };
            ensure!(
                builtin
                    || (key.len() == 32
                        && key
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
                "invalid retained Screen resource key"
            );
        }
        if let Some(theme) = &self.theme {
            theme.validate()?;
            ensure!(
                theme.page_kind() == llcontrol_ipc::LcdPageKind::Basic
                    && theme.basic_editor.is_some(),
                "stored Screen editor requires basic editor banks"
            );
        }
        Ok(())
    }
    pub fn image_keys(&self) -> BTreeSet<&str> {
        self.theme
            .iter()
            .flat_map(|theme| theme.image_assets())
            .chain(self.resources.values().filter_map(|asset| match asset {
                ScreenBackgroundAsset::Image(key) => Some(key.as_str()),
                _ => None,
            }))
            .collect()
    }
    pub fn video_keys(&self) -> BTreeSet<&str> {
        self.theme
            .iter()
            .flat_map(|theme| theme.video_assets())
            .chain(self.resources.values().filter_map(|asset| match asset {
                ScreenBackgroundAsset::Video(key) => Some(key.as_str()),
                _ => None,
            }))
            .collect()
    }
    pub fn font_keys(&self) -> BTreeSet<&str> {
        self.theme
            .iter()
            .flat_map(|theme| theme.font_assets())
            .collect()
    }
}
