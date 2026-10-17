# TLLCD - Cross-Device Survey

## Family

TLLCD - the per-fan LCD screen module mounted on Lian-Li TL-LCD fans
(generation 1). The TLLCD hub controls up to 4 ports x 3 LCD-fans
(`Constants.cs:17-19`); each "device" surfaced by `TLLCDDevice` is a
single fan-mounted screen, addressed by `(PortNo, LCDIndex)`.

This family does **not** include fan PWM, fan RPM, or fan-ring lighting -
those are handled by the sibling `TLFan` family. The TLLCD enum scope is
LCD-only (display control + JPEG/AVI frames + serial/firmware admin).

**Status legend:** verified-live | inferred-static | unclear. All claims
in this file are **inferred-static** unless otherwise marked - Phase 0e
is a static-source survey and no live TLLCD hardware run has been
performed.

## Source folder

`L-Connect.Core/LConnectCore/Products/TLLCD/`

Files surveyed:

- `LCDCommands.cs` (wire-byte enum)
- `TLLCDDevice.cs` (device class)
- `ITLLCDDevice.cs` (public surface)
- `LCDPacket.cs` (single framing class - 512/64)
- `Constants.cs`
- `HandshakeInfo.cs`, `SerialNumberInfo.cs`, `FirmwareVersion.cs`
- `LCDControlSetting.cs`, `LCDControlMode.cs`, `ScreenRotation.cs`
- `SettingType.cs`
- `TLLCDConfig.cs`, `TLLCDSetting.cs`, `TLLCDScreenSetting.cs`,
  `TLLCDSensorSetting.cs`, `TLLCDSensorType.cs`, `TLLCDMediaType.cs`,
  `TLLCDGroupType.cs`, `TLLCDFrame.cs`, `TLLCDFanRPM.cs`,
  `TLLCDState.cs`, `TLLCDDefaultSet.cs`, `TLLCDScreenRotate.cs`,
  `LCDGroupingInfo.cs`, `ProductInfo.cs`,
  `GetHandshakeInfoEventHandler.cs`

## Marketed name

TL-LCD fan (the screen module on the Lian-Li TL-LCD fan series).

## ControllerTypeID / ControllerClass

- `ControllerTypeID.TLLCD = 16912384` - `L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:24`
- `ControllerClass.TLLCD = 33` - `L-Connect.Core/LConnectCore/Devices/ControllerClass.cs:18`
- `ControllerTypeIDDefinitions.ClassTLLCD = 135168` - `L-Connect.Core/LConnectCore/Devices/ControllerTypeIDDefinitions.cs:49`

## VID:PID

| ControllerTypeID | VID (decimal) | PID (decimal) | VID:PID (hex)   |
|------------------|---------------|---------------|-----------------|
| `TLLCD`          | 1276          | 29587         | `0x04FC:0x7393` |

Source: `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs:51`.

VID `0x04FC` is **Sunplus** - distinct from the Galahad II / HydroShift
LCD families which all use Winbond/Nuvoton (`0x0416`). This MCU-vendor
divergence is consistent with the TLLCD using a different framing class
(see below) than the LCDV2/HS-I baseline.

## Framing class

TLLCD uses a **single, family-specific framing class** (not the standard
A/B/C trio used by Galahad II / HydroShift). Source: `LCDPacket.cs:10-153`.

### Class T (TLLCD) - 512 out / 64 in

```
offset  size  field          description
------  ----  -----          ---------------------------------------------
  0      1   ReportID        always 0x02
  1      1   Command         wire byte
  2      4   DataSize        big-endian u32; total payload across ALL chunks
  6      3   PacketNumber    big-endian u24; 0-based
  9      2   Length          big-endian u16; payload bytes IN THIS chunk
 11    501   Data            up to 501 bytes of payload, zero-padded
```

Constants from `LCDPacket.cs:12-20`:

- `ReportID = 2`
- `PacketLength_Output = 512`
- `PacketLength_Input = 64`
- `HeaderLength = 11`
- `PayloadLength_Output = 501`

Multi-packet chunker: `LCDPacket.BuildPacketsForWrite`
(`LCDPacket.cs:109-139`) replicates `Command` and `DataSize` on every
chunk, increments `PacketNumber` from 0, zero-pads the trailing region
of the final chunk via `Array.Clear` (`LCDPacket.cs:128-131`). Pattern
mirrors LCDV2 B/C class chunkers.

### Framing comparison vs LCDV2 baseline

| Property      | LCDV2 A | LCDV2 B | LCDV2 C | TLLCD T (this family)           |
|---------------|---------|---------|---------|---------------------------------|
| ReportID      | 0x01    | 0x02    | 0x03    | 0x02                            |
| Output length | 64      | 1024    | 512     | **512**                         |
| Input length  | 64      | 512     | 512     | **64** (asymmetric)             |
| Header length | 6       | 11      | 11      | 11                              |
| Payload max   | 58      | 1013    | 501     | 501                             |
| DataSize      | -       | BE u32  | BE u32  | BE u32 (`LCDPacket.cs:60-73`)   |
| PacketNumber  | BE u16  | BE u24  | BE u24  | BE u24 (`LCDPacket.cs:89-101`)  |

TLLCD T-class is **asymmetric in a way no LCDV2 class is**: 512-byte
writes but only **64-byte** reads. This forces all read paths to use a
small response buffer regardless of how the request is sized. The 64-byte
read length implies the firmware never returns large bulk replies - all
queries (handshake, firmware, serial, product info) fit in a single
64-byte response packet.

There is **no separate A-class** for short commands on this family - even
single-byte query commands (e.g., cmd 60 GetHandshakeInfo) build a full
512-byte output packet via `LCDPacket.BuildPacketsForWrite` with
`Length=0` and read back a 64-byte packet.

## Subsystem fingerprint

| Subsystem                    | Present? | Evidence                                               |
|------------------------------|----------|--------------------------------------------------------|
| Fan PWM                      | NO       | No PWM cmd in `LCDCommands.cs`; lives on `TLFan`       |
| Fan RPM read                 | NO       | No RPM cmd in `LCDCommands.cs`; lives on `TLFan`       |
| Fan-ring lighting            | NO       | No lighting cmds in enum; lives on `TLFan`             |
| **Per-fan LCD screen**       | yes      | Whole enum is LCD-centric; `Constants.cs:11-13` 400x400 |
| LCD on/off / brightness / rotation / mode | yes | cmd 64 `LCDControl` (`TLLCDDevice.cs:205-213`)         |
| LCD frame upload (transient JPEG) | yes | cmd 65 `WriteJPG` (`TLLCDDevice.cs:267-270`)             |
| LCD frame upload (synchronised JPEG) | yes | cmd 70 `WriteSYNCJPG` (`TLLCDDevice.cs:272-275`)       |
| LCD video upload (AVI)       | yes      | cmd 69 `WriteAVI` (`TLLCDDevice.cs:247-250`)           |
| LCD boot-image upload        | yes      | cmd 71 `WriteBootAVI`, cmd 72 `WriteBootJPG` (`TLLCDDevice.cs:252-260`) |
| Sensor / handshake           | yes      | cmd 60 `GetHandshakeInfo` (3-byte resp; `HandshakeInfo.cs:29-44`) |
| Per-fan addressing (port + index) | yes | cmd 62 `ReadSerialNumber` returns `Port` (0..3) and `Index` (0..2) (`SerialNumberInfo.cs:36-53`) |
| Set serial number            | yes      | cmd 63 `WriteSerialNumber` (`TLLCDDevice.cs:282-290`)  |
| Firmware version read        | yes      | cmd 61 `GetProductInfo` returns 2-packet version + datetime (`TLLCDDevice.cs:100-119`) |
| Firmware activation / write  | enum-only | cmd 58 `ActiveFirmware`, cmd 59 `WriteFirmware` - no call site in `TLLCDDevice.cs` |
| Reset / soft-reset           | NO       | No reset cmd in `LCDCommands.cs`                       |
| Continuous re-issue timer    | unclear  | No evidence in device class; would live in upper-layer controller |

## Commands

All command bytes from `L-Connect.Core/LConnectCore/Products/TLLCD/LCDCommands.cs:5-19`.

| Cmd | Hex   | Name              | Framing | Direction   | Source                                      | Status        | Notes |
|----:|------:|-------------------|:-------:|:-----------:|---------------------------------------------|:-------------:|-------|
| 58  | `0x3A` | `ActiveFirmware`  | T       | host->dev   | enum-only at `LCDCommands.cs:7`; **no call site** in `TLLCDDevice.cs` | inferred-static | Phase 1 firmware path; assumed B-class-equivalent on T framing. Risky. |
| 59  | `0x3B` | `WriteFirmware`   | T       | host->dev   | enum-only at `LCDCommands.cs:8`; **no call site** in `TLLCDDevice.cs` | inferred-static | Raw firmware bytes; chunked. **Destructive - Phase 1 only.** |
| 60  | `0x3C` | `GetHandshakeInfo`| T       | host<->dev  | `TLLCDDevice.cs:121-134, 262-265`; `HandshakeInfo.cs:29-44` | inferred-static | Bare request (`Length=0`). Response: 3 bytes - `Mode` (u8), `FrameIndex` (BE u16). 64-byte read. |
| 61  | `0x3D` | `GetProductInfo`  | T       | host<->dev  | `TLLCDDevice.cs:100-119`; `FirmwareVersion.cs:71-135` | inferred-static | Bare request. **Two-packet response** demuxed by sequential reads: pkt 0 = 7 comma-separated version fields (`ChipCode`, `BootCode`, `Type`, `ModelA`, `ModelB`, `SoftwareVersion`, `UserVersion`); pkt 1 = build datetime string. ASCII payloads. |
| 62  | `0x3E` | `ReadSerialNumber`| T       | host<->dev  | `TLLCDDevice.cs:146-174`; `SerialNumberInfo.cs:36-53` | inferred-static | **Per-fan addressing.** Bare request. Response: 34 bytes - 32-byte ASCII serial + `Port` (u8, 0..3) + `Index` (u8, 0..2). Read called with explicit 64-byte length (`TLLCDDevice.cs:156`). |
| 63  | `0x3F` | `WriteSerialNumber`| T      | host->dev   | `TLLCDDevice.cs:282-290`                    | inferred-static | 32-byte payload (zero-padded ASCII GUID). **Destructive on identity bytes.** Driver re-runs cmd 62 immediately after to re-read assigned port/index. |
| 64  | `0x40` | `LCDControl`      | T       | host<->dev (`read=true`) | `TLLCDDevice.cs:205-213`; `LCDControlSetting.cs:69-80` | inferred-static | **11-byte payload** (T-class `Length=11`): `[mode, jpg_index_hi, jpg_index_lo, 0, brightness, video_fps, rotation, enable_test, R, G, B]`. See LCDControlMode below. |
| 65  | `0x41` | `WriteJPG`        | T       | host<->dev (`read=true`) | `TLLCDDevice.cs:267-270`                    | inferred-static | Raw JPEG bytes. Multi-chunk via `BuildPacketsForWrite`. Echo-check on each chunk's response (`TLLCDDevice.cs:316`). Likely transient (analogue of LCDV2 cmd 14 `SendJpeg`). |
| 69  | `0x45` | `WriteAVI`        | T       | host<->dev (`read=true`) | `TLLCDDevice.cs:247-250`                    | inferred-static | Raw AVI bytes; chunked. Method named `SendAVI`. Likely transient streaming path. |
| 70  | `0x46` | `WriteSYNCJPG`    | T       | host->dev (`read=false`) | `TLLCDDevice.cs:272-275`                    | inferred-static | Raw JPEG bytes; chunked. Method named `SendSyncJPG`. **Note: `read=false`** - no response expected per chunk; suggests time-critical sync-broadcast where ACK round-trips would jitter playback. The "SYNC" qualifier likely refers to multi-fan-screen synchronised display. |
| 71  | `0x47` | `WriteBootAVI`    | T       | host<->dev (`read=true`) | `TLLCDDevice.cs:257-260`                    | inferred-static | Method named `SendBootVideo`. Naming is **explicit**: stored boot-time AVI. Strong cross-device evidence for the `Send*` (transient) vs `Upload*`/`WriteBoot*` (stored) hypothesis. |
| 72  | `0x48` | `WriteBootJPG`    | T       | host<->dev (`read=true`) | `TLLCDDevice.cs:252-255`                    | inferred-static | Method named `SendBootJPG`. Stored boot-time JPEG. Same hypothesis sibling as cmd 71. |

### Direction notation

`read=true` means the device class waits for and parses an echo
(`TLLCDDevice.cs:311-322`); `read=false` is fire-and-forget. Echo check
verifies `bytes[1] == lCDPacket.Command`.

## Subsystem patterns

### Per-fan addressing - the interesting bit

A single TLLCD hub enumerates as one HID device, but each TLLCD fan has
its own screen and its own logical identity. Addressing is **resolved at
device-open time** (`TLLCDDevice.cs:215-224`) via cmd 62 `ReadSerialNumber`:

1. `Open()` calls `GetPortAndIndex()`.
2. `GetPortAndIndex()` writes cmd 62 (bare request).
3. The response (read with explicit 64-byte buffer at
   `TLLCDDevice.cs:156`) decodes via `SerialNumberInfo.TryParse`:
   - bytes 0..31: ASCII serial number (32 bytes)
   - byte 32: `Port` (validated to `0 <= Port < 4`)
   - byte 33: `Index` (validated to `0 <= Index < 3`)
4. The `(Port, Index)` pair is stored on the `TLLCDDevice` instance.

Caps from `Constants.cs:17-19`: `PortCount = 4`, `MaxLCDFanCount = 3`.
The hub thus addresses **up to 12 fan screens** (4 ports x 3 fans/port).

The `Port` likely corresponds to a physical hub port on the TLLCD
controller; `Index` is the daisy-chain position on that port. This is a
**device-side-assigned identity** scheme: each fan's MCU stores its own
`Port` + `Index` (otherwise cmd 62 wouldn't return per-fan distinct
values via the same hub). `WriteSerialNumber` (cmd 63) then *re*-runs
cmd 62 (`TLLCDDevice.cs:289`), which suggests the firmware re-derives or
echoes the port/index after the new serial is committed.

`TLLCDGroupType.cs:5-15` enumerates the supported physical layouts:
`Group1, Group11, Group2, Group111, Group21, Group12, Group3` - i.e.
the user-configurable groupings of 1, 1+1, 2, 1+1+1, 2+1, 1+2, or 3
fan screens per group. This is upper-layer config, not a wire field.

### Handshake (cmd 60)

Source: `HandshakeInfo.cs:29-44`. Response payload is **3 bytes**:

| offset | size | field         |
|-------:|-----:|---------------|
| 0      | 1    | `Mode` (u8)   |
| 1      | 2    | `FrameIndex` (BE u16) |

This is **much smaller** than the LCDV2 7-byte handshake (which packs
fan/pump RPM + temperature). Consistent with TLLCD having no PWM/RPM
subsystem - the per-screen handshake only reports playback state
(`Mode` likely echoes `LCDControlMode`; `FrameIndex` likely tracks
current AVI frame position for sync purposes).

### LCD control (cmd 64)

`LCDControlSetting.ToBytes()` (`LCDControlSetting.cs:69-80`) produces
**11 bytes**:

| offset | size | field        | notes                                          |
|-------:|-----:|--------------|------------------------------------------------|
| 0      | 1    | `Mode`       | `LCDControlMode`                               |
| 1      | 2    | `JPGIndex`   | BE u16 (selects stored JPEG slot - see below)  |
| 3      | 1    | reserved (0) |                                                |
| 4      | 1    | `Brightness` | 0..=100 (`Constants.cs:7-9`)                   |
| 5      | 1    | `VideoFPS`   | default 30 (`Constants.cs:15`); max 30         |
| 6      | 1    | `Rotation`   | enum 0/1/2/3 = 0/90/180/270 deg                |
| 7      | 1    | `EnableTest` | bool                                           |
| 8      | 3    | `TestColor`  | R, G, B                                        |

Defaults: brightness 50, FPS 30, rotation 0, test disabled, test color
black, mode `Reserver` (0), `JPGIndex` 0
(`LCDControlSetting.cs:58-67`).

#### `LCDControlMode` - **divergent from LCDV2**

`LCDControlMode.cs:5-13`:

| Wire | Name         |
|-----:|--------------|
| 0    | `Reserver`   |
| 1    | `ShowJPG`    |
| 3    | `ShowAVI`    |
| 4    | `ShowAppSync`|
| 5    | `LCDSetting` |
| 6    | `LCDTest`    |

Note: byte 2 is **skipped** (no LCDV2-style `LocalH264`). TLLCD never
exposes an H.264 path. The `ShowJPG` mode (1) combined with the
`JPGIndex` field in `LCDControlSetting` is **strongly suggestive**: the
device has stored-JPEG slots, and the `LCDControl` packet selects which
slot to display. This is direct evidence for a stored-image subsystem
on TLLCD. Compare with the LCDV2 cmd-15 hypothesis (separate stored vs
transient cmds) - on TLLCD the stored path appears to be cmd 72
`WriteBootJPG` (writes the slot) plus cmd 64 `LCDControl` with
`Mode=ShowJPG` and `JPGIndex=N` (selects + activates).

### Send vs Write naming

The `LCDCommands` enum uses the prefix `Write*` for all 6 LCD-data
commands (cmd 65 `WriteJPG`, cmd 69 `WriteAVI`, cmd 70 `WriteSYNCJPG`,
cmd 71 `WriteBootAVI`, cmd 72 `WriteBootJPG`, plus cmd 59 `WriteFirmware`),
but the device-class methods are **all named `Send*`** (`SendJPG`,
`SendAVI`, `SendSyncJPG`, `SendBootJPG`, `SendBootVideo`). This is
purely a naming inconsistency within the C# layer - the wire byte is
what matters.

### Firmware read (cmd 61)

`TLLCDDevice.cs:100-119` issues cmd 61 once, then reads **two response
packets** sequentially (`numArray`, `numArray1`). Both are echoed from
cmd 61. Pkt 0 = ASCII version-string with 7 comma-separated fields
(`FirmwareVersion.cs:115-132`); pkt 1 = ASCII datetime string. Pattern
mirrors LCDV2 cmd `0x86` (`GetFirmwareVersion`) - same two-packet
ASCII shape, different command byte and framing class.

### Sensor display (LCD-side)

`TLLCDSensorType.cs:5-13` lists 5 sensor types the LCD can render:
`CPUTemp, GPUTemp, CPULoad, GPULoad, FanRPM`. These are **rendered
host-side and pushed as JPEG/AVI frames** - there is no wire command
for "display sensor X." `TLLCDSensorSetting.cs:5-53` carries style /
text colour / background fields used to compose the sensor frame
client-side before a `WriteJPG` (cmd 65).

Note that `FanRPM` here is a *display* type, not a wire-protocol field
- the actual RPM source is the sibling `TLFan` device, with the value
  composited into the rendered image.

### `TLLCDMediaType.cs:5-14` (upper-layer)

Five upper-layer media kinds: `None, Image, Dynamic, Video, Sensor,
Animation`. These are settings categories, not wire bytes. They map
roughly to: `Image`->cmd 65 `WriteJPG`, `Video`->cmd 69 `WriteAVI`,
`Sensor`/`Animation`/`Dynamic`->cmd 65 `WriteJPG` rendered host-side.

### Multi-packet I/O

`writeCommand(IEnumerable<Packet>, bool read)` at
`TLLCDDevice.cs:292-341`:

- Iterates chunks from `LCDPacket.BuildPacketsForWrite`.
- After each chunk write: if `read=true`, drains a 64-byte response and
  verifies `bytes[1] == lCDPacket.Command` echo.
- On any HID write/read failure, logs the (PortNo, LCDIndex, Cmd) and
  returns -1.
- `read=false` fast-path skips the response read entirely - used by
  cmd 70 `WriteSYNCJPG` and the bare cmd 60 `SendGetHandshakeInfo`.

## Notable divergences vs LCDV2 baseline

1. **Different MCU vendor** - VID `0x04FC` (Sunplus) vs LCDV2's
   `0x0416` (Winbond/Nuvoton). Suggests an entirely separate firmware
   codebase, which is consistent with the disjoint command numbering
   (TLLCD: 58..72; LCDV2: 10..23, 129..142).

2. **Single framing class, not three.** TLLCD uses one packet shape
   (`LCDPacket`, ReportID=0x02) for everything. There is no A/B/C
   trio. The framing layout is closest to LCDV2 C-class (501-byte
   payloads, 11-byte header, BE u32 DataSize, BE u24 PacketNumber)
   but with the **read length asymmetric** at 64 bytes (LCDV2 C-class
   reads back 512). See framing comparison table above.

3. **Per-fan addressing scheme.** No equivalent on LCDV2 (which is
   one-LCD-per-pump-cap). TLLCD requires `(Port, Index)` tuple
   addressing via cmd 62, with the device-side serial number storing
   the assignment. The hub fans out one HID interface to up to 12
   physical fan screens.

4. **No fan/pump/lighting subsystem in this enum.** PWM, RPM, and
   fan-ring lighting are split off into the sibling `TLFan` family.
   LCDV2 keeps everything on a single device class.

5. **No reset command.** LCDV2 has cmd 142 `ResetDevice`; TLLCD's
   enum has nothing analogous. Hardware reset is power-cycle only.

6. **No H.264 path.** LCDV2 has cmd 13 (B/C dual-framing) for H.264
   streaming. TLLCD's only video path is AVI (cmd 69). `LCDControlMode`
   wire byte 2 is unused, possibly reserved for an H.264 mode that
   never shipped on this firmware.

7. **Two-packet firmware response uses cmd 61 (`GetProductInfo`)**, not
   LCDV2's cmd 134 (`GetFirmwareVersion`). Wire shape (ASCII
   comma-separated + datetime) is similar.

8. **Handshake is 3 bytes, not 7.** Carries no temperature or RPM -
   only LCD playback state (`Mode` + `FrameIndex`). Reflects the
   LCD-only subsystem scope.

9. **Stored-image subsystem is explicit.** TLLCD ships with cmd 71
   `WriteBootAVI` and cmd 72 `WriteBootJPG`, both with explicit
   "Boot" semantics, plus a `JPGIndex` field in `LCDControlSetting`
   suggesting indexed slot selection. This is much more concrete than
   the LCDV2 cmd-15 puzzle.

### Cmd-15 hypothesis cross-check (Send* vs Upload* / WriteBoot*)

The Phase 0d open question on LCDV2 cmd 15 (`UploadJpeg` ACK'd but no
visible activation) is whether the `Upload*` family is for stored /
boot / non-volatile content vs `Send*` for transient frames.

**TLLCD evidence supports the hypothesis:**

- `Send*`/`Write*` transient pair: cmd 65 `WriteJPG` (one-shot frame)
  and cmd 69 `WriteAVI` (streaming video).
- `WriteBoot*` stored pair: cmd 71 `WriteBootAVI` and cmd 72 `WriteBootJPG`.
  The naming is **explicit** ("Boot"), and both have full call sites in
  `TLLCDDevice.cs:252-260` (named `SendBootJPG` and `SendBootVideo`),
  unlike LCDV2 cmd 15 which has only an enum entry.
- The pairing is **symmetric per media type**: JPEG transient (65) +
  JPEG boot (72); AVI transient (69) + AVI boot (71). This is the same
  symmetry HS I displays for `UploadStartupAviMovie = 22` (boot AVI
  sibling) vs `UploadJpeg = 15` and `UploadAvi = 18` (no `Startup`
  qualifier on those enum names but wire-position symmetric).

Importantly, TLLCD also exposes a **stored-slot selection mechanism**
not present on HS I or LCDV2's catalogued commands: the `JPGIndex` field
in `LCDControlSetting` (`LCDControlSetting.cs:23-28`) is a 16-bit slot
index sent with cmd 64 `LCDControl` when `Mode=ShowJPG`. This suggests
the activation flow for a stored image is:

1. Upload/store: cmd 72 `WriteBootJPG` (or cmd 65 with implicit slot?)
2. Activate / select: cmd 64 with `Mode=ShowJPG` and `JPGIndex=N`

If LCDV2 cmd 15 follows the same pattern, the activation may require
a `LCDControl` write with a mode/index combo that the LCDV2 source
doesn't currently generate. **Recommended LCDV2 follow-up:** test cmd
15 followed by `LCDControl{mode=Application, ...}` variants and see if
any combination of LCDV2's `LCDControlMode` triggers display of the
stored frame. (Note: LCDV2 has no `JPGIndex` field on its
`LCDControlSetting` per the LCDV2 doc, which weakens the analogy -
LCDV2 may simply not have a stored-slot subsystem despite cmd 15's
name suggesting otherwise.)

### Relationship to TLV2LCD (LWireless family)

`L-Connect.Core/LConnectCore/Products/LWireless/TLV2LCD/` is a
**separate, related family** (the v2 generation, accessed over
LWireless's named-pipe bridge rather than direct HID). `ControllerClass`
defines them as siblings: `TLLCD = 33` and `TLV2LCD = 50`
(`L-Connect.Core/LConnectCore/Devices/ControllerClass.cs:18,21`).
`TLV2LCDFirmwareVersion` regex
(`LConnectCore/Products/LWireless/TLV2LCD/TLV2LCDFirmwareVersion.cs:46`)
matches `tlv2_<8 digits>(<2 digits>)(?:_<2 digits>)?` - distinct from
TLLCD's comma-separated version format. TLV2LCD is **not documented in
this file** - it belongs to the LWireless task in this Phase 0e survey.

## Open questions / unclear

1. **Stored-slot capacity and addressing.** `JPGIndex` is a 16-bit
   field but `Constants.cs` doesn't advertise a slot count. Whether
   the device exposes 1, N, or 65536 stored slots is unclear from
   static analysis. cmd 65 `WriteJPG` has no slot-selector field in
   its call site (just raw bytes) - so either every `WriteJPG` writes
   to a single transient buffer, or there's an implicit slot tied to
   call order, or the slot index is encoded in the JPEG payload
   prefix in a way not visible at the C# level.

2. **`Mode` byte in handshake response.** `HandshakeInfo.Mode`
   (`HandshakeInfo.cs:22-27`) is a u8, but its value space is not
   defined in source. Is it a mirror of the last `LCDControlMode`?
   A separate firmware state? Inferred-static cannot resolve.

3. **`FrameIndex` semantics.** The field is BE u16 in the handshake
   response. Likely an AVI playback position counter, but no source
   comment confirms this. If so, it would be useful for daemon-side
   sync of the SYNC-JPG broadcast.

4. **Cmd 70 `WriteSYNCJPG` actual sync semantics.** The `read=false`
   call signature (`TLLCDDevice.cs:274`) is the only static evidence.
   It's plausibly a hub-side broadcast that writes the same JPEG to
   *all* connected fan screens at once (vs cmd 65 which targets one
   addressed fan), but no proof from the device class alone -
   TLLCDDevice is per-fan, so any "broadcast" routing must happen at
   the upper-layer controller (not in `L-Connect.Core/Products/TLLCD/`).

5. **Cmd 58 `ActiveFirmware` and cmd 59 `WriteFirmware`** have no
   call sites in `TLLCDDevice.cs`. By analogy with LCDV2's
   `ActiveFirmware = 10` and HS I's identical pattern, these are
   plausibly `Length=1, Data[0]=0x01` activation + chunked firmware
   blob respectively, but unconfirmed.

6. **Per-fan vs per-hub command targeting.** Within the device class,
   cmd 62 `ReadSerialNumber` is the only command that returns the
   fan's `(Port, Index)`. All other commands are sent on the same
   HID handle - so how the firmware routes (e.g.) a `WriteJPG` to the
   correct fan screen is unclear. Plausible options:
   - The hub auto-routes based on the daisy-chain position the
     command arrives at (only one fan can be "active receiver" at a
     time via some addressing prefix in the HID descriptor we don't
     see at the C# level).
   - Each fan has its own HID interface number, and the Linux port's
     `hidapi` enumeration would surface them as distinct `hidraw`
     nodes (each `TLLCDDevice` is a wrapper around a separate
     `IHidDevice`).

   The second option is more consistent with `TLLCDDevice` having a
   per-instance `HidDevice` field (`TLLCDDevice.cs:18-28`) - i.e.,
   one HID interface per fan, with the hub presenting up to 12 HID
   interfaces. Confirmation would require live `lsusb -v` against a
   TLLCD hub.

7. **Default `VideoFPS = 30`** (`LCDControlSetting.cs:66`,
   `Constants.cs:15`). The actual sustainable rate over the 512/64
   asymmetric framing for 400x400 frames is unmeasured.

8. **No `LCDControlMode = 2`.** The skipped wire value is suspicious -
   either reserved for a future H.264 mode or a deprecated path.
   Live behaviour of writing 2 is unverified.

9. **Continuous re-issue / refresh requirement.** Unlike LCDV2 (where
   cmd 14 `SendJpeg` requires a <=15 s refresh loop), no static
   evidence indicates whether TLLCD cmd 65 `WriteJPG` frames are held
   indefinitely or auto-revert. Live verification needed.
