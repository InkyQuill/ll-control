# HydroShift LCD (HS I) — Cross-Device Survey

## Family

HydroShift LCD (the **original** HS I generation; AIO with LCD on the
pump cap). HS II is a separate device family living in its own
folder and is **not** covered here.

**Status legend:** ✅ verified live | 🌗 inferred (static-only) | ❓ unclear.
All claims here are 🌗 inferred unless explicitly noted otherwise — Phase
0e is a static-source survey and no live HS-I hardware run has been
performed in this project.

## Source folder

`L-Connect.Core/LConnectCore/Products/HydroShiftLCD/`

Files surveyed:

- `RequestType.cs` (high-level controller-side request enum)
- `Commands.cs` (wire byte → name map)
- `HydroShiftLCDDevice.cs` (device class; binds commands to framings)
- `IHydroShiftLCDDevice.cs` (public surface)
- `Constants.cs`, `HandshakeInfo.cs`, `FirmwareVersion.cs`
- `ACommandPacket.cs`, `BCommandPacket.cs`, `CCommandPacket.cs`
- `LCDControlMode.cs`, `LCDControlSetting.cs`, `ScreenRotation.cs`
- `FanLightingMode.cs`, `FanLightingSetting.cs`, `FanLightingInfo.cs`
- `PumpPWMSource.cs`, `ARGBSignalSource.cs`, `LightingDirection.cs`
- `SettingType.cs`, `SyncScreenInfo.cs`, `GetHandshakeInfoEventHandler.cs`

## Marketed names

Three retail SKUs share the **same** `HydroShiftLCDDevice` class and
`Commands` enum, distinguished only by USB PID:

| ControllerTypeID    | PID (decimal) | PID (hex) | Likely retail SKU         |
|---------------------|---------------|-----------|---------------------------|
| `HydroShiftLCD`     | 29592         | `0x7398`  | HydroShift LCD (base)     |
| `HydroShiftLCDRGB`  | 29593         | `0x7399`  | HydroShift LCD RGB        |
| `HydroShiftLCDTL`   | 29594         | `0x739A`  | HydroShift LCD TL (Trinity-Lite fans, presumably) |

## ControllerTypeID

`KnownDevices.cs:43,45,47` (HS I)
`ControllerTypeID.cs:20-22`:

- `HydroShiftLCD = 16850944`
- `HydroShiftLCDRGB = 16850945`
- `HydroShiftLCDTL = 16850946`

## VID:PID

| ControllerTypeID    | VID (decimal) | PID (decimal) | VID:PID (hex)     |
|---------------------|---------------|---------------|-------------------|
| `HydroShiftLCD`     | 1046          | 29592         | `0x0416:0x7398`   |
| `HydroShiftLCDRGB`  | 1046          | 29593         | `0x0416:0x7399`   |
| `HydroShiftLCDTL`   | 1046          | 29594         | `0x0416:0x739A`   |

VID `0x0416` is Winbond/Nuvoton (same MCU vendor used for Galahad II
LCDV2 at `0x0416:0x7395`). Source: `KnownDevices.cs:43,45,47`.

## Framing classes

HS I implements **all three** of the standard Lian-Li L-Connect framings.
Layouts are byte-for-byte identical to the Galahad II LCDV2 baseline
documented in `docs/protocol/galahad2-lcdv2.md`.

### Class A — `ACommandPacket` (64 in / 64 out) 🌗

**Source:** `HydroShiftLCD/ACommandPacket.cs:12-130`.

```
offset  size  field         description
------  ----  -----         ---------------------------------------------
  0      1   ReportID       always 0x01
  1      1   Command        wire byte
  2      1   reserved       always 0x00
  3      2   PacketNumber   big-endian u16
  5      1   Length         valid data bytes (0..=58)
  6     58   Data           command-specific payload, zero-padded
```

Constants: `PACKET_LEN = 64`, `HEADER_LEN = 6`, `PAYLOAD_MAX = 58`
(`ACommandPacket.cs:14-18`).

### Class B — `BCommandPacket` (1024 out / 512 in) 🌗

**Source:** `HydroShiftLCD/BCommandPacket.cs:12-189`.

```
offset  size  field          description
------  ----  -----          ---------------------------------------------
  0      1   ReportID        always 0x02
  1      1   Command         wire byte
  2      4   DataSize        big-endian u32; total payload across ALL chunks
  6      3   PacketNumber    big-endian u24; 0-based
  9      2   Length          big-endian u16; payload bytes IN THIS chunk
 11    1013   Data            up to 1013 bytes of payload, zero-padded
```

Constants: `OUTPUT_LEN = 1024`, `INPUT_LEN = 512`, `HEADER_LEN = 11`,
`PAYLOAD_MAX = 1013` (`BCommandPacket.cs:14-20`). Multi-packet chunker
behaves identically to LCDV2 (`BCommandPacket.cs:145-175`).

### Class C — `CCommandPacket` (512 out / 512 in) 🌗

**Source:** `HydroShiftLCD/CCommandPacket.cs:12-189`.

```
offset  size  field          description
------  ----  -----          ---------------------------------------------
  0      1   ReportID        always 0x03
  1      1   Command         wire byte
  2      4   DataSize        big-endian u32
  6      3   PacketNumber    big-endian u24
  9      2   Length          big-endian u16
 11    501   Data            up to 501 bytes of payload, zero-padded to 512
```

Constants: `OUTPUT_LEN = 512`, `INPUT_LEN = 512`, `HEADER_LEN = 11`,
`PAYLOAD_MAX = 501` (`CCommandPacket.cs:14-20`).

> **Divergence vs LCDV2 (this codebase):** the HS-I device class
> **explicitly exposes** the C-class H.264 path via `SendH264Frame512`
> (`HydroShiftLCDDevice.cs:252-257`). On LCDV2, the parallel
> `Galahad2VisionDevice.SendH264Frame512` exists but the corresponding
> CCommandPacket codec is currently deferred in the Linux port (see
> `galahad2-lcdv2.md` lines 76-79, 819-820). HS I treats both B and C
> framings as first-class for cmd 13.

## Subsystem fingerprint

| Subsystem            | Present? | Evidence |
|----------------------|----------|----------|
| Pump PWM             | 🌗 yes   | `HydroShiftLCDDevice.cs:290-297` (cmd 138) |
| Fan PWM              | 🌗 yes   | `HydroShiftLCDDevice.cs:264-271` (cmd 139) |
| Pump RPM read        | 🌗 yes   | `HandshakeInfo.cs:69-74` (`PumpRpm` field in handshake) |
| Fan RPM read         | 🌗 yes   | `HandshakeInfo.cs:69-74` (`FanRpm` field in handshake) |
| Coolant temperature  | 🌗 yes   | `HandshakeInfo.cs:71-74` (`TemperatureInteger`/`Fraction`) |
| Pump-cap LCD         | 🌗 yes   | `Constants.cs:43-47` (480×480 @ 24 fps); `IsLCDAvailable` cmd 23 |
| Fan-ring lighting    | 🌗 yes   | `Commands.cs:21-22` (132/133); `FanLightingMode.cs:1-23` (16 modes) |
| Per-LED RGB control  | 🌗 yes   | `HydroShiftLCDDevice.cs:273-288` (cmd 20) |
| **Pump-cap dedicated lighting** (cmds 130/131) | 🌗 **NO** | `Commands.cs` has no entries for 130/131; `HydroShiftLCDDevice.cs` has no `*PumpLighting*` methods |

## Commands table

All command bytes from `HydroShiftLCD/Commands.cs:7-27`. Framing class
inferred from binding in `HydroShiftLCDDevice.cs` write helpers.

| Cmd byte | Hex | Name | Framing | Direction | Source (HS I) | Status | Notes |
|---------:|----:|------|:-------:|:---------:|---------------|:------:|-------|
| 10 | `0x0A` | `ActiveFirmware`         | B | rw | `HydroShiftLCDDevice.cs:55-71` (writeBCommand + readBCommand) | 🌗 | Sends `[0x01]` payload; expects echo with `Length=1`, `Data[0]==1`, `Data[1]==1`. |
| 11 | `0x0B` | `UploadFirmware`         | B | rw | `HydroShiftLCDDevice.cs:299-304` | 🌗 | Raw firmware bytes; B-class chunked. **Destructive — Phase 1 only.** |
| 12 | `0x0C` | `LCDControl`             | B | rw | `HydroShiftLCDDevice.cs:173-178` | 🌗 | 8-byte payload from `LCDControlSetting.ToBytes()` (`LCDControlSetting.cs:62-72`). Identical to LCDV2. |
| 13 | `0x0D` | `SendH264Frame`          | B | rw | `HydroShiftLCDDevice.cs:245-250` | 🌗 | Raw H.264 frame; reads 512-byte response with 20 ms timeout (discarded). |
| 13 | `0x0D` | `SendH264Frame512`       | C | rw | `HydroShiftLCDDevice.cs:252-257` | 🌗 | **Same cmd byte as #13/B**, demuxed by ReportID at HID layer. C-class framing. |
| 14 | `0x0E` | `SendJpeg`               | B | w  | enum-only at `Commands.cs:11`; **no call site** in `HydroShiftLCDDevice.cs` | 🌗 | Same name/byte as LCDV2 cmd 14. By analogy: B-class chunked raw JPEG. |
| 15 | `0x0F` | `UploadJpeg`             | B | w  | enum-only at `Commands.cs:12`; **no call site** in `HydroShiftLCDDevice.cs` | ❓ | Same name/byte as LCDV2 cmd 15. **No new HS-I evidence on activation path.** |
| 18 | `0x12` | `UploadAvi`              | B | w  | enum-only at `Commands.cs:13`; no call site | ❓ | Naming suggests stored AVI. No call site on HS I. |
| 19 | `0x13` | `UploadH264`             | B | w  | enum-only at `Commands.cs:14`; no call site | ❓ | Naming suggests stored H.264. No call site on HS I. |
| 20 | `0x14` | `SetLEDColors`           | B | w  | `HydroShiftLCDDevice.cs:273-288` | 🌗 | Payload = `[0x01]` flag byte + RGB triples. **Differs from LCDV2 layout** — see Notable divergences below. |
| 21 | `0x15` | `UploadH264Info`         | B | w  | enum-only at `Commands.cs:16`; no call site | ❓ | No call site on HS I. |
| 22 | `0x16` | `UploadStartupAviMovie`  | B | w  | enum-only at `Commands.cs:17`; no call site | ❓ | **Name explicitly says "startup" — strong cmd-15 sibling hypothesis.** See Open questions. |
| 23 | `0x17` | `IsLCDAvailable`         | B | rw | `HydroShiftLCDDevice.cs:160-171` | 🌗 | Empty payload; response `Length=1`, `Data[0]==0x00` ⇒ available (counterintuitive zero-means-yes, identical to LCDV2). |
| 129 | `0x81` | `GetHandshakeInfo`      | A | rw | `HydroShiftLCDDevice.cs:107-120, 240-243` | 🌗 | Same 7-byte response shape as LCDV2 (`HandshakeInfo.cs:59-77`): FanRpm BE u16, PumpRpm BE u16, TempIsValid u8, TempInteger u8, TempFraction u8. |
| 132 | `0x84` | `GetFanLightingInfo`    | A | rw | `HydroShiftLCDDevice.cs:73-86` | 🌗 | Returns mode list + max-speed/max-brightness via the standard escape-byte (`A5 5A`) terminator protocol (`FanLightingInfo.cs:30-84`). |
| 133 | `0x85` | `SetFanLighting`        | A | w  | `HydroShiftLCDDevice.cs:259-262` | 🌗 | 20-byte payload from `FanLightingSetting.ToBytes()` (`FanLightingSetting.cs:86-106`). Identical layout to LCDV2 cmd 133. |
| 134 | `0x86` | `GetFirmwareVersion`    | A | rw | `HydroShiftLCDDevice.cs:88-105` | 🌗 | Two A-packets: version-string then datetime; identical pattern to LCDV2. |
| 136 | `0x88` | `GetSerialNumber`        | A | rw | enum-only at `Commands.cs:23`; no call site | ❓ | |
| 137 | `0x89` | `SetSerialNumber`        | A | w  | enum-only at `Commands.cs:24`; no call site | ❓ | Destructive on identity bytes. |
| 138 | `0x8A` | `SetPumpPWM`             | A | w  | `HydroShiftLCDDevice.cs:290-297` | 🌗 | 2-byte payload `[source, percent]` where `PumpPWMSource: MCU=0, Motherboard=1` (`PumpPWMSource.cs:5-9`). Clamps `percent` to 100. |
| 139 | `0x8B` | `SetFanPWM`              | A | w  | `HydroShiftLCDDevice.cs:264-271` | 🌗 | 2-byte payload `[0x00, percent]`. Group always 0; clamps to 100. |
| 142 | `0x8E` | `ResetDevice`            | A | rw | `HydroShiftLCDDevice.cs:223-238` | 🌗 | Empty payload; success iff response `Length=1`, `Data[0]==1`. **Destructive — drops device off USB.** |

> **Conspicuous absence vs LCDV2:** HS I's `Commands.cs` has **no
> `GetPumpLightingInfo` (130)** and **no `SetPumpLighting` (131)**.
> The Galahad II LCDV2 enum has both (`Galahad2Vision/Commands.cs:20-21`).
> The pump-cap LED ring on HS I appears to be addressable only via
> per-LED cmd 20 (`SetLEDColors`) and the fan-side broadcast cmd 133.

## Subsystem patterns

### Handshake (cmd 129)

Source `HandshakeInfo.cs:59-77`. Fields, all BE-where-multibyte:

| offset | size | field |
|-------:|-----:|-------|
| 0 | 2 | `FanRpm` (u16) |
| 2 | 2 | `PumpRpm` (u16) |
| 4 | 1 | `TemperatureIsValid` (bool) |
| 5 | 1 | `TemperatureInteger` (u8, °C) |
| 6 | 1 | `TemperatureFraction` (u8; tenths via `% 10`) |

Display temp formula: `int + (frac % 10) / 10` (`HandshakeInfo.cs:33-35`).
**Identical to LCDV2.**

### Pump RPM ranges (`Constants.cs:15-21`)

Two ranges are advertised, suggesting two pump SKU variants in the
HS-I family:

- Range 1: `2200..=3800` RPM
- Range 2: `2000..=3200` RPM

The presence of two ranges (vs LCDV2's single curve) is a HS-I
fingerprint worth surfacing during device probing if multi-SKU support
is added.

### LCD geometry (`Constants.cs:43-47`)

`ScreenWidth = 480`, `ScreenHeight = 480`, `ScreenFramePerSecond = 24`.
**Identical to LCDV2.** Frame budget for streaming paths: 480×480 RGB
≈ 691 KiB/frame raw, hence the JPEG/H.264 codec choice.

### Fan lighting modes (`FanLightingMode.cs:5-23`)

16 modes, byte values 1..=16:

```
1=Rainbow, 2=RainbowMorph, 3=StaticColor, 4=Breathing, 5=Runway,
6=Meteor, 7=TickerTape, 8=Fluctuation, 9=Transmit,
10=ColorfulStarryNight, 11=StaticStarryNight, 12=Voice, 13=BigBang,
14=Burst, 15=ColorsMorph, 16=Bounce
```

> **Divergence vs LCDV2:** LCDV2's *fan*-side enum has **6 modes**
> (Rainbow..Meteor — see `galahad2-lcdv2.md` line 354-356), while LCDV2's
> *pump*-side enum has all 16 of the modes listed above. HS I uses the
> 16-mode set on the **fan** side — i.e., HS I exposes the full 16
> animation modes through cmd 132/133 with no separate pump-lighting
> entry point.

### `LCDControlMode` (`LCDControlMode.cs:5-13`)

Discriminants identical to LCDV2:

| Wire | Name |
|----:|------|
| 0 | `LocalUI` |
| 1 | `Application` |
| 2 | `LocalH264` |
| 3 | `LocalAVI` |
| 4 | `LCDSetting` |
| 5 | `LCDTest` |

`LCDControlSetting.ToBytes()` produces 8 bytes:
`[mode, brightness, rotation, enable_test, R, G, B, video_fps]`
(`LCDControlSetting.cs:62-72`). Default `VideoFPS = 30`
(`LCDControlSetting.cs:51-58`); LCDV2's service uses 24 in the
brightness-update profile.

### `cmd 20 SetLEDColors` payload — **HS I divergence**

`HydroShiftLCDDevice.cs:273-288`:

```
size = colors.Length * 3 + 1
payload[0] = 0x01                   // hard-coded flag byte
payload[1..] = R0,G0,B0,R1,G1,B1,...
```

**HS I writes a single cmd-20 payload with flag = `0x01` always.** It
does **not** branch on a `isFanLed` flag the way LCDV2 does. There is
no zero-pad / 8-slot screen-path region. This is a meaningful
divergence: the LCDV2 wire format reserves bytes 1..25 as a 24-byte
zero pad on the screen path (see `galahad2-lcdv2.md` lines 698-704),
which HS I does not produce.

### Multi-packet chunking

Both `BCommandPacket.BuildPacketsForWrite` (`BCommandPacket.cs:145-175`)
and `CCommandPacket.BuildPacketsForWrite` (`CCommandPacket.cs:145-175`)
follow the same pattern as LCDV2: replicate `Command` and `DataSize`
on every chunk, increment `PacketNumber` from 0, zero-pad the trailing
region of the final chunk. The C# implementation reuses a single
buffer through the iterator — Linux-port chunkers should produce
independent owned buffers per chunk (same caveat as LCDV2).

### Handshake-loop event

`HydroShiftLCDDevice.cs:185-201` (`ReadACommandPacket`) parses any
incoming A-packet and, if it matches cmd 129 with a valid
`HandshakeInfo`, fires `OnGetHandshakeInfo`. This implies the firmware
emits **unsolicited** handshake packets, mirroring the LCDV2 pattern
where unsolicited cmd-`0x8E` status packets appear (see
`galahad2-lcdv2.md` line 162). Same drain-stale logic recommended.

### Write-abort retry

`HydroShiftLCDDevice.cs:336-373` shows a retry-on-`ERROR_IO_PENDING`
(`997`) loop with `IsWritePending` polling, max 3 retries with 10 s
timeouts. **Identical pattern to LCDV2**, suggesting a shared
`HidApiHelper`-style transport layer.

### `RequestType` vs `SettingType` (controller-side)

`RequestType.cs:5-20` (13 entries) is the **higher-level** controller
queue type, not a wire-byte enum. Notable entries that don't map 1-to-1
to a wire command:

- `SyncScreenImage` (index 2) — likely a high-level "push current
  rendered frame," dispatching to cmds 13/14/15 internally.
- `ScreenConfigJson` (index 4) and `ApplyDefaultScreenConfigJson`
  (index 10) — the **C# UI config layer** (likely sent via
  `SendH264Frame` or similar with a JSON payload, or via a CefSharp
  bridge); not a distinct wire command. ❓
- `EnterManualMode` (index 11) and `ExitManualMode` (index 12) —
  high-level on/off for an Application-mode push loop, presumably
  realised as `LCDControl{mode=Application/LocalUI}` writes. ❓

`SettingType.cs:5-14` (7 entries) covers the user-facing per-product
preference categories; no new wire-byte info.

## Notable divergences vs LCDV2 baseline

1. **No pump-lighting commands (130/131).** HS I has no
   `GetPumpLightingInfo` / `SetPumpLighting` in `Commands.cs`. The
   pump-cap LEDs are reachable only via the per-LED cmd 20 path (the
   16-LED pump-cap ring on LCDV2 corresponds to the screen-path of
   cmd 20 with the 24-byte zero pad; HS I appears to reach the pump
   cap differently — see point 3).
2. **Fan-side mode set is the full 16 modes**, not the LCDV2 6-mode
   subset. `FanLightingMode.cs:5-23` lists Rainbow..Bounce.
3. **cmd 20 wire format differs.** HS I always writes flag = `0x01`
   and a flat RGB triple stream. There is no `isFanLed` branch and no
   24-byte zero-pad region. Linux-port codecs cannot be shared
   between LCDV2 cmd 20 and HS I cmd 20.
4. **C-class H.264 path is first-class.** `HydroShiftLCDDevice.cs:252-257`
   exposes `SendH264Frame512` directly; LCDV2's equivalent is deferred.
5. **Two pump RPM ranges** in `Constants.cs:15-21` suggest the HS-I
   family covers two pump SKU variants. LCDV2 advertises a single
   range.
6. **Default `VideoFPS` is 30** (`LCDControlSetting.cs:51-58`), not the
   LCDV2 service's 24.
7. **Three sibling PIDs share one device class.** `HydroShiftLCD`,
   `HydroShiftLCDRGB`, and `HydroShiftLCDTL` (PIDs `0x7398`, `0x7399`,
   `0x739A`) bind to the same `HydroShiftLCDDevice` and the same
   `Commands` enum — they likely diverge only in fan-LED count and
   accessory wiring, not protocol.

## Open questions

1. **cmd-15 hypothesis from HS I — `UploadStartupAviMovie = 22`.**
   Source: `HydroShiftLCD/Commands.cs:17`. Name is **explicit**:
   "Startup AVI Movie." The `Upload*` family on HS I is:
   - `UploadFirmware = 11` (B-class; firmware blob) — known persistent.
   - `UploadJpeg = 15` — same byte/name as LCDV2 cmd 15.
   - `UploadAvi = 18`, `UploadH264 = 19`, `UploadH264Info = 21`,
     `UploadStartupAviMovie = 22`.

   The naming pattern `Send*` (cmds 13, 14) for transient frames vs
   `Upload*` (cmds 11, 15, 18, 19, 21, 22) for stored content is
   **internally consistent across HS I**. Crucially, cmd 22's
   `Startup` qualifier strongly implies the family writes to
   non-volatile storage for boot-time / "until-overwritten" display.
   By analogy, cmd 15 (`UploadJpeg`) is plausibly the JPEG sibling of
   cmd 22 — a stored / boot / power-on splash JPEG. This is a strong
   candidate hypothesis for the LCDV2 cmd-15 activation puzzle:
   **cmd 15 may not need a separate "activate" command on the same
   USB session — it may only become visible after a firmware-driven
   power-cycle or upon entering an LCD mode that displays the stored
   slot** (e.g., `LocalAVI = 3` for AVI, possibly an undocumented
   counterpart for JPEG).

   ❓ Static evidence cannot distinguish among:
   - cmd 15 = boot logo (visible only after power-cycle), or
   - cmd 15 = "stored slot" displayed when `LCDControlMode` enters a
     specific value (none of the documented values 0..5 obviously
     selects "stored JPEG"), or
   - cmd 15 needs a follow-up activation command **not in the
     enum** (the enum is `byte`-typed and 10..23, 129..142 are
     catalogued — there are still unenumerated bytes such as 16, 17,
     24..127, 135, 140, 141, 143..255 that the firmware could
     honour).

   Recommended HS-I empirical follow-up: upload via cmd 15, then
   **power-cycle** the device (USB unplug + replug) and observe the
   LCD on next boot. If the uploaded JPEG appears as a splash, the
   stored-boot-image hypothesis is confirmed.

2. **No HS-I-specific "activate stored image" command found.** Beyond
   the shared enum, there is nothing in `HydroShiftLCD/` that
   resembles "ActivateStoredImage", "DisplayBootLogo",
   "ShowSavedFrame", etc. The text strings searched include: "boot",
   "logo", "default", "stored", "saved", "flash", "persistent",
   "startup", "splash". The **only** match is
   `UploadStartupAviMovie = 22` (cited above) and
   `RequestType.ApplyDefaultScreenConfigJson` (an upper-layer concept,
   no direct wire byte; `RequestType.cs:17`).

3. **Does HS I actually accept cmd 15?** No call site for cmd 15 in
   `HydroShiftLCDDevice.cs`. Only the enum entry exists. It is
   possible the byte is reserved across the family but not implemented
   on HS I firmware. Same for cmds 14, 18, 19, 21, 22.

4. **`SyncScreenImage` (RequestType index 2)** — high-level entry
   point's wire-byte mapping is not pinned in the device class. Is it
   `SendJpeg (14)`? `SendH264Frame (13)`? Both? The mapping likely
   lives in the upper-layer controller (not in `L-Connect.Core`); not
   resolvable from `HydroShiftLCD/` alone.

5. **Pump-cap LED ring addressing on HS I.** With no cmd 130/131,
   how does HS I drive the pump-cap ring? Plausible (unverified):
   - cmd 20 `SetLEDColors` with the flat-flag layout addresses both
     fan and pump LEDs in a single concatenated chain (different
     byte offsets per SKU), or
   - cmd 133 `SetFanLighting` with `SyncToPump = true`
     (`FanLightingSetting.cs:65-70`) cascades to the pump ring.

   Unresolvable from static source.

6. **Cmd 13 dual-framing.** Both `SendH264Frame` (B) and
   `SendH264Frame512` (C) write cmd byte 13. The HID layer
   demultiplexes via ReportID (`0x02` vs `0x03`). No HS-I-specific
   evidence indicates when each path is used, but the explicit
   exposure of the C-class path in the device class (vs LCDV2 where
   it's deferred) suggests HS I firmware actively prefers the
   higher-throughput C-class for streaming video.
