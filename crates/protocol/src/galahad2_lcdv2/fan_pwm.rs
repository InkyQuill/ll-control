//! SetFanPWM (Command 139) — write-only fan duty-cycle setter.
//!
//! Source: `Galahad2VisionDevice.cs:271-281`. The C# call is
//! `writeACommand(139, new byte[] { 0, pwm })` — the leading `0` is the
//! fan group (the LCDV2 hardware exposes a single group, but the byte is
//! reserved in the wire format).

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};
use crate::galahad2_lcdv2::pwm::Pwm;

/// Build the 64-byte HID packet to set the fan PWM.
///
/// No response is expected from the device for this command.
pub fn build_request(pwm: Pwm) -> [u8; PACKET_LEN] {
    let data = [0u8 /* group */, pwm.percent()];
    *ACommandPacket::new_request(Command::SetFanPWM as u8, &data).as_bytes()
}
