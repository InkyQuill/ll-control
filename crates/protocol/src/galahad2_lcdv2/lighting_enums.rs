//! Wire-format enums used by the lighting subsystem.
//!
//! Discriminants are part of the wire format — never reorder. Sources:
//! - `Products/Galahad2Vision/FanLightingMode.cs`
//! - `Products/Galahad2Vision/PumpLightingMode.cs`
//! - `Products/Galahad2Vision/LightingDirection.cs`
//! - `Products/Galahad2Vision/ARGBSignalSource.cs`
//! - `Products/Galahad2Vision/PumpLightingScope.cs`
//!
//! Each enum has a `from_u8` helper that maps unknown bytes to `Unknown`
//! (where present) or the defining variant. This mirrors the C# pattern
//! `Enum.IsDefined(...) ? value : <fallback>` used in the lighting-info
//! parsers and lets us tolerate firmware revisions that introduce new
//! modes the Linux client doesn't yet know about.

/// Lighting modes available on the fan ring (7 variants).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FanLightingMode {
    /// Unknown / firmware-introduced mode the client doesn't recognise.
    Unknown = 0,
    /// Rainbow cycle.
    Rainbow = 1,
    /// Rainbow with morphing transitions.
    RainbowMorph = 2,
    /// Solid static colour from `Color1`.
    StaticColor = 3,
    /// Breathing animation cycling through `Color1..Color4`.
    Breathing = 4,
    /// Runway / chase pattern.
    Runway = 5,
    /// Meteor effect.
    Meteor = 6,
}

impl FanLightingMode {
    /// Map a byte to a known mode, returning [`Self::Unknown`] for any
    /// out-of-range value.
    pub fn from_u8(b: u8) -> Self {
        match b {
            1 => Self::Rainbow,
            2 => Self::RainbowMorph,
            3 => Self::StaticColor,
            4 => Self::Breathing,
            5 => Self::Runway,
            6 => Self::Meteor,
            _ => Self::Unknown,
        }
    }
}

/// Lighting modes available on the pump cap (17 variants — much richer than fans).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PumpLightingMode {
    /// Unknown / firmware-introduced mode.
    Unknown = 0,
    /// Rainbow cycle.
    Rainbow = 1,
    /// Rainbow with morphing transitions.
    RainbowMorph = 2,
    /// Solid static colour from `Color1`.
    StaticColor = 3,
    /// Breathing animation.
    Breathing = 4,
    /// Runway / chase.
    Runway = 5,
    /// Meteor.
    Meteor = 6,
    /// Ticker-tape scroll.
    TickerTape = 7,
    /// Fluctuation effect.
    Fluctuation = 8,
    /// Transmit pulse.
    Transmit = 9,
    /// Multi-colour starry-night.
    ColorfulStarryNight = 10,
    /// Single-colour starry-night.
    StaticStarryNight = 11,
    /// Voice / audio reactive.
    Voice = 12,
    /// Big-bang burst.
    BigBang = 13,
    /// Burst.
    Burst = 14,
    /// Colours-morph.
    ColorsMorph = 15,
    /// Bounce.
    Bounce = 16,
}

impl PumpLightingMode {
    /// Map a byte to a known mode, returning [`Self::Unknown`] for any
    /// out-of-range value.
    pub fn from_u8(b: u8) -> Self {
        match b {
            1 => Self::Rainbow,
            2 => Self::RainbowMorph,
            3 => Self::StaticColor,
            4 => Self::Breathing,
            5 => Self::Runway,
            6 => Self::Meteor,
            7 => Self::TickerTape,
            8 => Self::Fluctuation,
            9 => Self::Transmit,
            10 => Self::ColorfulStarryNight,
            11 => Self::StaticStarryNight,
            12 => Self::Voice,
            13 => Self::BigBang,
            14 => Self::Burst,
            15 => Self::ColorsMorph,
            16 => Self::Bounce,
            _ => Self::Unknown,
        }
    }
}

/// Direction of motion-based lighting effects (e.g., Runway, Meteor).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LightingDirection {
    /// Rightward (linear) or clockwise (circular).
    RightOrClockwise = 0,
    /// Leftward or counter-clockwise.
    LeftOrCounterClockwise = 1,
    /// Upward.
    Up = 2,
    /// Downward.
    Down = 3,
    /// Spread outward from centre.
    Spreaded = 4,
    /// Gather inward to centre.
    Gathered = 5,
}

/// Source of the ARGB signal driving the LEDs.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArgbSignalSource {
    /// Internal MCU drives lighting from settings.
    Mcu = 0,
    /// Motherboard ARGB header drives lighting.
    Motherboard = 1,
    /// Per-frame data sent by host application.
    Application = 2,
}

/// Which part of the pump cap a `PumpLightingSetting` targets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PumpLightingScope {
    /// Inner LED ring.
    Inner = 0,
    /// Outer LED ring.
    Outer = 1,
    /// Both rings.
    All = 2,
}
