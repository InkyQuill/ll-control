# HydroShift II LCD (HS II) — Cross-Device Survey

## Family

HydroShift II LCD — the **second-generation** AIO with an LCD on the
pump cap (480x480 round-or-square display, customisable themes, modular
widgets). This is a **distinct family** from HS I and the largest
LCD-product family in the survey: it ships in two SKU shapes (Circle /
Square) and supports custom-theme image-or-video backgrounds, font
families, modular layout widgets, and multi-category template management.

It is the **closest LCDV2 LCD-on-pump analog**, and therefore the most
useful family for the cmd-15 (UploadJpeg) activation hypothesis — but
with one crucial caveat documented under "Notable divergences": HS II's
HID protocol is **fully encapsulated inside the proprietary `slv3` SDK**,
so wire-byte semantics are not visible from the open C# source.

**Status legend:** ✅ verified live | 🌗 inferred (static-only) | ❓ unclear.
All claims here are 🌗 inferred unless explicitly noted otherwise — Phase
0e is a static-source survey and no live HS II hardware run has been
performed in this project.

## Control modes

Per Lian Li's published product spec, HS II LCD has three operator-
selectable control modes:

- **Offline** — the device runs from on-board firmware; no host comms
  on the LCD-content path. Fan / pump PWM and lighting may still be
  driven via the wired HID side, but the LCD shows whatever is in
  device-side flash (boot logo or last-applied stored content).
- **Wireless** — the host talks to the device through the L-Wireless
  transmitter dongle using the SL Wireless RF protocol implemented in
  `slv3.dll`. Image content is shipped via `RF_SendPic` chunks (chunked
  through `MasterDevice.SendRfData` into 60-B WinUSB fragments under
  cmd `0x10 Usb_SendRf`); see `slv3-sdk.md`.
- **USB Streaming** — the host talks to the device directly over WinUSB
  using the protocol implemented in `lcd207.dll`. Image content is
  shipped via `PushJpg = 0x65` (transient JPG / `0x66` for PNG) or
  `UpdateFireWare = 0x28` with filename `/usr/data/boot.jpg` (stored
  boot logo, same opcode as firmware update distinguished by filename);
  see `lcd207-sdk.md`.

Both DLLs participate in the host-side stack — which one drives a
particular write depends on the user's chosen control mode at runtime.
A Linux daemon supporting HS II would need to implement both transports
to feature-match the official software (or a documented subset, with
the unsupported mode disabled in the daemon UI).

The two-PID-per-shape USB topology (`...Circle` + `...CircleDisplayMode`,
analogous Square pair) is a separate axis from the three control modes:
the `*DisplayMode` PIDs are the OS-level "secondary monitor" view of
the panel, switchable via the SDK `SetDesktopMode()` / `SetMonitorMode()`
calls inside USB Streaming mode. Wireless mode does not involve the
display-mode PID.

## Source folder

`L-Connect.Core/LConnectCore/Products/HydroShiftIILCD/` plus
`L-Connect.Core/LConnectCore/Products/HydroShiftIILCD/Requests/` (DTOs).

The HID-side controller (the place where wire bytes would normally be
assembled) lives in the **service** project at
`L-Connect-Service/LConnectService/Products/HydroShiftIILCD/`. It does
**not** assemble HID bytes directly; it delegates to the `slv3` SDK via
`H2SDKWrapper` (one of `H2CSDK` for Circle, `H2SSDK` for Square).

Files surveyed (core):

- `RequestType.cs` (high-level controller-side request enum, 60 entries)
- `HydroShiftIILCDConstants.cs` (LCD dimensions, RPM ranges, brightness)
- `HandshakeInfo.cs` (handshake DTO, no command bytes here)
- `HydroShiftIILCDFontFamilies.cs` (8-entry font enum)
- `HydroShiftIILCDColorTypes.cs` (3-entry: solid / horizontal / vertical gradient)
- `HydroShiftIILCDCustomTheme.cs` + `HydroShiftIILCDCustomThemeTypes.cs`
  (image-video / color / theme-info / modulars)
- `HydroShiftIILCDModular.cs` + `HydroShiftIILCDModularSetting.cs`
  + `HydroShiftIILCDModularTypes.cs`
- `HydroShiftIILCDTemplate.cs` + `HydroShiftIILCDTemplateCategory.cs`
- `HydroShiftIILCDThemeInfo.cs` + `HydroShiftIILCDThemeInfoSetting.cs`
- `HydroShiftIILCDLightingSetting.cs` + `H2LightingEffectsSetting.cs`
  + `CLLightingEffectsSetting.cs` + `SyncLightingEffectsSetting.cs`
  + `HydroShiftIILCDLightingViewSetting.cs`
  + `HydroShiftIILCDViewLightingEffectsSetting.cs`
- `HydroShiftIILCDFanSourceSetting.cs`
- `HydroShiftIILCDPreviewBackgroundResults.cs`
- `RotateType.cs`, `SyncEffects.cs`
- `HydroShiftIILCDFirmwareVersion.cs` + Circle/Square subclasses
  + Receiver subclasses
- `HydroShiftIILCDFirmwareUpdateStatus.cs` (enum: None/Ready/Updating/Success/Failed)

Files surveyed (Requests/, all simple DTOs — **none** assemble HID bytes):

- `HydroShiftIILCDChangeTemplateBackgroundRequest.cs`
- `HydroShiftIILCDGetThemeInfoImageRequest.cs`
- `HydroShiftIILCDImportProfileRequest.cs`
- `HydroShiftIILCDRenameTemplateCategoryRequest.cs`
- `HydroShiftIILCDSyncScreenTopLayerImageRequest.cs`
- `HydroShiftIILCDUpdateModularSettingRequest.cs`
- `HydroShiftIILCDUpdateTemplateAssignedCategoriesRequest.cs`
- `HydroShiftIILCDUpdateThemeInfoSettingRequest.cs`

Service-side cross-references:

- `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/HydroShiftIILCDController.cs`
  (5666 lines; dispatch table, timers, SDK glue)
- `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/HydroShiftIILCDReceiverController.cs`
- `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2SDKWrapper.cs`
  (abstract, 30 abstract methods)
- `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2CSDK.cs`
  (Circle SDK shim → `H2Controller` from `slv3`)
- `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2SSDK.cs`
  (Square SDK shim → `H2SController` from `slv3`)

## Marketed names

`L-Connect3/LConnect3cs/Products/HydroShiftIILCD/HydroShiftIILCDController.cs:187-198`
hard-codes the public type names:

| Shape  | Brand string         |
|--------|----------------------|
| Circle | `HydroShift II LCD-C`|
| Square | `HydroShift II LCD-S`|

`H2CSDK.DevType` returns `10`, `H2SSDK.DevType` returns `11`
(`H2CSDK.cs:16`, `H2SSDK.cs:16`) — internal SDK identifiers.

## ControllerTypeID

`ControllerTypeID.cs:31-34,40-41`:

- `HydroShiftIILCDCircle = 16986112`
- `HydroShiftIILCDCircleDisplayMode = 16986113`
- `HydroShiftIILCDSquare = 16986128`
- `HydroShiftIILCDSquareDisplayMode = 16986129`
- `HydroShiftIILCDCircleReceiver = 50540546`
- `HydroShiftIILCDSquareReceiver = 50540562`

`ControllerTypeIDDefinitions.cs:57`:
`ClassHydroShiftIILCD = 208896` (the family-class constant; the
`<shape>Receiver` entries are **not** in `KnownDevices` so they are
reached over the L-Wireless transmitter, not direct USB).

## VID:PID per SKU

`KnownDevices.cs:65-71`:

| ControllerTypeID                    | VID (dec) | PID (dec) | VID:PID (hex)     |
|-------------------------------------|-----------|-----------|-------------------|
| `HydroShiftIILCDCircle`             | 7358      | 40993     | `0x1CBE:0xA021`   |
| `HydroShiftIILCDCircleDisplayMode`  | 6790      | 44320     | `0x1A86:0xAD20`   |
| `HydroShiftIILCDSquare`             | 7358      | 41012     | `0x1CBE:0xA034`   |
| `HydroShiftIILCDSquareDisplayMode`  | 6790      | 44322     | `0x1A86:0xAD22`   |

VID `0x1CBE` is Luminary Micro / TI Stellaris; VID `0x1A86` is QinHeng /
WCH (used for the alternate display-mode interface). This is a **major
divergence** from HS I and Galahad II LCDV2, both of which use VID
`0x0416` (Winbond/Nuvoton). HS II appears to be a TI-Cortex-M-based
redesign rather than a successor to HS I's MCU.

The two PIDs per shape are the same physical device in two USB
interface configurations: one for L-Connect-style HID control
(`...Circle`/`...Square`) and one for OS-level "secondary monitor"
display mode (`...DisplayMode`). The
`SetDesktopMode()`/`SetMonitorMode()` SDK calls toggle between them
(`HydroShiftIILCDController.cs:4940`).

## Framing classes

**Family-specific: opaque (proprietary `slv3` SDK).**

Unlike HS I, Galahad II LCDV2, Galahad2 Trinity, and TLFan — which all
expose `ACommandPacket.cs`, `BCommandPacket.cs`, and/or
`CCommandPacket.cs` source files where the byte layout is plain to read
— HS II has **no equivalent files**. There is no `Commands.cs` wire-byte
enum, no `<X>CommandPacket.cs`, and no `new byte[64]` / `new byte[1024]`
allocations in the open code. The `RequestType` enum is a host-side
operation enum, not a wire-byte enum.

The HID byte protocol is hidden behind the `H2Controller` and
`H2SController` static classes in the `slv3` proprietary DLL
(`H2CSDK.cs:42,47,52,...`; `H2SSDK.cs` analogous). All 30 wrapper
methods (`H2SDKWrapper.cs:31-89`) — including `ApplyTemplate`,
`UpdatePumpFan`, `UpdateRgbData`, `UpdateSensorValues`,
`SetDesktopMode`, `SetMonitorMode`, `FirmwareUpdate`, etc. — go through
this SDK boundary.

The one place we see a raw byte buffer on the C# side is
`HydroShiftIILCDController.cs:379` (`screenFrameBuffer = new byte[921600]`,
which is `480 * 480 * 4` BGRA bytes for one frame) and
`HydroShiftIILCDController.cs:1277`
(`new byte[bitmap.Width * bitmap.Height * 4]`). These are decoded-frame
buffers handed to the SDK, **not** HID packets.

**Implication for Linux port:** unlike LCDV2, HS II cannot be
reverse-engineered solely from the open C# source. Phase 1+ work for HS
II will need either a clean-room reimplementation driven by USB capture,
or a port deferred until after the LCDV2-class baseline is solid.

## Subsystem fingerprint

Expected (per task brief): pump PWM ✅, lighting ✅, LCD on pump ✅, theme
files / templates / modular layouts ✅.

Confirmed via `RequestType.cs:6-69` and the dispatch table at
`HydroShiftIILCDController.cs:2845-3138`:

| Subsystem            | Present | Evidence (RequestType entries)            |
|----------------------|---------|-------------------------------------------|
| Pump PWM             | 🌗      | `SetPumpSpeed` (RequestType.cs:8)         |
| Fan PWM              | 🌗      | `SetFanSpeed` (RequestType.cs:9)          |
| ARGB lighting        | 🌗      | `SetLightingEffects` (RequestType.cs:10), `SyncEffects.cs` (10 modes) |
| LCD on pump          | 🌗      | `SetLCDBrightness`, `SetLCDRotate`, `SetSecondaryScreen` (RequestType.cs:18-20) |
| Templates            | 🌗      | 11 entries: `GetSelectedTemplateId`..`UpdateTemplateAssignedCategories` (RequestType.cs:32-44) |
| Custom themes        | 🌗      | `GetCustomThemeImageVideoFilePath`, `SetCustomThemeImageVideoFilePath`, `ApplyBackgroundOnlyCustomTheme`, plus 9 ThemeInfo entries (RequestType.cs:45-56) |
| Modular widgets      | 🌗      | 11 entries: `GetModulars`..`SaveModularsToAsTemplate` (RequestType.cs:57-66) |
| Wireless integration | 🌗      | `WirelessControllerDispose`, `WiredDispose`, `PlayWirelessContents` (RequestType.cs:12-14); `IsWirelessBound`, `MacStr` on `HandshakeInfo.cs:21,26` |
| Offline mode clock   | 🌗      | `GetOfflineModeClock`, `SetOfflineModeClock` (RequestType.cs:16-17) |
| Profile management   | 🌗      | `SaveProfile`, `ExportProfile`, `ImportProfile`, `ResetProfile` (RequestType.cs:15,29-31) |
| Firmware update      | 🌗      | `GetFirmwareVersion`, `StartFirmwareUpdate`, `CheckFirmwareUpdateStatus` (RequestType.cs:24-26) — **deferred per Phase 1 scope** |

## Commands table (host-side `RequestType`, not wire bytes)

Because there is **no wire-byte enum** in the open source, the table
below lists `RequestType.cs:7-69` operation names with the ordinal value
they take in the C# enum (this is the in-process IPC discriminator, not
a USB byte). All entries are 🌗 inferred and have an opaque framing.

| Ord | RequestType                          | Direction | Notes (file:line)                                                             |
|-----|--------------------------------------|-----------|-------------------------------------------------------------------------------|
| 0   | `GetHandshakeInfo`                   | read      | `RequestType.cs:7`; returns `HandshakeInfo` (`HandshakeInfo.cs`)              |
| 1   | `SetPumpSpeed`                       | write     | `RequestType.cs:8`; range Circle 1600-2450 / Square 1600-3200 RPM (`HydroShiftIILCDConstants.cs:15-21`) |
| 2   | `SetFanSpeed`                        | write     | `RequestType.cs:9`; CL120 200-2150 RPM (`HydroShiftIILCDConstants.cs:11`)     |
| 3   | `SetLightingEffects`                 | write     | `RequestType.cs:10`; selects one of `SyncEffects` (Rainbow..None, 10 entries `SyncEffects.cs`); per-effect setting via `H2LightingEffectsSetting`/`CLLightingEffectsSetting`/`SyncLightingEffectsSetting` |
| 4   | `CheckAdvanceMode`                   | read      | `RequestType.cs:11`                                                           |
| 5   | `WirelessControllerDispose`          | write     | `RequestType.cs:12`                                                           |
| 6   | `WiredDispose`                       | write     | `RequestType.cs:13`                                                           |
| 7   | `PlayWirelessContents`               | write     | `RequestType.cs:14`                                                           |
| 8   | `SaveProfile`                        | write     | `RequestType.cs:15`                                                           |
| 9   | `GetOfflineModeClock`                | read      | `RequestType.cs:16`                                                           |
| 10  | `SetOfflineModeClock`                | write     | `RequestType.cs:17`; SDK call `SetOfflineClock(bool)` (`H2SDKWrapper.cs:73`)  |
| 11  | `SetSecondaryScreen`                 | write     | `RequestType.cs:18`; toggles SDK `SetDesktopMode()` vs `SetMonitorMode()` (`HydroShiftIILCDController.cs:4940`) |
| 12  | `SetLCDBrightness`                   | write     | `RequestType.cs:19`; 0-100 (`HydroShiftIILCDConstants.cs:23-25`)              |
| 13  | `SetLCDRotate`                       | write     | `RequestType.cs:20`; `RotateType` 0/90/180/270 (`RotateType.cs:7-10`)         |
| 14  | `GetIsAvailable`                     | read      | `RequestType.cs:21`                                                           |
| 15  | `SyncScreenTopLayerImage`            | write     | `RequestType.cs:22`; uses shared-memory pointer `(long Address, int ProcessId)` (`HydroShiftIILCDSyncScreenTopLayerImageRequest.cs:8-22`); **see cmd-15 hypothesis below** |
| 16  | `GetPreviewBackground`               | read      | `RequestType.cs:23`; returns `HydroShiftIILCDPreviewBackgroundResults` (jpgBinary + isVideo + isApplied) |
| 17  | `GetFirmwareVersion`                 | read      | `RequestType.cs:24`; returns `HydroShiftIILCDFirmwareVersion` (`HydroShiftIILCDFirmwareVersion.cs`) |
| 18  | `StartFirmwareUpdate`                | write     | `RequestType.cs:25`; **firmware path — deferred per Phase 1 scope; not expanded** |
| 19  | `CheckFirmwareUpdateStatus`          | read      | `RequestType.cs:26`; returns `HydroShiftIILCDFirmwareUpdateStatus` (5-state enum `HydroShiftIILCDFirmwareUpdateStatus.cs`); **firmware — deferred** |
| 20  | `ReloadAssets`                       | write     | `RequestType.cs:27`; reloads template list from disk (`HydroShiftIILCDController.cs:2738-2746`) |
| 21  | `StopScreenRender`                   | write     | `RequestType.cs:28`; calls SDK `PlayStop()` (`HydroShiftIILCDController.cs:5168-5187,5209-5222`) |
| 22  | `ExportProfile`                      | read      | `RequestType.cs:29`                                                           |
| 23  | `ImportProfile`                      | write     | `RequestType.cs:30`; payload `byte[] Data` + `ProfileTypes Type` (`HydroShiftIILCDImportProfileRequest.cs`) |
| 24  | `ResetProfile`                       | write     | `RequestType.cs:31`                                                           |
| 25  | `GetSelectedTemplateId`              | read      | `RequestType.cs:32`                                                           |
| 26  | `GetTemplates`                       | read      | `RequestType.cs:33`; returns `HydroShiftIILCDTemplate[]` from SDK `GetTemplates(isCache)` (`H2SDKWrapper.cs:49`) |
| 27  | `ImportTemplate`                     | write     | `RequestType.cs:34`; SDK `ImportTemplate(filePath)` (`H2SDKWrapper.cs:53`)    |
| 28  | `DeleteTemplate`                     | write     | `RequestType.cs:35`; SDK `RemoveTemplate(id)` (`H2SDKWrapper.cs:61`)          |
| 29  | `ApplyTemplate`                      | write     | `RequestType.cs:36`; SDK `ApplyTemplate(Theme)` (`H2SDKWrapper.cs:31`); **see cmd-15 hypothesis** |
| 30  | `ChangeTemplateBackground`           | write     | `RequestType.cs:37`; payload `string Id, string Path` (`HydroShiftIILCDChangeTemplateBackgroundRequest.cs`) |
| 31  | `RevertTemplateBackground`           | write     | `RequestType.cs:38`                                                           |
| 32  | `GetTemplateCategories`              | read      | `RequestType.cs:39`; returns `Dictionary<string, HydroShiftIILCDTemplateCategory>` |
| 33  | `AddTemplateCategory`                | write     | `RequestType.cs:40`                                                           |
| 34  | `DeleteTemplateCategory`             | write     | `RequestType.cs:41`                                                           |
| 35  | `RenameTemplateCategory`             | write     | `RequestType.cs:42`; payload `string Id, string Name` (`HydroShiftIILCDRenameTemplateCategoryRequest.cs`) |
| 36  | `GetAllTemplateAssignedCategories`   | read      | `RequestType.cs:43`                                                           |
| 37  | `UpdateTemplateAssignedCategories`   | write     | `RequestType.cs:44`; payload `string TemplateId, string[] TemplateCategoryIds` (`HydroShiftIILCDUpdateTemplateAssignedCategoriesRequest.cs`) |
| 38  | `GetCustomThemeImageVideoFilePath`   | read      | `RequestType.cs:45`                                                           |
| 39  | `SetCustomThemeImageVideoFilePath`   | write     | `RequestType.cs:46`                                                           |
| 40  | `ApplyBackgroundOnlyCustomTheme`     | write     | `RequestType.cs:47`                                                           |
| 41  | `GetThemeInfos`                      | read      | `RequestType.cs:48`; returns `HydroShiftIILCDThemeInfo[]`                     |
| 42  | `SetThemeInfoId`                     | write     | `RequestType.cs:49`                                                           |
| 43  | `GetThemeInfoId`                     | read      | `RequestType.cs:50`                                                           |
| 44  | `GetThemeInfoSetting`                | read      | `RequestType.cs:51`; returns `HydroShiftIILCDThemeInfoSetting` (font + color) |
| 45  | `UpdateThemeInfoSetting`             | write     | `RequestType.cs:52`; payload `string Id, FontFamilies FontFamiliy, Color Color` (`HydroShiftIILCDUpdateThemeInfoSettingRequest.cs`) |
| 46  | `SetDefaultThemeInfoSetting`         | write     | `RequestType.cs:53`                                                           |
| 47  | `ApplyThemeInfo`                     | write     | `RequestType.cs:54`; **see cmd-15 hypothesis**                                |
| 48  | `SyncThemeInfoImage`                 | write     | `RequestType.cs:55`; payload `(long Address, int ProcessId)` (`HydroShiftIILCDGetThemeInfoImageRequest.cs`) |
| 49  | `SaveThemeInfoToAsTemplate`          | write     | `RequestType.cs:56`                                                           |
| 50  | `GetModulars`                        | read      | `RequestType.cs:57`; returns `HydroShiftIILCDModular[]` (catalog of widget kinds) |
| 51  | `GetModularSetting`                  | read      | `RequestType.cs:58`                                                           |
| 52  | `GetModularSettings`                 | read      | `RequestType.cs:59`                                                           |
| 53  | `AddModularSetting`                  | write     | `RequestType.cs:60`                                                           |
| 54  | `UpdateModularSetting`               | write     | `RequestType.cs:61`; payload `int Index, HydroShiftIILCDModularSetting Setting` (`HydroShiftIILCDUpdateModularSettingRequest.cs`) |
| 55  | `SetDefaultModularSetting`           | write     | `RequestType.cs:62`                                                           |
| 56  | `DeleteModularSetting`               | write     | `RequestType.cs:63`                                                           |
| 57  | `ApplyModulars`                      | write     | `RequestType.cs:64`; **see cmd-15 hypothesis**                                |
| 58  | `GetModularImage`                    | read      | `RequestType.cs:65`                                                           |
| 59  | `SaveModularsToAsTemplate`           | write     | `RequestType.cs:66`                                                           |
| 60  | `StopContents`                       | write     | `RequestType.cs:67`                                                           |
| 61  | `PlayContents`                       | write     | `RequestType.cs:68`; reapplies rotate + brightness + profile if not in display mode (`HydroShiftIILCDController.cs:2702-2736`) |

The two SKUs (Circle, Square) **share the same RequestType enum** — the
SDK wrapper (`H2CSDK` vs `H2SSDK`) handles per-shape protocol
differences internally. So `Circle + Square combined = 62 distinct
host-side operations` (the same 62 names dispatched to two different
SDK shims).

## Subsystem patterns

### LCD constants

`HydroShiftIILCDConstants.cs:27-31`:

- `ScreenWidth = 480`
- `ScreenHeight = 480`
- `ScreenFrameRate = 24`

Both Circle and Square use the same 480x480 buffer. (The display is
physically circular on Circle and square on Square, but the SDK
back-buffer is always 480x480.) Frame rate matches LCDV2's 24 fps.

`HydroShiftIILCDConstants.cs:7-13`:

- Fan: PWM min 0; CL120 fan speed range 200..2150 RPM; minimum duty 10
- Pump RPM Circle: 1600..2450
- Pump RPM Square: 1600..3200

`HydroShiftIILCDConstants.cs:23-25`: brightness 0..100.

`HydroShiftIILCDConstants.cs:33`: `MaxModularTextLength = 100`
(matches `HydroShiftIILCDModularSetting.cs:147` truncation).

### Refresh timers

`HydroShiftIILCDController.cs` (service) timer cadences:

- `UpdatePumpSpeedInterval = 1000` ms (line 112) - fan/pump RPM read
- `UpdateHandshakeInfoInterval = 2000` ms (line 114) - presence/MAC poll
- `CheckFirmwareVersionInterval = 2000` ms (line 88)
- Sensor values pushed at `TimeSpan.FromSeconds(1)` cadence
  (`HydroShiftIILCDController.cs:2719,4066,4222,4523`)
- All timers are `NonOverlappingTimer` (custom serial-tick wrapper, not
  raw `System.Threading.Timer`)

### Frame buffers

`HydroShiftIILCDController.cs:379`: pre-allocated 921600-byte buffer
(`480 * 480 * 4` BGRA). Frame chunking and packetisation happens inside
the SDK; the C# side hands a complete decoded frame.

### Lighting dispatch

Three separate effect-setting subclasses inherit from
`HydroShiftIILCDLightingSetting` (`HydroShiftIILCDLightingSetting.cs`):

- `H2LightingEffectsSetting` - on-pump LEDs (`H2Effects` from `slv3.models`)
- `CLLightingEffectsSetting` - CL120 fan ring LEDs (`CLEffects` from `slv3.models`)
- `SyncLightingEffectsSetting` - 10-mode unified set
  (`SyncEffects.cs:7-16`: Rainbow, RainbowMorph, StaticColor, Breathing,
  Runway, Meteor, Taichi, Twinkle, Voice, None)

Brightness, speed, and direction enums also come from `slv3.models`
(`RFBrightnessType`, `RFLampEffectSpeedType`, `EffectDirections`).

### Theme model (high level)

Per task brief: brief overview only, no field-by-field expansion. Cite
the type files for follow-up.

The model has four orthogonal layers, captured by
`HydroShiftIILCDCustomThemeTypes.cs:7-10`:
`ImageVideo`, `Color`, `ThemeInfo`, `Modulars`. A `CustomTheme`
(`HydroShiftIILCDCustomTheme.cs`) holds the user's selected
image/video path, a per-id dictionary of saved `ThemeInfoSetting`
entries (font + color), and an ordered list of `ModularSetting` widgets.
"Applied" mirrors of all three exist alongside "Selected" copies — the
typical edit-then-apply pattern. Each `ModularSetting`
(`HydroShiftIILCDModularSetting.cs`) carries up to three text strings
(`Text1`/`Text2`/`Text3`, each capped at 100 chars), three independent
color arrays (`MainColors`, `Colors1`, `Colors2`) with per-array
`ColorType` (solid / horizontal-gradient / vertical-gradient,
`HydroShiftIILCDColorTypes.cs:7-9`), a `FontFamiliy` (sic),
`DataSourceName`, X/Y position, and a `ZoomRate` (default 100). Modular
widget kinds (`HydroShiftIILCDModularTypes.cs:7-10`):
`General`, `WirelessLCD`, `DateTime`, `CustomText`. The widget catalog
itself (`HydroShiftIILCDModular.cs`) declares per-kind constraints:
`MinWidth`, `MinHeight`, `MinZoomRate` (=100), `MaxZoomRate`,
`SupportedDataSources` (each with `Id`, `Placeholder1`, `Placeholder2`,
optional `IsFixedName`/`FixedName`).

Font families enum (`HydroShiftIILCDFontFamilies.cs:7-14`): 8 entries -
AbrilFatface, AgencyGothicCT_Bold, BakerieSmooth_WideMediumRegular,
Caprasimo_Regular, HyperspaceRace_BoldItalic, NotoSansTC_Regular (TC =
Traditional Chinese), NotoSansTC_Bold, Cubic11.

Templates (`HydroShiftIILCDTemplate.cs`): `Id`, `PreviewImagePath`,
`Deletable`. Categories (`HydroShiftIILCDTemplateCategory.cs`): `Id`,
`Name`, `Renamable`, `Deletable`. Many-to-many between templates and
categories via `UpdateTemplateAssignedCategories`.

### Firmware versioning (read-only metadata; firmware path deferred)

Two separate version regexes:

- Circle: `^lianlih2_(\d{4})_(\d{4})$`
  (`HydroShiftIILCDCircleFirmwareVersion.cs:13`)
- Square: `^lianlih2s_(\d{4})_(\d{4})$`
  (`HydroShiftIILCDSquareFirmwareVersion.cs:13`)

Receivers carry an int `RawValue` mapped to a `Version` via
`Math.Max(rawValue, 11)-2` (Circle) or `Math.Max(rawValue, 18)-10`
(Square) (`HydroShiftIILCDCircleReceiverFirmwareVersion.cs:14`,
`HydroShiftIILCDSquareReceiverFirmwareVersion.cs:14`).

### Display-mode toggle (the "secondary monitor" path)

`HydroShiftIILCDController.cs:4940`:
`SetSecondaryScreen(true)` → SDK `SetDesktopMode()` (device exposes a
USB display interface to the OS; `...DisplayMode` PIDs apply). `false`
→ SDK `SetMonitorMode()` (LCD-control HID interface; `...Circle`/
`...Square` PIDs apply).

### Wireless / receiver pattern

`KnownDevices.cs` does **not** map the two `*Receiver` ControllerTypeIDs
to USB IDs — only the LCD itself does. Wireless association is mediated
by the L-Wireless transmitter (named-pipe IPC,
`L-Connect.Core/LConnectCore/Products/LWireless/HydroShiftIILCD/*`),
which exposes wireless DTOs:
`HydroShiftIILCDWirelessTemplate`, `HydroShiftIILCDBrightnessSetting`,
`HydroShiftIILCDRotateSetting`, `LWirelessPumpConfig`,
`WirelessAdvanceModeRequest`, `WirelessH2SensorUpdateSetting`,
`WirelessTemplateRequest`, `ScreenRotate`. Direct HID to a Receiver is
not used.

### Fan source mapping

`HydroShiftIILCDFanSourceSetting.cs:7-15` is a tuple
`(SourceName, AliasName)`. The detailed source enumeration is hidden in
the SDK (`H2SDKWrapper.UpdatePumpFan(int pump, int fanSpeed1,
int fanSpeed2, int fanSpeed3)` — three independent fan channels per
pump unit, `H2SDKWrapper.cs:81`).

## Notable divergences (vs Galahad II LCDV2 baseline)

These are the points where HS II differs most from
`docs/protocol/galahad2-lcdv2.md`. They are critical context for the
Linux port roadmap.

1. **No exposed wire-byte protocol in the per-family C# source.** LCDV2
   has a public `Command` enum and `ACommandPacket`/`BCommandPacket`/
   `CCommandPacket` source files. HS II has **none of these** (no
   `Commands.cs`, no packet files, no `new byte[64]` in `Products/HydroShiftIILCD/`).
   All wire-level work is performed inside two proprietary DLLs that
   participate depending on the user's selected control mode (see
   "Control modes" above): `slv3.dll` for **Wireless** mode (via
   `RFController` + `MasterDevice`; see `slv3-sdk.md`) and `lcd207.dll`
   for **USB Streaming** mode (via static classes `H2Controller` /
   `H2SController`; see `lcd207-sdk.md`). Both wire formats are now
   resolved by the SDK surveys; the original "in slv3.dll" claim from
   the first pass of this writeup was partially right (Wireless) and
   partially wrong (USB Streaming, which is in lcd207.dll). HS II uses
   **WinUSB**, not HID, for frames in USB Streaming mode; HID is only
   used out-of-band for SN discovery and `SetMonitorMode` magic-byte
   signalling.

2. **Different MCU vendor.** LCDV2 = `0x0416` (Winbond/Nuvoton).
   HS II = `0x1CBE` (TI Stellaris) for the HID interface and `0x1A86`
   (WCH) for the display-mode interface. Different vendor, different
   stack — the LCDV2 codecs almost certainly do not transfer.

3. **Two-PID-per-shape design.** LCDV2 is single-PID. HS II ships each
   shape under two PIDs (`...Circle` + `...CircleDisplayMode` and
   analogous Square pair) and switches at runtime via SDK
   `SetDesktopMode()` / `SetMonitorMode()` calls. Linux port will need
   to handle two distinct USB interfaces per device.

4. **High-level theme/template/modular system.** LCDV2 only has
   transient image streaming + a stored AVI startup movie. HS II has a
   full theme engine — categorisable templates, persistent custom
   themes with image/video/color/text-info/modular-widget overlays,
   font families, multi-string modular widgets with per-string color
   gradients. This is "much closer to a TV than to a status LED" and is
   the key reference for Phase 4 theme-engine design.

5. **Different RPM ranges.** LCDV2 follows Galahad2 family limits.
   HS II Circle pump = 1600..2450, Square pump = 1600..3200
   (`HydroShiftIILCDConstants.cs:15-21`).

6. **No `SettingType` enum.** LCDV2 (and HS I, Galahad2 Trinity, TLFan)
   subdivide writes by a per-product `SettingType`. HS II flattens
   everything into a single 60+ entry `RequestType` instead — operations
   *and* read/write distinction live in one enum.

7. **No `Commands.cs` byte map.** Sibling LCD/AIO families (HS I, LCDV2,
   Galahad2 Trinity) all have a `Commands.cs` file enumerating wire
   byte values. HS II has nothing of the kind.

8. **In-process shared-memory image transfer.** Two `RequestType`
   entries (`SyncScreenTopLayerImage`, `SyncThemeInfoImage`) pass image
   payloads as `(long Address, int ProcessId)` tuples
   (`HydroShiftIILCDSyncScreenTopLayerImageRequest.cs:8-22`,
   `HydroShiftIILCDGetThemeInfoImageRequest.cs:8-22`) — the GUI hands a
   pointer to the service. This is a Windows IPC detail (not over HID),
   and it does not exist in LCDV2. Linux will need a different transport
   (e.g. memfd_create / unix domain socket).

## Cmd-15 hypothesis: HydroShift II findings

**Critical caveat:** because HS II's wire-byte protocol is sealed inside
the proprietary `slv3` SDK, we cannot directly identify a wire byte
`0x15` here. What we **can** identify is the **set of high-level
operations that match the cmd-15 semantic shape** ("activate stored
content" / "apply image-or-theme that is already on the device's
flash"), as candidates for what the SDK might internally emit.

Comparison against HS I's smoking-gun (`UploadStartupAviMovie = 22`,
which suggests `Send*` = transient, `Upload*` = stored on device):

HS II does **not** carry the `Send*`/`Upload*` distinction at the C#
level — the verb pair is gone, since the SDK swallows it. However, HS
II **does** distinguish between **transient sync** ("show this image
once") and **persistent apply** ("activate this stored layer") by
naming pattern: `Sync*` for transient, `Apply*` for persistent.

### Candidate activate-style commands

These are the entries that match the cmd-15 hypothesis ("activate
stored content"):

#### 1. `ApplyTemplate` (host-ord 29)

- **Cmd byte (wire):** opaque (sealed in `slv3`)
- **Framing class:** opaque (proprietary SDK)
- **Citation:** `RequestType.cs:36`; SDK call
  `H2SDKWrapper.cs:31` (`bool ApplyTemplate(Theme theme)`); service
  dispatch `HydroShiftIILCDController.cs:2154-2172,2978`; private
  helper that calls SDK at `HydroShiftIILCDController.cs:677`
- **What it triggers:** activates a stored template (image background +
  theme-info text overlay + modular widgets) by id. The template is
  already known to the device (uploaded via `ImportTemplate`,
  `RequestType.cs:34`). This is the closest functional analog to
  "display the AVI that's already in flash."

#### 2. `ApplyThemeInfo` (host-ord 47)

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:54`; service dispatch
  `HydroShiftIILCDController.cs:2174-2198,3068`; private helper at
  `HydroShiftIILCDController.cs:683`
- **What it triggers:** activates a previously-stored theme-info
  setting (font + color) on top of the current background. Pre-stored
  state (`SelectedThemeInfoId` lookup is mandatory before apply).

#### 3. `ApplyModulars` (host-ord 57)

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:64`; service dispatch
  `HydroShiftIILCDController.cs:2123-2141,3118`
- **What it triggers:** activates the currently-edited list of modular
  widgets onto the LCD. State already persisted via
  `UpdateModularSetting` / `AddModularSetting`.

#### 4. `ApplyBackgroundOnlyCustomTheme` (host-ord 40)

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:47`; service dispatch
  `HydroShiftIILCDController.cs:2143-2152,3033`
- **What it triggers:** activates **just** the background image-or-video
  layer of a custom theme (no text/widgets). This is the cleanest
  cmd-15 analog because it's the most narrowly-scoped activation
  operation — no companion data, just "show the background that was
  already set."

#### 5. `PlayContents` (host-ord 61)

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:68`; service dispatch
  `HydroShiftIILCDController.cs:2702-2736,3138`
- **What it triggers:** kicks the screen back into render mode
  (re-applies rotate, brightness, and profile). Possibly an analog to
  "show whatever is currently programmed." Counterpart is
  `StopContents` (host-ord 60, `RequestType.cs:67`) which calls SDK
  `PlayStop()` (`HydroShiftIILCDController.cs:5214`).

### Sync-style (transient) candidates for the inverse pole

#### 6. `SyncScreenTopLayerImage` (host-ord 15) — number coincidence noted

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:22`; payload
  `HydroShiftIILCDSyncScreenTopLayerImageRequest(long address, int processId)`
  (`HydroShiftIILCDSyncScreenTopLayerImageRequest.cs:6-22`)
- **What it triggers:** GUI hands the service a shared-memory pointer
  to a fresh image; service hands it (decoded) to the SDK to display
  immediately as a top-layer overlay. **Transient** (re-sent every
  frame at ~24 fps). Numerically this is host-ord 15, but that's the
  in-process enum index in `RequestType.cs`, not a USB wire byte —
  the coincidence with HS I `UploadJpeg = 15` is **almost certainly
  spurious** but flagged here for completeness.

#### 7. `SyncThemeInfoImage` (host-ord 48)

- **Cmd byte (wire):** opaque
- **Framing class:** opaque
- **Citation:** `RequestType.cs:55`; payload identical shape to
  `SyncScreenTopLayerImage`
  (`HydroShiftIILCDGetThemeInfoImageRequest.cs:6-25`)
- **What it triggers:** transient theme-info preview render, same
  shared-memory pointer protocol.

### Activate-step shape for the image-upload sequence

HS II's image-upload-and-display sequence has **three distinguishable
steps at the C# level**, suggesting the underlying wire protocol does
have a separate "activate" concept:

1. **Path-set** (no upload yet): `SetCustomThemeImageVideoFilePath`
   (`RequestType.cs:46`) — points the profile at a host-side file.
2. **Implicit upload** (when applied): the SDK (`ApplyTemplate` /
   `ApplyBackgroundOnlyCustomTheme`) reads the file, decodes/transcodes,
   and uploads to device flash. We cannot see the wire-level chunking.
3. **Activate**: `ApplyBackgroundOnlyCustomTheme` /
   `ApplyTemplate` / `ApplyThemeInfo` / `ApplyModulars` —
   the four `Apply*` verbs. These operate on
   already-stored-on-device state.

So **yes**, HS II has a separate "activate stored content" step,
distinct from the upload step. This **supports** the cmd-15 hypothesis
that LCDV2 might similarly have a separate "show what's in flash" wire
byte — and the four `Apply*` operations are good candidates for what
that byte triggers semantically.

## Open questions

> **Wire-format opacity is now closed for both transport-bearing modes**:
> Wireless mode = slv3-sdk (`slv3-sdk.md`); USB Streaming mode =
> lcd207-sdk (`lcd207-sdk.md`). The remaining open questions are about
> HS II's higher-level orchestration and Linux-side feature parity.

1. **cmd-15 hypothesis from HS II:** does
   `ApplyBackgroundOnlyCustomTheme` (`RequestType.cs:47`) emit a single
   wire-byte command equivalent to LCDV2 cmd `0x15`?
   - This is the strongest candidate because it's the most narrowly-
     scoped activation: no payload data, just "switch on the stored
     background." Phase 1 USB capture should record this operation in
     isolation against an HS II Circle and/or Square.

2. **cmd-15 hypothesis from HS II:** does `ApplyTemplate`
   (`RequestType.cs:36`) emit cmd `0x15` plus a template-id parameter?
   - If so, LCDV2 cmd `0x15` may take an inline parameter (e.g. layer
     index). Worth comparing the payload shape between
     `ApplyBackgroundOnlyCustomTheme` (no id) and `ApplyTemplate`
     (string id).

3. **cmd-15 hypothesis from HS II:** is `ApplyThemeInfo`
   (`RequestType.cs:54`) a separate wire byte (e.g. `0x16`) or a
   variant of cmd `0x15`?
   - Three "apply" verbs with no shared parameter type suggests three
     distinct wire bytes, not one polymorphic byte.

4. **cmd-15 hypothesis from HS II:** is `PlayContents`
   (`RequestType.cs:68`) a "resume rendering" command (cmd `0x15`-ish)
   or a "set state" command (different byte)?
   - Counterpart `StopContents` calls SDK `PlayStop()`
     (`HydroShiftIILCDController.cs:5214`); both pair like a render-on
     / render-off toggle.

5. **cmd-15 hypothesis from HS II:** is the host-ord 15 numeric
   coincidence (`SyncScreenTopLayerImage = RequestType[22] but enum
   index 15 in some encodings`) meaningful?
   - Almost certainly **no**. The `RequestType` enum is in-process
     C# IPC, ordered by feature group, not wire byte. The coincidence
     should be ruled out by capture, not relied on.

6. **Is the `slv3` DLL the same binary as on the LCDV2 install?**
   - If yes, dynamic analysis of one may inform the other. Out of
     scope for the open-source survey but a candidate for Phase 1
     reverse-engineering work.

7. **What does USB display-mode interface (PID `0xAD20` / `0xAD22`,
   VID `0x1A86`) carry?**
   - Likely a USB-display class (UVC-like) for OS-level "secondary
     monitor" usage. Not the HID command channel. Worth a `lsusb -v`
     pass on a real device.

8. 🚧 **Receiver firmware version meaning.**
   `HydroShiftIILCDCircleReceiverFirmwareVersion.cs:14` does
   `Math.Max(rawValue, 11) - 2`; Square does `Math.Max(rawValue, 18) - 10`.
   The hard-coded `11`/`18`/`-2`/`-10` mean we cannot decode receiver
   firmware versions below those thresholds, only render them as a
   floor. Significance unclear — possibly compatibility flooring for
   older firmware.

9. 🚧 **Two-PID switching mechanics.** Switching from
   `...Circle` to `...CircleDisplayMode` PID requires the device to
   re-enumerate over USB. Whether the SDK's
   `SetDesktopMode()`/`SetMonitorMode()` triggers a USB
   bus-reset, a SET_CONFIGURATION, or an out-of-band command channel is
   not visible from the C# code.
