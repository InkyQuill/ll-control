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

## Commands not yet documented

The full enum lives in
[`commands.rs`](../../crates/protocol/src/galahad2_lcdv2/commands.rs).
Coverage in subsequent phases:

- **Phase 0b:** `GetFirmwareVersion` (134), `SetFanPWM` (139),
  `SetPumpPWM` (138), fan curve, lighting (130-133), `LCDControl` (12),
  `IsLCDAvailable` (23), `ResetDevice` (142), `SendJpeg` (14),
  `UploadJpeg` (15), `SendH264Frame` (13).
- **Phase 0b:** crypto investigation (whether `CryptoHelper.cs` wraps
  packets or only theme bundles).
