//! String <-> wire-enum conversions used by both the IPC layer (when
//! decoding user input) and the keeper actors (when re-issuing from
//! persisted state). Keeping them here is a single source of truth so
//! the two paths cannot drift on, e.g., the empty-string arm or the
//! lower-cased alias list.
//!
//! Names are standardised on the `parse_*` form (the more idiomatic Rust
//! variant). The IPC server's older `*_from_str` names were retired
//! during consolidation; the body of each parser is the union of the
//! prior call-site arms so neither path loses behaviour.

use llcontrol_ipc::Rgb as IpcRgb;
use llcontrol_protocol::galahad2_lcdv2::lcd::LCDControlMode;
use llcontrol_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection, PumpLightingMode, PumpLightingScope,
};
use llcontrol_protocol::galahad2_lcdv2::Rgb as ProtoRgb;
use tracing::warn;

// `ArgbSignalSource` is re-exported indirectly via the keepers' settings
// struct fields; the consolidation does not currently need a parser for
// it, but we silence the unused-import lint defensively in case a future
// caller wires one in. (Removing the import would force callers to
// re-add it; keeping it is the safer no-op.)
#[allow(dead_code)]
const _ARGB_SIGNAL_SOURCE_KEEPALIVE: Option<ArgbSignalSource> = None;

/// Parse a fan-lighting mode string into the wire enum.
///
/// Unknown values default to [`FanLightingMode::Rainbow`] (the codec's
/// own `FanLightingSetting` default), with a warning. Empty strings are
/// treated the same way (no warning, since the empty string is the
/// default value of the persisted TOML field on first attach).
pub(crate) fn parse_fan_mode(s: &str) -> FanLightingMode {
    match s.to_ascii_lowercase().as_str() {
        "rainbow" => FanLightingMode::Rainbow,
        "rainbow_morph" | "rainbowmorph" => FanLightingMode::RainbowMorph,
        "static" | "static_color" | "staticcolor" => FanLightingMode::StaticColor,
        "breathing" => FanLightingMode::Breathing,
        "runway" => FanLightingMode::Runway,
        "meteor" => FanLightingMode::Meteor,
        "" => FanLightingMode::Rainbow,
        other => {
            warn!(
                "codec_str: unknown fan mode {:?}, defaulting to Rainbow",
                other
            );
            FanLightingMode::Rainbow
        }
    }
}

/// Parse a pump-lighting mode string into the wire enum. Unknown ->
/// [`PumpLightingMode::Rainbow`].
pub(crate) fn parse_pump_mode(s: &str) -> PumpLightingMode {
    match s.to_ascii_lowercase().as_str() {
        "rainbow" => PumpLightingMode::Rainbow,
        "rainbow_morph" | "rainbowmorph" => PumpLightingMode::RainbowMorph,
        "static" | "static_color" | "staticcolor" => PumpLightingMode::StaticColor,
        "breathing" => PumpLightingMode::Breathing,
        "runway" => PumpLightingMode::Runway,
        "meteor" => PumpLightingMode::Meteor,
        "ticker_tape" | "tickertape" => PumpLightingMode::TickerTape,
        "fluctuation" => PumpLightingMode::Fluctuation,
        "transmit" => PumpLightingMode::Transmit,
        "colorful_starry_night" | "colorfulstarrynight" => PumpLightingMode::ColorfulStarryNight,
        "static_starry_night" | "staticstarrynight" => PumpLightingMode::StaticStarryNight,
        "voice" => PumpLightingMode::Voice,
        "big_bang" | "bigbang" => PumpLightingMode::BigBang,
        "burst" => PumpLightingMode::Burst,
        "colors_morph" | "colorsmorph" => PumpLightingMode::ColorsMorph,
        "bounce" => PumpLightingMode::Bounce,
        "" => PumpLightingMode::Rainbow,
        other => {
            warn!(
                "codec_str: unknown pump mode {:?}, defaulting to Rainbow",
                other
            );
            PumpLightingMode::Rainbow
        }
    }
}

/// Parse a direction string into the wire enum. Empty / unknown ->
/// [`LightingDirection::RightOrClockwise`] (the codec's default,
/// matching the C# constructor).
pub(crate) fn parse_direction(s: &str) -> LightingDirection {
    match s.to_ascii_lowercase().as_str() {
        "forward" | "right" | "clockwise" | "right_or_clockwise" => {
            LightingDirection::RightOrClockwise
        }
        "reverse"
        | "left"
        | "counter_clockwise"
        | "counterclockwise"
        | "left_or_counter_clockwise" => LightingDirection::LeftOrCounterClockwise,
        "up" => LightingDirection::Up,
        "down" => LightingDirection::Down,
        "spreaded" | "spread" => LightingDirection::Spreaded,
        "gathered" | "gather" => LightingDirection::Gathered,
        "" => LightingDirection::RightOrClockwise,
        other => {
            warn!(
                "codec_str: unknown direction {:?}, defaulting to RightOrClockwise",
                other
            );
            LightingDirection::RightOrClockwise
        }
    }
}

/// Parse a pump scope string into the wire enum. Unknown / empty ->
/// [`PumpLightingScope::Inner`].
pub(crate) fn parse_pump_scope(s: &str) -> PumpLightingScope {
    match s.to_ascii_lowercase().as_str() {
        "inner" => PumpLightingScope::Inner,
        "outer" => PumpLightingScope::Outer,
        "all" => PumpLightingScope::All,
        "" => PumpLightingScope::Inner,
        other => {
            warn!(
                "codec_str: unknown pump scope {:?}, defaulting to Inner",
                other
            );
            PumpLightingScope::Inner
        }
    }
}

/// Parse an LCD-control-mode string into the wire enum.
///
/// Empty / unknown -> [`LCDControlMode::LocalUI`]. Critically,
/// `LCDTest` and `LCDSetting` are intentionally rejected here as
/// defense in depth: those are firmware-internal modes that lock the
/// panel until explicitly cleared. Both the IPC and the persistence
/// replay paths route through this parser, so a hand-edited TOML or a
/// direct IPC caller cannot re-issue a panel-locking mode on every
/// hot-plug. The fallback warns at `warn!` level so the misuse is
/// observable in the daemon log.
pub(crate) fn parse_lcd_mode(s: &str) -> LCDControlMode {
    match s {
        "LocalUI" | "local_ui" => LCDControlMode::LocalUI,
        "Application" | "application" => LCDControlMode::Application,
        "LocalH264" | "local_h264" => LCDControlMode::LocalH264,
        "LocalAVI" | "local_avi" => LCDControlMode::LocalAVI,
        _ => {
            warn!("ignoring unknown/unsafe LCD mode '{s}'; falling back to LocalUI");
            LCDControlMode::LocalUI
        }
    }
}

/// Parse a `#rrggbb` (or `rrggbb`) hex color into a protocol [`ProtoRgb`].
///
/// On parse failure, returns [`ProtoRgb::BLACK`] and logs a warning. This
/// is the safest no-op default -- a black `color1` paired with
/// `disabled = false` effectively produces no visible artifact for
/// solid-color modes.
pub(crate) fn parse_hex_color(s: &str) -> ProtoRgb {
    let trimmed = s.trim().trim_start_matches('#');
    if trimmed.len() != 6 {
        if !s.is_empty() {
            warn!(
                "codec_str: malformed hex color {:?} (expected #rrggbb), defaulting to black",
                s
            );
        }
        return ProtoRgb::BLACK;
    }
    let parse = |start: usize| u8::from_str_radix(&trimmed[start..start + 2], 16);
    match (parse(0), parse(2), parse(4)) {
        (Ok(r), Ok(g), Ok(b)) => ProtoRgb { r, g, b },
        _ => {
            warn!("codec_str: non-hex color {:?}, defaulting to black", s);
            ProtoRgb::BLACK
        }
    }
}

/// Format an IPC [`IpcRgb`] as a `#rrggbb` hex string for persistence.
pub(crate) fn rgb_to_hex(c: IpcRgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Convert a `0..=100` percentage to the wire value (`0..=4`) the LCDV2
/// firmware expects for `brightness` and `speed` fields. This mirrors
/// verify-galahad's `args.brightness / 25` convention from Phase 0c.
/// Out-of-range percent values clamp at 4.
pub(crate) fn percent_to_wire(percent: u8) -> u8 {
    (percent / 25).min(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fan_mode_empty_is_rainbow() {
        // The empty-string arm was the drift hazard that motivated the
        // consolidation: the IPC layer's old fan_mode_from_str fell
        // through to its `_ =>` arm and emitted Rainbow without the
        // arm being explicit. This pins the behaviour so a future
        // edit cannot regress.
        assert_eq!(parse_fan_mode(""), FanLightingMode::Rainbow);
    }

    #[test]
    fn parse_pump_mode_static_aliases() {
        // Three external spellings (CLI / persisted TOML / IPC JSON
        // could plausibly emit any of these) must collapse to the
        // same wire variant.
        assert_eq!(parse_pump_mode("static"), PumpLightingMode::StaticColor);
        assert_eq!(
            parse_pump_mode("static_color"),
            PumpLightingMode::StaticColor
        );
        assert_eq!(
            parse_pump_mode("staticcolor"),
            PumpLightingMode::StaticColor
        );
    }

    #[test]
    fn parse_lcd_mode_filters_lcdtest() {
        // LCDTest must NOT round-trip through the parser; if it ever
        // does, a direct IPC caller (or a hand-edited TOML on the
        // replay path) could persist a mode that re-locks the panel
        // on every hot-plug.
        assert!(matches!(parse_lcd_mode("LCDTest"), LCDControlMode::LocalUI));
        assert!(matches!(
            parse_lcd_mode("LCDSetting"),
            LCDControlMode::LocalUI
        ));
    }

    #[test]
    fn parse_hex_color_round_trips() {
        let c = parse_hex_color("#c4a8ff");
        assert_eq!(c.r, 0xc4);
        assert_eq!(c.g, 0xa8);
        assert_eq!(c.b, 0xff);
    }

    #[test]
    fn percent_to_wire_clamps() {
        assert_eq!(percent_to_wire(0), 0);
        assert_eq!(percent_to_wire(25), 1);
        assert_eq!(percent_to_wire(100), 4);
        // Defensive: percent > 100 should clamp, not overflow the
        // 0..=4 wire range. Phase 0c found the firmware silently
        // ignores out-of-range bytes -- we still don't want to ship them.
        assert_eq!(percent_to_wire(200), 4);
    }
}
