## Family: Galahad II Vision (LCDV2)

**Source folder:** `LConnectCore/Products/Galahad2Vision/`
**Marketed name(s):** Galahad II Vision (Galahad II LCDV2). USB descriptor `LianLi-GA_II-LCD_v1.6` (`docs/development/protocol/galahad2-lcdv2.md:155-157`).
**ControllerTypeID enum value(s):** `Galahad2LCDV2 = 16846849` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:19`). Sibling family entries for context: `Galahad2LCD = 16846848` (`ControllerTypeID.cs:18`), `Galahad2TrinityPerformance` / `Galahad2TrinityRegular` (also in that enum). Class bucket: `ControllerClass.Galahad2LCD = 17` (`L-Connect.Core/LConnectCore/Devices/ControllerClass.cs:15`).
**USB VID:PID:** `0x0416:0x7395` (decimal `1046:29589`) — `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs:41`. Cross-checked vs `docs/development/protocol/galahad2-lcdv2.md:4` (`Winbond / Lian Li, GA_II-LCD`).
**Framing classes used:** A, B, C — `Galahad2Vision/ACommandPacket.cs`, `Galahad2Vision/BCommandPacket.cs`, `Galahad2Vision/CCommandPacket.cs` all confirmed present in the source folder. C-framing (ReportID=3, 512/512, 501-byte chunks) is used by `SendH264Frame512` (`Galahad2VisionDevice.cs:262`, `353`); see `docs/development/protocol/galahad2-lcdv2.md:76-79` for framing notes.
**Phase 0d status:** verified live (see `docs/development/protocol/galahad2-lcdv2.md`).

**Subsystem fingerprint:**
- Sensors / handshake: yes — cmd 129 `GetHandshakeInfo` (FanRpm, PumpRpm, temperature integer + fraction tenths; `galahad2-lcdv2.md:83-126`)
- Fan PWM: yes — cmd 139 `SetFanPWM` (`galahad2-lcdv2.md:173-195`)
- Pump PWM: yes — cmd 138 `SetPumpPWM` with `PumpPWMSource` enum (MCU=0, Motherboard=1; `galahad2-lcdv2.md:198-225`)
- Fan curve upload: no (deferred — no firmware-curve write command catalogued in `Commands.cs`)
- Lighting (preset modes): yes — cmd 131 `SetPumpLighting` and cmd 133 `SetFanLighting`, live-verified Phase 0c (`galahad2-lcdv2.md:296-348`, `385-429`); plus enumeration via cmd 130 `GetPumpLightingInfo` and cmd 132 `GetFanLightingInfo`
- Lighting (per-pixel LED): yes — cmd 20 `SetLEDColors`, live-verified Phase 0d (`galahad2-lcdv2.md:670-744`)
- LCD on/off / brightness / rotation: yes — cmd 12 `LCDControl` (codec-verified) and cmd 23 `IsLCDAvailable` (live-verified) (`galahad2-lcdv2.md:432-481`, `748-785`)
- LCD frame upload: yes cmd 14 `SendJpeg` (live-verified, ~20 s mode-independent hold; `galahad2-lcdv2.md:538-602`); unclear cmd 15 `UploadJpeg` (codec-only, activation path unknown on FW 1.6; `galahad2-lcdv2.md:606-666`); partial cmd 13 `SendH264Frame` (codec-only, no live verification yet; `galahad2-lcdv2.md:485-534`)
- Continuous re-issue pattern: yes — C# service has continuous re-issue timer on cmd 131 (`Galahad2VisionController.cs:2443`, cited in `galahad2-lcdv2.md:314-316`)
- Reset / soft-reset: yes — cmd 142 `ResetDevice` (codec-only; success iff `Length == 1 && Data[0] == 0x01`; device drops off USB 1-3 s; `galahad2-lcdv2.md:228-252`)
- Firmware upload / activation: deferred (Phase 1) — `UploadFirmware = 11`, `ActiveFirmware = 10` exist in `Galahad2Vision/Commands.cs` but are explicitly deferred in `galahad2-lcdv2.md:827-834`

### Commands

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| `0x0A` (10) | host->dev | `ActiveFirmware` | (TBD) | `Galahad2Vision/Commands.cs:8` | deferred | Phase 1 firmware path; not codec-implemented |
| `0x0B` (11) | host->dev | `UploadFirmware` | (TBD) | `Galahad2Vision/Commands.cs:9` | deferred | Phase 1 firmware path; not codec-implemented |
| `0x0C` (12) | host->dev | `LCDControl` | B | `Galahad2VisionDevice.cs:188-193`; `LCDControlSetting.cs:7-73`; `galahad2-lcdv2.md:432-481` | codec-only | 8-byte payload: mode, brightness 0..=100, rotation enum (wire 0/1/2/3 = 0/90/180/270 deg), enable_test, test_color RGB, video_fps |
| `0x0D` (13) | host->dev | `SendH264Frame` | B | `Galahad2VisionDevice.cs:255-260`; `galahad2-lcdv2.md:485-534` | codec-only | Raw H.264 frame as payload, no flag/header. 512-byte read after each write (response semantics unverified). Sibling `SendH264Frame512` uses C-framing |
| `0x0E` (14) | host->dev | `SendJpeg` | B | `Galahad2Vision/Commands.cs:11`; `galahad2-lcdv2.md:538-602` | live | Raw JPEG bytes payload. Transient single-frame display; firmware auto-reverts ~20 s (mode-independent). Daemon must refresh <=15 s for sustained display |
| `0x0F` (15) | host->dev | `UploadJpeg` | B | `Galahad2Vision/Commands.cs:12`; `galahad2-lcdv2.md:606-666` | unclear | Same wire shape as cmd 14, only Command byte differs. ACK'd on FW 1.6 but no visible LCD change in default or Application-mode preflight; activation path unknown |
| `0x12` (18) | host->dev | `UploadAvi` | (TBD) | `Galahad2Vision/Commands.cs:13` | deferred | Not codec-implemented |
| `0x13` (19) | host->dev | `UploadH264` | (TBD) | `Galahad2Vision/Commands.cs:14` | deferred | Not codec-implemented |
| `0x14` (20) | host->dev | `SetLEDColors` | B | `Galahad2VisionDevice.cs:283-308`; `galahad2-lcdv2.md:670-744` | live | First payload byte is `isFanLed` (0x01 fan / 0x00 screen). Fan path: total = `colors.len()*3 + 1`. Screen path: 24-byte zero pad before RGB triples; total = `(colors.len()+8)*3 + 1`. Caps: `MAX_FAN_LEDS=337`, `MAX_SCREEN_LEDS=329` per single B-packet; multi-packet chunks transparently |
| `0x15` (21) | host->dev | `UploadH264Info` | (TBD) | `Galahad2Vision/Commands.cs:16` | deferred | Not codec-implemented |
| `0x16` (22) | host->dev | `UploadStartupAviMovie` | (TBD) | `Galahad2Vision/Commands.cs:17` | deferred | Not codec-implemented |
| `0x17` (23) | host<->dev | `IsLCDAvailable` | B | `Galahad2VisionDevice.cs:175-186`; `galahad2-lcdv2.md:748-785` | live | Request: `Length=0`. Response: `Length=1`, `Data[0]==0x00` means available (counterintuitive zero-means-yes). Service controller polls ~1 Hz |
| `0x81` (129) | host<->dev | `GetHandshakeInfo` | A | `Galahad2VisionDevice.cs:107-119,250-253`; `HandshakeInfo.cs:58-76`; `galahad2-lcdv2.md:83-126` | live | Bare request. Response `Length=7`: FanRpm BE u16, PumpRpm BE u16, TempIsValid u8, TempInteger u8, TempFraction u8 (`% 10` = tenths C) |
| `0x82` (130) | host<->dev | `GetPumpLightingInfo` | A | `Galahad2VisionDevice.cs:122-135`; `PumpLightingInfo.cs:30-84`; `galahad2-lcdv2.md:256-292` | live | Escape-byte protocol terminated by `0xA5 0x5A`; trailer = MaxSpeed, MaxBrightness. 16 modes returned on FW 1.6, max_speed=4, max_brightness=4 |
| `0x83` (131) | host->dev | `SetPumpLighting` | A | `Galahad2VisionDevice.cs:310-313`; `PumpLightingSetting.cs:101-136`; `galahad2-lcdv2.md:296-348` | live | 19-byte payload: scope, mode, brightness, speed, color1..4 RGB, direction, disabled, source. LCDV2 SKU: only `--scope outer` drives visible ring; `inner` writes ACK but no visible change. Continuous re-issue timer present in C# (`Galahad2VisionController.cs:2443`) — likely workaround for transient buffering |
| `0x84` (132) | host<->dev | `GetFanLightingInfo` | A | `Galahad2VisionDevice.cs:73-86`; `FanLightingInfo.cs:30-84`; `galahad2-lcdv2.md:351-381` | live | Same escape-byte protocol as cmd 130; `FanLightingMode` enum (6 variants on FW 1.6: Rainbow, RainbowMorph, StaticColor, Breathing, Runway, Meteor). max_speed=4, max_brightness=4 |
| `0x85` (133) | host->dev | `SetFanLighting` | A | `Galahad2VisionDevice.cs:269-272`; `FanLightingSetting.cs:109-145`; `galahad2-lcdv2.md:385-429` | live | 20-byte payload: mode, brightness, speed, color1..4 RGB, direction, disabled, source, sync_to_pump, number_of_led (default 24). Visibility conditional on fans being wired to AIO fan header |
| `0x86` (134) | host<->dev | `GetFirmwareVersion` | A | `Galahad2VisionDevice.cs:88-105`; `FirmwareVersion.cs`; `galahad2-lcdv2.md:128-170` | live | Two-packet response demuxed by PacketNumber (BE u16 at `bytes[3..4]`). Pkt 0: 7 comma-separated version fields. Pkt 1: build datetime, only sent once per USB-attach lifecycle on FW 1.6. Unsolicited cmd-0x8e status packets sometimes interleave — drain/filter by cmd echo + PacketNumber |
| `0x88` (136) | host<->dev | `GetSerialNumber` | (TBD) | `Galahad2Vision/Commands.cs:21` | deferred | Phase 1+ |
| `0x89` (137) | host->dev | `SetSerialNumber` | (TBD) | `Galahad2Vision/Commands.cs:22` | deferred | Risky (writes serial); Phase 1+ |
| `0x8A` (138) | host->dev | `SetPumpPWM` | A | `Galahad2VisionDevice.cs:316-322`; `PumpPWMSource.cs`; `galahad2-lcdv2.md:198-225` | live | `Length=2`. Data: source (0=MCU/firmware curve, 1=Motherboard/MB header), percent 0..=100. No response. MCU mode treats percent as a *cap* over the firmware's own curve — direct percent control requires source=Motherboard |
| `0x8B` (139) | host->dev | `SetFanPWM` | A | `Galahad2VisionDevice.cs:271-281`; `galahad2-lcdv2.md:173-195` | live | `Length=2`. Data: group (always 0x00 on LCDV2), percent 0..=100 (firmware clamps higher values). Fire-and-forget; verify via cmd 129 fan_rpm |
| `0x8E` (142) | host<->dev | `ResetDevice` | A | `Galahad2VisionDevice.cs:233-248`; `galahad2-lcdv2.md:228-252` | codec-only | Bare request. Response: `Length=1`, `Data[0]==0x01`. Device drops off USB 1-3 s; caller must re-open handle. Codec only — not run live to avoid dropping pump during normal ops |

Sibling deferred (C-framing, same cmd byte 13):

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| `0x0D` (13) | host->dev | `SendH264Frame512` | C | `Galahad2VisionDevice.cs:262-267,353`; `Galahad2Vision/CCommandPacket.cs:10-200`; `galahad2-lcdv2.md:76-79,495-496` | deferred | C-framing: ReportID=0x03, 512 out / 512 in, 501-byte chunks. Higher-bandwidth H.264 path on certain device variants. Cmd byte collides with cmd 13 (B-framing); discrimination is presumably via ReportID at HID layer but device-variant / FW-revision sensitivity unverified. Phase 1 codec target |

### Subsystem patterns

- **Sensors / control loop:** cmd 129 (`GetHandshakeInfo`) is the single-shot status read covering fan RPM, pump RPM, and coolant temperature (integer + fraction-tenths). Sufficient for a daemon's polling loop without separate sensor commands. See `galahad2-lcdv2.md:83-126`.
- **PWM:** cmds 138 / 139 are simple 2-byte fire-and-forget writes; pump cmd 138 carries a source enum that gates whether `percent` is a direct setpoint or a cap over the firmware's MCU curve. Verify post-write via cmd 129. See `galahad2-lcdv2.md:173-225`.
- **Lighting (preset):** the cmd 130/132 enumeration commands round-trip the available `PumpLightingMode` / `FanLightingMode` lists plus `MaxSpeed` / `MaxBrightness` via an escape-byte protocol terminated by `0xA5 0x5A`. The cmd 131 / 133 setters then carry mode / RGB / direction / source / disabled fields. The C# service re-issues cmd 131 on a timer — likely a workaround for the transient-buffering behaviour observed in Phase 0c. See `galahad2-lcdv2.md:256-429`.
- **Per-pixel LED:** cmd 20 with the `isFanLed` flag selects the fan-ring (337 LEDs/packet cap) or screen / pump-cap (329 LEDs/packet cap with a load-bearing 24-byte zero-pad prefix) path. Multi-packet chunking via `BCommandPacket::build_packets_for_write`. Cmd 20 writes supersede cmd 131 in the LED render pipeline. See `galahad2-lcdv2.md:670-744`.
- **LCD lifecycle / frame upload:** cmd 23 confirms LCD presence (counterintuitive zero-means-available); cmd 12 sets mode / brightness / rotation / FPS. Frame upload via cmd 14 is a transient single-frame display (~20 s mode-independent firmware-side hold) — sustained display requires a refresh loop at <=15 s. Cmd 15 has the same wire shape but no observed activation on FW 1.6 — activation path is the Phase 0e open question. Cmd 13 (B-framing H.264) is codec-only; sibling C-framing variant is deferred. See `galahad2-lcdv2.md:432-666`.
- **Reset:** cmd 142 with a `Length=1, Data[0]=0x01` ACK; device drops off USB for 1-3 s and the caller must re-open the handle. Deliberately not run live in verification. See `galahad2-lcdv2.md:228-252`.
- **Crypto:** confirmed unused at the HID transport level for this family — see `docs/development/protocol/galahad2-lcdv2.md:837-846` and `docs/development/protocol/crypto-usage.md`.

### Notable divergences from LCDV2

This is the LCDV2 — this entry is the synthesis baseline. The cross-reference matrix uses this family's command set as the reference column. N/A for this section.

### Open questions / unclear from static analysis

- **Cmd 15 (`UploadJpeg`) activation path:** Phase 0d found the codec accepts 56-packet writes (ACK'd in full) but no visible LCD activation occurs in either default mode or after a `LCDControl{mode=Application}` preflight on FW 1.6 (`docs/development/protocol/galahad2-lcdv2.md:606-666`). Plausible (unverified) explanations enumerated in that doc include non-volatile / boot-image storage, an undocumented separate trigger command, or an internal display mode (`LocalAVI=3` or `LocalH264=2`) we have not activated. Hypothesis tree to be tested in the Phase 0e probe set.
- **Cmd 13 (`SendH264Frame`, B-framing):** wire shape verified against the C# call site at `Galahad2VisionDevice.cs:255-260` but no live verification (would require an H.264 frame stream). Hold time, decoder readiness, and auto-revert semantics are unmeasured. The C# discards the 512-byte read response; our codec exposes a typed parser. See `galahad2-lcdv2.md:485-534`.
- **Cmd 13 alternate (`SendH264Frame512`, C-framing):** uses the third framing class (ReportID=0x03, 512/512 lengths, 501-byte chunks per `CCommandPacket.cs:108-176`). Cmd byte collides with B-framing cmd 13; HID-layer discrimination via ReportID is presumed but device-variant / firmware-revision sensitivity is unverified. Codec deferred to Phase 1.
- **Cmd 142 (`ResetDevice`) live behaviour:** codec-only on the verification harness to avoid dropping the pump during normal operation. The C# wire flow at `Galahad2VisionDevice.cs:233-248` is the only source for the 1-3 s USB-detach window — duration on FW 1.6 is empirically unconfirmed.
- **Cmd 134 (`GetFirmwareVersion`) datetime packet:** PacketNumber=1 datetime is observed only once per USB-attach lifecycle on FW 1.6. Whether this is firmware-deterministic or a race is unconfirmed. Linux port tolerates its absence. See `galahad2-lcdv2.md:160-163`.
- **`SetPumpLighting` (cmd 131) transient buffering:** ~6-7 rapid back-to-back writes can leave the LED "stuck" on a previously rendered colour; subsequent writes are ACK'd but only the latest renders after some idle time. Cause unknown — firmware rate-limit, LCD-frame contention, or host-side USB/HID stall. See `galahad2-lcdv2.md:309-317`.
- **`SetPumpLighting` scope semantics:** `--scope inner` writes successfully but produces no visible change on the LCDV2 SKU; only `--scope outer` (or `all`) drives the visible LED ring. Suggests `inner` targets a region this SKU doesn't expose. See `galahad2-lcdv2.md:303-308`.
- **`LCDControl` `video_fps` field:** known-safe values are 24 (service brightness updates) and 30 (C# default constructor). Effects of other values undocumented. See `galahad2-lcdv2.md:472-475`.
- **Deferred-but-catalogued commands:** `ActiveFirmware` (10), `UploadFirmware` (11), `UploadAvi` (18), `UploadH264` (19), `UploadH264Info` (21), `UploadStartupAviMovie` (22), `GetSerialNumber` (136), `SetSerialNumber` (137) — all enumerated in `Galahad2Vision/Commands.cs` but not codec-implemented. Wire shape, framing class assignment (A/B/C), and risk profile to be characterised on demand. See `galahad2-lcdv2.md:827-834`.
