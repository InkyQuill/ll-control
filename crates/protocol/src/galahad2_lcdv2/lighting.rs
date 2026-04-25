//! Lighting subsystem types — `Rgb`, `FanLightingSetting`, `PumpLightingSetting`.
//!
//! `to_bytes()` produces the wire-format payload for `SetFanLighting` /
//! `SetPumpLighting` commands. Sources:
//! - `Products/Galahad2Vision/FanLightingSetting.cs:117-150` (20 bytes)
//! - `Products/Galahad2Vision/PumpLightingSetting.cs:107-141` (19 bytes)

use crate::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection, PumpLightingMode, PumpLightingScope,
};

/// 24-bit RGB color, one byte per channel. Wire layout: `[r, g, b]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Red channel (0..=255).
    pub r: u8,
    /// Green channel (0..=255).
    pub g: u8,
    /// Blue channel (0..=255).
    pub b: u8,
}

impl Rgb {
    /// Black (`#000000`). All zeroes — also the `Default`.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// White (`#FFFFFF`).
    pub const WHITE: Self = Self {
        r: 0xFF,
        g: 0xFF,
        b: 0xFF,
    };
    /// Pure red (`#FF0000`).
    pub const RED: Self = Self {
        r: 0xFF,
        g: 0,
        b: 0,
    };
    /// Pure green (`#00FF00`).
    pub const GREEN: Self = Self {
        r: 0,
        g: 0xFF,
        b: 0,
    };
    /// Pure blue (`#0000FF`).
    pub const BLUE: Self = Self {
        r: 0,
        g: 0,
        b: 0xFF,
    };
}

/// Fan-ring lighting configuration (20 bytes on the wire).
///
/// Field defaults match the C# `FanLightingSetting()` constructor:
/// `Mode = Rainbow`, `Brightness = 2`, `Speed = 2`, all colours black,
/// `Direction = RightOrClockwise`, `Disabled = false`, `Source = Mcu`,
/// `SyncToPump = false`, `NumberOfLED = 24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanLightingSetting {
    /// Lighting mode (effect).
    pub mode: FanLightingMode,
    /// Brightness 0..=`MaxBrightnessValue` (firmware-defined).
    pub brightness: u8,
    /// Animation speed 0..=`MaxSpeedValue` (firmware-defined).
    pub speed: u8,
    /// Primary colour.
    pub color1: Rgb,
    /// Secondary colour.
    pub color2: Rgb,
    /// Tertiary colour.
    pub color3: Rgb,
    /// Quaternary colour.
    pub color4: Rgb,
    /// Direction of motion-based effects.
    pub direction: LightingDirection,
    /// `true` to switch the LEDs off entirely.
    pub disabled: bool,
    /// Where the ARGB signal comes from.
    pub source: ArgbSignalSource,
    /// `true` to mirror pump lighting onto the fans.
    pub sync_to_pump: bool,
    /// Number of LEDs in the fan ring (24 by default; varies by fan SKU).
    pub number_of_led: u8,
}

impl Default for FanLightingSetting {
    fn default() -> Self {
        Self {
            mode: FanLightingMode::Rainbow,
            brightness: 2,
            speed: 2,
            color1: Rgb::BLACK,
            color2: Rgb::BLACK,
            color3: Rgb::BLACK,
            color4: Rgb::BLACK,
            direction: LightingDirection::RightOrClockwise,
            disabled: false,
            source: ArgbSignalSource::Mcu,
            sync_to_pump: false,
            number_of_led: 24,
        }
    }
}

impl FanLightingSetting {
    /// Encode the setting as a 20-byte wire payload.
    ///
    /// Byte layout (mirrors `FanLightingSetting.cs:117-150`):
    /// `[Mode, Brightness, Speed, Color1×3, Color2×3, Color3×3, Color4×3,
    ///  Direction, Disabled, Source, SyncToPump, NumberOfLED]`.
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0] = self.mode as u8;
        b[1] = self.brightness;
        b[2] = self.speed;
        b[3] = self.color1.r;
        b[4] = self.color1.g;
        b[5] = self.color1.b;
        b[6] = self.color2.r;
        b[7] = self.color2.g;
        b[8] = self.color2.b;
        b[9] = self.color3.r;
        b[10] = self.color3.g;
        b[11] = self.color3.b;
        b[12] = self.color4.r;
        b[13] = self.color4.g;
        b[14] = self.color4.b;
        b[15] = self.direction as u8;
        b[16] = u8::from(self.disabled);
        b[17] = self.source as u8;
        b[18] = u8::from(self.sync_to_pump);
        b[19] = self.number_of_led;
        b
    }
}

/// Pump-cap lighting configuration (19 bytes on the wire).
///
/// Differs from [`FanLightingSetting`] in two ways: it has a `Scope`
/// byte at the front (Inner / Outer / All), and it has no `SyncToPump`
/// or `NumberOfLED` fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PumpLightingSetting {
    /// Which LED ring(s) this setting applies to.
    pub scope: PumpLightingScope,
    /// Lighting mode (effect).
    pub mode: PumpLightingMode,
    /// Brightness 0..=`MaxBrightnessValue`.
    pub brightness: u8,
    /// Animation speed 0..=`MaxSpeedValue`.
    pub speed: u8,
    /// Primary colour.
    pub color1: Rgb,
    /// Secondary colour.
    pub color2: Rgb,
    /// Tertiary colour.
    pub color3: Rgb,
    /// Quaternary colour.
    pub color4: Rgb,
    /// Direction of motion-based effects.
    pub direction: LightingDirection,
    /// `true` to switch the pump LEDs off entirely.
    pub disabled: bool,
    /// Where the ARGB signal comes from.
    pub source: ArgbSignalSource,
}

impl Default for PumpLightingSetting {
    fn default() -> Self {
        Self {
            scope: PumpLightingScope::Inner,
            mode: PumpLightingMode::Rainbow,
            brightness: 2,
            speed: 2,
            color1: Rgb::BLACK,
            color2: Rgb::BLACK,
            color3: Rgb::BLACK,
            color4: Rgb::BLACK,
            direction: LightingDirection::RightOrClockwise,
            disabled: false,
            source: ArgbSignalSource::Mcu,
        }
    }
}

impl PumpLightingSetting {
    /// Encode the setting as a 19-byte wire payload.
    ///
    /// Byte layout (mirrors `PumpLightingSetting.cs:107-141`):
    /// `[Scope, Mode, Brightness, Speed, Color1×3, Color2×3, Color3×3,
    ///  Color4×3, Direction, Disabled, Source]`.
    pub fn to_bytes(&self) -> [u8; 19] {
        let mut b = [0u8; 19];
        b[0] = self.scope as u8;
        b[1] = self.mode as u8;
        b[2] = self.brightness;
        b[3] = self.speed;
        b[4] = self.color1.r;
        b[5] = self.color1.g;
        b[6] = self.color1.b;
        b[7] = self.color2.r;
        b[8] = self.color2.g;
        b[9] = self.color2.b;
        b[10] = self.color3.r;
        b[11] = self.color3.g;
        b[12] = self.color3.b;
        b[13] = self.color4.r;
        b[14] = self.color4.g;
        b[15] = self.color4.b;
        b[16] = self.direction as u8;
        b[17] = u8::from(self.disabled);
        b[18] = self.source as u8;
        b
    }
}

/// Parsed `Get*LightingInfo` response.
///
/// Generic over the mode enum so the same shape serves both fan and
/// pump info responses. Use the [`FanLightingInfo`] / [`PumpLightingInfo`]
/// type aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightingInfo<M> {
    /// Modes the device advertises as supported.
    pub modes: Vec<M>,
    /// Maximum value the device accepts for `Speed` in a setting.
    pub max_speed: u8,
    /// Maximum value the device accepts for `Brightness` in a setting.
    pub max_brightness: u8,
}

/// `Get*LightingInfo` parsed for fan settings.
pub type FanLightingInfo = LightingInfo<FanLightingMode>;
/// `Get*LightingInfo` parsed for pump settings.
pub type PumpLightingInfo = LightingInfo<PumpLightingMode>;

/// The escape-byte marker (`0xA5`).
const ESCAPE: u8 = 0xA5;
/// The mode-list terminator that follows the escape byte (`0x5A`).
const TERMINATOR: u8 = 0x5A;

/// Parse a `Get*LightingInfo` response data slice.
///
/// Generic so it serves both fan and pump cases. `from_byte` maps a
/// byte to a mode enum (handling unknowns).
fn parse_lighting_info<M>(
    data: &[u8],
    from_byte: impl Fn(u8) -> M,
) -> crate::Result<LightingInfo<M>> {
    if data.is_empty() {
        return Err(crate::Error::BufferTooShort { need: 1, got: 0 });
    }

    // Mode list is bounded above by `data.len()`; pre-size to avoid reallocations.
    let mut modes: Vec<M> = Vec::with_capacity(data.len());
    let mut escape = false;
    let mut idx = 0usize;
    let mut found_terminator = false;
    while idx < data.len() {
        let b = data[idx];
        idx += 1;
        if escape {
            // C# bug-compat: terminator ends the loop; anything else is
            // silently dropped (escape clears, no mode pushed).
            escape = false;
            if b == TERMINATOR {
                found_terminator = true;
                break;
            }
        } else if b == ESCAPE {
            escape = true;
        } else {
            modes.push(from_byte(b));
        }
    }

    if !found_terminator {
        // Reached end-of-buffer without seeing the A5/5A pair. The C# code
        // does not distinguish this from a too-short trailer — both surface
        // as `length - num < 2`. We re-use BufferTooShort and report the
        // actual remaining length so callers see "had N bytes, never found
        // a terminator" rather than a synthetic 0.
        return Err(crate::Error::BufferTooShort {
            need: 2,
            got: data.len(),
        });
    }

    let trailer_remaining = data.len().saturating_sub(idx);
    if trailer_remaining < 2 {
        return Err(crate::Error::BufferTooShort {
            need: 2,
            got: trailer_remaining,
        });
    }

    Ok(LightingInfo {
        modes,
        max_speed: data[idx],
        max_brightness: data[idx + 1],
    })
}

/// Parse a `GetFanLightingInfo` (cmd 132) response data slice.
pub fn parse_fan_lighting_info(data: &[u8]) -> crate::Result<FanLightingInfo> {
    parse_lighting_info(data, FanLightingMode::from_u8)
}

/// Parse a `GetPumpLightingInfo` (cmd 130) response data slice.
pub fn parse_pump_lighting_info(data: &[u8]) -> crate::Result<PumpLightingInfo> {
    parse_lighting_info(data, PumpLightingMode::from_u8)
}
