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
    /// Load from `$XDG_CONFIG_HOME/llcontrold/daemon.toml`, falling back to
    /// defaults when the file is missing.
    ///
    /// # Errors
    /// Returns Err when an existing file cannot be read or parsed.
    pub fn load() -> anyhow::Result<Self> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("llcontrold")?;
        let path = xdg_dirs.find_config_file("daemon.toml");
        Self::load_optional(path.as_deref())
    }

    fn load_optional(path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        use anyhow::Context;

        match path {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("failed to read configuration {}", path.display()))?;
                toml::from_str(&text)
                    .with_context(|| format!("failed to parse configuration {}", path.display()))
            }
            None => Ok(Self::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn malformed_config_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("daemon.toml");
        std::fs::write(&path, "invalid toml").unwrap();

        let error = Config::load_optional(Some(&path)).unwrap_err();

        assert!(error.to_string().contains("failed to parse configuration"));
    }

    #[test]
    fn unreadable_config_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("daemon.toml");
        std::fs::create_dir(&path).unwrap();

        let error = Config::load_optional(Some(&path)).unwrap_err();

        assert!(error.to_string().contains("failed to read configuration"));
    }

    #[test]
    fn unsupported_config_returns_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("daemon.toml");
        std::fs::write(&path, "[poll]\nsensor_hz = \"fast\"\n").unwrap();

        let error = Config::load_optional(Some(&path)).unwrap_err();

        assert!(error.to_string().contains("failed to parse configuration"));
    }

    #[test]
    fn missing_config_returns_defaults() {
        let config = Config::load_optional(None).unwrap();

        assert_eq!(config.daemon.log_level, "info");
        assert!(!config.daemon.allow_raw_send);
        assert_eq!(config.poll.hotplug_ms, 500);
        assert_eq!(config.poll.sensor_hz, 1);
        assert_eq!(config.lcd.refresh_ms, 15_000);
        assert_eq!(config.lighting.reissue_ms, 1_500);
    }
}
