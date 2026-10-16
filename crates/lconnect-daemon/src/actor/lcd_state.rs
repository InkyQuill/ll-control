//! Pure LCD state validation shared by the display actor.

use crate::codec_str::parse_lcd_mode;
use lconnect_ipc::{IpcError, LcdConfig, LcdConfigPatch};
use lconnect_protocol::galahad2_lcdv2::lcd::LCDControlMode;

/// Merge a partial LCD configuration into the current requested configuration.
///
/// # Errors
///
/// Returns [`IpcError::InvalidArgument`] when a supplied field is outside the
/// supported LCD configuration domain.
pub fn merge_lcd_config(
    current: &LcdConfig,
    patch: &LcdConfigPatch,
) -> lconnect_ipc::Result<LcdConfig> {
    let mode = canonical_lcd_mode(patch.mode.as_deref().unwrap_or(&current.mode))?.to_owned();
    let brightness_percent = patch
        .brightness_percent
        .unwrap_or(current.brightness_percent);
    if brightness_percent > 100 {
        return Err(IpcError::InvalidArgument(format!(
            "LCD brightness must be between 0 and 100, got {brightness_percent}"
        )));
    }

    let rotation_degrees = patch.rotation_degrees.unwrap_or(current.rotation_degrees);
    if !matches!(rotation_degrees, 0 | 90 | 180 | 270) {
        return Err(IpcError::InvalidArgument(format!(
            "LCD rotation must be one of 0, 90, 180, or 270 degrees, got {rotation_degrees}"
        )));
    }

    Ok(LcdConfig::new(mode, brightness_percent, rotation_degrees))
}

fn canonical_lcd_mode(mode: &str) -> lconnect_ipc::Result<&'static str> {
    let canonical = match parse_lcd_mode(mode) {
        LCDControlMode::LocalUI if matches!(mode, "LocalUI" | "local_ui") => "LocalUI",
        LCDControlMode::Application if matches!(mode, "Application" | "application") => {
            "Application"
        }
        LCDControlMode::LocalH264 if matches!(mode, "LocalH264" | "local_h264") => "LocalH264",
        LCDControlMode::LocalAVI if matches!(mode, "LocalAVI" | "local_avi") => "LocalAVI",
        _ => {
            return Err(IpcError::InvalidArgument(format!(
                "unsupported LCD mode '{mode}'"
            )))
        }
    };
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LcdConfig {
        LcdConfig::new("Application", 70, 90)
    }

    #[test]
    fn merge_rejects_invalid_retained_mode() {
        assert!(merge_lcd_config(
            &LcdConfig::new("LCDSetting", 70, 0),
            &LcdConfigPatch {
                mode: None,
                brightness_percent: Some(40),
                rotation_degrees: None
            }
        )
        .is_err());
    }

    #[test]
    fn merge_lcd_config_preserves_omitted_fields() {
        let old = config();
        let patch = LcdConfigPatch {
            mode: None,
            brightness_percent: Some(40),
            rotation_degrees: None,
        };

        let next = merge_lcd_config(&old, &patch).unwrap();

        assert_eq!(next.rotation_degrees, 90);
        assert_eq!(next.mode, "Application");
        assert_eq!(next.brightness_percent, 40);
    }

    #[test]
    fn merge_lcd_config_rejects_invalid_patch_without_mutating_current() {
        let old = config();
        let original = old.clone();
        let patch = LcdConfigPatch {
            mode: None,
            brightness_percent: Some(101),
            rotation_degrees: None,
        };

        assert!(merge_lcd_config(&old, &patch).is_err());
        assert_eq!(old, original);
    }

    #[test]
    fn merge_lcd_config_rejects_invalid_rotation() {
        let patch = LcdConfigPatch {
            mode: None,
            brightness_percent: None,
            rotation_degrees: Some(45),
        };

        assert!(merge_lcd_config(&config(), &patch).is_err());
    }

    #[test]
    fn merge_lcd_config_rejects_internal_and_unknown_modes() {
        for mode in ["LCDTest", "LCDSetting", "unknown"] {
            let patch = LcdConfigPatch {
                mode: Some(mode.into()),
                brightness_percent: None,
                rotation_degrees: None,
            };

            assert!(merge_lcd_config(&config(), &patch).is_err(), "mode={mode}");
        }
    }

    #[test]
    fn merge_lcd_config_canonicalizes_supported_aliases() {
        let patch = LcdConfigPatch {
            mode: Some("local_h264".into()),
            brightness_percent: None,
            rotation_degrees: None,
        };

        let next = merge_lcd_config(&config(), &patch).unwrap();

        assert_eq!(next.mode, "LocalH264");
    }
}
