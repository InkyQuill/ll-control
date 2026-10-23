//! Plasma panel colors take precedence over the application/window color scheme.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    DarkPanel,
    LightPanel,
}

impl Appearance {
    pub fn image(self) -> tauri::Result<tauri::image::Image<'static>> {
        let bytes: &[u8] = match self {
            Self::DarkPanel => include_bytes!("../icons/tray-on-dark.png"),
            Self::LightPanel => include_bytes!("../icons/tray-on-light.png"),
        };
        tauri::image::Image::from_bytes(bytes)
    }
}

fn entry(text: &str, group: &str, key: &str) -> Option<String> {
    let mut selected = false;
    let mut value = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            selected = line == format!("[{group}]");
        } else if selected {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    value = Some(v.trim().to_owned());
                }
            }
        }
    }
    value
}

fn colors(path: &Path) -> Option<Appearance> {
    let text = std::fs::read_to_string(path).ok()?;
    let rgb = entry(&text, "Colors:Window", "BackgroundNormal")?;
    let channels = rgb
        .split(',')
        .map(|v| v.trim().parse::<u8>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let [r, g, b] = channels.as_slice() else {
        return None;
    };
    let brightness = 299 * u32::from(*r) + 587 * u32::from(*g) + 114 * u32::from(*b);
    Some(if brightness < 128_000 {
        Appearance::DarkPanel
    } else {
        Appearance::LightPanel
    })
}

fn plasma(configs: &[PathBuf], data: &[PathBuf]) -> Option<Appearance> {
    let theme = configs
        .iter()
        .find_map(|config| {
            let text = std::fs::read_to_string(config.join("plasmarc")).ok()?;
            entry(&text, "Theme", "name")
        })
        .unwrap_or_else(|| "default".into());
    let global = || {
        configs
            .iter()
            .find_map(|config| colors(&config.join("kdeglobals")))
    };
    // A theme is a directory name, not a path supplied to the filesystem.
    if theme.is_empty() || theme == "." || theme == ".." || theme.contains(['/', '\\']) {
        return None;
    }
    for root in data {
        let theme_dir = root.join("plasma/desktoptheme").join(&theme);
        if theme_dir.is_dir() {
            // Adaptive themes (including Breeze default) use the global palette
            // when their package doesn't supply its own colors file.
            return colors(&theme_dir.join("colors")).or_else(global);
        }
    }
    global()
}

/// Read on a worker thread. Polling also catches custom theme palette edits and
/// desktop-theme changes that don't emit a window ThemeChanged event.
pub fn detect_plasma() -> Option<Appearance> {
    if !std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|v| v.eq_ignore_ascii_case("KDE"))
    {
        return None;
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let mut data = vec![std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))];
    data.extend(
        std::env::split_paths(
            &std::env::var_os("XDG_DATA_DIRS")
                .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
        )
        .filter(|p| p.is_absolute()),
    );
    let mut configs = vec![config];
    configs.extend(
        std::env::split_paths(
            &std::env::var_os("XDG_CONFIG_DIRS").unwrap_or_else(|| "/etc/xdg".into()),
        )
        .filter(|p| p.is_absolute()),
    );
    plasma(&configs, &data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_palette_overrides_windows_and_follows_theme_changes() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let data = dir.path().join("data");
        std::fs::create_dir_all(&config).unwrap();
        for (name, rgb) in [("dark", "20,25,30"), ("light", "240,240,240")] {
            let theme = data.join("plasma/desktoptheme").join(name);
            std::fs::create_dir_all(&theme).unwrap();
            std::fs::write(
                theme.join("colors"),
                format!("[Colors:Window]\nBackgroundNormal={rgb}\n"),
            )
            .unwrap();
        }
        std::fs::write(
            config.join("kdeglobals"),
            "[Colors:Window]\nBackgroundNormal=250,250,250\n",
        )
        .unwrap();
        let roots = [data];
        let system = dir.path().join("system-config");
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(system.join("plasmarc"), "[Theme]\nname=dark\n").unwrap();
        assert_eq!(
            plasma(&[config.clone(), system.clone()], &roots),
            Some(Appearance::DarkPanel)
        );
        for (theme, expected) in [
            ("dark", Appearance::DarkPanel),
            ("light", Appearance::LightPanel),
            ("dark", Appearance::DarkPanel),
        ] {
            std::fs::write(config.join("plasmarc"), format!("[Theme]\nname={theme}\n")).unwrap();
            assert_eq!(
                plasma(std::slice::from_ref(&config), &roots),
                Some(expected)
            );
        }
        std::fs::remove_file(roots[0].join("plasma/desktoptheme/dark/colors")).unwrap();
        assert_eq!(
            plasma(std::slice::from_ref(&config), &roots),
            Some(Appearance::LightPanel)
        );
        std::fs::write(
            config.join("kdeglobals"),
            "[Colors:Window]\nBackgroundNormal=bad\n",
        )
        .unwrap();
        assert_eq!(plasma(std::slice::from_ref(&config), &roots), None);
    }

    #[test]
    fn packaged_icons_decode_with_same_size() {
        for appearance in [Appearance::DarkPanel, Appearance::LightPanel] {
            let image = appearance.image().unwrap();
            assert_eq!((image.width(), image.height()), (64, 64));
        }
    }
}
