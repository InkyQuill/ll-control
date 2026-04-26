## Family: TLFan

**Source folder:** `LConnectCore/Products/TLFan/`

**Marketed name(s):** Lian Li TL series fans — addressable RGB case fans with daisy-chain controller. Two SKU classes are bucketed under the TL umbrella in this codebase: the `TLFan` controller (LED-only TL fans, no on-fan LCD) and `TLLCD` (TL fans with on-fan LCD screens — different USB ID, separate `Products/TLLCD/` folder, out of scope here). Constants name 120 mm and 140 mm chassis variants for both `LED` and `LCD` port types (`LConnectCore/Products/TLFan/Constants.cs:21-35`), so the TLFan controller actually drives both flavours of fan over its 4 ports — the `PortType` (LED vs LCD) is configured per-port and changes the maximum-RPM table only.

**ControllerTypeID enum value(s):** `TLFan = 16908288` (`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:23`). The sibling `TLLCD = 16912384` (`ControllerTypeID.cs:24`) is a *separate device*; `TLFan` is the family covered here.

**USB VID:PID:** `0x0416:0x7372` (decimal `1046:29554`) — one row, single SKU at the controller level. From `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs:49`. The TLFan registration also carries a `USBDeviceExtendedID` with `Usage = 0xFF1B` (decimal `65307`) and a null `UsagePage`, suggesting HID Usage-Page-based disambiguation against another interface that shares the same VID:PID (the same VID `0x0416` is used by the Galahad/HydroShift AIO families). VID `0x0416` is Winbond / shared with the AIO families; PID `0x7372` is unique to the TL fan controller.

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | UsageID | Source |
|---|---|---|---|---|
| `TLFan` (16908288) | `0x0416:0x7372` | `1046:29554` | `0xFF1B` | `KnownDevices.cs:49` |

**Framing classes used:** **family-specific** — TLFan uses two parallel 64-byte ReportID=1 packet types, structurally identical but semantically partitioned:
- `ControllerPacket` (`ControllerPacket.cs:6-97`) — for the **bootloader / firmware-flashing controller MCU**, command byte at offset 1, no observed callers in the live device-driver path (`ControllerCommands` enum is referenced only by its own `.cs` file in this codebase, see Commands table below).
- `LEDPacket` (`LEDPacket.cs:10-131`) — for the **LED/runtime MCU**, command byte at offset 1, all runtime traffic in `TLFanDevice.cs` flows through this class.

Both packet types share the LCDV2 A-class layout exactly:

```
offset  size  field          description
------  ----  -----          -------------------------------------------
  0      1   ReportID       always 0x01
  1      1   Command        opcode (see Commands table)
  2      1   reserved       always 0x00
  3      2   PacketNumber   big-endian u16; 0 unless multi-packet upload
  5      1   Length         valid data bytes (0..=58)
  6     58   Data           command-specific payload, zero-padded
```

Constants (`ControllerPacket.cs:8-14`, `LEDPacket.cs:12-18`): `PacketLength = 64`, `HeaderLength = 6`, `PayloadLength = 58`, `ReportID = 0x01`. This matches the LCDV2 A-class framing exactly.

`LEDPacket.BuildPacketsForWrite` (`LEDPacket.cs:89-117`) defines a multi-packet chunker, but **no caller in `TLFanDevice.cs` or `TLFanController.cs` invokes it** — every command's payload fits in the single-packet 58-byte limit. The chunker is dead code at the callsite level; the framing class is effectively single-packet only at runtime.

**Phase 0d status:** not in scope; static-analysis only.

**Subsystem fingerprint:**
- Sensors / handshake: **yes** — `GetHandshakeInfo` (cmd 161) returns a flat array of 3-byte `FanInfo` records, one per detected fan, encoding `IsDetected/IsUpgrading` flags + 4-bit port + 4-bit fan-index in byte 0 and BE-u16 fan RPM in bytes 1–2 (`TLFanHandshakeInfo.cs:64-84`, `129-136`). Up to 64 fans (4 ports × 16 fans) can be reported; payload is up to ~58 bytes ÷ 3 bytes/fan ≈ 19 fans per packet.
- Fan PWM: **yes** — `SetFanSpeed` (cmd 170) writes a single `(port, fanIndex, pwm)` tuple per call (`TLFanDevice.cs:274-283`). PWM is u8 0..=100 percent, applied per individual fan rather than per group at the wire layer; the *service controller* fans out a curve-derived PWM to each fan via this command in a 1 Hz loop (`L-Connect-Service/LConnectService/Products/TLFan/TLFanController.cs:1392-1485`).
- Pump PWM: **N/A** — no pump on TL fans.
- Fan curve upload: **no on-device curve.** All curve evaluation runs on the host: the service computes a target PWM per fan from `RPMConfig.Phases[]` against CPU/GPU/water temperature and pushes it via cmd 170 each tick (`TLFanController.cs:1396-1485`). The device exposes only the per-fan PWM setpoint, not a curve.
- Lighting (preset modes): **yes** — `SetFanLight` (cmd 163, 20-byte payload, `TLFanDevice.cs:244-272`) and `SetGroupLight` (cmd 176, 20-byte payload, `TLFanDevice.cs:285-312`). Both encode `(mode, brightness, speed, 4×RGB color, direction, disabled, color-count)`. Modes are `LightingMode` (29 base modes 1..28 plus `_Merge`, `_Side`, `_Side_Merge` variants — `LightingMode.cs:7-83`); the high-byte category is stripped by `getLightingModeValue` (mode % 1000) before being sent on the wire (`TLFanDevice.cs:190-193`), so only the base mode byte 0..28 reaches the firmware.
- Lighting (per-pixel LED): **no** — the protocol is preset-mode + 4-color seed only. There is no per-LED RGB stream command analogous to LCDV2's `SetLEDColors` (cmd 20). The LED count is implicit (4-color slots, with the 20th byte being `colors.Length` as a count).
- LCD on/off / brightness / rotation: **N/A** in the live device path. The `LEDCommands` enum declares `LCDScreenSwitch = 178` (`LEDCommands.cs:22`) but **no call site in `TLFanDevice.cs` or `TLFanController.cs` issues byte 178**. The TLFan family controller does not drive the on-fan LCD here — the separate `TLLCD` device class is the LCD path.
- LCD frame upload: **N/A** — same reason. No JPEG/H.264/AVI frame upload commands in this family's enums.
- Continuous re-issue pattern: **yes** — `TLFanController.writeFanSpeedTimer` is a `NonOverlappingTimer` that re-evaluates every per-fan curve and writes cmd 170 once per detected fan every tick (`TLFanController.cs:1392-1485`); `updateFanInfoTimer` polls cmd 161 once per second to refresh `FanGroupStatus` (`TLFanController.cs:1345-1385`). A fan-disconnect / fan-detach is detected by diffing `FanQuantity` arrays across two ticks.
- Reset / soft-reset: **no.** No `ResetDevice` analogue. Nothing in `LEDCommands` or `ControllerCommands` corresponds to LCDV2's cmd 142.
- Firmware upload / activation: **declared but unused at runtime.** `LEDCommands.SendFirmware = 172` and `LEDCommands.ActiveFirmware = 171` exist (`LEDCommands.cs:15-16`), and the `ControllerCommands` enum carries `EnterUpgradeMode = 11`, `WriteFirmware = 12` for the controller MCU's bootloader path (`ControllerCommands.cs:7-12`). **Neither is called from `TLFanDevice.cs` or the service controller** in the decompiled tree — the firmware-flashing path lives in a tool we do not have visibility into here. Treat as inferred-static and out of scope for the daemon port.
- Crypto / obfuscation at HID transport: **none observed.** Plain bytes throughout. The `CryptoHelper` reference at `TLFanController.cs:571` wraps buffered handshake records for outbound diagnostic telemetry, not on the wire.

### Commands

`LEDPacket` traffic (the runtime/LED MCU; `LEDCommands` enum at `LEDCommands.cs:5-25`):

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| 161 (0xA1) | host<->dev | `Handshake` (`GetHandshakeInfo`) | A-class (LEDPacket, 64B) | `TLFanDevice.cs:155-188` | inferred-static | Tx: `Command=161`, no data. Rx: same cmd echo, `Data` = N×3 bytes where each triple = `(byte0=detected[7] \| upgrading[6] \| port[5:4] \| fanIndex[3:0], byte1=fanSpeed_hi, byte2=fanSpeed_lo)` (`TLFanHandshakeInfo.cs:129-136`). `FanSpeed` is BE u16. Up to ~19 fans per single packet. |
| 162 (0xA2) | — | `GetLightModeList` | A-class | `LEDCommands.cs:8` | declared-only | **Enum value declared but no call site** in `TLFanDevice.cs` or `TLFanController.cs`. Presumed mode-list query analogous to LCDV2 cmd 132/130, but unused in the decompiled runtime — flagged 🚧. |
| 163 (0xA3) | host->dev | `SetFanLight` | A-class | `TLFanDevice.cs:244-272` | inferred-static | 20-byte payload. See "SetFanLight payload" below. Sets one fan's lighting (port + fanIndex select target) or a port-wide sync write. |
| 166 (0xA6) | host<->dev | `GetProductInfo` (firmware version) | A-class | `TLFanDevice.cs:55-153` | inferred-static | Tx: `Command=166`, `Data=[isSlave, target]` where `target = 0` for the controller, `target = (port<<4)\|fanIndex` for a per-fan firmware query. **Two response packets** are read (`TLFanDevice.cs:84-85`); both must echo cmd 166 and carry the same `(isSlave, target)` in `Data[0..1]` plus a 1-based packet index in `Data[2]` (1 = first half of the version string, 2 = second half — `TLFanDevice.cs:130-139`). The full firmware-version string is the concatenation of `Data[3..]` from the two response packets, parsed by `FirmwareVersion.TryParse`. |
| 167 (0xA7) | — | `LEDTestMode` | A-class | `LEDCommands.cs:11` | declared-only | **Enum value declared, no call site.** Probably a manufacturing-mode toggle. Distinct from `TestLight` (179). 🚧 |
| 168 (0xA8) | — | `GetSerialNumber` | A-class | `LEDCommands.cs:12` | declared-only | Enum-only. No caller. 🚧 |
| 169 (0xA9) | — | `SetSerialNumber` | A-class | `LEDCommands.cs:13` | declared-only | Enum-only. No caller. **Destructive — likely factory-only.** 🚧 |
| 170 (0xAA) | host->dev | `SetFanSpeed` | A-class | `TLFanDevice.cs:274-283` | inferred-static | Tx: `Command=170`, `Data=[(port<<4)\|fanIndex, pwm]`. PWM is u8 0..=100 percent. Per-fan PWM setpoint; the service re-issues this in a curve-driven 1 Hz loop. No response read. |
| 171 (0xAB) | — | `ActiveFirmware` | A-class | `LEDCommands.cs:15` | declared-only | Enum-only. No caller. **Destructive — firmware flash activation.** 🚧 |
| 172 (0xAC) | — | `SendFirmware` | A-class | `LEDCommands.cs:16` | declared-only | Enum-only. No caller. **Destructive — firmware payload write.** 🚧 |
| 173 (0xAD) | host->dev | `SetFanGrouping` | A-class | `TLFanDevice.cs:222-242` | inferred-static | Tx: `Command=173`, `Data=[groupNumber, count, member0, member1, …]` where each `memberN` byte packs `(top<<7) \| (bottom<<6) \| (port<<4) \| fanIndex`. Defines a logical lighting / RPM group spanning fans across ports. `groupNumber` 0..=30 (`Constants.MaxGroupNumber = 31`). Read-after-write reads (and discards) the response. |
| 174 (0xAE) | host->dev | `SetFanDirection` | A-class | `TLFanDevice.cs:211-220` | inferred-static | Tx: `Command=174`, `Data=[(port<<4)\|fanIndex, (swapTopBottom<<1) \| swapLeftRight]`. Per-fan light-direction swap (used to compensate for fans installed upside-down). |
| 175 (0xAF) | host->dev | `SetPortDirection` | A-class | `TLFanDevice.cs:325-334` | inferred-static | Tx: `Command=175`, `Data=[port<<4, swap]`. Per-port direction swap. The fan-index nibble is forced to 0; only the high nibble of byte 0 selects the port. |
| 176 (0xB0) | host->dev | `SetFanGroupLight` | A-class | `TLFanDevice.cs:285-312` | inferred-static | 20-byte payload, mirrors `SetFanLight` shape but `byte 0 = 0` (reserved/unused), `byte 1 = groupNumber`, no isSync/disable in byte 0. See "SetGroupLight payload" below. |
| 177 (0xB1) | host->dev | `SetMBFanSync` (`SetMBRPMSync`) | A-class | `TLFanDevice.cs:314-323` | inferred-static | Tx: `Command=177`, `Data=[(isSync<<7) \| (port<<4) \| fanIndex]`. 1-byte payload. Switches a single fan between motherboard-PWM-following and host-driven-PWM modes. The service controller's `IsMotherboardRPMSync` AppSetting (`TLFanController.cs:1399`) gates the curve-loop write of cmd 170 when sync is enabled — i.e., when MB sync is active, the host stops fighting the motherboard for PWM control. |
| 178 (0xB2) | — | `LCDScreenSwitch` | A-class | `LEDCommands.cs:22` | declared-only | **Enum value declared, no call site** in `TLFanDevice.cs` or `TLFanController.cs`. Suggests a dormant LCD-power control path — but the TLFan controller does not drive on-fan LCDs in this code; the separate `TLLCD` family does. May be vestigial or used by an off-tree tool. 🚧 |
| 179 (0xB3) | host->dev | `TestLight` | A-class | `TLFanDevice.cs:336-360` | inferred-static | Tx: `Command=179`, `Data=[type, number, isBottom]`. `type=1` → fan (`number = (port<<4)\|fanIndex`), `type=2` → group (`number = groupNumber & 0x1F`), `type=3` → port (`number = port & 0x03`). Used to flash a target's LEDs for visual identification during configuration. |
| 180 (0xB4) | host->dev | `BlinkPort` | A-class | `TLFanDevice.cs:44-53` | inferred-static | Tx: `Command=180`, `Data=[port]`. Briefly blinks all LEDs on a port. Read-after-write discards the response. |

`ControllerPacket` traffic (the bootloader / controller MCU; `ControllerCommands` enum at `ControllerCommands.cs:5-12`):

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| 1 (0x01) | — | `Handshake` (controller) | A-class (ControllerPacket, 64B) | `ControllerCommands.cs:7` | declared-only | No call site found in this tree. Likely used by the firmware-flashing tool to identify the bootloader MCU before issuing `EnterUpgradeMode`. 🚧 |
| 11 (0x0B) | — | `EnterUpgradeMode` | A-class | `ControllerCommands.cs:8` | declared-only | **Destructive.** No call site. Puts the controller MCU into bootloader/firmware-flash state. 🚧 |
| 12 (0x0C) | — | `WriteFirmware` | A-class | `ControllerCommands.cs:9` | declared-only | **Destructive.** No call site. Writes a firmware blob to the controller MCU. 🚧 |
| 13 (0x0D) | — | `GetProductInfo` (controller) | A-class | `ControllerCommands.cs:10` | declared-only | No call site. Distinct from cmd 166 on the LED MCU — this is the controller MCU's product-info query. 🚧 |
| 21 (0x15) | — | `GetSerialNumber` (controller) | A-class | `ControllerCommands.cs:11` | declared-only | No call site. 🚧 |

**SetFanLight payload (cmd 163, 20 bytes — `TLFanDevice.cs:244-272`):**

| offset | size | field | encoding |
|---|---|---|---|
| 0 | 1 | `port_sync` | `(port<<4) \| isSync` (low bit = sync flag, high nibble = source port to copy from when sync) |
| 1 | 1 | `port_fan` | `(port<<4) \| fanIndex` (target fan to write) |
| 2 | 1 | `mode` | base `LightingMode` byte (`mode % 1000`) |
| 3 | 1 | `brightness` | u8 (0..=4 in the C# UI's `LightingConfig.Brightness` default 2) |
| 4 | 1 | `speed` | u8 (0..=4 in the C# UI's default 2) |
| 5..7 | 3 | `color1` | RGB |
| 8..10 | 3 | `color2` | RGB |
| 11..13 | 3 | `color3` | RGB |
| 14..16 | 3 | `color4` | RGB |
| 17 | 1 | `direction` | `LightingDirection` (0..=5: Right/CW, Left/CCW, Up, Down, Spread, Gather — `LightingDirection.cs:5-13`) |
| 18 | 1 | `disabled` | bool 0/1 |
| 19 | 1 | `colorCount` | u8 — number of color slots actually used (0..=4) |

When `isSync=true` or `disabled=true`, the mode/brightness/speed/colors/direction/colorCount fields stay zeroed — only the `port_sync`/`port_fan`/`disabled` fields are meaningful (`TLFanDevice.cs:251-265`).

**SetGroupLight payload (cmd 176, 20 bytes — `TLFanDevice.cs:285-312`):**

Identical layout to SetFanLight except byte 0 is unused (always 0) and byte 1 carries `groupNumber` (0..=30) instead of port/fan addressing. There is no `isSync` flag for groups. `disabled=true` zeros only the color slots — mode/brightness/speed/direction/count are still written (`TLFanDevice.cs:295-305`), which is a subtle asymmetry vs `SetFanLight`.

### Subsystem patterns

- **Two-MCU device.** TLFan exposes two distinct command spaces — `ControllerCommands` for the bootloader/controller MCU and `LEDCommands` for the runtime/LED MCU — over the same HID pipe and the same 64-byte ReportID=1 framing. The two are disambiguated purely by the command byte at offset 1. All runtime traffic in the decompiled C# stays on the LED MCU side; the bootloader side is wired up to the enum but never called from the tree we have, implying a separate flashing tool. (`ControllerCommands.cs`, `LEDCommands.cs`, `ControllerPacket.cs`, `LEDPacket.cs`).
- **Per-fan addressing scheme.** Almost every write encodes its target fan as a single `(port<<4) | fanIndex` byte. `port` is 2 bits (0..=3, `Constants.PortCount = 4`), `fanIndex` is 4 bits (0..=15, `Constants.MaxFanCountPerPort = 16`). Some commands (cmd 173 group members) additionally pack `topSide` and `bottomSide` into the high two bits of the same byte. This is the dominant addressing primitive on the wire.
- **Fan curves run on the host.** No on-device curve upload command exists. The service controller (`TLFanController.cs:1392-1485`) maintains an `RPMConfig` per `(port, fanIndex, FanSize)` and re-evaluates a target PWM each tick from CPU / GPU / water temperature, then writes one cmd 170 per fan. Curves come pre-baked per fan size: `(LED, 120mm)` and `(LED, 140mm)` and `(LCD, 120mm)` and `(LCD, 140mm)` get distinct RPM-to-PWM tables (`TLFanController.cs:122-149`). A Linux daemon must own this loop end-to-end — the device offers no hardware-side fallback curve.
- **Two lighting addressing modes.** `SetFanLight` (cmd 163) targets a single `(port, fanIndex)` and supports a "sync from source port" shortcut where `isSync=1` causes the receiver to copy another port's settings without re-encoding the whole 20-byte payload. `SetGroupLight` (cmd 176) targets a logical group defined by a prior `SetFanGrouping` (cmd 173). The C# UI typically does: first call cmd 173 to define groups, then cmd 176 to drive them. Every write is read-after-write (the response is read and discarded; failure is silent — `TLFanDevice.cs:241, 271, 311`).
- **Lighting modes are u32 categories collapsed to u8 on wire.** `LightingMode` enum values use `1000 / 2000 / 3000` numeric prefixes for `_Merge`, `_Side`, `_Side_Merge` UI scopes (`LightingMode.cs`). The wire byte is `mode % 1000` (`TLFanDevice.cs:190-193`), so on the wire only 28 base modes are addressable — the scope distinctions are host-side metadata for the UI, not the firmware.
- **Read-after-write everywhere.** Almost every command in `TLFanDevice.cs` calls `this.readPacket()` after the write (cmd 170, 174, 175, 173, 163, 176, 177, 179, 180). The result is discarded for every command except 161 (handshake) and 166 (firmware version). This is closer to a request/response RPC pattern than LCDV2's mostly fire-and-forget command set.
- **Continuous re-issue pattern.** Two `NonOverlappingTimer`s on the service side: `updateFanInfoTimer` polls cmd 161 at 1 Hz; `writeFanSpeedTimer` writes one cmd 170 per detected fan per tick (also ~1 Hz given the timer construction). Compared to LCDV2's per-LED frame loop, this is a much lower-bandwidth, much steadier cadence — and it gates fan-curve enforcement, so a Linux daemon must keep this running for thermal safety once the host takes over from MB-sync.

### Notable divergences from LCDV2

- **Same A-class framing, different command space.** Both LCDV2 and TLFan use ReportID=1, 64-byte packets, 6-byte header, 58-byte payload. TLFan adds a *parallel* `ControllerPacket` class for bootloader traffic (semantically distinct, structurally identical). LCDV2 has no equivalent split.
- **No B-class (1024/512) or C-class (512/512) framing.** TLFan defines a multi-packet chunker on `LEDPacket` but never invokes it; LCDV2 relies heavily on B-class for `SendJpeg` (cmd 14), `SetLEDColors` (cmd 20), and similar. TLFan needs no high-bandwidth path because it has no LCD frame upload and no per-pixel LED stream.
- **Per-fan addressing is the wire primitive.** LCDV2 sends a single `group=0` byte to address its fan ring as a unit; TLFan addresses individual fans by `(port<<4)|fanIndex`. A Linux daemon for TLFan needs a topology model (4 ports × up to 16 fans) discovered at handshake time.
- **Lighting payload is 20 bytes instead of 19/20.** TLFan's `SetFanLight` (20 bytes: adds `colorCount` at byte 19) closely mirrors LCDV2's `SetFanLighting` (cmd 133, 20 bytes), but the field layout is different — TLFan has `port_sync`/`port_fan` at bytes 0–1, no `ArgbSignalSource` byte, no `sync_to_pump` (TL fans have no pump). LCDV2 has `ArgbSignalSource`, `sync_to_pump`, and `number_of_led` at bytes 17–19.
- **Lighting modes are richer.** TLFan exposes 28 base modes plus three categorical variants (`_Merge`, `_Side`, `_Side_Merge`) for 80+ enum values total. LCDV2's pump (16 modes) and fan (6 modes) lighting are far smaller mode spaces.
- **No reset and no LCD subsystem.** LCDV2's `ResetDevice` (cmd 142), `LCDControl` (12), `IsLCDAvailable` (23), `SendJpeg` (14), `SendH264Frame` (13), `SetLEDColors` (20) — all absent in TLFan.
- **No pump.** LCDV2's `GetPumpLightingInfo` (130), `SetPumpLighting` (131), `SetPumpPWM` (138) all absent. The handshake response also lacks a `PumpRpm` field — only per-fan RPM via the `FanInfo` triples.
- **Handshake response is a fan list, not a fixed struct.** LCDV2 returns 7 bytes: `(fan_rpm, pump_rpm, temp_valid, temp_int, temp_frac)`. TLFan returns N×3 bytes, one triple per detected fan, with no temperature, no pump, no fixed slot count. Up to ~19 fans fit in a single A-class packet's 58-byte payload — sufficient given the 4×16=64 theoretical max only if the firmware truncates or paginates; static analysis suggests one packet only and physical layouts rarely exceed 12 fans (4 ports × 3 typical TL daisy-chain length), but the host-side parser uses `data.Length / 3` as the count (`TLFanHandshakeInfo.cs:71-81`) and would silently truncate a longer response.
- **Continuous re-issue is for fan PWM, not lighting.** LCDV2's continuous re-issue pattern is on cmd 131 (lighting) at the service layer, hypothesized as a workaround for transient LED-buffering. TLFan's continuous re-issue is on cmd 170 (fan PWM), driven by the host-side curve loop — a different motivation for the same pattern.
- **VID/PID:** `0x0416:0x7372` shares the Winbond/AIO VID `0x0416` with Galahad and HydroShift, but uses a distinct PID (`0x7372` vs `0x7395` for LCDV2). Disambiguation against other interfaces sharing the same VID:PID uses the HID Usage `0xFF1B`.

### Open questions / unclear from static analysis

- 🚧 **Cmd 162 (`GetLightModeList`) is enum-only.** No caller in the decompiled tree, but the byte is reserved between cmd 161 (handshake) and cmd 163 (set fan light), structurally where LCDV2 puts its `Get{Pump,Fan}LightingInfo` (cmd 130/132). Plausibly the same escape-byte mode-list response shape. Live capture needed to confirm format and whether the firmware accepts it.
- 🚧 **Cmd 167 (`LEDTestMode`), cmd 168 (`GetSerialNumber`), cmd 169 (`SetSerialNumber`), cmd 171 (`ActiveFirmware`), cmd 172 (`SendFirmware`), cmd 178 (`LCDScreenSwitch`)** are enum-declared but unused at runtime. They are presumably reachable via off-tree tooling (factory test rig, firmware flasher). Cmds 169, 171, 172 are destructive — flag for hid-safety-reviewer if implemented in the Linux daemon.
- 🚧 **`ControllerPacket` traffic (cmds 1, 11, 12, 13, 21) has no caller** in the decompiled C#. The bootloader MCU is reachable in principle on the same HID pipe by sending these command bytes, but the calling tool is absent from this tree. **Do not implement bootloader commands without a separate firmware-flashing harness.**
- 🚧 **Handshake response truncation behaviour.** With 4 ports × 16 fan slots = 64 theoretical fans × 3 bytes = 192 bytes, a fully populated handshake cannot fit in a single 58-byte A-class payload. Whether the firmware paginates (and via what mechanism — `PacketNumber`?), truncates, or simply never reports more than ~19 fans is undetermined from static analysis. Realistic deployments are far below the limit, but a Linux daemon must handle the boundary explicitly.
- 🚧 **Cmd 166 firmware-version response packet ordering.** The C# reads two packets sequentially (`TLFanDevice.cs:84-85`) and then validates each one's `Data[2]` against literal values 1 and 2 (`TLFanDevice.cs:132`). If the device emits the packets in a different order, or interleaves an unsolicited handshake-style packet between them, the C# silently returns an empty `FirmwareVersion`. The Linux port should de-multiplex on `Data[2]` rather than on read order — same lesson as LCDV2's firmware command.
- 🚧 **`getLightingModeValue` truncation at `% 1000`.** Modes 1001/2001/3001 all collapse to wire byte 1 (Rainbow). The host-side scope distinction (`_Merge`, `_Side`, `_Side_Merge`) is invisible to the firmware. Whether the firmware actually has any concept of these scopes or whether the C# UI just uses them for layout decisions is unclear — a Linux daemon may safely ignore the scope categories on the wire.
- 🚧 **`SetFanGroupLight` (cmd 176) `disabled` semantics asymmetry vs `SetFanLight` (cmd 163).** When `disabled=true`, cmd 163 zeroes mode/brightness/speed/colors/direction/count, but cmd 176 zeroes only the colors and still writes mode/brightness/speed/direction/count. This is likely a C# bug or an artefact of the decompiler; the wire-effect of writing (mode=Rainbow, count=4) with `disabled=1` to a group is unverified.
- 🚧 **`isSync` shortcut in cmd 163.** When `isSync=1`, the high nibble of byte 0 is the *source* port to copy from, while byte 1's high nibble is the target port. The firmware-side semantics — does it copy from the source's last preset, or from the source's currently-rendered LEDs, or from a stored profile slot? — is undocumented.
- 🚧 **Read-after-write discard.** Every write in `TLFanDevice.cs` reads exactly one response packet and discards it. Whether the response carries an ACK byte, an error code, or nothing meaningful is unverified. Real hardware capture should record the response payload for each cmd to determine whether failures can be detected at this layer.
- 🚧 **Fan-curve update cadence.** The service `writeFanSpeedTimer` is `NonOverlappingTimer.StartNew(...)` with no explicit interval at the visible call site; the `updateFanInfoTimer` is at 1000 ms (`TLFanController.cs:1385`). Whether the fan-speed timer matches that 1 Hz cadence or runs faster is not visible in the snippet — needs a wider read of `NonOverlappingTimer` defaults.
