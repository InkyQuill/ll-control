# SLV3LCD - Cross-Device Survey

## Family

SLV3LCD - the per-fan LCD screen module mounted on Lian-Li **SL Wireless
LCD** fans. The "SLV3" prefix names the **SL Wireless** protocol family
(the same family the `slv3.dll` SDK implements - see `slv3-sdk.md`); the
"LCD" suffix distinguishes the LCD-screen-mounted SKU from the sibling
RGB-only SL Wireless fan (`L-Connect.Core/LConnectCore/Products/LWireless/SLV3/`).
The product name is corroborated by the firmware-version regex
`^slv3_\d{8}(\d{2})(?:_\d{2})?$`
(`L-Connect.Core/LConnectCore/Products/LWireless/SLV3LCD/SLV3LCDFirmwareVersion.cs:46`).

This family is **architecturally divergent** from every other LCD family
documented so far: the C# layer here builds **JSON over the L-Connect
service IPC**, not HID packets. All wire-level protocol work happens
inside the L-Connect-Service process via the proprietary native
`slv3.dll` SDK (namespaces `slv3` / `slv3.models`, classes `RFController`
and `STRController`). There is no `XCommandPacket` class for SLV3LCD,
no `LCDPacket`, and no direct `IHidDevice.Write` from any C# code we
can read. See "Notable divergences" below for the implications.

**Status legend:** verified-live | inferred-static | unclear. All claims
in this file are **inferred-static** unless otherwise marked - Phase 0e
is a static-source survey. SLV3LCD live runs are blocked at the C#
layer because the wire protocol lives inside `slv3.dll` (closed source).

## Source folder

Two related folders cooperate:

- `L-Connect.Core/LConnectCore/Products/SLV3LCD/` -
  upper-layer DTOs (settings, request envelopes, media converter).
  Files surveyed:
  - `RequestType.cs` (the cmd enum - **but values are dispatch keys for
    the service IPC, not wire bytes**)
  - `Constants.cs`
  - `LWirelessLCDGroupType.cs`, `LWirelessLCDRequestData_T_.cs`
  - `LWirelessLCDScreenSetting.cs`, `LWirelessLCDSensorSetting.cs`,
    `LWirelessLCDSensorSettings.cs`
  - `LWirelessLCDMediaConverter.cs`
  - `ScreenRotation.cs`
- `L-Connect.Core/LConnectCore/Products/LWireless/SLV3LCD/` -
  family-specific firmware-version parsing only:
  - `SLV3LCDFirmwareVersion.cs`

Service-side (where the actual SDK calls happen):

- `L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs` -
  20-way dispatch table that maps `RequestType` -> SDK calls.
- `L-Connect-Service/LConnectService/Products/SLV3LCD/TimerManager.cs` -
  per-key timer registry used by the sensor refresh loop.

Client-side controller wrapper (for completeness):

- `L-Connect3/LConnect3cs/Products/LWireless/SLV3LCDController.cs` -
  thin client; serialises `LWirelessLCDRequestData<T>` into JSON and
  hands it to `Controller.sendControllerRequest` (see "Framing" below).

Wired-only firmware-update plumbing (shared with TLV2LCD over the same
LWireless pipeline):

- `L-Connect.Core/LConnectCore/LWirelessRequestType.cs:7-11`
  (`GetLCDFirmwareVersion`, `StartSLV3LCDUpgrade`,
  `GetSLV3LCDUpgradeProgress`, `StartTLV2LCDUpgrade`,
  `GetTLV2LCDUpgradeProgress`)
- `L-Connect.Core/LConnectCore/Products/LWireless/WirelessLCDUpgradeRequest.cs`
- `L-Connect.Core/LConnectCore/Products/LWireless/WirelessLCDUpgradeResponse.cs`
- `L-Connect.Core/LConnectCore/Products/LWireless/WirelessLCDUpgradeProgressResponse.cs`

## Marketed name

**Lian-Li SL Wireless LCD fan** (the screen-equipped SKU in the
SL Wireless fan family; "SLV3" = SL Wireless LCD, the SDK is `slv3.dll`
which implements the SL Wireless protocol - not "SL Version 3"). The
fan ships in 120 mm and 140 mm variants,
with per-variant RPM ranges declared at
`L-Connect.Core/LConnectCore/Products/LWireless/Constants.cs:35-41`:

| Variant            | Min RPM | Max RPM |
|--------------------|--------:|--------:|
| SLV3LCD 120 mm     |    300  |   1900  |
| SLV3LCD 140 mm     |    400  |   1600  |

The 400x400 LCD module sits on the fan hub (one screen per fan).
`Constants.cs:7-19` (the SLV3LCD-namespaced one) declares
`ScreenWidth = 400`, `ScreenHeight = 400`, `MaxScreenFramePerSecond = 30`,
`PortCount = 4`, `MaxLCDFanCount = 3` - same per-fan-screen ceiling
shape as TLLCD but with a higher 30 FPS cap (TLLCD is also 30 default,
but TLV2LCD/HS-II differ).

## ControllerTypeID / ControllerClass

- `ControllerTypeID.SLV3LCD = 16977920` -
  `L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:29`
  (hex `0x01032000`)
- `ControllerClass.SLV3LCD = 49` -
  `L-Connect.Core/LConnectCore/Devices/ControllerClass.cs:20`
- `ControllerTypeIDDefinitions.ClassSLV3LCD = 200704` -
  `L-Connect.Core/LConnectCore/Devices/ControllerTypeIDDefinitions.cs:53`
  (hex `0x031000`)

## VID:PID

| ControllerTypeID | VID (decimal) | PID (decimal) | VID:PID (hex)   |
|------------------|---------------|---------------|-----------------|
| `SLV3LCD`        | 7358          | 5             | `0x1CBE:0x0005` |

Source: `L-Connect.Core/LConnectCore/Devices/KnownDevices.cs:61`.

VID `0x1CBE` is **Lian-Li's own assigned USB-IF VID** (distinct from the
Sunplus VID `0x04FC` used by TLLCD / TLFan, and the Winbond VID `0x0416`
used by Galahad II / HydroShift). Other devices on the same Lian-Li VID
include the wireless-bridge SKUs and the second-generation LCD fans:

| Device                | PID (dec) | PID (hex) | KnownDevices line |
|-----------------------|----------:|-----------|-------------------|
| `SLV3LCD`             | 5         | `0x0005`  | `:61` |
| `TLV2LCD`             | 6         | `0x0006`  | `:63` |
| `HydroShiftIILCDCircle`| 40993    | `0xA021`  | `:65` |
| `HydroShiftIILCDSquare`| 41012    | `0xA034`  | `:69` |
| `Lancool207Digital`   | 41061     | `0xA065`  | `:73` |
| `UniversalScreen8p8Inch`| 41096   | `0xA088`  | `:77` |

(SLV3LCD is the **lowest PID** on the Lian-Li VID, suggesting it shipped
first in the Lian-Li-branded USB enumeration era.)

The `0x1CBE` VID with the small linear PID suggests this is an
**embedded MCU exposed by a Lian-Li wireless transmitter**, not a
direct fan-MCU enumeration. See "Notable divergences" for the wired vs
wireless plumbing.

## Framing classes

**SLV3LCD has no HID-level framing class accessible from C# source.**

Instead, the framing is two-layered:

### Layer 1 (C#-visible): JSON-over-IPC envelope

The L-Connect 3 GUI builds requests in
`L-Connect3/LConnect3cs/Products/LWireless/SLV3LCDController.cs:24-228`
using `base.sendControllerRequest((RequestType)N, payload, 2000)`.
The base implementation at
`L-Connect3/LConnect3cs/Products/Controller.cs:156-174` does:

1. Build a query string with `devicePath` (Base64 of HID device path)
   and `type` (the `RequestType` enum **name** as ASCII, e.g.
   `"SetPicture"`).
2. JSON-serialise the body object (typically
   `LWirelessLCDRequestData<T>` with fields `Data`, `LcdIndex`, `Position`).
3. Call `LConnectServiceHttpUtility.TrySendRequest(1, ..., timeout=2000)`,
   which routes the request to the L-Connect-Service Windows service
   over its HTTP-style IPC.
4. Receive a string response (JSON), deserialise via `JsonSerializer`.

**The "command byte" surface here is the `RequestType` enum name**, not
a wire byte. There is no `ReportID`, no `Length` field, no chunker -
the body is an arbitrary JSON document.

### Layer 2 (inside L-Connect-Service): proprietary RF / HID via slv3.dll

`L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs:454-577`
dispatches each `RequestType` to a `handle*` method, and those methods
delegate exclusively to closed-source SDK classes from `slv3.dll`:

- `RFController` (LWireless RF transmitter) -
  `Bind`, `GetAllGroup`, `GetAllMasterController`, `GetVersionInfo`,
  `SetFansRPM`, `SetAioPumpSpeed`, `RebootLcdGroup`, `Init`, etc.
  (see `L-Connect.Core/LConnectCore/Products/LWireless/LWirelessDevice.cs:46-197`)
- `STRController` (per-screen RGB / image / sensor renderer) -
  `GetAllControlDevice`, `ShowAndSaveImg`, `PlayVGroupMedia`,
  `RestLcdHub`, `set_StyleType`, `set_iRotate`, `set_MediaPath`,
  `RefreshSTRPosInfo`
  (used heavily throughout the service-side `SLV3LCDController.cs`)

Both classes are in the `slv3` and `slv3.models` namespaces, supplied by
the proprietary `slv3.dll` (decompilation gives no further source). The
HID/RF wire format underneath is **opaque to this survey**.

### Implication for the Linux port

There is no equivalent of "build a 64-byte HID packet and send it" at
the C# layer. To support SLV3LCD on Linux, Phase 0e/1 will need a
Wireshark/usbmon capture against a live device (the windows service
talking to the device through `slv3.dll`) to reverse the actual wire
format on the `0x1CBE:0x0005` interface. **No static-source path
yields the wire bytes.**

## Subsystem fingerprint

| Subsystem                    | Present? | Evidence                                               |
|------------------------------|----------|--------------------------------------------------------|
| Fan PWM                      | yes (indirect) | `LWirelessDevice.SetFanSpeed` -> `RFController.SetFansRPM` (`L-Connect.Core/.../LWireless/LWirelessDevice.cs:161-170`); per-fan-size limits at `LWireless/Constants.cs:35-41` |
| Fan RPM read                 | yes      | `SLV3LCDController.handleGetRPM` (`SLV3LCDController.cs:305-322`) -> `RFController.GetAllGroup().Speeds` |
| Fan-ring lighting            | yes (separate enum) | Sibling family `LConnectCore/Products/LWireless/SLV3/` (`LWirelessLightingConfig.cs`, `LWirelessUtilityLightEffectsSetting.cs`) - **not in the SLV3LCD enum scope; LWireless task surveys this** |
| **Per-fan LCD screen**       | yes      | Whole `RequestType.cs` enum is screen-centric; `Constants.cs:11-19` 400x400 px / 30 FPS / 4 ports x 3 fans |
| LCD on/off                   | unclear  | No explicit "off" cmd; `RestartLCD` (cmd 16) covers reboot only (`SLV3LCDController.cs:579-614`) |
| LCD brightness               | yes      | cmd 8 `SetBrightness` (`SLV3LCDController.cs:752-802`) - **discrete LUT**, see "Subsystem patterns" |
| LCD rotation (per-screen)    | yes      | cmd 7 `SetRotate` (`SLV3LCDController.cs:930` dispatch); 4-value enum `Rotate0/90/180/270` (`ScreenRotation.cs:5-11`) |
| LCD rotation (all screens)   | yes      | cmd 9 `SetRotateAllScreen` (`SLV3LCDController.cs:1015`) |
| LCD image upload             | yes (via SDK file path) | cmd 5 `SetPicture` (`SLV3LCDController.cs:867`); cmd 18 `PictureSyncToAll` (`:324-390`) - both pass `FilePath` to `STRController.ShowAndSaveImg` |
| LCD video / GIF playback     | yes      | cmd 6 `SetVideo`, cmd 19 `VideoSyncToAll`, cmd 12 `PlayVirtualGroupVideo` (`SLV3LCDController.cs:392-452`) |
| Sensor display (LCD-side)    | yes      | cmd 3 `SetSensor`, cmd 20 `SensorSyncToAll`, with style enums `Style0..Style8` (`LWirelessLCDSensorStyle.cs`); 1000 ms refresh timer (`SLV3LCDController.cs:42` `SensorTimerInterval = 1000`) |
| Default (logo) image         | yes      | cmd 4 `SetDefaultPic` (`SLV3LCDController.cs:804-865`) |
| Virtual group / multi-screen | yes      | cmd 10 `SetVirtualGroup`, cmd 11 `UngroupVirtualGroup`, cmd 12 `PlayVirtualGroupVideo` (`SLV3LCDController.cs:1163-...`); group layouts at `LWirelessLCDGroupType.cs:5-14` |
| Group enumeration / discovery| yes      | cmd 0 `RefreshGroupInfo`, cmd 1 `GetGroupCount`, cmd 2 `GetControllers`, cmd 22 `SelectedController` (gap, see below) |
| File deletion                | yes      | cmd 13 `DeleteFile` (`SLV3LCDController.cs:178-233`) - upper-layer file path management; clears settings on stop |
| Restart / reset              | yes      | cmd 16 `RestartLCD` -> `RFController.RebootLcdGroup` + `STRController.RestLcdHub` (`SLV3LCDController.cs:579-614`) |
| Firmware version read        | yes      | `LWirelessRequestType.GetLCDFirmwareVersion = 0` (`LConnectCore/LWirelessRequestType.cs:7`) - distinct dispatch enum, served by the LWireless controller, not SLV3LCD's own enum |
| Firmware upgrade             | yes      | `LWirelessRequestType.StartSLV3LCDUpgrade = 1`, `GetSLV3LCDUpgradeProgress = 2` (`LWirelessRequestType.cs:8-9`); request/response shapes at `WirelessLCDUpgradeRequest.cs:6-29`, `WirelessLCDUpgradeResponse.cs:6-29`, `WirelessLCDUpgradeProgressResponse.cs:6-47` |
| Continuous re-issue timer    | yes (sensor only) | `TimerManager.cs` per-key timer registry; sensor refresh at 1 Hz (`SensorTimerInterval`) |

## Commands

The "commands" surveyed here are **service-IPC dispatch keys**, not HID
wire bytes. The wire bytes underneath live in `slv3.dll` and are not
visible to this static survey.

All values from
`L-Connect.Core/LConnectCore/Products/SLV3LCD/RequestType.cs:5-28`.
Handler line numbers from
`L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs`
(`HandleRequestData` switch at `:454-577`).

| Cmd | Name                    | Service handler         | Body type (JSON) | Status          | Notes |
|----:|-------------------------|-------------------------|------------------|:---------------:|-------|
| 0   | `RefreshGroupInfo`      | `handlerRefreshGroupInfo` | (none / `int`?) | inferred-static | Calls `STRController.RefreshSTRPosInfo(0)` (`SLV3LCDController.cs:616-626`). |
| 1   | `GetGroupCount`         | `handleGetGroupCount`     | `int` (group idx) | inferred-static | Returns count of `STRController` entries with matching `GroupIndex` (`:265-303`). |
| 2   | `GetControllers`        | `handleGetControllers`    | `int` (group idx) | inferred-static | Returns `List<STRController>` filtered by `GroupIndex` (`:235-263`). |
| 3   | `SetSensor`             | `handlerSetLCDSensor`     | `LWirelessLCDRequestData<LWirelessLCDSensorSettings>` | inferred-static | Per-screen sensor display config (style/colours/type). |
| 4   | `SetDefaultPic`         | `handleSetDefaultPic`     | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | "Logo" / fallback image; sets `StyleType` from `data.Sensor.Style` then `STRController.ShowAndSaveImg` (`:804-865`). |
| 5   | `SetPicture`            | `handleSetPicture`        | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Single-position picture; `StyleType=6`. |
| 6   | `SetVideo`              | `handleSetVideo`          | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Single-position video; `STRController.PlayVGroupMedia` for group, `ShowVid` (or similar SDK method) for single. |
| 7   | `SetRotate`             | `handleSetRotate`         | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Per-screen rotation; one position. |
| 8   | `SetBrightness`         | `handleSetBrightness`     | `LWirelessLCDRequestData<int>` (0..100) | inferred-static | **Discrete LUT** of 5 client values -> 5 hardware levels. See "Subsystem patterns". |
| 9   | `SetRotateAllScreen`    | `handleSetRotateAllScreen`| `LWirelessLCDRequestData<List<LWirelessLCDScreenSetting>>` | inferred-static | Each `LWirelessLCDScreenSetting.Index` selects one screen in a virtual group; rotation is "inverted" by `invertIndex(PosIndex, controllers.Count)` (`SLV3LCDController.cs:1015-...`). |
| 10  | `SetVirtualGroup`       | `handleSetVirtualGroup`   | `LWirelessLCDRequestData<int>` | inferred-static | Returns int (the new group ID). Combines screens at the named positions into one virtual group for synced video. |
| 11  | `UngroupVirtualGroup`   | `handleUngroupVirtualGroup` | `int` (group idx) | inferred-static | Splits a virtual group back to per-screen mode. |
| 12  | `PlayVirtualGroupVideo` | `handlePlayVirtualGroupVideo` | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Plays one video across the virtual group; uses `STRController.PlayVGroupMedia(vGroupIndex, FilePath, true)` (`:392-452`). |
| 13  | `DeleteFile`            | `handleDeleteFile`        | `string` (file path) | inferred-static | Stops any screen showing that path; `StyleType=9`; clears single+group setting maps (`:178-233`). |
| 14  | `SyncToAllLCD`          | **(no-op)**               | -                | unclear         | `case 14: case 15: { all = null; break; }` (`SLV3LCDController.cs:538-543`). **Enum entry exists with no service-side handler.** Possibly delegated client-side (broadcast loop over per-screen cmds) or vestigial. |
| 15  | `GetSelectedController` | **(no-op)**               | -                | unclear         | Same `case 14: case 15:` no-op. May be a UI-state stub that never reached the service. |
| 16  | `RestartLCD`            | `handleRestartLCD`        | `int` (group idx) | inferred-static | `RFController.RebootLcdGroup(macStr)` then `Thread.Sleep(1000)` then `STRController.RestLcdHub(num)` (`:579-614`). **Destructive (interrupts active media).** |
| 17  | `GetRPM`                | `handleGetRPM`            | `int` (group idx) | inferred-static | Returns `List<int>` of RPMs for all `RfDevice` entries with matching `LcdGroup` (`:305-322`). |
| 18  | `PictureSyncToAll`      | `handlePictureSyncToAll`  | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Sets the same picture on every screen in the LCD group (`:324-390`). `StyleType=6`. |
| 19  | `VideoSyncToAll`        | `handleVideoSyncToAll`    | `LWirelessLCDRequestData<LWirelessLCDScreenSetting>` | inferred-static | Same as 18 but video (`:1257-...`). |
| 20  | `SensorSyncToAll`       | `handlerSensorSyncToAllLCD` | `LWirelessLCDRequestData<List<LWirelessLCDScreenSetting>>` | inferred-static | Per-screen sensor config applied to every screen in the LCD group (`:628-...`). Note the **list** form: each list element targets one position. |

### Adjacent firmware-update enum (separate dispatch)

These are NOT in `Products/SLV3LCD/RequestType.cs`; they live on the
LWireless service-controller dispatch (`LConnectCore/LWirelessRequestType.cs:5-12`):

| Cmd | Name                          | Body type (JSON)                | Notes |
|----:|-------------------------------|---------------------------------|-------|
| 0   | `GetLCDFirmwareVersion`       | -                               | Returns version string parsed by `SLV3LCDFirmwareVersion.TryParse` (`SLV3LCDFirmwareVersion.cs:49-77`) - regex-validated `slv3_<8 digits><2 digits>(_<2 digits>)?`. |
| 1   | `StartSLV3LCDUpgrade`         | `WirelessLCDUpgradeRequest` (`FilePath`, `IsForce`, `Version`) | Initiates firmware push to all SLV3LCD fans in the group; returns `WirelessLCDUpgradeResponse` (`Success`, `GroupCount`, `LCDCount`). |
| 2   | `GetSLV3LCDUpgradeProgress`   | -                               | Returns `WirelessLCDUpgradeProgressResponse` with `IsRunning`/`IsFinished`/`GroupCount`/`LCDCount`/`TotalGroupCount`/`TotalLCDCount`. |
| 3   | `StartTLV2LCDUpgrade`         | -                               | Sibling for TLV2LCD. |
| 4   | `GetTLV2LCDUpgradeProgress`   | -                               | Sibling for TLV2LCD. |

(The actual firmware blob bytes go through `slv3.dll` - no static
visibility of the wire chunker.)

## Subsystem patterns

### IPC envelope (the only "framing" we can see)

`Controller.sendControllerRequest` (`L-Connect3/LConnect3cs/Products/Controller.cs:156-174`):

```
GET-style query string parameters:
  devicePath = Base64(UTF8(HID device path))
  type       = RequestType.ToString()    # ASCII enum name
Body: JSON serialised payload (LWirelessLCDRequestData<T>, primitives, ...)
Timeout: 2000 ms (per call)
Response: JSON string, deserialised by tryGetResponseData<T>
```

The request rides on `LConnectServiceHttpUtility.TrySendRequest(channel=1, ...)`
- channel 1 is the "controllers" pipe in the L-Connect-Service IPC. The
"1" magic is unmodelled in source.

### Discrete brightness LUT

`SLV3LCDController.handleSetBrightness` (`SLV3LCDController.cs:752-802`)
maps the **client-side 0..100 input** to a **5-step hardware brightness**:

| Client value | Hardware level |
|-------------:|---------------:|
| 0            | 0              |
| 25           | 10             |
| 50           | 30             |
| 75           | 40             |
| 100          | 100            |
| any other    | 30 (default)   |

This is a strong divergence from LCDV2 / TLLCD, which use linear 0..100
brightness. Any value outside the 5 canonical points falls through to
30. Linux-port implication: do NOT expose a continuous slider at the
daemon API for SLV3LCD; expose 5 discrete levels (or pre-quantise).

### Sensor refresh timer

`SLV3LCDController.cs:42-44`:
- `SensorTimerInterval = 1000` (ms) - one-Hz refresh
- `SetLCDValueDelay = 200` (ms) - debounce on screen updates

`TimerManager` (`L-Connect-Service/.../SLV3LCD/TimerManager.cs:11-...`)
holds a `ConcurrentDictionary<string, Timer>` keyed by screen SN; the
sensor timer ticks once per second and pushes a freshly rendered sensor
JPEG into `STRController.ShowAndSaveImg`. The host renders the sensor
overlay (CPU/GPU temp/load) client-side and pushes a JPEG via the SDK -
just like TLLCD's pattern of host-side composited sensor frames.

The five rendered sensor types (`SLV3LCDController.sensorTypeAutoList`
at `:73-77`): `CPUTemp(1)`, `GPUTemp(4)`, `CPULoad(0)`, `GPULoad(3)`.
A fifth slot at index 6 (`SensorTimerInterval`-mapped to `() => 500`)
is a **placeholder that always returns 500** - likely a coolant-temp
slot the SDK reserves but the client never wires up. Inferred-static.

### Virtual group flow

`SetVirtualGroup` (cmd 10) accepts a list of `(LcdIndex, Position)`
tuples and merges those screens into a synced display group. The
returned `int` is the assigned `vGroupIndex`. Subsequent
`PlayVirtualGroupVideo` (cmd 12) targets the group by `vGroupIndex`
not by individual screen. `UngroupVirtualGroup` (cmd 11) reverses.
This is **upper-layer state in the L-Connect-Service**; the SDK
exposes it via `STRController.PlayVGroupMedia` and the `vGroupIndex`
field on `STRController`.

The available group geometries are enumerated at
`L-Connect.Core/LConnectCore/Products/SLV3LCD/LWirelessLCDGroupType.cs:5-14`:
`Group1, Group11, Group2, Group111, Group21, Group12, Group3` - same
seven layouts as TLLCD (1, 1+1, 2, 1+1+1, 2+1, 1+2, 3).

### Image pipeline

`LWirelessLCDMediaConverter.SaveImage` (`LWirelessLCDMediaConverter.cs:281-347`):

- Opens source as `Bitmap`, resizes to a `(400, 400 * screenCount)` target
  (or the rotated equivalent for horizontal layouts - swap W/H), draws
  with high-quality interpolation (mode `7` = HighQualityBicubic), and
  writes a JPEG into `LWirelessUploadedDirectory`.
- Filename is `<GUID>.jpg`.
- Per-screen-count subdirs: `image/<screenCount>/{h,v}/`.

`SaveMP4` and `SaveGif` are **byte copies** (no transcode) into
`video/<screenCount>/{h,v}/<GUID>.mp4` and
`gif/<screenCount>/{h,v}/<GUID>.gif`.

`TargetSizeInBytes = 65535` constant at `:17` is **declared but
unreferenced** in this file - likely a stale reference to a previous
chunked-upload size that's now handled inside `slv3.dll`.

### Upload directories

`LConnectApp.Path.LWirelessUploadedDirectory` (user-uploaded media) and
`LConnectApp.Path.AssetsSLV3Directory` (shipped Lian-Li assets - copyright)
are the two roots scanned by `getMediaList` (`LWirelessLCDMediaConverter.cs:163-169`).
`MediaDirectoryType.LWireless` is the only value used here; the
`TLLCDUploadedDirectory` branch (`:183`) is dead code in the SLV3LCD
context.

### Sensor settings shape

`LWirelessLCDSensorSetting` (`LWirelessLCDSensorSetting.cs:7-69`) carries:

| Field             | Type / source           | Notes |
|-------------------|-------------------------|-------|
| `Type`            | `SenSorTypes` (default 1 = CPUTemp) | Which metric to render |
| `Style`           | `StyleTypes` (default 9) | Style 9 = "blank" - the file-deletion fallback |
| `Sensor6Colors`   | `SensorColors` (default 2) | Background colour palette idx |
| `Sensor7Colors`   | `SensorColors` (default 0) |  |
| `BackgroundColor1/2` | `int[]` (RGB triple)   | Style-specific background colours |
| `TextColor1/2/3`  | `int[]` (RGB triples)   | Three text colours per style |

`SenSorTypes` and `StyleTypes` are `slv3.models` enums (closed source);
inferred mapping from usage: `Type` 0=CPULoad, 1=CPUTemp, 3=GPULoad,
4=GPUTemp, 6=??? (the always-500 slot).

### `LWirelessLCDScreenSetting` envelope

`LWirelessLCDScreenSetting.cs:9-112` is the main payload shape:

| Field                  | Type                              | Notes |
|------------------------|-----------------------------------|-------|
| `MediaType`            | `TLLCDMediaType` (default `None`) | **shared with TLLCD** - 0=None, 1=Image, 2=Dynamic, 3=Video, 4=Sensor, 5=Animation |
| `FilePath`             | `string`                          | Absolute path on the host |
| `FileMD5`              | `string`                          | Computed via `string.GetMD5Hash` extension (used to dedupe uploads) |
| `Horizontal`           | `bool`                            | Layout orientation |
| `Index`                | `int`                             | Screen index within the (virtual) group |
| `TotalBlock` / `Block` | `int` (default 1 / 1)             | Sub-block addressing for split media (e.g. "image #1 of 3") |
| `Rotate`               | `ScreenRotation` (0/90/180/270)   | |
| `Sensor`               | `LWirelessLCDSensorSetting`       | Inline (not by ref) |
| `SensorBackgroundPath` | `Dictionary<StyleTypes,string>`   | Per-style background image paths; init populated for styles 0,1,2,3,4,7,8 |
| `SensorLoopTimeInterval`| `int` (default 5)                | Seconds between auto-cycle in sensor display |
| `Sync`                 | `int?`                            | Optional virtual-group sync target |
| `Info`                 | `slv3.models.StyleInfo`           | SDK style descriptor |
| `NamedPipeName`        | `string`                          | When a named-pipe image source is in use (live host-rendered overlay) |

The named-pipe path is interesting: `PipeImageSender`
(`L-Connect.Core/LConnectCore/Products/LWireless/PipeImageSender.cs`)
streams host-rendered frames over a Windows named pipe to the
L-Connect-Service, which then hands them to `STRController` for display.
This is the only "live frame source" plumbing visible at the C# layer.

## Notable divergences vs LCDV2 / TLLCD baselines

1. **No HID-level wire format visible from C# source.** All other LCD
   families (LCDV2, HS-I, HS-II, TLLCD) expose explicit HID packet
   classes (`ACommandPacket` / `BCommandPacket` / `CCommandPacket` /
   `LCDPacket`) in `LConnectCore/Products/<family>/`. SLV3LCD exposes
   only DTOs and a JSON-IPC dispatch enum. The wire layer lives inside
   the proprietary `slv3.dll` SDK. **This is the largest single
   divergence in the family survey so far.**

2. **Indirected via Lian-Li RF SDK.** The native classes `RFController`
   (group-level / fan RPM / firmware) and `STRController` (per-screen
   media / rotation / brightness) are the only entry points. They are
   used by both the wired SLV3LCD and the wireless variants the same
   way - the SDK abstracts wired-vs-wireless underneath. There is no
   separate "wired" code path in C#.

3. **Brightness is a 5-point LUT, not linear.** Unique among the LCD
   families. (LCDV2: 0..100 linear; TLLCD: 0..100 linear via
   `LCDControlSetting`.)

4. **Per-fan addressing uses `(GroupIndex, PosIndex)`, not `(Port, Index)`.**
   The screen identity primitive is `STRController` SDK objects
   keyed by `SN` (32-byte serial), `GroupIndex` (the LCD-group), and
   `PosIndex` (position within the group). Compare TLLCD's
   `(Port=0..3, Index=0..2)` shape - SLV3LCD's `GroupIndex` is closer
   to a logical group than a physical port, since the wireless layer
   blurs port-vs-group.

5. **No "transient vs stored" command pair visible.** All image/video
   commands take a `FilePath` and call `STRController.ShowAndSaveImg`
   or `PlayVGroupMedia`. There is **no analogue of LCDV2 cmd 14 vs
   cmd 15** at this layer, and no analogue of TLLCD's cmd 65
   `WriteJPG` vs cmd 72 `WriteBootJPG`. If SLV3LCD has a stored-slot
   system, it lives entirely inside `slv3.dll` (the `ShowAndSaveImg`
   method name suggests **always-store-then-show**, but this is
   speculative). See cmd-15 hypothesis section below.

6. **Two enum entries with no handler.** Cmd 14 (`SyncToAllLCD`) and
   cmd 15 (`GetSelectedController`) hit `case 14: case 15: { all = null; break; }`
   (`SLV3LCDController.cs:538-543`). Either the UI does the work
   client-side (looping cmd 5/6/3 over each screen) or these are
   vestigial enum slots.

7. **Sensor refresh runs at 1 Hz on the host.** TLLCD has no static
   evidence of a refresh cadence; LCDV2 has a 15-second auto-revert
   on cmd 14. SLV3LCD's `SensorTimerInterval = 1000` ms is the
   tightest sensor-refresh cadence in the family.

8. **Restart / reboot is destructive across the whole LCD group.**
   `handleRestartLCD` reboots **all** screens in the group via
   `RFController.RebootLcdGroup(macStr)` - there is no per-screen
   reboot. Mark as Phase-1-only on the Linux port.

9. **Firmware update is two-stage with an explicit progress poll.**
   `LWirelessRequestType.StartSLV3LCDUpgrade` is fire-and-forget;
   the GUI polls `GetSLV3LCDUpgradeProgress` for status. This is
   distinct from LCDV2 / HS-I where firmware is a single chunked
   write. (The chunking still exists - it's just hidden inside
   `slv3.dll`.)

### Cmd-15 hypothesis cross-check (Send* / Upload* / WriteBoot* pattern)

Phase 0d found that LCDV2 cmd 14 (`SendJpeg`) is transient (auto-revert
at 15 s) and cmd 15 (`UploadJpeg`) is ACK'd-but-not-displayed,
hypothesised to be a stored-slot writer. TLLCD provided strong evidence
with explicit `WriteJPG` (cmd 65, transient) vs `WriteBootJPG` (cmd 72,
stored) pairs.

**SLV3LCD provides no direct evidence for the hypothesis at the C#
layer**, because:

- All image/video commands (cmd 4/5/6/12/18/19) take a `FilePath`
  string and delegate to `STRController.ShowAndSaveImg` /
  `PlayVGroupMedia`. There is no transient-vs-stored split at the
  enum level.
- The method name `ShowAndSaveImg` is **strongly suggestive of always-
  save-then-show semantics** (i.e., every image is stored before
  display), which would make the cmd-15 distinction moot at this
  abstraction level.
- The discrete enum slots cmd 14/15 (`SyncToAllLCD` / `GetSelectedController`)
  are unhandled service-side and unrelated to image storage.

**Implication:** if there is a transient-vs-stored split on the SLV3LCD
wire, it's pushed down into `slv3.dll`. The Linux port would discover
it during the live-capture phase (Phase 0e/1), not from this static
survey. The C# layer is too high to see it.

(Coincidentally, the **LWireless firmware-upgrade enum** has a clean
Start/Progress pair - which is structurally similar to a Send/Upload
pair but for a different purpose. It's not the same hypothesis.)

## Wired-vs-wireless relationship

`L-Connect.Core/LConnectCore/Products/SLV3LCD/` (the **upper-layer
DTO folder** - confusingly named) and
`L-Connect.Core/LConnectCore/Products/LWireless/SLV3LCD/` (the
firmware-version-only sibling) cooperate through the **same**
service-side controller (`L-Connect-Service/.../SLV3LCD/SLV3LCDController.cs`).
The "wired" SLV3LCD device path
(VID:PID `0x1CBE:0x0005`, ControllerTypeID 16977920) and a "wireless
transmitter" device path (`ControllerTypeID.WirelessControllerTransmitter`
/ `WirelessControllerTransmitterV2`) both feed into the same
`STRController.GetAllControlDevice` enumeration. The SDK
(`slv3.dll`) abstracts wired-vs-wireless underneath - the C# layer
sees **one logical fleet of LCD screens** indexed by SN and GroupIndex.

The sibling `Products/LWireless/SLV3/` folder
(`LWirelessLightingConfig.cs`, `LWirelessUtilityLightEffectsSetting.cs`)
is the **non-LCD SLV3 fan** family (RGB-only fans without a screen);
it is **not** the wireless variant of SLV3LCD - it's a different SKU.
Detailed survey of those types belongs to the **LWireless** task in
this Phase 0e batch.

A practical caveat for the Linux port: because the wire layer is
opaque, SLV3LCD support cannot be implemented from this survey alone.
Phase 0e/1 will need:
1. usbmon capture of L-Connect-Service traffic on `0x1CBE:0x0005`
   while exercising each `RequestType`.
2. (Possibly) reverse `slv3.dll` directly via Ghidra to recover the
   per-cmd byte layouts.
3. A separate look at the wireless transmitter dongle's HID
   descriptor to see how addressing works when multiple SLV3LCD
   fans are bound to one transmitter.

## Open questions / unclear

1. **Wire byte format for every command.** Static-source visibility
   ends at the `RFController` / `STRController` API boundary. usbmon
   capture required.

2. **Cmd 14 (`SyncToAllLCD`) and cmd 15 (`GetSelectedController`) are
   unhandled service-side.** Possible explanations: (a) UI does this
   client-side via per-screen loops, (b) vestigial enum entries, (c)
   handler stubs deliberately disabled before shipping. No way to tell
   from static source.

3. **`Sync` field on `LWirelessLCDScreenSetting` (nullable int).**
   No call-site sets or reads it in the surveyed source. Likely a
   future-feature placeholder (cross-fan playback sync timestamp?).

4. **`TargetSizeInBytes = 65535`** in `LWirelessLCDMediaConverter.cs:17`
   is declared but unused. Possibly a stale chunk-size from a previous
   wire format that's now hidden in `slv3.dll`.

5. **Sensor type 6 always returns 500.** `SLV3LCDController.cs:132-138`
   maps a `SenSorTypes` index 6 to `() => 500`. No identifying
   comment; could be coolant temp, fan duty, water flow rate, or
   simply a debug stub. Inferred-static cannot resolve.

6. **`SLV3LCDController.handleSetBrightness` only accepts 5 input
   values.** Anything else falls through to 30. The enclosing GUI
   apparently exposes only those 5, but a third-party client sending
   any 0..100 value would silently get 30 for non-canonical inputs.
   Worth flagging in the Linux port API: clamp to canonical points or
   document the LUT.

7. **MD5 dedup semantics for upload.** `LWirelessLCDScreenSetting.FileMD5`
   is set client-side via `filePath.GetMD5Hash()`, but where it's
   consumed (the SDK? the upload code?) is not visible. If the SDK
   skips re-uploads on MD5 hit, that affects daemon caching strategy.

8. **Firmware version regex.** `^slv3_\d{8}(\d{2})(?:_\d{2})?$` -
   the parsing arithmetic adds 9 to the captured group and decomposes
   into `Version(num/10, num%10)` (`SLV3LCDFirmwareVersion.cs:39`).
   Why "+9"? Probably an offset baked into the firmware build script
   - inferred-static can't resolve and the value affects displayed
   version strings.

9. **Per-fan vs per-group command targeting.** Cmds 5/6/7 take
   `Position` lists for per-screen targeting, while cmds 18/19/20
   ("SyncToAll") target the whole group. There is no per-fan address
   mechanism beyond the `STRController.SN` (32-byte serial). How the
   SDK demultiplexes commands to specific fans inside a group is
   inside `slv3.dll`.

10. **Discovery flow.** `GetGroupCount` / `GetControllers` /
    `RefreshGroupInfo` (cmds 0/1/2) all appear to read pre-existing
    `STRController` state. The actual scan/bind sequence is in
    `RFController.Bind` / `GetAllGroup` in the LWireless device class
    - but how a wired SLV3LCD vs a wireless SLV3LCD are bound at
    enumeration time is unclear from the C# layer.
