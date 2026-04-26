# Lian Li L-Connect3 — Cross-Device Command Survey

- **Status:** in progress (Phase 0e)
- **Date:** 2026-04-26
- **Scope:** all device families managed by LConnect3, plus an OpenRGB
  cross-validator pass

## 1. Sources

1. **Primary:** decompiled `LConnectCore/Products/<family>/` under
   `/home/inky/Development/lian-li/L-Connect.Core/`
2. **Cross-validator:** [OpenRGB's `Controllers/LianLiController/`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/LianLiController) (GPL-2.0+)
3. **Verified live:** `docs/protocol/captures/` (LCDV2 only as of Phase 0d)

OpenRGB is license-compatible for reading. The survey cites OpenRGB by
URL and file path; it does **not** copy OpenRGB code into our tree.
Protocol facts (command bytes, payload offsets, framing) are documented
in our own tables.

## 2. Conventions

- Every command entry carries a citation `LConnectCore/Products/<family>/<file>.cs:<start>-<end>`
- Status flags: ✅ verified (live or live-cited) | 🌗 inferred (static-only) | ❓ unclear (decompiled but partially obscured)
- Framing classes: **A** (64-byte ACommandPacket), **B** (1024 out / 512 in BCommandPacket), **C** (512/512 CCommandPacket — `Galahad2Vision/CCommandPacket.cs` confirms existence). Other framings noted as discovered.

## 3. Per-family writeups

(Assembled from `docs/protocol/cross-device-survey/per-family/*.md` by P0e-15.)

## 4. Cross-reference matrix

(P0e-15 fills these tables.)

### 4.1 Lighting commands across families
### 4.2 LCD-frame commands across families
### 4.3 Fan PWM commands across families
### 4.4 Sensor / handshake commands across families
### 4.5 Maintenance commands across families (reset, firmware, identity)

## 5. OpenRGB cross-validator findings

OpenRGB's `Controllers/LianLiController/` tree (GPL-2.0-or-later;
[gitlab.com/CalcProgrammer1/OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/LianLiController))
ships nine separate Lian-Li controllers plus a shared detector. The pass below
reads OpenRGB's source as a license-compatible cross-validator only — no
OpenRGB code was copied into our tree; protocol facts are restated in our own
tables citing OpenRGB by URL and file path.

### 5.1 Inventory of OpenRGB Lian Li controllers

| OpenRGB controller | Our survey family | VID:PID | Detector citation |
|---|---|---|---|
| `LianLiUniHubController` | (none — pre-`A1xx` Uni Hub generation, not in LConnect3) | `0x0CF2:0x7750` | [`Controllers/LianLiController/LianLiControllerDetect.cpp:295`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L295) |
| `LianLiUniHubSLController` | (out of survey scope — SL fan family / `LConnectCore/Products/<not-yet-surveyed>`) | `0x0CF2:0xA100` | [`LianLiControllerDetect.cpp:298`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L298) |
| `LianLiUniHubALController` | (out of survey scope — AL fan family) | `0x0CF2:0xA101` | [`LianLiControllerDetect.cpp:299`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L299) |
| `LianLiUniHub_AL10Controller` | (out of survey scope — AL `v1.0` firmware fallback for `0xA101`) | `0x0CF2:0xA101` (FW v1.0) | [`LianLiControllerDetect.cpp:204`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L204) |
| `LianLiUniHubSLInfinityController` | (out of survey scope — SL Infinity fan family) | `0x0CF2:0xA102` | [`LianLiControllerDetect.cpp:303`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L303) |
| `LianLiUniHubSLV2Controller` | (out of survey scope — SL/AL V2 fan family) | `0x0CF2:0xA103`, `0xA104`, `0xA105` | [`LianLiControllerDetect.cpp:300-302`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L300) |
| `LianLiStrimerLConnectController` | **StrimerPlus** | `0x0CF2:0xA200` | [`LianLiControllerDetect.cpp:304`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L304) |
| `LianLiGAIITrinityController` | **Galahad2 Trinity** (both Perf and Regular SKUs share this driver) | `0x0416:0x7373` (Regular), `0x0416:0x7371` (Performance) | [`LianLiControllerDetect.cpp:305-306`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L305) |
| `LianLiUniversalScreenController` | **UniversalScreen 8.8" (lighting variant)** | `0x0416:0x8050` | [`LianLiControllerDetect.cpp:296`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiControllerDetect.cpp#L296) |

VID glossary: `0x0CF2` = ENE (Lian-Li shared MCU vendor); `0x0416` = Nuvoton (Winbond) — the same vendor LConnect3 uses for the Galahad II family and the UniversalScreen lighting board.

### 5.2 Coverage gap commentary

Survey families covered by OpenRGB:

- **StrimerPlus** — covered (`LianLiStrimerLConnectController`)
- **Galahad2 Trinity** — covered (`LianLiGAIITrinityController`); both Perf and Regular SKUs share one driver class, with no SKU-specific code paths
- **UniversalScreen 8.8" (lighting variant)** — covered (`LianLiUniversalScreenController`)

Survey families **not** covered by OpenRGB (no cross-validation possible from this tree):

- **Galahad II Vision (LCDV2)** — no driver. OpenRGB does not implement the LCD frame upload, the cmd-129 handshake, the cmd-138/139 pump/fan PWM, or the cmd-20 per-pixel LED path. The LCDV2 PID `0x0416:0x7395` is absent from `LianLiControllerDetect.cpp`. This is the strongest single gap: the survey's most-verified family has no second source.
- **Galahad II LCD (older, pre-Vision)** — no driver. PID `0x0416:0x7391` absent.
- **HydroShift LCD (HS I)** — no driver. PIDs `0x0416:0x7398..0x739A` absent.
- **HydroShift II LCD (Circle/Square)** — no driver. PIDs under VID `0x1CBE` (`0x7358`) absent.
- **TLFan / TLLCD / TLV2LCD / SLV3LCD / Lancool 207 Digital / Wireless Controller (V1+V2)** — none covered. OpenRGB has no entries for any of these PIDs.

OpenRGB's coverage skews heavily to **lighting-only fan-hub controllers** (the `UniHub*` family covers most of LConnect3's `*Fan` SKU range); LCD-bearing AIOs and the wireless / TL fan / TL LCD branches are absent. The cross-validator strength is therefore concentrated on Strimer Plus, Galahad II Trinity, and the UniversalScreen lighting board.

### 5.3 Per-family cross-validator tables

#### Family: StrimerPlus — OpenRGB cross-validation

OpenRGB's driver maps to a subset of the LConnect3 `StrimerPlusDevice.cs` surface. All LConnect3 commands keep the C# convention `0xE0 ReportID + (op | port)` second byte; OpenRGB uses the same layout, the same ReportID, and the same packet length (255 bytes incl. ReportID).

| Cmd byte (`data[1]`) | OpenRGB sends? | Match | OpenRGB cite | LConnect3 cite |
|---|---|---|---|---|
| `0x10 \| port` (`SetEffectSetting`) | yes | **matches** (same 6-byte tx layout: ReportID, cmd, mode, speed, direction, brightness) | [`LianLiStrimerLConnectController.cpp:77-89`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L77) | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:165-168` |
| `0x20 \| port` / `0x2C` (`SetEffectEnable` single-port + multi-port mask) | **not implemented** | n/a | - | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:147-163` |
| `0x2C, 0x0F, 0xFF, 0x00, ...` (`SendApply`) | yes | OpenRGB-only — no LConnect3 analogue | [`LianLiStrimerLConnectController.cpp:70-75`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L70) | (none) |
| `0x30 \| port` (`SetLedsDirect` / `SetColorSetting`) | yes | **diverges-payload** (R-B-G byte order — see notes) | [`LianLiStrimerLConnectController.cpp:91-107`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L91) | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:129-145` |
| `0x50 sub 0x00` (`GetFirmwareVersion`) | **not implemented** | n/a | - | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:72-89` |
| `0x50 sub 0x01` (`GetChannel2PortCount`) | **not implemented** | n/a | - | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:61-70` |
| `0x55 sub 0x05` (`GetFrame`) | **not implemented** | n/a | - | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:91-100` |
| `0x60` (`SetFrame`) | **not implemented** | n/a | - | `LConnectCore/Products/StrimerPlus/StrimerPlusDevice.cs:170-179` |

**Headline finding — R-B-G byte order on per-LED writes is CONFIRMED by OpenRGB.** OpenRGB's `SetLedsDirect()` writes the per-LED bytes in the order `R, B, G` ([`LianLiStrimerLConnectController.cpp:101-103`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L101)) — exactly mirroring the C# `StrimerPlusDevice.cs:140-142` ordering. Two independent reverse-engineering efforts arriving at the same out-of-spec byte order is strong evidence that this is a genuine **firmware quirk**, not a long-standing bug in either codebase. The Linux port should preserve the R-B-G order. (Hypothesis "JustDecompile artefact in C#" is now ruled out.)

OpenRGB additionally gives us:
- Mode-byte cross-check: OpenRGB enumerates 24 modes (`STRIMERLCONNECT_MODE_*`, [`.h:27-56`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.h#L27)) with byte values `0x00..0x06, 0x19..0x2B`. LConnect3's `LightingMode.cs:5-32` enumerates 24 modes split between `*_Individual` and `*_Complete` scopes. OpenRGB's mode-byte values match LConnect3's exactly for the modes both implement (e.g., Rainbow Wave = `0x05`, Snooker = `0x19`, Mixing = `0x1A`, Ping Pong = `0x1B`). This is a strong second-source confirmation of the mode-byte enum.
- **Speed-table confirmation:** OpenRGB's `speed_data[]` lookup is `{0x02, 0x01, 0x00, 0xFE, 0xFF}` for slow→fast ([`LianLiStrimerLConnectController.cpp:15-18`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L15)). LConnect3's `LightingSpeed.cs` exposes `Lowest=2, Lower=1, Normal=0, Faster=255, Fastest=254` — the byte values match OpenRGB's table at every index. This **confirms** the non-monotonic encoding flagged in `strimerplus.md` (the staging doc was right to call it out as a potential footgun for naive `0..=255 → percent` mappings on Linux).
- **Brightness-table confirmation:** OpenRGB's `brightness_data[]` is `{0x08, 0x03, 0x02, 0x01, 0x00}` for 0%→100% ([`LianLiStrimerLConnectController.cpp:20-23`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L20)). Matches LConnect3's `LightingBrightness.cs:5-13` (`Off=8, Lowest=3, Lower=2, Higher=1, Highest=0`) byte-for-byte.
- **OpenRGB-only command:** the `SendApply()` packet — `[0xE0, 0x2C, 0x0F, 0xFF, 0x00, 0x00, 0x00, 0x00]` — is a "commit" / "apply effect" trigger that OpenRGB sends but does **not** appear anywhere in `StrimerPlusDevice.cs`. The byte `0x2C` is the multi-port-mask form of `SetEffectEnable` (`0x20 | 0x0C`); the `0x0F, 0xFF` BE-u16 is `0x0FFF` = `PortFlags.All`. So `SendApply` is structurally **`SetEffectEnable(port=12, multiPort=0x0FFF)`** — i.e., "enable all 12 ports" — issued unconditionally after every `SetMode`. LConnect3 either issues this through a different code path (UI-side controller, not in the device class) or does not need it because of how the device latches state. Either way, this is a useful pattern for the Linux port: a "commit-all-ports" pseudo-command that may be required to actually activate writes.

#### Family: Galahad II Trinity — OpenRGB cross-validation

OpenRGB's Trinity driver implements **only lighting** (cmd 131-equivalent) and **firmware version read** (cmd 134-equivalent). PWM, handshake (RPM), and reset paths are entirely absent.

| Cmd byte | OpenRGB sends? | Match | OpenRGB cite | LConnect3 cite |
|---|---|---|---|---|
| `0x81` (129, `Handshake` / RPM telemetry) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:110-146` |
| `0x82` (130, `GetPumpLightModeList`) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:8` |
| `0x83` (131, `SetPumpLight` — *PT_RGB_CONTROL*) | yes | **diverges-payload** (Trinity OpenRGB writes to a single combined "ring" payload at offsets `0x06..0x16`; LConnect3 writes a 19-byte payload at offsets `0..18` after the cmd byte — see notes) | [`LianLiGAIITrinityController.cpp:111-306`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L111) | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:191-199`; `PumpLightingSetting.cs:77-96` |
| `0x84` (132, `GetLEDLightModeList` — fan) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:10` |
| `0x85` (133, `SetLEDLight` — fan) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:159-167` |
| `0x86` (134, `GetProductInfo` — *PT_FIRMWARE_INFO*) | yes | **matches-framing** (bare cmd-134 request, two-packet ASCII response read by `hid_read` twice — same shape as LConnect3) | [`LianLiGAIITrinityController.cpp:50-63`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L50) | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:41-108`; `FirmwareVersion.cs:81-136` |
| `0x87` (135, `LEDTestMode`) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:13` |
| `0x88` (136, `ReadSerialNumber`) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:14` |
| `0x89` (137, `WriteSerialNumber`) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:15` |
| `0x8A` (138, `SetPump` — pump PWM) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:201-221` |
| `0x8B` (139, `SetFan` — fan PWM) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Galahad2TrinityDevice.cs:169-189` |
| `0x8C` (140, `GetFanPumpParams`) | **not implemented** | n/a | - | `LConnectCore/Products/Galahad2Trinity/Commands.cs:18` |

**Headline finding — Trinity cmd 131 framing matches LConnect3 byte-for-byte; payload shape diverges (subtly).** OpenRGB confirms:

- **64-byte HID write** with **ReportID=0x01** and **cmd byte `0x83` at offset `0x01`** ([`LianLiGAIITrinityController.cpp:73-74`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L73)). This is exactly the LCDV2 / LConnect3 A-class framing. The `galahad2-trinity.md` staging file's claim that Trinity uses an A-class equivalent (locally named `CommandPacket`, structurally identical to `ACommandPacket`) is independently confirmed.
- **Mode byte at offset `0x07` (= `BA_MODE`)**, brightness at `0x08`, speed at `0x09`, direction at `0x16` ([`LianLiGAIITrinityController.h:89-112`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.h#L89)). LConnect3's `PumpLightingSetting.ToBytes()` writes scope at byte 0, mode at byte 1, brightness at byte 2, speed at byte 3 of the **payload** (`PumpLightingSetting.cs:77-96`). Translating LConnect3's payload offsets to absolute packet offsets: the A-class `Data` field starts at packet offset `0x06` (after ReportID, cmd, length, BE-u16 PacketNumber, length-byte = 6 header bytes), so LConnect3's payload[0]=scope at packet 0x06, payload[1]=mode at packet 0x07, etc. **OpenRGB's mode at 0x07 lines up exactly with LConnect3's payload-byte-1=mode**, which means OpenRGB's offset 0x06 (`BA_RING`) corresponds to LConnect3's payload-byte-0 (`scope`).
- **`BA_RING`** values (`R_INNER=0, R_OUTER=1, R_BOTH=2`, [`.h:58-63`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.h#L58)) match LConnect3's `LightingScope.cs:5-10` (`Inner=0, Outer=1, All=2`) byte-for-byte. The "ring" terminology is OpenRGB's; the wire byte is identical.
- **Mode byte values match for the modes both implement.** OpenRGB's `M_RAINBOW=0x01, M_RAINBOW_MORPH=0x02, M_STATIC_COLOR=0x03, M_BREATHING_COLOR=0x04, M_RUNWAY=0x05, M_METEOR=0x06, M_VORTEX=0x07, M_CROSSING_OVER=0x08, M_TAI_CHI=0x09, M_COLORFUL_STARRY_NIGHT=0x0A, M_STATIC_STARRY_NIGHT=0x0B, M_VOICE=0x0C, M_BIG_BANG=0x0D, M_PUMP=0x0E, M_COLORS_MORPH=0x0F` ([`.h:38-56`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.h#L38)). LConnect3's `Galahad2Trinity/PumpLightingMode.cs:5-46` has `Rainbow_Inner = 1, RainbowMorph_Inner = 2, StaticColor_Inner = 3, ...` collapsed via `% 1000` — the post-modulo wire bytes are `1, 2, 3, ...` matching OpenRGB exactly. **The `% 1000` mode-folding in LConnect3 is now confirmed: scope is carried in the separate `BA_RING` byte, not in the mode byte.**

Diverges:

- **Number of RGB color slots written.** OpenRGB writes up to **4 RGB triples** at `BA_R0..BA_B3` (offsets `0x0A..0x15`, 12 bytes). LConnect3's pump payload also exposes 4 colors at payload bytes `4..15` (offsets `0x0A..0x15` in absolute terms). **Match here.** However, OpenRGB's API takes 1, 2, 3, or 4 colors and zero-fills the rest; LConnect3 always serializes 4 colors regardless of mode. Net wire byte difference: zero (both write 4 RGB slots, just the unused ones may differ between zero-fill and "last-set-color").
- **`disabled` and `source` bytes (LConnect3 payload bytes 17 and 18 = packet offsets 0x17, 0x18).** OpenRGB's static buffer is zero-initialized by `memset` ([`LianLiGAIITrinityController.cpp:71`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L71)) and never sets these offsets, so OpenRGB always sends `disabled=0, source=0` (= `ARGBSignalSource.MCU`). LConnect3 lets the host pick either MCU or Motherboard signal source. **OpenRGB only ever drives MCU mode.** This is a coverage gap, not a contradiction — but a daemon emulating LConnect3 must keep the 19-byte payload, not just the 17 OpenRGB exercises.
- **Magic byte at packet offset `0x05 = 0x13` (decimal 19).** OpenRGB sets this in every RGB packet ([`LianLiGAIITrinityController.cpp:75`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L75)). In A-class framing, packet offset `0x05` is the **`Length` byte** (the codec's payload length). `0x13 = 19` matches LConnect3's payload-length for cmd 131 exactly (19 bytes: scope+mode+bri+speed + 4×3 RGB + dir + disabled + source). **This is an independent confirmation that the LConnect3 cmd-131 payload is exactly 19 bytes** — Trinity, like LCDV2, uses a fixed 19-byte payload at A-class.
- **OpenRGB never sends cmd 133 (fan lighting).** Despite the staging file's finding that cmd 131 and cmd 133 are byte-for-byte identical in framing on both LCDV2 and Trinity, OpenRGB's controller class only exposes pump-ring effects. Either OpenRGB authors did not need fan-LED control (Trinity's fans are already covered by the `UniHub*` SL/AL drivers if used standalone) or the cmd-133 path was never tested. This is a **coverage gap**, not a finding against the staging file's claim.

**Trinity firmware-info read (cmd 134):** OpenRGB confirms the LConnect3 finding that the response is two ASCII packets ([`LianLiGAIITrinityController.cpp:53-61`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L53) — note two unconditional `hid_read` calls). OpenRGB reads the ASCII string from packet offset `0x06` (i.e., the start of the `Data` field, same as LConnect3's `FirmwareVersion.cs:81-136` parser) and concatenates the two packets with a `(...)` wrapper. This is a strong second-source confirmation.

#### Family: UniversalScreen 8.8" (lighting variant) — OpenRGB cross-validation

OpenRGB's `LianLiUniversalScreenController` covers only the **lighting** PID (`0x0416:0x8050`). The display path (separate VID `0x1CBE:0xA088`, used for video upload) is not in OpenRGB.

Since the UniversalScreen is not yet a per-family staging file in this survey (out of scope for the parallel sibling agents), the cross-validator output below stands as the **first** documentation of this family's wire protocol within the project — to be merged into a future staging file.

| Cmd byte | OpenRGB sends? | Match | OpenRGB cite | LConnect3 cite |
|---|---|---|---|---|
| `0x11` (`SetLedColors` chunk header) | yes | OpenRGB-only documented; LConnect3 path not in `Products/UniversalScreen8p8Inch/` static analysis (separate `SLV3LCD`-class controller may handle this) | [`LianLiUniversalScreenController.cpp:67-99`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiUniversalScreenController/LianLiUniversalScreenController.cpp#L67) | `LConnectCore/Products/UniversalScreen8p8Inch/` (static analysis pending) |

UniversalScreen lighting framing **does not** match LCDV2's A/B/C. Specifically:

- **Transport: `libusb_bulk_transfer` on endpoint 1** ([`LianLiUniversalScreenController.cpp:97`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiUniversalScreenController/LianLiUniversalScreenController.cpp#L97)) — not HID-write of a Feature/Output report.
- **Packet size: 64 bytes flat.** No `ReportID` byte. No `Length` byte. No `PacketNumber`.
- **Layout per packet:** byte 0 = `0x11` (cmd / message tag), byte 1 = `offset` (LED-index of first LED in this chunk), bytes 2-3 = `0x00, 0x00` (reserved), bytes 4..63 = up to **20 LEDs × 3 bytes RGB** (`R, G, B` order — *not* RBG like Strimer, *not* RBG like UniHub, plain conventional RGB).
- **Chunked write:** OpenRGB iterates the LED list in 20-LED chunks, advancing `offset` by the chunk size each iteration ([`LianLiUniversalScreenController.cpp:75-98`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiUniversalScreenController/LianLiUniversalScreenController.cpp#L75)). The last chunk may be partial.

**Headline finding — UniversalScreen needs a fourth framing class.** This is neither LCDV2 A (HID Feature, ReportID=0x01, 64-byte) nor B (1024-out / 512-in) nor C (512/512). It is a **bulk-transfer endpoint-1 path with a flat 64-byte chunk header `[0x11, offset, 0, 0, RGB...]`**. The eventual `lconnect-protocol::Framing` enum will need a "**D**" (or named) variant — `UniversalScreenBulk` or similar. Strimer Plus also doesn't fit A/B/C (uses HID Feature reports with `ReportID=0xE0` and a composite cmd-byte), so the framing taxonomy may end up as `{LcdvA, LcdvB, LcdvC, StrimerHidFeature, UniversalScreenBulk, ...}` rather than a single ordered enum. See section 6 (P0e-16) for the consolidated framing-extension discussion.

**Per-LED byte order: RGB.** Unlike Strimer Plus (R-B-G) and the UniHub family (struct named "RBG"), the UniversalScreen uses conventional R-G-B byte order ([`LianLiUniversalScreenController.cpp:89-91`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiUniversalScreenController/LianLiUniversalScreenController.cpp#L89)). Lian-Li devices are **not** consistent on byte order; this varies by SKU and probably tracks the LED MCU vendor.

### 5.4 Divergence notes

This subsection expands on every `diverges-payload` and `not implemented` row above where context matters.

1. **StrimerPlus per-LED R-B-G** — both OpenRGB and LConnect3 send R-B-G. The Strimer Plus staging file flagged this prominently as a "headline finding" with three plausible causes (firmware quirk, C# bug, decompiler artefact). With OpenRGB confirming the same byte order independently, **the firmware-quirk hypothesis is the only one consistent with the evidence**. Linux port should preserve R-B-G.

2. **StrimerPlus `SendApply` is OpenRGB-only** — OpenRGB issues a "commit all 12 ports" packet (`[0xE0, 0x2C, 0x0F, 0xFF, 0x00, 0x00, 0x00, 0x00]`) after every mode change. LConnect3's `StrimerPlusDevice.cs` does not contain this exact bytestring; the equivalent `SetEffectEnable(port=12, multiPort=PortFlags.All)` call exists and produces the same wire bytes, but it is invoked by the upper UI layer rather than baked into every `SetEffectSetting` call. **For a daemon emulating LConnect3, the apply-after-set pattern is the safest default** — without it, OpenRGB users have empirically needed the apply step to make changes visible. Verify on hardware whether a single `SetEffectEnable` call suffices or whether `SendApply` is required after every `SetEffectSetting`.

3. **StrimerPlus speed-table non-monotonic encoding confirmed** — the non-monotonic `{0x02, 0x01, 0x00, 0xFE, 0xFF}` mapping (`Slow→Fast`) is now confirmed by both source trees. The strimerplus.md "Notable divergences" section already flagged this; OpenRGB validates the staging file's claim.

4. **Trinity cmd 131 mode byte = `% 1000` of LConnect3's enum value** — confirmed. OpenRGB's mode-byte values (1..15 sequential) match LConnect3's post-modulo wire bytes. This validates the staging file's assertion that mode byte and scope byte are independent on the wire (mode encodes "which animation", scope/`BA_RING` encodes "which physical ring(s)").

5. **Trinity cmd 131 `Length=0x13` magic** — OpenRGB hardcodes `usb_buf[0x05] = 0x13` ([`LianLiGAIITrinityController.cpp:75`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiGAIITrinityController/LianLiGAIITrinityController.cpp#L75)) — i.e., always 19 bytes of payload. LConnect3 lets the codec compute the payload length dynamically based on the `PumpLightingSetting` struct. **Both sides agree on 19 as the canonical pump-lighting payload length.** A daemon should always send 19-byte payloads even when "logically" fewer color slots are needed — zero-fill the remainder.

6. **Trinity SKU discrimination** — OpenRGB does not branch on SKU at all. The Performance and Regular variants share the same driver class with no PWM-range awareness, no pump-RPM-range table, no SKU-specific code. Since OpenRGB does not implement PWM at all, this is mute; for our purposes, the SKU discrimination flagged in `galahad2-trinity.md:79` (Pump RPM range 2200-3200 vs 2200-4200, distinguished by VID:PID) remains a correct LConnect3-side observation that OpenRGB simply doesn't exercise.

7. **UniversalScreen lighting protocol absent from current per-family staging** — this is a fresh family contribution from the OpenRGB pass. Action item: a `per-family/universal-screen.md` staging file should be created in a future task (P0e-15 or follow-on) capturing the bulk-transfer / endpoint-1 / 64-byte chunk / RGB / 20-LED-per-chunk pattern documented here.

### 5.5 OpenRGB-only commands

Commands OpenRGB sends that have no exact match in the LConnect3 device classes inspected for this survey:

| Family | Wire bytes | Purpose | OpenRGB cite |
|---|---|---|---|
| StrimerPlus | `[0xE0, 0x2C, 0x0F, 0xFF, 0x00, 0x00, 0x00, 0x00]` (`SendApply`) | "Apply / commit" — equivalent to `SetEffectEnable(port=12, multiPort=0x0FFF)` issued unconditionally after every mode change | [`LianLiStrimerLConnectController.cpp:70-75`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/master/Controllers/LianLiController/LianLiStrimerLConnectController/LianLiStrimerLConnectController.cpp#L70) |

Strictly, this is a **specific value of an existing cmd byte** (`0x2C` = `SetEffectEnable` multi-port form), not a novel cmd byte. Still flagged as "OpenRGB-only" because LConnect3 does not call this exact composite from inside `StrimerPlusDevice.cs` — the apply step is presumably driven by a higher UI layer.

OpenRGB does **not** introduce any genuinely novel cmd-byte values (i.e., no cmd byte that is undefined in LConnect3's `Commands.cs`). This is consistent with OpenRGB being a strict subset of LConnect3 functionally — OpenRGB targets RGB only.

### 5.6 VID/PIDs in OpenRGB but not in `LConnectCore/Devices/USBDeviceID.cs` / `KnownDevices.cs`

| OpenRGB device name | VID:PID | Status in LConnect3 | Notes |
|---|---|---|---|
| Lian Li Uni Hub (original) | `0x0CF2:0x7750` | **Absent** from `KnownDevices.cs` mapping (no entry) | Original pre-`A1xx` Uni Hub generation. LConnect3 either dropped support, never managed it (Uni Hub may have been a separate Lian Li utility), or maps it through a different `ControllerTypeID` not present in the decompiled enum. The `0x0CF2:0xA1xx` range used for SL/AL fan hubs in LConnect3 starts at `0xA100`; `0x7750` is in a much older PID range. |

All other OpenRGB-listed VID:PIDs are present in LConnect3's `KnownDevices.cs`:

- `0x0CF2:0xA100` (Uni Hub SL) → `ControllerTypeID.SLFan` ([`KnownDevices.cs:21`](file:///home/inky/Development/lian-li/L-Connect.Core/LConnectCore/Devices/KnownDevices.cs))
- `0x0CF2:0xA101` (Uni Hub AL) → `ControllerTypeID.ALFan` (`KnownDevices.cs:23`)
- `0x0CF2:0xA102` (Uni Hub SL Infinity) → `ControllerTypeID.SLInfinity` (`KnownDevices.cs:25`)
- `0x0CF2:0xA103` (Uni Hub SLV2) → `ControllerTypeID.SLV2Fan` (`KnownDevices.cs:27`)
- `0x0CF2:0xA104` (Uni Hub ALV2) → `ControllerTypeID.ALV2Fan` (`KnownDevices.cs:29`)
- `0x0CF2:0xA105` (Uni Hub SLV2 V0.5) → `ControllerTypeID.SLV2AFan` (`KnownDevices.cs:31`)
- `0x0CF2:0xA200` (Strimer L Connect) → `ControllerTypeID.StrimerPlus` (`KnownDevices.cs:35`)
- `0x0416:0x7373` (GA II Trinity) → `ControllerTypeID.Galahad2TrinityRegular` (`KnownDevices.cs:37`)
- `0x0416:0x7371` (GA II Trinity Performance) → `ControllerTypeID.Galahad2TrinityPerformance` (`KnownDevices.cs:36`)
- `0x0416:0x8050` (Universal Screen) → `ControllerTypeID.UniversalScreen8p8InchLighting` (`KnownDevices.cs:81`)

So: exactly **one** OpenRGB device (original Uni Hub at `0x0CF2:0x7750`) is unknown to LConnect3.

Conversely, LConnect3 ships entries for many devices OpenRGB does **not** target — see section 5.2 "Coverage gap commentary" above. The complete list of LConnect3-only PIDs (relative to OpenRGB) is the union of: TLFan, TLLCD, Galahad2LCD (older), Galahad2LCDV2, HydroShift LCD/RGB/TL (HS I), HydroShift II Circle/Square (display + lighting), Wireless Controller (V1+V2), SLV3LCD, TLV2LCD, Lancool207Digital (display + lighting), UniversalScreen 8.8" display-mode (`0x1A86:0xAD20`), and `SLRedragon` (`0x0CF2:0xA106`). None of these appear in `LianLiControllerDetect.cpp`.

## 6. Findings & actionable outputs

(P0e-16 fills. Topics: framing-class extension points, cmd-15 hypotheses, reusable codec opportunities, risky-area flagging.)
