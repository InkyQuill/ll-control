## Family: Lancool 207 Digital

**Source folder:** `LConnectCore/Products/Lancool207Digital/` (plus
the service-side controller at
`L-Connect-Service/LConnectService/Products/Lancool207Digital/Lancool207DigitalController.cs`,
~5 000 lines).

**Marketed name(s):** Lian Li **LANCOOL 207 Digital** — case with a
front-mounted high-resolution wide LCD (1472×720) showing live
sensor data, configurable templates, and customizable modular
widgets. Two transport ControllerTypeIDs:
`Lancool207Digital` (the active control endpoint) and
`Lancool207DigitalDisplayMode` (the display-mode-only endpoint —
used when the LCD is in passive monitor mode).

**Note:** Lancool **217** Digital is a **different product** — it
appears as a wireless child device behind the LWireless hub
(`LWireless/Lancool217Inf/`) and is documented in
`per-family/lwireless.md`.

**ControllerTypeID enum value(s):**
`Lancool207Digital = 17039360`,
`Lancool207DigitalDisplayMode = 17039361`
(`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:35-36`).

**USB VID:PID:** two endpoints (`KnownDevices.cs:73-75`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | UsageID | Source |
|---|---|---|---|---|
| `Lancool207Digital` (17039360) | `0x1CBE:0xA065` | `7358:41061` | (none) | `KnownDevices.cs:73` |
| `Lancool207DigitalDisplayMode` (17039361) | `0x1A86:0xAD11` | `6790:44305` | (none) | `KnownDevices.cs:75` |

VID `0x1CBE` (decimal `7358`) is Lian-Li's own USB vendor ID — also
used by `SLV3LCD` (`0x1CBE:0x0005`), `TLV2LCD` (`0x1CBE:0x0006`),
`HydroShiftIILCDCircle` (`0x1CBE:0xA021`), `HydroShiftIILCDSquare`
(`0x1CBE:0xA034`), and `UniversalScreen8p8Inch` (`0x1CBE:0xA088`).
This is the **modern Lian-Li-internal MCU family** for the LCD-class
products. VID `0x1A86` (QinHeng / WCH) on the DisplayMode endpoint
matches the DisplayMode-only endpoints of HydroShift II LCD and
Universal Screen 8p8 — implying a **shared LCD-DisplayMode bridge
silicon** across these LCD-class products in Lian-Li's lineup.

**Framing classes used:** **opaque — proprietary `lcd207` SDK.** The
controller imports `using lcd207;`
(`L-Connect-Service/LConnectService/Products/Lancool207Digital/Lancool207DigitalController.cs:2`)
and dispatches every device-touching operation through a static
`LCDController` class (e.g.,
`LCDController.IsAvailable()`,
`LCDController.SetBrightness(brightness)`,
`LCDController.ApplyTemplate(theme)` —
`Lancool207DigitalController.cs:1137, 4330, 507`). The actual HID
ReportID, framing class, and per-command byte layouts are
**invisible from the L-Connect codebase** — encapsulated inside
`lcd207.dll` (a native binary at `winver/L-Connect 3/lcd207.dll`,
not in this tree).

```
Lancool207DigitalController (this codebase)
        |
        v   (delegates to)
LCDController (lcd207.dll, third-party / proprietary)
        |
        v   (HID ReportID + framing not visible from C#)
USB HID device (VID 0x1CBE:0xA065 control, 0x1A86:0xAD11 display)
```

The control flow inside the C# layer is purely:
1. Parse JSON request (`Lancool207DigitalRequestType`).
2. Update host-side `Lancool207DigitalProfile` JSON state.
3. Compose a `Theme` (with applied modulars / theme-info) using
   `LCDController.UpdateModular(theme, updateSet)`,
   `LCDController.UpdateThemeInfo(theme, ...)`, etc. — these
   manipulate an opaque `Theme` model from the SDK.
4. Hand the composed `Theme` to `LCDController.ApplyTemplate(theme)`
   to push the rendered frame to the device.

`Phase 0d status:` not in scope; transport is opaque.

**Subsystem fingerprint (via SDK API surface):**
- Sensors / handshake: **yes** —
  `LCDController.IsAvailable()`
  (`Lancool207DigitalController.cs:1137`),
  `LCDController.GetDeviceVer(action)`
  (`Lancool207DigitalController.cs:1094`),
  `LCDController.get_Ver()` (`:1108`).
  Sensor-value updates run on a host-side `NonOverlappingTimer`
  (`sensorValueUpdateTimer` — `:77-79`) that polls system sensors
  (HWiNFO, etc.) and feeds them into the LCD frame composition.
- Fan PWM: **N/A on the LCD itself.** The Lancool 207 Digital case
  has its own fans, but those are configured via
  `Lancool207DigitalFanSourceSetting`
  (`Lancool207DigitalFanSourceSetting.cs:1-23`) which selects
  *which sensor source* drives the displayed fan readout — not
  fan-PWM control. Actual fan control on the case probably runs
  through the motherboard's fan headers, not via the digital LCD
  controller. **Flagged unclear.**
- Pump PWM: **N/A** — no pump on this case.
- Fan curve upload: **N/A.**
- Lighting (preset modes): **N/A on the LCD path.** The Lancool 207
  Digital is a screen device; any case lighting it carries is
  controlled separately.
- Lighting (per-pixel LED): **N/A on the LCD path.**
- LCD on/off / brightness / rotation: **yes** —
  `LCDController.SetBrightness(brightness)`
  (`Lancool207DigitalController.cs:4330`).
  Brightness range 0..=100 (`Lancool207DigitalConstants.cs:7-9` —
  `ScreenBrightnessMin = 0`, `ScreenBrightnessMax = 100`).
  No rotation API surfaced — the screen is fixed-orientation
  (1472×720 landscape).
- LCD frame upload: **yes via SDK theme / template / modular
  composition.** Frames are not raw bitmaps; they're composed
  client-side as a `Theme` (template + modulars + theme-info
  variables) and pushed via `LCDController.ApplyTemplate`. Image
  syncing for top-layer overlay images uses the
  `Lancool207DigitalSyncScreenTopLayerImageRequest` — and reads
  bytes from another process's memory via `copyImageMemory(processId,
  address, screenFrameBuffer)`
  (`Lancool207DigitalController.cs:3306-3320`), which is a
  cross-process shared-memory IPC pattern (not a named pipe).
- Continuous re-issue pattern: **yes** — the firmware-version
  checker (`checkFirmwareVersionTask`,
  `Lancool207DigitalController.cs:111-113, 4516-4540`) polls every
  2000 ms, and the sensor value update timer drives re-renders for
  live-data widgets.
- Reset / soft-reset: **inferred** —
  `Lancool207DigitalRequestType.ResetProfile` (idx 18) resets the
  host-side profile JSON; whether it also issues an SDK reset is
  unclear. **No `ResetDevice`** entry in the request enum.
- Firmware upload / activation: **yes** —
  `StartFirmwareUpdate` (idx 12) /
  `CheckFirmwareUpdateStatus` (idx 13) form an upload + poll-status
  pair (`Lancool207DigitalController.cs:2628-2637, 3776-3794`).
  An `isFirmwareUpdating` flag gates all other request handling
  (`:2562-2565`).
- Crypto / obfuscation at HID transport: **unknown** — opaque
  inside `lcd207.dll`.

### Special: cross-process shared-memory image transport

For top-layer image overlays and theme-info image syncing, the
controller reads pixel data **directly from another process's
address space** rather than via pipe or HID:

```
Lancool207DigitalSyncScreenTopLayerImageRequest:
  - ProcessId    (int, must be > 0)
  - Address      (memory address inside that process)

handleSyncScreenTopLayerImageRequest():
  - copyImageMemory(processId, address, screenFrameBuffer)
    (Lancool207DigitalController.cs:3316)
```

`copyImageMemory` (declared at `:3316, 3375`) — implementation not
shown — almost certainly wraps Windows' `OpenProcess` +
`ReadProcessMemory` to copy a pre-rendered frame buffer from the
UI process into the service's `screenFrameBuffer`. **This pattern
is Windows-specific**; Linux equivalents (`process_vm_readv` or a
shared-memory fd via `memfd_create` + `SCM_RIGHTS`) would need to
replace it.

`screenFrameBuffer` is sized by `screenFrameRect`
(`:107`) which corresponds to `1472 * 720 * <bpp>` bytes
(probably 4 for RGBA). The buffer is then handed to the SDK to
compose into a `Theme` and apply to the device.

This is a different IPC pattern from the LWireless named pipe, and
is Lancool 207 Digital-specific.

### Commands (logical Lancool207DigitalRequestType enum, 54 entries)

The enum at `Lancool207DigitalRequestType.cs:5-61` carries 54
host-side dispatch entries. Wire layer is opaque
(through `lcd207.dll`), so the column "SDK call" is the visible
SDK surface; status flag is `inferred-static` for all entries.

| Idx | Name | Handler citation | SDK call (where visible) | Notes |
|---|---|---|---|---|
| 0 | `SaveProfile` | `Lancool207DigitalController.cs:2570` | (host-only — saves JSON) | Persists profile via debounced disk write. |
| 1 | `GetOfflineModeClock` | `:2575` | (host-only — JSON read) | Returns offline-mode clock state. |
| 2 | `SetOfflineModeClock` | `:2580` | `LCDController.SetOfflineClock(isEnabled)` (`:4453`) | Toggles offline clock display. |
| 3 | `SetSecondaryScreen` | `:2585` | `LCDController.SetDesktopMode()` / `SetMonitorMode()` (`:4485`) | Switches display role. |
| 4 | `GetBrightness` | `:2590` | (lookup) | Returns current brightness 0..=100. |
| 5 | `SetBrightness` | `:2595` | `LCDController.SetBrightness(brightness)` (`:4330`) | u8 0..=100. |
| 6 | `GetIsAvailable` | `:2600` | `LCDController.IsAvailable()` (`:1137`) | Health probe. |
| 7 | `GetFanSourceSettings` | `:2605` | (host-only — JSON read) | Per-display-fan sensor source. |
| 8 | `SetFanSourceSettings` | `:2610` | (host-only) | See `Lancool207DigitalFanSourceSetting.cs`. |
| 9 | `SyncScreenTopLayerImage` | `:2615` | `copyImageMemory(processId, address, screenFrameBuffer)` (`:3316`) | **Cross-process shared-memory read.** Carries `processId` + `address`. |
| 10 | `GetPreviewBackground` | `:2620` | (host-side composition) | Returns a preview JPEG of the current background. |
| 11 | `GetFirmwareVersion` | `:2625` | `LCDController.GetDeviceVer` / `get_Ver` (`:1094, 1108`) | Returns `Lancool207DigitalFirmwareVersion`. |
| 12 | `StartFirmwareUpdate` | `:2630` | (SDK) | Begins firmware blob upload. **Destructive.** |
| 13 | `CheckFirmwareUpdateStatus` | (special-cased before `isFirmwareUpdating` check, `:2558`) | (SDK) | Always-allowed poll during update. |
| 14 | `ReloadAssets` | `:2640` | (SDK) | Reload bundled asset pack. |
| 15 | `StopScreenRender` | `:2645` | (SDK) | Halt the rendering pipeline. |
| 16 | `ExportProfile` | `:2650` | (host-only — gzip write) | Export profile to disk (`:4218`). |
| 17 | `ImportProfile` | `:2655` | (host-only) | Import profile + assets. |
| 18 | `ResetProfile` | `:2660` | (host-only) | Reset to factory defaults. |
| 19 | `GetSelectedTemplateId` | `:2665` | (lookup) | Currently-applied template. |
| 20 | `GetTemplates` | `:2670` | `LCDController.GetTemplates(true)` (`:1634`) | List of stored templates. |
| 21 | `ImportTemplate` | `:2675` | `LCDController.ImportTemplate(path)` (`:3723`) | Import a template file. |
| 22 | `DeleteTemplate` | `:2680` | `LCDController.RemoveTemplate(id)` (`:4045`) | Delete a stored template. |
| 23 | `ApplyTemplate` | `:2685` | `LCDController.ApplyTemplate(theme)` (`:507`) | **Renders to display.** |
| 24 | `ChangeTemplateBackground` | `:2690` | (host-side + SDK save) | Carries a `Lancool207DigitalChangeTemplateBackgroundRequest`. |
| 25 | `RevertTemplateBackground` | `:2695` | (host-side) | Reverts to packaged background. |
| 26 | `GetTemplateCategories` | `:2700` | (host-side) | List user-defined categories. |
| 27 | `AddTemplateCategory` | `:2705` | (host-side) | Add a category. |
| 28 | `DeleteTemplateCategory` | `:2710` | (host-side) | Remove a category. |
| 29 | `RenameTemplateCategory` | `:2715` | (host-side) | Carries a `Lancool207DigitalRenameTemplateCategoryRequest`. |
| 30 | `GetAllTemplateAssignedCategories` | `:2720` | (host-side) | Reads category-assignment map. |
| 31 | `UpdateTemplateAssignedCategories` | `:2725` | (host-side) | Updates assignments. |
| 32 | `GetCustomThemeImageVideoFilePath` | `:2730` | (host-side) | Path to currently-selected custom-theme media. |
| 33 | `SetCustomThemeImageVideoFilePath` | `:2735` | (host-side) | Set new path. |
| 34 | `ApplyBackgroundOnlyCustomTheme` | `:2740` | (SDK) | Apply a background-only custom theme. |
| 35 | `GetThemeInfos` | `:2745` | `LCDController.GetThemeInfos()` (`:1702`) | List built-in themes. |
| 36 | `SetThemeInfoId` | `:2750` | (host-side) | Select active theme-info. |
| 37 | `GetThemeInfoId` | `:2755` | (host-side) | Read selected theme-info. |
| 38 | `GetThemeInfoSetting` | `:2760` | (host-side) | Get per-theme-info settings (color, font). |
| 39 | `UpdateThemeInfoSetting` | `:2765` | `LCDController.UpdateThemeInfo(theme, name, fontStyle, color)` (`:1024`) | Mutate theme-info on the composed `Theme`. |
| 40 | `SetDefaultThemeInfoSetting` | `:2770` | (host-side) | Reset to default. |
| 41 | `ApplyThemeInfo` | `:2775` | (SDK) | Apply theme-info to display. |
| 42 | `SyncThemeInfoImage` | `:2780` | `copyImageMemory(processId, address, currentThemeInfoImage)` (`:3375`) | **Cross-process shared-memory read** for per-theme image. |
| 43 | `SaveThemeInfoToAsTemplate` | `:2785` | `LCDController.SaveTemplate(theme, false)` (`:4093`) | Save current theme-info as a new template. |
| 44 | `GetModulars` | `:2790` | `LCDController.GetModulars()` (`:1225`) | List available modular-widget definitions. |
| 45 | `GetModularSetting` | `:2795` | (host-side) | Read a modular's config. |
| 46 | `GetModularSettings` | `:2800` | (host-side) | Read all modulars' config. |
| 47 | `AddModularSetting` | `:2805` | (host-side + SDK compose) | Add a modular instance. |
| 48 | `UpdateModularSetting` | `:2810` | `LCDController.UpdateModular(theme, updateSet)` (`:1427`) | Mutate a modular widget. Carries a `Lancool207DigitalUpdateModularSettingRequest`. |
| 49 | `SetDefaultModularSetting` | `:2815` | (host-side) | Reset modular to default. |
| 50 | `DeleteModularSetting` | `:2820` | (host-side) | Remove a modular instance. |
| 51 | `ApplyModulars` | `:2825` | (SDK) | Apply modulars to display. |
| 52 | `GetModularImage` | `:2830` | `LCDController.GetBitmapByTheme(modularWithSetting)` (`:1168`) | Render a modular to a bitmap for preview. |
| 53 | `SaveModularsToAsTemplate` | `:2835` | (SDK) | Save current modular set as template. |

### Subsystem patterns

- **Theme-based composition.** Unlike LCDV2 (raw JPEG/H.264 bytes
  pushed straight to the device) or LWireless (per-frame image via
  pipe), Lancool 207 Digital uses a **structured `Theme` object
  model**: a template defines the layout, modulars are widgets
  bound to sensor-data variables, and theme-info carries
  per-instance styling (color, font). The SDK composes the final
  framebuffer.
- **Sensor data flows through host-side `sensorValues` dict.** The
  controller maintains a `Dictionary<string, double> sensorValues`
  (`:99`) that pulls from HWiNFO and other sources, then injects
  values into modular widgets at render time via the SDK's
  `dataNamePlaceholderLookup` mechanism (`:39`).
- **Cross-process shared-memory image transport** for top-layer
  overlays and theme-info images. `processId` + `address` →
  `ReadProcessMemory`-equivalent. Linux-port-blocking pattern.
- **SDK init lifecycle.** `initSDK(initSettings, ...)`
  (`:3805-3820`) wraps `LCDController.Init(initSettings, null,
  null)` (`:3840`) and `LCDController.Cleanup()` (`:3860`) with
  retry logic (`SDKInitializationMaxRetries = 2`, `:47`). Init
  happens lazily on first device discovery.
- **Firmware-update gate.** `isFirmwareUpdating` blocks all
  request handling except `CheckFirmwareUpdateStatus`
  (`:2562-2565, 2558-2561`). Standard "device unavailable during
  flash" pattern.
- **Display vs control mode endpoints.** Two ControllerTypeIDs
  (Lancool207Digital and Lancool207DigitalDisplayMode) on
  different VID/PID. The DisplayMode endpoint runs `isDisplayMode = true`
  (`:103`) which **disables the firmware-version checker**
  (`:4517-4519` — `if (this.isDisplayMode || ...)`). DisplayMode is
  presumably a passive secondary-display path that doesn't accept
  control commands.

### Notable divergences from LCDV2

- **Opaque transport.** Like LWireless, Lancool 207 Digital's
  wire-level framing is hidden inside `lcd207.dll`. LCDV2 has
  fully documented A/B-class HID framing.
- **Theme model instead of frame model.** LCDV2 takes raw JPEG /
  H.264 / per-LED RGB. Lancool 207 Digital takes a structured
  template + modular widgets + theme-info, and the **SDK does the
  rendering**. This is a higher-level interface — the device
  handles its own framebuffer composition (or the SDK does).
- **Cross-process memory IPC.** LCDV2 has nothing equivalent.
- **Two endpoints per device** (control + display-mode) on
  different USB silicon. LCDV2 is one endpoint.
- **Resolution is much higher.** 1472×720 vs LCDV2's 480×480.
- **No fan/pump/lighting subsystem.** Pure LCD device. Case fans
  are managed via the motherboard, case lighting via separate
  controllers.
- **No reset command.** LCDV2 has cmd 142; Lancool 207 Digital's
  `ResetProfile` only resets host-side state.
- **Firmware-update is in-band.** LCDV2 declares cmds 10/11
  (`ActiveFirmware`, `UploadFirmware`) but they're
  unimplemented in the C# tree; Lancool 207 Digital has
  `StartFirmwareUpdate` + `CheckFirmwareUpdateStatus` actively
  wired with progress reporting (`Lancool207DigitalFirmwareUpdateStatus.cs`).

### Open questions / unclear from static analysis

- **The full wire-level framing.** Without a USB capture or
  `lcd207.dll` reverse engineering, the byte-level packet shape
  of the control endpoint (`0x1CBE:0xA065`) and the
  display-mode endpoint (`0x1A86:0xAD11`) is unknown.
- **`copyImageMemory` implementation.** Confirmed to be a
  cross-process memory read, but the exact API surface (Windows
  `ReadProcessMemory`? a P/Invoke wrapper around something else?
  Authentication / handle inheritance?) is not visible. Linux
  port needs a different IPC path.
- **DisplayMode endpoint protocol.** The `0x1A86:0xAD11`
  (DisplayMode) endpoint is on a different USB bridge silicon
  (WCH `0x1A86`) than the control endpoint (`0x1CBE:0xA065`).
  Whether the device exposes both interfaces simultaneously, or
  one at a time depending on a mode switch, is unclear.
- **Theme / template / modular wire format.** The composed `Theme`
  is rendered by the SDK before transmission. Whether the
  rendered frame is sent as raw RGBA, encoded JPEG, or some
  proprietary tile/sprite list is invisible.
- **Sensor data binding.** `dataNamePlaceholderLookup`
  (`:39, :3306-3375`) maps placeholder strings to sensor values,
  but the full set of accepted placeholder names is not
  enumerated in this file — likely hard-coded inside `lcd207.dll`.
- **Fan source settings semantics.** The `Lancool207DigitalFanSourceSetting`
  selects a sensor source for a displayed fan readout, but
  whether it also *controls* the case fan (via the
  motherboard's PWM passthrough, EC, or some other pathway) or
  is purely cosmetic is unclear from the LCD-controller code
  alone.
- **`StartFirmwareUpdate` payload.** The firmware blob format,
  size, and whether it's encrypted/signed is hidden inside the
  SDK call. Flag as **destructive** for any future Linux
  implementation.
- **`SetSecondaryScreen` semantics.** Toggles between
  `LCDController.SetDesktopMode()` and
  `LCDController.SetMonitorMode()` (`:4485`). Whether this
  affects host-side window behavior, device-side input routing,
  or USB configuration is unclear.
- **The relationship between `ResetProfile` (host-side reset)
  and any device-side reset.** No SDK call surfaced.
