//! Command codes for Galahad II LCDV2.
//!
//! Mirrors `Products/Galahad2Vision/Commands.cs`. The numeric values are
//! the **wire format** — never reorder.

/// Command byte sent in `ACommandPacket.bytes[1]`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command {
    /// Activate already-uploaded firmware. (Code 10.)
    ActiveFirmware = 10,
    /// Begin a firmware upload. (Code 11.) `BCommandPacket`.
    UploadFirmware = 11,
    /// Set LCD control mode (manual / auto / etc.). (Code 12.)
    LCDControl = 12,
    /// Send one H.264 frame. (Code 13.) `BCommandPacket`.
    SendH264Frame = 13,
    /// Send one JPEG frame. (Code 14.) `BCommandPacket`.
    SendJpeg = 14,
    /// Upload a JPEG file. (Code 15.)
    UploadJpeg = 15,
    /// Upload an AVI file. (Code 18.)
    UploadAvi = 18,
    /// Upload an H.264 file. (Code 19.)
    UploadH264 = 19,
    /// Set per-LED screen colours. (Code 20.)
    SetLEDColors = 20,
    /// Send H.264 stream metadata. (Code 21.)
    UploadH264Info = 21,
    /// Set the boot animation. (Code 22.)
    UploadStartupAviMovie = 22,
    /// Query whether an LCD is attached. (Code 23.)
    IsLCDAvailable = 23,
    /// Sensors + temp + RPMs. (Code 129.)
    GetHandshakeInfo = 129,
    /// Read pump-lighting state. (Code 130.)
    GetPumpLightingInfo = 130,
    /// Write pump-lighting state. (Code 131.)
    SetPumpLighting = 131,
    /// Read fan-lighting state. (Code 132.)
    GetFanLightingInfo = 132,
    /// Write fan-lighting state. (Code 133.)
    SetFanLighting = 133,
    /// Read firmware version string. (Code 134.)
    GetFirmwareVersion = 134,
    /// Read serial number. (Code 136.)
    GetSerialNumber = 136,
    /// Write serial number. (Code 137.)
    SetSerialNumber = 137,
    /// Set pump PWM (data: `[source, percent]`). (Code 138.)
    SetPumpPWM = 138,
    /// Set fan PWM (data: `[group, percent]`). (Code 139.)
    SetFanPWM = 139,
    /// Soft-reset the device. (Code 142.)
    ResetDevice = 142,
}
