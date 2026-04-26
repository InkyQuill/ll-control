//! daemon.toml loading + defaults.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonSection,
    pub poll: PollSection,
    pub lcd: LcdSection,
    pub lighting: LightingSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonSection {
    pub log_level: String,
    pub allow_raw_send: bool,
}
impl Default for DaemonSection {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            allow_raw_send: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct PollSection {
    pub hotplug_ms: u64,
    pub sensor_hz: u32,
}
impl Default for PollSection {
    fn default() -> Self {
        Self {
            hotplug_ms: 500,
            sensor_hz: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct LcdSection {
    pub refresh_ms: u64,
}
impl Default for LcdSection {
    fn default() -> Self {
        Self { refresh_ms: 15_000 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSection {
    pub reissue_ms: u64,
}
impl Default for LightingSection {
    fn default() -> Self {
        Self { reissue_ms: 1_500 }
    }
}

impl Config {
    /// Load from `$XDG_CONFIG_HOME/lconnectd/daemon.toml`, falling back to
    /// defaults when the file is missing.
    ///
    /// # Errors
    /// Returns Err on parse failures (existing-but-invalid file).
    pub fn load() -> anyhow::Result<Self> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("lconnectd")?;
        let path = xdg_dirs.find_config_file("daemon.toml");
        match path {
            Some(p) => {
                let text = std::fs::read_to_string(&p)?;
                Ok(toml::from_str(&text)?)
            }
            None => Ok(Self::default()),
        }
    }
}
