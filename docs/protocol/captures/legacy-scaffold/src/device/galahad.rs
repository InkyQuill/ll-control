use anyhow::{anyhow, Result};
use hidapi::{HidApi, HidDevice};
use serde::Serialize;
use tracing::{debug, error, info};

// VID/PID for GALAHAD II Vision
const VID: u16 = 0x416;
const PID: u16 = 0x7395; // Updated - device reports 0x7395 instead of 0x7381

// Commands
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Command {
    GetHandshakeInfo = 129,
    SetFanPWM = 139,
    SetPumpPWM = 138,
    GetFirmwareVersion = 134,
}

#[derive(Debug, Clone, Serialize)]
pub struct HandshakeInfo {
    pub fan_rpm: u16,
    pub pump_rpm: u16,
    pub temperature: f32,
    pub temp_valid: bool,
}

pub struct GalahadDevice {
    api: HidApi,
    device: HidDevice,
}

impl GalahadDevice {
    pub fn connect() -> Result<Self> {
        info!("Connecting to GALAHAD II device (VID={:04x}, PID={:04x})", VID, PID);

        let api = HidApi::new()?;

        // Check if device exists
        let device_info = api
            .device_list()
            .find(|d| d.vendor_id() == VID && d.product_id() == PID);

        if let Some(info) = device_info {
            info!(
                "Found device: {}",
                info.product_string().unwrap_or("Unknown")
            );
        }

        // Open device using VID/PID
        let device = api.open(VID, PID)?;

        // Set non-blocking
        device.set_blocking_mode(false)?;

        Ok(Self { api, device })
    }

    pub fn read_handshake(&self) -> Result<HandshakeInfo> {
        debug!("Sending handshake command");

        // Send Command 129 (GetHandshakeInfo)
        // ACommandPacket: [ReportID=1, Command=129, ...]
        let mut packet = [0u8; 64];
        packet[0] = 1; // Report ID
        packet[1] = Command::GetHandshakeInfo as u8;

        self.device.write(&packet)?;

        // Read response
        let mut buf = [0u8; 64];
        let bytes_read = self.device.read_timeout(&mut buf, 1000)?;

        if bytes_read < 13 {
            return Err(anyhow!("Invalid response length: {}", bytes_read));
        }

        // Parse response according to C# code structure:
        // ACommandPacket.Data = bytes[6..64] (58 bytes of payload)
        // HandshakeInfo uses data[0..7] of that payload:
        //   data[0-1] = Fan RPM (big-endian)
        //   data[2-3] = Pump RPM (big-endian)
        //   data[4] = Temperature valid flag
        //   data[5] = Temperature integer
        //   data[6] = Temperature fraction (tenths)

        let fan_rpm = u16::from_be_bytes([buf[6], buf[7]]);
        let pump_rpm = u16::from_be_bytes([buf[8], buf[9]]);
        let temp_valid = buf[10] != 0;
        let temp_integer = buf[11];
        let temp_fraction = buf[12];

        let temperature = if temp_valid {
            temp_integer as f32 + (temp_fraction as f32 / 10.0)
        } else {
            0.0
        };

        debug!(
            "Handshake: fan={}rpm, pump={}rpm, temp={:.1}°C (valid={})",
            fan_rpm, pump_rpm, temperature, temp_valid
        );

        Ok(HandshakeInfo {
            fan_rpm,
            pump_rpm,
            temperature,
            temp_valid,
        })
    }

    pub fn set_fan_pwm(&self, percentage: u8) -> Result<()> {
        if percentage > 100 {
            return Err(anyhow!("Percentage must be 0-100"));
        }

        debug!("Setting fan PWM to {}%", percentage);

        // ACommandPacket for SetFanPWM (139)
        let mut packet = [0u8; 64];
        packet[0] = 1; // Report ID
        packet[1] = Command::SetFanPWM as u8;
        packet[2] = percentage;

        self.device.write(&packet)?;
        Ok(())
    }

    pub fn set_pump_pwm(&self, percentage: u8) -> Result<()> {
        if percentage > 100 {
            return Err(anyhow!("Percentage must be 0-100"));
        }

        debug!("Setting pump PWM to {}%", percentage);

        // ACommandPacket for SetPumpPWM (138)
        let mut packet = [0u8; 64];
        packet[0] = 1; // Report ID
        packet[1] = Command::SetPumpPWM as u8;
        packet[2] = percentage;

        self.device.write(&packet)?;
        Ok(())
    }

    pub fn get_firmware_version(&self) -> Result<String> {
        debug!("Getting firmware version");

        // Send Command 134 (GetFirmwareVersion)
        let mut packet = [0u8; 64];
        packet[0] = 1;
        packet[1] = Command::GetFirmwareVersion as u8;

        self.device.write(&packet)?;

        // Read response
        let mut buf = [0u8; 64];
        let bytes_read = self.device.read_timeout(&mut buf, 1000)?;

        if bytes_read < 20 {
            return Err(anyhow!("Invalid response length: {}", bytes_read));
        }

        // Parse firmware string from bytes 6 onwards
        // Format: "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6"
        let fw_bytes = &buf[6..bytes_read];
        let null_terminator = fw_bytes.iter().position(|&b| b == 0).unwrap_or(fw_bytes.len());
        let fw_string = String::from_utf8_lossy(&fw_bytes[..null_terminator]).to_string();

        debug!("Firmware version: {}", fw_string);

        Ok(fw_string)
    }
}

impl Drop for GalahadDevice {
    fn drop(&mut self) {
        info!("Closing device connection");
    }
}
