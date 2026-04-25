# Galahad II LCDV2 — Protocol

**Device:** Lian Li Galahad II LCDV2 ("Vision")
**USB ID:** `0x0416:0x7395` (Winbond / Lian Li, GA_II-LCD)
**Verified firmware:** `1.6`
**Decompiled-source map:** [decompiled-source-map.md](./decompiled-source-map.md)
**Status legend:** `verified` (live capture exists), `codec-verified`
(byte-level codec covered by tests; no live device run yet), `inferred`
(only static analysis), `unknown` (TBD).

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

## `GetPumpLightingInfo` — Command 130 (`0x82`)

**Status:** `verified` (live-captured 2026-04-25 against firmware 1.6, see
[captures/2026-04-25-lighting-live.txt](./captures/2026-04-25-lighting-live.txt)).
- 16 pump modes returned: Rainbow, RainbowMorph, StaticColor, Breathing,
  Runway, Meteor, TickerTape, Fluctuation, Transmit, ColorfulStarryNight,
  StaticStarryNight, Voice, BigBang, Burst, ColorsMorph, Bounce.
- `max_speed = 4`, `max_brightness = 4` (matches the C# UI's percent / 25
  mapping: `0..=100` percent → `0..=4` wire byte).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:122-135`.
- Response parser: `PumpLightingInfo.cs:30-84` (escape-byte protocol).

### Request

A bare `ACommandPacket` with `Command = 130`, no data.

### Response

`ACommandPacket` echoing cmd 130 with a data slice that uses the
escape-byte protocol:

- Bytes are `PumpLightingMode` discriminants (mapped to `Unknown` for
  unknown values) until the literal sequence `0xA5 0x5A` terminates the
  list.
- After termination, two trailer bytes: `MaxSpeed`, `MaxBrightness`.
- Bug-compat: an `0xA5 X` sequence where `X != 0x5A` silently drops X
  without pushing a mode (mirrors the C# behaviour).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs`
  (`parse_pump_lighting_info`, `LightingInfo<M>`, type aliases)
- `crates/protocol/tests/galahad2_lcdv2_get_lighting.rs`,
  `crates/protocol/tests/galahad2_lcdv2_lighting_info.rs`

---

## `SetPumpLighting` — Command 131 (`0x83`)

**Status:** `verified` (live-confirmed 2026-04-25 across magenta, green,
yellow, and `--disable` writes — all rendered visibly on the pump-cap
LED ring with `--scope outer`. See
[captures/2026-04-25-lighting-live.txt](./captures/2026-04-25-lighting-live.txt)).

> **LCDV2 scope semantics:** only `--scope outer` (or `all`) drives the
> visible LED ring on this SKU. `--scope inner` (the `PumpLightingScope`
> default in the C# struct) writes successfully but produces no visible
> change — it appears to target a region the LCDV2 doesn't expose.
>
> **Transient buffering observed:** during one session of ~6-7 rapid
> back-to-back writes, the LED appeared "stuck" on the previously
> rendered colour. Subsequent writes were ACKed but not rendered until
> some idle time elapsed, after which the most recent buffered write
> rendered and normal responsiveness resumed. Cause is unknown — likely
> firmware rate-limiting, LCD-frame contention, or USB/HID stack stalls
> on the host side. The C# service's pattern of re-issuing cmd 131 on a
> timer (`Galahad2VisionController.cs:2443`) may be a workaround for
> exactly this. For verify-galahad usage: leave a few seconds between
> writes and the LED tracks the latest setting.

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:310-313`.
- Payload encoder: `PumpLightingSetting.cs:101-136` (19 bytes).

### Request

`ACommandPacket` with `Command = 131` and `Length = 19`. Data:

| offset | size | field        | type                    |
|--------|------|--------------|-------------------------|
| 0      | 1    | `scope`      | `PumpLightingScope`     |
| 1      | 1    | `mode`       | `PumpLightingMode`      |
| 2      | 1    | `brightness` | u8                      |
| 3      | 1    | `speed`      | u8                      |
| 4–6    | 3    | `color1`     | RGB                     |
| 7–9    | 3    | `color2`     | RGB                     |
| 10–12  | 3    | `color3`     | RGB                     |
| 13–15  | 3    | `color4`     | RGB                     |
| 16     | 1    | `direction`  | `LightingDirection`     |
| 17     | 1    | `disabled`   | bool (0/1)              |
| 18     | 1    | `source`     | `ArgbSignalSource`      |

No response. Visually verify by looking at the pump cap LEDs.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs` (`PumpLightingSetting`,
  `to_bytes`)

---

## `GetFanLightingInfo` — Command 132 (`0x84`)

**Status:** `verified` (live-captured 2026-04-25 against firmware 1.6, see
[captures/2026-04-25-lighting-live.txt](./captures/2026-04-25-lighting-live.txt)).
- 6 fan modes returned: Rainbow, RainbowMorph, StaticColor, Breathing,
  Runway, Meteor.
- `max_speed = 4`, `max_brightness = 4` (matches the C# UI's percent / 25
  mapping: `0..=100` percent → `0..=4` wire byte).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:73-86`.
- Response parser: `FanLightingInfo.cs:30-84` (same escape-byte
  protocol as pump-lighting-info; only the mode enum differs).

### Request

A bare `ACommandPacket` with `Command = 132`, no data.

### Response

Same shape as the `GetPumpLightingInfo` response except modes are
`FanLightingMode` (7 variants) instead of `PumpLightingMode` (17).
Implementation reuses the same generic `LightingInfo<M>` parser
(escape-byte rules, A5/5A terminator, two trailer bytes for
`MaxSpeed` / `MaxBrightness`).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/get_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs`
  (`parse_fan_lighting_info`)

---

## `SetFanLighting` — Command 133 (`0x85`)

**Status:** `verified` (live-confirmed 2026-04-25: fans went blue under
`color = #0000FF` with `mode = StaticColor`, see
[captures/2026-04-25-lighting-live.txt](./captures/2026-04-25-lighting-live.txt)).

> **Conditional visibility:** the cmd 133 write succeeds regardless of
> fan wiring, but the LED change is only visible when fans are
> physically connected to the AIO's fan header. On systems where fans
> plug directly into motherboard headers, the bytes are acknowledged
> but no fan LEDs are reachable. The pump's own LED ring is targeted
> separately by `SetPumpLighting` (cmd 131) with `scope = Outer`.

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:269-272`.
- Payload encoder: `FanLightingSetting.cs:109-145` (20 bytes).

### Request

`ACommandPacket` with `Command = 133` and `Length = 20`. Data:

| offset | size | field           | type                    |
|--------|------|-----------------|-------------------------|
| 0      | 1    | `mode`          | `FanLightingMode`       |
| 1      | 1    | `brightness`    | u8                      |
| 2      | 1    | `speed`         | u8                      |
| 3–5    | 3    | `color1`        | RGB                     |
| 6–8    | 3    | `color2`        | RGB                     |
| 9–11   | 3    | `color3`        | RGB                     |
| 12–14  | 3    | `color4`        | RGB                     |
| 15     | 1    | `direction`     | `LightingDirection`     |
| 16     | 1    | `disabled`      | bool (0/1)              |
| 17     | 1    | `source`        | `ArgbSignalSource`      |
| 18     | 1    | `sync_to_pump`  | bool (0/1)              |
| 19     | 1    | `number_of_led` | u8 (24 default; varies) |

No response. Visually verify by looking at fan-ring LEDs (if fans are
wired to the AIO's fan header).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/set_lighting.rs`
- `crates/protocol/src/galahad2_lcdv2/lighting.rs` (`FanLightingSetting`,
  `to_bytes`)

---

## `LCDControl` — Command 12 (`0x0C`)

**Status:** `codec-verified` (8-byte payload tested byte-by-byte; live
write not yet exercised).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:188-193`.
- Payload encoder: `LCDControlSetting.cs:7-73` (`ToBytes` returns 8 bytes).
- Modes enum: `LCDControlMode.cs:5-13`.
- Rotation enum: `ScreenRotation.cs:5-11` (wire bytes `0/1/2/3` corresponding to angles `0°/90°/180°/270°` — NOT literal angle values).
- Service controller's brightness-update profile: `Galahad2VisionController.cs:1483-1487`
  (`Mode=LCDSetting (4)`, `VideoFPS=24`).

### Request

`BCommandPacket` with `Command = 12` and `Length = 8`. Data:

| offset | size | field          | type                |
|--------|------|----------------|---------------------|
| 0      | 1    | `mode`         | `LCDControlMode`    |
| 1      | 1    | `brightness`   | u8 (0..=100; clamp via `Brightness::new`) |
| 2      | 1    | `rotation`     | `ScreenRotation` (wire 0/1/2/3) |
| 3      | 1    | `enable_test`  | bool (0/1)          |
| 4-6    | 3    | `test_color`   | RGB                 |
| 7      | 1    | `video_fps`    | u8 (known: 24, 30)  |

`LCDControlMode` discriminants: `LocalUI=0`, `Application=1`, `LocalH264=2`,
`LocalAVI=3`, `LCDSetting=4`, `LCDTest=5`.

No response expected.

### Caveats

- `LCDControlMode::LCDTest` puts the LCD into a solid-colour test pattern
  using `test_color`. Once entered, the only way out is another
  `LCDControl` write.
- `LCDControlMode::Application` may or may not be required as a preflight
  for `SetLEDColors` (cmd 20) on some firmware revisions; on FW 1.6 it is
  NOT required (live captures show cmd 20 renders without it).
- The `Brightness` newtype enforces 0..=100 at construction time.
- `video_fps` is a u8 with no codec-level clamp; known-safe values are 24
  (service brightness updates) and 30 (C# default constructor). Effects of
  other values are undocumented.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/lcd.rs` (`Brightness`,
  `LCDControlMode`, `ScreenRotation`, `LCDControlSetting`,
  `build_lcd_control_request`).
- `crates/protocol/tests/galahad2_lcdv2_lcd_control.rs`.

---

## `SendJpeg` — Command 14 (`0x0E`)

**Status:** `verified` (live-confirmed 2026-04-25 against firmware 1.6,
see [captures/2026-04-25-jpeg-upload-live.txt](./captures/2026-04-25-jpeg-upload-live.txt)).
Three sequential runs against varied 480x480 JPEGs (radial gradient
20343 B / 21 packets, strawberry 56162 B / 56 packets, clock face
45740 B / 46 packets in the cmd-15 cross-check) confirmed the
multi-chunk write is tear-free and the firmware decodes and displays
the JPEG within ~50 ms of the last packet.

**Sources:**
- Enum: `Galahad2Vision/Commands.cs:11` (`SendJpeg = 14`).
- Wire shape: derived from cmd 20 (`SetLEDColors`) by analogy and
  confirmed live in this commit's capture file. No located C# call
  site for cmd 14 itself.

### Request

`BCommandPacket` with `Command = 14`. The payload IS the raw JPEG
bytes (no `isFanLed` flag, no zero-pad, no frame header) — they
start at offset 11 of the first packet and span all chunks via the
standard `BCommandPacket::build_packets_for_write` chunker:
per-chunk Command/DataSize replicated, PacketNumber incrementing
from 0.

| offset (logical) | size       | content                |
|------------------|------------|------------------------|
| 0..              | 1..=N      | raw JPEG byte stream   |

The 480x480 LCD accepts JPEG-encoded frames; for arbitrary input
images, the `lconnect-image` workspace crate produces 480x480 JPEG
bytes ready to feed straight in.

No response expected.

### Caveats

- **Cmd 14 is a transient single-frame command, not a persistent
  display.** The firmware decodes and displays the JPEG within
  ~50 ms of the last packet but auto-reverts to LocalUI default
  after a bounded hold time. User-observed approximations on FW 1.6:
    - Default LocalUI mode: ~2 second hold.
    - `LCDControlMode::Application` (cmd 12 preflight): ~20 second
      hold (~10x extension).
  These numbers are empirical, observed by the user across the runs
  in the capture file; they are not firmware-documented timeouts.
- **For sustained static-image display, the host must refresh
  periodically.** A daemon-level refresh loop at ≤15 s intervals (in
  Application mode) keeps the image up indefinitely. The C# UI's
  default `LCDControlSetting.VideoFPS = 24` and the service's
  brightness-path `VideoFPS = 24` are consistent with a
  continuous-frame architecture for streamed dynamic content; we
  haven't proven that the firmware actually polls at 24 Hz, but the
  observed timeout extension under Application mode is consistent
  with that hypothesis.
- **Do not interleave cmd 14 (`SendJpeg`) and cmd 15 (`UploadJpeg`)
  writes on the same device handle.** If the firmware uses
  different buffering or apply semantics for the two commands,
  switching mid-transfer may leave the LCD decoder in an undefined
  state.
- **Partial-write recovery is undefined.** If a per-packet write
  fails on packet `N` of `M`, the firmware's transfer state is
  unknown. Recovery requires either a fresh complete transfer or a
  device reset (cmd 142).

### Implementation

- `crates/protocol/src/galahad2_lcdv2/jpeg.rs`
  (`build_send_jpeg_request`, `MAX_JPEG_BYTES`).
- `crates/protocol/tests/galahad2_lcdv2_send_jpeg.rs`,
  `crates/protocol/tests/galahad2_lcdv2_upload_jpeg.rs` (cross-check
  vs cmd 15).

---

## `UploadJpeg` — Command 15 (`0x0F`)

**Status:** `codec-verified` (byte-level codec tested and
cross-checked against cmd 14; live device run on 2026-04-25 against
firmware 1.6 saw 56-packet transfers ACK'd in full but produced no
visible LCD change in either default mode or after
`LCDControl{mode=Application}` preflight — see
[captures/2026-04-25-jpeg-upload-live.txt](./captures/2026-04-25-jpeg-upload-live.txt)).
The activation path on FW 1.6 is currently unknown.

**Sources:**
- Enum: `Galahad2Vision/Commands.cs:12` (`UploadJpeg = 15`).
- Wire shape: presumed identical to cmd 14 (cross-check tests pin
  byte-level equivalence except for the Command byte). No located
  C# call site for cmd 15 itself.

### Request

Same wire shape as cmd 14, only the Command byte changes:
`BCommandPacket` with `Command = 15` carrying the raw JPEG bytes via
`BCommandPacket::build_packets_for_write`.

| offset (logical) | size       | content                |
|------------------|------------|------------------------|
| 0..              | 1..=N      | raw JPEG byte stream   |

No response expected.

### Caveats

- **Cmd 15 produces no visible LCD activation on FW 1.6.** Both the
  default-mode and Application-mode preflight runs in the capture
  file ACK'd every 1024-byte write (46 and 56-packet transfers) but
  the LCD did not change. The activation path is unclear; plausible
  (unverified) explanations:
    - Stored to non-volatile storage for use as a boot / startup
      image; would only become visible after device power-cycle.
    - Requires a separate trigger command not in the documented
      `Galahad2Vision/Commands.cs` enum (cmds 10-23, 129-142 are all
      catalogued).
    - Used by an internal display mode (`LocalAVI = 3`,
      `LocalH264 = 2`) we have not activated via cmd 12.
- **Recommendation: do not use this codec in production daemon code
  until the activation path is empirically discovered.** Cmd 14 +
  `LCDControlMode::Application` + a daemon-level refresh loop is the
  established working path for static-image display.
- **Do not interleave cmd 14 and cmd 15 writes** — same caveat as
  cmd 14.
- **Partial-write recovery is undefined** — same caveat as cmd 14.
- **If cmd 15 ultimately turns out to write to non-volatile
  storage** (consistent with the broader `Send*` / `Upload*` naming
  pattern in the C# enums but unproven), CLI consumers should treat
  it as destructive: `verify-galahad upload-jpeg --mode upload`
  already gates on `--yes`.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/jpeg.rs`
  (`build_upload_jpeg_request`, `MAX_JPEG_BYTES`).
- `crates/protocol/tests/galahad2_lcdv2_upload_jpeg.rs`
  (cross-check vs cmd 14 pins byte-level equivalence).

---

## `SetLEDColors` — Command 20 (`0x14`)

**Status:** `verified` (live-confirmed 2026-04-25 against firmware 1.6 —
see [captures/2026-04-25-set-led-colors-live.txt](./captures/2026-04-25-set-led-colors-live.txt)).
Pump-cap screen ring rendered red, green, and yellow on three sequential
screen-path writes; fan-path write painted 2 of 3 fans (configuration-
specific, see Caveats).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:283-308` (chunked via `writeBCommand`,
  multi-packet for large LED arrays).

### Request

`BCommandPacket` with `Command = 20`. The first byte of the payload is
the `isFanLed` flag (`0x01` for fan ring, `0x00` for screen / pump-cap).
The rest of the payload is per-LED RGB. Multi-packet writes chunk
automatically via `BCommandPacket::build_packets_for_write`.

**Fan path** (`isFanLed = 0x01`):

| offset    | size  | content                                   |
|-----------|-------|-------------------------------------------|
| 0         | 1     | `isFanLed` flag (= `0x01`)                |
| 1..       | 3*n   | RGB triples `R0,G0,B0,...,Rn-1,Gn-1,Bn-1` |

Total length = `colors.len() * 3 + 1`.

**Screen path** (`isFanLed = 0x00`):

| offset    | size  | content                                   |
|-----------|-------|-------------------------------------------|
| 0         | 1     | `isFanLed` flag (= `0x00`)                |
| 1..25     | 24    | zero pad (8 padding Color slots × 3)      |
| 25..      | 3*n   | RGB triples `R0,G0,B0,...,Rn-1,Gn-1,Bn-1` |

Total length = `(colors.len() + 8) * 3 + 1`.

No response expected.

### Single-packet LED count limits

- Fan: `MAX_FAN_LEDS = 337` LEDs fit in one B-packet (`(1013 - 1) / 3`).
- Screen: `MAX_SCREEN_LEDS = 329` LEDs fit in one B-packet (`(1013 - 25) / 3`).

Larger arrays chunk transparently. The codec rejects empty arrays and
arrays exceeding these caps with `Error::InvalidLedCount`.

### Caveats

- **Fan ring is an aggregate of multiple fans' LEDs in series.** Each fan
  contributes ~16 LEDs sequentially. With `--count 16` only the first
  fan in the chain is painted; subsequent fans retain their previous
  state (whether from a prior cmd 20 or a cmd 133 preset). To paint an
  N-fan configuration uniformly, set `--count` to `16 * N`.
- **Cmd 20 supersedes cmd 131 in the LED render pipeline.** A pump cap
  held at a colour by cmd 131 (`SetPumpLighting`) will be overridden by
  the next cmd 20 write. Both commands are independently functional;
  the C# UI uses cmd 20 for dynamic / animated effects and cmd 131 for
  preset modes.
- **`Application` mode is NOT a prerequisite on FW 1.6.** The
  `--set-application-mode` flag in the verify-galahad CLI is a defensive
  escape hatch; live captures show cmd 20 renders without it.
- The screen-path 24-byte zero pad is a load-bearing wire-format
  invariant — see `Galahad2VisionDevice.cs:288-291` for the C# size
  calculation `(colors.Length + 8) * 3 + 1` and the `num = 25` start
  index. Don't refactor `screen_led_payload` to a non-zero-init
  allocation strategy.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/set_led_colors.rs` (`fan_led_payload`,
  `screen_led_payload`, `build_set_fan_led_colors_request`,
  `build_set_screen_led_colors_request`, `MAX_FAN_LEDS`, `MAX_SCREEN_LEDS`).
- `crates/protocol/tests/galahad2_lcdv2_set_led_colors.rs`.

---

## `IsLCDAvailable` — Command 23 (`0x17`)

**Status:** `verified` (live-confirmed 2026-04-25 against firmware 1.6,
see [captures/2026-04-25-set-led-colors-live.txt](./captures/2026-04-25-set-led-colors-live.txt)).

**Sources:**
- Wire flow: `Galahad2VisionDevice.cs:175-186`.
- Service controller's polling timer (which carries a latent stale-local bug not to replicate): `Galahad2VisionController.cs:2410-2411`.

### Request

`BCommandPacket` with `Command = 23`, no payload. Length = 0, DataSize = 0,
PacketNumber = 0.

### Response

`BCommandPacket` with `Command = 23` echo, `Length = 1`. The single data
byte is the availability sentinel:

| `Data[0]` | Meaning |
|---|---|
| `0x00` | LCD available (counterintuitive: zero means yes) |
| anything else (or Length != 1) | LCD unavailable |

### Caveats

- The C# service controller has a latent stale-local bug where it stores
  the result into `this.isAvailable` but the early-return check inspects
  a different `nullable` local. Linux callers should branch directly on
  the `Result<bool>` returned by `parse_is_lcd_available_response`.
- The polling cadence in the C# service is roughly 1 Hz; we don't yet
  know whether a tighter loop changes firmware behaviour.

### Implementation

- `crates/protocol/src/galahad2_lcdv2/lcd.rs` (`build_is_lcd_available_request`,
  `parse_is_lcd_available_response`).
- `crates/protocol/tests/galahad2_lcdv2_lcd_available.rs`.

---

## Commands not yet documented

The full enum lives in
[`commands.rs`](../../crates/protocol/src/galahad2_lcdv2/commands.rs).
Coverage in subsequent phases:

- **Phase 0c:** ✅ landed and live-verified 2026-04-25.
  `Get{Pump,Fan}LightingInfo` (cmd 130/132) round-trip the expected
  mode lists; `SetPumpLighting` (cmd 131) drives the LCDV2 pump-cap LED
  ring with `--scope outer` (the `Inner` default hits a region this SKU
  doesn't expose); `SetFanLighting` (cmd 133) drives fans wired to the
  AIO's fan header. Rapid back-to-back writes can transiently buffer
  with only the latest rendering — see the per-section status lines.
- **Phase 0d:** ⏳ mostly landed.
  - ✅ `LCDControl` (12) — codec-verified (see section above).
  - ✅ `SetLEDColors` (20) — live-verified 2026-04-25, screen-path
    rendered red/green/yellow on the pump cap; fan-path rendered 2 of 3
    fans (configuration-specific, see section's Caveats).
  - ✅ `IsLCDAvailable` (23) — live-verified 2026-04-25, round-trips
    `0x00` (available) on a healthy device.
  - ✅ `SendJpeg` (14) — live-verified 2026-04-25 against FW 1.6, see
    section above. Empirically a transient single-frame command:
    user-observed ~2 s hold in default LocalUI mode, ~20 s hold after
    `LCDControl{mode=Application}` preflight. Daemon-level static-image
    display will need a refresh loop.
  - ✅ `UploadJpeg` (15) — codec-verified; live 56-packet write ACK'd in
    full but no visible LCD activation on FW 1.6 in either default or
    Application-mode preflight. Activation path unknown (stored-for-boot
    / undocumented trigger / unactivated internal mode are the three
    plausible hypotheses); see section above. Not recommended for
    production daemon use until activation is empirically discovered.
  - ⏳ `SendH264Frame` (13) — stretch goal, not landed in Phase 0d.
  - ⏳ `UploadFirmware` (11), `ActiveFirmware` (10) — deferred to a later
    sub-phase (firmware-touching, higher risk).
- **Phase 1+:** `GetSerialNumber` (136), `SetSerialNumber` (137),
  `UploadAvi` (18), `UploadH264` (19), `UploadH264Info` (21),
  `UploadStartupAviMovie` (22). These are either rarely used or risky
  (writing serial numbers); they get implemented on demand.

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
