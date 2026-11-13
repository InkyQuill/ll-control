//! HID framing classes used by Lian Li devices.
//!
//! Phase 1 ships A/B/C. Phase 0e identified at least 7 distinct framing
//! classes across the LConnect3 device universe (TLLCD T-class, StrimerPlus
//! HID-feature, lcd207 WinUSB-DES, slv3 WinUSB-RF, UniversalScreen bulk).
//! The enum is `#[non_exhaustive]` so future families add variants without
//! breaking existing matches.
//!
//! # See also
//!
//! - [`crate::backend::HidBackend`] — uses `Framing` to size buffers
//! - `docs/development/protocol/cross-device-survey.md` section 6.1 — extension points
//! - `docs/development/protocol/cross-device-survey.md` section 4.10 — per-family transport matrix

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Framing {
    /// A-class: 64-byte output / 64-byte input, ReportID 0x01.
    /// Used by Galahad II family for handshake / firmware / PWM / lighting-info commands.
    A,
    /// B-class: 1024-byte output / 512-byte input, ReportID 0x02.
    /// Used by Galahad II LCDV2 for LCD control + JPEG/H264 frame transfers.
    B,
    /// C-class: 512-byte output / 512-byte input, ReportID 0x03.
    /// Used by Galahad II LCDV2 for `SendH264Frame512`.
    C,
}

impl Framing {
    /// Bytes to write per HID frame for this framing class (incl. ReportID byte).
    #[must_use]
    pub fn output_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 1024,
            Framing::C => 512,
        }
    }

    /// Bytes to read per HID frame.
    #[must_use]
    pub fn input_len(self) -> usize {
        match self {
            Framing::A => 64,
            Framing::B => 512,
            Framing::C => 512,
        }
    }

    /// HID Report ID byte (offset 0 of the buffer).
    #[must_use]
    pub fn report_id(self) -> u8 {
        match self {
            Framing::A => 0x01,
            Framing::B => 0x02,
            Framing::C => 0x03,
        }
    }
}
