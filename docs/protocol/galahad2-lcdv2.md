# Galahad II LCDV2 — Protocol

**Device:** Lian Li Galahad II LCDV2 ("Vision")
**USB ID:** `0x0416:0x7395` (Winbond / Lian Li, GA_II-LCD)
**Verified firmware:** `1.6`
**Decompiled-source map:** [decompiled-source-map.md](./decompiled-source-map.md)
**Status legend:** `verified` (live capture exists), `inferred` (only static
analysis), `unknown` (TBD).

This document supersedes `PROTOCOL_DOCUMENTATION_RU.md` for any command
listed below. The Russian doc remains as a legacy reference for commands
not yet covered here.

---

## Packet framing — `ACommandPacket` (64 bytes)

**Source:** `LConnectCore/Products/Galahad2Vision/ACommandPacket.cs:12-80`.
**Status:** `verified` for handshake (responses arrive with this layout).

```
offset  size  field         description
------  ----  -----         ---------------------------------------------
  0      1   ReportID       always 0x01
  1      1   Command        e.g. 0x81 = 129 = GetHandshakeInfo
  2      1   reserved       always 0x00 in current firmware
  3      2   PacketNumber   big-endian u16; 0 unless multi-packet upload
  5      1   Length         valid data bytes (0..=58)
  6     58   Data           command-specific payload, zero-padded
```

Constants:

- `PACKET_LEN = 64`
- `HEADER_LEN = 6`
- `MAX_PAYLOAD = 58`

---

## `GetHandshakeInfo` — Command 129 (`0x81`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-handshake-live.txt](./captures/2026-04-25-handshake-live.txt)).

**Sources:**
- Send: `Galahad2VisionDevice.cs:250-253` (`SendGetHandshakeInfo`)
- Receive (sync): `Galahad2VisionDevice.cs:107-119`
- Decode: `HandshakeInfo.cs:58-76`

### Request (host → device)

A bare `ACommandPacket` with `Command = 129` and no data:

```
01 81 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00  00 00 00 00 00 00 00 00 00 00
```

### Response (device → host)

`ACommandPacket` with `Command = 129` echo and `Length = 7`. Data bytes:

| offset (data) | offset (raw) | size | field            | encoding              |
|---------------|--------------|------|------------------|-----------------------|
| 0             | 6            | 2    | `FanRpm`         | big-endian u16, RPM   |
| 2             | 8            | 2    | `PumpRpm`        | big-endian u16, RPM   |
| 4             | 10           | 1    | `TemperatureIsValid` | `0` or `1`        |
| 5             | 11           | 1    | `TemperatureInteger` | u8, °C             |
| 6             | 12           | 1    | `TemperatureFraction`| u8, **% 10 = tenths °C** |

Display temperature: `TemperatureInteger + (TemperatureFraction % 10) / 10`.

The C# `IsValid` field is set true unconditionally inside `TryParse` whenever
the buffer is ≥ 7 bytes; we model failure with `Result` instead.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/handshake.rs`
- `crates/protocol/tests/galahad2_lcdv2_handshake.rs`

---

## `GetFirmwareVersion` — Command 134 (`0x86`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-firmware-live.txt](./captures/2026-04-25-firmware-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:88-105` (writes cmd 134, reads two `ACommandPacket`s).
- Parser: `FirmwareVersion.cs` (7 comma-separated fields + raw datetime).

### Request

A bare `ACommandPacket` with `Command = 134` and no data.

### Response (two packets)

Both packets echo `Command = 134`, distinguished by **PacketNumber** (BE u16 at bytes[3..4]):

| PacketNumber | Content |
|---|---|
| 0 | Version string, 7 comma-separated fields |
| 1 | Build datetime (free-form), **only sent once per USB-attach lifecycle on FW 1.6** |

Version-string fields, in order:
1. `chip_code` — e.g., `"N9"`
2. `boot_code` — e.g., `"01"`
3. `kind` — e.g., `"HS"` (HydroShift cooler family)
4. `model_a` — e.g., `"SQ"` (square pump-cap variant)
5. `model_b` — e.g., `"CA_II-Vision"` (matches USB descriptor)
6. `software_version` — e.g., `"V2.01.041"`
7. `user_version` — e.g., `"1.6"` (matches USB descriptor `LianLi-GA_II-LCD_v1.6`)

### Real-hardware quirks (FW 1.6)

- **Datetime packet (PacketNumber=1) is unreliable.** Observed only once during verification — likely sent at most once per USB-attach. Subsequent firmware requests return only the version packet.
- **Unsolicited cmd-0x8e status packets** sometimes appear in the read queue between commands. Drain stale packets before each read.
- **PacketNumber MUST be used to demux multi-packet responses.** The decompiled C# reads sequentially without filtering, which is fragile against unsolicited interleaved packets.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/firmware.rs`
- `crates/protocol/tests/galahad2_lcdv2_firmware.rs`
- Live verification: `tools/verify-galahad/src/main.rs` (drains stale, filters by cmd echo + PacketNumber, tolerates missing datetime).

---

## `SetFanPWM` — Command 139 (`0x8B`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-fan-pwm-live.txt](./captures/2026-04-25-fan-pwm-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:271-281`.

### Request

`ACommandPacket` with `Command = 139` and `Length = 2`. Data:

| offset (data) | size | field | encoding |
|---|---|---|---|
| 0 | 1 | `group` | always `0x00` for LCDV2 |
| 1 | 1 | `percent` | u8, `0..=100` (firmware clamps higher values) |

No response is expected; the command is fire-and-forget. Verify the effect by issuing `GetHandshakeInfo` afterward and observing `fan_rpm`.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/fan_pwm.rs` + `pwm.rs` (the `Pwm` newtype enforces the 0..=100 invariant).

---

## `SetPumpPWM` — Command 138 (`0x8A`)

**Status:** `verified` (live-captured 2026-04-25, see
[captures/2026-04-25-pump-pwm-live.txt](./captures/2026-04-25-pump-pwm-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:316-322`.
- Source enum: `PumpPWMSource.cs` (`MCU = 0`, `Motherboard = 1`).

### Request

`ACommandPacket` with `Command = 138` and `Length = 2`. Data:

| offset (data) | size | field | encoding |
|---|---|---|---|
| 0 | 1 | `source` | `0` MCU (firmware curve), `1` Motherboard (MB header) |
| 1 | 1 | `percent` | u8, `0..=100` |

No response. Verify via `GetHandshakeInfo`'s `pump_rpm`.

**Operational note:** running the pump below 30% can let coolant temperature spike on idle systems — only do so when you understand the thermal margin.

**MCU-mode caveat:** when `source = MCU` (the default), the firmware drives the pump from its own internal temperature curve and treats `percent` as a *cap*. On real hardware (FW 1.6, coolant 40.8°C), setting 75% and 100% in MCU mode both produced the same `pump_rpm = 2610` because the firmware's curve output was below both caps. To see direct percent control, use `source = Motherboard`.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/pump_pwm.rs` + `pwm.rs`.

---

## `ResetDevice` — Command 142 (`0x8E`)

**Status:** `inferred` (codec is TDD-tested against synthetic packets;
not run live to avoid dropping the pump during normal operation).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:233-248`. Success iff response
  `Length == 1 && Data[0] == 0x01`.

### Request / response

Request: `ACommandPacket` with `Command = 142` and no data.
Response: `ACommandPacket` with `Command = 142` echo, `Length = 1`,
`Data[0] = 0x01`.

The device drops off USB for 1–3 seconds after acknowledging — the
caller should expect to re-open the device handle.

The `verify-galahad reset` subcommand requires `--yes-i-really-want-to-reset`
to actually fire the command; without that flag it refuses and prints a
warning, ensuring no accidental resets during testing.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/reset.rs`

---

## Commands not yet documented

The full enum lives in
[`commands.rs`](../../crates/protocol/src/galahad2_lcdv2/commands.rs).
Coverage in subsequent phases:

- **Phase 0c:** `GetPumpLightingInfo` (130), `SetPumpLighting` (131),
  `GetFanLightingInfo` (132), `SetFanLighting` (133). Adds the lighting
  configuration types (modes, speeds, directions, brightness, ARGB sources).
- **Phase 0d:** `LCDControl` (12), `IsLCDAvailable` (23), `SendJpeg` (14),
  `UploadJpeg` (15), `SendH264Frame` (13), `UploadFirmware` (11),
  `ActiveFirmware` (10). Introduces `BCommandPacket` (512-byte) framing
  for bulk transfers.
- **Phase 1+:** `GetSerialNumber` (136), `SetSerialNumber` (137),
  `SetLEDColors` (20), `UploadAvi` (18), `UploadH264` (19),
  `UploadH264Info` (21), `UploadStartupAviMovie` (22). These are either
  rarely used or risky (writing serial numbers); they get implemented
  on demand.

## Crypto status

`LConnectCore.Crypto.CryptoHelper` is **not** used at the HID transport
level — confirmed empirically (Phase 0a unencrypted handshake) and by
static analysis (see [crypto-usage.md](./crypto-usage.md)). The single
caller in the entire decompiled tree (`L-Connect-Service/...
TLFanController.cs:571`) wraps buffered handshake records for outbound
diagnostic telemetry; not on any wire-format path. Phase 0d revisits
when firmware-upload codecs land — note that `CryptoHelper` exposes
only `Encrypt`, no `Decrypt`, so any encrypted firmware-blob assumption
needs separate empirical evidence.
