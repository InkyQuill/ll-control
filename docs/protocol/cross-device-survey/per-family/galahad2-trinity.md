## Family: Galahad2 Trinity

**Source folder:** `LConnectCore/Products/Galahad2Trinity/`
**Marketed name(s):** Lian Li Galahad II Trinity AIO liquid cooler — non-LCD ARGB pump-cap variant. Two SKUs:
- Galahad II Trinity (Regular): pump RPM range 2200–3200
- Galahad II Trinity Performance: pump RPM range 2200–4200
**ControllerTypeID enum value(s):**
- `Galahad2TrinityPerformance = 0x01010000` (16842752) — `LConnectCore/Devices/KnownDevices.cs:36`, defined at `LConnectCore/Devices/ControllerTypeID.cs:16`
- `Galahad2TrinityRegular = 0x01010001` (16842753) — `LConnectCore/Devices/KnownDevices.cs:37`, defined at `LConnectCore/Devices/ControllerTypeID.cs:17`
**USB VID:PID:**
- `0x0416:0x7371` (1046:29553) — Trinity Performance, `LConnectCore/Devices/KnownDevices.cs:36`
- `0x0416:0x7373` (1046:29555) — Trinity Regular, `LConnectCore/Devices/KnownDevices.cs:37`
- Both SKUs additionally filter on `UsagePage = 0xFF1B` (65307) and `Usage = 0x91` (145) — `LConnectCore/Devices/ControllerFinder.cs:32`
- VID `0x0416` is the standard Lian Li / Winbond VID also used by the Galahad II LCDV2 (`0x0416:0x7395`)
**Framing classes used:** **A only** (64-byte `ACommandPacket`-equivalent). The Trinity-local class is named simply `CommandPacket` (`LConnectCore/Products/Galahad2Trinity/CommandPacket.cs:10-131`) and is structurally identical to the LCDV2 `ACommandPacket`: ReportID=1, 6-byte header, 58-byte payload, big-endian u16 PacketNumber at bytes[3..4], Length at bytes[5]. No B-class (1024-byte) or C-class (512-byte) framing is referenced anywhere in this folder — consistent with no LCD/JPEG/H.264 path.

**Subsystem fingerprint** (✅ / ❌):
- Sensors / handshake: ✅ (cmd 129; FanRpm + PumpRpm only — **no temperature**, unlike LCDV2)
- Fan PWM: ✅ (cmd 139, `[mb_sync, percent]`)
- Pump PWM: ✅ (cmd 138, `[mb_sync, percent]`)
- Fan curve upload: ❌ (no curve commands; firmware-resident curve plus host-pushed setpoints)
- Lighting (preset modes): ✅ (cmd 131 pump, cmd 133 fan — same 19/20-byte framing as LCDV2, but mode enums diverge — see "Notable divergences")
- Lighting (per-pixel LED): ❌ (no cmd 20 / `SetLEDColors` analog; no per-LED RGB array path — Trinity exposes preset modes only)
- LCD on/off / brightness / rotation: ❌ (no LCD on this SKU)
- LCD frame upload: ❌ (no LCD on this SKU)
- Continuous re-issue pattern: 🚧 unclear from static analysis — service controller not inspected; the device class itself does not loop, but the LCDV2 service is known to re-issue lighting on a timer and Trinity may share that workaround
- Reset / soft-reset: ❌ (no cmd 142 analog in `Commands.cs`; no `ResetDevice` method on the device class)
- Firmware upload / activation: ❌ (no `UploadFirmware` / `ActiveFirmware` analogs in `Commands.cs`; cmds 10/11 are not defined here)

### Commands

| Cmd byte | Direction | Name | Framing | Citation | Status | Notes |
|---|---|---|---|---|---|---|
| 129 (0x81) | host→device + device→host | `Handshake` (RPM telemetry) | A | `Commands.cs:7`; flow `Galahad2TrinityDevice.cs:110-146`; parser `HandshakeInfo.cs:29-44` | 🌗 | Request: bare cmd-129 packet. Response: cmd-129 echo. Host code forces `Length = 4` before parse. Payload: 4 bytes — `FanRpm` BE u16 at [0..2], `PumpRpm` BE u16 at [2..4]. **No temperature byte** (Trinity has no coolant temp sensor exposed). LCDV2 returns 7 bytes including temp; Trinity returns 4 bytes. |
| 130 (0x82) | host→device | `GetPumpLightModeList` | A | `Commands.cs:8` | 🌗 | Listed in command enum but **no caller in the device class** in this folder. Likely consumed only by UI layer outside `LConnectCore`. Response payload format not exercised here; presumed analogous to LCDV2 cmd 130 (escape-byte mode list + max_speed/max_brightness) given mirrored numbering. |
| 131 (0x83) | host→device | `SetPumpLight` (`SetPumpLighting`) | A | `Commands.cs:9`; flow `Galahad2TrinityDevice.cs:191-199`; payload `PumpLightingSetting.cs:77-96` | 🌗 | 19-byte payload, **same offset layout as LCDV2 cmd 131** (scope, mode, brightness, speed, color1..color4 RGB, direction, disabled, source). **Mode-byte semantics diverge** — see "Notable divergences". `(byte)(mode % 1000)` is applied at `PumpLightingSetting.cs:81` to fold a 16-bit mode enum into one byte. |
| 132 (0x84) | host→device | `GetLEDLightModeList` (fan) | A | `Commands.cs:10` | 🌗 | Listed in enum; **no caller in the device class**. Presumed analogous to LCDV2 cmd 132. |
| 133 (0x85) | host→device | `SetLEDLight` (`SetFanLighting`) | A | `Commands.cs:11`; flow `Galahad2TrinityDevice.cs:159-167`; payload `FanLightingSetting.cs:85-105` | 🌗 | 20-byte payload, **same offset layout as LCDV2 cmd 133** (mode, brightness, speed, color1..color4, direction, disabled, source, sync_to_pump, number_of_led). Default `NumberOfLED = 24` matches LCDV2. **Mode-byte set diverges** — Trinity has 16 fan modes (1-16) vs LCDV2's 7 (0-6); see "Notable divergences". |
| 134 (0x86) | host→device + device→host (×2) | `GetProductInfo` (firmware version) | A | `Commands.cs:12`; flow `Galahad2TrinityDevice.cs:41-108`; parser `FirmwareVersion.cs:81-136` | 🌗 | Request: bare cmd-134. Response: **two** cmd-134 packets, both echoes. Packet 0 = ASCII version string (7 comma-separated fields: ChipCode, BootCode, Type, ModelA, ModelB, SoftwareVersion, UserVersion). Packet 1 = ASCII datetime. Trinity reads both packets unconditionally and demuxes by sequence (no PacketNumber filter, unlike LCDV2 firmware which uses PacketNumber to discriminate). |
| 135 (0x87) | host→device | `LEDTestMode` | A | `Commands.cs:13` | ❓ | Defined in command enum; **no caller in the device class**. Payload shape unknown from static analysis. Likely a UI-driven test pattern toggle (analogous to LCDV2's `LCDControlMode::LCDTest` but for LEDs since there is no LCD). |
| 136 (0x88) | host→device + device→host | `ReadSerialNumber` | A | `Commands.cs:14` | ❓ | Defined; no caller in device class. |
| 137 (0x89) | host→device | `WriteSerialNumber` | A | `Commands.cs:15` | ❓ | Defined; no caller in device class. **Write-side / potentially destructive** — gate behind `--yes` if ever exposed. |
| 138 (0x8A) | host→device | `SetPump` (`SetPumpPWM`) | A | `Commands.cs:16`; flow `Galahad2TrinityDevice.cs:201-221` | 🌗 | 2-byte payload: `[mb_sync, percent]`. `mb_sync` is a `bool` cast to byte (0 = MCU curve, 1 = Motherboard PWM passthrough). `percent` is clamped to ≤100 in code. Compare LCDV2 cmd 138 which uses a 1-byte `source` enum (MCU/Motherboard) at offset 0 — **byte 0 layout is semantically identical** (both Trinity's bool and LCDV2's enum encode 0/1 with same meaning). |
| 139 (0x8B) | host→device | `SetFan` (`SetFanPWM`) | A | `Commands.cs:17`; flow `Galahad2TrinityDevice.cs:169-189` | 🌗 | 2-byte payload: `[mb_sync, percent]`. Same shape as cmd 138. **Differs from LCDV2 cmd 139**, where byte 0 is `group` (always 0 on LCDV2) — Trinity uses byte 0 as MB-sync flag. Wire byte is identical when `mb_sync = false`, but a value of 1 means different things on the two devices (group ID vs MB-sync). Constants set `FanPWMMin = 10` (`Constants.cs:7`). |
| 140 (0x8C) | host→device + device→host | `GetFanPumpParams` | A | `Commands.cs:18` | ❓ | Defined; no caller in device class. Likely returns RPM ranges and PWM bounds. |

### Subsystem patterns

- **Single framing class.** Everything goes through 64-byte `CommandPacket` (A-class). Multi-packet writes are supported via `BuildPacketsForWrite` (`CommandPacket.cs:89-117`) but no current command needs them — all defined payloads fit in 58 bytes (max is 20 bytes for cmd 133).
- **No LCD subsystem.** Trinity is the LED-only Galahad II SKU. No `BCommandPacket`, no `SendJpeg` / `LCDControl` / `IsLCDAvailable` / `SetLEDColors` analogs.
- **PWM control: identical 2-byte shape for fan and pump.** Both cmd 138 and cmd 139 use `[mb_sync_bool, percent_clamped_to_100]`. `mb_sync = true` likely means motherboard PWM passthrough; `mb_sync = false` means firmware-resident curve. The C# struct offers no fan-curve upload hook, so the curve is presumably firmware-baked.
- **Lighting control: A-class only.** Both pump (cmd 131) and fan (cmd 133) lighting are sent as single 64-byte packets carrying 19/20-byte payloads — same shape as LCDV2 cmd 131/133. There is no per-LED-array (cmd 20 LCDV2-style) override on Trinity; preset modes are the entire surface.
- **PumpLightingMode is a 16-bit enum folded to a byte via `% 1000`** (`PumpLightingSetting.cs:81`). The host UI presents three "scope" variants per visual mode: `*_Inner` (1xxx range), `*_Outer` (1xxx range), and unscoped (2xxx range). After `% 1000`, all three collapse to the same wire byte (e.g., `Rainbow_Inner = 1`, `Rainbow_Outer = 1001`, `Rainbow = 2001` all serialize to `0x01`). The actual scope distinction is carried separately in `payload[0]` (`scope` byte: Inner/Outer/All). This is structurally redundant — the mode enum encodes scope but it gets stripped at serialization, leaving payload[0] as the authoritative scope.
- **Default lighting source:** both `PumpLightingSetting()` and `FanLightingSetting()` default to `ARGBSignalSource.MCU` (firmware-driven), `Brightness = 2`, `Speed = 2`, `Direction = RightOrClockwise`. Speed/brightness use a 0..=4 wire range with a 0..=100 percent UI mapped via 25-step (`Constants.cs:25-53` — `*Step = 25`).
- **Firmware version: 7-field comma-separated ASCII**, identical structural format to LCDV2 (`ChipCode, BootCode, Type, ModelA, ModelB, SoftwareVersion, UserVersion`) plus a separate datetime packet. Implies a shared firmware string format across the Galahad2 family (and likely beyond, given the `kind`/`model_a`/`model_b` fields are designed to disambiguate SKUs).
- **No `ResetDevice` and no firmware-upload commands.** The Trinity command surface stops at `GetFanPumpParams = 140`. LCDV2 has cmd 142 (Reset), cmd 11 (UploadFirmware), cmd 10 (ActiveFirmware) — all absent here. Either Trinity firmware lacks these features or they're handled by a separate bootloader-mode descriptor not exposed in this folder.

### Notable divergences from LCDV2

**Headline finding — cmd 131 (`SetPumpLighting`) and cmd 133 (`SetFanLighting`) are structurally identical to LCDV2 byte-for-byte at the framing and payload-offset level, but mode-byte semantics diverge.**

Identical:
- Framing: A-class 64-byte packet with the same ReportID=1 / Command / Length / Data layout (`Galahad2Trinity/CommandPacket.cs:10-131` mirrors the `ACommandPacket` shape).
- Cmd 131 payload: 19 bytes total, same offsets — scope (byte 0), mode (1), brightness (2), speed (3), 4×RGB triples (4..16), direction (16), disabled (17), source (18). Compare Trinity `PumpLightingSetting.cs:77-96` to LCDV2 `Galahad2Vision/PumpLightingSetting.cs:101-136` — the `ToBytes()` outputs are positionally identical.
- Cmd 133 payload: 20 bytes total, same offsets — mode (0), brightness (1), speed (2), 4×RGB triples (3..15), direction (15), disabled (16), source (17), sync_to_pump (18), number_of_led (19). Compare Trinity `FanLightingSetting.cs:85-105` to LCDV2 `Galahad2Vision/FanLightingSetting.cs:109-145` — positionally identical.
- Default `NumberOfLED = 24` on both.

Diverges:
- **PumpLightingMode wire-byte set is different.** LCDV2 enumerates 16 modes flat (`Unknown=0` … `Bounce=16`) — `Galahad2Vision/PumpLightingMode.cs:5-24`. Trinity enumerates 36 modes spread across three scope-tagged ranges (1-14 inner, 1001-1016 outer, 2001-2015 unscoped) — `Galahad2Trinity/PumpLightingMode.cs:5-46` — and folds them with `% 1000` to a wire byte in 0..=15. Some shared identifiers (Rainbow, RainbowMorph, StaticColor, Breathing, Runway, Meteor, ColorfulStarryNight, StaticStarryNight, Voice, ColorsMorph, Bounce) match by name, but Trinity lacks LCDV2's TickerTape/Fluctuation/Transmit/Burst and adds TaiChi/Pump/Vortex/CrossingOver/BigBang. Numeric wire-byte values for shared modes are NOT guaranteed equivalent — e.g., on LCDV2 `Rainbow = 1`, on Trinity `Rainbow_Inner = 1` (matches) but `Rainbow_Outer % 1000 = 1` (also serializes to 1, with scope-byte distinguishing). For a mode like `StaticStarryNight`, LCDV2 = 11 vs Trinity (any scope variant) = 11 modulo, so likely matches.
- **FanLightingMode wire-byte set is different.** LCDV2 has 7 modes (`Unknown=0` … `Meteor=6`) — `Galahad2Vision/FanLightingMode.cs:5-14`. Trinity has 16 modes (`Rainbow=1` … `Bounce=16`) with no `Unknown` zero variant — `Galahad2Trinity/FanLightingMode.cs:5-23`. Modes 1-6 match by name on both (Rainbow=1, RainbowMorph=2, StaticColor=3, Breathing=4, Runway=5, Meteor=6) — these likely render identically on the LED MCU. Modes 7-16 are Trinity-only.
- **Pump scope enum is wider on Trinity.** LCDV2 `PumpLightingScope` is `{Inner, Outer, Both}` (or similar 3-value enum); Trinity `LightingScope` is `{Inner=0, Outer=1, All=2}` (`LightingScope.cs:5-10`) and the default is `LightingScope.All` (Trinity, `PumpLightingSetting.cs:67`) vs `LightingScope.Inner` (LCDV2). The wire byte is at the same offset on both.

**Interpretation: the LED MCU firmware is *plausibly* shared across the Galahad2 family for the modes 1-6 (cmd 133) and the basic pump modes that overlap by numeric value, BUT the conclusion is qualified rather than emphatic.** The structural identity of cmd 131/133 framing is strong; the mode-byte set divergence is also strong. Likely the underlying microcode interprets a common subset of mode-bytes consistently, with Trinity's MCU additionally handling a larger mode catalog and LCDV2's MCU handling a different (but overlapping) catalog. Live cross-device byte-replay testing is needed to confirm whether sending Trinity's `BigBang = 13` to an LCDV2 produces a recognizable effect or a no-op/glitch.

Other family-level divergences:
- **No temperature byte in cmd 129 handshake.** Trinity returns 4 bytes (FanRpm + PumpRpm); LCDV2 returns 7 bytes (adds TemperatureIsValid + TemperatureInteger + TemperatureFraction). Trinity has no coolant-temperature sensor exposed via this command.
- **Cmd 139 byte 0 means different things.** LCDV2: `group` (always 0 — `Galahad2Vision/Galahad2VisionDevice.cs:271-281` writes `[0x00, percent]`). Trinity: `mb_sync` bool. A naive byte-for-byte port between the two will yield correct fan PWM only when `mb_sync = false`.
- **No reset, no firmware upload, no LCD.** LCDV2 has cmds 10, 11, 12, 13, 14, 15, 18, 19, 20, 21, 22, 23, 142 — all absent on Trinity. Trinity's command surface is `{129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140}` — 12 commands total vs LCDV2's ~21.
- **Pump RPM range varies by SKU.** Trinity Performance: 2200-4200 rpm; Trinity Regular: 2200-3200 rpm (`Constants.cs:15-21`). Discriminate via VID:PID (29553 vs 29555).

### Open questions / unclear from static analysis

- 🚧 **Cmd 130, 132, 135, 136, 137, 140 payload formats** are not exercised by the Trinity device class in this folder. They are declared in `Commands.cs` but the device class only implements 5 methods (Get/Set Handshake/Firmware/Pump-PWM/Fan-PWM/Pump-Light/Fan-Light). The UI/service layer probably handles them; need to widen the search to `L-Connect3/` or `L-Connect-Service/` to locate call sites.
- 🚧 **Continuous re-issue pattern** — LCDV2's service controller re-issues cmd 131 on a timer (per `galahad2-lcdv2.md` reference to `Galahad2VisionController.cs:2443`). Whether Trinity needs an analogous workaround is unverified; check `Galahad2TrinityController.cs` (outside this folder, in `L-Connect-Service/` or `L-Connect3/`).
- 🚧 **Cmd 135 `LEDTestMode` payload** — speculated to be a one-byte enable flag based on naming; needs the call-site to confirm.
- 🚧 **Cmd 140 `GetFanPumpParams` response** — likely a structure carrying RPM min/max and PWM step info (which on LCDV2 is hardcoded host-side); inferring without a call site is unsafe.
- 🚧 **Whether Trinity supports a `PumpPWMSource` distinction beyond bool.** LCDV2 has a typed `PumpPWMSource` enum (MCU=0, Motherboard=1). Trinity uses a raw `bool mbSync` parameter at the C# API surface (`IGalahad2TrinityDevice.cs:14,18`). Wire byte is 0/1 either way; the distinction is purely typing. No third "mode" appears to exist.
- 🚧 **Cmd 131/133 mode-byte cross-compatibility** — see "Notable divergences". Whether Trinity-family firmware would accept LCDV2 mode bytes or vice versa is testable on hardware but cannot be answered statically.
- 🚧 **`SignalSource` enum** at `SignalSource.cs:5-9` is duplicated alongside `ARGBSignalSource` (`ARGBSignalSource.cs:5-9`) — both are `{MCU, Motherboard}`. Only `ARGBSignalSource` is referenced by the lighting setting structs in this folder. `SignalSource` may be dead code or reserved for the (unimplemented) `LEDTestMode` / `GetFanPumpParams` paths.
- 🚧 **`FanLEDSize` enum** (`FanLEDSize.cs:5-9` — `Size280mm`, `Size360mm`) is not referenced by the device class or `FanLightingSetting`. Likely consumed only by UI for radiator-size presets that influence `NumberOfLED` (e.g., 280mm = 16×2 = 32 LEDs, 360mm = 16×3 = 48 LEDs); inferred but not verified.
