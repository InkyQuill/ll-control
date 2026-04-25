# Decompiled source map — Galahad II LCDV2

Maps subsystems we implement in `crates/protocol/galahad2_lcdv2/` to the
decompiled C# files at
`/home/inky/Development/lian-li/L-Connect.Core/LConnectCore/`.

All paths below are relative to that root.

## Device identity

| Topic | File | Notes |
|---|---|---|
| VID/PID table | `Devices/KnownDevices.cs` | `Galahad2LCDV2 = (1046, 29589)` = `0x0416:0x7395`. Lines 36–47. |
| Controller type enum | `Devices/ControllerTypeID.cs` | Maps SKU names to enum. |
| HID device interface | `Devices/IHidDevice.cs`, `Devices/HidApiHidDevice.cs` | Used at the I/O layer; not needed in `protocol` crate. |

## Galahad II Vision (folder: `Products/Galahad2Vision/`)

This folder implements multiple SKUs (Trinity Performance / Trinity Regular
/ LCD / LCDV2). The wire protocol within the folder is shared; SKU
divergence is by VID/PID at discovery time. **For Phase 0a we treat all of
`Galahad2Vision/` as describing LCDV2's protocol.** If Phase 0b finds a
SKU-specific branch, we annotate it then.

### Packet framing

| Packet type | File | Use |
|---|---|---|
| `ACommandPacket` (64 B) | `Products/Galahad2Vision/ACommandPacket.cs` | Standard request/response. **Used by all Phase 0a commands.** |
| `BCommandPacket` (512 B) | `Products/Galahad2Vision/BCommandPacket.cs` | Bulk transfers (firmware upload, JPEG/H264 frames). Phase 0b. |
| `CCommandPacket` | `Products/Galahad2Vision/CCommandPacket.cs` | Yet to characterise. Phase 0b. |
| Base `Packet` | `Devices/Packet.cs` | Holds the underlying `byte[]`. |

### Commands

| Topic | File | Notes |
|---|---|---|
| Command code enum | `Products/Galahad2Vision/Commands.cs` | 24 commands; lines 5–30. |
| Request types (high-level) | `Products/Galahad2Vision/RequestType.cs` | 14 entries. Lines 5–21. Note: `RequestType` is a higher-level abstraction; it maps to one or more `Commands` codes inside the device class. Phase 0a only needs `Commands`. |

### Per-command source

| Command (code) | Read in | Send in | Response parser |
|---|---|---|---|
| `GetHandshakeInfo` (129) | `Galahad2VisionDevice.cs:107–119` (sync), `:204–217` (event) | `Galahad2VisionDevice.cs:250–253` (`SendGetHandshakeInfo`) | `HandshakeInfo.cs:58–76` (`TryParse`) |
| `GetFirmwareVersion` (134) | `Galahad2VisionDevice.cs` (around `FirmwareVersion`) | `Galahad2VisionDevice.cs:writeACommand(134)` | `FirmwareVersion.cs` |
| `SetFanPWM` (139) | n/a (write only) | `Galahad2VisionDevice.cs:280` (`writeACommand(139, new byte[]{0, pwm})`) | n/a |
| `SetPumpPWM` (138) | n/a | `Galahad2VisionDevice.cs:321` (`writeACommand(138, new byte[]{source, pwm})`) | n/a |
| `GetPumpLightingInfo` (130) | TBD Phase 0b | `Galahad2VisionDevice.cs:` `writeACommand(130)` | `PumpLightingInfo.cs` |
| `SetPumpLighting` (131) | n/a | `Galahad2VisionDevice.cs:312` (`writeACommand(131, setting.ToBytes())`) | n/a |
| `GetFanLightingInfo` (132) | TBD Phase 0b | similar | `FanLightingInfo.cs` |
| `SetFanLighting` (133) | n/a | `Galahad2VisionDevice.cs:271` | `FanLightingSetting.cs` `ToBytes` |
| `LCDControl` (12) | (B-packet) | `Galahad2VisionDevice.cs:LCDControl(...)` | `LCDControlSetting.cs` |
| `IsLCDAvailable` (23) | TBD Phase 0b | TBD | TBD |
| `SendJpeg` (14) | n/a | TBD Phase 0b | n/a |
| `UploadJpeg` (15) | n/a | TBD Phase 0b | n/a |
| `SendH264Frame` (13) | n/a | `Galahad2VisionDevice.cs:SendH264Frame` | n/a |
| `ResetDevice` (142) | n/a | `Galahad2VisionDevice.cs:233–246` | response Length=1, data[0]=0x01 = success |

### Crypto

| Topic | File | Status |
|---|---|---|
| Crypto helper | `Crypto/CryptoHelper.cs` | Needs Phase 0b investigation: is it called per-packet (transport encrypted) or only on theme/profile bundles? Search for `CryptoHelper.` callers. |

### Sensor / telemetry types

| Type | File |
|---|---|
| `HandshakeInfo` (live sensor reading) | `Products/Galahad2Vision/HandshakeInfo.cs` |
| `FirmwareVersion` | `Products/Galahad2Vision/FirmwareVersion.cs` |
| `SensorType` enum | `Products/Galahad2Vision/SensorType.cs` |

### Lighting types

| Type | File |
|---|---|
| `FanLightingMode` | `Products/Galahad2Vision/FanLightingMode.cs` |
| `PumpLightingMode` | `Products/Galahad2Vision/PumpLightingMode.cs` |
| `FanLightingSetting` | `Products/Galahad2Vision/FanLightingSetting.cs` |
| `PumpLightingSetting` | `Products/Galahad2Vision/PumpLightingSetting.cs` |
| `FanLightingInfo` | `Products/Galahad2Vision/FanLightingInfo.cs` |
| `PumpLightingInfo` | `Products/Galahad2Vision/PumpLightingInfo.cs` |
| `LightingDirection` | `Products/Galahad2Vision/LightingDirection.cs` |
| `ARGBSignalSource` | `Products/Galahad2Vision/ARGBSignalSource.cs` |
| `ScreenLEDLightingSetting` | `Products/Galahad2Vision/ScreenLEDLightingSetting.cs` |
| `ScreenLEDLightingSubSetting` | `Products/Galahad2Vision/ScreenLEDLightingSubSetting.cs` |

### LCD / screen types

| Type | File |
|---|---|
| `LCDControlMode` | `Products/Galahad2Vision/LCDControlMode.cs` |
| `LCDControlSetting` | `Products/Galahad2Vision/LCDControlSetting.cs` |
| `ScreenRotation` | `Products/Galahad2Vision/ScreenRotation.cs` |
| `SyncScreenInfo` | `Products/Galahad2Vision/SyncScreenInfo.cs` |
