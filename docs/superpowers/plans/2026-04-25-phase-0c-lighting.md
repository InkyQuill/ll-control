# Phase 0c — Lighting Subsystem Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify the four Galahad II LCDV2 lighting commands end-to-end against live hardware: `GetPumpLightingInfo` (130), `SetPumpLighting` (131), `GetFanLightingInfo` (132), `SetFanLighting` (133). Land the supporting types — five enums, two setting structs with `to_bytes()` methods, and two info parsers using the device's escape-byte protocol.

**Architecture:** Continue the Phase 0a/0b pattern — pure `crates/protocol/galahad2_lcdv2/` codecs with golden-byte tests, exercised live via `verify-galahad`. New subsystem-level idioms: a small `Rgb { r, g, b }` value type shared between fan and pump settings; a `LightingInfo<Mode>` shape that's parameterised on its mode enum (the device's escape-byte parsing algorithm is identical for fan and pump — only the mode enum differs). The 19/20-byte setting payloads fit in one `ACommandPacket`; no `BCommandPacket` work yet (deferred to Phase 0d).

**Tech Stack:** Rust 1.92 / Edition 2021. Same workspace and dependencies as Phase 0a/0b. No new crates.

**Source-of-truth references** (all paths relative to `/home/inky/Development/lian-li/L-Connect.Core/LConnectCore/`):
- `Products/Galahad2Vision/FanLightingMode.cs` (7 variants), `PumpLightingMode.cs` (17), `LightingDirection.cs` (6), `ARGBSignalSource.cs` (3), `PumpLightingScope.cs` (3)
- `Products/Galahad2Vision/FanLightingSetting.cs:117-150` (`ToBytes`), `PumpLightingSetting.cs:107-141` (`ToBytes`)
- `Products/Galahad2Vision/FanLightingInfo.cs:21-67` (`TryParse`), `PumpLightingInfo.cs:21-67` (`TryParse`)
- `Products/Galahad2Vision/Galahad2VisionDevice.cs:73-86` (`GetFanLightingInfo`), `:122-135` (`GetPumpLightingInfo`), `:269-272` (`SetFanLighting`), `:310-313` (`SetPumpLighting`)

All paths in this plan are relative to `/home/inky/Development/lian-li/lconnect-linux/`.

Phase 0b ended at **36 tests, 14 commits**. Phase 0c targets **~52 tests, ~13 commits**. Final state: 7 verified commands across the device control surface, plus the lighting type system needed by the GUI in later phases.

---

## Part A — Lighting types (pure, no I/O)

### Task 1: Add the five lighting enums

`FanLightingMode`, `PumpLightingMode`, `LightingDirection`, `ARGBSignalSource`, `PumpLightingScope` are all `#[repr(u8)]` enums whose discriminants are wire-format and must never be reordered. They go in one file because they share the lighting subsystem and are small.

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/src/galahad2_lcdv2/lighting_enums.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_lighting_enums.rs`

- [ ] **Step 1: Module wiring**

Append to `crates/protocol/src/galahad2_lcdv2/mod.rs`:

```rust

pub mod lighting_enums;
```

- [ ] **Step 2: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_lighting_enums.rs`:

```rust
//! Tests for the lighting enums.
//!
//! Wire-format discriminants from FanLightingMode.cs, PumpLightingMode.cs,
//! LightingDirection.cs, ARGBSignalSource.cs, PumpLightingScope.cs.

use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection,
    PumpLightingMode, PumpLightingScope,
};

#[test]
fn fan_lighting_mode_discriminants() {
    assert_eq!(FanLightingMode::Unknown as u8, 0);
    assert_eq!(FanLightingMode::Rainbow as u8, 1);
    assert_eq!(FanLightingMode::RainbowMorph as u8, 2);
    assert_eq!(FanLightingMode::StaticColor as u8, 3);
    assert_eq!(FanLightingMode::Breathing as u8, 4);
    assert_eq!(FanLightingMode::Runway as u8, 5);
    assert_eq!(FanLightingMode::Meteor as u8, 6);
}

#[test]
fn pump_lighting_mode_discriminants() {
    assert_eq!(PumpLightingMode::Unknown as u8, 0);
    assert_eq!(PumpLightingMode::Rainbow as u8, 1);
    assert_eq!(PumpLightingMode::RainbowMorph as u8, 2);
    assert_eq!(PumpLightingMode::StaticColor as u8, 3);
    assert_eq!(PumpLightingMode::Breathing as u8, 4);
    assert_eq!(PumpLightingMode::Runway as u8, 5);
    assert_eq!(PumpLightingMode::Meteor as u8, 6);
    assert_eq!(PumpLightingMode::TickerTape as u8, 7);
    assert_eq!(PumpLightingMode::Fluctuation as u8, 8);
    assert_eq!(PumpLightingMode::Transmit as u8, 9);
    assert_eq!(PumpLightingMode::ColorfulStarryNight as u8, 10);
    assert_eq!(PumpLightingMode::StaticStarryNight as u8, 11);
    assert_eq!(PumpLightingMode::Voice as u8, 12);
    assert_eq!(PumpLightingMode::BigBang as u8, 13);
    assert_eq!(PumpLightingMode::Burst as u8, 14);
    assert_eq!(PumpLightingMode::ColorsMorph as u8, 15);
    assert_eq!(PumpLightingMode::Bounce as u8, 16);
}

#[test]
fn lighting_direction_discriminants() {
    assert_eq!(LightingDirection::RightOrClockwise as u8, 0);
    assert_eq!(LightingDirection::LeftOrCounterClockwise as u8, 1);
    assert_eq!(LightingDirection::Up as u8, 2);
    assert_eq!(LightingDirection::Down as u8, 3);
    assert_eq!(LightingDirection::Spreaded as u8, 4);
    assert_eq!(LightingDirection::Gathered as u8, 5);
}

#[test]
fn argb_signal_source_discriminants() {
    assert_eq!(ArgbSignalSource::Mcu as u8, 0);
    assert_eq!(ArgbSignalSource::Motherboard as u8, 1);
    assert_eq!(ArgbSignalSource::Application as u8, 2);
}

#[test]
fn pump_lighting_scope_discriminants() {
    assert_eq!(PumpLightingScope::Inner as u8, 0);
    assert_eq!(PumpLightingScope::Outer as u8, 1);
    assert_eq!(PumpLightingScope::All as u8, 2);
}

#[test]
fn fan_lighting_mode_from_u8() {
    assert_eq!(FanLightingMode::from_u8(1), FanLightingMode::Rainbow);
    assert_eq!(FanLightingMode::from_u8(0), FanLightingMode::Unknown);
    assert_eq!(FanLightingMode::from_u8(99), FanLightingMode::Unknown);
}

#[test]
fn pump_lighting_mode_from_u8() {
    assert_eq!(PumpLightingMode::from_u8(16), PumpLightingMode::Bounce);
    assert_eq!(PumpLightingMode::from_u8(0), PumpLightingMode::Unknown);
    assert_eq!(PumpLightingMode::from_u8(99), PumpLightingMode::Unknown);
}
```

- [ ] **Step 3: Confirm RED**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_lighting_enums 2>&1 | tail -10
```

Expected: compile error mentioning `lighting_enums` module / missing types.

- [ ] **Step 4: Implement the enums**

Create `crates/protocol/src/galahad2_lcdv2/lighting_enums.rs`:

```rust
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
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `43 passed; 0 failed` (36 from Phase 0b + 7 new).

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/lighting_enums.rs \
        crates/protocol/tests/galahad2_lcdv2_lighting_enums.rs
git commit -m "feat(protocol): add lighting enums for Galahad II LCDV2

Five #[repr(u8)] #[non_exhaustive] enums whose discriminants are the
wire format: FanLightingMode (7), PumpLightingMode (17),
LightingDirection (6), ArgbSignalSource (3), PumpLightingScope (3).
Each mode enum gets a from_u8 helper that maps unknown bytes to
Unknown — mirrors the C# Enum.IsDefined fallback used by the
LightingInfo parsers and tolerates firmware-introduced modes.

7 tests covering all 36 discriminant values + the from_u8 fallbacks."
```

---

### Task 2: Add the `Rgb` color type and `FanLightingSetting`

`Rgb` is a tiny shared value type used by both lighting settings. Bundling it with `FanLightingSetting` (which is the simpler of the two — fan settings have no scope byte) keeps the first setting struct compact.

**Files:**
- Create: `crates/protocol/src/galahad2_lcdv2/lighting.rs` (new — both Rgb and FanLightingSetting)
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_fan_lighting.rs`

- [ ] **Step 1: Module wiring**

Append to `crates/protocol/src/galahad2_lcdv2/mod.rs`:

```rust

pub mod lighting;
```

- [ ] **Step 2: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_fan_lighting.rs`:

```rust
//! Tests for Rgb color and FanLightingSetting.
//!
//! Source: Products/Galahad2Vision/FanLightingSetting.cs:117-150
//! (`ToBytes` returns 20 bytes).

use lconnect_protocol::galahad2_lcdv2::lighting::{FanLightingSetting, Rgb};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection,
};

#[test]
fn rgb_constructs_with_named_components() {
    let c = Rgb { r: 0x10, g: 0x20, b: 0x30 };
    assert_eq!(c.r, 0x10);
    assert_eq!(c.g, 0x20);
    assert_eq!(c.b, 0x30);
}

#[test]
fn rgb_const_helpers() {
    assert_eq!(Rgb::BLACK, Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(Rgb::WHITE, Rgb { r: 0xFF, g: 0xFF, b: 0xFF });
    assert_eq!(Rgb::RED,   Rgb { r: 0xFF, g: 0,    b: 0    });
    assert_eq!(Rgb::GREEN, Rgb { r: 0,    g: 0xFF, b: 0    });
    assert_eq!(Rgb::BLUE,  Rgb { r: 0,    g: 0,    b: 0xFF });
}

#[test]
fn fan_lighting_setting_default_matches_csharp() {
    // C# constructor sets Mode=Rainbow, Brightness=2, Speed=2,
    // Colors=BLACK, Direction=RightOrClockwise, Disabled=false,
    // Source=MCU, SyncToPump=false, NumberOfLED=24.
    let s = FanLightingSetting::default();
    assert_eq!(s.mode, FanLightingMode::Rainbow);
    assert_eq!(s.brightness, 2);
    assert_eq!(s.speed, 2);
    assert_eq!(s.color1, Rgb::BLACK);
    assert_eq!(s.color2, Rgb::BLACK);
    assert_eq!(s.color3, Rgb::BLACK);
    assert_eq!(s.color4, Rgb::BLACK);
    assert_eq!(s.direction, LightingDirection::RightOrClockwise);
    assert!(!s.disabled);
    assert_eq!(s.source, ArgbSignalSource::Mcu);
    assert!(!s.sync_to_pump);
    assert_eq!(s.number_of_led, 24);
}

#[test]
fn fan_lighting_setting_to_bytes_default() {
    // Default setting -> 20 bytes:
    //   [Mode=1 Rainbow] [Bright=2] [Speed=2] [C1 0,0,0] [C2 0,0,0]
    //   [C3 0,0,0] [C4 0,0,0] [Dir=0] [Disabled=0] [Source=0 Mcu]
    //   [Sync=0] [NumLED=24]
    let bytes = FanLightingSetting::default().to_bytes();
    assert_eq!(bytes.len(), 20);
    assert_eq!(bytes[0], 1, "Mode");
    assert_eq!(bytes[1], 2, "Brightness");
    assert_eq!(bytes[2], 2, "Speed");
    for i in 3..15 {
        assert_eq!(bytes[i], 0, "Color RGB byte at offset {i}");
    }
    assert_eq!(bytes[15], 0, "Direction");
    assert_eq!(bytes[16], 0, "Disabled");
    assert_eq!(bytes[17], 0, "Source");
    assert_eq!(bytes[18], 0, "SyncToPump");
    assert_eq!(bytes[19], 24, "NumberOfLED");
}

#[test]
fn fan_lighting_setting_to_bytes_static_red_at_50_percent() {
    let s = FanLightingSetting {
        mode: FanLightingMode::StaticColor,
        brightness: 50,
        speed: 0,
        color1: Rgb::RED,
        color2: Rgb::BLACK,
        color3: Rgb::BLACK,
        color4: Rgb::BLACK,
        direction: LightingDirection::RightOrClockwise,
        disabled: false,
        source: ArgbSignalSource::Mcu,
        sync_to_pump: false,
        number_of_led: 24,
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 3, "Mode = StaticColor");
    assert_eq!(bytes[1], 50, "Brightness");
    assert_eq!(bytes[3], 0xFF, "Color1.R");
    assert_eq!(bytes[4], 0, "Color1.G");
    assert_eq!(bytes[5], 0, "Color1.B");
    assert_eq!(bytes[19], 24, "NumberOfLED");
}

#[test]
fn fan_lighting_setting_to_bytes_disabled_and_app_source() {
    let s = FanLightingSetting {
        disabled: true,
        source: ArgbSignalSource::Application,
        ..FanLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[16], 1, "Disabled = true → 1");
    assert_eq!(bytes[17], 2, "Source = Application → 2");
}
```

- [ ] **Step 3: Confirm RED**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_fan_lighting 2>&1 | tail -10
```

Expected: compile error.

- [ ] **Step 4: Implement `lighting.rs`**

Create `crates/protocol/src/galahad2_lcdv2/lighting.rs`:

```rust
//! Lighting subsystem types — `Rgb`, `FanLightingSetting`, `PumpLightingSetting`.
//!
//! `to_bytes()` produces the wire-format payload for `SetFanLighting` /
//! `SetPumpLighting` commands. Sources:
//! - `Products/Galahad2Vision/FanLightingSetting.cs:117-150` (20 bytes)
//! - `Products/Galahad2Vision/PumpLightingSetting.cs:107-141` (19 bytes)

use crate::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, FanLightingMode, LightingDirection,
    PumpLightingMode, PumpLightingScope,
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
    pub const WHITE: Self = Self { r: 0xFF, g: 0xFF, b: 0xFF };
    /// Pure red (`#FF0000`).
    pub const RED: Self = Self { r: 0xFF, g: 0, b: 0 };
    /// Pure green (`#00FF00`).
    pub const GREEN: Self = Self { r: 0, g: 0xFF, b: 0 };
    /// Pure blue (`#0000FF`).
    pub const BLUE: Self = Self { r: 0, g: 0, b: 0xFF };
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
    /// ```text
    /// [0]    Mode             (FanLightingMode)
    /// [1]    Brightness       (u8)
    /// [2]    Speed            (u8)
    /// [3..6]  Color1           (RGB)
    /// [6..9]  Color2           (RGB)
    /// [9..12] Color3           (RGB)
    /// [12..15] Color4          (RGB)
    /// [15]   Direction        (LightingDirection)
    /// [16]   Disabled         (bool → 0/1)
    /// [17]   Source           (ArgbSignalSource)
    /// [18]   SyncToPump       (bool → 0/1)
    /// [19]   NumberOfLED      (u8)
    /// ```
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0] = self.mode as u8;
        b[1] = self.brightness;
        b[2] = self.speed;
        b[3] = self.color1.r;  b[4] = self.color1.g;  b[5] = self.color1.b;
        b[6] = self.color2.r;  b[7] = self.color2.g;  b[8] = self.color2.b;
        b[9] = self.color3.r;  b[10] = self.color3.g; b[11] = self.color3.b;
        b[12] = self.color4.r; b[13] = self.color4.g; b[14] = self.color4.b;
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
    /// ```text
    /// [0]    Scope            (PumpLightingScope)
    /// [1]    Mode             (PumpLightingMode)
    /// [2]    Brightness       (u8)
    /// [3]    Speed            (u8)
    /// [4..7]  Color1           (RGB)
    /// [7..10] Color2           (RGB)
    /// [10..13] Color3          (RGB)
    /// [13..16] Color4          (RGB)
    /// [16]   Direction        (LightingDirection)
    /// [17]   Disabled         (bool → 0/1)
    /// [18]   Source           (ArgbSignalSource)
    /// ```
    pub fn to_bytes(&self) -> [u8; 19] {
        let mut b = [0u8; 19];
        b[0] = self.scope as u8;
        b[1] = self.mode as u8;
        b[2] = self.brightness;
        b[3] = self.speed;
        b[4] = self.color1.r;  b[5] = self.color1.g;  b[6] = self.color1.b;
        b[7] = self.color2.r;  b[8] = self.color2.g;  b[9] = self.color2.b;
        b[10] = self.color3.r; b[11] = self.color3.g; b[12] = self.color3.b;
        b[13] = self.color4.r; b[14] = self.color4.g; b[15] = self.color4.b;
        b[16] = self.direction as u8;
        b[17] = u8::from(self.disabled);
        b[18] = self.source as u8;
        b
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `49 passed; 0 failed` (43 from Task 1 + 6 new for fan lighting).

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/lighting.rs \
        crates/protocol/tests/galahad2_lcdv2_fan_lighting.rs
git commit -m "feat(protocol): add Rgb + FanLightingSetting + PumpLightingSetting

Three pure types: Rgb (24-bit color with const helpers BLACK/WHITE/
RED/GREEN/BLUE), FanLightingSetting (20-byte wire payload with Default
matching the C# constructor), PumpLightingSetting (19-byte wire payload
with a leading Scope byte). Both settings expose to_bytes() that
produces the exact byte sequence the device's SetFanLighting (cmd 133)
and SetPumpLighting (cmd 131) commands accept.

6 fan-side tests this commit (Rgb + FanLightingSetting); pump tests
land in Task 3 alongside its codec."
```

---

### Task 3: Test `PumpLightingSetting::to_bytes`

The struct itself was added in Task 2 (it's small and lives next to FanLightingSetting in `lighting.rs`). This task adds the dedicated tests.

**Files:**
- Create: `crates/protocol/tests/galahad2_lcdv2_pump_lighting.rs`

- [ ] **Step 1: Write failing tests** (they'll actually pass since `lighting.rs` already has `PumpLightingSetting`, but we still TDD the *test side* to lock the byte layout)

Create `crates/protocol/tests/galahad2_lcdv2_pump_lighting.rs`:

```rust
//! Tests for PumpLightingSetting::to_bytes.
//!
//! Source: Products/Galahad2Vision/PumpLightingSetting.cs:107-141
//! (`ToBytes` returns 19 bytes — note Scope byte at offset 0, no
//! SyncToPump or NumberOfLED).

use lconnect_protocol::galahad2_lcdv2::lighting::{PumpLightingSetting, Rgb};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    ArgbSignalSource, LightingDirection, PumpLightingMode, PumpLightingScope,
};

#[test]
fn pump_lighting_setting_default_matches_csharp() {
    let s = PumpLightingSetting::default();
    assert_eq!(s.scope, PumpLightingScope::Inner);
    assert_eq!(s.mode, PumpLightingMode::Rainbow);
    assert_eq!(s.brightness, 2);
    assert_eq!(s.speed, 2);
    assert_eq!(s.color1, Rgb::BLACK);
    assert_eq!(s.direction, LightingDirection::RightOrClockwise);
    assert!(!s.disabled);
    assert_eq!(s.source, ArgbSignalSource::Mcu);
}

#[test]
fn pump_lighting_setting_to_bytes_default() {
    // Default -> 19 bytes:
    //   [Scope=0 Inner] [Mode=1 Rainbow] [Bright=2] [Speed=2]
    //   [C1..C4 black] [Dir=0] [Disabled=0] [Source=0 Mcu]
    let bytes = PumpLightingSetting::default().to_bytes();
    assert_eq!(bytes.len(), 19);
    assert_eq!(bytes[0], 0, "Scope = Inner");
    assert_eq!(bytes[1], 1, "Mode = Rainbow");
    assert_eq!(bytes[2], 2, "Brightness");
    assert_eq!(bytes[3], 2, "Speed");
    for i in 4..16 {
        assert_eq!(bytes[i], 0, "Color RGB byte at offset {i}");
    }
    assert_eq!(bytes[16], 0, "Direction");
    assert_eq!(bytes[17], 0, "Disabled");
    assert_eq!(bytes[18], 0, "Source");
}

#[test]
fn pump_lighting_setting_to_bytes_static_blue_outer_at_max() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::Outer,
        mode: PumpLightingMode::StaticColor,
        brightness: 100,
        speed: 0,
        color1: Rgb::BLUE,
        ..PumpLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 1, "Scope = Outer");
    assert_eq!(bytes[1], 3, "Mode = StaticColor");
    assert_eq!(bytes[2], 100, "Brightness");
    assert_eq!(bytes[4], 0, "Color1.R");
    assert_eq!(bytes[5], 0, "Color1.G");
    assert_eq!(bytes[6], 0xFF, "Color1.B");
}

#[test]
fn pump_lighting_setting_to_bytes_all_scope_with_motherboard_source() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::All,
        source: ArgbSignalSource::Motherboard,
        ..PumpLightingSetting::default()
    };
    let bytes = s.to_bytes();
    assert_eq!(bytes[0], 2, "Scope = All → 2");
    assert_eq!(bytes[18], 1, "Source = Motherboard → 1");
}
```

- [ ] **Step 2: Run tests** (these pass on the first try because `lighting.rs` already implements `PumpLightingSetting`)

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_pump_lighting 2>&1 | tail -5
```

Expected: `4 passed; 0 failed`.

- [ ] **Step 3: Run all tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `53 passed; 0 failed`.

- [ ] **Step 4: Commit**

```bash
git add crates/protocol/tests/galahad2_lcdv2_pump_lighting.rs
git commit -m "test(protocol): cover PumpLightingSetting::to_bytes byte layout

PumpLightingSetting was implemented in the previous commit alongside
FanLightingSetting (they share lighting.rs). This commit adds the
dedicated test file covering its 19-byte wire layout: Scope at offset 0,
no SyncToPump/NumberOfLED, four RGB triples at offsets 4..16, Direction/
Disabled/Source at 16/17/18.

4 tests covering default, static-blue-outer-at-max, all-scope with
motherboard source."
```

---

### Task 4: TDD `LightingInfo` parser (escape-byte protocol)

The device's `Get*LightingInfo` response uses an escape-byte protocol that's identical for fan and pump (the only difference is the mode-enum type). We model it as one generic `LightingInfo<M>` type parameterised on the mode enum, so both `FanLightingInfo` and `PumpLightingInfo` are type aliases.

**Algorithm** (mirrors `FanLightingInfo.cs:21-67` and `PumpLightingInfo.cs:21-67`):
- Walk bytes one at a time.
- `0xA5` (165) is an escape marker — set `escape=true` and continue.
- If `escape=true`:
  - `0x5A` (90) terminates the mode list — break.
  - Anything else clears `escape` and is treated as a regular mode byte.
  - **Bug-compatibility note:** the C# code does *not* push the post-escape byte if it's not 0x5A, so an `0xA5 X` sequence (where X != 0x5A) silently drops X. We mirror that behaviour for wire compatibility.
- Otherwise, the byte is a mode discriminant: push to the modes list (mapping unknown values to `Mode::Unknown`).
- After the loop ends (via 0x5A terminator), the *next two bytes* are `MaxSpeed` and `MaxBrightness`.
- If fewer than 2 trailer bytes remain → `Error::BufferTooShort`.

**Files:**
- Modify: `crates/protocol/src/galahad2_lcdv2/lighting.rs` (append `LightingInfo`, `parse_lighting_info`, type aliases)
- Create: `crates/protocol/tests/galahad2_lcdv2_lighting_info.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_lighting_info.rs`:

```rust
//! Tests for LightingInfo<Mode> parser (escape-byte protocol).
//!
//! Source: Products/Galahad2Vision/FanLightingInfo.cs:21-67 and
//! PumpLightingInfo.cs:21-67. The C# parses the data slice with these
//! rules: bytes are mode discriminants until an `0xA5` escape marker is
//! seen; the next byte after `0xA5` is `0x5A` to terminate, or anything
//! else (silently dropped) to clear the escape; after the terminator,
//! two trailer bytes give MaxSpeed and MaxBrightness.

use lconnect_protocol::galahad2_lcdv2::lighting::{
    parse_fan_lighting_info, parse_pump_lighting_info,
    FanLightingInfo, PumpLightingInfo,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    FanLightingMode, PumpLightingMode,
};

#[test]
fn parse_fan_info_simple_modes_list() {
    // Modes [Rainbow=1, StaticColor=3, Meteor=6], terminator a5 5a,
    // trailer [MaxSpeed=4, MaxBrightness=10].
    let data = [0x01, 0x03, 0x06, 0xA5, 0x5A, 0x04, 0x0A];
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(info.modes, vec![
        FanLightingMode::Rainbow,
        FanLightingMode::StaticColor,
        FanLightingMode::Meteor,
    ]);
    assert_eq!(info.max_speed, 4);
    assert_eq!(info.max_brightness, 10);
}

#[test]
fn parse_pump_info_full_mode_list() {
    // All 16 known pump modes (1..=16), then terminator + trailer.
    let mut data = vec![];
    for m in 1u8..=16 {
        data.push(m);
    }
    data.extend_from_slice(&[0xA5, 0x5A, 5, 100]);
    let info = parse_pump_lighting_info(&data).expect("should parse");
    assert_eq!(info.modes.len(), 16);
    assert_eq!(info.modes[0], PumpLightingMode::Rainbow);
    assert_eq!(info.modes[15], PumpLightingMode::Bounce);
    assert_eq!(info.max_speed, 5);
    assert_eq!(info.max_brightness, 100);
}

#[test]
fn parse_fan_info_unknown_byte_becomes_unknown() {
    // 0xFF is not a known FanLightingMode discriminant — must map to Unknown.
    let data = [0x01, 0xFF, 0x06, 0xA5, 0x5A, 1, 1];
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(info.modes, vec![
        FanLightingMode::Rainbow,
        FanLightingMode::Unknown,
        FanLightingMode::Meteor,
    ]);
}

#[test]
fn parse_fan_info_a5_then_non_5a_silently_drops_one_byte() {
    // C# bug-compat: `0xA5 X` where X != 0x5A drops X without pushing.
    // After the dropped byte, parsing continues normally.
    let data = [0x01, 0xA5, 0x77, 0x03, 0xA5, 0x5A, 1, 1];
    //          ^Rainbow    ^dropped ^Static  ^terminate
    let info = parse_fan_lighting_info(&data).expect("should parse");
    assert_eq!(info.modes, vec![
        FanLightingMode::Rainbow,
        FanLightingMode::StaticColor,
    ]);
}

#[test]
fn parse_fan_info_empty_input_rejected() {
    let err = parse_fan_lighting_info(&[]).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { .. }
    ));
}

#[test]
fn parse_fan_info_no_trailer_rejected() {
    // Mode list and terminator but only ONE trailer byte instead of TWO.
    let data = [0x01, 0xA5, 0x5A, 0x04];
    let err = parse_fan_lighting_info(&data).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { need: 2, got: 1 }
    ));
}

#[test]
fn parse_fan_info_no_terminator_rejected() {
    // Modes but never an A5 terminator at all.
    let data = [0x01, 0x02, 0x03, 0x04];
    let err = parse_fan_lighting_info(&data).unwrap_err();
    // Reaches end of buffer with no terminator — surfaces as too-short
    // because we never enter the "read trailer" stage.
    assert!(matches!(
        err,
        lconnect_protocol::Error::BufferTooShort { .. }
    ));
}
```

- [ ] **Step 2: Confirm RED**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_lighting_info 2>&1 | tail -10
```

Expected: compile error (`parse_fan_lighting_info` etc. don't exist).

- [ ] **Step 3: Implement `LightingInfo<M>` and the parser**

Append the following to `crates/protocol/src/galahad2_lcdv2/lighting.rs` (at the end of the file):

```rust

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
    from_byte: fn(u8) -> M,
) -> crate::Result<LightingInfo<M>> {
    if data.is_empty() {
        return Err(crate::Error::BufferTooShort { need: 1, got: 0 });
    }

    let mut modes: Vec<M> = Vec::new();
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
        // We reached end-of-buffer without seeing the A5/5A pair. The C#
        // doesn't distinguish this case from "no trailer bytes" — both
        // surface as `length - num < 2`. We surface as BufferTooShort.
        return Err(crate::Error::BufferTooShort {
            need: 2,
            got: 0,
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `60 passed; 0 failed` (53 + 7 new).

- [ ] **Step 5: Run clippy**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/lighting.rs \
        crates/protocol/tests/galahad2_lcdv2_lighting_info.rs
git commit -m "feat(protocol): add LightingInfo<Mode> + escape-byte parser

Parameterised over the mode enum so one parser serves both
GetFanLightingInfo and GetPumpLightingInfo. Implements the C#
escape-byte protocol from FanLightingInfo.cs:21-67 / PumpLightingInfo
.cs:21-67: bytes are mode discriminants until 0xA5 escape, then 0x5A
terminates and the next two bytes are MaxSpeed and MaxBrightness.

Bug-compatible with C#: an 0xA5 followed by anything other than 0x5A
silently drops the second byte without pushing a mode. Unknown mode
bytes map to Mode::Unknown.

7 tests covering: simple lists, full mode list, unknown bytes,
A5+non-5A drop, empty input, missing trailer, missing terminator."
```

---

## Part B — Codecs

### Task 5: TDD `GetFanLightingInfo` and `GetPumpLightingInfo` codecs

Both are bare-cmd-number requests; responses use an `ACommandPacket` whose data slice is parsed via the Task-4 parser.

**Files:**
- Create: `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_get_lighting.rs`

- [ ] **Step 1: Module wiring**

Append to `crates/protocol/src/galahad2_lcdv2/mod.rs`:

```rust

pub mod get_lighting;
```

- [ ] **Step 2: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_get_lighting.rs`:

```rust
//! Tests for GetFanLightingInfo (cmd 132) + GetPumpLightingInfo (cmd 130).
//!
//! Source: Galahad2VisionDevice.cs:73-86, :122-135.

use lconnect_protocol::galahad2_lcdv2::get_lighting::{
    build_get_fan_lighting_info_request,
    build_get_pump_lighting_info_request,
    parse_fan_lighting_info_response,
    parse_pump_lighting_info_response,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    FanLightingMode, PumpLightingMode,
};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;

#[test]
fn build_get_fan_request_emits_cmd_132_no_payload() {
    let bytes = build_get_fan_lighting_info_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 132);
    assert_eq!(bytes[5], 0, "Length = 0");
}

#[test]
fn build_get_pump_request_emits_cmd_130_no_payload() {
    let bytes = build_get_pump_lighting_info_request();
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 130);
    assert_eq!(bytes[5], 0, "Length = 0");
}

#[test]
fn parse_fan_response_full_round_trip() {
    // Synth response: cmd 132 echo, length 7, payload =
    //   [Rainbow=1, StaticColor=3, Meteor=6, A5, 5A, MaxSpeed=4, MaxBri=10]
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 132;
    buf[5] = 7;
    buf[6..13].copy_from_slice(&[0x01, 0x03, 0x06, 0xA5, 0x5A, 0x04, 0x0A]);

    let info = parse_fan_lighting_info_response(&buf).expect("should parse");
    assert_eq!(info.modes, vec![
        FanLightingMode::Rainbow,
        FanLightingMode::StaticColor,
        FanLightingMode::Meteor,
    ]);
    assert_eq!(info.max_speed, 4);
    assert_eq!(info.max_brightness, 10);
}

#[test]
fn parse_pump_response_full_round_trip() {
    // Synth response: cmd 130 echo, length 5, payload =
    //   [Rainbow=1, Bounce=16, A5, 5A, MaxSpeed=5, MaxBri=100]
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 130;
    buf[5] = 6;
    buf[6..12].copy_from_slice(&[0x01, 0x10, 0xA5, 0x5A, 0x05, 0x64]);

    let info = parse_pump_lighting_info_response(&buf).expect("should parse");
    assert_eq!(info.modes, vec![
        PumpLightingMode::Rainbow,
        PumpLightingMode::Bounce,
    ]);
    assert_eq!(info.max_speed, 5);
    assert_eq!(info.max_brightness, 100);
}

#[test]
fn parse_fan_response_with_wrong_command_echo_rejected() {
    let mut buf = [0u8; PACKET_LEN];
    buf[0] = 0x01;
    buf[1] = 0x86; // 134 — not 132
    buf[5] = 0;
    let err = parse_fan_lighting_info_response(&buf).unwrap_err();
    assert!(matches!(
        err,
        lconnect_protocol::Error::UnexpectedCommand { expected: 132, got: 0x86 }
    ));
}
```

- [ ] **Step 3: Confirm RED**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_get_lighting 2>&1 | tail -10
```

Expected: compile error.

- [ ] **Step 4: Implement `get_lighting.rs`**

Create `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`:

```rust
//! GetFanLightingInfo (cmd 132) + GetPumpLightingInfo (cmd 130).
//!
//! Both commands are bare-cmd-number requests with an `ACommandPacket`
//! response. The response's data slice is parsed via the lighting-info
//! escape-byte parser in `lighting.rs`.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::{
    parse_fan_lighting_info, parse_pump_lighting_info,
    FanLightingInfo, PumpLightingInfo,
};
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID request for `GetFanLightingInfo` (cmd 132).
pub fn build_get_fan_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetFanLightingInfo as u8, &[]).as_bytes()
}

/// Build the 64-byte HID request for `GetPumpLightingInfo` (cmd 130).
pub fn build_get_pump_lighting_info_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetPumpLightingInfo as u8, &[]).as_bytes()
}

/// Parse a `GetFanLightingInfo` response (must echo cmd 132).
pub fn parse_fan_lighting_info_response(buf: &[u8]) -> crate::Result<FanLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetFanLightingInfo as u8)?;
    parse_fan_lighting_info(parsed.data())
}

/// Parse a `GetPumpLightingInfo` response (must echo cmd 130).
pub fn parse_pump_lighting_info_response(buf: &[u8]) -> crate::Result<PumpLightingInfo> {
    let parsed = ACommandPacket::parse_response(buf, Command::GetPumpLightingInfo as u8)?;
    parse_pump_lighting_info(parsed.data())
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `65 passed; 0 failed` (60 + 5 new).

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/get_lighting.rs \
        crates/protocol/tests/galahad2_lcdv2_get_lighting.rs
git commit -m "feat(protocol): add Get{Fan,Pump}LightingInfo codecs

Both commands send a bare-cmd request and parse a single
ACommandPacket response via lighting::parse_{fan,pump}_lighting_info.

5 tests cover request shape (cmd 132/130, no payload) and
round-trip response decode for both fan and pump, plus a
wrong-command-echo rejection."
```

---

### Task 6: TDD `SetFanLighting` and `SetPumpLighting` codecs

Both Set commands take a setting struct, call its `to_bytes()`, and wrap the result in an `ACommandPacket` with the appropriate cmd byte. No response.

**Files:**
- Create: `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`
- Modify: `crates/protocol/src/galahad2_lcdv2/mod.rs`
- Create: `crates/protocol/tests/galahad2_lcdv2_set_lighting.rs`

- [ ] **Step 1: Module wiring**

Append to `crates/protocol/src/galahad2_lcdv2/mod.rs`:

```rust

pub mod set_lighting;
```

- [ ] **Step 2: Write failing tests**

Create `crates/protocol/tests/galahad2_lcdv2_set_lighting.rs`:

```rust
//! Tests for SetFanLighting (cmd 133) + SetPumpLighting (cmd 131).
//!
//! Source: Galahad2VisionDevice.cs:269-272, :310-313.

use lconnect_protocol::galahad2_lcdv2::lighting::{
    FanLightingSetting, PumpLightingSetting, Rgb,
};
use lconnect_protocol::galahad2_lcdv2::lighting_enums::{
    FanLightingMode, PumpLightingMode, PumpLightingScope,
};
use lconnect_protocol::galahad2_lcdv2::packets::PACKET_LEN;
use lconnect_protocol::galahad2_lcdv2::set_lighting::{
    build_set_fan_lighting_request, build_set_pump_lighting_request,
};

#[test]
fn build_set_fan_lighting_default() {
    let bytes = build_set_fan_lighting_request(&FanLightingSetting::default());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 133, "Command = SetFanLighting");
    assert_eq!(bytes[5], 20, "Length = 20 (FanLightingSetting payload size)");
    // First three data bytes: Mode=Rainbow=1, Brightness=2, Speed=2
    assert_eq!(bytes[6], 1);
    assert_eq!(bytes[7], 2);
    assert_eq!(bytes[8], 2);
    assert_eq!(bytes[6 + 19], 24, "NumberOfLED at last data offset");
}

#[test]
fn build_set_fan_lighting_static_red() {
    let s = FanLightingSetting {
        mode: FanLightingMode::StaticColor,
        brightness: 50,
        color1: Rgb::RED,
        ..FanLightingSetting::default()
    };
    let bytes = build_set_fan_lighting_request(&s);
    assert_eq!(bytes[5], 20, "Length = 20");
    assert_eq!(bytes[6], 3, "Mode = StaticColor = 3");
    assert_eq!(bytes[7], 50, "Brightness");
    assert_eq!(bytes[6 + 3], 0xFF, "Color1.R");
    assert_eq!(bytes[6 + 4], 0, "Color1.G");
    assert_eq!(bytes[6 + 5], 0, "Color1.B");
}

#[test]
fn build_set_pump_lighting_default() {
    let bytes = build_set_pump_lighting_request(&PumpLightingSetting::default());
    assert_eq!(bytes.len(), PACKET_LEN);
    assert_eq!(bytes[1], 131, "Command = SetPumpLighting");
    assert_eq!(bytes[5], 19, "Length = 19 (PumpLightingSetting payload size)");
    assert_eq!(bytes[6], 0, "Scope = Inner = 0");
    assert_eq!(bytes[7], 1, "Mode = Rainbow = 1");
    assert_eq!(bytes[8], 2, "Brightness");
    assert_eq!(bytes[9], 2, "Speed");
}

#[test]
fn build_set_pump_lighting_static_blue_outer_at_max() {
    let s = PumpLightingSetting {
        scope: PumpLightingScope::Outer,
        mode: PumpLightingMode::StaticColor,
        brightness: 100,
        color1: Rgb::BLUE,
        ..PumpLightingSetting::default()
    };
    let bytes = build_set_pump_lighting_request(&s);
    assert_eq!(bytes[5], 19);
    assert_eq!(bytes[6], 1, "Scope = Outer = 1");
    assert_eq!(bytes[7], 3, "Mode = StaticColor = 3");
    assert_eq!(bytes[8], 100, "Brightness");
    assert_eq!(bytes[6 + 4], 0, "Color1.R");
    assert_eq!(bytes[6 + 5], 0, "Color1.G");
    assert_eq!(bytes[6 + 6], 0xFF, "Color1.B");
}
```

- [ ] **Step 3: Confirm RED**

```bash
cargo test -p lconnect-protocol --test galahad2_lcdv2_set_lighting 2>&1 | tail -5
```

- [ ] **Step 4: Implement `set_lighting.rs`**

Create `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`:

```rust
//! SetFanLighting (cmd 133) + SetPumpLighting (cmd 131).
//!
//! Both commands wrap the setting struct's `to_bytes()` payload in an
//! `ACommandPacket`. No response. Sources: Galahad2VisionDevice.cs:269-272
//! (`SetFanLighting`) and :310-313 (`SetPumpLighting`).

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::{FanLightingSetting, PumpLightingSetting};
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID request for `SetFanLighting` (cmd 133, 20-byte payload).
pub fn build_set_fan_lighting_request(setting: &FanLightingSetting) -> [u8; PACKET_LEN] {
    let payload = setting.to_bytes();
    *ACommandPacket::new_request(Command::SetFanLighting as u8, &payload).as_bytes()
}

/// Build the 64-byte HID request for `SetPumpLighting` (cmd 131, 19-byte payload).
pub fn build_set_pump_lighting_request(setting: &PumpLightingSetting) -> [u8; PACKET_LEN] {
    let payload = setting.to_bytes();
    *ACommandPacket::new_request(Command::SetPumpLighting as u8, &payload).as_bytes()
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p lconnect-protocol 2>&1 | grep "test result"
```

Expected: total `69 passed; 0 failed`.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p lconnect-protocol --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/protocol/src/galahad2_lcdv2/mod.rs \
        crates/protocol/src/galahad2_lcdv2/set_lighting.rs \
        crates/protocol/tests/galahad2_lcdv2_set_lighting.rs
git commit -m "feat(protocol): add Set{Fan,Pump}Lighting codecs

Wrap a setting struct's to_bytes() payload in an ACommandPacket with
cmd 133 (fan, 20-byte payload) or 131 (pump, 19-byte payload).
Fire-and-forget — no response.

4 golden-byte tests for default + StaticColor with concrete RGB values
on both fan and pump."
```

---

## Part C — Live verification (interactive, with the user's device)

These tasks are **interactive** — they need the live device. Done inline with the user, not via subagent.

### Task 7 (interactive): Wire `verify-galahad get-pump-lighting`

Read-only first — see what modes/maxes the device advertises before we send anything. This also doubles as a quick smoke test of the escape-byte parser against real wire bytes.

Plan: add a `GetPumpLighting` subcommand to verify-galahad that sends cmd 130 and prints the parsed response (modes list + max speed/brightness).

Done together with the user inline. No subagent.

### Task 8 (interactive): Wire `verify-galahad set-pump-lighting`

The fun one — set the pump LEDs to a known colour (e.g. solid red at 50% brightness, MCU source) and visually confirm. This is the unambiguous "did it work?" test.

Plan: add `SetPumpLighting { mode, color, brightness, scope }` subcommand. Run with mode=static-color, color=red, brightness=50, scope=inner. If the user can SEE red on the pump cap, full round-trip is verified.

Done together with the user inline.

### Task 9 (interactive): Wire `verify-galahad get-fan-lighting` + `set-fan-lighting`

Same pattern, fan side. Whether or not anything physically happens depends on whether fans are on this AIO header (Phase 0a/0b showed fan_rpm = 0, so likely not) — but the wire-level command will still succeed.

Done together with the user inline.

---

## Part D — Wrap-up

### Task 10: Update `galahad2-lcdv2.md` with the four new lighting commands

Append four sections (one per command) to `docs/protocol/galahad2-lcdv2.md`, before the "Commands not yet documented" section. Update that footer section to drop the four lighting commands from the "Phase 0c" list (they're now documented) and update the heading.

**Files:**
- Modify: `docs/protocol/galahad2-lcdv2.md`

- [ ] **Step 1: Find the "Commands not yet documented" section**

Open `docs/protocol/galahad2-lcdv2.md`. The current "Commands not yet documented" section (added in Phase 0b Task 12) lists Phase 0c lighting commands.

- [ ] **Step 2: Replace the lighting bullet under "Commands not yet documented"**

Change the line:
```markdown
- **Phase 0c:** `GetPumpLightingInfo` (130), `SetPumpLighting` (131),
  `GetFanLightingInfo` (132), `SetFanLighting` (133). Adds the lighting
  configuration types (modes, speeds, directions, brightness, ARGB sources).
```

to:
```markdown
- **Phase 0c:** ✅ done — see the four `Get/Set {Fan,Pump} Lighting`
  sections above.
```

- [ ] **Step 3: Insert four new command sections immediately *before* the "Commands not yet documented" heading**

Add this content (the actual byte tables and source citations — replace placeholders like `<paste raw response from your device's get-pump-lighting capture>` with whatever the live capture from Tasks 7–9 actually produced before committing):

```markdown
## `GetPumpLightingInfo` — Command 130 (`0x82`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-pump-lighting-live.txt](./captures/2026-04-25-pump-lighting-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:122-135`.
- Response parser: `PumpLightingInfo.cs:21-67` (escape-byte protocol).

### Request

A bare `ACommandPacket` with `Command = 130`, no data.

### Response

`ACommandPacket` echoing cmd 130 with a data slice that uses the
escape-byte protocol:

- Bytes are `PumpLightingMode` discriminants (mapped to `Unknown` for
  unknown values) until the literal sequence `0xA5 0x5A` terminates the
  list.
- After termination, two trailer bytes: `MaxSpeed`, `MaxBrightness`.
- Bug-compat: an `0xA5 X` sequence where `X != 0x5A` silently drops X
  without pushing a mode (mirrors the C# behaviour).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs`
  (`parse_pump_lighting_info`, `LightingInfo<M>`, type aliases)
- `crates/protocol/tests/galahad2_lcdv2_get_lighting.rs`,
  `crates/protocol/tests/galahad2_lcdv2_lighting_info.rs`

---

## `SetPumpLighting` — Command 131 (`0x83`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-pump-lighting-live.txt](./captures/2026-04-25-pump-lighting-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:310-313`.
- Payload encoder: `PumpLightingSetting.cs:107-141` (19 bytes).

### Request

`ACommandPacket` with `Command = 131` and `Length = 19`. Data:

| offset | size | field        | type                    |
|--------|------|--------------|-------------------------|
| 0      | 1    | `scope`      | `PumpLightingScope`     |
| 1      | 1    | `mode`       | `PumpLightingMode`      |
| 2      | 1    | `brightness` | u8                      |
| 3      | 1    | `speed`      | u8                      |
| 4–6    | 3    | `color1`     | RGB                     |
| 7–9    | 3    | `color2`     | RGB                     |
| 10–12  | 3    | `color3`     | RGB                     |
| 13–15  | 3    | `color4`     | RGB                     |
| 16     | 1    | `direction`  | `LightingDirection`     |
| 17     | 1    | `disabled`   | bool (0/1)              |
| 18     | 1    | `source`     | `ArgbSignalSource`      |

No response. Visually verify by looking at the pump cap LEDs.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs` (`PumpLightingSetting`,
  `to_bytes`)

---

## `GetFanLightingInfo` — Command 132 (`0x84`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-fan-lighting-live.txt](./captures/2026-04-25-fan-lighting-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:73-86`.
- Response parser: `FanLightingInfo.cs:21-67` (same escape-byte
  protocol as pump-lighting-info; only the mode enum differs).

### Request / response

Same shape as `GetPumpLightingInfo` except `Command = 132` and modes
are `FanLightingMode` (7 variants) instead of `PumpLightingMode` (17).
Implementation reuses the same generic `LightingInfo<M>` parser.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs`
  (`parse_fan_lighting_info`)

---

## `SetFanLighting` — Command 133 (`0x85`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-fan-lighting-live.txt](./captures/2026-04-25-fan-lighting-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:269-272`.
- Payload encoder: `FanLightingSetting.cs:117-150` (20 bytes).

### Request

`ACommandPacket` with `Command = 133` and `Length = 20`. Data:

| offset | size | field           | type                    |
|--------|------|-----------------|-------------------------|
| 0      | 1    | `mode`          | `FanLightingMode`       |
| 1      | 1    | `brightness`    | u8                      |
| 2      | 1    | `speed`         | u8                      |
| 3–5    | 3    | `color1`        | RGB                     |
| 6–8    | 3    | `color2`        | RGB                     |
| 9–11   | 3    | `color3`        | RGB                     |
| 12–14  | 3    | `color4`        | RGB                     |
| 15     | 1    | `direction`     | `LightingDirection`     |
| 16     | 1    | `disabled`      | bool (0/1)              |
| 17     | 1    | `source`        | `ArgbSignalSource`      |
| 18     | 1    | `sync_to_pump`  | bool (0/1)              |
| 19     | 1    | `number_of_led` | u8 (24 default; varies) |

No response. Visually verify by looking at fan-ring LEDs (if fans are
wired to the AIO's fan header).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs` (`FanLightingSetting`,
  `to_bytes`)
```

- [ ] **Step 4: Commit**

```bash
git add docs/protocol/galahad2-lcdv2.md
git commit -m "docs(protocol): document Phase 0c lighting commands

Adds four verified entries for GetPumpLightingInfo (130),
SetPumpLighting (131), GetFanLightingInfo (132), SetFanLighting (133)
with byte tables, source citations, and links to live-capture files.
The fan and pump pairs share the escape-byte response protocol and
the (Mode, Brightness, Speed, Colors, Direction, Disabled, Source)
core fields — fan adds (SyncToPump, NumberOfLED), pump adds (Scope).

Updates 'Commands not yet documented' to mark Phase 0c done and
points readers at the new sections."
```

---

### Task 11: Update README + post-execution notes

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-04-25-phase-0c-lighting.md` (this file — append post-execution notes)

- [ ] **Step 1: Update README**

In `README.md`, replace the `## Status` section's bullet list with:

```markdown
## Status

- ✅ Cargo workspace set up
- ✅ `lconnect-protocol` crate, with verified `ACommandPacket` framing
- ✅ Galahad II LCDV2 sensors (`GetHandshakeInfo`), firmware identity
  (`GetFirmwareVersion`), fan/pump PWM control (`SetFanPWM`, `SetPumpPWM`),
  soft-reset codec (`ResetDevice`), **and full lighting subsystem**
  (`Get/Set {Fan,Pump}Lighting`) — all verified against real hardware
  (or codec-tested with safety gate)
- ✅ Crypto-not-used-at-transport confirmed (`docs/protocol/crypto-usage.md`)
- 🚧 LCD config + frame upload — Phase 0d
- 🚧 Daemon, GUI, theme engine — Phases 1–6
```

In the "Phase-0 verification harness against a live device" code block, append:

```bash
cargo run -p verify-galahad -- get-pump-lighting
cargo run -p verify-galahad -- set-pump-lighting --mode static-color --color red --brightness 50 --scope inner
cargo run -p verify-galahad -- get-fan-lighting
cargo run -p verify-galahad -- set-fan-lighting --mode static-color --color blue --brightness 50
```

(Adjust the exact subcommand/flag names to whatever Tasks 7–9 actually shipped.)

- [ ] **Step 2: Append post-execution notes to this plan**

Append to the end of `docs/superpowers/plans/2026-04-25-phase-0c-lighting.md`:

```markdown

---

## Post-execution notes

- **Final test count:** [fill in actual number] (Phase 0b: 36, +N in
  this phase). Breakdown:
  - lighting_enums: 7
  - fan_lighting (Rgb + setting + to_bytes): 6
  - pump_lighting (to_bytes): 4
  - lighting_info parser: 7
  - get_lighting (codec): 5
  - set_lighting (codec): 4
  - **Total Phase 0c new tests: 33**, expected grand total **69**.
- **Live captures shipped:** [count] — at minimum
  `2026-04-25-pump-lighting-live.txt` and `2026-04-25-fan-lighting-live.txt`.
- **Idiomatic Rust additions:**
  - `Rgb` value type with const helpers (BLACK/WHITE/RED/GREEN/BLUE).
  - `LightingInfo<M>` generic over the mode enum, with type aliases
    `FanLightingInfo`/`PumpLightingInfo`. One parser serves both.
  - `from_u8` helpers on the mode enums for "fall back to Unknown" mapping.
- **Plan deviations:** [list any during execution].
- **Real-hardware findings on FW 1.6:**
  - [Fill in: which modes appear in the device's GetPumpLightingInfo
    response, what MaxSpeed/MaxBrightness are reported, and whether
    SetPumpLighting at static-red actually changed the LEDs.]
- **Commits:** [count] in this phase, all conventional-commit-prefixed.
- **Next:** Phase 0d (LCD subsystem + BCommandPacket framing). This is
  the user's primary motivation for the project — the LCD pump cap
  display.
```

- [ ] **Step 3: Run final workspace verification**

```bash
cargo build --workspace 2>&1 | tail -3 && \
cargo test --workspace 2>&1 | grep "test result" && \
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3 && \
cargo build --workspace --release 2>&1 | tail -3
```

Expected:
- Build: `Finished`
- Tests: lines totalling **69 passed** (or whatever Phase 0c actually shipped)
- Clippy: `Finished`, no warnings
- Release: `Finished`

- [ ] **Step 4: Commit**

```bash
git add README.md \
        docs/superpowers/plans/2026-04-25-phase-0c-lighting.md
git commit -m "docs: update README and Phase 0c plan for completion

README reflects the lighting subsystem as verified. Plan gains
post-execution notes (test count, live captures, real-hardware
findings, plan deviations)."
```

---

## Phase 0c complete — exit criteria

- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo test --workspace` reports **69 passed; 0 failed** (or higher
      if any defensive tests were added during execution).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo build --workspace --release` succeeds.
- [ ] `verify-galahad get-pump-lighting` prints decoded modes list +
      max speed/brightness from the live device.
- [ ] `verify-galahad set-pump-lighting --mode static-color --color red
      --brightness 50 --scope inner` produces a visible red on the pump
      cap (or the user explicitly notes their pump LEDs aren't visible).
- [ ] `docs/protocol/galahad2-lcdv2.md` documents all four new lighting
      commands with byte tables.
- [ ] At least 1 new live capture file under `docs/protocol/captures/`.
- [ ] `git log --oneline` shows ≥10 new commits since Phase 0b's tail.

When all pass, write **Phase 0d (LCD)** plan as the next sub-project.
This is the centerpiece of the user's motivation.
