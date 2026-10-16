//! GetFirmwareVersion (Command 134) — read the device's firmware identity.
//!
//! Sources:
//! - Wire flow: `Galahad2VisionDevice.cs:88-105`. Sends one `ACommandPacket`
//!   with cmd=134 and no data; reads *two* response packets back-to-back,
//!   one for the version string and one for the build datetime.
//! - Version-string parser: `FirmwareVersion.cs`. The version is a 7-field
//!   comma-separated string (`ChipCode,BootCode,Type,ModelA,ModelB,
//!   SoftwareVersion,UserVersion`). The datetime is free-form ASCII; we
//!   keep the raw string and let callers parse if they need a `time::Date`.

use crate::galahad2_lcdv2::commands::Command;
use crate::galahad2_lcdv2::packets::{ACommandPacket, PACKET_LEN};

/// Build the 64-byte HID packet to request firmware version.
///
/// Returns the raw bytes ready for `hid_device.write(&buf)`. The device
/// replies with **two** `ACommandPacket`s — see
/// [`FirmwareVersion::from_two_responses`].
pub fn build_request() -> [u8; PACKET_LEN] {
    *ACommandPacket::new_request(Command::GetFirmwareVersion as u8, &[]).as_bytes()
}

/// A parsed firmware-version response.
///
/// Mirrors `LConnectCore.Products.Galahad2Vision.FirmwareVersion` but
/// drops the `IsValid` boolean (failure is expressed as `Result::Err`) and
/// keeps `raw_datetime` as a string rather than a `chrono::DateTime` —
/// callers who want a typed date can parse it themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FirmwareVersion {
    /// e.g., `"N9"`. Field 0 of the comma-separated version string.
    pub chip_code: String,
    /// e.g., `"01"`.
    pub boot_code: String,
    /// e.g., `"HS"` ("HydroShift" / cooler family).
    pub kind: String,
    /// e.g., `"SQ"` (square model variant).
    pub model_a: String,
    /// e.g., `"CA_II-Vision"`.
    pub model_b: String,
    /// e.g., `"V2.01.041"`.
    pub software_version: String,
    /// e.g., `"1.6"`. The user-facing firmware revision.
    pub user_version: String,
    /// The verbatim datetime from the second response packet, e.g.,
    /// `"Apr 12 2024 14:33:21"`.
    pub raw_datetime: String,
}

impl FirmwareVersion {
    /// Parse a comma-separated version string + datetime string into a
    /// structured value. Pure: no I/O.
    ///
    /// Inputs are trimmed of whitespace **and** trailing NUL bytes (mirrors
    /// the C# `.Trim().Trim('\0')` behaviour — HID payloads are usually
    /// zero-padded).
    pub fn parse(version_str: &str, datetime_str: &str) -> crate::Result<Self> {
        // C# does `.Trim().Trim('\0')`, but for inputs like `"…1.6 \0"`
        // that leaves a stray trailing space. We trim whitespace, NULs,
        // then whitespace again to handle either ordering.
        let version_str = version_str.trim().trim_matches('\0').trim();
        let datetime_str = datetime_str.trim().trim_matches('\0').trim();

        let fields: Vec<&str> = version_str.split(',').collect();
        if fields.len() < 7 || version_str.is_empty() {
            return Err(crate::Error::MalformedFirmwareVersion {
                got_fields: if version_str.is_empty() {
                    0
                } else {
                    fields.len()
                },
            });
        }

        Ok(FirmwareVersion {
            chip_code: fields[0].to_owned(),
            boot_code: fields[1].to_owned(),
            kind: fields[2].to_owned(),
            model_a: fields[3].to_owned(),
            model_b: fields[4].to_owned(),
            software_version: fields[5].to_owned(),
            user_version: fields[6].to_owned(),
            raw_datetime: datetime_str.to_owned(),
        })
    }

    /// Decode the *pair* of `ACommandPacket` responses the device sends in
    /// reply to a `GetFirmwareVersion` request.
    ///
    /// Both packets must echo `Command::GetFirmwareVersion` (= 134). The
    /// first packet's data is the version string (ASCII); the second is
    /// the datetime string (ASCII).
    pub fn from_two_responses(buf1: &[u8], buf2: &[u8]) -> crate::Result<Self> {
        Self::from_optional_responses(buf1, Some(buf2))
    }

    /// Decode already-classified PN0 version and optional PN1 raw datetime.
    /// Missing PN1 leaves `raw_datetime` empty; PN0 is never used as a date.
    pub fn from_optional_responses(version: &[u8], datetime: Option<&[u8]>) -> crate::Result<Self> {
        let cmd = Command::GetFirmwareVersion as u8;
        let p1 = ACommandPacket::parse_response(version, cmd)?;
        let check_number = |bytes: &[u8], expected| {
            let got = u16::from_be_bytes([bytes[3], bytes[4]]);
            if got != expected {
                return Err(crate::Error::UnexpectedPacketNumber { expected, got });
            }
            Ok(())
        };
        check_number(version, 0)?;
        let date = match datetime {
            Some(bytes) => {
                let packet = ACommandPacket::parse_response(bytes, cmd)?;
                check_number(bytes, 1)?;
                String::from_utf8_lossy(packet.data())
            }
            None => std::borrow::Cow::Borrowed(""),
        };
        Self::parse(&String::from_utf8_lossy(p1.data()), &date)
    }
}
