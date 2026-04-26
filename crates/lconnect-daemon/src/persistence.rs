//! Per-device persistence (Hybrid C from spec section 6).
//!
//! Small device settings (fan/pump PWM, lighting state, LCD config) are
//! persisted as TOML under `$XDG_CONFIG_HOME/lconnectd/devices/<key>.toml`.
//! LCD JPEG bytes are intentionally **not** persisted — they're regenerated
//! per frame upload.
//!
//! Writes are atomic: write to a `.toml.tmp` sibling, fsync, then `rename`
//! over the live file. Unknown `schema_version` values warn and fall back
//! to defaults (forward-compat: a future daemon downgraded to today's
//! version won't crash on a newer-schema file, it'll just reset settings).
//!
//! The debounce wrapper lives at the daemon level and lands in P1D-12;
//! replay-on-spawn lands in P1D-15.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CURRENT_SCHEMA: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSettings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub fan: FanSection,
    #[serde(default)]
    pub pump: PumpSection,
    #[serde(default)]
    pub lighting: LightingSection,
    #[serde(default)]
    pub lcd: LcdSection,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA,
            fan: FanSection::default(),
            pump: PumpSection::default(),
            lighting: LightingSection::default(),
            lcd: LcdSection::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FanSection {
    pub pwm_percent: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PumpSection {
    pub pwm_percent: u8,
    /// "mcu" | "application" — empty means "no preference yet".
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSection {
    pub fan: Option<LightingState>,
    pub pump: Option<PumpLightingState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingState {
    pub mode: String,
    pub brightness: u8,
    pub speed: u8,
    pub direction: String,
    pub color: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PumpLightingState {
    pub scope: String,
    pub mode: String,
    pub brightness: u8,
    pub speed: u8,
    pub direction: String,
    pub color: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LcdSection {
    pub brightness: u8,
    pub rotation: String,
    pub control_mode: String,
}

pub struct PersistenceStore {
    root: PathBuf, // typically $XDG_CONFIG_HOME/lconnectd
}

impl PersistenceStore {
    /// Production constructor: roots at `$XDG_CONFIG_HOME/lconnectd`.
    ///
    /// # Errors
    /// Returns Err when the XDG base-directory lookup fails.
    pub fn new() -> anyhow::Result<Self> {
        let dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        Ok(Self {
            root: dirs.get_config_home(),
        })
    }

    /// Test/embedded constructor: roots at an arbitrary directory.
    pub fn new_with_root(root: PathBuf) -> Self {
        Self { root }
    }

    fn key_path(&self, key: &str) -> PathBuf {
        self.root.join("devices").join(format!("{key}.toml"))
    }

    /// Read settings for `key`. Missing file -> defaults. Unknown
    /// `schema_version` -> warn + defaults.
    ///
    /// # Errors
    /// I/O errors reading the file or TOML parse errors.
    pub fn read(&self, key: &str) -> anyhow::Result<DeviceSettings> {
        let path = self.key_path(key);
        if !path.exists() {
            return Ok(DeviceSettings::default());
        }
        let text = std::fs::read_to_string(&path)?;
        let parsed: DeviceSettings = toml::from_str(&text)?;
        if parsed.schema_version != CURRENT_SCHEMA {
            tracing::warn!(
                "device settings schema_version={} unknown (we ship {}); falling back to defaults",
                parsed.schema_version,
                CURRENT_SCHEMA
            );
            return Ok(DeviceSettings::default());
        }
        Ok(parsed)
    }

    /// Atomic write: serialize, write to `<key>.toml.tmp`, fsync, then
    /// `rename` over `<key>.toml`. The parent `devices/` directory is
    /// created on demand.
    ///
    /// # Errors
    /// I/O errors creating the directory, writing/fsyncing the temp file,
    /// or renaming it; TOML serialization errors.
    pub fn write(&self, key: &str, settings: &DeviceSettings) -> anyhow::Result<()> {
        use std::io::Write;
        let path = self.key_path(key);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("toml.tmp");
        let text = toml::to_string_pretty(settings)?;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}
