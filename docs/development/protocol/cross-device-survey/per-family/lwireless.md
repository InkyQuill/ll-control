## Family: LWireless

**Source folder:** `LConnectCore/Products/LWireless/`

**Marketed name(s):** Lian Li Wireless ARGB / fan / pump hub — the
**RF transmitter dongle** that talks over an proprietary 2.4 GHz radio
to a swarm of wireless receivers. The hub itself is a USB HID dongle;
the actual end devices (TLV2 LCD fans, Lancool 217 Infinity case,
SLV3 fans, SLV3 LCD fans, Strimer Plus wireless, HydroShift II LCD
wireless) all hang off the radio bus and are addressed by 6-byte MAC.

**ControllerTypeID enum value(s):** the hub itself appears as one of:
`WirelessControllerTransmitter = 16973824`,
`WirelessControllerReceiver = 16973825`,
`WirelessControllerTransmitterV2 = 16973826`,
`WirelessControllerReceiverV2 = 16973827`
(`L-Connect.Core/LConnectCore/Devices/ControllerTypeID.cs:25-28`).
Child families documented in this writeup carry their own type IDs
(see per-child sections below).

**USB VID:PID:** four hub variants (`KnownDevices.cs:53-59`):

| ControllerTypeID | VID:PID (hex) | VID:PID (dec) | UsageID | Source |
|---|---|---|---|---|
| `WirelessControllerTransmitter` (16973824) | `0x0416:0x8040` | `1046:32832` | (none) | `KnownDevices.cs:53` |
| `WirelessControllerReceiver` (16973825) | `0x0416:0x8041` | `1046:32833` | (none) | `KnownDevices.cs:55` |
| `WirelessControllerTransmitterV2` (16973826) | `0x1A86:0xE304` | `6790:58116` | (none) | `KnownDevices.cs:57` |
| `WirelessControllerReceiverV2` (16973827) | `0x1A86:0xE305` | `6790:58117` | (none) | `KnownDevices.cs:59` |

V1 is on the Winbond/Lian-Li VID (`0x0416`); V2 is on QinHeng / WCH
(`0x1A86`, the CH343/CH9329 USB-bridge maker). This implies the V2
hub is a different USB-to-radio bridge silicon. Same logical RF
protocol, different USB transport on the host side.

**Framing classes used:** **opaque — proprietary `slv3` SDK.** This
is the central finding for LWireless: the device class
(`LWirelessDevice.cs:14-198`) does **not** issue any direct HID
read/write calls. Every operation is delegated to a static
`RFController` class imported from `slv3.dll`
(`LWirelessDevice.cs:5-6` — `using slv3; using slv3.models;`).

```
LWirelessDevice (this codebase)
        |
        v   (delegates to)
RFController (slv3.dll, third-party / proprietary)
        |
        v   (HID ReportID + framing not visible from C#)
USB HID dongle (VID 0x0416 or 0x1A86)
        |
        v   (proprietary 2.4 GHz RF)
Wireless receivers (TLV2 LCD, Lancool 217, SLV3, ...)
```

The wire-level framing of the hub's HID protocol is **invisible from
the L-Connect codebase** — it's encapsulated inside the
`slv3.dll` native binary. From this tree we see only the high-level
SDK API surface: `RFController.SetFansRPM(macStr, list)`,
`RFController.SetAioPumpSpeed(macStr, rpm)`, etc. To document the
actual wire-level framing classes, byte layouts, and command bytes,
**`slv3.dll` would need to be reverse-engineered separately** (DLL
binary lives at `winver/L-Connect 3/slv3.dll`).

`Phase 0d status:` not in scope; transport is opaque.

**Subsystem fingerprint (via SDK API surface):**
- Sensors / handshake: **yes** — `RFController.GetAllGroup()` returns
  a `List<RfDevice>` of bound devices; `RFController.GetAioTemp(macStr)`
  returns water temperature for an AIO pump
  (`LWirelessController.cs:1310, 1336, 1399, 2489, 2512`).
- Fan PWM: **yes** — `RFController.SetFansRPM(macStr, List<byte> duty)`
  takes 4 PWM bytes (one per fan slot, 0..=255 mapped from 0..=100
  percent via `NumberHelper.Map(x, 0, 100, 0, 255)` —
  `LWirelessDevice.cs:112`).
- Pump PWM: **yes** — `RFController.SetAioPumpSpeed(macStr, rpm)` for
  AIO type 10, `RFController.SetH2SAioPumpSpeed(macStr, rpm)` for
  AIO type 11 (`LWirelessDevice.cs:181-192`).
- Fan curve upload: **partial** — `RFController.SetAioParams(aioParams,
  devType)` uploads pump-config parameters
  (`LWirelessDevice.cs:172-179`). The shape of `AioParams` is in
  `slv3.models.AioParams` (proprietary). All speed-driven curves still
  run host-side via `LWirelessController.handleSetFanSpeed` etc.
- Lighting (preset modes): **yes** —
  `RFController.SetLightEffect(macStr, lightEffectSet, null)` for
  per-device lighting; `RFController.SetAllLightEffect(set, devType,
  null)` for type-wide; `RFController.SetUIEffect(portOrderList,
  effectSet, directionList)` for merge mode
  (`LWirelessController.cs:963, 1032, 1280, 2417, 2439`).
- Lighting (per-pixel LED): **inferred** — `lightEffectSet` is a
  `slv3.models.LightEffectSet` carrying mode/color/speed/brightness;
  no obvious raw-LED-stream entry point in the surfaced API.
- LCD on/off / brightness / rotation: **yes** —
  `SetPumpLCDBrightness` (`RequestType` index 32), `SetPumpLCDRotate`
  (33) handled separately (`LWirelessController.cs:1633-1641`).
- LCD frame upload: **passthrough via host-side image pipeline**, not
  via the LWireless device class itself — see "Named-pipe IPC" below.
- Continuous re-issue pattern: **yes** — sensor loops poll
  `GetAioTemp` and re-issue `SetFansRPM` per macStr.
- Reset / soft-reset: **yes** — `ResetDevice` (`RequestType` index 18)
  in the enum; handler is a no-op (case 18 falls through to `case 43`
  default which returns null — `LWirelessController.cs:1525-1537`).
  May be wired to a different SDK call elsewhere.
- Firmware upload / activation: **partial** — there is a
  `WirelessLCDUpgradeRequest.cs`, `WirelessLCDUpgradeResponse.cs`,
  and `WirelessLCDUpgradeProgressResponse.cs` (top-level
  `LWireless/` folder), implying the hub can carry firmware upgrade
  payloads to wireless LCD receivers, but the dispatch goes through
  the `slv3` SDK.
- Crypto / obfuscation at HID transport: **unknown** — opaque inside
  `slv3.dll`. The 2.4 GHz RF link itself is presumably
  encrypted/paired (BindDevice / UnBindDevice / LockBoundDevices
  enum entries imply pairing semantics) but the on-USB framing is
  not visible.

### Named-pipe IPC for image transport

**Pipe name format:** `LConnect_<Guid>` — generated by
`L-Connect3/LConnect3cs/Products/LWireless/SLV3LCD/SLV3UtilityLCDScreenPage.cs:1150-1153`:

```
private string generateNamedPipeName(string prefix = "LConnect")
{
    return string.Format("{0}_{1}", prefix, Guid.NewGuid());
}
```

(an identical helper at `TLV2UtilityLCDScreenPage.cs:1173`). The
generated pipe name is stored in `LWirelessLCDScreenSetting.NamedPipeName`
(`L-Connect.Core/LConnectCore/Products/SLV3LCD/LWirelessLCDScreenSetting.cs:53-57`)
and persists per-screen-config so the UI client and the service
agree on the pipe to use.

**Direction:** the **service** is the pipe **server** (writer); the
**UI client** is the pipe **client** (reader). The UI generates
images for the LCD screen and sends them to the service, which
forwards them via the `slv3` SDK over RF to the wireless LCD
receiver.

Wait — re-reading: `PipeImageSender` opens a `NamedPipeServerStream`
(`PipeImageSender.cs:45`) and writes images to whoever connects.
`PipeClientManager` opens a `NamedPipeClientStream`
(`PipeClientManager.cs:35`) and reads. So the **`PipeImageSender`
side is the producer**. In the service:
`SLV3LCDController.cs:1499` and `TLV2LCDController.cs:1500`
construct `new PipeImageSender(setting.NamedPipeName)`. The actual
image-providing callable is wired in via
`StartSendingImages(Func<byte[]> imageProvider)` and the
service-side composer feeds it.

Direction (corrected): the **service-side LCD controller** runs
the `PipeImageSender` (server, writer); the **UI-side image grid**
(`L-Connect3/LConnect3cs/Views/UserControls/NamedPipeImageGrid.cs:132`)
runs the `PipeClientManager` (client, reader). The service composes
the LCD frame (overlaying sensor values onto a background) and
publishes it to the pipe so the UI can display a live preview.
**The pipe does NOT carry the bytes that go on the radio** — RF
transmission still goes through `slv3.dll`. The pipe is purely a
host-side preview-mirror channel.

**Message framing:** length-prefixed binary records,
little-endian (`BitConverter.GetBytes(int)` is host-endian; on
x86/x64 Windows that is little-endian).

```
offset  size  field          description
------  ----  -----          ---------------------------------------------
  0      4   Length          int32 LE; size of the following Image
                             (in bytes)
  4   Length Image           raw image bytes (format determined by the
                             image provider — typically encoded JPEG or
                             pre-rendered RGBA buffer)
```

Source: `PipeImageSender.cs:56-58` writes a 4-byte length prefix
then the image bytes; `PipeClientManager.cs:41-50` reads the
4-byte prefix into an `int`, allocates a buffer, and reads the
image payload. Cadence: `Task.Delay(50)` between successive
frames (`PipeImageSender.cs:59` — i.e., capped at 20 fps).

**Server lifetime:** the service supports up to **2 concurrent pipe
clients** (`PipeImageSender.cs:45` —
`new NamedPipeServerStream(pipeName, 2, -1)` — `MaxNumberOfServerInstances=2`,
`PipeDirection.InOut=-1`).

### LWireless child families

The `LWireless/` folder contains six sub-folders (one per receiver
family). Each child rides the same `slv3` SDK passthrough — the
host-side serializes a per-family config object and passes it to
`RFController.Set*` methods that send it over RF.

#### Child family: TLV2LCD

- **Source folder:** `LConnectCore/Products/LWireless/TLV2LCD/`
  (one file: `TLV2LCDFirmwareVersion.cs`)
- **Marketed name:** Lian Li UNI FAN TL LCD V2 (the wireless variant)
- **ControllerTypeID:** `TLV2LCD = 16982016`
  (`ControllerTypeID.cs:30`)
- **USB VID:PID:** `0x1CBE:0x0006` (decimal `7358:6`) — note this is
  the **wired** USB ID for the TL V2 LCD controller; over wireless,
  it appears as a child of the LWireless hub
  (`KnownDevices.cs:63`). Wired and wireless paths share the type
  ID.
- **Wire format:** **passthrough via `slv3` SDK over RF.** The host
  serializes a `LightEffectSet` with mode = a `TLEffects` enum
  value (`LWirelessLightingConfig.cs:96` — `TLV2Mode = 11` default).
  No direct HID byte layout visible from this folder.
- **Firmware version format:** a string matching the regex
  `^tlv2_\d{8}(\d{2})(?:_\d{2})?$` (i.e.,
  `tlv2_YYYYMMDD<seq>[_<rev>]`) — see
  `TLV2LCDFirmwareVersion.cs:46`. Decoded version is
  `(seq + 9) / 10 . (seq + 9) % 10`.
- **Image transport:** named-pipe IPC (see above) — the service-side
  `TLV2LCDController` opens a `PipeImageSender` per device
  (`L-Connect-Service/LConnectService/Products/TLV2LCD/TLV2LCDController.cs:1500, 1608`)
  for UI preview mirroring.
- **Status:** **passthrough** — no transformation visible at the
  LWireless layer; per-device config is sent via the SDK's
  per-device `SetLightEffect(macStr, ...)` path.

#### Child family: Lancool 217 (Infinity)

- **Source folder:** `LConnectCore/Products/LWireless/Lancool217Inf/`
  (one file: `Lancool217InfScopes.cs` — defines effect scopes
  `All`, `FrontFan`, `BackFan`, `InfCase`)
- **Marketed name:** Lian Li Lancool 217 Infinity (case with built-in
  wireless ARGB front + rear fans + infinity-mirror panel)
- **ControllerTypeID:** **none** — Lancool 217 Infinity does not
  appear in `ControllerTypeID.cs`. It exists only as an addressable
  scope/device type *behind* the wireless hub. The hub
  (`WirelessController*`) is the ControllerTypeID; the Lancool 217
  is identified at runtime by its 6-byte MAC over RF.
- **USB VID:PID:** **N/A** — wireless-only; reachable via the hub's
  VID/PID.
- **Constants:** front/rear fan speed ranges are in
  `LWireless/Constants.cs:115-121`:
  - `Lancool217RearMinFanSpeed = 430`, `MaxFanSpeed = 1900`
  - `Lancool217FrontMinFanSpeed = 200`, `MaxFanSpeed = 1500`
- **Lighting effect enum:** `slv3.models.LC217Effects`
  (`LWirelessLightingConfig.cs:53`).
- **Special command:** `RFController.Close217Wifi(macStr)` — wired
  via `RequestType.DisableLC217WirelessSignal` (enum index 45,
  `LWirelessController.cs:1191, 1693-1697`). Unique to Lancool 217;
  no other child family has a "disable wireless signal" path.
- **Wire format:** **passthrough via `slv3` SDK over RF.** Per-scope
  effects are sent via `RFController.SetLightEffect(macStr,
  LightEffectSet)` where `LightEffectSet` carries an
  `LC217Effects` mode value.
- **Status:** **passthrough**, with a **single transformative
  command** (`Close217Wifi` / `DisableLC217WirelessSignal`) that
  is unique to this child family and has no analogue elsewhere.

#### Child family: SLV3

- **Source folder:** `LConnectCore/Products/LWireless/SLV3/`
  (`LWirelessLightingConfig.cs`, `LWirelessUtilityLightEffectsSetting.cs`)
- **Marketed name:** Lian Li UNI FAN SL V3 (wireless variant — wired
  SL V3 LCD has its own ControllerTypeID `SLV3LCD = 16977920`)
- **ControllerTypeID:** wireless SLV3 fans appear behind the hub
  (no top-level type ID); the wired-LCD sibling is `SLV3LCD`.
- **USB VID:PID:** **N/A** for wireless SLV3 fans (rides the hub).
- **Wire format:** **passthrough.** `LWirelessLightingConfig`
  (`SLV3/LWirelessLightingConfig.cs:1-132`) is a host-side
  data class carrying `SLV3Mode`, fan-count, LED-num, brightness,
  speed, direction, MAC string. Serialized into a `slv3` SDK
  `LightEffectSet` and sent via `RFController.SetLightEffect`.
- **Status:** **passthrough.**

#### Child family: SLV3LCD

- **Source folder:** `LConnectCore/Products/LWireless/SLV3LCD/`
  (one file: `SLV3LCDFirmwareVersion.cs` — separate from the
  top-level `Products/SLV3LCD/` folder)
- **Marketed name:** Lian Li UNI FAN SL V3 LCD (wireless variant of
  the LCD fan)
- **ControllerTypeID:** `SLV3LCD = 16977920`
  (`ControllerTypeID.cs:29`)
- **USB VID:PID:** `0x1CBE:0x0005` (decimal `7358:5`) for the wired
  USB path (`KnownDevices.cs:61`); wireless variant rides the hub.
- **Wire format:** **passthrough.** Image transport via named-pipe
  IPC (see above) — `SLV3LCDController` runs `PipeImageSender`
  (`L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs:1499, 1607`).
- **Status:** **passthrough.** Note: a sibling agent is documenting
  this child family in detail; this entry is a stub for the
  LWireless cross-reference.

#### Child family: HydroShiftIILCD

- **Source folder:** `LConnectCore/Products/LWireless/HydroShiftIILCD/`
  (`LWirelessPumpConfig.cs`, `HydroShiftIILCDBrightnessSetting.cs`,
  `HydroShiftIILCDRotateSetting.cs`,
  `HydroShiftIILCDWirelessTemplate.cs`,
  `WirelessAdvanceModeRequest.cs`,
  `WirelessH2SensorUpdateSetting.cs`,
  `WirelessTemplateRequest.cs`,
  `ScreenRotate.cs`)
- **Marketed name:** Lian Li HydroShift II LCD (Circle / Square AIO
  variants), wireless mode
- **ControllerTypeID:** four entries — `HydroShiftIILCDCircle =
  16986112`, `HydroShiftIILCDCircleDisplayMode = 16986113`,
  `HydroShiftIILCDSquare = 16986128`, `HydroShiftIILCDSquareDisplayMode =
  16986129` (`ControllerTypeID.cs:31-34`), plus the receivers
  `HydroShiftIILCDCircleReceiver = 50540546`,
  `HydroShiftIILCDSquareReceiver = 50540562` (lines 40-41).
- **USB VID:PID:** Circle wired `0x1CBE:0xA021` (`7358:40993`),
  Circle DisplayMode `0x1A86:0xAD20` (`6790:44320`), Square wired
  `0x1CBE:0xA034` (`7358:41012`), Square DisplayMode
  `0x1A86:0xAD22` (`6790:44322`) (`KnownDevices.cs:65-71`).
- **Wire format:** **partially transformative.** This child family
  has the most LWireless-specific commands in
  `LWireless/RequestType.cs` (entries 32-44):
  `SetPumpLCDBrightness`, `SetPumpLCDRotate`,
  `GetHydroShiftIILCDHandshakeInfo`, `GetWaterTemperature`,
  `SwitchAdvanceMode`, `SetPumpCarouselSettings`,
  `ApplyPumpWirelessTemplate`, `WiredHydroShiftIILCDDisposed`,
  `CheckAdvanceMode`, `PlayWirelessContents`, `WiredStopContents`,
  `WiredPlayContents`, `GetWirelessPumpSetting`. The
  "Wired*" / "Wireless*" prefix split implies the same logical
  operation has separate code paths for wired vs wireless
  transport.
- **Status:** **mostly transformative.** This child has the largest
  surface area in LWireless's `RequestType` enum and the most
  bespoke transport logic. A sibling agent is documenting the
  full HydroShift II LCD family separately.

#### Child family: StrimerPlus

- **Source folder:** `LConnectCore/Products/LWireless/StrimerPlus/`
  (`FirmwareVersion.cs`, `StrLightingConfig.cs`,
  `StrSeparateLightingConfig.cs`)
- **Marketed name:** Lian Li Strimer Plus (RGB cable extension),
  wireless variant
- **ControllerTypeID:** the wired Strimer Plus has its own type ID
  `StrimerPlus = 16801792` (`ControllerTypeID.cs:15`); the wireless
  variant rides the hub.
- **USB VID:PID:** wired `0x0CF2:0xA200` (`3314:41472`)
  (`KnownDevices.cs:35`); wireless rides the hub.
- **Wire format:** **passthrough.** `RFController.SetStrSingleLightEffect(macStr,
  arg1, arg2, arg3, arg4, arg5, arg6)` takes 6 separate effect-set
  arguments — one per Strimer Plus channel
  (`LWirelessController.cs:2444`). Constants:
  `StrMaxChannelCount = 3`, `StrPortPerChannel = 6`,
  `StrMaxLEDCountPerPort = 27`, plus per-type LED counts 88/116/132/174
  (`Constants.cs:99-111`).
- **Status:** **passthrough.**

### Commands (logical RequestType enum, 46 entries)

The `RequestType` enum (`LWireless/RequestType.cs:5-53`) is the
**host-side service-dispatch enum**, not a wire-byte enum. Each
entry maps to a `handle*` method in `LWirelessController.cs` which
calls 1+ `RFController.*` SDK methods. The numeric values are the
enum ordinals (sequential 0..45), not wire bytes. Status flag
`inferred-static` for all entries — wire layer is opaque.

| Idx | Name | Handler citation | SDK call | Notes |
|---|---|---|---|---|
| 0 | `FanGroupStatus` | `LWirelessController.cs:1502` | `RFController.GetAllGroup` | Returns bound-group list with status snapshot. |
| 1 | `WirelessDeviceStatus` | `LWirelessController.cs:1507` | `GetAllGroup` + `GetAioTemp` | Per-device live status. |
| 2 | `SortGroupIndex` | `LWirelessController.cs:1512` | `RFController.SortGroup` | Re-orders device groups. |
| 3 | `SortToLast` | `LWirelessController.cs:1517` | `RFController.SortToLast` | Moves a group to the end of the order. |
| 4 | `FirmwareVersion` | `LWirelessController.cs:1522` | `RFController.GetVersionInfo` | Returns hub TX/RX version pair. |
| 5 | `Direction` | falls to default (no-op) | (none) | Enum-declared, handler missing — flagged unclear. |
| 6 | `FanGroupConfig` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 7 | `FanLightingSetting` | `LWirelessController.cs:1540` | `RFController.SetLightEffect` | Per-device fan lighting. |
| 8 | `FanMergeLightingSetting` | `LWirelessController.cs:1545` | `RFController.SetUIEffect` | Merge-mode (multi-device sync) lighting. |
| 9 | `StrLightingSetting` | `LWirelessController.cs:1550` | `RFController.SetLightEffect` (DevType=Strimer) | Strimer Plus lighting. |
| 10 | `StrSingleLightSetting` | `LWirelessController.cs:1555` | `RFController.SetStrSingleLightEffect` | Per-channel Strimer effect. |
| 11 | `FanQuantity` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 12 | `MergeOrder` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 13 | `TestLight` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 14 | `BindDevice` | `LWirelessController.cs:1560` | `RFController.Bind` | Pair a wireless receiver. |
| 15 | `BindAllDevice` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 16 | `UnBindDevice` | `LWirelessController.cs:1565` | `RFController.UnBind` | Unpair a receiver. |
| 17 | `UnBindAllDevice` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 18 | `ResetDevice` | falls to default (no-op) | (none) | Enum-declared, handler missing. **Possibly destructive** — flagged. |
| 19 | `ChangeChannel` | `LWirelessController.cs:1570` | `RFController.ChangeChannel` | Change RF channel. |
| 20 | `Ping` | `LWirelessController.cs:1575` | (varies) | Visual-identification flash. |
| 21 | `SetFanSpeed` | `LWirelessController.cs:1580` | `RFController.SetFansRPM` | 4-byte PWM array per device, mapped 0..=100 percent → 0..=255. |
| 22 | `SetPumpSpeed` | `LWirelessController.cs:1585` | `RFController.SetAioPumpSpeed` (DevType=10) or `SetH2SAioPumpSpeed` (DevType=11) | RPM, not PWM. |
| 23 | `SetCaseSpeed` | `LWirelessController.cs:1590` | `RFController.SetFansRPM` (case-fan variant) | Lancool 217 case-fan path. |
| 24 | `SaveEffectConfig` | `LWirelessController.cs:1595` | `RFController.SaveCfg` | Persist current state to dongle's flash. |
| 25 | `SetAllFanLightEffect` | `LWirelessController.cs:1600` | `RFController.SetAllLightEffect` | Type-wide lighting (broadcast). |
| 26 | `SetAllStrLightEffect` | `LWirelessController.cs:1605` | `RFController.SetAllLightEffect` | Strimer broadcast. |
| 27 | `GetAllMasterController` | `LWirelessController.cs:1610` | `RFController.GetAllMasterController` | List paired hubs (multi-hub setup). |
| 28 | `GetMasterCount` | `LWirelessController.cs:1615` | `RFController.GetAllMasterController` | Count of paired hubs. |
| 29 | `GetChannel` | `LWirelessController.cs:1620` | (lookup) | Current RF channel. |
| 30 | `LockBoundDevices` | `LWirelessController.cs:1625` | `RFController.LockDevice` | Prevent further pairing. |
| 31 | `IsDevicesLocked` | `LWirelessController.cs:1630` | `RFController.IsLock` | Pairing-lock state. |
| 32 | `SetPumpLCDBrightness` | `LWirelessController.cs:1635` | (HydroShift II LCD path) | Brightness 0..=100. |
| 33 | `SetPumpLCDRotate` | `LWirelessController.cs:1640` | (HydroShift II LCD path) | Rotation enum. |
| 34 | `GetHydroShiftIILCDHandshakeInfo` | `LWirelessController.cs:1645` | (HydroShift II LCD path) | Per-pump handshake. |
| 35 | `GetWaterTemperature` | `LWirelessController.cs:1650` | `RFController.GetAioTemp` | AIO water temperature. |
| 36 | `SwitchAdvanceMode` | `LWirelessController.cs:1655` | (HydroShift II LCD path) | Advanced-mode toggle. |
| 37 | `SetPumpCarouselSettings` | `LWirelessController.cs:1660` | (HydroShift II LCD path) | Pump-cap carousel/template. |
| 38 | `ApplyPumpWirelessTemplate` | `LWirelessController.cs:1665` | `RFController.SetAioThemeIndex` | Apply pre-stored template. |
| 39 | `WiredHydroShiftIILCDDisposed` | `LWirelessController.cs:1670` | (cleanup) | Disposal hook for wired-mode HS II LCD. |
| 40 | `CheckAdvanceMode` | `LWirelessController.cs:1675` | (HS II LCD) | Query advanced-mode state. |
| 41 | `PlayWirelessContents` | `LWirelessController.cs:1680` | (HS II LCD) | Start wireless content stream. |
| 42 | `WiredStopContents` | `LWirelessController.cs:1685` | (HS II LCD) | Stop wired content stream. |
| 43 | `WiredPlayContents` | falls to default (no-op) | (none) | Enum-declared, handler missing. |
| 44 | `GetWirelessPumpSetting` | `LWirelessController.cs:1690` | (HS II LCD) | Read current pump config. |
| 45 | `DisableLC217WirelessSignal` | `LWirelessController.cs:1695` | `RFController.Close217Wifi` | **Lancool 217-only** — disables wireless on the case. |

`Phase 0d status:` not in scope; protocol layer is opaque.

### Subsystem patterns

- **SDK passthrough is the dominant pattern.** Almost every
  `LWirelessController.handle*` method does some host-side
  validation/serialization, then dispatches to one or two
  `RFController.*` SDK calls. Wire-level framing is invisible
  outside `slv3.dll`.
- **MAC-string addressing.** All per-device commands take a
  `macStr` (6-byte MAC formatted as a hex string) as their first
  argument. The hub's RF protocol presumably carries the MAC as a
  destination address. Example: `RFController.SetFansRPM(macStr,
  duty)`, `RFController.SetLightEffect(macStr, set, null)`.
- **PWM is mapped 0..=100 → 0..=255 before transmission.** See
  `LWirelessDevice.cs:112` — fan duty cycle scales from percent to
  byte before being passed to the SDK. Different from LCDV2/TLFan
  where PWM is u8 0..=100 directly on the wire.
- **Image transport is out-of-band.** UI-side LCD content is
  rendered by a host-side composer and passed to the service
  through a named pipe; the service forwards it via the SDK. The
  pipe never touches RF.
- **Multi-hub support.** `GetAllMasterController` returns a list,
  and per-MAC commands include the MAC explicitly — multiple hubs
  with overlapping ranges are supported, with channel disambiguation
  via `ChangeChannel`.
- **Pairing semantics.** `BindDevice` / `UnBindDevice` /
  `LockBoundDevices` / `IsDevicesLocked` form a pairing-state
  machine. Devices must be bound before any other command targeting
  their MAC will reach them.

### Notable divergences from LCDV2

- **Opaque transport.** LCDV2 has a documented A/B/C-class HID
  framing with explicit ReportIDs, command bytes, and payload
  layouts. LWireless has none of that visible — everything is
  inside `slv3.dll`. This is the largest divergence in the entire
  cross-device survey.
- **MAC-string addressing instead of fixed device-handle.** LCDV2
  is one HID handle per device; LWireless multiplexes many devices
  behind one handle by MAC.
- **Out-of-band image pipe** (named pipe, length-prefixed frames)
  for UI preview mirroring; LCDV2 has nothing equivalent.
- **PWM scaling 0..=100 → 0..=255.** LCDV2/TLFan pass PWM as 0..=100
  on the wire; LWireless rescales before handing to the SDK.
- **Multi-tier device hierarchy.** Hub → MAC → child-family. LCDV2
  is flat: one device, one command space.
- **Pairing / locking is first-class.** LCDV2 has no pairing
  concept (USB enumeration is the binding).
- **Two HW generations on different USB silicon.** V1 hub is
  `0x0416:0x80XX` (Winbond), V2 hub is `0x1A86:0xE3XX` (WCH). LCDV2
  is single-generation.

### Open questions / unclear from static analysis

- **The full wire-level framing.** Without a USB capture or `slv3.dll`
  reverse engineering, the byte-level packet shape, command bytes,
  and ReportID(s) of the hub HID interface are unknown. **Phase 1
  of LWireless support requires reverse-engineering `slv3.dll`** or
  capturing live USB traffic.
- **Enum entries 5, 6, 11, 12, 13, 15, 17, 18, 43** all fall through
  to the default (no-op) case in `HandleRequestData`
  (`LWirelessController.cs:1525-1537, 1688-1701`). Whether these
  are dead code, unfinished features, or routed to a different
  dispatch elsewhere is unclear.
- **`ResetDevice` (idx 18)** — declared but unhandled. If the
  wireless hub does support reset, the SDK call name is unknown.
  Flag as potentially destructive.
- **Pipe protocol image format.** The 4-byte length prefix is
  documented, but the payload format (raw RGBA buffer? encoded
  JPEG? encoded PNG?) is not. The image-provider callback in
  `PipeImageSender.StartSendingImages` returns `byte[]`; the
  format is set by the caller. Live capture or composer-side
  source inspection needed.
- **Pipe endianness.** `BitConverter.GetBytes(int)` uses host-
  endianness — Windows x64 is little-endian. A Linux daemon must
  hard-code little-endian to match (use `i32::to_le_bytes` /
  `i32::from_le_bytes`).
- **Pipe access control.** `NamedPipeServerStream(pipeName, 2, -1)`
  uses default ACL; on Windows that's per-user. The Linux
  equivalent (Unix-domain socket) needs an explicit access-mode
  policy.
- **Frame cadence.** `Task.Delay(50)` between successive image
  writes caps the pipe at 20 fps. Whether the wireless RF link can
  carry that rate to the LCD is unverified.
- **Pairing flow byte sequence.** `BindDevice` / `UnBindDevice` /
  `LockBoundDevices` are SDK calls; the actual RF frame exchange
  for pairing (button-press timing, key-exchange) is opaque.
- **Lancool 217 `Close217Wifi` semantics.** Stops the case from
  listening to the hub, but is the receiver fully disabled (radio
  off), or just unsubscribed from this hub? Whether it
  auto-resumes on power-cycle is undocumented.
- **HydroShift II LCD content streaming** (`PlayWirelessContents`,
  `WiredPlayContents`, `WiredStopContents`) is a substantial
  command set with no observable wire-level shape. Sibling agent
  documenting that family separately should clarify.
- **Cross-MCU code paths inside `slv3.dll`.** V1 vs V2 hubs
  presumably need different USB framing (different VIDs, different
  bridge silicon). Whether the SDK auto-detects or the host has
  to pick is invisible from the C# layer.
