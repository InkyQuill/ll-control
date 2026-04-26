# Lian Li L-Connect3 — Cross-Device Command Survey

- **Status:** Phase 0e static survey complete - pending live probes (P0e-20) and post-execution wrap-up - findings ready for Phase 1 plan author
- **Date:** 2026-04-26
- **Scope:** all device families managed by LConnect3, plus an OpenRGB
  cross-validator pass

## 1. Sources

1. **Primary:** decompiled `LConnectCore/Products/<family>/` under
   `/home/inky/Development/lian-li/L-Connect.Core/`
2. **Cross-validator:** [OpenRGB's `Controllers/LianLiController/`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/LianLiController) (GPL-2.0+)
3. **Verified live:** `docs/protocol/captures/` (LCDV2 only as of Phase 0d)
4. **Shared SDK decompiles:** `slv3.dll` at `/home/inky/Development/lian-li/slv3/slv3/`
   and `lcd207.dll` at `/home/inky/Development/lian-li/lcd207/lcd207/` (the
   two proprietary native SDKs that L-Connect-Service / L-Connect.Core
   delegate frame-upload and RF traffic to)

OpenRGB is license-compatible for reading. The survey cites OpenRGB by
URL and file path; it does **not** copy OpenRGB code into our tree.
Protocol facts (command bytes, payload offsets, framing) are documented
in our own tables.

## 2. Conventions

- Every command entry carries a citation `LConnectCore/Products/<family>/<file>.cs:<start>-<end>`
- **Status flag vocabulary** (used uniformly across sections 3 and 4):
  - `verified-live` - verified by Phase 0d hardware run (LCDV2 only at
    time of writing)
  - `codec-only` - codec implemented in Rust, not yet live-tested
  - `inferred-static` - read from C# source, not live-tested
  - `declared-only` - appears in enum but no caller found
  - `unclear` - partial info, gaps remain
  - `deferred` - explicitly deferred to a later phase
- Framing classes catalogued so far:
  - **A** - 64-byte ACommandPacket (`Galahad2Vision/ACommandPacket.cs`),
    ReportID=1, 6-byte header, 58-byte payload
  - **B** - 1024 out / 512 in BCommandPacket
    (`Galahad2Vision/BCommandPacket.cs`), ReportID=2, 11-byte header,
    1013-byte payload
  - **C** - 512 out / 512 in CCommandPacket
    (`Galahad2Vision/CCommandPacket.cs`), ReportID=3, 11-byte header,
    501-byte payload
  - **T** - TLLCD-specific (`TLLCD/LCDPacket.cs`), 512 out / 64 in
    asymmetric, ReportID=2, 11-byte header, 501-byte payload
  - **HID-feature `0xE0`** - Strimer Plus / Ene6K77Fan, flat HID feature
    + output reports keyed by ReportID `0xE0`
  - **WinUSB+DES (slv3 / lcd207)** - libusb / WinUSB EP1, 102,400-byte
    frame buffer with a DES-CBC-encrypted 504-byte header at offset 0
    (key/IV = `slv3tuzx`) + raw body at offset 512
  - **WinUSB raw 64-byte (lcd207 LED-side, slv3 RF dongle)** - libusb /
    WinUSB EP1, 64-byte unencrypted frames
  - **UniversalScreen lighting bulk-EP1** (per OpenRGB cross-validation)
    - libusb bulk transfer, 64-byte chunks with `[0x11, offset, 0, 0,
    RGB...]` header, 20 LEDs / chunk
  - **JSON-over-IPC + SDK passthrough** - SLV3LCD / TLV2LCD / HS II LCD
    / UniversalScreen / LWireless / Lancool 207 Digital - the C# layer
    builds JSON service requests; the actual wire bytes live inside
    `slv3.dll` or `lcd207.dll`

## 3. Per-family writeups

(Assembled from `docs/protocol/cross-device-survey/per-family/*.md` by
P0e-15. Each per-family file remains the editable source-of-truth; this
section is a synthesis snapshot.)

### 3.1 Device families

#### 3.1.1 TLFan

**Source folder:** `LConnectCore/Products/TLFan/`

**Marketed name(s):** Lian Li TL series fans - addressable RGB case fans with daisy-chain controller. Two SKU classes are bucketed under the TL umbrella in this codebase: the `TLFan` controller (LED-only TL fans, no on-fan LCD) and `TLLCD` (TL fans with on-fan LCD screens - separate `Products/TLLCD/` folder, out of scope here). Constants name 120 mm and 140 mm chassis variants for both `LED` and `LCD` port types (`TLFan/Constants.cs:21-35`); the TLFan controller drives both flavours of fan over its 4 ports. The `PortType` (LED vs LCD) is configured per-port and changes the maximum-RPM table only.

**ControllerTypeID enum value(s):** `TLFan = 16908288` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:23`). The sibling `TLLCD = 16912384` is a separate device.

**USB VID:PID:** `0x0416:0x7372` (decimal `1046:29554`) - `KnownDevices.cs:49`. The TLFan registration carries a `USBDeviceExtendedID` with `Usage = 0xFF1B` and a null `UsagePage`. VID `0x0416` is Winbond / shared with the AIO families; PID `0x7372` is unique to the TL fan controller.

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | UsageID | Source |
|---|---|---|---|---|
| `TLFan` (16908288) | `0x0416:0x7372` | `1046:29554` | `0xFF1B` | `KnownDevices.cs:49` |

**Framing classes used:** **family-specific** - TLFan uses two parallel 64-byte ReportID=1 packet types, structurally identical but semantically partitioned:
- `ControllerPacket` (`TLFan/ControllerPacket.cs:6-97`) - for the bootloader / firmware-flashing controller MCU.
- `LEDPacket` (`TLFan/LEDPacket.cs:10-131`) - for the LED/runtime MCU; all runtime traffic in `TLFanDevice.cs` flows through this class.

Both share the LCDV2 A-class layout exactly (ReportID=1, 6-byte header, 58-byte payload, BE-u16 PacketNumber at bytes[3..4], Length at bytes[5]). `LEDPacket.BuildPacketsForWrite` (`TLFan/LEDPacket.cs:89-117`) defines a multi-packet chunker, but no caller in `TLFanDevice.cs` or `TLFanController.cs` invokes it.

**Phase 0d status:** not in scope; static-analysis only.

**Subsystem fingerprint:**
- Sensors / handshake: `inferred-static` - `GetHandshakeInfo` (cmd 161) returns a flat array of 3-byte `FanInfo` records, one per detected fan, encoding `IsDetected/IsUpgrading` flags + 4-bit port + 4-bit fan-index in byte 0 and BE-u16 fan RPM in bytes 1-2 (`TLFan/TLFanHandshakeInfo.cs:64-84`, `129-136`). Up to ~19 fans per single packet.
- Fan PWM: `inferred-static` - `SetFanSpeed` (cmd 170) writes a single `(port, fanIndex, pwm)` tuple per call (`TLFan/TLFanDevice.cs:274-283`). PWM u8 0..=100 percent. Service controller fans out a curve-derived PWM via this command in a 1 Hz loop (`L-Connect-Service/LConnectService/Products/TLFan/TLFanController.cs:1392-1485`).
- Pump PWM: N/A.
- Fan curve upload: no on-device curve. All curve evaluation runs on the host.
- Lighting (preset modes): `inferred-static` - `SetFanLight` (cmd 163, 20-byte payload) and `SetGroupLight` (cmd 176, 20-byte payload). Modes are `LightingMode` (29 base modes plus `_Merge`/`_Side`/`_Side_Merge` variants - `TLFan/LightingMode.cs:7-83`); high-byte category stripped by `getLightingModeValue` (mode % 1000) before being sent on the wire.
- Lighting (per-pixel LED): no.
- LCD on/off: N/A. `LEDCommands.LCDScreenSwitch = 178` enum value declared but no call site issues byte 178.
- LCD frame upload: N/A.
- Continuous re-issue pattern: `inferred-static` - `writeFanSpeedTimer` re-issues cmd 170 per fan; `updateFanInfoTimer` polls cmd 161 at 1 Hz.
- Reset / soft-reset: no.
- Firmware upload / activation: `declared-only` (`SendFirmware = 172`, `ActiveFirmware = 171` - `TLFan/LEDCommands.cs:15-16`; `EnterUpgradeMode = 11`, `WriteFirmware = 12` for controller MCU - `TLFan/ControllerCommands.cs:7-12`).
- Crypto / obfuscation at HID transport: none observed.

##### Commands

`LEDPacket` traffic (the runtime/LED MCU; `LEDCommands` enum at `TLFan/LEDCommands.cs:5-25`):

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| 161 (0xA1) | host<->dev | `Handshake` (`GetHandshakeInfo`) | A | `TLFan/TLFanDevice.cs:155-188` | inferred-static | Tx: `Command=161`, no data. Rx: same cmd echo, `Data` = Nx3 bytes (FanInfo triples). |
| 162 (0xA2) | - | `GetLightModeList` | A | `TLFan/LEDCommands.cs:8` | declared-only | No call site. |
| 163 (0xA3) | host->dev | `SetFanLight` | A | `TLFan/TLFanDevice.cs:244-272` | inferred-static | 20-byte payload (see below). |
| 166 (0xA6) | host<->dev | `GetProductInfo` (firmware version) | A | `TLFan/TLFanDevice.cs:55-153` | inferred-static | Two response packets indexed by `Data[2]`. |
| 167 (0xA7) | - | `LEDTestMode` | A | `TLFan/LEDCommands.cs:11` | declared-only | |
| 168 (0xA8) | - | `GetSerialNumber` | A | `TLFan/LEDCommands.cs:12` | declared-only | |
| 169 (0xA9) | - | `SetSerialNumber` | A | `TLFan/LEDCommands.cs:13` | declared-only | **Destructive**. |
| 170 (0xAA) | host->dev | `SetFanSpeed` | A | `TLFan/TLFanDevice.cs:274-283` | inferred-static | Tx: `Command=170`, `Data=[(port<<4)\|fanIndex, pwm]`. PWM u8 0..=100. |
| 171 (0xAB) | - | `ActiveFirmware` | A | `TLFan/LEDCommands.cs:15` | declared-only | **Destructive**. |
| 172 (0xAC) | - | `SendFirmware` | A | `TLFan/LEDCommands.cs:16` | declared-only | **Destructive**. |
| 173 (0xAD) | host->dev | `SetFanGrouping` | A | `TLFan/TLFanDevice.cs:222-242` | inferred-static | Defines logical lighting/RPM groups. |
| 174 (0xAE) | host->dev | `SetFanDirection` | A | `TLFan/TLFanDevice.cs:211-220` | inferred-static | Per-fan direction swap. |
| 175 (0xAF) | host->dev | `SetPortDirection` | A | `TLFan/TLFanDevice.cs:325-334` | inferred-static | Per-port direction swap. |
| 176 (0xB0) | host->dev | `SetFanGroupLight` | A | `TLFan/TLFanDevice.cs:285-312` | inferred-static | 20-byte group-targeted lighting. |
| 177 (0xB1) | host->dev | `SetMBFanSync` | A | `TLFan/TLFanDevice.cs:314-323` | inferred-static | 1-byte payload toggling MB sync. |
| 178 (0xB2) | - | `LCDScreenSwitch` | A | `TLFan/LEDCommands.cs:22` | declared-only | TLFan does not drive on-fan LCDs in this code. |
| 179 (0xB3) | host->dev | `TestLight` | A | `TLFan/TLFanDevice.cs:336-360` | inferred-static | Visual identification flash. |
| 180 (0xB4) | host->dev | `BlinkPort` | A | `TLFan/TLFanDevice.cs:44-53` | inferred-static | |

`ControllerPacket` traffic (the bootloader / controller MCU; `ControllerCommands` enum at `TLFan/ControllerCommands.cs:5-12`): all entries are `declared-only` with no call site found.

| Cmd byte | Name | Citation | Notes |
|---|---|---|---|
| 1 (0x01) | `Handshake` (controller) | `TLFan/ControllerCommands.cs:7` | |
| 11 (0x0B) | `EnterUpgradeMode` | `TLFan/ControllerCommands.cs:8` | **Destructive.** |
| 12 (0x0C) | `WriteFirmware` | `TLFan/ControllerCommands.cs:9` | **Destructive.** |
| 13 (0x0D) | `GetProductInfo` (controller) | `TLFan/ControllerCommands.cs:10` | |
| 21 (0x15) | `GetSerialNumber` (controller) | `TLFan/ControllerCommands.cs:11` | |

**SetFanLight payload (cmd 163, 20 bytes - `TLFan/TLFanDevice.cs:244-272`):**

| offset | size | field | encoding |
|---|---|---|---|
| 0 | 1 | `port_sync` | `(port<<4) \| isSync` |
| 1 | 1 | `port_fan` | `(port<<4) \| fanIndex` |
| 2 | 1 | `mode` | base `LightingMode` byte (`mode % 1000`) |
| 3 | 1 | `brightness` | u8 (0..=4) |
| 4 | 1 | `speed` | u8 (0..=4) |
| 5..7 | 3 | `color1` | RGB |
| 8..10 | 3 | `color2` | RGB |
| 11..13 | 3 | `color3` | RGB |
| 14..16 | 3 | `color4` | RGB |
| 17 | 1 | `direction` | `LightingDirection` (0..=5) |
| 18 | 1 | `disabled` | bool 0/1 |
| 19 | 1 | `colorCount` | u8 (0..=4) |

**SetGroupLight payload (cmd 176, 20 bytes - `TLFan/TLFanDevice.cs:285-312`):** identical to SetFanLight except byte 0 is unused (always 0) and byte 1 carries `groupNumber`. `disabled=true` semantics differ subtly (zeroes only colors, retains mode/brightness/speed/direction/count).

##### Subsystem patterns

- **Two-MCU device** with shared HID pipe and 64-byte ReportID=1 framing; runtime traffic on LED MCU side only.
- **Per-fan addressing scheme** via `(port<<4) | fanIndex` byte primitive. Topology: 4 ports x up to 16 fans per port.
- **Fan curves run on the host** - no on-device curve upload.
- **Lighting modes** are u32 categories collapsed to u8 on wire via `% 1000`.
- **Read-after-write everywhere** - response is read and discarded for every write except cmds 161 and 166.
- **Continuous re-issue pattern** at 1 Hz on cmd 170 (PWM) and cmd 161 (handshake polling).

##### Notable divergences from LCDV2

- Same A-class framing, different command space; no B/C-class.
- Per-fan addressing primitive (LCDV2 has a single `group=0` byte).
- Lighting payload differs - 20 bytes with `colorCount` at byte 19; LCDV2 has `ArgbSignalSource` / `sync_to_pump` / `number_of_led` at bytes 17-19.
- Lighting modes are richer (28 base + scope variants).
- No reset, no LCD subsystem, no pump.
- Handshake response is a fan list (Nx3 bytes), not a fixed 7-byte struct.
- Continuous re-issue is for fan PWM, not lighting.

##### Open questions

- Cmds 162, 167, 168, 169, 171, 172, 178 are enum-declared but unused.
- `ControllerPacket` traffic (cmds 1, 11, 12, 13, 21) has no caller.
- Handshake response truncation behaviour for >19 fans is unverified.
- Cmd 166 firmware-version response packet ordering - C# de-multiplexes on read order rather than `Data[2]` index.
- `getLightingModeValue` truncation at `% 1000` collapses scope categories on wire.
- `SetFanGroupLight` (cmd 176) `disabled` semantics asymmetry vs cmd 163.
- `isSync` shortcut firmware-side semantics in cmd 163.
- Read-after-write discard - response payload contents are unverified.

#### 3.1.2 Galahad II Trinity

**Source folder:** `LConnectCore/Products/Galahad2Trinity/`

**Marketed name(s):** Lian Li Galahad II Trinity AIO liquid cooler - non-LCD ARGB pump-cap variant. Two SKUs:
- Galahad II Trinity (Regular): pump RPM range 2200-3200
- Galahad II Trinity Performance: pump RPM range 2200-4200

**ControllerTypeID enum value(s):**
- `Galahad2TrinityPerformance = 0x01010000` (16842752) - `LConnectCore/Devices/KnownDevices.cs:36`, defined at `LConnectCore/Devices/ControllerTypeID.cs:16`
- `Galahad2TrinityRegular = 0x01010001` (16842753) - `LConnectCore/Devices/KnownDevices.cs:37`, defined at `LConnectCore/Devices/ControllerTypeID.cs:17`

**USB VID:PID:**
- `0x0416:0x7371` (1046:29553) - Trinity Performance, `LConnectCore/Devices/KnownDevices.cs:36`
- `0x0416:0x7373` (1046:29555) - Trinity Regular, `LConnectCore/Devices/KnownDevices.cs:37`
- Both SKUs additionally filter on `UsagePage = 0xFF1B` and `Usage = 0x91` - `LConnectCore/Devices/ControllerFinder.cs:32`

**Framing classes used:** **A only.** The Trinity-local class is named simply `CommandPacket` (`Galahad2Trinity/CommandPacket.cs:10-131`) and is structurally identical to the LCDV2 `ACommandPacket`: ReportID=1, 6-byte header, 58-byte payload, BE u16 PacketNumber at bytes[3..4], Length at bytes[5]. No B-class or C-class framing.

**Subsystem fingerprint:**
- Sensors / handshake: `inferred-static` - cmd 129 (FanRpm + PumpRpm only - no temperature, unlike LCDV2)
- Fan PWM: `inferred-static` - cmd 139 (`[mb_sync, percent]`)
- Pump PWM: `inferred-static` - cmd 138 (`[mb_sync, percent]`)
- Fan curve upload: no.
- Lighting (preset modes): `inferred-static` - cmd 131 pump, cmd 133 fan (same 19/20-byte framing as LCDV2, mode enums diverge).
- Lighting (per-pixel LED): no.
- LCD on/off / brightness / rotation: N/A.
- LCD frame upload: N/A.
- Continuous re-issue pattern: `unclear` from static analysis.
- Reset / soft-reset: no.
- Firmware upload / activation: no (`UploadFirmware`/`ActiveFirmware` analogs absent).

##### Commands

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| 129 (0x81) | host<->dev | `Handshake` (RPM telemetry) | A | `Galahad2Trinity/Commands.cs:7`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:110-146`; parser `Galahad2Trinity/HandshakeInfo.cs:29-44` | inferred-static | Response payload: 4 bytes - `FanRpm` BE u16, `PumpRpm` BE u16. **No temperature byte**. |
| 130 (0x82) | host->dev | `GetPumpLightModeList` | A | `Galahad2Trinity/Commands.cs:8` | declared-only | No caller in the device class. |
| 131 (0x83) | host->dev | `SetPumpLight` | A | `Galahad2Trinity/Commands.cs:9`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:191-199`; payload `Galahad2Trinity/PumpLightingSetting.cs:77-96` | inferred-static | 19-byte payload, same offset layout as LCDV2 cmd 131. **Mode-byte semantics diverge.** `(byte)(mode % 1000)` applied. |
| 132 (0x84) | host->dev | `GetLEDLightModeList` (fan) | A | `Galahad2Trinity/Commands.cs:10` | declared-only | No caller. |
| 133 (0x85) | host->dev | `SetLEDLight` (fan) | A | `Galahad2Trinity/Commands.cs:11`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:159-167`; payload `Galahad2Trinity/FanLightingSetting.cs:85-105` | inferred-static | 20-byte payload, same offset layout as LCDV2 cmd 133. Trinity has 16 fan modes (1-16) vs LCDV2's 7 (0-6). |
| 134 (0x86) | host<->dev | `GetProductInfo` (firmware) | A | `Galahad2Trinity/Commands.cs:12`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:41-108` | inferred-static | Two response packets: ASCII version + ASCII datetime. |
| 135 (0x87) | host->dev | `LEDTestMode` | A | `Galahad2Trinity/Commands.cs:13` | declared-only | No caller. |
| 136 (0x88) | host<->dev | `ReadSerialNumber` | A | `Galahad2Trinity/Commands.cs:14` | declared-only | No caller. |
| 137 (0x89) | host->dev | `WriteSerialNumber` | A | `Galahad2Trinity/Commands.cs:15` | declared-only | **Destructive** - factory only. |
| 138 (0x8A) | host->dev | `SetPump` (pump PWM) | A | `Galahad2Trinity/Commands.cs:16`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:201-221` | inferred-static | 2-byte payload `[mb_sync, percent]`. |
| 139 (0x8B) | host->dev | `SetFan` (fan PWM) | A | `Galahad2Trinity/Commands.cs:17`; flow `Galahad2Trinity/Galahad2TrinityDevice.cs:169-189` | inferred-static | 2-byte payload `[mb_sync, percent]`. **Differs from LCDV2 cmd 139** (LCDV2 byte 0 is `group`, always 0). |
| 140 (0x8C) | host<->dev | `GetFanPumpParams` | A | `Galahad2Trinity/Commands.cs:18` | declared-only | No caller. |

##### Subsystem patterns

- **Single framing class.** Everything goes through 64-byte `CommandPacket` (A-class).
- **No LCD subsystem.**
- **PWM control: identical 2-byte shape for fan and pump.** Both cmd 138 and cmd 139 use `[mb_sync_bool, percent_clamped_to_100]`.
- **Lighting control: A-class only.** Both pump (cmd 131) and fan (cmd 133) lighting are sent as single 64-byte packets with 19/20-byte payloads.
- **PumpLightingMode is a 16-bit enum folded to a byte via `% 1000`** (`Galahad2Trinity/PumpLightingSetting.cs:81`). Scope distinction is carried separately in `payload[0]`.
- **No `ResetDevice` and no firmware-upload commands.**

##### Notable divergences from LCDV2

**Headline: cmd 131 / cmd 133 are structurally identical to LCDV2 byte-for-byte at framing and payload-offset level, but mode-byte semantics diverge.**

- PumpLightingMode wire-byte set differs. LCDV2: 16 modes flat (0..16); Trinity: 36 modes spread across three scope-tagged ranges, folded with `% 1000` to wire byte 0..=15.
- FanLightingMode wire-byte set differs. LCDV2: 7 modes (0..6); Trinity: 16 modes (1..16). Modes 1-6 match by name on both.
- Pump scope enum is wider on Trinity (`{Inner=0, Outer=1, All=2}` - `Galahad2Trinity/LightingScope.cs:5-10`).
- No temperature byte in cmd 129 handshake.
- Cmd 139 byte 0 means different things (LCDV2: `group`, always 0; Trinity: `mb_sync`).
- No reset, no firmware upload, no LCD.
- Pump RPM range varies by SKU.

##### Open questions

- Cmd 130, 132, 135, 136, 137, 140 payload formats not exercised by the device class.
- Continuous re-issue pattern - whether Trinity needs LCDV2's lighting-timer workaround is unverified.
- Cmd 131/133 mode-byte cross-compatibility with LCDV2 firmware.
- Whether Trinity supports a `PumpPWMSource` distinction beyond bool.

#### 3.1.3 Galahad II Vision (LCDV2) - synthesis baseline

**Source folder:** `LConnectCore/Products/Galahad2Vision/`

**Marketed name(s):** Galahad II Vision (Galahad II LCDV2). USB descriptor `LianLi-GA_II-LCD_v1.6` (`docs/protocol/galahad2-lcdv2.md:155-157`).

**ControllerTypeID enum value(s):** `Galahad2LCDV2 = 16846849` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:19`). Sibling family entries: `Galahad2LCD = 16846848` (`ControllerTypeID.cs:18`), `Galahad2TrinityPerformance` / `Galahad2TrinityRegular`. Class bucket: `ControllerClass.Galahad2LCD = 17`.

**USB VID:PID:** `0x0416:0x7395` (decimal `1046:29589`) - `KnownDevices.cs:41`.

**Framing classes used:** A, B, C - `Galahad2Vision/ACommandPacket.cs`, `Galahad2Vision/BCommandPacket.cs`, `Galahad2Vision/CCommandPacket.cs` all confirmed present.

**Phase 0d status:** `verified-live` - see `docs/protocol/galahad2-lcdv2.md`.

**Subsystem fingerprint:**
- Sensors / handshake: `verified-live` - cmd 129 (FanRpm, PumpRpm, temperature integer + fraction tenths).
- Fan PWM: `verified-live` - cmd 139 `SetFanPWM`.
- Pump PWM: `verified-live` - cmd 138 `SetPumpPWM` with `PumpPWMSource` enum.
- Fan curve upload: no (deferred).
- Lighting (preset modes): `verified-live` - cmd 131 / cmd 133.
- Lighting (per-pixel LED): `verified-live` - cmd 20 `SetLEDColors`.
- LCD on/off / brightness / rotation: `codec-only` for cmd 12 `LCDControl`; `verified-live` for cmd 23 `IsLCDAvailable`.
- LCD frame upload: `verified-live` cmd 14 `SendJpeg` (~20 s mode-independent hold); `unclear` cmd 15 `UploadJpeg` (codec-only, activation path unknown on FW 1.6); `codec-only` cmd 13 `SendH264Frame`.
- Continuous re-issue pattern: yes - C# service has continuous re-issue timer on cmd 131 (`Galahad2VisionController.cs:2443`).
- Reset / soft-reset: `codec-only` - cmd 142 `ResetDevice` (success iff `Length == 1 && Data[0] == 0x01`; device drops off USB 1-3 s).
- Firmware upload / activation: `deferred` (Phase 1) - `UploadFirmware = 11`, `ActiveFirmware = 10` exist in `Galahad2Vision/Commands.cs`.

##### Commands

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| `0x0A` (10) | host->dev | `ActiveFirmware` | (TBD) | `Galahad2Vision/Commands.cs:8` | deferred | Phase 1 firmware path. |
| `0x0B` (11) | host->dev | `UploadFirmware` | (TBD) | `Galahad2Vision/Commands.cs:9` | deferred | Phase 1 firmware path. |
| `0x0C` (12) | host->dev | `LCDControl` | B | `Galahad2Vision/Galahad2VisionDevice.cs:188-193`; `Galahad2Vision/LCDControlSetting.cs:7-73` | codec-only | 8-byte payload: mode, brightness 0..=100, rotation enum, enable_test, test_color RGB, video_fps. |
| `0x0D` (13) | host->dev | `SendH264Frame` | B | `Galahad2Vision/Galahad2VisionDevice.cs:255-260` | codec-only | Raw H.264 frame as payload. |
| `0x0E` (14) | host->dev | `SendJpeg` | B | `Galahad2Vision/Commands.cs:11` | verified-live | Raw JPEG bytes. Transient single-frame display; firmware auto-reverts ~20 s. Daemon must refresh <=15 s. |
| `0x0F` (15) | host->dev | `UploadJpeg` | B | `Galahad2Vision/Commands.cs:12` | unclear | Same wire shape as cmd 14, only Command byte differs. ACK'd on FW 1.6 but no visible LCD change in default or Application-mode preflight; activation path unknown. |
| `0x12` (18) | host->dev | `UploadAvi` | (TBD) | `Galahad2Vision/Commands.cs:13` | deferred | |
| `0x13` (19) | host->dev | `UploadH264` | (TBD) | `Galahad2Vision/Commands.cs:14` | deferred | |
| `0x14` (20) | host->dev | `SetLEDColors` | B | `Galahad2Vision/Galahad2VisionDevice.cs:283-308` | verified-live | First payload byte is `isFanLed` (0x01 fan / 0x00 screen). Caps: `MAX_FAN_LEDS=337`, `MAX_SCREEN_LEDS=329`. |
| `0x15` (21) | host->dev | `UploadH264Info` | (TBD) | `Galahad2Vision/Commands.cs:16` | deferred | |
| `0x16` (22) | host->dev | `UploadStartupAviMovie` | (TBD) | `Galahad2Vision/Commands.cs:17` | deferred | |
| `0x17` (23) | host<->dev | `IsLCDAvailable` | B | `Galahad2Vision/Galahad2VisionDevice.cs:175-186` | verified-live | Response: `Length=1`, `Data[0]==0x00` means available. |
| `0x81` (129) | host<->dev | `GetHandshakeInfo` | A | `Galahad2Vision/Galahad2VisionDevice.cs:107-119,250-253`; `Galahad2Vision/HandshakeInfo.cs:58-76` | verified-live | Response `Length=7`: FanRpm BE u16, PumpRpm BE u16, TempIsValid u8, TempInteger u8, TempFraction u8. |
| `0x82` (130) | host<->dev | `GetPumpLightingInfo` | A | `Galahad2Vision/Galahad2VisionDevice.cs:122-135`; `Galahad2Vision/PumpLightingInfo.cs:30-84` | verified-live | Escape-byte protocol terminated by `0xA5 0x5A`; trailer = MaxSpeed, MaxBrightness. 16 modes on FW 1.6. |
| `0x83` (131) | host->dev | `SetPumpLighting` | A | `Galahad2Vision/Galahad2VisionDevice.cs:310-313`; `Galahad2Vision/PumpLightingSetting.cs:101-136` | verified-live | 19-byte payload. Continuous re-issue timer present in C#. |
| `0x84` (132) | host<->dev | `GetFanLightingInfo` | A | `Galahad2Vision/Galahad2VisionDevice.cs:73-86`; `Galahad2Vision/FanLightingInfo.cs:30-84` | verified-live | Escape-byte protocol; 6 fan-side modes on FW 1.6. |
| `0x85` (133) | host->dev | `SetFanLighting` | A | `Galahad2Vision/Galahad2VisionDevice.cs:269-272`; `Galahad2Vision/FanLightingSetting.cs:109-145` | verified-live | 20-byte payload, default `number_of_led=24`. |
| `0x86` (134) | host<->dev | `GetFirmwareVersion` | A | `Galahad2Vision/Galahad2VisionDevice.cs:88-105`; `Galahad2Vision/FirmwareVersion.cs` | verified-live | Two-packet response demuxed by PacketNumber. |
| `0x88` (136) | host<->dev | `GetSerialNumber` | (TBD) | `Galahad2Vision/Commands.cs:21` | deferred | |
| `0x89` (137) | host->dev | `SetSerialNumber` | (TBD) | `Galahad2Vision/Commands.cs:22` | deferred | Risky. |
| `0x8A` (138) | host->dev | `SetPumpPWM` | A | `Galahad2Vision/Galahad2VisionDevice.cs:316-322`; `Galahad2Vision/PumpPWMSource.cs` | verified-live | Data: source (0=MCU, 1=Motherboard), percent 0..=100. |
| `0x8B` (139) | host->dev | `SetFanPWM` | A | `Galahad2Vision/Galahad2VisionDevice.cs:271-281` | verified-live | Data: group (always 0x00), percent 0..=100. |
| `0x8E` (142) | host<->dev | `ResetDevice` | A | `Galahad2Vision/Galahad2VisionDevice.cs:233-248` | codec-only | Response: `Length=1`, `Data[0]==0x01`. Device drops off USB 1-3 s. |

Sibling deferred (C-framing, same cmd byte 13):

| Cmd byte | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|
| `0x0D` (13) | `SendH264Frame512` | C | `Galahad2Vision/Galahad2VisionDevice.cs:262-267,353`; `Galahad2Vision/CCommandPacket.cs:10-200` | deferred | C-framing: ReportID=0x03, 512/512, 501-byte chunks. Phase 1 codec target. |

##### Subsystem patterns

- **Sensors / control loop:** cmd 129 single-shot status read.
- **PWM:** cmds 138 / 139 fire-and-forget; pump cmd 138 `source` enum gates direct setpoint vs cap over MCU curve.
- **Lighting (preset):** cmds 130/132 enumerate available modes via escape-byte `0xA5 0x5A`; cmds 131/133 set them.
- **Per-pixel LED:** cmd 20 with `isFanLed` flag.
- **LCD lifecycle / frame upload:** cmd 23 confirms LCD presence; cmd 12 sets mode/brightness/rotation/FPS; cmd 14 transient single-frame.
- **Reset:** cmd 142 with ACK; device drops off USB 1-3 s.
- **Crypto:** confirmed unused at HID transport.

##### Notable divergences from LCDV2

This is the LCDV2 - the synthesis baseline. N/A for this section.

##### Open questions

- Cmd 15 (`UploadJpeg`) activation path unresolved on FW 1.6.
- Cmd 13 (B-framing H.264) live verification pending.
- Cmd 13 alternate (`SendH264Frame512`, C-framing) cmd-byte collision discrimination via ReportID is presumed but unverified.
- Cmd 142 live behaviour - drop window duration on FW 1.6 empirically unconfirmed.
- Cmd 134 datetime packet appearance is observed only once per USB-attach lifecycle on FW 1.6.
- `SetPumpLighting` (cmd 131) transient buffering quirk - ~6-7 rapid back-to-back writes can leave LED stuck.
- `SetPumpLighting` scope semantics - `--scope inner` writes successfully but produces no visible change on the LCDV2 SKU.
- `LCDControl` `video_fps` field - known-safe values 24 and 30; effects of other values undocumented.
- Deferred-but-catalogued commands: `ActiveFirmware` (10), `UploadFirmware` (11), `UploadAvi` (18), `UploadH264` (19), `UploadH264Info` (21), `UploadStartupAviMovie` (22), `GetSerialNumber` (136), `SetSerialNumber` (137).

#### 3.1.4 Galahad II LCD (older, pre-Vision)

**Source folder:** none - no dedicated `Galahad2LCD/` product folder exists in `LConnectCore/Products/`. Only `Galahad2Trinity/` and `Galahad2Vision/` are present.

**ControllerTypeID enum value:** `Galahad2LCD = 16846848` cited from `LConnectCore/Devices/ControllerTypeID.cs:18`.

**USB VID:PID:** `0x0416:0x7391` (decimal `1046:29585`) cited from `LConnectCore/Devices/KnownDevices.cs:39`.

**Status:** `unclear`.

A repository-wide search for `Galahad2LCD` (excluding the `Galahad2LCDV2` substring) returns hits only in:
- `LConnectCore/Devices/ControllerClass.cs:15` - enum value
- `LConnectCore/Devices/ControllerTypeID.cs:18` - enum value
- `LConnectCore/Devices/ControllerTypeIDDefinitions.cs:43,90` - class constant
- `LConnectCore/Devices/KnownDevices.cs:39` - VID/PID registration
- `L-Connect3/LConnect3cs/Views/Pages/Settings/Galahad2LCDDevice.cs` - a WPF `UserControl` shell (no protocol logic)

There is **no** `Galahad2LCDDevice` protocol class, no factory or dispatcher branch that constructs a Vision/Trinity device for the older LCD enum value, and no `if (controllerType == ControllerTypeID.Galahad2LCD)` branching inside `Galahad2Vision/Galahad2VisionDevice.cs`.

The hypothesis from the survey plan ("the older LCD is one of the SKUs that `Galahad2Vision/` serves via ControllerTypeID branching") is **not confirmed** by the decompiled source.

##### Notable divergences from LCDV2

Cannot be enumerated - there is no older-LCD device class to compare. Wire-format command set for the older variant is **unknown** from this snapshot alone.

##### Open questions

- Is the older `Galahad2LCD` device-class implementation present in a part of the legacy app that is missing from the current decompile?
- Does the older variant share the Vision wire format at runtime?
- Pre-Vision SKU is not in user's hands; live verification out of scope for Phase 0e.

##### Recommendation

Treat the older Galahad II LCD as **unsupported / unknown** for the Linux port until either (a) decompiled source for its device class surfaces, or (b) live hardware becomes available. Do not assume LCDV2-compatible framing.

#### 3.1.5 HydroShift LCD (HS I)

**Source folder:** `L-Connect.Core/LConnectCore/Products/HydroShiftLCD/`

**Marketed name(s):** Three retail SKUs share the same `HydroShiftLCDDevice` class and `Commands` enum, distinguished only by USB PID:

| ControllerTypeID    | PID (decimal) | PID (hex) | Likely retail SKU |
|---------------------|---------------|-----------|-------------------|
| `HydroShiftLCD`     | 29592 | `0x7398` | HydroShift LCD (base) |
| `HydroShiftLCDRGB`  | 29593 | `0x7399` | HydroShift LCD RGB |
| `HydroShiftLCDTL`   | 29594 | `0x739A` | HydroShift LCD TL |

**ControllerTypeID:** `HydroShiftLCD = 16850944`, `HydroShiftLCDRGB = 16850945`, `HydroShiftLCDTL = 16850946` (`KnownDevices.cs:43,45,47`; `ControllerTypeID.cs:20-22`).

**USB VID:PID:**

| ControllerTypeID    | VID:PID (hex)     |
|---------------------|-------------------|
| `HydroShiftLCD`     | `0x0416:0x7398`   |
| `HydroShiftLCDRGB`  | `0x0416:0x7399`   |
| `HydroShiftLCDTL`   | `0x0416:0x739A`   |

VID `0x0416` is Winbond/Nuvoton (same MCU vendor used for Galahad II LCDV2).

**Framing classes used:** all three of the standard Lian-Li L-Connect framings (A, B, C). Layouts are byte-for-byte identical to the Galahad II LCDV2 baseline. Source files: `HydroShiftLCD/ACommandPacket.cs:12-130`, `HydroShiftLCD/BCommandPacket.cs:12-189`, `HydroShiftLCD/CCommandPacket.cs:12-189`.

**Status:** all entries `inferred-static` - Phase 0e is a static-source survey and no live HS-I hardware run has been performed.

**Divergence vs LCDV2 framing:** the HS-I device class **explicitly exposes** the C-class H.264 path via `SendH264Frame512` (`HydroShiftLCD/HydroShiftLCDDevice.cs:252-257`). On LCDV2, the parallel `Galahad2VisionDevice.SendH264Frame512` exists but the corresponding CCommandPacket codec is currently deferred in the Linux port. HS I treats both B and C framings as first-class for cmd 13.

**Subsystem fingerprint:**

| Subsystem            | Present? | Evidence |
|----------------------|----------|----------|
| Pump PWM             | inferred-static | `HydroShiftLCD/HydroShiftLCDDevice.cs:290-297` (cmd 138) |
| Fan PWM              | inferred-static | `HydroShiftLCD/HydroShiftLCDDevice.cs:264-271` (cmd 139) |
| Pump RPM read        | inferred-static | `HydroShiftLCD/HandshakeInfo.cs:69-74` |
| Fan RPM read         | inferred-static | `HydroShiftLCD/HandshakeInfo.cs:69-74` |
| Coolant temperature  | inferred-static | `HydroShiftLCD/HandshakeInfo.cs:71-74` |
| Pump-cap LCD         | inferred-static | `HydroShiftLCD/Constants.cs:43-47` (480x480 @ 24 fps); `IsLCDAvailable` cmd 23 |
| Fan-ring lighting    | inferred-static | `HydroShiftLCD/Commands.cs:21-22` (132/133); 16 modes |
| Per-LED RGB control  | inferred-static | `HydroShiftLCD/HydroShiftLCDDevice.cs:273-288` (cmd 20) |
| **Pump-cap dedicated lighting** (cmds 130/131) | **NO** | `HydroShiftLCD/Commands.cs` has no entries for 130/131 |

##### Commands

All command bytes from `HydroShiftLCD/Commands.cs:7-27`. Framing inferred from `HydroShiftLCD/HydroShiftLCDDevice.cs` write helpers.

| Cmd byte | Hex | Name | Framing | Source (HS I) | Status | Notes |
|----:|----:|------|:-------:|---------------|:------:|-------|
| 10 | `0x0A` | `ActiveFirmware`         | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:55-71` | inferred-static | Sends `[0x01]` payload. |
| 11 | `0x0B` | `UploadFirmware`         | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:299-304` | inferred-static | **Destructive**. |
| 12 | `0x0C` | `LCDControl`             | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:173-178` | inferred-static | 8-byte payload. Identical to LCDV2. |
| 13 | `0x0D` | `SendH264Frame`          | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:245-250` | inferred-static | Raw H.264 frame. |
| 13 | `0x0D` | `SendH264Frame512`       | C | `HydroShiftLCD/HydroShiftLCDDevice.cs:252-257` | inferred-static | Same cmd byte, demuxed by ReportID. |
| 14 | `0x0E` | `SendJpeg`               | B | enum-only at `HydroShiftLCD/Commands.cs:11`; **no call site** | inferred-static | |
| 15 | `0x0F` | `UploadJpeg`             | B | enum-only at `HydroShiftLCD/Commands.cs:12`; **no call site** | unclear | |
| 18 | `0x12` | `UploadAvi`              | B | enum-only at `HydroShiftLCD/Commands.cs:13` | unclear | |
| 19 | `0x13` | `UploadH264`             | B | enum-only at `HydroShiftLCD/Commands.cs:14` | unclear | |
| 20 | `0x14` | `SetLEDColors`           | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:273-288` | inferred-static | Payload = `[0x01]` flag byte + RGB triples. **Differs from LCDV2** - no `isFanLed` branch, no 24-byte zero-pad. |
| 21 | `0x15` | `UploadH264Info`         | B | enum-only at `HydroShiftLCD/Commands.cs:16` | unclear | |
| 22 | `0x16` | `UploadStartupAviMovie`  | B | enum-only at `HydroShiftLCD/Commands.cs:17` | unclear | **Name explicitly says "startup"** - strong cmd-15 sibling hypothesis. |
| 23 | `0x17` | `IsLCDAvailable`         | B | `HydroShiftLCD/HydroShiftLCDDevice.cs:160-171` | inferred-static | Counterintuitive zero-means-yes. |
| 129 | `0x81` | `GetHandshakeInfo`      | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:107-120, 240-243` | inferred-static | Same 7-byte response shape as LCDV2. |
| 132 | `0x84` | `GetFanLightingInfo`    | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:73-86` | inferred-static | Standard escape-byte protocol. |
| 133 | `0x85` | `SetFanLighting`        | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:259-262` | inferred-static | 20-byte payload. Identical to LCDV2 cmd 133. |
| 134 | `0x86` | `GetFirmwareVersion`    | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:88-105` | inferred-static | Two A-packets. |
| 136 | `0x88` | `GetSerialNumber`        | A | enum-only at `HydroShiftLCD/Commands.cs:23` | unclear | |
| 137 | `0x89` | `SetSerialNumber`        | A | enum-only at `HydroShiftLCD/Commands.cs:24` | unclear | Destructive. |
| 138 | `0x8A` | `SetPumpPWM`             | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:290-297` | inferred-static | 2-byte payload `[source, percent]`. |
| 139 | `0x8B` | `SetFanPWM`              | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:264-271` | inferred-static | 2-byte payload `[0x00, percent]`. |
| 142 | `0x8E` | `ResetDevice`            | A | `HydroShiftLCD/HydroShiftLCDDevice.cs:223-238` | inferred-static | **Destructive**. |

> **Conspicuous absence vs LCDV2:** HS I's `Commands.cs` has no `GetPumpLightingInfo` (130) and no `SetPumpLighting` (131).

##### Subsystem patterns

- **Handshake (cmd 129)** identical to LCDV2 (7 bytes: FanRpm/PumpRpm/TempIsValid/TempInteger/TempFraction).
- **Pump RPM ranges** - two ranges advertised: 2200..=3800 RPM and 2000..=3200 RPM (`HydroShiftLCD/Constants.cs:15-21`).
- **LCD geometry** - 480x480 @ 24 fps, identical to LCDV2.
- **Fan lighting modes** - 16 modes, byte values 1..=16 (full set, vs LCDV2's 6-mode fan-side subset).
- **`LCDControlMode`** discriminants identical to LCDV2 (LocalUI=0, Application=1, LocalH264=2, LocalAVI=3, LCDSetting=4, LCDTest=5).
- **`cmd 20 SetLEDColors`** - HS I always writes flag `0x01` (no `isFanLed` branch). No 24-byte zero-pad region.
- **Multi-packet chunking** mirrors LCDV2.
- **Handshake-loop event** suggests firmware emits unsolicited handshake packets.
- **Write-abort retry** identical to LCDV2.

##### Notable divergences from LCDV2

1. **No pump-lighting commands (130/131).**
2. **Fan-side mode set is the full 16 modes**, not the LCDV2 6-mode subset.
3. **cmd 20 wire format differs** - HS I always writes flag = `0x01`, no `isFanLed` branch, no 24-byte zero-pad.
4. **C-class H.264 path is first-class** via `SendH264Frame512`.
5. **Two pump RPM ranges** in `HydroShiftLCD/Constants.cs:15-21`.
6. **Default `VideoFPS` is 30**, not LCDV2 service's 24.
7. **Three sibling PIDs share one device class.**

##### Open questions

- **cmd-15 hypothesis from HS I - `UploadStartupAviMovie = 22`** strongly implies the family writes to non-volatile storage for boot-time / "until-overwritten" display. By analogy, cmd 15 (`UploadJpeg`) is plausibly the JPEG sibling - a stored / boot / power-on splash JPEG.
- No HS-I-specific "activate stored image" command found beyond shared enum.
- Does HS I actually accept cmd 15? No call site for cmd 15 in `HydroShiftLCD/HydroShiftLCDDevice.cs`.
- `SyncScreenImage` (RequestType index 2) wire-byte mapping not pinned in the device class.
- Pump-cap LED ring addressing on HS I (with no cmd 130/131): plausibly cmd 20 with the flat-flag layout addresses both, or cmd 133 with `SyncToPump = true` cascades.
- Cmd 13 dual-framing - HS I explicitly exposes the C-class path, suggesting firmware preference for higher-throughput streaming.

#### 3.1.6 HydroShift II LCD (HS II)

**Source folder:** `L-Connect.Core/LConnectCore/Products/HydroShiftIILCD/` plus `Requests/`. Service-side controller at `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/HydroShiftIILCDController.cs`.

**Marketed names:** HydroShift II LCD-C (Circle) and HydroShift II LCD-S (Square).

**ControllerTypeID enum value(s):** `HydroShiftIILCDCircle = 16986112`, `HydroShiftIILCDCircleDisplayMode = 16986113`, `HydroShiftIILCDSquare = 16986128`, `HydroShiftIILCDSquareDisplayMode = 16986129` (`ControllerTypeID.cs:31-34`), plus receivers `HydroShiftIILCDCircleReceiver = 50540546`, `HydroShiftIILCDSquareReceiver = 50540562` (`ControllerTypeID.cs:40-41`).

**VID:PID per SKU** (`KnownDevices.cs:65-71`):

| ControllerTypeID                    | VID:PID (hex)     |
|-------------------------------------|-------------------|
| `HydroShiftIILCDCircle`             | `0x1CBE:0xA021`   |
| `HydroShiftIILCDCircleDisplayMode`  | `0x1A86:0xAD20`   |
| `HydroShiftIILCDSquare`             | `0x1CBE:0xA034`   |
| `HydroShiftIILCDSquareDisplayMode`  | `0x1A86:0xAD22`   |

VID `0x1CBE` is Lian-Li's own assigned USB-IF VID; VID `0x1A86` is QinHeng / WCH (used for the alternate display-mode interface).

**Framing classes:** **WinUSB+DES envelope**, owned by `lcd207.dll` (per the lcd207-sdk survey - see section 3.2.2). The HS II per-family staging file's original "in slv3.dll" claim has been **superseded**: HS II's wired wire format is in `lcd207.dll` via `H2Controller.cs` / `H2SController.cs` / `WinUsbH2.cs` / `WinUsbH2S.cs`, NOT slv3, and **HS II uses WinUSB, not HID, for frames**. HID is only used out-of-band for SN discovery and `SetMonitorMode` magic-byte signalling.

**Status:** all entries `inferred-static` - no live HS II hardware run has been performed.

**Subsystem fingerprint** (per `RequestType.cs:6-69` + dispatch table at `HydroShiftIILCDController.cs:2845-3138`):

| Subsystem            | Present | Evidence (RequestType entries) |
|----------------------|---------|-------------------------------------------|
| Pump PWM             | inferred-static | `SetPumpSpeed` (RequestType.cs:8) |
| Fan PWM              | inferred-static | `SetFanSpeed` (RequestType.cs:9) |
| ARGB lighting        | inferred-static | `SetLightingEffects` (RequestType.cs:10) |
| LCD on pump          | inferred-static | `SetLCDBrightness`, `SetLCDRotate`, `SetSecondaryScreen` (RequestType.cs:18-20) |
| Templates            | inferred-static | 11 entries, RequestType.cs:32-44 |
| Custom themes        | inferred-static | RequestType.cs:45-56 |
| Modular widgets      | inferred-static | 11 entries, RequestType.cs:57-66 |
| Wireless integration | inferred-static | RequestType.cs:12-14 |
| Offline mode clock   | inferred-static | RequestType.cs:16-17 |
| Profile management   | inferred-static | RequestType.cs:15,29-31 |
| Firmware update      | deferred | RequestType.cs:24-26 |

##### Commands table (host-side `RequestType`, not wire bytes)

Because there is **no wire-byte enum** in the open source, the table below lists `HydroShiftIILCD/RequestType.cs:7-69` operation names with the ordinal value they take in the C# enum. All entries have an opaque framing.

| Ord | RequestType                          | Direction | Status | Notes (file:line) |
|-----|--------------------------------------|-----------|--------|-------------------|
| 0   | `GetHandshakeInfo`                   | read      | inferred-static | `HydroShiftIILCD/RequestType.cs:7` |
| 1   | `SetPumpSpeed`                       | write     | inferred-static | Range Circle 1600-2450 / Square 1600-3200 RPM |
| 2   | `SetFanSpeed`                        | write     | inferred-static | CL120 200-2150 RPM |
| 3   | `SetLightingEffects`                 | write     | inferred-static | `SyncEffects` 10-mode set |
| 4   | `CheckAdvanceMode`                   | read      | inferred-static | |
| 5   | `WirelessControllerDispose`          | write     | inferred-static | |
| 6   | `WiredDispose`                       | write     | inferred-static | |
| 7   | `PlayWirelessContents`               | write     | inferred-static | |
| 8   | `SaveProfile`                        | write     | inferred-static | |
| 9   | `GetOfflineModeClock`                | read      | inferred-static | |
| 10  | `SetOfflineModeClock`                | write     | inferred-static | |
| 11  | `SetSecondaryScreen`                 | write     | inferred-static | SDK `SetDesktopMode` / `SetMonitorMode` |
| 12  | `SetLCDBrightness`                   | write     | inferred-static | 0-100 |
| 13  | `SetLCDRotate`                       | write     | inferred-static | RotateType 0/90/180/270 |
| 14  | `GetIsAvailable`                     | read      | inferred-static | |
| 15  | `SyncScreenTopLayerImage`            | write     | inferred-static | shared-memory `(Address, ProcessId)` - **see cmd-15 hypothesis** |
| 16  | `GetPreviewBackground`               | read      | inferred-static | |
| 17  | `GetFirmwareVersion`                 | read      | inferred-static | |
| 18  | `StartFirmwareUpdate`                | write     | deferred | **firmware path - deferred per Phase 1 scope** |
| 19  | `CheckFirmwareUpdateStatus`          | read      | deferred | |
| 20  | `ReloadAssets`                       | write     | inferred-static | |
| 21  | `StopScreenRender`                   | write     | inferred-static | SDK `PlayStop()` |
| 22  | `ExportProfile`                      | read      | inferred-static | |
| 23  | `ImportProfile`                      | write     | inferred-static | |
| 24  | `ResetProfile`                       | write     | inferred-static | |
| 25  | `GetSelectedTemplateId`              | read      | inferred-static | |
| 26  | `GetTemplates`                       | read      | inferred-static | |
| 27  | `ImportTemplate`                     | write     | inferred-static | |
| 28  | `DeleteTemplate`                     | write     | inferred-static | |
| 29  | `ApplyTemplate`                      | write     | inferred-static | **see cmd-15 hypothesis** |
| 30  | `ChangeTemplateBackground`           | write     | inferred-static | |
| 31  | `RevertTemplateBackground`           | write     | inferred-static | |
| 32  | `GetTemplateCategories`              | read      | inferred-static | |
| 33  | `AddTemplateCategory`                | write     | inferred-static | |
| 34  | `DeleteTemplateCategory`             | write     | inferred-static | |
| 35  | `RenameTemplateCategory`             | write     | inferred-static | |
| 36  | `GetAllTemplateAssignedCategories`   | read      | inferred-static | |
| 37  | `UpdateTemplateAssignedCategories`   | write     | inferred-static | |
| 38  | `GetCustomThemeImageVideoFilePath`   | read      | inferred-static | |
| 39  | `SetCustomThemeImageVideoFilePath`   | write     | inferred-static | |
| 40  | `ApplyBackgroundOnlyCustomTheme`     | write     | inferred-static | **see cmd-15 hypothesis** |
| 41  | `GetThemeInfos`                      | read      | inferred-static | |
| 42  | `SetThemeInfoId`                     | write     | inferred-static | |
| 43  | `GetThemeInfoId`                     | read      | inferred-static | |
| 44  | `GetThemeInfoSetting`                | read      | inferred-static | |
| 45  | `UpdateThemeInfoSetting`             | write     | inferred-static | |
| 46  | `SetDefaultThemeInfoSetting`         | write     | inferred-static | |
| 47  | `ApplyThemeInfo`                     | write     | inferred-static | **see cmd-15 hypothesis** |
| 48  | `SyncThemeInfoImage`                 | write     | inferred-static | shared-memory pointer |
| 49  | `SaveThemeInfoToAsTemplate`          | write     | inferred-static | |
| 50  | `GetModulars`                        | read      | inferred-static | |
| 51  | `GetModularSetting`                  | read      | inferred-static | |
| 52  | `GetModularSettings`                 | read      | inferred-static | |
| 53  | `AddModularSetting`                  | write     | inferred-static | |
| 54  | `UpdateModularSetting`               | write     | inferred-static | |
| 55  | `SetDefaultModularSetting`           | write     | inferred-static | |
| 56  | `DeleteModularSetting`               | write     | inferred-static | |
| 57  | `ApplyModulars`                      | write     | inferred-static | **see cmd-15 hypothesis** |
| 58  | `GetModularImage`                    | read      | inferred-static | |
| 59  | `SaveModularsToAsTemplate`           | write     | inferred-static | |
| 60  | `StopContents`                       | write     | inferred-static | |
| 61  | `PlayContents`                       | write     | inferred-static | |

The two SKUs (Circle, Square) **share the same RequestType enum** - the SDK wrapper (`H2CSDK` vs `H2SSDK`) handles per-shape protocol differences internally.

##### Subsystem patterns

- **LCD constants:** 480x480 buffer, 24 fps; pump RPM ranges Circle 1600..2450 / Square 1600..3200; brightness 0..100.
- **Refresh timers:** `UpdatePumpSpeedInterval = 1000` ms; `UpdateHandshakeInfoInterval = 2000` ms; `CheckFirmwareVersionInterval = 2000` ms.
- **Frame buffers:** 921,600-byte (480x480x4 BGRA) pre-allocated buffer.
- **Lighting dispatch:** three effect-setting subclasses for on-pump LEDs, CL120 fan-ring LEDs, and a unified Sync set (10-mode).
- **Theme model:** four orthogonal layers (`ImageVideo`, `Color`, `ThemeInfo`, `Modulars`).
- **Display-mode toggle:** `SetSecondaryScreen(true)` -> SDK `SetDesktopMode()`; `false` -> SDK `SetMonitorMode()`.
- **Wireless / receiver pattern:** Direct HID to a Receiver is not used; mediated by L-Wireless transmitter.

##### Notable divergences from LCDV2

1. **No exposed wire-byte protocol from open C# source** - all HID I/O performed inside `lcd207.dll` (per section 3.2.2 / lcd207-sdk survey).
2. **Different MCU vendor.** LCDV2 = `0x0416`; HS II = `0x1CBE` (TI Stellaris) for HID + `0x1A86` (WCH) for display-mode.
3. **Two-PID-per-shape design.** Linux port will need to handle two distinct USB interfaces per device.
4. **High-level theme/template/modular system** - much closer to a TV than to a status LED.
5. **Different RPM ranges.**
6. **No `SettingType` enum** - flattens everything into a single 60+ entry `RequestType`.
7. **No `Commands.cs` byte map** in the per-family folder.
8. **In-process shared-memory image transfer** for `SyncScreenTopLayerImage` / `SyncThemeInfoImage` using `(long Address, int ProcessId)` tuples - Windows IPC, not over HID.

##### Cmd-15 hypothesis: HS II findings

Resolved via the lcd207-sdk survey (see section 3.2.2): HS II's wired LCD pipeline uses `CmdType.PushJpg = 0x65` (transient JPG) and `CmdType.UpdateFireWare = 0x28` with filename `/usr/data/boot.jpg` (stored boot logo, same opcode as firmware update distinguished by filename).

The high-level "activate stored content" RequestType verbs (`ApplyTemplate`, `ApplyThemeInfo`, `ApplyModulars`, `ApplyBackgroundOnlyCustomTheme`) drive the wire-level mix of `PushJpg` + `UpdateFireWare` calls inside `lcd207.dll`. The `Sync*` (transient sync) vs `Apply*` (persistent apply) host-side distinction does not map to a single transient-vs-stored opcode pair on the wire - it maps to a filename routing inside the shared `UpdateFireWare = 0x28` opcode.

##### Open questions

1. cmd-15 hypothesis from HS II resolved by lcd207-sdk survey (transient = `PushJpg=0x65`, stored = `UpdateFireWare=0x28` with filename).
2. Receiver firmware version meaning - hard-coded `Math.Max(rawValue, 11) - 2` (Circle) / `18) - 10` (Square) thresholds.
3. Two-PID switching mechanics - whether SDK `SetDesktopMode()` triggers USB bus-reset or out-of-band command channel.

#### 3.1.7 TLLCD

**Source folder:** `L-Connect.Core/LConnectCore/Products/TLLCD/`

**Marketed name:** TL-LCD fan (the screen module on the Lian-Li TL-LCD fan series, generation 1).

**ControllerTypeID / ControllerClass:**
- `ControllerTypeID.TLLCD = 16912384` - `LConnectCore/Devices/ControllerTypeID.cs:24`
- `ControllerClass.TLLCD = 33` - `LConnectCore/Devices/ControllerClass.cs:18`
- `ControllerTypeIDDefinitions.ClassTLLCD = 135168` - `LConnectCore/Devices/ControllerTypeIDDefinitions.cs:49`

**USB VID:PID:** `0x04FC:0x7393` (`KnownDevices.cs:51`). VID `0x04FC` is **Sunplus** - distinct from the Galahad II / HydroShift LCD families which all use Winbond/Nuvoton (`0x0416`).

**Framing classes used:** **single, family-specific framing class** (T-class). Source: `TLLCD/LCDPacket.cs:10-153`. ReportID=0x02, 512 out / 64 in (asymmetric), 11-byte header, 501-byte payload max. T-class is asymmetric in a way no LCDV2 class is - 512-byte writes but only 64-byte reads.

**Status:** all entries `inferred-static` - Phase 0e is a static-source survey.

##### Subsystem fingerprint

| Subsystem                    | Present? | Evidence |
|------------------------------|----------|----------|
| Fan PWM / RPM / lighting     | NO       | Lives on `TLFan` |
| **Per-fan LCD screen**       | yes      | Whole enum is LCD-centric; 400x400 |
| LCD on/off / brightness / rotation / mode | yes | cmd 64 `LCDControl` |
| LCD frame upload (transient JPEG) | yes | cmd 65 `WriteJPG` |
| LCD frame upload (synchronised JPEG) | yes | cmd 70 `WriteSYNCJPG` |
| LCD video upload (AVI)       | yes      | cmd 69 `WriteAVI` |
| LCD boot-image upload        | yes      | cmd 71 `WriteBootAVI`, cmd 72 `WriteBootJPG` |
| Sensor / handshake           | yes      | cmd 60 `GetHandshakeInfo` (3-byte resp) |
| Per-fan addressing (port + index) | yes | cmd 62 returns `Port` (0..3) and `Index` (0..2) |
| Set serial number            | yes      | cmd 63 `WriteSerialNumber` |
| Firmware version read        | yes      | cmd 61 `GetProductInfo` |
| Firmware activation / write  | declared-only | cmds 58/59 |
| Reset / soft-reset           | NO       | |
| Continuous re-issue timer    | unclear  | |

##### Commands

All command bytes from `TLLCD/LCDCommands.cs:5-19`.

| Cmd | Hex | Name | Framing | Direction | Source | Status | Notes |
|----:|----:|------|:-------:|:---------:|--------|:------:|-------|
| 58  | `0x3A` | `ActiveFirmware`  | T | host->dev   | enum-only at `TLLCD/LCDCommands.cs:7`; **no call site** | declared-only | Risky. |
| 59  | `0x3B` | `WriteFirmware`   | T | host->dev   | enum-only at `TLLCD/LCDCommands.cs:8`; **no call site** | declared-only | **Destructive**. |
| 60  | `0x3C` | `GetHandshakeInfo`| T | host<->dev  | `TLLCD/TLLCDDevice.cs:121-134, 262-265`; `TLLCD/HandshakeInfo.cs:29-44` | inferred-static | Bare request. Response: 3 bytes - `Mode` (u8), `FrameIndex` (BE u16). |
| 61  | `0x3D` | `GetProductInfo`  | T | host<->dev  | `TLLCD/TLLCDDevice.cs:100-119`; `TLLCD/FirmwareVersion.cs:71-135` | inferred-static | Two-packet response: ASCII version + ASCII datetime. |
| 62  | `0x3E` | `ReadSerialNumber`| T | host<->dev  | `TLLCD/TLLCDDevice.cs:146-174`; `TLLCD/SerialNumberInfo.cs:36-53` | inferred-static | **Per-fan addressing.** Response: 34 bytes - 32-byte ASCII serial + `Port` + `Index`. |
| 63  | `0x3F` | `WriteSerialNumber`| T | host->dev   | `TLLCD/TLLCDDevice.cs:282-290` | inferred-static | 32-byte payload. **Destructive on identity bytes.** |
| 64  | `0x40` | `LCDControl`      | T | host<->dev | `TLLCD/TLLCDDevice.cs:205-213`; `TLLCD/LCDControlSetting.cs:69-80` | inferred-static | 11-byte payload. |
| 65  | `0x41` | `WriteJPG`        | T | host<->dev | `TLLCD/TLLCDDevice.cs:267-270` | inferred-static | Raw JPEG; transient. |
| 69  | `0x45` | `WriteAVI`        | T | host<->dev | `TLLCD/TLLCDDevice.cs:247-250` | inferred-static | Raw AVI; transient streaming. |
| 70  | `0x46` | `WriteSYNCJPG`    | T | host->dev | `TLLCD/TLLCDDevice.cs:272-275` | inferred-static | Raw JPEG; chunked. **`read=false`** - no response per chunk. |
| 71  | `0x47` | `WriteBootAVI`    | T | host<->dev | `TLLCD/TLLCDDevice.cs:257-260` | inferred-static | **Stored boot AVI.** |
| 72  | `0x48` | `WriteBootJPG`    | T | host<->dev | `TLLCD/TLLCDDevice.cs:252-255` | inferred-static | **Stored boot JPEG.** |

##### Subsystem patterns

- **Per-fan addressing** resolved at device-open time via cmd 62. Up to 12 fan screens (4 ports x 3 fans/port).
- **Handshake (cmd 60)** is 3 bytes (Mode + FrameIndex) - much smaller than LCDV2's 7 bytes.
- **LCD control (cmd 64)** uses 11 bytes including a `JPGIndex` BE u16 selecting stored JPEG slot.
- **`LCDControlMode` divergent from LCDV2**: byte 2 is **skipped** (no `LocalH264`). Includes `ShowJPG=1` mode strongly suggestive of stored-JPEG slots selected by `JPGIndex`.
- **Send vs Write naming inconsistency** in C# layer (enum says `Write*`, methods named `Send*`).
- **Firmware read (cmd 61)** mirrors LCDV2 cmd 134 in two-packet ASCII shape, different byte/framing.
- **Sensor display (LCD-side)** - 5 sensor types rendered host-side and pushed as JPEG/AVI frames.

##### Notable divergences from LCDV2 baseline

1. **Different MCU vendor** - VID `0x04FC` (Sunplus) vs LCDV2's `0x0416` (Winbond).
2. **Single framing class, not three.**
3. **Per-fan addressing scheme.**
4. **No fan/pump/lighting subsystem** in this enum.
5. **No reset command.**
6. **No H.264 path.**
7. **Two-packet firmware response uses cmd 61, not LCDV2's cmd 134.**
8. **Handshake is 3 bytes, not 7.**
9. **Stored-image subsystem is explicit** (cmds 71/72 + `JPGIndex`).

##### Cmd-15 hypothesis cross-check

**TLLCD evidence supports the hypothesis:** the `Send*`/`Write*` transient pair (cmd 65 `WriteJPG`, cmd 69 `WriteAVI`) and the `WriteBoot*` stored pair (cmd 71 `WriteBootAVI`, cmd 72 `WriteBootJPG`) form a clean symmetry. This validates the LCDV2 cmd 14 (transient) vs cmd 15 (stored) hypothesis structurally.

Importantly, TLLCD also exposes a **stored-slot selection mechanism** via the `JPGIndex` field in `LCDControlSetting` (`TLLCD/LCDControlSetting.cs:23-28`). The activation flow appears to be: (1) upload via cmd 72 `WriteBootJPG`; (2) activate via cmd 64 with `Mode=ShowJPG` and `JPGIndex=N`. LCDV2's `LCDControlSetting` lacks a `JPGIndex` field, which weakens the analogy on the LCDV2 side.

##### Open questions

1. Stored-slot capacity - `JPGIndex` is 16-bit but no slot count advertised.
2. `Mode` byte in handshake response value space undefined in source.
3. `FrameIndex` semantics - likely AVI playback position counter.
4. Cmd 70 `WriteSYNCJPG` actual sync semantics - hub-side broadcast vs targeted.
5. Cmds 58/59 firmware-flash framing details.
6. Per-fan vs per-hub command targeting - whether each fan exposes its own HID interface or single hub auto-routes.
7. Default `VideoFPS = 30` actual sustainable rate over 512/64 framing.
8. `LCDControlMode = 2` skipped wire value semantics.
9. Continuous re-issue / refresh requirement for cmd 65 frames.

#### 3.1.8 SLV3LCD

**Source folder:** Two related folders cooperate:
- `L-Connect.Core/LConnectCore/Products/SLV3LCD/` - upper-layer DTOs (settings, request envelopes, media converter)
- `L-Connect.Core/LConnectCore/Products/LWireless/SLV3LCD/SLV3LCDFirmwareVersion.cs` - firmware-version parsing only

Service-side: `L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs`.

**Marketed name:** Lian-Li SL-Wireless V3 LCD fan. 120 mm and 140 mm variants with per-variant RPM ranges (`LConnectCore/Products/LWireless/Constants.cs:35-41`).

**ControllerTypeID / ControllerClass:**
- `ControllerTypeID.SLV3LCD = 16977920` - `LConnectCore/Devices/ControllerTypeID.cs:29` (hex `0x01032000`)
- `ControllerClass.SLV3LCD = 49` - `LConnectCore/Devices/ControllerClass.cs:20`
- `ControllerTypeIDDefinitions.ClassSLV3LCD = 200704` - `LConnectCore/Devices/ControllerTypeIDDefinitions.cs:53`

**USB VID:PID:** `0x1CBE:0x0005` (decimal `7358:5`) - `LConnectCore/Devices/KnownDevices.cs:61`. VID `0x1CBE` is **Lian-Li's own assigned USB-IF VID**.

**Framing classes used:** **opaque - JSON-over-IPC + slv3 SDK passthrough.** The C# layer here builds JSON over the L-Connect service IPC, not HID packets. All wire-level protocol work happens inside the L-Connect-Service process via the proprietary native `slv3.dll` SDK (resolved by section 3.2.1).

**Status:** all entries `inferred-static` (or `unclear` where service-side has no handler) - SLV3LCD live runs are blocked at the C# layer because the wire protocol lives inside `slv3.dll`.

##### Subsystem fingerprint

| Subsystem                    | Present? | Evidence |
|------------------------------|----------|----------|
| Fan PWM                      | inferred-static (indirect) | `LWirelessDevice.SetFanSpeed` -> `RFController.SetFansRPM` |
| Fan RPM read                 | inferred-static | cmd 17 `GetRPM` |
| Fan-ring lighting            | (separate enum in `LConnectCore/Products/LWireless/SLV3/`) | |
| **Per-fan LCD screen**       | inferred-static | 400x400 px / 30 FPS / 4 ports x 3 fans (`SLV3LCD/Constants.cs:11-19`) |
| LCD on/off                   | unclear | No explicit "off" cmd |
| LCD brightness               | inferred-static | cmd 8 `SetBrightness` - **discrete LUT** |
| LCD rotation (per-screen)    | inferred-static | cmd 7 `SetRotate` |
| LCD rotation (all screens)   | inferred-static | cmd 9 `SetRotateAllScreen` |
| LCD image upload             | inferred-static | cmd 5 `SetPicture`, cmd 18 `PictureSyncToAll` |
| LCD video / GIF playback     | inferred-static | cmds 6, 19, 12 |
| Sensor display (LCD-side)    | inferred-static | cmd 3 `SetSensor`, cmd 20 `SensorSyncToAll`; 1000 ms refresh timer |
| Default (logo) image         | inferred-static | cmd 4 `SetDefaultPic` |
| Virtual group / multi-screen | inferred-static | cmds 10, 11, 12 |
| Group enumeration            | inferred-static | cmds 0, 1, 2 |
| File deletion                | inferred-static | cmd 13 `DeleteFile` |
| Restart / reset              | inferred-static | cmd 16 `RestartLCD` - **destructive across whole LCD group** |
| Firmware version read        | inferred-static | `LWirelessRequestType.GetLCDFirmwareVersion = 0` (separate dispatch enum) |
| Firmware upgrade             | inferred-static | `LWirelessRequestType.StartSLV3LCDUpgrade = 1`, etc. |
| Continuous re-issue timer    | inferred-static (sensor only) | 1 Hz |

##### Commands

The "commands" surveyed here are **service-IPC dispatch keys**, not HID wire bytes. All values from `LConnectCore/Products/SLV3LCD/RequestType.cs:5-28`. Handler line numbers from `L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs`.

| Cmd | Name                    | Service handler         | Status | Notes |
|----:|-------------------------|-------------------------|:------:|-------|
| 0   | `RefreshGroupInfo`      | `handlerRefreshGroupInfo` | inferred-static | `STRController.RefreshSTRPosInfo(0)` |
| 1   | `GetGroupCount`         | `handleGetGroupCount`     | inferred-static | |
| 2   | `GetControllers`        | `handleGetControllers`    | inferred-static | |
| 3   | `SetSensor`             | `handlerSetLCDSensor`     | inferred-static | |
| 4   | `SetDefaultPic`         | `handleSetDefaultPic`     | inferred-static | "Logo" / fallback image. |
| 5   | `SetPicture`            | `handleSetPicture`        | inferred-static | |
| 6   | `SetVideo`              | `handleSetVideo`          | inferred-static | |
| 7   | `SetRotate`             | `handleSetRotate`         | inferred-static | |
| 8   | `SetBrightness`         | `handleSetBrightness`     | inferred-static | **Discrete LUT** of 5 client values -> 5 hardware levels. |
| 9   | `SetRotateAllScreen`    | `handleSetRotateAllScreen`| inferred-static | |
| 10  | `SetVirtualGroup`       | `handleSetVirtualGroup`   | inferred-static | |
| 11  | `UngroupVirtualGroup`   | `handleUngroupVirtualGroup` | inferred-static | |
| 12  | `PlayVirtualGroupVideo` | `handlePlayVirtualGroupVideo` | inferred-static | |
| 13  | `DeleteFile`            | `handleDeleteFile`        | inferred-static | |
| 14  | `SyncToAllLCD`          | **(no-op)**               | unclear | `case 14: case 15: { all = null; break; }` |
| 15  | `GetSelectedController` | **(no-op)**               | unclear | Same case as 14. |
| 16  | `RestartLCD`            | `handleRestartLCD`        | inferred-static | **Destructive**. |
| 17  | `GetRPM`                | `handleGetRPM`            | inferred-static | |
| 18  | `PictureSyncToAll`      | `handlePictureSyncToAll`  | inferred-static | |
| 19  | `VideoSyncToAll`        | `handleVideoSyncToAll`    | inferred-static | |
| 20  | `SensorSyncToAll`       | `handlerSensorSyncToAllLCD` | inferred-static | |

Adjacent firmware-update enum (separate dispatch, `LConnectCore/LWirelessRequestType.cs:5-12`):

| Cmd | Name                          | Status | Notes |
|----:|-------------------------------|:------:|-------|
| 0   | `GetLCDFirmwareVersion`       | inferred-static | Regex `^slv3_\d{8}(\d{2})(?:_\d{2})?$`. |
| 1   | `StartSLV3LCDUpgrade`         | inferred-static | `WirelessLCDUpgradeRequest`. |
| 2   | `GetSLV3LCDUpgradeProgress`   | inferred-static | |
| 3   | `StartTLV2LCDUpgrade`         | inferred-static | Sibling for TLV2LCD. |
| 4   | `GetTLV2LCDUpgradeProgress`   | inferred-static | Sibling for TLV2LCD. |

##### Subsystem patterns

- **IPC envelope:** GET-style query string with `devicePath` (Base64 of HID device path) and `type` (the `RequestType` enum name as ASCII). Body is JSON-serialised.
- **Discrete brightness LUT:** client 0/25/50/75/100 -> hardware 0/10/30/40/100; any other value falls through to 30.
- **Sensor refresh timer:** 1 Hz with `SetLCDValueDelay = 200` ms debounce.
- **Virtual group flow:** `SetVirtualGroup` (cmd 10) merges screens; `PlayVirtualGroupVideo` (cmd 12) targets the merged group.
- **Image pipeline:** `LWirelessLCDMediaConverter.SaveImage` resizes to `(400, 400 * screenCount)`.

##### Notable divergences from LCDV2 / TLLCD baselines

1. **No HID-level wire format visible from C# source** - largest single divergence in the family survey. Resolved by section 3.2.1 (slv3-sdk).
2. **Indirected via Lian-Li RF SDK** (`RFController` + `STRController`).
3. **Brightness is a 5-point LUT, not linear.**
4. **Per-fan addressing uses `(GroupIndex, PosIndex)`, not `(Port, Index)`.**
5. **No "transient vs stored" command pair visible at this layer** - resolved at slv3-sdk level (`PushJpg=0x65` transient vs `UpdateFireWare=0x28` with filename `/usr/data/boot.jpg` stored).
6. **Two enum entries with no handler** (cmds 14, 15).
7. **Sensor refresh runs at 1 Hz on the host.**
8. **Restart / reboot is destructive across the whole LCD group.**
9. **Firmware update is two-stage with explicit progress poll.**

##### Cmd-15 hypothesis cross-check

Resolved at the slv3-sdk layer (section 3.2.1): wired SLV3LCD path uses transient = `CmdType.PushJpg = 0x65` and stored = `CmdType.UpdateFireWare = 0x28` with target filename `/usr/data/boot.jpg`. The 5 s-of-inactivity heuristic in `STRController.ShowAndSaveImg` triggers the auto-store path; the user does not explicitly choose to save.

##### Wired-vs-wireless relationship

The wired SLV3LCD device path (VID:PID `0x1CBE:0x0005`, ControllerTypeID 16977920) and a wireless transmitter device path both feed into the same `STRController.GetAllControlDevice` enumeration. The SDK abstracts wired-vs-wireless underneath - the C# layer sees one logical fleet of LCD screens indexed by SN and GroupIndex.

##### Open questions

1. Wire byte format for every command - resolved by section 3.2.1 for wired path.
2. Cmd 14 / cmd 15 unhandled service-side - possibly client-side broadcast or vestigial.
3. `Sync` field on `LWirelessLCDScreenSetting` - no caller sets or reads it.
4. `TargetSizeInBytes = 65535` declared but unused in `LWirelessLCDMediaConverter.cs:17`.
5. Sensor type 6 always returns 500 - identifying meaning unclear.
6. `handleSetBrightness` falls through to 30 for non-canonical inputs.
7. MD5 dedup semantics for upload - SDK-side consumption invisible.
8. Firmware version regex offset `+9` opaque.
9. Per-fan vs per-group command targeting demultiplexer is inside `slv3.dll`.
10. Discovery flow / scan-bind sequence handling for wired vs wireless.

#### 3.1.9 UniversalScreen 8.8"

**Source folder:** `LConnectCore/Products/UniversalScreen8p8Inch/` (codec/data) and `LConnectService/Products/UniversalScreen8p8Inch/UniversalScreen8p8InchController.cs` (transport via SDK).

**Marketed name(s):** Lian Li Universal Screen 8.8" - large external 1920x480 RGBA LCD module with surrounding addressable RGB ring; ships with companion lighting controller and (in some configurations) a CH340-class USB-to-serial display-mode pass-through.

**ControllerTypeID enum value(s):** three entries - `UniversalScreen8p8Inch = 17043456`, `UniversalScreen8p8InchDisplayMode = 17043457`, `UniversalScreen8p8InchLighting = 17043458` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:37-39`).

**USB VID:PIDs (three sibling endpoints, one logical product):**
- Main display path: `0x1CBE:0xA088` (decimal `7358:41096`) - `KnownDevices.cs:77`. Vendor `0x1CBE` is Lian-Li.
- Display-mode (UART bridge): `0x1A86:0xAD21` (decimal `6790:44321`) - `KnownDevices.cs:79`. Vendor `0x1A86` is **WCH/QinHeng (CH340)**.
- Lighting board: `0x0416:0x8050` (decimal `1046:32848`) - `KnownDevices.cs:81`. Same Nuvoton/Winbond VID as the Galahad II family lighting MCUs. **OpenRGB `LianLiUniversalScreenController` matches this PID exactly** (cross-survey section 5.3).

Only the main `0x1CBE:0xA088` PID is registered for WMI auto-discovery (`UniversalScreen8p8Inch/USBDeviceFinder.cs:23`); the two siblings are bound dynamically by the SDK.

**Framing classes used:** **family-specific**:
- The L-Connect3 source does not construct USB packet bytes directly. Per the lcd207-sdk survey (section 3.2.2), the **panel transport (`0x1CBE:0xA088`) is in `lcd207.dll`** via `LEDController` and `WinUsbLed` (the LCD-side endpoint inherits the same WinUSB+DES envelope as Lancool 207 / HS II). The per-family staging file's original "in slv3.dll" claim has been **superseded**.
- The **LED-side endpoint (`0x0416:0x8050`)** uses unencrypted raw 64-byte WinUSB frames per `WinUsbLed.cs` (lcd207-sdk). OpenRGB has independently reverse-engineered this path: bulk EP1, 64-byte chunks with `[0x11, offset, 0, 0, RGB...]` header, 20 LEDs / chunk, RGB byte order.
- The **CH340 UART bridge (`0x1A86:0xAD21`)** is unmapped in either SDK survey; its command set is unknown.

**Phase 0d status:** static-analysis only. Not in lconnect-linux scope until Phase 1+.

**Status:** all entries `inferred-static` unless otherwise marked.

##### Subsystem fingerprint

- Sensors / handshake (device-side): no - host pushes sensor values to SDK via `UpdateSensorValues(dictionary)`.
- Serial number / identification: `GetScreenSerialNumber` and `GetLightingSerialNumber` (separate request types).
- Firmware version: `GetFirmwareVersion` returns string `lianli88_NNNN_NNNN`. Separate `LightingFirmwareVersion` for the lighting MCU.
- Firmware update: `StartFirmwareUpdate` + `CheckFirmwareUpdateStatus` (display-mode-only).
- Fan PWM / pump PWM: N/A.
- LCD on/off / brightness / rotation: `inferred-static` - `SetScreenBrightness`, `SetScreenOrientation`, `SetScreenRotation`.
- LCD frame upload: yes via SDK render pipeline (1920x480 BGRA, 24 fps); not as discrete protocol commands.
- Lighting (preset effect modes): `inferred-static` - 19 modes in `UniversalScreen8p8Inch/UniversalScreen8p8InchLightingMode.cs:5-26`. Brightness rescaled host-side `0..100 -> 0..255`; speed rescaled `0..100 -> 7..3` (inverted).
- Lighting (per-LED RGB): not exposed as a command in L-Connect3; OpenRGB-confirmed `0x11`-header bulk packets on EP1.
- Continuous re-issue / polling pattern: yes - 1 Hz serial-number readers; SDK-internal 24 fps render.
- Reset / soft-reset: `ResetProfile` is host-side only; no on-device reset exposed.
- Display-mode SKU vs main SKU gated by `isDisplayMode` boolean.

##### Commands

The L-Connect3 surface for this family is a 66-entry `UniversalScreen8p8InchRequestType` enum routed through L-Connect-Service IPC. All entries are `inferred-static`; framing is `lcd207-sdk` (panel) / `slv3-sdk` (lighting LED) / unknown (CH340 UART) per the SDK surveys.

The Req # column reflects the actual ordinals from `UniversalScreen8p8InchRequestType.cs` (re-verified against source for this synthesis).

| Req # | Name | Direction | SDK call (or kind) | Citation | Notes |
|---|---|---|---|---|---|
| 0 | `GetScreenSerialNumber` | host<-dev | SDK `get_SN` (display panel) | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:7` | |
| 1 | `GetLightingSerialNumber` | host<-dev | SDK `get_ledUsb().get_SN` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:8` | |
| 2 | `SaveProfile` | host-only | host-side persistence | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:9` | |
| 3 | `GetFirmwareVersion` | host<-dev | SDK `get_Ver` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:10` | |
| 4 | `StartFirmwareUpdate` | host->dev | SDK `FirmwareUpdate(path)` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:11` | Display-mode-blocked. |
| 5 | `CheckFirmwareUpdateStatus` | host<-dev | SDK status read | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:12` | |
| 6 | `ReloadAssets` | host-only | filesystem copy | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:13` | |
| 7 | `ExportProfile` | host-only | JSON serialize | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:14` | |
| 8 | `ImportProfile` | host-only | JSON deserialize | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:15` | |
| 9 | `ResetProfile` | host-only | profile reset | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:16` | Not a hardware reset. |
| 10 | `GetLightingEffectSettings` | host-side | profile lookup | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:17` | |
| 11 | `GetLightingEffectSetting` | host-side | profile lookup | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:18` | |
| 12 | `GetLightingEffectMode` | host-side | profile lookup | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:19` | |
| 13 | `SetLightingEffectSetting` | host->dev | SDK `SetEffect(LightEffectSet, 0)` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:20` | **Lighting wire path: bulk EP1, 0x11-header chunks (cross-survey 5.3).** |
| 14 | `SetDefualtLightingEffectSetting` | host->dev | same as 13 with default args | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:21` | Typo `Defualt` is in source. |
| 15 | `StartScreenRender` | host->dev | SDK render-loop start | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:22` | |
| 16 | `StopScreenRender` | host->dev | SDK `StopEffect`/`PlayStop` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:23` | |
| 17 | `GetScreenName` | host-side | profile lookup | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:24` | Up to 16 chars. |
| 18 | `SetScreenName` | host->dev | SDK setter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:25` | |
| 19 | `GetOfflineModeClock` | host<-dev | SDK getter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:26` | |
| 20 | `SetOfflineModeClock` | host->dev | SDK `SetOfflineClock` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:27` | **Display-mode-only**. |
| 21 | `SetSecondaryScreen` | host->dev | SDK `SetDesktopMode`/`SetMonitorMode` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:28` | |
| 22 | `GetScreenBrightness` | host<-dev | SDK getter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:29` | 0..=100. |
| 23 | `SetScreenBrightness` | host->dev | SDK `SetBrightness` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:30` | |
| 24 | `GetScreenOrientation` | host<-dev | SDK getter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:31` | Landscape vs portrait. |
| 25 | `SetScreenOrientation` | host->dev | profile + `Rotate` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:32` | |
| 26 | `GetScreenRotation` | host<-dev | SDK getter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:33` | 180-deg flip flag. |
| 27 | `SetScreenRotation` | host->dev | SDK `Rotate(num)` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:34` | |
| 28 | `GetIsAvailable` | host<-dev | SDK `IsAvailable` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:35` | |
| 29 | `SyncScreenTopLayerImage` | host->host | `copyImageMemory` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:36` | **Cross-process IPC**. Buffer is 3,686,400 bytes (1920x480x4 BGRA). |
| 30 | `GetPreviewBackground` | host<-dev | SDK preview getter | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:37` | |
| 31 | `GetSelectedTemplateId` | host-only | profile lookup | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:38` | |
| 32 | `GetTemplates` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:39` | |
| 33 | `ImportTemplate` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:40` | |
| 34 | `DeleteTemplate` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:41` | |
| 35 | `ApplyTemplate` | host->dev | SDK `ApplyTemplate(theme)` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:42` | |
| 36 | `ChangeTemplateBackground` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:43` | |
| 37 | `RevertTemplateBackground` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:44` | |
| 38 | `GetTemplateCategories` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:45` | |
| 39 | `AddTemplateCategory` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:46` | |
| 40 | `DeleteTemplateCategory` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:47` | |
| 41 | `RenameTemplateCategory` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:48` | |
| 42 | `GetAllTemplateAssignedCategories` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:49` | |
| 43 | `UpdateTemplateAssignedCategories` | host-only | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:50` | |
| 44 | `GetCustomThemeImageVideoFilePath` | host-side | profile JSON | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:51` | |
| 45 | `SetCustomThemeImageVideoFilePath` | host->dev | SDK + filesystem | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:52` | |
| 46 | `ApplyBackgroundOnlyCustomTheme` | host->dev | SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:53` | |
| 47 | `GetThemeInfos` | mixed | SDK + profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:54` | |
| 48 | `SetThemeInfoId` | host-only | profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:55` | |
| 49 | `GetThemeInfoId` | host-only | profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:56` | |
| 50 | `GetThemeInfoSetting` | host-only | profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:57` | |
| 51 | `UpdateThemeInfoSetting` | host->dev | SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:58` | |
| 52 | `SetDefaultThemeInfoSetting` | host-only | profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:59` | |
| 53 | `ApplyThemeInfo` | host->dev | SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:60` | |
| 54 | `SyncThemeInfoImage` | host->host | `copyImageMemory` | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:61` | Same IPC pattern as Req 29. |
| 55 | `SaveThemeInfoToAsTemplate` | host-only | profile | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:62` | |
| 56 | `GetModulars` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:63` | |
| 57 | `GetModularSetting` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:64` | |
| 58 | `GetModularSettings` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:65` | |
| 59 | `AddModularSetting` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:66` | |
| 60 | `UpdateModularSetting` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:67` | |
| 61 | `SetDefaultModularSetting` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:68` | |
| 62 | `DeleteModularSetting` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:69` | |
| 63 | `ApplyModulars` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:70` | |
| 64 | `GetModularImage` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:71` | |
| 65 | `SaveModularsToAsTemplate` | mixed | profile + SDK | `UniversalScreen8p8Inch/UniversalScreen8p8InchRequestType.cs:72` | |

##### Subsystem patterns

- **Three-endpoint architecture.** A cluster of three USB devices glued by SDKs into one logical product.
- **LCD frame-upload is invisible to source.** 1920x480x4 BGRA framebuffer rendered host-side; SDK pushes frames internally. Resolved at lcd207-sdk level (section 3.2.2): the panel side is `WinUsbLed` LCD-side endpoint, encrypted via the same DES-CBC envelope.
- **Cross-process memory shipping.** `copyImageMemory(processId, destAddress, byte[] sourceBuffer)` is a one-way `WriteProcessMemory`-class IPC channel.
- **Sensor flow is one-way host->device.**
- **Non-monotonic speed encoding (lighting).** Host `Speed = 0..=100` -> SDK `RFLampEffectSpeedType = 7..3`.
- **24 fps fixed render rate.**

##### Notable divergences from LCDV2

- **Framing class.** No A/B/C wire framing on any endpoint.
- **No host-built packets.** Entirely mediated through `lcd207.dll` (panel) or `slv3.dll` (lighting LED side, plus opaque CH340 UART side).
- **Sensor direction inverted.**
- **Frame-upload is not a discrete command.**
- **Three USB IDs for one product.**
- **No reset / no soft-reset.**

##### Unclear flags

- Lighting wire bytes: documented in OpenRGB section 5.3.
- Display panel (`0x1CBE:0xA088`) wire format: resolved by section 3.2.2 (lcd207-sdk).
- Display-mode bridge (`0x1A86:0xAD21`, CH340 UART): unknown command set.
- `RgbEffectTypes` -> wire byte mapping: 19 host modes vs 16 OpenRGB-confirmed; reconciliation needs live capture.
- Lighting `Speed` / `Brightness` SDK-byte mapping post-SDK on-wire byte unclear.

##### Open questions for live capture (Phase 1+)

- Capture EP1/EP2/EP3 traffic on `0x1CBE:0xA088` while SDK is rendering a known template.
- Sniff CH340 UART for the display-mode SKU.
- Cross-check OpenRGB's `M_*` lighting-mode enumeration against the 19 host modes.
- Verify whether `screenFrameBuffer` BGRA byte order is preserved on the wire or swapped to RGB / RGB565 / JPEG-encoded before transmit.

#### 3.1.10 Strimer Plus (wired)

**Source folder:** `LConnectCore/Products/StrimerPlus/`

**Marketed name(s):** Lian Li Strimer Plus - RGB cable extensions for ATX 24-pin / VGA 8-pin / VGA-triple. LED-only addressable lighting strips, no fans, no LCD, no sensors.

**ControllerTypeID:** `StrimerPlus = 16801792` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:15`). Class bucket: `ControllerClass.StrimerPlus = 6` (`L-Connect.Core/LConnectCore/Devices/ControllerClass.cs:13`).

**USB VID:PID:** `0x0CF2:0xA200` (decimal `3314:41472`) - `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs:35`.

**Framing classes used:** none of the LCDV2 A/B/C framings - Strimer Plus uses a flat **HID Feature-Report** path for control packets and a flat **HID Output-Report** path for per-LED RGB data, with **`ReportID = 0xE0`** as the leading byte and a single-byte command code in `data[1]` whose low 4 bits sometimes encode a port index (`StrimerPlus/StrimerPlusDevice.cs:11`, `129-145`, `147-163`, `165-168`).

**Phase 0d status:** not in scope; static-analysis only.

**Status:** all entries `inferred-static`.

##### Subsystem fingerprint

- Sensors / handshake: no - only firmware version and a Channel-2 port-count probe (4 or 6 ports) are read back.
- Fan PWM / pump PWM / fan curve / temperature: N/A.
- LCD on/off / brightness / rotation / frame upload: N/A.
- Lighting (preset effect modes): yes - `SetEffectSetting(port, mode, speed, direction, brightness)` writes a 6-byte feature-report packet selecting one of 24 catalogued lighting modes split between two scopes.
- Lighting (per-LED RGB writes): yes - `SetColorSetting(port, Color[])` streams up to 27 RGB triples per port over an HID Output report. **Bytes are emitted as R, B, G** (not R, G, B) on the wire (`StrimerPlus/StrimerPlusDevice.cs:129-145`, `StrimerPlus/Constants.cs:18`). Confirmed by OpenRGB cross-validator.
- Effect-enable / port mask: yes - `SetEffectEnable(port, multiPort)` either enables a single port (low 4 bits = port index, 0..=11) or, when `port >= 12`, switches into multi-port mask mode.
- Frame counter: yes - `GetFrame` reads a BE-u16 frame counter; `SetFrame` writes one with constrained valid ranges. Purpose unclear.
- Continuous re-issue pattern: not observed in `StrimerPlus/StrimerPlusDevice.cs`. `WriteDelayTime` defaults to **40 ms** between every feature/output report.
- Reset / soft-reset: not present.
- Firmware upload / activation: not present (only `GetFirmwareVersion` is exposed).
- Crypto / obfuscation at HID transport: none observed.

##### Commands

Strimer Plus does **not** use the LCDV2 single-byte `Command` enum at offset 1. Instead, the byte at `data[1]` is a *command-and-port composite*.

| Operation | Byte at `data[1]` | Direction | Method | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|---|
| `GetFirmwareVersion` | `0x50` with sub-arg `0x00` | host<->dev | `GetFirmwareVersion()` | Feature report (3-byte tx, 5-byte rx) | `StrimerPlus/StrimerPlusDevice.cs:72-89`; `StrimerPlus/FirmwareVersion.cs:66-79` | inferred-static | Sentinel-byte validity check (`0xE0, 0x52, 0xFF, 0x40`). |
| `GetChannel2PortCount` | `0x50` with sub-arg `0x01` | host<->dev | `GetChannel2PortCount()` | Feature report (3-byte tx, 1-byte rx) | `StrimerPlus/StrimerPlusDevice.cs:61-70` | inferred-static | Returns 4 or 6 (other values mapped to -1). |
| `GetFrame` | `0x55` with sub-arg `0x05` | host<->dev | `GetFrame()` | Feature report (3-byte tx, 2-byte rx) | `StrimerPlus/StrimerPlusDevice.cs:91-100` | inferred-static | BE u16 frame counter. Purpose unclear. |
| `SetFrame` | `0x60` (96) | host->dev | `SetFrame(int frame)` | Feature report (4-byte tx) | `StrimerPlus/StrimerPlusDevice.cs:170-179` | inferred-static | Validity gate excludes `lo < 15`, `60 < lo < 80`, or `lo > 235`. |
| `SetEffectEnable` (single port) | `0x20 \| port` for `port < 12` | host->dev | `SetEffectEnable(port, _)` | Feature report (4-byte tx) | `StrimerPlus/StrimerPlusDevice.cs:147-163` | inferred-static | |
| `SetEffectEnable` (multi-port mask) | `0x2C` (`0x20 \| 12`) | host->dev | `SetEffectEnable(port>=12, multiPort)` | Feature report (4-byte tx) | `StrimerPlus/StrimerPlusDevice.cs:147-163`; `StrimerPlus/PortFlags.cs:5-27` | inferred-static | Bytes 2-3 carry BE u16 bitmask. |
| `SetEffectSetting` | `0x10 \| (port & 0x0F)` | host->dev | `SetEffectSetting(...)` | Feature report (6-byte tx) | `StrimerPlus/StrimerPlusDevice.cs:165-168` | inferred-static | Tx: `[0xE0, 0x10\|port, mode, speed, direction, brightness]`. Speed is **non-monotonic** (`Normal=0, Lower=1, Lowest=2, Fastest=254, Faster=255`). |
| `SetColorSetting` (per-LED RGB) | `0x30 \| (port & 0x0F)` | host->dev | `SetColorSetting(port, Color[])` | **Output report** (variable: `2 + N*3` bytes) | `StrimerPlus/StrimerPlusDevice.cs:129-145`; `StrimerPlus/Constants.cs:18` | inferred-static | **NB: byte order on the wire is R, B, G** - confirmed by OpenRGB. |

`RequestType` enum has only three nominal entries (`FirmwareVersion`, `Channel2PortCount`, `EffectSetting`) and `SettingType` is a per-port discriminator `Port0..Port11`. Neither enum is consumed inside `StrimerPlusDevice.cs`.

##### Subsystem patterns

- **Single transport, two HID directions:** every read and almost every write goes through HID Feature reports. The exception is the per-LED RGB stream, which uses HID Output reports.
- **Command-byte = operation high nibble + port low nibble:** operation tags `0x10`/`0x20`/`0x30`/`0x50`/`0x55`/`0x60`. Magic value 12 (`0x0C`) on `0x20` operation switches to multi-port-mask mode.
- **Two-channel x six-port topology.** Channel 1 hard-coded to 6 ports; Channel 2 is 4 or 6 depending on SKU. `PortFlags` exposes pre-baked masks (`Channel1 = 0x003F`, `Channel2_4 = 0x03C0`, `Channel2_6 = 0x0FC0`, `All = 0x0FFF`).
- **Two lighting scopes** (`Individual` vs `Complete`).
- **Pacing:** `WriteDelayTime` default = 40 ms - much slower than LCDV2.
- **Sentinel-byte firmware-validity check** (`0xE0, 0x52, 0xFF, 0x40`).

##### Notable divergences from LCDV2

- **No A/B/C `CommandPacket` framing.**
- **No 1-byte `Command` enum at offset 1** - composite byte instead.
- **Subsystem coverage is lighting-only.**
- **Different USB VID** (`0x0CF2` vs `0x0416`).
- **Different `ReportID`** (`0xE0` vs `0x01`/`0x02`/`0x03`).
- **Per-LED transport uses HID Output reports.**
- **R, B, G byte order on per-LED writes.**
- **`LightingSpeed` non-monotonic encoding.**
- **Pacing** (40 ms default).

##### Cross-cutting note: OpenRGB overlap

Strimer Plus is the family that most closely fits OpenRGB's target profile. OpenRGB cross-validation (section 5.3) confirms the R-B-G byte order and the non-monotonic speed table.

##### Wireless variant relationship

A separate, **wireless** Strimer Plus exists at `LConnectCore/Products/LWireless/StrimerPlus/` (`StrLightingConfig.cs`, `StrSeparateLightingConfig.cs`, `FirmwareVersion.cs`). Documented under section 3.1.12 LWireless.

##### Open questions

- Per-LED color byte order R-B-G - confirmed firmware quirk by OpenRGB cross-validation.
- `SetFrame` / `GetFrame` semantics.
- `FirmwareVersion.GetVersion` decoding offset `-3` opaque.
- `SetEffectEnable` payload-byte 2-3 in single-port mode unverified.
- Effect-enable persistence (mask = enabled set vs additive enable).
- Number of LEDs per port (`Constants.MaxLEDCountPerPort = 27` is host-side cap).
- Pacing tolerance (whether 40 ms is firmware requirement or conservative default).
- `SettingType` / `RequestType` consumers (defined but inert in this folder).

##### Recommendation

Strimer Plus is a clean, lighting-only family with a substantially different transport from LCDV2. The codec layer for it should be a separate module - **not** a parameterisation of the LCDV2 A/B/C `CommandPacket` machinery.

#### 3.1.11 Ene6K77Fan

**Source folder:** `LConnectCore/Products/Ene6K77Fan/`

**Marketed name(s):** ENE 6K77-class Lian Li fan controllers - the older RGB-fan controller line that predates TLFan. Five concrete SKU classes share `IEne6K77FanDevice` (`Ene6K77Fan/IEne6K77FanDevice.cs:7-38`): `SLFanDevice`, `ALFanDevice`, `SLInfinityDevice`, `SLV2FanDevice`, `ALV2FanDevice`, `SLRedragonDevice`.

**ControllerTypeID enum value(s):** `SLFan = 16781312`, `SLRedragon = 16781313`, `ALFan = 16785408`, `SLInfinity = 16789504`, `SLV2Fan = 16793600`, `SLV2AFan = 16793601`, `ALV2Fan = 16797696` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:8-14`).

**USB VID:PID:** Every Ene6K77Fan SKU sits on VID `0x0CF2` (decimal `3314`, ENE Technology) with sequential PIDs (`KnownDevices.cs:21-35`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | Source |
|---|---|---|---|
| `SLFan` (16781312) | `0x0CF2:0xA100` | `3314:41216` | `KnownDevices.cs:21` |
| `ALFan` (16785408) | `0x0CF2:0xA101` | `3314:41217` | `KnownDevices.cs:23` |
| `SLInfinity` (16789504) | `0x0CF2:0xA102` | `3314:41218` | `KnownDevices.cs:25` |
| `SLV2Fan` (16793600) | `0x0CF2:0xA103` | `3314:41219` | `KnownDevices.cs:27` |
| `ALV2Fan` (16797696) | `0x0CF2:0xA104` | `3314:41220` | `KnownDevices.cs:29` |
| `SLV2AFan` (16793601) | `0x0CF2:0xA105` | `3314:41221` | `KnownDevices.cs:31` |
| `SLRedragon` (16781313) | `0x0CF2:0xA106` | `3314:41222` | `KnownDevices.cs:33` |

VID `0x0CF2` is shared with `StrimerPlus` (PID `0xA200`).

**Framing classes used:** **family-specific (E-class).** HID feature reports (host-to-device) and HID input reports (device-to-host) keyed by `ReportID = 0xE0` (224). Host issues each request as `SendFeatureReport(data, data.Length)` of variable length (3..6 bytes for tags, `2 + colors.Length * 3` for color streams). Color streams use a separate output-report path (`Ene6K77Fan/SLFanDevice.cs:122-127`).

**Phase 0d status:** not in scope; static-analysis only.

**Status:** all entries `inferred-static`.

##### Subsystem fingerprint

- Sensors / handshake: partial - `GetFanSpeed` (tag `0xE0 0x50 0x00`) returns 8 or 9 bytes decoded as 4 x BE u16 fan RPM values. No temperature, no pump RPM, no PWM read-back.
- Fan PWM: `inferred-static` - `SetFanSpeed(groupIndex, duty)` writes `[0xE0, 0x20|group, 0x00, duty]` as a 4-byte feature report.
- Pump PWM: N/A.
- Fan curve upload: no on-device curve.
- Lighting (preset modes): `inferred-static` - `SetEffectSetting(port, mode, speed, direction, brightness)` writes `[0xE0, 0x10|port, mode, speed, direction, brightness]`.
- Lighting (per-pixel LED): `inferred-static` via output report - `SetColorSetting(port, Color[])` builds `[0xE0, 0x30|port, R, B, G, R, B, G, ...]`. **R, B, G byte order, not R, G, B.**
- LCD: N/A.
- "Frame" concept (`GetFrame` / `SetFrame`) - appears to be an animation frame index, not a JPEG/H.264 upload. Unclear semantics.
- Continuous re-issue pattern: yes for fan speed (host curve loop).
- Reset / soft-reset: no.
- Firmware upload / activation: none in this device class.
- Crypto / obfuscation at HID transport: none observed.

##### Commands

The wire is "tag-byte multiplexed". `RequestType` is a logical host-side dispatch enum (`Ene6K77Fan/RequestType.cs:5-14`) with seven entries: `FirmwareVersion, GetFanSpeed, SetFanSpeed, LightingSetting, FanQuantity, MergeOrder, Ping`.

| Logical name | Tag bytes (byte 1, byte 2) | Direction | Citation | Notes |
|---|---|---|---|---|
| `GetFanSpeed` | `0x50 0x00` | host->dev + dev->host (5..9-byte input report) | `Ene6K77Fan/SLFanDevice.cs:61-75`, `Ene6K77Fan/ALV2FanDevice.cs:61-75` | Returns 4 x BE u16 fan RPM. SLFan/SLInfinity/SLRedragon read 8 bytes; ALFan/SLV2Fan/ALV2Fan read 9 bytes (header byte then RPMs). |
| `GetFirmwareVersion` | `0x50 0x01` | host->dev + dev->host (5-byte input report) | `Ene6K77Fan/SLFanDevice.cs:77-84` | Decoded by per-SKU `*FirmwareVersion.cs::Load(byte[])`. |
| `GetFrame` | `0x55 0x05` (SL/SLInfinity), `0x80 0x05` (SLV2) | host->dev + dev->host | `Ene6K77Fan/SLFanDevice.cs:86-95`, `Ene6K77Fan/SLV2FanDevice.cs:86-95`, `Ene6K77Fan/ALFanDevice.cs:86-95` | u16 frame index. ALFan/ALV2Fan check `inputReportData[0] == 0xF5`. |
| `Ping` | `0x10\|group, 0x11/0x17/0x34/...` | host->dev | `Ene6K77Fan/SLFanDevice.cs:110-113`, `Ene6K77Fan/ALFanDevice.cs:110-113`, etc. | Per-SKU sub-tag varies. |
| `SetColorSetting` | `0x30\|port` (output report) | host->dev | `Ene6K77Fan/SLFanDevice.cs:129-149` | **Wire color order R, B, G**. |
| `SetEffectSetting` | `0x10\|port, mode` | host->dev | `Ene6K77Fan/SLFanDevice.cs:151-154` | 6-byte feature report. |
| `SetFanMotherboardSync` | per-SKU sub-tag (`0x10 0x31` SL, `0x10 0x42` AL, `0x10 0x61` ALV2/SLV2/SLInfinity) | host->dev | `Ene6K77Fan/SLFanDevice.cs:156-161`, etc. | Byte 3 packs `(1<<(group+4)) \| (isSync<<group)`. |
| `SetFanSpeed` | `0x20\|group, 0x00` | host->dev | `Ene6K77Fan/SLFanDevice.cs:163-166` | PWM 0..=100. |
| `SetFrame` | `0x60` | host->dev | `Ene6K77Fan/SLFanDevice.cs:168-171` | 4-byte feature report. |
| `SetLightingMotherboardSync` | per-SKU sub-tag | host->dev | `Ene6K77Fan/SLFanDevice.cs:173-178`, etc. | |
| `SetMergeOrder` | per-SKU sub-tag | host->dev | `Ene6K77Fan/SLV2FanDevice.cs:187-195`, etc. | **No-op on SLFan and ALFan** (empty method bodies). |
| `SetQuantity` | per-SKU sub-tag | host->dev | `Ene6K77Fan/SLFanDevice.cs:184-189`, `Ene6K77Fan/ALFanDevice.cs:191-197` | Per-SKU encoding differs. |
| `StartMerge` | `0x10 0x33 0x00 0x01 0x02 0x03 0x08` | host->dev | `Ene6K77Fan/SLFanDevice.cs:191-194` | **SLFan-only**. |
| `StopMerge` | `0x10 0x34 0x00 0x00 0x00` | host->dev | `Ene6K77Fan/SLFanDevice.cs:196-199` | **SLFan-only**. |
| `SendMergeCommand(enable)` | `0x10 0x43, byte3=enable` | host->dev | `Ene6K77Fan/ALFanDevice.cs:122-127` | **ALFan-only**. |

##### Subsystem patterns

- **Tag-byte multiplexed wire.** No `Command` enum byte at fixed offset.
- **Two HID transport channels.** Feature reports for control; output reports for color streams. Both share `ReportID = 0xE0`.
- **Per-SKU sub-tag drift.** Same logical operation uses different sub-tag bytes across SKUs. ALV2Fan and SLV2Fan use `0x6X`; ALFan uses `0x4X`; SLFan uses `0x3X`.
- **Color byte order R, B, G on the wire.**
- **`SetMergeOrder` is a SKU-conditional no-op.**
- **WriteDelayTime = 20 ms** after every report. Without this delay the firmware appears to drop or coalesce successive commands.
- **No read-after-write on writes.**

##### Notable divergences from LCDV2

- **Different framing class entirely.**
- **No B-class or C-class framing.**
- **No fixed `Command` byte at known offset.**
- **VID is `0x0CF2` (ENE Technology), not `0x0416`.**
- **No pump, no LCD, no temperature.**
- **Wire color order R, B, G.**
- **`ReportID = 0xE0` (224).**

##### Notable comparison to TLFan (cross-cutting)

TLFan and Ene6K77Fan **do not share command bytes** despite both being addressable-RGB fan controllers from the same vendor. Different ReportIDs, framings, command-byte ranges, and VIDs. A Linux daemon cannot reuse codecs across these two families.

##### Open questions

- Whether the `0x55 0x05` `GetFrame` / `0x60` `SetFrame` pair drives an animation playback index or something else.
- Exact meaning of the 4-byte `groupIndexOrder` argument in `SetMergeOrder`.
- 5-byte input-report layouts decoded by each `*FirmwareVersion::Load(byte[])` differ per SKU.
- Hidden command space - tags `0x70`, `0x80`, `0x90`, `0xA0`, etc. may be defined in firmware but unused by host.
- Whether ALFan and SLFan have no `SetMergeOrder` implementation, or held in different command.
- Whether 20 ms `WriteDelayTime` is firmware requirement or conservative default.
- Service-side ping cadence not visible from this folder.
- `0xF5` sentinel ALFan/ALV2Fan check on `GetFrame` responses - "frame ready" vs "data valid" flag.

#### 3.1.12 LWireless

**Source folder:** `LConnectCore/Products/LWireless/`

**Marketed name(s):** Lian Li Wireless ARGB / fan / pump hub - the **RF transmitter dongle** that talks over a proprietary 2.4 GHz radio to a swarm of wireless receivers. Hub is USB HID dongle; end devices (TLV2 LCD fans, Lancool 217 Infinity case, SLV3 fans, SLV3 LCD fans, Strimer Plus wireless, HydroShift II LCD wireless) hang off the radio bus and are addressed by 6-byte MAC.

**ControllerTypeID enum value(s):** the hub itself appears as one of: `WirelessControllerTransmitter = 16973824`, `WirelessControllerReceiver = 16973825`, `WirelessControllerTransmitterV2 = 16973826`, `WirelessControllerReceiverV2 = 16973827` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:25-28`).

**USB VID:PID:** four hub variants (`KnownDevices.cs:53-59`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | Source |
|---|---|---|---|
| `WirelessControllerTransmitter` | `0x0416:0x8040` | `1046:32832` | `KnownDevices.cs:53` |
| `WirelessControllerReceiver` | `0x0416:0x8041` | `1046:32833` | `KnownDevices.cs:55` |
| `WirelessControllerTransmitterV2` | `0x1A86:0xE304` | `6790:58116` | `KnownDevices.cs:57` |
| `WirelessControllerReceiverV2` | `0x1A86:0xE305` | `6790:58117` | `KnownDevices.cs:59` |

V1 is on the Winbond/Lian-Li VID (`0x0416`); V2 is on QinHeng / WCH (`0x1A86`).

**Framing classes used:** **opaque - proprietary `slv3` SDK.** Resolved via section 3.2.1 (slv3-sdk): hub HID interface speaks `Usb_SendRf=0x10` carrier frames in 64-byte WinUSB packets, RF payload fragmented into 60-byte chunks, with the `RF_Select=0x12` envelope identifying per-device-targeted RF frames. `MasterDevice.cs` is the orchestrator.

**Phase 0d status:** not in scope; transport now resolved by slv3-sdk survey.

**Status:** all entries `inferred-static`.

##### Subsystem fingerprint (via SDK API surface)

- Sensors / handshake: `inferred-static` - `RFController.GetAllGroup()` returns bound devices; `RFController.GetAioTemp(macStr)` returns water temperature.
- Fan PWM: `inferred-static` - `RFController.SetFansRPM(macStr, List<byte> duty)` takes 4 PWM bytes (mapped 0..=100 -> 0..=255).
- Pump PWM: `inferred-static` - `RFController.SetAioPumpSpeed(macStr, rpm)` for AIO type 10, `SetH2SAioPumpSpeed` for type 11.
- Fan curve upload: partial - `RFController.SetAioParams(aioParams, devType)` uploads pump-config parameters.
- Lighting (preset modes): `inferred-static` - `SetLightEffect`, `SetAllLightEffect`, `SetUIEffect`.
- Lighting (per-pixel LED): inferred via `lightEffectSet` (no obvious raw-LED-stream entry point in the surfaced API).
- LCD on/off / brightness / rotation: `inferred-static` for HS II LCD path.
- LCD frame upload: passthrough via host-side image pipeline (named pipe IPC, see below).
- Continuous re-issue pattern: yes - sensor loops poll `GetAioTemp` and re-issue `SetFansRPM`.
- Reset / soft-reset: `ResetDevice` (RequestType 18) declared but unhandled.
- Firmware upload / activation: partial - `WirelessLCDUpgradeRequest`, `WirelessLCDUpgradeResponse`, `WirelessLCDUpgradeProgressResponse`.
- Crypto / obfuscation at HID transport: 2.4 GHz RF link presumably encrypted/paired (see `BindDevice`/`UnBindDevice`/`LockBoundDevices`); on-USB framing is plain-byte per slv3-sdk.

##### Named-pipe IPC for image transport

**Pipe name format:** `LConnect_<Guid>` - generated by `L-Connect3/LConnect3cs/Products/LWireless/SLV3LCD/SLV3UtilityLCDScreenPage.cs:1150-1153`.

**Direction:** the **service-side LCD controller** runs the `PipeImageSender` (server, writer); the **UI-side image grid** (`L-Connect3/LConnect3cs/Views/UserControls/NamedPipeImageGrid.cs:132`) runs the `PipeClientManager` (client, reader). The service composes the LCD frame and publishes it to the pipe so the UI can display a live preview. **The pipe does NOT carry the bytes that go on the radio** - RF transmission still goes through `slv3.dll`. The pipe is purely a host-side preview-mirror channel.

**Message framing:** length-prefixed binary records, little-endian (`PipeImageSender.cs:56-58`, `PipeClientManager.cs:41-50`).

```
offset  size  field          description
------  ----  -----          ---------------------------------------------
  0      4   Length          int32 LE; size of the following Image
  4   Length Image           raw image bytes
```

Cadence: `Task.Delay(50)` between successive frames - capped at 20 fps. Server lifetime: `MaxNumberOfServerInstances=2` clients.

##### LWireless child families

The `LWireless/` folder contains six sub-folders (one per receiver family). Each child rides the same `slv3` SDK passthrough.

###### Child family: TLV2LCD

- **Source folder:** `LConnectCore/Products/LWireless/TLV2LCD/` (one file: `TLV2LCDFirmwareVersion.cs`)
- **Marketed name:** Lian Li UNI FAN TL LCD V2 (wireless variant)
- **ControllerTypeID:** `TLV2LCD = 16982016` (`ControllerTypeID.cs:30`)
- **USB VID:PID:** `0x1CBE:0x0006` (decimal `7358:6`) - wired USB ID; over wireless, child of LWireless hub.
- **Wire format:** passthrough via `slv3` SDK over RF.
- **Firmware version regex:** `^tlv2_\d{8}(\d{2})(?:_\d{2})?$`. Decoded version is `(seq + 9) / 10 . (seq + 9) % 10`.
- **Image transport:** named-pipe IPC (UI preview only).
- **Status:** `inferred-static` passthrough.

###### Child family: Lancool 217 (Infinity)

- **Source folder:** `LConnectCore/Products/LWireless/Lancool217Inf/` (one file: `Lancool217InfScopes.cs`)
- **Marketed name:** Lian Li Lancool 217 Infinity
- **ControllerTypeID:** none - exists only as addressable scope/device type behind the wireless hub.
- **Constants:** front/rear fan speed ranges in `LWireless/Constants.cs:115-121` (Lancool217Rear: 430-1900; Lancool217Front: 200-1500).
- **Special command:** `RFController.Close217Wifi(macStr)` - unique to Lancool 217.
- **Status:** `inferred-static` passthrough plus one transformative command.

###### Child family: SLV3

- **Source folder:** `LConnectCore/Products/LWireless/SLV3/` (`LWirelessLightingConfig.cs`, `LWirelessUtilityLightEffectsSetting.cs`)
- **Marketed name:** Lian Li UNI FAN SL V3 (wireless variant)
- **Status:** `inferred-static` passthrough.

###### Child family: SLV3LCD (wireless variant)

- See section 3.1.8 for the wired-side detailed survey. Wireless variant rides the hub.
- **Status:** `inferred-static` passthrough.

###### Child family: HydroShiftIILCD (wireless variant)

- **Source folder:** `LConnectCore/Products/LWireless/HydroShiftIILCD/` (multiple files)
- **Wire format:** **partially transformative.** Most LWireless-specific commands in `LWireless/RequestType.cs` (entries 32-44).
- **Status:** mostly transformative.

###### Child family: StrimerPlus (wireless variant)

- **Source folder:** `LConnectCore/Products/LWireless/StrimerPlus/` (`FirmwareVersion.cs`, `StrLightingConfig.cs`, `StrSeparateLightingConfig.cs`)
- **Wire format:** passthrough. `RFController.SetStrSingleLightEffect(macStr, arg1..arg6)` takes 6 separate effect-set arguments.
- **Status:** `inferred-static` passthrough.

##### Commands (logical RequestType enum, 46 entries)

The `RequestType` enum (`LWireless/RequestType.cs:5-53`) is the host-side service-dispatch enum, not a wire-byte enum. Numeric values are enum ordinals 0..45.

| Idx | Name | Handler citation | SDK call | Status | Notes |
|---|---|---|---|---|---|
| 0 | `FanGroupStatus` | `LWirelessController.cs:1502` | `RFController.GetAllGroup` | inferred-static | |
| 1 | `WirelessDeviceStatus` | `LWirelessController.cs:1507` | `GetAllGroup` + `GetAioTemp` | inferred-static | |
| 2 | `SortGroupIndex` | `LWirelessController.cs:1512` | `RFController.SortGroup` | inferred-static | |
| 3 | `SortToLast` | `LWirelessController.cs:1517` | `RFController.SortToLast` | inferred-static | |
| 4 | `FirmwareVersion` | `LWirelessController.cs:1522` | `RFController.GetVersionInfo` | inferred-static | |
| 5 | `Direction` | falls to default (no-op) | (none) | declared-only | |
| 6 | `FanGroupConfig` | falls to default (no-op) | (none) | declared-only | |
| 7 | `FanLightingSetting` | `LWirelessController.cs:1540` | `RFController.SetLightEffect` | inferred-static | |
| 8 | `FanMergeLightingSetting` | `LWirelessController.cs:1545` | `RFController.SetUIEffect` | inferred-static | |
| 9 | `StrLightingSetting` | `LWirelessController.cs:1550` | `RFController.SetLightEffect` | inferred-static | |
| 10 | `StrSingleLightSetting` | `LWirelessController.cs:1555` | `RFController.SetStrSingleLightEffect` | inferred-static | |
| 11 | `FanQuantity` | falls to default (no-op) | (none) | declared-only | |
| 12 | `MergeOrder` | falls to default (no-op) | (none) | declared-only | |
| 13 | `TestLight` | falls to default (no-op) | (none) | declared-only | |
| 14 | `BindDevice` | `LWirelessController.cs:1560` | `RFController.Bind` | inferred-static | |
| 15 | `BindAllDevice` | falls to default (no-op) | (none) | declared-only | |
| 16 | `UnBindDevice` | `LWirelessController.cs:1565` | `RFController.UnBind` | inferred-static | |
| 17 | `UnBindAllDevice` | falls to default (no-op) | (none) | declared-only | |
| 18 | `ResetDevice` | falls to default (no-op) | (none) | declared-only | **Possibly destructive**. |
| 19 | `ChangeChannel` | `LWirelessController.cs:1570` | `RFController.ChangeChannel` | inferred-static | |
| 20 | `Ping` | `LWirelessController.cs:1575` | (varies) | inferred-static | |
| 21 | `SetFanSpeed` | `LWirelessController.cs:1580` | `RFController.SetFansRPM` | inferred-static | |
| 22 | `SetPumpSpeed` | `LWirelessController.cs:1585` | `RFController.SetAioPumpSpeed`/`SetH2SAioPumpSpeed` | inferred-static | RPM, not PWM. |
| 23 | `SetCaseSpeed` | `LWirelessController.cs:1590` | `RFController.SetFansRPM` | inferred-static | |
| 24 | `SaveEffectConfig` | `LWirelessController.cs:1595` | `RFController.SaveCfg` | inferred-static | |
| 25 | `SetAllFanLightEffect` | `LWirelessController.cs:1600` | `RFController.SetAllLightEffect` | inferred-static | |
| 26 | `SetAllStrLightEffect` | `LWirelessController.cs:1605` | `RFController.SetAllLightEffect` | inferred-static | |
| 27 | `GetAllMasterController` | `LWirelessController.cs:1610` | `RFController.GetAllMasterController` | inferred-static | |
| 28 | `GetMasterCount` | `LWirelessController.cs:1615` | `RFController.GetAllMasterController` | inferred-static | |
| 29 | `GetChannel` | `LWirelessController.cs:1620` | (lookup) | inferred-static | |
| 30 | `LockBoundDevices` | `LWirelessController.cs:1625` | `RFController.LockDevice` | inferred-static | |
| 31 | `IsDevicesLocked` | `LWirelessController.cs:1630` | `RFController.IsLock` | inferred-static | |
| 32 | `SetPumpLCDBrightness` | `LWirelessController.cs:1635` | (HS II LCD path) | inferred-static | |
| 33 | `SetPumpLCDRotate` | `LWirelessController.cs:1640` | (HS II LCD path) | inferred-static | |
| 34 | `GetHydroShiftIILCDHandshakeInfo` | `LWirelessController.cs:1645` | (HS II LCD path) | inferred-static | |
| 35 | `GetWaterTemperature` | `LWirelessController.cs:1650` | `RFController.GetAioTemp` | inferred-static | |
| 36 | `SwitchAdvanceMode` | `LWirelessController.cs:1655` | (HS II LCD path) | inferred-static | |
| 37 | `SetPumpCarouselSettings` | `LWirelessController.cs:1660` | (HS II LCD path) | inferred-static | |
| 38 | `ApplyPumpWirelessTemplate` | `LWirelessController.cs:1665` | `RFController.SetAioThemeIndex` | inferred-static | |
| 39 | `WiredHydroShiftIILCDDisposed` | `LWirelessController.cs:1670` | (cleanup) | inferred-static | |
| 40 | `CheckAdvanceMode` | `LWirelessController.cs:1675` | (HS II LCD) | inferred-static | |
| 41 | `PlayWirelessContents` | `LWirelessController.cs:1680` | (HS II LCD) | inferred-static | |
| 42 | `WiredStopContents` | `LWirelessController.cs:1685` | (HS II LCD) | inferred-static | |
| 43 | `WiredPlayContents` | falls to default (no-op) | (none) | declared-only | |
| 44 | `GetWirelessPumpSetting` | `LWirelessController.cs:1690` | (HS II LCD) | inferred-static | |
| 45 | `DisableLC217WirelessSignal` | `LWirelessController.cs:1695` | `RFController.Close217Wifi` | inferred-static | **Lancool 217-only**. |

##### Subsystem patterns

- **SDK passthrough is the dominant pattern.**
- **MAC-string addressing** for all per-device commands.
- **PWM is mapped 0..=100 -> 0..=255 before transmission.**
- **Image transport is out-of-band** via named pipe (preview mirror only).
- **Multi-hub support** via `GetAllMasterController` and `ChangeChannel`.
- **Pairing semantics:** `BindDevice` / `UnBindDevice` / `LockBoundDevices` / `IsDevicesLocked`.

##### Notable divergences from LCDV2

- **Opaque transport** (now resolved by section 3.2.1).
- **MAC-string addressing instead of fixed device-handle.**
- **Out-of-band image pipe.**
- **PWM scaling 0..=100 -> 0..=255.**
- **Multi-tier device hierarchy.**
- **Pairing / locking is first-class.**
- **Two HW generations on different USB silicon.**

##### Open questions

- The full wire-level framing - resolved by section 3.2.1.
- Enum entries 5, 6, 11, 12, 13, 15, 17, 18, 43 fall through to default (no-op).
- `ResetDevice` (idx 18) declared but unhandled.
- Pipe protocol image format (length prefix documented; payload format set by caller).
- Pipe access control (Linux equivalent needs explicit access-mode policy).
- Frame cadence cap of 20 fps; whether RF link can carry that to LCD unverified.
- Pairing flow byte sequence opacity.
- Lancool 217 `Close217Wifi` semantics (radio-off vs unsubscribed).
- Cross-MCU code paths inside `slv3.dll` for V1 vs V2 hubs.

#### 3.1.13 Lancool 207 Digital

**Source folder:** `LConnectCore/Products/Lancool207Digital/` plus the service-side controller at `L-Connect-Service/LConnectService/Products/Lancool207Digital/Lancool207DigitalController.cs`.

**Marketed name(s):** Lian Li **LANCOOL 207 Digital** - case with a front-mounted high-resolution wide LCD (1472x720). Two transport ControllerTypeIDs: `Lancool207Digital` (control endpoint) and `Lancool207DigitalDisplayMode` (display-mode-only endpoint).

**Note:** Lancool **217** Digital is a different product - see section 3.1.12.

**ControllerTypeID enum value(s):** `Lancool207Digital = 17039360`, `Lancool207DigitalDisplayMode = 17039361` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:35-36`).

**USB VID:PID:** two endpoints (`KnownDevices.cs:73-75`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | Source |
|---|---|---|---|
| `Lancool207Digital` (17039360) | `0x1CBE:0xA065` | `7358:41061` | `KnownDevices.cs:73` |
| `Lancool207DigitalDisplayMode` (17039361) | `0x1A86:0xAD11` | `6790:44305` | `KnownDevices.cs:75` |

VID `0x1CBE` is Lian-Li's own USB vendor ID. VID `0x1A86` (QinHeng / WCH) on the DisplayMode endpoint matches the DisplayMode-only endpoints of HS II LCD and Universal Screen 8p8.

**Framing classes used:** **WinUSB+DES envelope (lcd207-sdk).** The controller imports `using lcd207;` (`Lancool207DigitalController.cs:2`) and dispatches via `LCDController` (e.g., `IsAvailable`, `SetBrightness`, `ApplyTemplate`). Per section 3.2.2 (lcd207-sdk): the wire format is identical to HS II at the byte level (same `CmdType`, same DES envelope, same body-at-offset-512 layout). Differences are dimensions (1472x720 vs HS II's 480x480) and which subset of `CmdType` opcodes are used (no AIO-specific opcodes like `SyncPumpFan` / `GetH2Params` / `PushRgbData` / `SetWthemeIndex`).

**Phase 0d status:** not in scope; transport now resolved.

**Status:** all entries `inferred-static`.

##### Subsystem fingerprint (via SDK API surface)

- Sensors / handshake: `inferred-static` - `LCDController.IsAvailable()`, `GetDeviceVer(action)`, `get_Ver()`. Sensor-value updates run on a host-side `NonOverlappingTimer`.
- Fan PWM: N/A on the LCD itself. `Lancool207DigitalFanSourceSetting` selects which sensor source drives the displayed fan readout, not fan-PWM control.
- Pump PWM / fan curve / lighting: N/A.
- LCD on/off / brightness / rotation: `inferred-static` - `LCDController.SetBrightness(brightness)` (0..=100). No rotation API surfaced.
- LCD frame upload: `inferred-static` via SDK theme / template / modular composition. Cross-process shared-memory IPC for top-layer overlays via `copyImageMemory(processId, address, screenFrameBuffer)`.
- Continuous re-issue pattern: `inferred-static` - firmware-version checker polls every 2000 ms; sensor value update timer drives re-renders.
- Reset / soft-reset: `inferred-static` - `ResetProfile` resets host-side JSON; no `ResetDevice` entry.
- Firmware upload / activation: `inferred-static` - `StartFirmwareUpdate` + `CheckFirmwareUpdateStatus`. `isFirmwareUpdating` flag gates all other request handling.
- Crypto / obfuscation at HID transport: DES-CBC encryption resolved by lcd207-sdk (key = `slv3tuzx`, IV = key).

##### Cross-process shared-memory image transport

For top-layer image overlays and theme-info image syncing, the controller reads pixel data **directly from another process's address space**:

```
Lancool207DigitalSyncScreenTopLayerImageRequest:
  - ProcessId    (int, must be > 0)
  - Address      (memory address inside that process)

handleSyncScreenTopLayerImageRequest():
  - copyImageMemory(processId, address, screenFrameBuffer)
    (Lancool207DigitalController.cs:3316)
```

`copyImageMemory` almost certainly wraps Windows' `OpenProcess` + `ReadProcessMemory`. **This pattern is Windows-specific**; Linux equivalents (`process_vm_readv` or `memfd_create` + `SCM_RIGHTS`) would need to replace it.

`screenFrameBuffer` is sized by `screenFrameRect` corresponding to `1472 * 720 * <bpp>` bytes (probably 4 for RGBA).

##### Commands (logical Lancool207DigitalRequestType enum, 54 entries)

Wire layer is `WinUSB+DES envelope` (resolved via lcd207-sdk).

| Idx | Name | Handler citation | SDK call (where visible) | Status | Notes |
|---|---|---|---|---|---|
| 0 | `SaveProfile` | `Lancool207DigitalController.cs:2570` | (host-only) | inferred-static | |
| 1 | `GetOfflineModeClock` | `:2575` | (host-only) | inferred-static | |
| 2 | `SetOfflineModeClock` | `:2580` | `LCDController.SetOfflineClock` | inferred-static | |
| 3 | `SetSecondaryScreen` | `:2585` | `LCDController.SetDesktopMode/SetMonitorMode` | inferred-static | |
| 4 | `GetBrightness` | `:2590` | (lookup) | inferred-static | |
| 5 | `SetBrightness` | `:2595` | `LCDController.SetBrightness` | inferred-static | |
| 6 | `GetIsAvailable` | `:2600` | `LCDController.IsAvailable` | inferred-static | |
| 7 | `GetFanSourceSettings` | `:2605` | (host-only) | inferred-static | |
| 8 | `SetFanSourceSettings` | `:2610` | (host-only) | inferred-static | |
| 9 | `SyncScreenTopLayerImage` | `:2615` | `copyImageMemory` | inferred-static | **Cross-process shared-memory read**. |
| 10 | `GetPreviewBackground` | `:2620` | (host-side) | inferred-static | |
| 11 | `GetFirmwareVersion` | `:2625` | `LCDController.GetDeviceVer/get_Ver` | inferred-static | |
| 12 | `StartFirmwareUpdate` | `:2630` | (SDK) | inferred-static | **Destructive**. |
| 13 | `CheckFirmwareUpdateStatus` | `:2558` | (SDK) | inferred-static | Always-allowed during update. |
| 14 | `ReloadAssets` | `:2640` | (SDK) | inferred-static | |
| 15 | `StopScreenRender` | `:2645` | (SDK) | inferred-static | |
| 16 | `ExportProfile` | `:2650` | (host-only) | inferred-static | |
| 17 | `ImportProfile` | `:2655` | (host-only) | inferred-static | |
| 18 | `ResetProfile` | `:2660` | (host-only) | inferred-static | |
| 19 | `GetSelectedTemplateId` | `:2665` | (lookup) | inferred-static | |
| 20 | `GetTemplates` | `:2670` | `LCDController.GetTemplates` | inferred-static | |
| 21 | `ImportTemplate` | `:2675` | `LCDController.ImportTemplate` | inferred-static | |
| 22 | `DeleteTemplate` | `:2680` | `LCDController.RemoveTemplate` | inferred-static | |
| 23 | `ApplyTemplate` | `:2685` | `LCDController.ApplyTemplate` | inferred-static | **Renders to display**. |
| 24 | `ChangeTemplateBackground` | `:2690` | (host + SDK save) | inferred-static | |
| 25 | `RevertTemplateBackground` | `:2695` | (host-side) | inferred-static | |
| 26 | `GetTemplateCategories` | `:2700` | (host-side) | inferred-static | |
| 27 | `AddTemplateCategory` | `:2705` | (host-side) | inferred-static | |
| 28 | `DeleteTemplateCategory` | `:2710` | (host-side) | inferred-static | |
| 29 | `RenameTemplateCategory` | `:2715` | (host-side) | inferred-static | |
| 30 | `GetAllTemplateAssignedCategories` | `:2720` | (host-side) | inferred-static | |
| 31 | `UpdateTemplateAssignedCategories` | `:2725` | (host-side) | inferred-static | |
| 32 | `GetCustomThemeImageVideoFilePath` | `:2730` | (host-side) | inferred-static | |
| 33 | `SetCustomThemeImageVideoFilePath` | `:2735` | (host-side) | inferred-static | |
| 34 | `ApplyBackgroundOnlyCustomTheme` | `:2740` | (SDK) | inferred-static | |
| 35 | `GetThemeInfos` | `:2745` | `LCDController.GetThemeInfos` | inferred-static | |
| 36 | `SetThemeInfoId` | `:2750` | (host-side) | inferred-static | |
| 37 | `GetThemeInfoId` | `:2755` | (host-side) | inferred-static | |
| 38 | `GetThemeInfoSetting` | `:2760` | (host-side) | inferred-static | |
| 39 | `UpdateThemeInfoSetting` | `:2765` | `LCDController.UpdateThemeInfo` | inferred-static | |
| 40 | `SetDefaultThemeInfoSetting` | `:2770` | (host-side) | inferred-static | |
| 41 | `ApplyThemeInfo` | `:2775` | (SDK) | inferred-static | |
| 42 | `SyncThemeInfoImage` | `:2780` | `copyImageMemory` | inferred-static | **Cross-process shared-memory read**. |
| 43 | `SaveThemeInfoToAsTemplate` | `:2785` | `LCDController.SaveTemplate` | inferred-static | |
| 44 | `GetModulars` | `:2790` | `LCDController.GetModulars` | inferred-static | |
| 45 | `GetModularSetting` | `:2795` | (host-side) | inferred-static | |
| 46 | `GetModularSettings` | `:2800` | (host-side) | inferred-static | |
| 47 | `AddModularSetting` | `:2805` | (host + SDK) | inferred-static | |
| 48 | `UpdateModularSetting` | `:2810` | `LCDController.UpdateModular` | inferred-static | |
| 49 | `SetDefaultModularSetting` | `:2815` | (host-side) | inferred-static | |
| 50 | `DeleteModularSetting` | `:2820` | (host-side) | inferred-static | |
| 51 | `ApplyModulars` | `:2825` | (SDK) | inferred-static | |
| 52 | `GetModularImage` | `:2830` | `LCDController.GetBitmapByTheme` | inferred-static | |
| 53 | `SaveModularsToAsTemplate` | `:2835` | (SDK) | inferred-static | |

##### Subsystem patterns

- **Theme-based composition.** Structured `Theme` object model, not raw frames.
- **Sensor data flows through host-side `sensorValues` dict** (`Dictionary<string, double>`).
- **Cross-process shared-memory image transport** for top-layer overlays.
- **SDK init lifecycle.** `LCDController.Init` + `Cleanup` with retry logic.
- **Firmware-update gate.** `isFirmwareUpdating` blocks all request handling except status poll.
- **Display vs control mode endpoints.** Two ControllerTypeIDs on different VID/PID.

##### Notable divergences from LCDV2

- **Opaque transport** at C# layer (resolved via section 3.2.2).
- **Theme model instead of frame model.**
- **Cross-process memory IPC.**
- **Two endpoints per device** (control + display-mode).
- **Resolution is much higher** (1472x720 vs LCDV2's 480x480).
- **No fan/pump/lighting subsystem.**
- **No reset command.**
- **Firmware-update is in-band** (vs LCDV2's declared-only).

##### Open questions

- The full wire-level framing - resolved by section 3.2.2.
- `copyImageMemory` implementation details (Linux equivalent needed).
- DisplayMode endpoint (`0x1A86:0xAD11`) protocol.
- Theme / template / modular wire format internal to SDK.
- Sensor data binding placeholder names hard-coded inside `lcd207.dll`.
- Fan source settings semantics (cosmetic vs control).
- `StartFirmwareUpdate` payload format (encryption / signing).
- `SetSecondaryScreen` semantics.
- Relationship between `ResetProfile` (host-side reset) and any device-side reset.

### 3.2 Shared protocol-implementation libraries

#### 3.2.1 slv3 SDK

**Source:** decompiled `slv3.dll` at `/home/inky/Development/lian-li/slv3/slv3/` + `slv3.models.dll` at `/home/inky/Development/lian-li/slv3.models/slv3/models/`.

**Status:** all citations are `inferred-static` (decompiled artifacts: anonymous-method names retained, some raw IL hints in `WinUsb.cs:151-161,304` etc., but the wire-relevant code is intact).

**Used by these families:**

- **SLV3LCD** - `L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs:190-846` (consumes `STRController.GetAllControlDevice`, `ShowAndSaveImg`, `RefreshSTRPosInfo`, `RestLcdHub`, `PlayVGroupMedia`).
- **TLV2LCD** - `L-Connect-Service/LConnectService/Products/TLV2LCD/TLV2LCDController.cs:191-847` (same call surface as SLV3LCD).
- **HydroShift II LCD** - **only via `slv3.models` enums** (effect IDs and `LightEffectSet` DTO). The HID transport for HS II LCD itself is in `lcd207.dll` (per section 3.2.2 below), not slv3. Wireless HS II variants are routed through `RFController`.
- **UniversalScreen 8.8"** - slv3 reaches this product only through `WinUsb.Get88inch_LED()` / `WinUsb.Led_Send` (the LED side of the 8.8" board, VID `0x0416` / PID `0x8050` = 1046 / 32848). The **panel** side (LCD frames) goes through `lcd207.dll`'s WinUSB stack (per section 3.2.2), not slv3. The lighting board IS in slv3.
- **LWireless hub (RF / wireless variants)** - `L-Connect.Core/LConnectCore/Products/LWireless/LWirelessDevice.cs:48-196` (consumes `RFController.SetAioThemeIndex`, `Bind`, `GetAllGroup`, `Init`, `SetFansRPM`, `FgSync`, etc.).
- **Lancool 207 Digital** - **NOT** a direct slv3 caller. The "Lancool 207" path inside slv3 (`DevTypes.LC217 = 65`, `RF_217CloseWifi = 35`, `Close217Wifi(macStr)`) is the **wireless** Lancool 217 case fronted by the LWireless hub, not the wired 207.

##### Transports

slv3 wraps three wire transports plus two non-wire integrations.

| Transport class | Wire protocol | VID / PID(s) | Endpoint / signature | Buffer / chunking | Used by |
|---|---|---|---|---|---|
| `HIDUsb.cs` | HID (HidLibrary) | VID `0x1A86` (6790), PID `0x2107` (8455) - `HIDUsb.cs:13-15` | 64-byte HID reports, ReportId = 0 - `HIDUsb.cs:79-83,217-219` | 64 B in, 64 B out | "Mac map" probe of CH340-class hubs. Provides `(USB-port -> MAC)` table. Cmd `0x1C` (28) only. **Not** a per-device LCD transport. |
| `WinUsb.cs` | LibUsbDotNet WinUSB / interrupt endpoints | LCD: VID `0x1CBE` (7358), PID 5 or 6; RF receiver: VID `0x0416` (1046) PID `0x8041` or VID `0x1A86` PID `0xE305`; RF sender: VID `0x0416` PID `0x8040` or VID `0x1A86` PID `0xE304`; 8.8" LED side: VID `0x0416` PID `0x8050` (32848) | `WriteEndpointID.Ep01` writer + `ReadEndpointID.Ep01` interrupt reader, 512-byte read packets | LCD path: 102,400-byte buffer, encrypted 504-byte header (DES-CBC, key `slv3tuzx`) at offset 0 + raw image bytes at offset 512. RF path: 64-byte raw frames, no encryption. 8.8" LED path: raw write through `Led_Send`. | SLV3LCD, TLV2LCD, UniversalScreen 8.8" LED side, RF dongles. |
| `RFController.cs` (logical) over `WinUsb` (physical) | Lian-Li proprietary 2.4 GHz RF, encapsulated in 64-byte WinUSB frames | (uses RF sender dongle above) | `MasterDevice.SendRfData` builds a 64-byte USB frame: `[0]=0x10 (Usb_SendRf)`, `[1]=fragment_seq`, `[2]=channel`, `[3]=rx_type`, `[4..63]=60-byte RF-payload chunk`. RF-level payload (240 B+) is fragmented into 60-B chunks. | `MasterDevice.SyncMasterClock`, `SendAioInfo`, `SendPicDataOnce`, `Update`, `QuerryMasterMac`, etc. | LWireless hub (Strimer wireless, TL/SLV3 wireless fans, wireless HS II LCD, Lancool 217, V150, Led88, etc.). |
| `OpenRGBController.cs` | OpenRGB SDK (TCP, "ORGB" magic) | TCP loopback, default port **6742** | TCP socket | One-shot RGB sync to third-party RGB devices. | Universal RGB sync feature. |
| `UdpServer.cs` | UDP server, port **61447** | localhost UDP | 64-byte command frames; `UdpCmdTypes` enum: `GetLst=101, SetLightEffect=102, SaveEffect=103, StartRecord=104, StopRecord=105` | Local IPC for in-process effect record/replay. Not device-facing. | |

##### USB command table (USB_CMD.cs)

Citations: `USB_CMD.cs:5-14`. Values are flat USB-frame opcodes sent over the `WinUsb` RF-dongle transport.

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| `0x0E` (14) | `Usb_SendRfLong` | host -> dongle | (RGB effect machinery) | `USB_CMD.cs:14`, `RgbEffect.cs:64465,64897` | Long-form RF-payload variant. |
| `0x10` (16) | `Usb_SendRf` | host -> dongle | every RF write | `USB_CMD.cs:8`, `MasterDevice.cs:955` | Standard RF-fragment carrier. |
| `0x11` (17) | `USB_GetMac` | host <-> dongle | `MasterDevice.QuerryMasterMac` | `USB_CMD.cs:7`, `MasterDevice.cs:209-238` | Reads master dongle's own 6-byte MAC + 4-byte tmos clock + 2-byte version. |
| `0x12` (18) | `USB_SendHeart` | host -> dongle | (heartbeat, not seen actively used) | `USB_CMD.cs:6` | Likely vestigial / firmware-side heartbeat. |
| `0x15` (21) | `USB_ResetAnother` | host -> dongle | `MasterDevice.ResetRx` / `ResetTx` | `USB_CMD.cs:5`, `MasterDevice.cs:169,192` | Tells local dongle to reset its peer. |
| `0x16` (22) | (no enum entry; `CloseRecUsb` / `CloseSenderUsb`) | host -> dongle | `MasterDevice.cs:973,981` | | Close-current-USB-session. Raw byte. |
| `0x20` (32) | `Usb_FwErase` | host -> dongle | `MasterDevice.Update` | `USB_CMD.cs:9`, `MasterDevice.cs:1453` | Step 1 of dongle firmware update. |
| `0x21` (33) | `Usb_FwUpload` | host -> dongle | `MasterDevice.Update` | `USB_CMD.cs:10`, `MasterDevice.cs:1461-1487` | Step 2 of dongle firmware update; 4096-B page-aligned. |
| `0x22` (34) | `Usb_FwVerify` | host -> dongle | (declared, not seen used) | `USB_CMD.cs:11` | |
| `0x23` (35) | `Usb_FwReboot` | host -> dongle | `MasterDevice.Update` / `ResetAll` | `USB_CMD.cs:12`, `MasterDevice.cs:1504,1514` | Final step - reboot into new firmware. |
| `0x25` (37) | `Usb_FwGetInfo` | host -> dongle | (declared, not seen actively used) | `USB_CMD.cs:13` | |
| `0x1C` (28) | (no enum entry; HID MAC probe) | host -> CH340 hub | `HIDUsb.GetMac` | `HIDUsb.cs:79` | Only the HID transport uses 28; raw byte. |

##### LCD WinUSB command table (`slv3.models.CmdType`)

The opcode lives at `header[0]` *before* DES-CBC encryption. Citations: `CmdType.cs:5-17`.

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| `0x0A` (10) | `GetVer` | host -> device, response | `WinUsb.GetVer`, `SendHeartBeat` | `CmdType.cs:5`, `WinUsb.cs:1219-1228,1238-1257` | `[0]=0x0A`, `[8..40]` = UTF-8 version string. |
| `0x0B` (11) | `Reboot` | host -> device | `WinUsb.Reboot` | `CmdType.cs:6`, `WinUsb.cs:1206-1212` | |
| `0x0D` (13) | `Rotate` | host -> device | `STRController.Rotate` -> `WinUsb.Send(CmdType.Rotate, byte)` | `CmdType.cs:7`, `STRController.cs:1337,1346`, `WinUsb.cs:1030` | `header[8]` = rotation enum (0-3). |
| `0x0E` (14) | `BrigthSet` | host -> device | `STRController.SetBright` (line 1413) | `CmdType.cs:8` | Single-byte payload at `header[8]`. |
| `0x0F` (15) | `SetFrameRate` | host -> device | `WinUsb.SetFrameRate` (always called with 120) | `CmdType.cs:9`, `WinUsb.cs:637,1233-1236` | Always sends 120 fps regardless of arg. |
| `0x28` (40) | `UpdateFireWare` | host -> device | `WinUsb.UpdateFile` (firmware **and** boot.jpg) | `CmdType.cs:10`, `WinUsb.cs:1083-1204` | **Repurposed as a generic filesystem-write opcode.** Targets: `/usr/data/update.app` (FW), `/usr/data/boot.jpg` (stored boot logo). |
| `0x29` (41) | `StopClock` | host -> device | (declared) | `CmdType.cs:11` | Not seen used in `WinUsb.cs` / `STRController.cs`. |
| `0x65` (101) | `PushJpg` | host -> device | every transient frame | `CmdType.cs:12`, `WinUsb.cs:718-728,742`, `STRController.cs:186,263,321,1305,1924,1978,...` | Transient frame upload. **No persistence.** |
| `0x79` (121) | `StartPlay` | host -> device | (declared) | `CmdType.cs:13` | Likely on-device animation playback. |
| `0x7A` (122) | `QueryBlock` | host -> device | (declared) | `CmdType.cs:14` | |
| `0x7B` (123) | `StopPlay` | host -> device | (declared) | `CmdType.cs:15` | |
| `0xC9` (201) | `GetPosIndex` | host -> device, response | `WinUsb.GetPosIndex` | `CmdType.cs:16`, `WinUsb.cs:1259-1287` | Returns group index. |
| `0xFD` (253) | `FactoryH264Test` | host -> device | `STRController.FactoryH264Test` | `CmdType.cs:17`, `STRController.cs:1391` | Factory diagnostic. |

##### RF command table (RF_CMD.cs)

Citations: `RF_CMD.cs:5-19`. Each value is the second byte of the RF payload.

| Cmd byte | Name | Used by | Citation | Notes |
|---|---|---|---|---|
| `0x10` (16) | `RF_Bind` | `RFController.Bind` / `BindAll` | `RF_CMD.cs:5`, `RFController.cs:1702,1794` | Pair a wireless device to the master MAC. |
| `0x12` (18) | `RF_Select` | every per-device write | `RF_CMD.cs:6`, `MasterDevice.cs:337,362,532,569,587,...,939` | Outermost RF "envelope" byte. |
| `0x13` (19) | `RF_Print` | (declared; debug print) | `RF_CMD.cs:7` | Likely device-side log echo. |
| `0x14` (20) | `RF_ClockSync` | `MasterDevice.SyncMasterClock` | `RF_CMD.cs:8`, `MasterDevice.cs:939-942` | Broadcast. |
| `0x15` (21) | `RF_SaveCfg` | `MasterDevice.SaveConfig` | `RF_CMD.cs:9` | Asks devices to flush in-RAM config to flash. |
| `0x16` (22) | `RF_RebootLcd` | `RFController.RebootLcdGroup` | `RF_CMD.cs:10`, `RFController.cs:1592` | |
| `0x17` (23) | `RF_UpdatePre` | `MasterDevice.Update` (sub-device FW) | `RF_CMD.cs:11`, `RFController.cs:1940` | Pre-flight before sub-device firmware streaming. |
| `0x18` (24) | `RF_UpdateData` | sub-device FW upload chunks | `RF_CMD.cs:12` | |
| `0x19` (25) | `RF_SwitchWirelessTheme` | `RFController.SwitchAioLcdWirelessMode` | `RF_CMD.cs:13`, `RFController.cs:1607` | |
| `0x20` (32) | `RF_RgbSync` | `MasterDevice.SyncRgbData` | `RF_CMD.cs:14`, `MasterDevice.cs:831` | Bulk RGB-frame sync. |
| `0x21` (33) | `RF_UpdateAio` | `MasterDevice.SendAioInfo` | `RF_CMD.cs:15`, `MasterDevice.cs:362` | Sends 32-B `aio_param` block. |
| `0x22` (34) | `RF_SendPic` | `MasterDevice.SendPicDataOnce` | `RF_CMD.cs:16`, `MasterDevice.cs:337,386,391` | **Wireless AIO LCD image upload.** |
| `0x23` (35) | `RF_217CloseWifi` | `RFController.Close217Wifi` | `RF_CMD.cs:17`, `RFController.cs:174` | Lancool 217-specific. |
| `0x24` (36) | `RF_V150MBSyncSwitch` | `RFController.MBSyncSwitch` | `RF_CMD.cs:18`, `RFController.cs:191` | V150-specific. |
| `0x26` (38) | `RF_V150LightSyncSwitch` | `RFController.LightSwitch` | `RF_CMD.cs:19`, `RFController.cs:209` | V150-specific. |

##### Frame upload mechanism (cmd-15 hypothesis answer)

This resolves the LCD-frame-upload opacity flagged in the SLV3LCD, HS II LCD, and UniversalScreen surveys. slv3 has two distinct image-upload pipelines:

###### Pipeline A: wired LCD (SLV3LCD / TLV2LCD)

1. **Entry point.** `STRController.ShowAndSaveImg(string filePath)` (`STRController.cs:1303-1318`).
2. **Transient send.** `STRController.PushImg(filePath)` -> `Util.GetJpgBytes(ResizeImage(bmp, 400, 400), quality=95)` -> `winUsb.PushJpg(jpgBytes)` -> `WinUsb.Send(CmdType.PushJpg=0x65, bytes, IsImg=true)`.
3. **Wire format of `PushJpg`:** allocate 102,400-B output buffer, build 504-B header `[0]=0x65, [2]=0x1A, [3]=0x6D, [4..7]=LE uint32 ms-since-app-start`, encode body length BE into `[8..11]`, copy body to offset 512, **DES-CBC encrypt header** with key `slv3tuzx` and IV = key, write over WinUSB EP1 OUT.
4. **Stored-logo send (delayed, conditional).** `ShowAndSaveImg` schedules a Task that sleeps 5000 ms, then if no newer `PushImg` has happened, calls `UploadLogo(lastPath)` -> `winUsb.UpdateFile(MemoryStream, "/usr/data/boot.jpg")`.
5. **Wire format of `UpdateFile`:** opcode `0x28` (`UpdateFireWare = 40`). Header: `[0]=0x28`, `[8..11]=BE filename_len`, `[12..15]=BE body_len`, `[16..]=ASCII filename`. Body at offset 512. Same DES-CBC encryption.

###### Pipeline B: wireless AIO LCD (over RF, no stored variant)

1. **Entry point.** `RFController.SendAioPic(macstr, byte[] imgData)` (`RFController.cs:723`).
2. **Internal queueing.** Sets `rfd.imageData` and `rfd.aio_send_image = true`. The 1-Hz poll in `MasterDevice.Run` triggers `SendAioInfo()`.
3. **Chunking.** `SendAioInfo` (`MasterDevice.cs:351-403`): splits `imageData` into 220-B chunks; final chunk uses `chunk_index = 0xFF` and first two bytes carry BE total-image-length.
4. **Wire format of `SendPicDataOnce`:** 240-B RF payload `[0]=0x12 (RF_Select), [1]=0x22 (RF_SendPic), [2..7]=device_mac, [8..13]=master_mac, ...`, fragmented into 60-B WinUSB chunks under cmd `0x10 Usb_SendRf`.
5. **No stored variant** at the slv3 level - wireless AIO LCDs persist via `RF_SaveCfg` (cmd 0x15) on a periodic schedule.

###### Cmd-15-hypothesis answer

**Yes**, slv3 has a transient-vs-stored split for wired-LCD families, using a different mechanism than LCDV2 / TLLCD:

- **Transient frame** = `CmdType.PushJpg = 0x65` (101). Every frame for animation / video / sensor display. No persistence.
- **Stored boot logo** = `CmdType.UpdateFireWare = 0x28` (40), with target filename `/usr/data/boot.jpg`. *Same opcode as firmware update, distinguished by filename in header `[16..]`*.
- Trigger heuristic: 5 s of inactivity after the most recent `PushJpg` triggers one auto-`UpdateFile("/usr/data/boot.jpg")` per session. The user does **not** explicitly choose to save.

Wireless AIO LCDs (Pipeline B) have **no stored variant**: only transient `RF_SendPic` chunks; persistence happens via per-device on-board flash on `RF_SaveCfg`.

##### Master-device abstraction

- `MasterDev.cs` is a 12-line stub - **not** the device-management surface (decompilation artifact).
- `MasterDevice.cs` (1534 lines) is the real model. Represents the RF receiver/sender pair (two USB dongles), not an end device.
- `RfDevice` is the per-peer model: `MacStr`, `MasterMacStr`, `BindStatus`, `RecType[]`, etc.
- `STRController` is parallel device-management for wired LCDs (SLV3LCD / TLV2LCD).

##### slv3.models contents

44 `.cs` files. Highlights:
- Per-family RGB effect ID enums: `Slv3Effects` (25 effects), `H2Effects`, `CLEffects`, `LC217Effects`, `SLINFEffects`, `StrEffects`, `StrSingleEffects`, `TLEffects`, `UIEffects`.
- `LightEffectSet` - generic light-effect carrier.
- `AioParams` (`AioParams.cs:5-69`) - 32-B AIO display params struct.
- `DevTypes` - `Strimer=1, WaterBlock=10, WaterBlock2=11, SLV3Fan=20, TLV2Fan=28, SLINF=36, RL120=40, CLV1=41, LC217=65, V150=66, Led88=88, OpenRgbDev=99`.

##### Notable findings

1. **The "cmd 15" question for slv3 families is answered.** Wired LCDs: transient = `PushJpg=0x65`, stored = `UpdateFireWare=0x28` with filename `/usr/data/boot.jpg`. Wireless AIO LCDs: transient only (`RF_SendPic=0x22` chunks).
2. **DES-CBC encryption of the LCD command header is novel.** Key + IV = `slv3tuzx`. Key + IV are static and shipped in plaintext in the DLL - anti-tamper signalling rather than confidentiality.
3. **Dual-VID dongles.** RF receiver/sender each have two VID/PID variants. Older: VID `0x0416` (1046, "Winbond") with PIDs `0x8040`/`0x8041`/`0x8050`. Newer: VID `0x1A86` (6790, "QinHeng/CH340") with PIDs `0xE304`/`0xE305`. The Linux udev rule will need both pairs.
4. **OpenRGB is a first-class transport, not a workaround.** `DevTypes.OpenRgbDev=99` makes it a real device class.
5. **HID transport is *not* what we expected.** The HID path (VID `0x1A86` / PID `0x2107`) is a CH340-class USB hub MAC probe, not a per-device LCD transport. None of the slv3-using families speak HID for primary protocol - they all speak WinUSB (LCD) or WinUSB-encapsulated RF (wireless).
6. **`SendRfData` is the universal RF chunker.** Implement one chunker, dispatch all 15 RF cmds through it.
7. **`MasterDev.cs` and `MasterDevice.cs` are unrelated despite the name.** Decompilation artifact.

##### Open questions

- `CmdType.StartPlay=121` / `StopPlay=123` / `QueryBlock=122` declared but no call sites found in inspected scope.
- `Usb_SendHeart=18` declared but no USB-layer caller found.
- `Usb_FwVerify=34` declared but no caller in `MasterDevice.Update`.
- Status of encrypted header on RF transport - confirmed unencrypted on RF; LCD WinUSB only.
- 8.8" LED transport (`Get88inch_LED`, `Led_Send`, `Led_Read`) uses unencrypted raw writes.
- `OpenRGBController.cs` request encoding only confirmed for connection setup.
- HS II LCD wired path opcodes - resolved by section 3.2.2 (lcd207-sdk).
- UniversalScreen 8.8" panel-side transport - resolved by section 3.2.2 (lcd207-sdk).

> **Typo fix applied:** the slv3-sdk staging file referred to UniversalScreen as "VID 1046 / PID 32848 = `0x0426` / `0x8050`". The hex form is corrected here to `0x0416` / `0x8050` (1046 = `0x0416`).

#### 3.2.2 lcd207 SDK

**Source:** decompiled `lcd207.dll` at `/home/inky/Development/lian-li/lcd207/lcd207/`.

**Status:** all citations are `inferred-static`.

**Used by these families** (confirmed by L-Connect-Service `using lcd207;` directives):

- **HydroShift II LCD Circle** - `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2CSDK.cs:1-187` is a thin pass-through to `lcd207.H2Controller`.
- **HydroShift II LCD Square** - `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2SSDK.cs:1-187` is a thin pass-through to `lcd207.H2SController`.
- **Lancool 207 Digital** - `L-Connect-Service/LConnectService/Products/Lancool207Digital/Lancool207DigitalController.cs:1-2` calls `lcd207.LCDController` methods.
- **UniversalScreen 8.8"** - `L-Connect-Service/LConnectService/Products/UniversalScreen8p8Inch/UniversalScreen8p8InchController.cs:1` (`using lcd207;`); the `sdkController` field is typed `LEDController`. **This confirms the open question raised in the per-family staging files** - UniversalScreen 8.8" is also a lcd207 client, on the LCD side (PID `0xA088`) plus an LED side (PID `0x8050` VID `0x0416`) and a HID-side wireless-sensor companion (PID `0xAD21` VID `0x1A86`).
- **L-Connect-Service shared logger** - `L-Connect-Service/LConnectService/Products/LCD207DLLLoggerWrapper.cs` adapts the SDK's `Loger.cs` to Serilog.

The four product families plus the legacy-wrapper share **one** `CmdType` enum, **one** DES-CBC encryption envelope (key `slv3tuzx`, identical to slv3), and three fan-out transport classes: `WinUsb` (Lancool 207 LCD), `WinUsbH2` (HS II Circle), `WinUsbH2S` (HS II Square). `WinUsbLed` is a sub-class of `WinUsb` for the UniversalScreen 8.8" LED-side. `HIDUsb` is independent of the encrypted CmdType protocol.

##### Transports

| Transport class | Wire protocol | VID / PID(s) | Endpoint / signature | Buffer / chunking | Used by |
|---|---|---|---|---|---|
| `HIDUsb.cs` | HID (HidLibrary) | VID `0x1A86`; PID `0xAD21` (UniversalScreen 8.8"), `0xAD20` (HS II), `0xAD11` (Lancool 207) | 64-byte HID reports, ReportId = 0 | 64 B in, 64 B out. SN discovery: probe writes `[0xAA, 0x55, 0x35]` and reads SN as ASCII at offset 2..18. `SetMonitorMode` writes magic 8-byte string `35 66 33 37 35 39 64 66` ("5f3759df"). | Out-of-band SN probe and desktop-mode toggle. Not the per-frame transport. |
| `WinUsb.cs` | LibUsbDotNet WinUSB / interrupt endpoints | VID `0x1CBE`, PID `0xA065` (Lancool 207) | `Ep01` writer + `Ep01` interrupt reader, 512-byte packets | 102,400+ B output buffer; encrypted 504-B header + raw image body at offset 512. H264 video-stream chunking: 202,752 B per `Read` slice fed under cmd `StartPlay = 0x79`. | Lancool 207 Digital LCD. |
| `WinUsbH2.cs` | LibUsbDotNet WinUSB | VID `0x1CBE`, PID `0xA021` | Same Ep01 writer + Ep01 reader | Same envelope as `WinUsb`. Adds in-process FIFO `msgQueue` polled at 330 ms cadence with periodic `GetH2Params` and `SyncPumpFan`. | HydroShift II LCD Circle. |
| `WinUsbH2S.cs` | LibUsbDotNet WinUSB | VID `0x1CBE`, PID `0xA034` | Same as `WinUsbH2` | Identical wire-level format to `WinUsbH2` (only PID, class-name, log-string, and pump-RPM-to-PWM mapping table differ). | HydroShift II LCD Square. |
| `WinUsbLed.cs` | LibUsbDotNet WinUSB (subclasses `WinUsb`) | LCD-side: VID `0x1CBE`, PID `0xA088`. LED-side: VID `0x0416`, PID `0x8050`. | Inherits Ep01 writer/reader. `IsLcd = true` for LCD-side device. | LCD-side: encrypted 504-B header (inherited). LED-side: raw 64-B frames, no encryption. | UniversalScreen 8.8" panel + LED side. |

`WinUsbH2.SendPumpFan` / `WinUsbH2S.SendPumpFan` call `slv3.RFController.GetAllGroup()` to **skip** local pump-fan sync if a wireless waterblock peer is bound. This is the only behavioural cross-link between the two SDKs.

##### USB command table (CmdType.cs)

Citations: `CmdType.cs:5-25`. Header layout: `[0]=cmd, [2]=0x1A, [3]=0x6D, [4..7]=LE uint32 ms-since-app-start`. Body length at `[8..11]` BE for single-buffer commands or `[12..15]` BE for two-buffer commands.

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| `0x0A` (10) | `GetVer` | host -> device, response | `WinUsb.GetVer`, `WinUsbH2.GetVer`, `WinUsbH2S.GetVer` | `CmdType.cs:5`, `WinUsb.cs:777-789`, `WinUsbH2.cs:855-867` | Reply: `[0]=0x0A`, `[8..40]` = UTF-8 version string. |
| `0x0B` (11) | `Reboot` | host -> device | `Reboot()` after FW write; `SetDesktopMode` | `CmdType.cs:6`, `WinUsb.cs:717-721`, `H2Controller.cs:1129` | |
| `0x0D` (13) | `Rotate` | host -> device | `H2Controller.Rotate`, `H2SController.Rotate`, `LCDController.Rotate`, `LEDController.Rotate` | `CmdType.cs:7`, `H2Controller.cs:1659`, etc. | `header[8]` = rotation enum byte (0-3). |
| `0x0E` (14) | `BrigthSet` | host -> device, response | every transport's `SetBrightness` | `CmdType.cs:8`, `H2Controller.cs:1675`, etc. | `header[8]` = brightness 0-100. |
| `0x0F` (15) | `SetFrameRate` | host -> device | every transport's `SetFrameRate` and `InitDev` | `CmdType.cs:9`, `WinUsb.cs:905-909`, etc. | Sets device-side decode FPS for H264 playback. |
| `0x28` (40) | `UpdateFireWare` | host -> device | `WinUsb.UpdateFile` (firmware **and** boot.jpg) | `CmdType.cs:10`, `WinUsb.cs:633-715`, `WinUsbH2.cs:711-793` | **Repurposed as a generic filesystem-write opcode** (identical to slv3's pattern). |
| `0x2A` (42) | `DelFile` | host -> device | `WinUsb.DelFileOrDir` and equivalents | `CmdType.cs:11`, `WinUsb.cs:723-733` | `header[0..3]=BE path_len`, `header[8..]=ASCII path`. |
| `0x33` (51) | `SetClock` | host -> device, response | `WinUsb.SyncClock` family | `CmdType.cs:12`, `WinUsb.cs:745-770` | Sets device RTC. |
| `0x34` (52) | `StopClock` | host -> device | `WinUsb.StopClock`, `H2Controller.ApplyTemplate` | `CmdType.cs:13`, `WinUsb.cs:772-775`, `H2Controller.cs:1209` | Suppresses device-side built-in clock face. |
| `0x63` (99) | `QueryDir` | host -> device, response | `WinUsbH2.QuerryDir`, `WinUsbH2S.QuerryDir` | `CmdType.cs:14`, `WinUsbH2.cs:313-329` | Lists a device-side directory. |
| `0x65` (101) | `PushJpg` | host -> device | every transient JPG frame upload | `CmdType.cs:15`, `WinUsb.cs:509-583`, `WinUsbH2.cs:587-661` | **Transient frame upload**. Co-opts opcode 102 (raw byte, no enum entry) for PNG variant. |
| `0x79` (121) | `StartPlay` | host -> device | `WinUsb.PlayH264`, `WinUsbH2.PlayH264`, `WinUsbH2S.PlayH264` | `CmdType.cs:16`, `WinUsb.cs:335-407` | **H264 video-frame upload**. 202,752 B per chunk. |
| `0x7A` (122) | `QueryBlock` | host -> device, response | `waitBuffer` / `waitBufferAsync` | `CmdType.cs:17`, `WinUsb.cs:313-327` | Returns device's outstanding-block count. |
| `0x7B` (123) | `StopPlay` | host -> device | `WinUsb.StopPlay`, `H2Controller.PlayStop` | `CmdType.cs:18`, `WinUsb.cs:329-333` | Cancels H264 playback. |
| `0x96` (150) | `SwitchToDesktop` | host -> device, response | `WinUsb.SwitchToDesktop` | `CmdType.cs:19`, `WinUsb.cs:735-743`, `H2Controller.cs:1128` | Switches device into "desktop mode". |
| `0xC9` (201) | `GetPosIndex` | host -> device, response | (declared) | `CmdType.cs:20` | No caller in lcd207. |
| `0xF9` (249) | `SetWthemeIndex` | host -> device | `WinUsbH2.SetAioParamsByUSB`, `WinUsbH2S.SetAioParamsByUSB` | `CmdType.cs:21`, `WinUsbH2.cs:1067` | **HS II built-in theme + AIO sensor display config**. 32-byte payload. |
| `0xFA` (250) | `GetH2Params` | host -> device, response | `WinUsbH2.GetH2Params`, `WinUsbH2S.GetH2Params` | `CmdType.cs:22`, `WinUsbH2.cs:981-984` | Reads 32-B AIO sensor block. Polled every ~3.3 s. |
| `0xFB` (251) | `SyncPumpFan` | host -> device | `WinUsbH2.SendPumpFan`, `WinUsbH2S.SendPumpFan` | `CmdType.cs:23`, `WinUsbH2.cs:1134` | **Wired pump+fan PWM sync**. 16-B payload. Suppressed if wireless waterblock bound. |
| `0xFC` (252) | `PushRgbData` | host -> device | `WinUsbH2.UpdateRgbData`, `WinUsbH2S.UpdateRgbData` | `CmdType.cs:24`, `WinUsbH2.cs:1216-1244` | RGB frame upload for AIO's onboard LEDs. |
| `0xFD` (253) | `FactoryH264Test` | host -> device | (declared) | `CmdType.cs:25` | No caller in lcd207. |
| `0xF7` (247) | (no enum entry) | host -> device | `WinUsbH2.UpdateMcuFirmware`, `WinUsbH2S.UpdateMcuFirmware` | `WinUsbH2.cs:1161` | **MCU firmware update prep / "enter bootloader"**. Cast `(CmdType)247`. |
| `0xF8` (248) | (no enum entry) | host -> device | MCU firmware data chunks | `WinUsbH2.cs:1199` | **MCU firmware data chunk**. 4096-B chunks with CRC-16 trailer. |

**Cross-reference:** the lcd207 `CmdType` enum is structurally aligned with the slv3 `CmdType` enum (same opcodes for `GetVer`, `Reboot`, `Rotate`, `BrigthSet`, `SetFrameRate`, `UpdateFireWare`, `PushJpg`, `StartPlay`, `QueryBlock`, `StopPlay`, `GetPosIndex`, `FactoryH264Test`) and adds AIO-specific opcodes. **Strong evidence that lcd207 is a fork/refactor of slv3's `WinUsb.cs`**.

##### Frame upload mechanism (the cmd-15 hypothesis answer for HS II)

###### HS II Circle / Square - JPG / PNG transient pipeline

1. **Service-layer entry point.** L-Connect-Service's `HydroShiftIILCDController` dispatches to `H2CSDK.ApplyTemplate(theme)` (Circle) or `H2SSDK.ApplyTemplate(theme)` (Square); both pass through to `lcd207.H2Controller.ApplyTemplate(theme)` / `lcd207.H2SController.ApplyTemplate(theme)`.
2. **SDK orchestration** (`H2Controller.cs:1196-1264`): `SyncClock` -> `StopClock` -> `ClearPngLayer` (PNG cmd 102) -> `ClearJpgLayer` (`PushJpg=0x65`) -> if `theme.videoPath` set, `PlayH264(text)` (cmd `StartPlay=0x79`); else if `theme.BgImg` set, encode JPG and `PushJpg`.
3. **Wire format of `PushImage(bytes, IsPng)`:** allocate `512 + bytes.Length` output buffer; `cmd = IsPng ? 102 : 101`; build 500-B header; encrypt 504-B header to 512 B output; copy body to offset 512; write over WinUSB EP1 OUT.
4. **Stored boot-logo path.** `H2Controller.UploadLogo(filePath)` (`H2Controller.cs:1880-1899`): resize bitmap, encode JPG, `winUsb.UpdateFile(MemoryStream, "/usr/data/boot.jpg")`. Cmd `UpdateFireWare=0x28` with filename in header `[16..]`.
5. **Firmware path.** `H2Controller.FirmwareUpdate(filePath)` -> `winUsb.UpdateFile(filePath, "/usr/data/update.app")`. Identical wire format, different filename. Followed by `Reboot()`.
6. **MCU firmware path.** `H2Controller.UpdateAioFirmware(path)` -> `winUsb.UpdateMcuFirmware(path)`. Uses opcodes 247 + 248 (undeclared in `CmdType`), 4096-B chunks with CRC-16 trailer.

###### HS II - H264 video-stream pipeline (transient, repeating)

`WinUsbH2.PlayH264(filePath, isRepeat)` reads up to `bufLen = 202,752` B per iteration, sends under cmd `StartPlay=0x79` with 30 ms pacing. If `ret[8] > 3` (back-pressure), poll `QueryBlock=0x7A` until device buffer drops to <= 2 outstanding blocks.

###### Cmd-15-hypothesis answer for HS II

**Yes - lcd207 has a transient-vs-stored split at the wire level, and it mirrors slv3's pattern exactly.** Confirmed for HS II Circle, HS II Square, and Lancool 207:

- **Transient JPG frame** = `PushJpg = 0x65` (101). No persistence.
- **Transient PNG frame** = raw byte `0x66` (102). Same wire format, different leading byte (no enum entry).
- **Transient H264 chunk** = `StartPlay = 0x79` (121). Body is a `.h264` elementary-stream slice.
- **Stored boot logo** = `UpdateFireWare = 0x28` (40), with target filename `/usr/data/boot.jpg`.
- **Stored firmware** = `UpdateFireWare = 0x28` (40), with filename `/usr/data/update.app`. Followed by `Reboot()`.
- **Stored MCU firmware (AIO controller)** = raw bytes 247 / 248 (undeclared in `CmdType`).

**The HS II per-family survey's wired-HID-transport opacity is closed.** HS II LCD does not use HID for frames - it uses WinUSB with DES-CBC-encrypted headers, exactly like Lancool 207. HID is only used for one-time SN discovery and the "switch into desktop pixel-streaming mode" magic-byte signal.

##### Lancool 207 Digital pipeline

Wire format **identical to HS II** at the byte level (same `CmdType`, same DES envelope, same body-at-offset-512 layout). Differences:

1. **Dimensions.** `LCDController.Width = 720`, `Height = 1472` (Lancool 207 panel resolution). HS II is 480x480; UniversalScreen 8.8" is 480x1920.
2. **No AIO-specific opcodes.** Lancool 207 does not use `SetWthemeIndex=0xF9`, `GetH2Params=0xFA`, `SyncPumpFan=0xFB`, or `PushRgbData=0xFC` - these are HS II-specific.
3. **Cross-process IPC.** L-Connect-Service uses `copyImageMemory` to ingest pre-rendered desktop captures from the L-Connect 3 GUI process. The lcd207 SDK is not involved in the IPC mechanism.

##### Theme / graph rendering helpers

This is a host-side widget-rendering pipeline that produces a `Bitmap` for each frame, then JPG/PNG-encoded and uploaded via `PushImage`. It is **prior art for Phase 4's theme engine design** but is not itself part of the wire protocol.

###### Graph hierarchy

`GraphItem` is the abstract base. Concrete subclasses: `GraphImage`, `GraphClock`, `GraphSensor`, `GraphLine`, `GraphArchBar`, `GraphStatuBar`, `GraphDynamicBar`, `GraphAnimation`. All override `Render(Graphics g, ...)`.

###### Theme model

`Theme` aggregates `GraphList` with a background `BgImg`, optional `videoPath`, dimensions, colours, and three discriminator enums:
- `ThemeTypes` - `Template`, `ThemeInfo`, `Modulars`.
- `ModularTypes` - `General`, `WireLessLcd`, `DataAndTime`, `CustomText`.
- `IsRemovable` / `isTempTheme`.

Render pipeline (per frame, host-side):
1. `LCDController.GetBitmapByTheme(theme)` allocates Bitmap, draws background.
2. For each `GraphItem`, calls `item.Render(g, ...)`.
3. Sensor values looked up via `M_Data.MaxVals: Dictionary<string, double>`.
4. Encode to JPG (quality 95) or PNG.
5. Push via `PushJpg` / `PushPng`.

**Phase 4 design note:** the `(Theme, GraphItem*)` model is a sound starting reference for the Linux port's theme engine, but **do not** mechanically transcribe - `Graph*` classes are tightly coupled to `System.Drawing.Graphics` (GDI+). A Linux port would naturally use Skia/Cairo/`tiny-skia` and need a re-design.

##### Notable findings

1. **The "what does `lcd207.dll` actually do?" question is fully answered.** It's a four-product LCD-rendering and AIO-control SDK that wraps four transports under one shared `CmdType` opcode space, one shared DES-CBC envelope, and one shared host-side theme-rendering pipeline.
2. **The HS II per-family survey's wired-HID-transport opacity is closed.** HS II LCD uses WinUSB (PID `0xA021` / `0xA034`), not HID, for frames.
3. **lcd207 and slv3 share the same DES-CBC envelope, key, and IV** (`slv3tuzx`). Strong evidence lcd207 was forked from slv3.
4. **HS II Circle vs HS II Square wire-level differences are minimal** (PID, class names, log strings, pump-RPM-to-PWM LUT). One Rust codec can serve both.
5. **lcd207 calls slv3 across SDKs** for the wired-vs-wireless waterblock priority decision.
6. **The "MCU firmware" vs "application firmware" distinction is real.** Two separate firmware-update paths with different opcodes and different chunking. **MCU FW path must NOT be invoked during normal testing** - bricking risk much higher than application FW.
7. **`WinUsbLed` is the only family that speaks an unencrypted protocol over the same writer/reader.** UniversalScreen 8.8"'s LED-side endpoint bypasses the DES envelope.

##### Open questions

- `CmdType.GetPosIndex = 201` declared but no caller located.
- `CmdType.FactoryH264Test = 253` declared, no caller.
- `Util.Compute(buffer, 0)` CRC algorithm not inspected line-by-line. Likely CRC-16/MODBUS or CRC-16/CCITT.
- `WinUsb.cs` Lancool 207 path missing `QuerryDir` method (only `WinUsbH2` / `WinUsbH2S` define it).
- `copyImageMemory` IPC source - decompilation choked on the implementation body.
- `HIDUsb.GetHidLst` SN-discovery probe payload firmware-side handling unknown.
- `SetMonitorMode` magic 8 bytes = `35 66 33 37 35 39 64 66` decode as ASCII `"5f3759df"` (Quake III's "fast inverse square root" magic constant). Used as host-authentication token.
- `H2SController.cs` was not read line-by-line. Spot-check during Phase-1 implementation.
- `startmp4` static field on `WinUsbH2` is shared across all H2 instances - concurrent multi-device race risk to **not** replicate in Linux port.
- `aioParamsTX[0] = 2080` initial value in `WinUsbH2.InitDev` doesn't fall on segment boundaries.

## 4. Cross-reference matrix

This section synthesizes the per-family writeups into device-by-device cross-tables. Status flags use the canonical vocabulary from section 2.

### 4.1 Lighting commands across families

One row per family, columns showing the lighting command bytes / framings exercised. Cells show the cmd byte and a short status note.

| Family | Preset-mode cmd (pump) | Preset-mode cmd (fan/ring) | Per-pixel cmd | Mode list query | Framing | Status |
|---|---|---|---|---|---|---|
| Galahad2 Vision (LCDV2) | 131 (`SetPumpLighting`, 19-byte payload) | 133 (`SetFanLighting`, 20-byte payload) | 20 (`SetLEDColors`, `isFanLed` flag) | 130 / 132 (escape-byte `0xA5 0x5A`) | A | verified-live (baseline) |
| Galahad2 Trinity | 131 (same offset layout, `% 1000` mode folding, 19-byte payload) | 133 (same offset layout, 20-byte payload) | not implemented | 130 / 132 (declared-only) | A | inferred-static (mode-byte set diverges from LCDV2) |
| Galahad2 LCD (older) | unknown | unknown | unknown | unknown | unknown | unclear (no device class in source) |
| HydroShift LCD (HS I) | **absent** (no 130/131 in `Commands.cs`) | 133 (20-byte payload, full 16-mode set) | 20 (flat `[0x01]` flag, no `isFanLed` branch, **diverges from LCDV2**) | 132 only | A + B | inferred-static |
| HydroShift II LCD | opaque (lcd207-sdk; `H2Effects` enum) | opaque (lcd207-sdk; `CLEffects` enum) | opaque (`PushRgbData=0xFC` for AIO LEDs) | opaque | WinUSB+DES | inferred-static |
| TLLCD | N/A (LCD-only family) | N/A | N/A | N/A | T | (lighting on sibling TLFan) |
| TLFan | N/A (no pump) | 163 (`SetFanLight`, 20-byte payload, per-fan addressing) + 176 (`SetFanGroupLight`) | not implemented (preset-mode + 4-color seed only) | 162 (declared-only) | A | inferred-static |
| Strimer Plus | N/A | composite `0x10 \| port` (`SetEffectSetting`, 6-byte) | composite `0x30 \| port` (`SetColorSetting`, R-B-G byte order, output report) | N/A (no mode-list query) | HID-feature `0xE0` | inferred-static (R-B-G confirmed by OpenRGB) |
| Ene6K77Fan | N/A | composite `0x10 \| port, mode` (6-byte feature report) | composite `0x30 \| port` (R-B-G byte order, output report) | N/A | HID-feature `0xE0` | inferred-static |
| SLV3LCD | (separate folder `LWireless/SLV3/`) | (separate folder) | opaque (slv3-sdk) | opaque | JSON-IPC + slv3-sdk | inferred-static |
| LWireless | opaque (`SetLightEffect` / `SetUIEffect`) | opaque (per-MAC) | inferred (no raw-LED entry surfaced) | opaque | JSON-IPC + slv3-sdk | inferred-static |
| Lancool 207 Digital | N/A (LCD-only) | N/A | N/A | N/A | WinUSB+DES (lcd207-sdk) | inferred-static |
| UniversalScreen 8.8" (lighting board) | N/A | OpenRGB-confirmed `0x11`-header bulk EP1 chunks, 20 LEDs/chunk, RGB byte order | (host-rendered template only at SDK layer) | N/A | bulk-EP1 (separate from lcd207-sdk LCD path) | inferred-static + OpenRGB-validated |

### 4.2 LCD-frame commands across families - cmd-15 transient-vs-stored resolution

The cmd-15 hypothesis is now **closed**. The transient-vs-stored split exists in multiple families with different opcodes:

| Family | Transient frame | Stored boot/persistent | Activation mechanism | Status |
|---|---|---|---|---|
| Galahad2 Vision (LCDV2) | cmd 14 `SendJpeg` (~20 s firmware-side hold) | cmd 15 `UploadJpeg` (codec accepted, ACK'd, no visible activation on FW 1.6) | **unknown - the LCDV2 cmd-15 activation path is still open**. Possibly via sibling firmware/activate command, or via `LCDControlMode` slot-selection trigger (TLLCD-style) | verified-live (cmd 14); unclear (cmd 15) |
| TLLCD | cmd 65 `WriteJPG`, cmd 69 `WriteAVI` | cmd 71 `WriteBootAVI`, cmd 72 `WriteBootJPG` (explicit `Boot` qualifier) | cmd 64 `LCDControl` with `Mode=ShowJPG` and `JPGIndex=N` selects stored slot | inferred-static (clean transient/stored pair) |
| HydroShift LCD (HS I) | cmd 14 / 15 (enum-only, no call site) + cmd 13 `SendH264Frame` | cmd 22 `UploadStartupAviMovie` (explicit `Startup`, no call site visible) | unknown | unclear / inferred-static |
| slv3-using families (SLV3LCD, TLV2LCD, wired) | `CmdType.PushJpg = 0x65` (transient, body-at-offset-512, DES-encrypted header) | `CmdType.UpdateFireWare = 0x28` with filename `/usr/data/boot.jpg` (same opcode as firmware update, distinguished by filename) | 5 s of inactivity after most recent `PushJpg` triggers auto-`UpdateFile("/usr/data/boot.jpg")` per session | inferred-static |
| lcd207-using families (HS II Circle/Square + Lancool 207 + UniversalScreen panel) | identical to slv3 (`PushJpg=0x65` transient + raw `0x66` for PNG variant + `StartPlay=0x79` for H264) | identical to slv3 (`UpdateFireWare=0x28` with filename `/usr/data/boot.jpg`); same DES envelope; lcd207 is a fork of slv3's `WinUsb.cs` | same as slv3 (filename-keyed reuse of FW-update opcode) | inferred-static |
| Wireless AIO LCDs (over RF) | `RF_SendPic = 0x22` (220-byte chunks, last chunk index `0xFF`) | **no stored variant at slv3 level**; on-device persistence via `RF_SaveCfg = 0x15` periodic | not applicable (transient only) | inferred-static |

**LCDV2 cmd-15 next-steps:** the pattern across slv3 / lcd207 / TLLCD says cmd 15 should activate via either (a) a separate sibling activate command (like slv3/lcd207's filename-keyed filesystem-write pattern) or (b) a `LCDControlMode` / `JPGIndex` slot-selection trigger (per TLLCD's `LCDControlSetting.JPGIndex` finding). Hardware capture during a live USB session with cmd 15 + LCDControlMode permutations is required to close.

### 4.3 Fan PWM commands across families

| Family | Fan PWM cmd | Per-fan vs per-group | Wire encoding | Continuous re-issue cadence |
|---|---|---|---|---|
| Galahad2 Vision (LCDV2) | 139 (`SetFanPWM`) | per-group (always `group=0`) | `[group=0x00, percent_0..100]` | (not directly observed; lighting cmd 131 is the re-issued one) |
| Galahad2 Trinity | 139 (`SetFan`) | global | `[mb_sync_bool, percent_0..100]` (byte 0 differs from LCDV2) | unclear |
| HydroShift LCD (HS I) | 139 (`SetFanPWM`) | per-group (always `group=0`) | `[0x00, percent_0..100]` | inferred (mirrors LCDV2 service pattern) |
| HydroShift II LCD | opaque (`UpdatePumpFan(int pump, int fanSpeed1, int fanSpeed2, int fanSpeed3)` SDK call - 3 fan channels per pump unit) | per-channel (3 fans / unit) | opaque (wraps via `SyncPumpFan=0xFB` 16-B payload at lcd207 level) | 1 Hz pump speed; 0.5 Hz handshake; 0.5 Hz FW version |
| TLFan | 170 (`SetFanSpeed`) | per-fan (`(port<<4)\|fanIndex` byte) | `[(port<<4)\|fanIndex, percent_0..100]` | 1 Hz curve loop |
| Ene6K77Fan | tag `0x20\|group, 0x00, duty` | per-group (low nibble of byte 1) | `[0xE0, 0x20\|group, 0x00, duty]` | 1 Hz host-curve loop (service-side) |
| LWireless | opaque (`RFController.SetFansRPM(macStr, List<byte> duty)`) | per-MAC, 4 fans / device | host-side scaled `0..=100 -> 0..=255` then over RF | inferred (sensor loops poll and re-issue) |
| Strimer Plus / SLV3LCD / TLLCD / Lancool 207 / UniversalScreen | N/A | - | - | - |

### 4.4 Sensor / handshake commands across families

| Family | Handshake cmd | Response shape | Cadence |
|---|---|---|---|
| Galahad2 Vision (LCDV2) | 129 (`GetHandshakeInfo`) | 7 bytes: FanRpm BE u16 + PumpRpm BE u16 + TempIsValid u8 + TempInteger u8 + TempFraction u8 | service-driven |
| Galahad2 Trinity | 129 (`Handshake`) | 4 bytes: FanRpm BE u16 + PumpRpm BE u16 (**no temperature**) | unclear |
| HydroShift LCD (HS I) | 129 (`GetHandshakeInfo`) | 7 bytes (identical to LCDV2) | inferred 1 Hz |
| HydroShift II LCD | opaque (RequestType 0 `GetHandshakeInfo`); SDK reads 32-B AIO sensor block via `GetH2Params=0xFA` | opaque + 32 B AIO block (CPU/GPU temp/load + 4xu16 fan tachs + ...) | 0.5 Hz handshake; ~3.3 s per `GetH2Params` |
| TLLCD | 60 (`GetHandshakeInfo`) | 3 bytes: Mode u8 + FrameIndex BE u16 (**no RPM, no temp**; LCD playback state only) | unclear |
| TLFan | 161 (`Handshake`) | Nx3 bytes (one triple per detected fan): `(byte0=detected/upgrading/port/fanIndex, BE u16 fanSpeed)` | 1 Hz |
| Strimer Plus | N/A (no handshake-equivalent) | - | - |
| Ene6K77Fan | tag `0x50 0x00` (`GetFanSpeed`) | 8 or 9 bytes: 4 x BE u16 fan RPM (no temp/no pump) | service-driven |
| SLV3LCD | dispatch cmd 17 `GetRPM` (returns `List<int>`) | opaque | 1 Hz sensor refresh (host-side rendering) |
| LWireless | dispatch cmd 0/1 (`FanGroupStatus` / `WirelessDeviceStatus`); `GetAioTemp` | opaque (RfDevice list + temp) | inferred sensor loops |
| Lancool 207 Digital | dispatch cmd 6 `GetIsAvailable`; `LCDController.IsAvailable` | opaque | host-side timer |
| UniversalScreen 8.8" | dispatch cmd 28 `GetIsAvailable` | opaque (no on-device sensor read; host pushes values **to** SDK) | 1 Hz serial-number reader |

### 4.5 Maintenance commands across families (reset, firmware, identity)

| Family | Reset | Firmware version read | Firmware activate / upload | Serial number read / write |
|---|---|---|---|---|
| Galahad2 Vision (LCDV2) | cmd 142 `ResetDevice` (codec-only; `Length=1, Data[0]=0x01` ACK; USB drop 1-3 s) | cmd 134 `GetFirmwareVersion` (verified-live; two-packet ASCII demuxed by PacketNumber) | cmds 10/11 (`ActiveFirmware`/`UploadFirmware`) deferred | cmds 136/137 deferred |
| Galahad2 Trinity | none in enum | cmd 134 (two-packet ASCII; demuxed by sequential reads) | none | cmds 136/137 declared-only |
| HydroShift LCD (HS I) | cmd 142 `ResetDevice` (inferred-static; **destructive**) | cmd 134 (two-packet ASCII) | cmds 10/11 (`ActiveFirmware`/`UploadFirmware`) inferred-static (B-class) | cmds 136/137 declared-only |
| HydroShift II LCD | none exposed (`ResetProfile` is host-side only) | RequestType 17 (returns `lianlih2_NNNN_NNNN` Circle / `lianlih2s_NNNN_NNNN` Square) | RequestType 18/19 (deferred); MCU-FW path uses raw bytes 247/248 (lcd207-sdk) - **bricking risk** | none |
| TLLCD | none | cmd 61 `GetProductInfo` (two-packet ASCII; comma-separated 7 fields + datetime) | cmds 58/59 declared-only | cmds 62/63 (`ReadSerialNumber`/`WriteSerialNumber`) - **per-fan addressing**; cmd 63 destructive |
| TLFan | none | cmd 166 `GetProductInfo` (two-packet ASCII; demuxed by `Data[2]` index) | cmds 171/172 (LED MCU) + cmds 11/12 (controller MCU) all declared-only | cmds 168/169 declared-only |
| Strimer Plus | none | composite `0x50 0x00` (5-byte input report; 4-byte sentinel `0xE0 0x52 0xFF 0x40`) | not exposed | not exposed |
| Ene6K77Fan | none | composite `0x50 0x01` (5-byte input report; per-SKU `*FirmwareVersion::Load`) | not exposed | not exposed |
| SLV3LCD | dispatch cmd 16 `RestartLCD` (**destructive across whole LCD group**) | `LWirelessRequestType.GetLCDFirmwareVersion = 0` | `LWirelessRequestType.StartSLV3LCDUpgrade = 1` + progress poll | not exposed |
| LWireless | RequestType 18 `ResetDevice` declared-only (no handler) | RequestType 4 `FirmwareVersion` (returns hub TX/RX version pair) | inferred (`MasterDevice.Update` opcodes 32/33/35 in slv3) | not exposed |
| Lancool 207 Digital | none (only `ResetProfile` host-side) | RequestType 11 (returns `Lancool207DigitalFirmwareVersion`) | RequestType 12/13 (in-band, with progress poll) - **destructive** | not exposed |
| UniversalScreen 8.8" | none (only `ResetProfile` host-side; req 9) | RequestType 3 (returns `lianli88_NNNN_NNNN` panel + separate lighting MCU FW) | RequestType 4/5 (display-mode-blocked) | RequestType 0/1 (Get screen + lighting SN; read-only) |

### 4.6 VID-clustering as MCU-vendor signal

A clean MCU-vendor signal pattern emerged from cross-referencing the per-family VID/PID tables with the slv3-sdk and lcd207-sdk transports.

| VID | Hex | Vendor name | Lian-Li families using it |
|---|---|---|---|
| 0x0416 | (1046) | Winbond / Nuvoton | Galahad II family (Trinity, LCDV2, older LCD), HydroShift LCD (HS I), LWireless V1 hub (TX `0x8040`/RX `0x8041`), UniversalScreen lighting board (`0x8050`); legacy TLFan (`0x7372`) shares the same VID with `Usage = 0xFF1B` disambiguation |
| 0x0CF2 | (3314) | ENE Technology | StrimerPlus (`0xA200`), Ene6K77Fan family (`0xA100..0xA106`) |
| 0x04FC | (1276) | Sunplus | TLLCD only (`0x7393`) |
| 0x1CBE | (7358) | Lian-Li own assigned VID | SLV3LCD (`0x0005`), TLV2LCD (`0x0006`), HydroShift II LCD wired (`0xA021` Circle, `0xA034` Square), Lancool207Digital (`0xA065`), UniversalScreen 8.8" panel (`0xA088`) |
| 0x1A86 | (6790) | QinHeng / WCH | LWireless V2 hub (TX `0xE304`/RX `0xE305`), all DisplayMode endpoints (HS II LCD `0xAD20`/`0xAD22`, Lancool 207 `0xAD11`, UniversalScreen `0xAD21`); slv3-sdk's CH340 hub MAC-probe path (`0x2107`) |

**Implication for Linux daemon:** the udev rule will need to enumerate all five VIDs. The per-VID family clusters drive natural code-organization boundaries: a `winbond_*` codec module for Galahad / HydroShift / TLFan / LWireless V1, an `ene_*` module for StrimerPlus / Ene6K77Fan, a `sunplus_tllcd` module, a `lianli_lcd_*` module for the modern LCD family (consumes lcd207-sdk transport), and a `wch_*` module for V2 hub + DisplayMode endpoints. Cross-VID code reuse is limited - each cluster has its own framing class.

### 4.7 Continuous re-issue cadences

Per-family cadence findings the daemon-implementer needs for Phase 1's `lighting_keeper` and `lcd_keeper` design:

| Family | Cmd / operation | Cadence | Rationale |
|---|---|---|---|
| LCDV2 (Galahad II Vision) | cmd 131 lighting (`SetPumpLighting`) | continuous | per `Galahad2VisionController.cs:2443`; workaround for transient buffering |
| LCDV2 | cmd 14 `SendJpeg` | <= 15 s | firmware auto-revert at ~20 s mode-independent hold |
| TLFan | cmd 170 fan PWM | 1 Hz | host-side curve loop |
| TLFan | cmd 161 handshake | 1 Hz | service-side `updateFanInfoTimer` |
| SLV3LCD | sensor frame (rendered host-side, pushed to SDK) | 1 Hz | `SensorTimerInterval = 1000` ms |
| HS II LCD | pump speed (`UpdatePumpFan` -> `SyncPumpFan=0xFB`) | 1 Hz | `UpdatePumpSpeedInterval = 1000` ms |
| HS II LCD | handshake | 0.5 Hz | `UpdateHandshakeInfoInterval = 2000` ms |
| HS II LCD | FW version | 0.5 Hz | `CheckFirmwareVersionInterval = 2000` ms |
| HS II LCD (lcd207 internal) | message-queue worker tick | 330 ms | `WinUsbH2.cs:480-555` in-process FIFO |
| LWireless | named-pipe image preview | 20 Hz | `Task.Delay(50)` cadence (host-side preview only, **not** wire) |

**Daemon implications:** the Linux port must reproduce the LCDV2 lighting re-issue pattern (otherwise rapid back-to-back writes leave the pump LED stuck), and the LCDV2 cmd-14 refresh loop at <=15 s for sustained LCD content. The 1 Hz pump/fan loops apply to TLFan, HS II, SLV3LCD's sensor render, and the LWireless RF poll.

### 4.8 Three-tier opacity model

A three-tier classification emerged from the SDK surveys:

- **Tier 1 (fully visible from C# source, codec-implementable today):** Galahad2 Vision (LCDV2), Galahad2 Trinity, HydroShift LCD (HS I), TLFan, TLLCD, StrimerPlus, Ene6K77Fan.
- **Tier 2 (visible after slv3-sdk + lcd207-sdk surveys):** SLV3LCD, TLV2LCD, HS II LCD (Circle + Square), Lancool 207 Digital, UniversalScreen 8.8" (panel + LED), LWireless wireless variants.
- **Tier 3 (still needs USB capture or further reverse-engineering):** [empty in this snapshot - all surveyed families are Tier 1 or Tier 2 after the SDK passes]. The remaining unknowns are within-family details (e.g., LCDV2 cmd-15 activation path, CH340 UART command set on UniversalScreen DisplayMode endpoint, the 2.4 GHz RF link encryption / pairing flow) rather than whole-family opacity.

### 4.9 Daemon-architecture mapping (a Phase 1 input)

Reference-graph facts surfaced via the SDK surveys:

- **L-Connect.Service is the unique consumer of `lcd207.dll`** -> lcd207's HS II + Lancool 207 + UniversalScreen-panel logic is service-shaped (long-running process, named-pipe IPC, refresh loops).
- **lcd207 -> slv3 + slv3.models** (lcd207 layered on top of slv3, transitively pulls slv3 helpers via the wireless-waterblock priority decision in `SendPumpFan`).
- **L-Connect.Core -> slv3 only** (no lcd207) -> the protocol-logic library uses slv3 for wireless paths; HS II / Lancool 207 LCD-image paths bypass LConnect.Core and go straight Service -> lcd207.
- **L-Connect3 (WPF) -> slv3 only** -> desktop app talks to Service via IPC for HS II / Lancool 207, never directly to lcd207.

**The Linux daemon's architectural role** = `L-Connect.Service + LConnect.Core + slv3 + lcd207` collapsed into one Rust process. The daemon must:

1. Own all four code paths (HID for the Tier-1 families, WinUSB+DES for the lcd207 families, WinUSB+RF chunking for the slv3 wireless families, plus pipe-image preview server).
2. Implement the cross-SDK cross-link (lcd207's `SendPumpFan` skipping when slv3's `RFController.GetAllGroup` reports a wireless waterblock peer bound).
3. Replace the cross-process shared-memory IPC patterns (Windows `WriteProcessMemory` / `ReadProcessMemory`) with Linux equivalents (`process_vm_readv`, `memfd_create` + `SCM_RIGHTS`, or D-Bus byte streams).
4. Handle the named-pipe preview channel as a Unix-domain socket with an explicit access-mode policy.

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

This section is the strategic layer on top of section 4's data. Section 4
catalogues *what* the survey found across families; this section turns those
facts into Phase-1-actionable recommendations. Section 6.5 is a clean recap
intended to be lifted verbatim into the Phase 1 plan's "Phase 0e inputs"
section by the plan author.

### 6.1 Daemon framing-class extension points

The current Rust workspace (`crates/protocol/`) carries codecs for class **A**
(64-byte `ACommandPacket`) and class **B** (1024-out / 512-in
`BCommandPacket`); class **C** (`CCommandPacket`, 512/512) is enum-listed in
section 2 but its first caller is deferred to Phase 1. The cross-device
survey shows that A/B/C alone do **not** cover the rest of the LConnect3
device universe. At minimum **seven distinct framing classes** are needed
for full multi-device support across Phase 3+ (see section 4.1's "Framing"
column for the per-family mapping):

| Framing class | First family that introduces it | Identifying traits |
|---|---|---|
| **A** (already in protocol crate) | Galahad2 Vision (LCDV2) | 64-byte HID, ReportID=1, 6-byte header, BE u16 fields |
| **B** (already in protocol crate) | Galahad2 Vision (LCDV2) | 1024-out / 512-in HID, ReportID=2, 11-byte header |
| **C** (deferred to Phase 1) | Galahad2 Vision (LCDV2 firmware path) | 512-out / 512-in HID, ReportID=3, 11-byte header |
| **T** (TLLCD-class) | TLLCD | 512-out / **64-in** asymmetric HID, ReportID=2, BE u32 `DataSize`, BE u24 `PacketNumber` - looks B-shaped on the write side, A-shaped on the read side |
| **HID-feature `0xE0`** | StrimerPlus, Ene6K77Fan | feature-report (no `Length`/`PacketNumber` framing); composite second byte `(op | port)`; R-B-G colour byte order |
| **WinUSB+DES** (lcd207 / slv3) | HS II Circle/Square, Lancool 207 Digital, UniversalScreen panel, SLV3LCD, TLV2LCD | libusb / WinUSB EP1, 504-byte DES-CBC-encrypted command header at offset 0 (key/IV = `slv3tuzx`), raw body at offset 512; total frame buffer 102,400 bytes |
| **WinUSB-RF** (slv3 wireless) | LWireless V1+V2 hubs, wireless waterblocks | 60-byte WinUSB chunks of `Usb_SendRf=16`, RF payload fragmented by `MasterDevice.SendRfData`; cmd-set distinct from wired (`RF_*` namespace) |
| **bulk-EP1 lighting** (UniversalScreen LED side, separate from lcd207 panel path) | UniversalScreen 8.8" lighting board | bulk-EP1 chunks with `0x11` header byte, 20 LEDs per chunk, RGB byte order (not R-B-G) - OpenRGB-validated (section 5) |

The lcd207 path is **not** functionally identical to slv3 - lcd207 is a
fork that adds fan-control opcodes (`SyncPumpFan=0xFB`, `GetH2Params=0xFA`,
`PushRgbData=0xFC`) and the raw-byte 247/248 MCU-firmware-write path on top
of the slv3 envelope. From a framing perspective they share the same
504+512 DES envelope, but each ships a distinct opcode table and a distinct
set of safety-relevant commands.

**Recommendation for the daemon HID layer (`lconnect-hid`):** the
`Framing` enum must be extensible from day one. Don't constrain the type to
A/B/C and then bolt on variants ad hoc; instead treat the eight classes
above as the design target and build the abstraction such that adding the
ninth (some new SKU's framing) is a contained change. Either:

- model `Framing` as a trait with one impl per class, with the dispatcher
  picking the impl by VID:PID match - this allows Phase 3+ to add a new
  family by writing one impl and registering it, no enum churn; or
- if the project prefers an enum, mark it `#[non_exhaustive]` and require
  every codec to declare its framing class at construction so a missing
  class surfaces at codec-write time rather than at runtime.

Either way, **don't** burn A/B/C into a fixed taxonomy in `lconnect-hid`'s
public API - the survey shows that's the wrong shape.

The Phase-1 deliverable adds **C** alone (per the existing design spec); the
extension surface above is what Phase 3+ slots into.

### 6.2 Cmd-15 hypothesis - resolved at the cross-family level

The cmd-15 (`UploadJpeg`) activation question that motivated the cross-device
survey is now closed at the **strategy** level (section 4.2 closes it at the
data level). Every LConnect3 family with both a transient and a stored image
path uses a clean two-opcode pair:

- **TLLCD** - transient `cmd 65 WriteJPG`; stored `cmd 72 WriteBootJPG`;
  slot-keyed display via `cmd 64 LCDControl` with `Mode=ShowJPG, JPGIndex=N`
- **HydroShift LCD (HS I)** - transient `cmd 14 / 15` (declared but
  unsourced); stored `cmd 22 UploadStartupAviMovie` (explicit `Startup`)
- **slv3-using families** (SLV3LCD, TLV2LCD, wireless wired-mode) -
  transient `CmdType.PushJpg = 0x65`; stored `CmdType.UpdateFireWare = 0x28`
  with filename `/usr/data/boot.jpg` (the FW-update opcode is repurposed for
  generic filesystem-write)
- **lcd207-using families** (HS II Circle/Square, Lancool 207, UniversalScreen
  panel) - identical pattern to slv3 (lcd207 is a fork)

**Likely conclusion for LCDV2 cmd 15:** the device almost certainly has both
a transient path (cmd 14, verified-live) and a stored path (cmd 15, codec
accepts it and the device ACKs but no visible activation on FW 1.6). The two
most plausible activation mechanisms, drawn directly from the cross-family
pattern:

1. **Boot-time display** - cmd 15 writes to flash and is shown only at the
   next power-cycle. This is the dominant pattern: it mirrors TLLCD's
   `WriteBootJPG`, HS I's `UploadStartupAviMovie` (`Startup` qualifier), and
   slv3/lcd207's `/usr/data/boot.jpg` filesystem-write.
2. **Slot-keyed display via LCDControl** - TLLCD selects a stored JPG via
   `LCDControlSetting.JPGIndex` + `LCDControlMode.ShowJPG = 1`. LCDV2's
   current `LCDControlSetting` codec doesn't expose a JPGIndex field, but
   the cross-family pattern says one may exist - the static survey didn't
   exhaust every Galahad2VisionController call site for `LCDControl`
   payload variants, and a re-pass is cheap.

**Phase 0e probe-set candidates** (to be formalised in P0e-19's probe-set
authoring task and exercised live in P0e-20):

- USB-power-cycle the device after a successful cmd 15 upload and observe
  the LCD state on next boot (tests hypothesis 1)
- Re-read `Galahad2VisionController.cs` for any `LCDControlSetting` payload
  shape that includes a slot-index byte; if found, exercise it after a
  cmd-15 upload (tests hypothesis 2)
- Try `LCDControl(mode = LocalAVI = 3)` and `LCDControl(mode = LocalH264 = 2)`
  after a cmd-15 upload to see whether non-default modes activate it
- Compare the cmd-14 vs cmd-15 ACK byte stream byte-for-byte: any divergence
  in `Length` / `Data[0]` after the upload may hint at an internal-state
  flag the daemon can read

**Phase 1 caveat** (independent of probe outcome): even if Phase 0e probing
doesn't activate cmd 15, the codec stays implemented and in-tree. Per the
Phase 1 design spec, the daemon's IPC surface will not expose cmd 15 until
its activation path is found - it stays codec-implemented but not
ipc-exposed, so the day cmd-15 activation is discovered, the only remaining
work is unsealing it on the IPC side.

### 6.3 Reusable codec opportunities

Commands that appear byte-identical (or near-identical with a single
parametrisable difference) across three or more families - these are
candidates for refactoring into a shared `crates/protocol/src/common/`
module **before** Phase 3's second device family lands. Drawn from sections
4.1, 4.4, and 4.5.

| Command | Families that share it | Divergence | Refactor recommendation |
|---|---|---|---|
| Lighting `Set*Lighting` (cmd 131 / 133) | LCDV2 (verified-live), Galahad2 Trinity (inferred-static), HS I (cmd 133 only - no pump cmd 131) | mode-byte set differs (Trinity does `% 1000` mode folding, LCDV2 does not); field offsets identical | LCDV2's `lighting.rs` codec is the reference; Trinity slots in as a parametrised variant |
| `SetLEDColors` (cmd 20) | LCDV2 (verified-live), HS I (inferred-static) | LCDV2 uses `isFanLed` flag-byte branch; HS I always writes flag = `0x01` and omits the zero-pad | one shared body-builder + a per-family flag-byte parameter |
| `GetHandshakeInfo` (cmd 0x0A / cmd 129) | LCDV2 (7-byte response), Trinity (4-byte response, no temp), HS I (7-byte response identical to LCDV2), TLFan (cmd 161, Nx3-byte response) | response shapes differ enough that "share the request, parse per-family" is the right move | shared `build_request`, per-family `from_response` |
| `GetFirmwareVersion` (cmd 0x86 / cmd 134) | LCDV2, Trinity, HS I, TLFan (cmd 166), TLLCD (cmd 61) | response demuxing differs (PacketNumber vs `Data[2]` vs sequential reads); ASCII payload format differs | document, **don't** refactor yet - too divergent for a clean shared API |
| `ResetDevice` (cmd 0x0E / cmd 142) | LCDV2 (codec-only, gated), HS I (inferred-static, destructive) | identical wire shape | small enough that sharing isn't worth the abstraction yet |

**Recommendation:** **defer the actual extraction to the Phase 3 first-task
window** (when the second device family lands and the second consumer
appears). Phase 1 ships the LCDV2-specific codec as-is in
`crates/protocol/src/galahad2_lcdv2/`. To keep the candidates surfaced for
the Phase 3 implementer:

- add a `// TODO(phase-3-refactor): candidate for crates/protocol/src/common/ -
  shared with <family list>` comment on each codec that's a refactor
  candidate, citing the families that would consume it
- list the candidates in this section (above) so the Phase 3 plan author
  can lift them without re-doing the cross-family analysis

The general principle: don't pre-extract for a single consumer. The
abstractions are easiest to design correctly when there are at least two
real consumers, which won't happen until Phase 3 begins.

### 6.4 Risky-area flagging for `hid-safety-reviewer`

Commands and patterns that future `hid-safety-reviewer` dispatches must
treat with extra scrutiny. These are areas where a coding mistake risks
device bricking, persistent-state corruption, or silently-wrong colour /
data on the wire. Drawn from sections 4.5, 4.6, and 4.7 plus the OpenRGB
cross-validation in section 5.

**Firmware-update commands (Phase 1: deferred for all families)**

Every family ships a firmware-update opcode pair: LCDV2's cmds 10/11
(`ActiveFirmware`/`UploadFirmware`), HS I cmds 10/11, TLLCD cmds 58/59,
TLFan cmds 11/12 + 171/172, slv3's `Usb_FwErase=32` / `Usb_FwUpload=33` /
`Usb_FwReboot=35`, lcd207's `UpdateFireWare=0x28` (which doubles as a
generic filesystem-write - see below). **Phase 1 explicitly defers all
firmware paths.** The reviewer should reject any HID write to these cmd
bytes unless a firmware flow is the explicit current task.

**Reset-device commands**

LCDV2 cmd 0x0E `ResetDevice`, lcd207's `Reboot=11` opcode, SLV3LCD dispatch
cmd 16 `RestartLCD` (destructive across the whole LCD group). Reset drops
the device off USB; an in-flight write at the moment of reset risks brick if
the firmware doesn't checkpoint state. The Phase 0d
`--yes-i-really-want-to-reset` CLI gate is the right pattern; future codecs
that wrap reset-equivalent commands must require an analogous explicit
opt-in at the IPC boundary.

**DES-CBC-encrypted command headers (slv3 + lcd207)**

The 504-byte DES-CBC envelope at offset 0 of the WinUSB EP1 frame uses a
hardcoded `slv3tuzx` key+IV. A wrong key, wrong IV, or wrong padding
produces malformed-but-decryptable commands - the firmware sees a parsable
envelope with garbage opcodes and may go into undefined state. If the
daemon ever reimplements this path (Phase 3+), the reviewer should require:
exact-bytes test vectors covering at least one each of `PushJpg`, `Reboot`,
and a `UpdateFireWare` filesystem-write before any live probe.

**lcd207's `UpdateFireWare = 0x28` filesystem-write opcode**

This opcode is repurposed at the lcd207 layer to mean "write a file at the
filename embedded in the header at offset 16". The intended filename for
the boot-image flow is `/usr/data/boot.jpg`. A wrong filename byte sequence
in the header could overwrite firmware-binary slots
(`/usr/data/update.app` is a known target for the actual FW path) instead.
**The Phase 1+ daemon must hardcode the filename string on the trust
boundary and never accept it from IPC** - this is a category-A
brick-risk path.

**lcd207 raw-byte opcodes 247/248**

Undeclared MCU-firmware-write opcodes that ship with CRC-16-checksummed
4096-byte chunks. Higher bricking risk than the application-firmware path
because it touches the MCU rather than the user-space partition. Phase 1
must not expose these. Phase 3+ must not expose these either, until/unless
a separate "firmware-update mode" is built with its own gate.

**StrimerPlus and Ene6K77Fan R-B-G byte order**

OpenRGB cross-validation (section 5.3) confirms the LConnect3 source: per-LED
colour bytes are written in **R-B-G** order on these families, not RGB. The
Linux daemon must preserve R-B-G; getting the byte order wrong produces
wrong colours but no brick risk - reversible by re-issuing the command.
Document, don't gate.

**LCDV2 cmd 131 buffering quirk**

Phase 0c finding: rapid back-to-back writes can leave the LED stuck in an
intermediate state. Section 4.7 documents the mitigation (`lighting_keeper`
re-issues at 1.5 s). The reviewer should reject any LCDV2 lighting write
that is not bracketed by the keeper loop, except for explicit one-shot
debug paths (`verify-galahad`).

**LCDV2 cmd 14 (`SendJpeg`) ~20 s auto-revert**

Section 4.7: firmware auto-reverts the LCD content after ~20 s mode-
independent. The `lcd_keeper` refresh cadence must be <= 15 s (5 s margin)
for sustained content. The reviewer should flag any cmd-14 caller that
isn't in the keeper loop.

### 6.5 Phase 1 plan inputs

A clean recap intended to be lifted into the Phase 1 plan author's
"Phase 0e inputs" section. Each bullet is concrete enough to drive a
specific Phase 1 decision; cross-references point back to the matrix that
authorises it.

- **Framing-enum extensibility (section 6.1):** `lconnect-hid::Framing` must
  be designed extensible from day one - at least seven framing classes
  exist across the LConnect3 device universe, and Phase 1's CCommandPacket
  addition is just the first beyond A/B. Either model `Framing` as a trait
  with per-class impls, or as a `#[non_exhaustive]` enum that requires
  per-codec class declaration. Don't burn A/B/C in as a closed taxonomy.
- **Cmd 15 stays codec-implemented but IPC-unexposed (section 6.2):** Phase
  0e probe set (P0e-19, P0e-20) may close cmd-15 activation; if it doesn't,
  the codec remains in-tree but the daemon's IPC surface does not expose
  it. The day activation is discovered, the codec is ready and only the
  IPC seal needs to be lifted.
- **`lighting_keeper` cadence = 1.5 s (sections 4.7, 6.4):** mirrors the
  C# service's continuous re-issue on cmd 131; consistent with sibling
  families per the cadence matrix.
- **`lcd_keeper` cadence <= 15 s for cmd 14 (sections 4.7, 6.4):** 5 s
  margin under the firmware's ~20 s auto-revert. Sustained LCD content
  requires the keeper loop.
- **No firmware paths in Phase 1 (section 6.4):** `UploadFirmware`,
  `ActiveFirmware`, lcd207's `UpdateFireWare` (both as FW-update and as
  filesystem-write), slv3's `Fw*` opcodes, TLFan's cmds 171/172, TLLCD's
  cmds 58/59 - all deferred. Codec-only is acceptable for cmds whose
  activation has been verified (LCDV2 only), gated by IPC-side seal.
- **No DES-CBC encryption needed in Phase 1 (sections 4.1, 6.4):** LCDV2
  doesn't use it. The `slv3tuzx`-keyed envelope is only relevant to slv3
  / lcd207 families, which are Phase 3+ work.
- **Code organisation (section 6.3):** keep
  `crates/protocol/src/galahad2_lcdv2/` as-is in Phase 1. The shared
  extraction to `crates/protocol/src/common/` is deferred to Phase 3's
  first task (when the second family lands and a second consumer exists);
  add `// TODO(phase-3-refactor): ...` comments on the candidates listed
  in 6.3.
- **Daemon-architecture role (section 4.9):** the Linux daemon's role =
  `L-Connect.Service + LConnect.Core + slv3 + lcd207` collapsed into one
  Rust process. **For Phase 1, only the LCDV2-relevant subset (the
  `LConnect.Core/Products/Galahad2Vision/` slice) ships;** slv3 and
  lcd207 equivalents are Phase 3+ work.
- **Phase 1 device scope:** LCDV2 only - no StrimerPlus, no Trinity, no
  fan-only families, no HS I, no HS II, no TLFan / TLLCD, no SLV3LCD /
  TLV2LCD, no Lancool 207, no UniversalScreen, no LWireless. Per the
  Phase 1 design spec.
- **udev VID coverage (section 4.6):** the Phase 1 udev rule only needs
  VID `0x0416` for the LCDV2 PID `0x7395`. The five-VID multi-device rule
  (`0x0416`, `0x0CF2`, `0x04FC`, `0x1CBE`, `0x1A86`) is Phase 3+ scope; the
  cross-VID family clusters from section 4.6 drive Phase 3+
  code-organisation boundaries (`winbond_*`, `ene_*`, `sunplus_tllcd`,
  `lianli_lcd_*`, `wch_*`).
- **Risky-area gating (section 6.4):** `hid-safety-reviewer` should treat
  any HID write to the firmware-update opcodes, the reset opcodes, the
  lcd207 `UpdateFireWare` filesystem-write, the lcd207 raw-byte 247/248
  paths, or the slv3/lcd207 DES envelope as a category-A review even if
  the diff looks innocuous. Phase 1 only touches LCDV2's firmware /
  reset / serial-number opcodes (codec-only, IPC-sealed), so the live
  surface is small - but the reviewer expectations carry forward to
  every subsequent phase.
