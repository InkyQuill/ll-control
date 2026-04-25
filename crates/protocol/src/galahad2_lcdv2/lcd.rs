//! LCD lifecycle commands.
//!
//! - `IsLCDAvailable` (cmd 23) — read-only probe. Source:
//!   Galahad2VisionDevice.cs:175-186. Builder
//!   [`build_is_lcd_available_request`] + parser
//!   [`parse_is_lcd_available_response`].
//! - `LCDControl`     (cmd 12) — set mode/brightness/rotation/test
//!   pattern. Source: Galahad2VisionDevice.cs:188-193 +
//!   LCDControlSetting.cs. Types [`LCDControlMode`], [`ScreenRotation`],
//!   [`LCDControlSetting`]; builder [`build_lcd_control_request`].
//!
//! ## Latent C# bug (do NOT replicate)
//!
//! The C# service's `IsLCDAvailable` polling timer at
//! `Galahad2VisionController.cs:2410-2411` checks `nullable.HasValue`
//! against a stale local that is no longer in sync with the value
//! actually stored into `this.isAvailable`. The codec parse is correct;
//! the bug is at the call site. The Linux port's contract is: callers
//! branch on the `bool` returned by [`parse_is_lcd_available_response`]
//! directly — never reintroduce a separate "has value" indirection.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::Rgb;

/// LCD brightness percent in the range `0..=100`.
///
/// The C# UI clamps user input to `0..=100` via `IntExtensions.Constrain`
/// before sending. The firmware's behaviour for values > 100 is
/// undocumented; this newtype encodes the invariant into the type system
/// so callers cannot construct a `Brightness` holding `101+`. Mirrors
/// the established `Pwm(u8)` newtype precedent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    /// Construct a `Brightness` from a percent. Returns
    /// [`crate::Error::InvalidBrightness`] for values `> 100`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidBrightness`] when `percent > 100`.
    pub fn new(percent: u8) -> crate::Result<Self> {
        if percent > 100 {
            return Err(crate::Error::InvalidBrightness(percent));
        }
        Ok(Self(percent))
    }

    /// The maximum-valid brightness (100%).
    #[must_use]
    pub fn max() -> Self {
        Self(100)
    }

    /// The contained percent value, guaranteed `0..=100`.
    #[must_use]
    pub fn percent(self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        // Matches LCDControlSetting.cs:51-59 default of 50.
        Self(50)
    }
}

/// Build the 1024-byte HID request for `IsLCDAvailable` (cmd 23).
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_is_lcd_available_request() -> [u8; OUTPUT_LEN] {
    *BCommandPacket::new_request(Command::IsLCDAvailable as u8, 0, &[], 0).as_bytes()
}

/// Parse an `IsLCDAvailable` response. Returns `true` iff the device
/// reported a single `0x00` byte (the "available" sentinel). Anything
/// else — non-zero byte, missing payload, mismatched length — means
/// not available.
///
/// # Caveats
///
/// The C# service controller has a latent stale-local bug at
/// `Galahad2VisionController.cs:2410-2411`: it checks
/// `nullable.HasValue` against a value that is no longer in sync with
/// `this.isAvailable`. Callers MUST branch on the returned `bool`
/// directly; the Linux port deliberately surfaces only the `bool` to
/// make that anti-pattern unrepresentable.
///
/// # Errors
///
/// Returns:
/// - [`crate::Error::BufferTooShort`] if `buf` is shorter than the
///   B-packet header (11 bytes), OR if the parsed `Length` field
///   announces more payload bytes than `buf` contains.
/// - [`crate::Error::UnexpectedReportId`] if `buf[0]` is not `0x02`.
/// - [`crate::Error::UnexpectedCommand`] if `buf[1]` is not `23`.
/// - [`crate::Error::InvalidLength`] if the header's `Length` field
///   exceeds the available payload bytes in `buf`.
pub fn parse_is_lcd_available_response(buf: &[u8]) -> crate::Result<bool> {
    let parsed = BCommandPacket::from_response(buf, Command::IsLCDAvailable as u8)?;
    Ok(parsed.length() == 1 && parsed.data()[0] == 0x00)
}

/// LCD operating mode (8-bit wire-format).
///
/// # Source
///
/// `LCDControlMode.cs:5-13` — C# `enum LCDControlMode : byte`. The
/// Linux port preserves the implicit byte discriminants (0..=5).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LCDControlMode {
    /// Built-in firmware UI — the default boot mode.
    LocalUI = 0,
    /// Host application drives content via `SetLEDColors` / `SendJpeg`.
    Application = 1,
    /// Internal H.264 looped video.
    LocalH264 = 2,
    /// Internal AVI looped video.
    LocalAVI = 3,
    /// Brightness / rotation settings update path used by the C#
    /// service controller (see [`LCDControlSetting::for_brightness_change`]).
    LCDSetting = 4,
    /// Solid-colour test pattern (uses `test_color`).
    ///
    /// # Foot-gun
    ///
    /// This mode is a foot-gun: the device renders the configured
    /// `test_color` as a solid full-screen pattern and keeps rendering
    /// it until another `LCDControl` command (or a power cycle) changes
    /// the mode. Treat it as a diagnostic, not a runtime mode.
    LCDTest = 5,
}

/// 90-degree screen rotation step.
///
/// # Source
///
/// `ScreenRotation.cs:5-11` — C# `enum ScreenRotation : byte`. The wire
/// byte is the discriminant (0/1/2/3), **not** the rotation angle in
/// degrees.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScreenRotation {
    /// No rotation (0°).
    Rotate0 = 0,
    /// 90° clockwise.
    Rotate90 = 1,
    /// 180°.
    Rotate180 = 2,
    /// 270° clockwise (i.e. 90° counter-clockwise).
    Rotate270 = 3,
}

/// 8-byte payload for `LCDControl` (cmd 12).
///
/// Wire layout: `[Mode, Brightness, Rotation, EnableTest, R, G, B,
/// VideoFPS]`.
///
/// # Source
///
/// `LCDControlSetting.cs:7-73` — C# `struct LCDControlSetting`, with
/// `ToBytes` at lines 61-72.
///
/// # Caveats
///
/// - **`mode = LCDTest`** locks the panel to a solid `test_color` until
///   another `LCDControl` is sent. See the foot-gun note on
///   [`LCDControlMode::LCDTest`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LCDControlSetting {
    /// LCD operating mode — see [`LCDControlMode`].
    pub mode: LCDControlMode,
    /// Backlight brightness, range-checked at construction. See
    /// [`Brightness`].
    pub brightness: Brightness,
    /// Screen rotation — see [`ScreenRotation`].
    pub rotation: ScreenRotation,
    /// `true` enables the `LCDTest` solid-colour pattern; effective only
    /// when `mode == LCDControlMode::LCDTest`.
    pub enable_test: bool,
    /// Solid-colour value used when `enable_test` is set.
    pub test_color: Rgb,
    /// Frame rate hint for `LocalH264` / `LocalAVI` playback paths.
    ///
    /// Known values: 24 (used by the L-Connect-Service brightness-update
    /// path) and 30 (the C# default constructor). The effect of values
    /// outside this range is undocumented; verified safe range pending a
    /// live test.
    pub video_fps: u8,
}

impl Default for LCDControlSetting {
    /// Defaults from the C# parameterless constructor at
    /// `LCDControlSetting.cs:51-59`:
    /// `Mode=LocalUI, Brightness=50, Rotation=Rotate0,
    ///  EnableTest=false, TestColor=Black, VideoFPS=30`.
    fn default() -> Self {
        Self {
            mode: LCDControlMode::LocalUI,
            brightness: Brightness::default(),
            rotation: ScreenRotation::Rotate0,
            enable_test: false,
            test_color: Rgb::BLACK,
            video_fps: 30,
        }
    }
}

impl LCDControlSetting {
    /// Match the service controller's brightness-change pattern
    /// (`Mode=LCDSetting`, `VideoFPS=24`); other fields take
    /// [`Default`] values.
    ///
    /// Calling this constructor repeatedly with the same [`Brightness`]
    /// is idempotent at the protocol layer — every call produces the
    /// same wire bytes. The C# service treats brightness updates as
    /// cheap re-applies of `Mode=LCDSetting`.
    ///
    /// # Source
    ///
    /// `Galahad2VisionController.cs:1483-1487`.
    #[must_use]
    pub fn for_brightness_change(brightness: Brightness) -> Self {
        Self {
            mode: LCDControlMode::LCDSetting,
            brightness,
            video_fps: 24,
            ..Self::default()
        }
    }

    /// Encode as the 8-byte wire payload.
    ///
    /// `mode` and `rotation` are `#[repr(u8)]` enums, so the `as u8`
    /// casts are lossless.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            self.mode as u8,
            self.brightness.percent(),
            self.rotation as u8,
            u8::from(self.enable_test),
            self.test_color.r,
            self.test_color.g,
            self.test_color.b,
            self.video_fps,
        ]
    }
}

/// Build the 1024-byte HID request for `LCDControl` (cmd 12).
///
/// The 8-byte payload is laid down at offset 11 with `DataSize=8` and
/// `Length=8`. See [`LCDControlSetting`] and [`LCDControlMode::LCDTest`]
/// for the foot-gun caveat around test-pattern mode.
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_lcd_control_request(setting: LCDControlSetting) -> [u8; OUTPUT_LEN] {
    let payload = setting.to_bytes();
    *BCommandPacket::new_request(Command::LCDControl as u8, 0, &payload, 8).as_bytes()
}
