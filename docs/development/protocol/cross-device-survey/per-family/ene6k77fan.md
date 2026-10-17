## Family: Ene6K77Fan

**Source folder:** `LConnectCore/Products/Ene6K77Fan/`

**Marketed name(s):** ENE 6K77-class Lian Li fan controllers — the
older RGB-fan controller line that predates TLFan. Five concrete SKU
classes share `IEne6K77FanDevice` (`IEne6K77FanDevice.cs:7-38`):
`SLFanDevice` (Strimer-style SL fans), `ALFanDevice` (AL fans),
`SLInfinityDevice` (SL Infinity), `SLV2FanDevice` (SL V2),
`ALV2FanDevice` (AL V2), `SLRedragonDevice` (Redragon-branded SL).
Each maps to a distinct VID:PID below.

**ControllerTypeID enum value(s):**
`SLFan = 16781312`, `SLRedragon = 16781313`, `ALFan = 16785408`,
`SLInfinity = 16789504`, `SLV2Fan = 16793600`, `SLV2AFan = 16793601`,
`ALV2Fan = 16797696`
(`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:8-14`).

**USB VID:PID:** Every Ene6K77Fan SKU sits on VID `0x0CF2` (decimal
`3314`, ENE Technology — the Realtek/ENE-branded MCU vendor that names
this family) with sequential PIDs (`KnownDevices.cs:21-35`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | UsageID | Source |
|---|---|---|---|---|
| `SLFan` (16781312) | `0x0CF2:0xA100` | `3314:41216` | (none) | `KnownDevices.cs:21` |
| `ALFan` (16785408) | `0x0CF2:0xA101` | `3314:41217` | (none) | `KnownDevices.cs:23` |
| `SLInfinity` (16789504) | `0x0CF2:0xA102` | `3314:41218` | (none) | `KnownDevices.cs:25` |
| `SLV2Fan` (16793600) | `0x0CF2:0xA103` | `3314:41219` | (none) | `KnownDevices.cs:27` |
| `ALV2Fan` (16797696) | `0x0CF2:0xA104` | `3314:41220` | (none) | `KnownDevices.cs:29` |
| `SLV2AFan` (16793601) | `0x0CF2:0xA105` | `3314:41221` | (none) | `KnownDevices.cs:31` |
| `SLRedragon` (16781313) | `0x0CF2:0xA106` | `3314:41222` | (none) | `KnownDevices.cs:33` |

VID `0x0CF2` is shared with `StrimerPlus` (PID `0xA200`) — both come
out of the same ENE chip family but each takes a distinct PID.

**Framing classes used:** **family-specific (E-class).** Unlike LCDV2's
A/B-class HID output reports or TLFan's 64-byte ReportID-1 packets,
Ene6K77Fan uses **HID feature reports** (host-to-device) and **HID
input reports** (device-to-host) keyed by `ReportID = 0xE0`
(decimal 224 — the literal opening byte on every wire buffer in
`SLFanDevice.cs:53-200`).

```
offset  size  field            description
------  ----  -----            -------------------------------------------
  0      1   ReportID         always 0xE0 (224)
  1      1   tag/cmd          high nibble = command class, low nibble = address
                                tag values observed: 0x10 (group/global op),
                                0x20 (set fan speed), 0x30 (color stream),
                                0x50 (read query), 0x55 (read frame variant),
                                0x60 (set frame)
  2     ..   sub-cmd / data   command-specific; total length varies per cmd
                                (no fixed packet size — write length = data.length)
```

Constants per device file: `ReportID = 224`, `InputReportDataSize = 64`
(`SLFanDevice.cs:11-13`, `ALFanDevice.cs:11-13`, etc.). The host
issues each request as a `SendFeatureReport(data, data.Length)` of
variable length (3..6 bytes for tags, `2 + colors.Length * 3` for
color streams). Input is read by allocating a 65-byte buffer with
byte 0 set to `0xE0` and calling `GetInputReport` — the response
length needed is command-specific (5 bytes for firmware/frame, 8 or
9 bytes for fan speed). This is **NOT** the same wire shape as
either TLFan or LCDV2 — different transport class, different
multiplexing, different MCU command space.

A separate path uses `Write` (output report, `sendOutputReport`) for
**color streams** (`SLFanDevice.cs:122-127`), distinct from the
feature-report path used for control commands. So this family
actually uses **two HID transport channels**: feature reports for
config/RPM/effect control, output reports for raw color data.

**Phase 0d status:** not in scope; static-analysis only.

**Subsystem fingerprint:**
- Sensors / handshake: **partial** — `GetFanSpeed` (tag `0xE0 0x50 0x00`)
  is the closest analogue to LCDV2's handshake; it returns 8 or 9
  bytes (per-SKU) decoded as 4 × BE u16 fan RPM values
  (`SLFanDevice.cs:61-75`, `ALFanDevice.cs:61-75`, `SLV2FanDevice.cs:61-75`).
  No temperature, no pump RPM, no PWM read-back.
- Fan PWM: **yes** — `SetFanSpeed(groupIndex, duty)` writes
  `[0xE0, 0x20|group, 0x00, duty]` as a 4-byte feature report
  (`SLFanDevice.cs:163-166`). PWM is u8 0..=100 percent (no clamping
  in the device class — caller is expected to pre-clamp). Per-group
  rather than per-fan; `groupIndex` is encoded in the low nibble of
  byte 1.
- Pump PWM: **N/A** — no pump on this family.
- Fan curve upload: **no on-device curve.** Like TLFan, all curve
  evaluation runs host-side. The service controllers
  (`L-Connect-Service/LConnectService/Products/Ene6K77Fan/{SLFan,
  ALFan, ...}Controller.cs`) re-issue `SetFanSpeed` per group on a
  timer.
- Lighting (preset modes): **yes** — `SetEffectSetting(port, mode,
  speed, direction, brightness)` writes
  `[0xE0, 0x10|port, mode, speed, direction, brightness]`
  (`SLFanDevice.cs:151-154`). 6-byte feature report. Mode/speed/
  direction/brightness are each 1 byte. Modes are listed in
  `LightingMode.cs` (this family's own enum, not the TLFan one).
- Lighting (per-pixel LED): **yes via output report** —
  `SetColorSetting(port, Color[])` builds
  `[0xE0, 0x30|port, R, B, G, R, B, G, ...]` as an output report
  (`SLFanDevice.cs:129-149`). **Note the byte order: R, B, G — not
  R, G, B.** The C# explicitly reads `color.R`, `color.B`, `color.G`
  in that order (`SLFanDevice.cs:142-145`). This is a wire-format
  invariant, not a decompiler artefact (the same pattern repeats in
  every `*FanDevice.cs` file in this folder).
- LCD on/off / brightness / rotation: **N/A** — no LCD on this family.
- LCD frame upload: **N/A** — but there is a "frame" concept
  (`GetFrame` / `SetFrame`, tags `0xE0 0x55 0x05` / `0xE0 0x60 ...`,
  `SLFanDevice.cs:86-95`, `168-171`) which appears to be an
  **animation frame index** (not a JPEG/H.264 upload). `SetFrame`
  takes a u16 frame number split into two bytes; `GetFrame` returns
  it. Plausibly used to step through a multi-frame stored animation
  on the controller MCU, but the actual frame contents are not
  uploaded by this device class — flagged unclear.
- Continuous re-issue pattern: **yes** for fan speed (host curve loop).
- Reset / soft-reset: **no.** No `ResetDevice` analogue.
- Firmware upload / activation: **none in this device class.** Unlike
  TLFan which has `SendFirmware`/`ActiveFirmware` enum values (even
  if unused at runtime), the Ene6K77Fan device class has no firmware-
  flashing methods — flashing is presumably out-of-tree.
- Crypto / obfuscation at HID transport: **none observed.**

### Commands

The wire is "tag-byte multiplexed" — there is no single "command
enum" in `RequestType.cs`; instead, `RequestType` is a logical
host-side dispatch enum (`RequestType.cs:5-14`) with seven entries:
`FirmwareVersion, GetFanSpeed, SetFanSpeed, LightingSetting,
FanQuantity, MergeOrder, Ping`. The actual wire-level commands are
distinct tag-byte values inside the device class. Status flag
`inferred-static` for all entries — no live verification.

| Logical name | Tag bytes (byte 1, byte 2) | Direction | Citation | Notes |
|---|---|---|---|---|
| `GetFanSpeed` | `0x50 0x00` | host->dev (write) + dev->host (5..9-byte input report) | `SLFanDevice.cs:61-75`, `ALV2FanDevice.cs:61-75` | Returns 4 × BE u16 fan RPM. SLFan/SLInfinity/SLRedragon read 8 bytes starting at offset 0; ALFan/SLV2Fan/ALV2Fan read 9 bytes starting at offset 1 (header byte then RPMs). |
| `GetFirmwareVersion` | `0x50 0x01` | host->dev + dev->host (5-byte input report) | `SLFanDevice.cs:77-84` | Decoded by per-SKU `*FirmwareVersion.cs::Load(byte[])`. Layout differs per SKU. |
| `GetFrame` | `0x55 0x05` (SL/SLInfinity), `0x80 0x05` (SLV2 — see SLV2FanDevice.cs:88) | host->dev + dev->host (5-byte input report) | `SLFanDevice.cs:86-95`, `SLV2FanDevice.cs:86-95`, `ALFanDevice.cs:86-95` | Returns a u16 frame index. ALFan/ALV2Fan additionally check `inputReportData[0] == 0xF5` as a sentinel. |
| `Ping` | `0x10\|group, 0x11/0x17/0x34/...` | host->dev (6-byte feature report) | `SLFanDevice.cs:110-113`, `ALFanDevice.cs:110-113`, `SLV2FanDevice.cs:110-120`, `ALV2FanDevice.cs:110-120` | Per-SKU sub-tag: SLFan/SLV2Fan use `0x11`, ALFan uses `0x34`, SLInfinity uses `0x3E`, ALV2Fan uses `0x36`. ALV2Fan/SLV2Fan also have a "merged ping" path using sub-tag `0x50` / `0x2F`. |
| `SetColorSetting` | `0x30\|port` (output report, not feature) | host->dev | `SLFanDevice.cs:129-149` | `[0xE0, 0x30\|port, R, B, G, R, B, G, ...]`. Length = `2 + colors.Length * 3`. **Wire color order is R, B, G** — not R, G, B. |
| `SetEffectSetting` | `0x10\|port, mode` | host->dev | `SLFanDevice.cs:151-154` | `[0xE0, 0x10\|port, mode, speed, direction, brightness]`. 6-byte feature report. |
| `SetFanMotherboardSync` | `0x10 0x31` (SLFan), `0x10 0x42` (ALFan), `0x10 0x61` (ALV2Fan/SLV2Fan/SLInfinity) | host->dev | `SLFanDevice.cs:156-161`, `ALFanDevice.cs:163-168`, `ALV2FanDevice.cs:163-168` | 6-byte feature report. Byte 3 packs `(1<<(group+4)) \| (isSync<<group)` — high nibble enables the bit, low nibble carries the value. |
| `SetFanSpeed` | `0x20\|group, 0x00` | host->dev | `SLFanDevice.cs:163-166` | `[0xE0, 0x20\|group, 0x00, duty]`. 4-byte feature report. PWM 0..=100. |
| `SetFrame` | `0x60` | host->dev | `SLFanDevice.cs:168-171` | `[0xE0, 0x60, frame_hi, frame_lo]`. 4-byte feature report. |
| `SetLightingMotherboardSync` | `0x10 0x30` (SLFan), `0x10 0x41` (ALFan), `0x10 0x61` (ALV2/SLV2/SLInfinity) | host->dev | `SLFanDevice.cs:173-178`, `ALFanDevice.cs:180-185` | 6-byte feature report. Byte 3 = `0/1`. |
| `SetMergeOrder` | `0x10 0x33` (SLFan), `0x10 0x63` (ALV2/SLV2/SLInfinity) | host->dev | `SLV2FanDevice.cs:187-195`, `ALV2FanDevice.cs:187-195` | 8-byte feature report carrying 4 group indices. **No-op on SLFan and ALFan** (`SLFanDevice.cs:180-182`, `ALFanDevice.cs:187-189` — the method body is empty). |
| `SetQuantity` | `0x10 0x32` (SLFan), `0x10 0x40` (ALFan), `0x10 0x60` (ALV2/SLV2/SLInfinity) | host->dev | `SLFanDevice.cs:184-189`, `ALFanDevice.cs:191-197` | Per-SKU encoding: SLFan packs `(group<<4 \| qty&0x0F)` into byte 3; ALFan/ALV2Fan use byte 3 = `group+1`, byte 4 = `qty`. SLV2Fan packs `(group<<4 \| qty&0x0F)`. |
| `StartMerge` | `0x10 0x33 0x00 0x01 0x02 0x03 0x08` | host->dev | `SLFanDevice.cs:191-194` | 8-byte feature report. **SLFan-only** — present on `SLFanDevice` but not in the interface (`IEne6K77FanDevice.cs`). |
| `StopMerge` | `0x10 0x34 0x00 0x00 0x00` | host->dev | `SLFanDevice.cs:196-199` | 6-byte feature report. **SLFan-only** — same caveat. |
| `SendMergeCommand(enable)` | `0x10 0x43, byte3=enable` | host->dev | `ALFanDevice.cs:122-127` | 6-byte feature report. **ALFan-only.** |

### Subsystem patterns

- **Tag-byte multiplexed wire.** No `Command` enum byte at a fixed
  offset like LCDV2/TLFan. Each method in `*FanDevice.cs` builds a
  hand-rolled byte array starting with `0xE0` (ReportID) and
  follows it with a tag byte that encodes both the operation class
  (high nibble) and its target (low nibble — port, group, or fixed
  zero). The "command space" is the full 256 possible byte-1 values
  intersected with whatever sub-tag the caller passes in byte 2.
- **Two HID transport channels.** Feature reports
  (`SendFeatureReport`) for the bulk of commands; output reports
  (`Write`) for color streams. Both share `ReportID = 0xE0`. The
  read path is exclusively HID input reports
  (`GetInputReport(buffer, 65)` with byte 0 forced to `0xE0`).
- **Per-SKU sub-tag drift.** The same logical operation (mb-sync,
  quantity-set, ping, frame) uses different sub-tag bytes across
  SKUs. ALV2Fan and SLV2Fan both use `0x6X` for sync/quantity/merge,
  ALFan uses `0x4X`, SLFan uses `0x3X`. This implies the firmware
  command space migrated between MCU revisions and the host
  contains per-SKU shims. A Linux daemon will need a per-SKU
  command table — there is no single shared protocol.
- **Color byte order is R, B, G on the wire.** Confirmed identical
  in every `*FanDevice.cs::SetColorSetting` (line ~140-145 in each).
  Don't byte-swap this — emit `R, B, G` triples directly.
- **`SetMergeOrder` is a SKU-conditional no-op.** SLFan and ALFan
  declare the method but leave it empty; only ALV2Fan, SLV2Fan, and
  SLInfinity actually emit bytes. A linter would catch this; a
  daemon must branch by SKU.
- **WriteDelayTime = 20 ms after every report.** Each device class
  hard-codes a `Thread.Sleep(this.writeDelayTime)` inside
  `sendFeatureReport` and `sendOutputReport`
  (`SLFanDevice.cs:115-127`). Default 20 ms (configurable via
  `WriteDelayTime`). Without this delay the firmware appears to
  drop or coalesce successive commands — flagged as a wire-level
  rate-limit assumption.
- **No read-after-write on writes.** Unlike TLFan which reads-and-
  discards a response after every write, Ene6K77Fan writes are
  fire-and-forget. Only the explicit `Get*` methods read input
  reports.

### Notable divergences from LCDV2

- **Different framing class entirely.** LCDV2 uses A-class (64-byte
  ReportID-1 output reports with a 6-byte header). Ene6K77Fan uses
  HID feature reports + output reports keyed by ReportID `0xE0`.
  No fixed packet length, no `PacketNumber`, no `Length` field —
  the length is the byte-array length passed to `SendFeatureReport`.
- **No B-class or C-class framing.** No multi-packet writes; every
  command fits in one feature report (max observed: 8 bytes for
  `SetMergeOrder` / `StartMerge`; color streams scale to
  `2 + colors.Length * 3` bytes but are a single write).
- **No fixed `Command` byte at a known offset.** The "command" is
  encoded across bytes 1 and 2 with per-SKU sub-tag drift. A Linux
  decoder cannot use the same `match command_byte { ... }` shape
  as the LCDV2 protocol crate.
- **VID is `0x0CF2` (ENE Technology), not `0x0416` (Winbond).** This
  family is on a completely different MCU vendor than LCDV2/HydroShift/
  Galahad/TLFan. Different USB descriptor strings, different driver
  expectations.
- **No pump, no LCD, no temperature.** Subsystem fingerprint is
  fan-only (PWM + RPM) plus lighting (preset modes + per-pixel RGB
  via output report).
- **Wire color order is R, B, G** (not R, G, B as LCDV2/TLFan use).
  Carry-over from the ENE chip's native color register layout.
- **`ReportID = 0xE0` (224), not `0x01` or `0x02`.** Different HID
  multiplexing namespace.

### Notable comparison to TLFan (cross-cutting)

TLFan and Ene6K77Fan are commonly suspected of sharing an MCU
generation. **They do not share command bytes.** TLFan uses cmd
bytes in the `0xA1..0xB4` range (161..180) atop a 64-byte ReportID-1
A-class packet (`L-Connect.Core/LConnectCore/Products/TLFan/LEDCommands.cs:7-25`).
Ene6K77Fan uses tag bytes in the `0x10..0x60` range atop ReportID
`0xE0` HID feature/output reports. Different ReportIDs, different
framing, different command-byte ranges, different VIDs (`0x0416` vs
`0x0CF2`). Despite both being addressable-RGB fan controllers from
the same vendor, **the protocols are unrelated at the wire level**
and the MCU lineage almost certainly differs (probably ENE 6K77 for
this family, vs the Winbond/Lian-Li-internal MCU for TLFan). A Linux
daemon cannot reuse codecs across these two families.

### Open questions / unclear from static analysis

- Whether the `0x55 0x05` `GetFrame` / `0x60` `SetFrame` pair drives
  an animation playback index or something else (LED brightness
  curve frame? motor PWM frame? ARGB pre-stored sequence?). The C#
  side names it `Frame` but provides no docstring; live capture
  needed.
- The exact meaning of the 4-byte `groupIndexOrder` argument in
  `SetMergeOrder` (only ALV2/SLV2/SLInfinity emit bytes; SLFan/ALFan
  declare-and-no-op). Probably maps logical merge-group order into
  physical port slots, but the firmware-side semantics are
  undocumented.
- The 5-byte input-report layouts decoded by each `*FirmwareVersion::
  Load(byte[])` differ per SKU — full byte-by-byte breakdown not
  inspected here. Live capture per SKU is needed to validate.
- Whether tags `0x70`, `0x80`, `0x90`, `0xA0`, etc. are defined in
  the firmware but unused by the host (i.e., is there a hidden
  command space we haven't surfaced?). Static analysis only sees
  what the host emits.
- Whether ALFan and SLFan really have no `SetMergeOrder`
  implementation, or whether the merge-order configuration on
  those SKUs is held in a different command (e.g., `SetQuantity`
  with a different sub-tag). The C# UI's merge feature might just
  be disabled for those SKUs at the UI layer.
- The 20 ms `WriteDelayTime` — is this an empirically-derived rate
  limit, or does the firmware actually require it? A Linux daemon
  could plausibly drop it but the C# value is consistent across
  SKUs.
- The `RequestType.Ping` enum value (`RequestType.cs:13`) maps to
  the `Ping` method, but the C# UI's actual ping cadence (visual
  identification flash) is not visible from this folder; service
  controller has the timing.
- Whether the `0xF5` sentinel ALFan/ALV2Fan check on `GetFrame`
  responses (`ALFanDevice.cs:90`) is a "frame ready" flag or a
  "data valid" flag. SLFan does not check it — possibly an MCU
  revision difference.
