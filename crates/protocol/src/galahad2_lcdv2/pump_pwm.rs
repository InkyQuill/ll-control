//! SetPumpPWM (Command 138) — write-only pump duty-cycle setter.
//!
//! Source: `Galahad2VisionDevice.cs:316-322`. The C# call is
//! `writeACommand(138, new byte[] { source, pwm })`.
//!
//! The `source` byte selects who controls the pump:
//!   - `MCU` (0) — the AIO firmware drives the pump from its internal
//!     temperature curve, ignoring whatever the motherboard sends.
//!   - `Motherboard` (1) — the AIO honours the PWM signal on the MB pump
//!     header. The percent we send becomes the *upper bound*.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};
use crate::galahad2_lcdv2::pwm::Pwm;

/// Selects which controller drives the pump duty cycle.
///
/// Mirrors `LConnectCore.Products.Galahad2Vision.PumpPWMSource`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PumpPwmSource {
    /// AIO firmware drives the pump from an internal curve.
    Mcu = 0,
    /// Pump follows the motherboard PWM signal on the AIO_PUMP header.
    Motherboard = 1,
}

/// Build the 64-byte HID packet to set the pump PWM.
///
/// No response is expected from the device for this command.
pub fn build_request(source: PumpPwmSource, pwm: Pwm) -> [u8; PACKET_LEN] {
    let data = [source as u8, pwm.percent()];
    *ACommandPacket::new_request(Command::SetPumpPWM as u8, &data).as_bytes()
}
