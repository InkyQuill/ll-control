//! SetLEDColors (cmd 20). The actual per-LED RGB array driver — both for
//! the fan ring and for the pump-cap screen LEDs.
//!
//! Two payload variants share command byte 20, distinguished by the
//! `isFanLed` flag at offset 0:
//!
//! - **Fan ring** (`isFanLed=true`): `[0x01, R0, G0, B0, R1, G1, B1, ...]`.
//!   Length = `colors.len() * 3 + 1`. RGB data starts at offset 1.
//! - **Screen / pump-cap** (`isFanLed=false`): `[0x00, <24 zero bytes>,
//!   R0, G0, B0, ...]`. Length = `(colors.len() + 8) * 3 + 1`. RGB data
//!   starts at offset 25, after a 24-byte zero pad (= 8 extra Color slots
//!   × 3 channels).
//!
//! Each payload is wrapped in [`BCommandPacket::build_packets_for_write`],
//! which auto-chunks anything exceeding the 1013-byte single-packet
//! payload cap. With that cap:
//!
//! - Fan-path single-packet maximum: `(1013 - 1) / 3 = 337` LEDs.
//! - Screen-path single-packet maximum: `(1013 - 25) / 3 = 329` LEDs.
//!
//! Beyond those counts the chunker auto-splits across multiple
//! `BCommandPacket`s; physical Galahad II LCDV2 configurations are well
//! within the single-packet range, so chunking only matters for larger
//! aggregations (e.g. wireless-hub fan-outs).
//!
//! Atomicity caveat: the device firmware applies a write as a unit, but
//! callers passing fewer LEDs than the physical ring count will leave the
//! unaddressed LEDs at whatever state they held before the write. This
//! codec does not pad short arrays — that's the caller's responsibility.
//!
//! Source: Galahad2VisionDevice.cs:283-308.
//!
//! ## Future refactor note
//!
//! If a third LED path is ever added (e.g. a wireless-hub aggregating
//! external rings), prefer extracting a `LedKind::{Fan, Screen, Hub}`
//! enum and a single `build_set_led_colors_request(kind, colors)` over
//! adding a third pair of functions. The current two-fn split is fine
//! for two paths; three would warrant unification.

use crate::galahad2_lcdv2::bpacket::{BCommandPacket, OUTPUT_LEN};
use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::lighting::Rgb;

/// Maximum LED count for the fan path that fits in a single B-packet.
///
/// Derived from `(MAX - 1) / 3` where `MAX` is `B_PAYLOAD_MAX = 1013` and
/// `1` is the `isFanLed` flag byte at offset 0. Single-packet writes only;
/// multi-packet `SetLEDColors` is not yet live-tested.
pub const MAX_FAN_LEDS: usize = 337;

/// Maximum LED count for the screen / pump-cap path that fits in a single
/// B-packet.
///
/// Derived from `(MAX - 25) / 3` where `MAX` is `B_PAYLOAD_MAX = 1013` and
/// `25` is the offset where RGB pixel data starts (after the `isFanLed`
/// flag byte at offset 0 and the 24-byte zero pad at offsets 1..25).
/// Single-packet writes only; multi-packet `SetLEDColors` is not yet
/// live-tested.
pub const MAX_SCREEN_LEDS: usize = 329;

/// Build the fan-LED payload (`isFanLed=true`). Layout:
/// `[0x01, R0, G0, B0, R1, G1, B1, ...]`. Length = `colors.len() * 3 + 1`.
///
/// **Note:** This is a low-level helper that does NOT validate `colors.len()`.
/// For the safety-checked path that rejects empty and over-max LED counts,
/// use [`build_set_fan_led_colors_request`] / [`build_set_screen_led_colors_request`].
///
/// # Examples
///
/// ```
/// use llcontrol_protocol::galahad2_lcdv2::lighting::Rgb;
/// use llcontrol_protocol::galahad2_lcdv2::set_led_colors::fan_led_payload;
///
/// // One red LED → flag byte + RGB triple
/// assert_eq!(fan_led_payload(&[Rgb::RED]), vec![0x01, 0xFF, 0x00, 0x00]);
/// ```
#[must_use = "returns the wire payload; dropping without writing has no effect"]
pub fn fan_led_payload(colors: &[Rgb]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + colors.len() * 3);
    out.push(0x01);
    for c in colors {
        out.extend_from_slice(&[c.r, c.g, c.b]);
    }
    out
}

/// Build the screen-LED payload (`isFanLed=false`). Layout:
/// `[0x00, <24 zero bytes>, R0, G0, B0, ...]`. Length =
/// `(colors.len() + 8) * 3 + 1`.
///
/// **Note:** This is a low-level helper that does NOT validate `colors.len()`.
/// For the safety-checked path that rejects empty and over-max LED counts,
/// use [`build_set_fan_led_colors_request`] / [`build_set_screen_led_colors_request`].
///
/// # Examples
///
/// ```
/// use llcontrol_protocol::galahad2_lcdv2::lighting::Rgb;
/// use llcontrol_protocol::galahad2_lcdv2::set_led_colors::screen_led_payload;
///
/// // One blue LED → flag byte + 24-byte zero pad + RGB triple at offset 25
/// let payload = screen_led_payload(&[Rgb::BLUE]);
/// assert_eq!(payload.len(), 28);
/// assert_eq!(payload[0], 0x00);
/// assert!(payload[1..25].iter().all(|&b| b == 0));
/// assert_eq!(&payload[25..28], &[0x00, 0x00, 0xFF]);
/// ```
#[must_use = "returns the wire payload; dropping without writing has no effect"]
pub fn screen_led_payload(colors: &[Rgb]) -> Vec<u8> {
    // SAFETY-OF-PROTOCOL: bytes 1..25 must remain zero per
    // Galahad2VisionDevice.cs:288-291 (the 24-byte zero pad — 8 extra Color
    // slots × 3 channels — between the isFanLed flag at offset 0 and the
    // pixel data at offset 25). The `vec![0u8; ...]` zero-initialises the
    // whole buffer, which is what makes the pad correct WITHOUT an explicit
    // `fill` or `extend(repeat(0).take(24))`. Do NOT refactor to
    // `Vec::with_capacity(...)` + `extend_from_slice` — that would lose the
    // pad guarantee silently.
    let mut out = vec![0u8; (colors.len() + 8) * 3 + 1];
    out[0] = 0x00;
    let mut idx = 25;
    for c in colors {
        out[idx] = c.r;
        out[idx + 1] = c.g;
        out[idx + 2] = c.b;
        idx += 3;
    }
    out
}

/// Build SetLEDColors B-packets for the fan ring path.
///
/// Wraps [`fan_led_payload`] in [`BCommandPacket::build_packets_for_write`],
/// which yields one 1024-byte packet per 1013-byte chunk. Up to
/// [`MAX_FAN_LEDS`] (337) LEDs fit in a single packet; the multi-packet
/// chunked path is not yet live-tested, so over-max calls are rejected.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidLedCount`] if `colors` is empty (the
/// firmware behaviour for "zero LEDs" is undocumented and the C# UI never
/// sends this) or if `colors.len() > MAX_FAN_LEDS` (would silently chunk
/// on a write path with no live test).
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_set_fan_led_colors_request(colors: &[Rgb]) -> crate::Result<Vec<[u8; OUTPUT_LEN]>> {
    if colors.is_empty() || colors.len() > MAX_FAN_LEDS {
        return Err(crate::Error::InvalidLedCount {
            kind: "fan",
            count: colors.len(),
            max: MAX_FAN_LEDS,
        });
    }
    let payload = fan_led_payload(colors);
    Ok(
        BCommandPacket::build_packets_for_write(Command::SetLEDColors as u8, &payload)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}

/// Build SetLEDColors B-packets for the screen / pump-cap LEDs.
///
/// Wraps [`screen_led_payload`] in [`BCommandPacket::build_packets_for_write`].
/// Up to [`MAX_SCREEN_LEDS`] (329) LEDs fit in a single packet (the 24-byte
/// zero pad eats into the 1013-byte cap); the multi-packet chunked path is
/// not yet live-tested, so over-max calls are rejected.
///
/// # Errors
///
/// Returns [`crate::Error::InvalidLedCount`] if `colors` is empty (the
/// firmware behaviour for "zero LEDs" is undocumented and the C# UI never
/// sends this) or if `colors.len() > MAX_SCREEN_LEDS` (would silently chunk
/// on a write path with no live test).
#[must_use = "returns the request packet bytes; dropping without writing has no effect"]
pub fn build_set_screen_led_colors_request(colors: &[Rgb]) -> crate::Result<Vec<[u8; OUTPUT_LEN]>> {
    if colors.is_empty() || colors.len() > MAX_SCREEN_LEDS {
        return Err(crate::Error::InvalidLedCount {
            kind: "screen",
            count: colors.len(),
            max: MAX_SCREEN_LEDS,
        });
    }
    let payload = screen_led_payload(colors);
    Ok(
        BCommandPacket::build_packets_for_write(Command::SetLEDColors as u8, &payload)
            .map(|p| *p.as_bytes())
            .collect(),
    )
}
