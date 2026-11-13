//! Versioned manual RGB controls/defaults from Windows 2.1.28 LCDV2 profiles.
use crate::{LightingConfig, LightingKind, Rgb};
use serde::{Deserialize, Serialize};

pub const FAN_LIGHTING_MODES: &[&str] = &[
    "rainbow",
    "rainbow_morph",
    "static",
    "breathing",
    "runway",
    "meteor",
];
pub const PUMP_LIGHTING_MODES: &[&str] = &[
    "rainbow",
    "rainbow_morph",
    "static",
    "breathing",
    "runway",
    "meteor",
    "ticker_tape",
    "fluctuation",
    "transmit",
    "colorful_starry_night",
    "static_starry_night",
    "voice",
    "big_bang",
    "burst",
    "colors_morph",
    "bounce",
];

pub fn canonical_lighting_mode(mode: &str) -> &str {
    if mode == "static_color" {
        "static"
    } else {
        mode
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingModeControls {
    pub mode: String,
    pub color_count: usize,
    pub speed: bool,
    pub brightness: bool,
    pub direction: bool,
    pub defaults: LightingConfig,
}

pub fn lighting_catalog(kind: LightingKind) -> Vec<LightingModeControls> {
    let modes = match kind {
        LightingKind::Fan => FAN_LIGHTING_MODES,
        LightingKind::Pump => PUMP_LIGHTING_MODES,
    };
    modes
        .iter()
        .map(|&mode| {
            let (count, speed) =
                crate::dynamic_rgb_controls(mode).expect("catalog modes have controls");
            let direction = match kind {
                LightingKind::Fan => matches!(mode, "rainbow" | "meteor"),
                _ => matches!(mode, "rainbow" | "meteor" | "ticker_tape" | "transmit"),
            };
            let color = |r, g, b| Rgb { r, g, b };
            let colors = if count == 0 {
                Vec::new()
            } else if kind == LightingKind::Fan {
                [
                    color(0, 215, 255),
                    color(255, 0, 0),
                    color(0, 255, 0),
                    color(150, 255, 0),
                ][..count]
                    .to_vec()
            } else if mode == "static_starry_night" {
                vec![color(255, 255, 255)]
            } else {
                [
                    color(255, 0, 0),
                    color(0, 0, 255),
                    color(0, 255, 0),
                    color(255, 255, 0),
                ][..count]
                    .to_vec()
            };
            let mut defaults =
                LightingConfig::new(mode, colors.first().copied().unwrap_or(color(0, 0, 0)));
            defaults.additional_colors = colors.into_iter().skip(1).collect();
            defaults.speed_percent = if !speed {
                0
            } else if mode == "runway" {
                50
            } else {
                75
            };
            if kind == LightingKind::Pump {
                defaults.scope = Some("inner".into());
            }
            LightingModeControls {
                mode: mode.into(),
                color_count: count,
                speed,
                brightness: true,
                direction,
                defaults,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanLedSize {
    #[serde(rename = "280")]
    Mm280,
    #[default]
    #[serde(rename = "360")]
    Mm360,
}

/// Global fan topology: manual count is retained while sync uses the selected SKU.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanLightingTopology {
    pub sync_to_pump: bool,
    pub size: FanLedSize,
    pub manual_led_count: u8,
}
impl Default for FanLightingTopology {
    fn default() -> Self {
        Self {
            sync_to_pump: false,
            size: FanLedSize::Mm360,
            manual_led_count: 24,
        }
    }
}
impl FanLightingTopology {
    pub fn validate(&self) -> crate::Result<()> {
        if !(8..=50).contains(&self.manual_led_count) {
            return Err(crate::IpcError::InvalidArgument(
                "manual fan LED count must be8..50".into(),
            ));
        }
        Ok(())
    }
    pub fn effective_led_count(&self) -> u8 {
        if self.sync_to_pump {
            match self.size {
                FanLedSize::Mm280 => 16,
                FanLedSize::Mm360 => 24,
            }
        } else {
            self.manual_led_count
        }
    }
}
