## Shared protocol layer: lcd207 SDK

**Source:** decompiled `lcd207.dll` at `/home/inky/Development/lian-li/lcd207/lcd207/`
**Status:** all citations are inferred-static (decompiled artefacts: anonymous-method names retained, `IL_xxxx` hints sprinkled where the WPF Color marshalling roundtripped through unverifiable IL — e.g. `WinUsbH2.cs:988-991`, `AioParams.cs:45-47` — but every wire-relevant code path is intact and unambiguous).
**Used by these families (confirmed by L-Connect-Service `using lcd207;` directives):**

- **HydroShift II LCD Circle** — `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2CSDK.cs:1-187` is a thin pass-through to `lcd207.H2Controller`. Selected by `HydroShiftIILCDController.cs:311` as the SDK for `DeviceType.Circle` (PID 0xA021 / 40993).
- **HydroShift II LCD Square** — `L-Connect-Service/LConnectService/Products/HydroShiftIILCD/H2SSDK.cs:1-187` is a thin pass-through to `lcd207.H2SController`. Selected by `HydroShiftIILCDController.cs:312` as the SDK for `DeviceType.Square` (PID 0xA034 / 41012).
- **Lancool 207 Digital** — `L-Connect-Service/LConnectService/Products/Lancool207Digital/Lancool207DigitalController.cs:1-2` (`using lcd207;`); calls `lcd207.LCDController.{ApplyTemplate, GetTemplate, GetBitmapByTheme, IsAvailable, Ver, GetDeviceVer, GetModulars, UpdateModular, UpdateThemeInfo}` (lines 454, 507, 924, 1024-1025, 1094, 1108, 1137, 1168, 1225, 1427).
- **UniversalScreen 8.8"** — `L-Connect-Service/LConnectService/Products/UniversalScreen8p8Inch/UniversalScreen8p8InchController.cs:1` (`using lcd207;`); the `sdkController` field at line 132 is typed `LEDController`, with calls to `LEDController.GetTemplate` (432), `LEDController.UpdateThemeInfo` (1105), and `bindSDKController(out LEDController, ...)` (547-549). **This confirms the open question raised in the per-family staging files** — UniversalScreen 8.8" is *also* a lcd207 client, on the LCD side (PID 0xA088 / 41096) plus an LED side (PID 0x8050 / 32848 VID 0x0416 / 1046) and a HID-side wireless-sensor companion (PID 0xAD21 / 44321 VID 0x1A86 / 6790).
- **L-Connect-Service shared logger** — `L-Connect-Service/LConnectService/Products/LCD207DLLLoggerWrapper.cs` adapts the SDK's `Loger.cs` to Serilog. Not a wire consumer.

The four product families plus the legacy-wrapper share **one** `CmdType` enum, **one** DES-CBC encryption envelope (key `slv3tuzx`, identical to the slv3 SDK — see "Notable findings" §3), and three fan-out transport classes: `WinUsb` (Lancool 207 LCD), `WinUsbH2` (HS II Circle), `WinUsbH2S` (HS II Square). `WinUsbLed` is a sub-class of `WinUsb` for the UniversalScreen 8.8" LED-side and adds an unencrypted 64-B raw-frame protocol on top of the same writer/reader. `HIDUsb` is a thin HidLibrary wrapper used for SN-discovery and `SetMonitorMode` / `SetDesktopMode` magic-byte signalling; it is independent of the encrypted CmdType protocol.

### Transports

| Transport class | Wire protocol | VID / PID(s) | Endpoint / signature | Buffer / chunking | Used by |
|---|---|---|---|---|---|
| `HIDUsb.cs` | HID (HidLibrary) | VID 0x1A86 (6790); PID 0xAD21 (44321) for UniversalScreen 8.8"; PID 0xAD20 (44320) for HS II; PID 0xAD11 (44305) for Lancool 207 — `HIDUsb.cs:27-29,46-47`, `H2Controller.cs:127,1144`, `LCDController.cs:122` | 64-byte HID reports, ReportId = 0 — `HIDUsb.cs:124-134` | 64 B in, 64 B out. SN discovery: probe writes `[0xAA, 0x55, 0x35]` and reads back the SN as ASCII at offset 2..18 — `HIDUsb.cs:82-89`. `SetMonitorMode` writes the magic 8-byte string `35 66 33 37 35 39 64 66` ("5f3759df") — `H2Controller.cs:1147`. | Out-of-band SN/serial-number probe and desktop-mode toggle. Not the per-frame transport. |
| `WinUsb.cs` | LibUsbDotNet WinUSB / interrupt endpoints | VID 0x1CBE (7358), PID 0xA065 (41061) — `WinUsb.cs:25-27,167,206,423` | `WriteEndpointID.Ep01` writer + `ReadEndpointID.Ep01` interrupt reader, 512-byte packets — `WinUsb.cs:450-464` | LCD path: 102,400+ B output buffer; encrypted 504-B header + raw image body at offset 512 — `WinUsb.cs:518-546,590-610`. H264 video-stream chunking: 202,752 B per `Read` slice fed under cmd `StartPlay = 0x79` — `WinUsb.cs:343-371`. | Lancool 207 Digital LCD (the wired Lancool 207 panel). |
| `WinUsbH2.cs` | LibUsbDotNet WinUSB / interrupt endpoints | VID 0x1CBE (7358), PID 0xA021 (40993) — `WinUsbH2.cs:30-32,167,208,425` | Same Ep01 writer + Ep01 interrupt reader, 512-byte packets — `WinUsbH2.cs:448-485` | Same envelope as `WinUsb` (102,400+ B, encrypted 504-B header at offset 0, body at offset 512). Adds an in-process FIFO `msgQueue` of `(CmdType, byte[])` pairs polled at 330 ms cadence with periodic `GetH2Params` and `SyncPumpFan` — `WinUsbH2.cs:480-555`. | HydroShift II LCD Circle. |
| `WinUsbH2S.cs` | LibUsbDotNet WinUSB / interrupt endpoints | VID 0x1CBE (7358), PID 0xA034 (41012) — `WinUsbH2S.cs:30-32,167,208,425` | Same as `WinUsbH2`. | Identical to `WinUsbH2` byte-for-byte at the wire level (`diff WinUsbH2.cs WinUsbH2S.cs` shows only PID, class-name, log-string, and the pump-RPM-to-PWM mapping table differing — see "Notable findings" §4). The pump-RPM-to-PWM mapping table differs: H2 covers 1600-2500 RPM in 6 segments; H2S covers 1600-3200 RPM in 8 segments — `WinUsbH2.cs:1031-1063` vs `WinUsbH2S.cs:1037-1071`. | HydroShift II LCD Square. |
| `WinUsbLed.cs` | LibUsbDotNet WinUSB / interrupt endpoints (subclasses `WinUsb`) | LCD-side: VID 0x1CBE (7358), PID 0xA088 (41096) — `WinUsbLed.cs:26-28`. LED-side: VID 0x0416 (1046), PID 0x8050 (32848) — `WinUsbLed.cs:30-32`. | Inherits Ep01 writer/reader. `IsLcd = true` for the LCD-side device. | LCD-side: encrypted 504-B header (inherited from `WinUsb` parent). LED-side: **raw 64-B frames**, no encryption — `WinUsbLed.cs:108-174`. The LED firmware-update path uses 60-B chunks with 4080-B end-of-page boundaries (very similar to slv3's RF firmware update). | UniversalScreen 8.8" panel + LED side. The LCD-side and LED-side endpoints are paired by USB-hub topology (`HubID` matching) in `LEDController.GetAllDevices` — `LEDController.cs:135-160`. |

The lcd207 SDK shares `slv3.models.WinDevice` / `WinDeviceType` / `USBStatus` for transport descriptors (`WinUsb.cs:14-15` `using slv3.models;`). It also calls `slv3.RFController.GetAllGroup()` from inside `WinUsbH2.SendPumpFan` / `WinUsbH2S.SendPumpFan` (`WinUsbH2.cs:1086`, `WinUsbH2S.cs:1095`) to **skip** local pump-fan sync if a wireless waterblock peer is bound (`item.bind_to_master && item.dev_type == 10` for H2; `dev_type == 10 || 11` for H2S). This is the only behavioural cross-link between the two SDKs.

### USB command table (CmdType.cs)

Citations: `CmdType.cs:5-25`. The opcode lives at `header[0]` *before* DES-CBC encryption. Header layout (built by `GetBaseCmdBuf`): `[0]=cmd`, `[2]=0x1A`, `[3]=0x6D` (constant magic), `[4..7]=little-endian uint32 ms-since-app-start` — `WinUsb.cs:590-599`, identical in `WinUsbH2.cs:668-677` and `WinUsbH2S.cs:668-677`. The body length goes at `[8..11]` BE for single-buffer commands or at `[12..15]` BE for two-buffer commands (`UpdateFireWare`, MCU FW chunks, RGB). After encryption, the trailing two bytes of the 512-B encrypted header are forced to `0xA1 0x1A` — `WinUsb.cs:607-609`.

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| 0x0A (10) | `GetVer` | host → device, response | `WinUsb.GetVer`, `WinUsbH2.GetVer`, `WinUsbH2S.GetVer` | `CmdType.cs:5`, `WinUsb.cs:777-789`, `WinUsbH2.cs:855-867`, `WinUsbH2S.cs:855-867` | Reply: `[0]=0x0A`, `[8..40]` = UTF-8 version string, NUL-trimmed. |
| 0x0B (11) | `Reboot` | host → device | `Reboot()` after FW write; `SetDesktopMode` | `CmdType.cs:6`, `WinUsb.cs:717-721`, `H2Controller.cs:1129` | Sent post-firmware-write to relaunch into new image. |
| 0x0D (13) | `Rotate` | host → device | `H2Controller.Rotate`, `H2SController.Rotate`, `LCDController.Rotate`, `LEDController.Rotate` | `CmdType.cs:7`, `H2Controller.cs:1659`, `H2SController.cs:1663`, `LCDController.cs:1533`, `LEDController.cs:1722` | `header[8]` = rotation enum byte (0–3). |
| 0x0E (14) | `BrigthSet` | host → device, response | `H2Controller.SetBrightness`, `H2SController.SetBrightness`, `LCDController.SetBrightness`, `LEDController.SetBrightness` | `CmdType.cs:8`, `H2Controller.cs:1675`, `H2SController.cs:1679`, `LCDController.cs:1549`, `LEDController.cs:1740` | `header[8]` = brightness 0–100. |
| 0x0F (15) | `SetFrameRate` | host → device | every transport's `SetFrameRate` and `InitDev` | `CmdType.cs:9`, `WinUsb.cs:905-909`, `WinUsbH2.cs:473`, `WinUsbH2S.cs:473`, `H2Controller.cs:1742` | Sets device-side decode FPS for H264 playback. `WinUsbH2.InitDev` always sends 30 fps; `H2Controller.PlayH264` overrides with `GetAvFrameNum` from the file. |
| 0x28 (40) | `UpdateFireWare` | host → device | `WinUsb.UpdateFile` (firmware **and** boot.jpg) | `CmdType.cs:10`, `WinUsb.cs:633-715`, `WinUsbH2.cs:711-793`, `WinUsbH2S.cs:711-793` | **Repurposed as a generic filesystem-write opcode** (identical to slv3's pattern). Header layout: `[0]=40`, `[8..11]=BE filename length`, `[12..15]=BE file body length`, `[16..]=ASCII filename`. Body at offset 512 of the outer buffer. Targets: `/usr/data/update.app` (FW), `/usr/data/boot.jpg` (stored boot logo via `H2Controller.UploadLogo` — `H2Controller.cs:1880-1899`). After firmware writes, also calls `Reboot()`. |
| 0x2A (42) | `DelFile` | host → device | `WinUsb.DelFileOrDir` and equivalents | `CmdType.cs:11`, `WinUsb.cs:723-733`, `WinUsbH2.cs:801-811`, `WinUsbH2S.cs:801-811` | `header[0..3]=BE path_len`, `header[8..]=ASCII path`. Filesystem deletion on the device's embedded Linux. |
| 0x33 (51) | `SetClock` | host → device, response | `WinUsb.SyncClock` family | `CmdType.cs:12`, `WinUsb.cs:745-770`, `WinUsbH2.cs:823-848`, `WinUsbH2S.cs:823-848` | Sets device RTC. `header[8..15]` = `[year_hi, year_lo, month, day, hour, minute, second, mode]`. `mode = 0` disable, `1` enable, `2` one-shot sync. Reply `[0]=0x33` for ack. |
| 0x34 (52) | `StopClock` | host → device | `WinUsb.StopClock`, called from `H2Controller.ApplyTemplate` | `CmdType.cs:13`, `WinUsb.cs:772-775`, `H2Controller.cs:1209` | Suppresses the device-side built-in clock face overlay so the host can render its own. |
| 0x63 (99) | `QueryDir` | host → device, response | `WinUsbH2.QuerryDir`, `WinUsbH2S.QuerryDir` (Lancool 207 `WinUsb.cs` does NOT define it but it would inherit) | `CmdType.cs:14`, `WinUsbH2.cs:313-329` | Lists a device-side directory. Loops 20× (reads 20 × 500-B chunks = 10 KB total) and concatenates to UTF-8 string. Used to discover stored `/media/bg_h264/*.h264` slot names — `H2Controller.cs:1700`. |
| 0x65 (101) | `PushJpg` | host → device | every transient JPG frame upload | `CmdType.cs:15`, `WinUsb.cs:509-583`, `WinUsbH2.cs:587-661`, `WinUsbH2S.cs:587-661` | **Transient frame upload (JPG).** Header `[8..11]` = BE body length; body at offset 512. **No persistence.** Co-opts opcode 102 (no enum entry; raw byte) for the PNG variant: `cmd = IsPng ? 102 : 101` — `WinUsb.cs:519`, `WinUsbH2.cs:597`. The PNG path is the same wire format with a different leading byte. |
| 0x79 (121) | `StartPlay` | host → device | `WinUsb.PlayH264`, `WinUsbH2.PlayH264`, `WinUsbH2S.PlayH264` | `CmdType.cs:16`, `WinUsb.cs:335-407`, `WinUsbH2.cs:337-409`, `WinUsbH2S.cs:337-409` | **H264 video-frame upload.** `header[8..11]` = BE chunk-length (per call). Each call carries up to 202,752 B at body offset 512. Repeats by seeking back to file start; the file is a host-side `.h264` elementary stream produced by `FFmpegHelper.TransferH264` (`H2Controller.cs:1734`). After every chunk: `await Task.Delay(30)`; if `ret[8] > 3` (back-pressure indicator from device), `await waitBuffer(2)` — i.e. poll `QueryBlock` until the device's buffer drops to ≤ 2 outstanding blocks. |
| 0x7A (122) | `QueryBlock` | host → device, response | `waitBuffer` / `waitBufferAsync` | `CmdType.cs:17`, `WinUsb.cs:313-327`, `WinUsbH2.cs:297-311`, `WinUsbH2S.cs:297-311` | Returns the device's current outstanding-block count at `ret[8]`. Pure flow-control. |
| 0x7B (123) | `StopPlay` | host → device | `WinUsb.StopPlay`, `H2Controller.PlayStop` | `CmdType.cs:18`, `WinUsb.cs:329-333`, `H2Controller.cs:1760` | Cancels H264 playback. |
| 0x96 (150) | `SwitchToDesktop` | host → device, response | `WinUsb.SwitchToDesktop`, called from `H2Controller.SetDesktopMode` | `CmdType.cs:19`, `WinUsb.cs:735-743`, `H2Controller.cs:1128` | Tells the device to switch into "desktop mode" (rendering host-streamed pixels rather than its own theme). Reply: `ret[0]=0x96` for ack. Followed immediately by `Reboot()` and `CloseUSB()`. |
| 0xC9 (201) | `GetPosIndex` | host → device, response | (declared) | `CmdType.cs:20` | Defined in enum; no caller located in `lcd207/`. Mirror of slv3's `GetPosIndex=0xC9` — likely group-position query reused at the protocol level but not invoked by lcd207's host code. |
| 0xF9 (249) | `SetWthemeIndex` | host → device | `WinUsbH2.SetAioParamsByUSB`, `WinUsbH2S.SetAioParamsByUSB` | `CmdType.cs:21`, `WinUsbH2.cs:1067`, `WinUsbH2S.cs:1076` | **HS II built-in theme + AIO sensor display config.** 32-byte payload: per-CPU/GPU temp/load enables, fan speed (BE u16), pump speed (mapped to PWM 60-1500 via segment LUT), three RGBA colours (str/val/uint), brightness, theme-index, rotation. Tells the on-device firmware to render its own AIO theme rather than expecting host frames. |
| 0xFA (250) | `GetH2Params` | host → device, response | `WinUsbH2.GetH2Params`, `WinUsbH2S.GetH2Params` | `CmdType.cs:22`, `WinUsbH2.cs:981-984`, `WinUsbH2S.cs:982-985` | Reads 32-B AIO sensor block (CPU/GPU/load/temp + 4 × u16 fan tachs + 6 × ?? + RFVer at byte 31). Polled every ~3.3 s by the in-process queue worker (`getFanCnt > 6` at 330 ms tick). |
| 0xFB (251) | `SyncPumpFan` | host → device | `WinUsbH2.SendPumpFan`, `WinUsbH2S.SendPumpFan` | `CmdType.cs:23`, `WinUsbH2.cs:1134`, `WinUsbH2S.cs:1142` | **Wired pump+fan PWM sync.** 16-B payload: `[0..3]=FF 0F A1 00`, `[4..5]=BE pump_pwm`, `[6..8]=fan1, fan2, fan3` (0–255 PWM bytes), `[9..15]=zeros`. Suppressed if a wireless waterblock is bound (`RFController.GetAllGroup` check). |
| 0xFC (252) | `PushRgbData` | host → device | `WinUsbH2.UpdateRgbData`, `WinUsbH2S.UpdateRgbData` | `CmdType.cs:24`, `WinUsbH2.cs:1216-1244`, `WinUsbH2S.cs:1226-1254` | RGB frame upload for the AIO's onboard LEDs. `header[12..15]` = BE body length (note: NOT `[8..11]`, distinguishing it from `PushJpg`). Body: arbitrary `data[]` followed by 4-byte trailer `[totalFrame_hi, totalFrame_lo, interval, ledNum]`. Enqueued through the message-queue worker. |
| 0xFD (253) | `FactoryH264Test` | host → device | (declared) | `CmdType.cs:25` | No caller in lcd207. Same opcode as slv3's `FactoryH264Test` — factory diagnostic. |
| 0xF7 (247) | (no enum entry) | host → device | `WinUsbH2.UpdateMcuFirmware`, `WinUsbH2S.UpdateMcuFirmware` | `WinUsbH2.cs:1161`, `WinUsbH2S.cs:1177` | **MCU firmware update prep / "enter bootloader" signal.** Cast `(CmdType)247` — undeclared in the enum. Followed by 2 s sleep before chunk streaming. |
| 0xF8 (248) | (no enum entry) | host → device | MCU firmware data chunks | `WinUsbH2.cs:1199`, `WinUsbH2S.cs:1215` | **MCU firmware data chunk.** 4096-B chunks with a 6-B trailer: `[0xAA, 0x55, chunk_index, chunk_count, crc16_hi, crc16_lo]`. Last chunk uses `0xBB` instead of `0xAA` as the start marker. CRC computed by `Util.Compute` (a CRC-16 — not inspected line-by-line). 2 s sleep between chunks. Distinct from the `UpdateFireWare = 40` opcode, which targets the *application* firmware (the embedded Linux); this one targets the *MCU* (the AIO controller microcontroller). |

**Cross-reference:** the lcd207 `CmdType` enum is structurally aligned with the slv3 `CmdType` enum (same opcode for `GetVer`, `Reboot`, `Rotate`, `BrigthSet`, `SetFrameRate`, `UpdateFireWare`, `PushJpg`, `StartPlay`, `QueryBlock`, `StopPlay`, `GetPosIndex`, `FactoryH264Test`) and **adds** `Rotate`, `DelFile`, `SetClock`, `StopClock`, `QueryDir`, `SwitchToDesktop`, `SetWthemeIndex`, `GetH2Params`, `SyncPumpFan`, `PushRgbData`. The shared opcodes have identical wire format. This strongly suggests **lcd207 is a fork/refactor of slv3's `WinUsb.cs`** (probably to add HS II AIO control without polluting slv3's wireless-master abstraction).

### Frame upload mechanism (the cmd-15-hypothesis answer for HS II)

This is the headline finding: the HS II LCD per-family survey could not reach the wire-byte sequence from C# alone because the bytes lived inside this DLL.

#### HS II Circle / Square — JPG / PNG transient pipeline

1. **Service-layer entry point.** L-Connect-Service's `HydroShiftIILCDController` dispatches to `H2CSDK.ApplyTemplate(theme)` for Circle (`H2CSDK.cs:40-42`) or `H2SSDK.ApplyTemplate(theme)` for Square (`H2SSDK.cs:40-42`); both pass through to `lcd207.H2Controller.ApplyTemplate(theme)` / `lcd207.H2SController.ApplyTemplate(theme)`.
2. **SDK orchestration.** `H2Controller.ApplyTemplate` (`H2Controller.cs:1196-1264`):
   1. `winUsb.SyncClock(isEnable: true, onlySync: true)` → cmd `SetClock=0x33` with mode byte `2` (one-shot RTC sync, no built-in clock face).
   2. `winUsb.StopClock()` → cmd `StopClock=0x34` (suppress on-device clock overlay).
   3. `ClearPngLayer()` → renders a blank `Bitmap(480,480)`, encodes PNG, calls `winUsb.PushImage(bytes, IsPng: true)` → cmd byte 102.
   4. `ClearJpgLayer()` → same, encodes JPG quality=95, calls `winUsb.PushImage(bytes, IsPng: false)` → cmd `PushJpg=0x65`.
   5. If `theme.videoPath` is set → `PlayH264(text)` (cmd `StartPlay=0x79` chunked stream). Otherwise if `theme.BgImg` is set → encode JPG via `Util.GetJpgBytes(val, 95L, 512000)` → `PushJpg(jpgBytes)`.
3. **Wire format of `PushImage(bytes, IsPng)`** — `WinUsbH2.cs:626-661`:
   1. Allocate `512 + bytes.Length` output buffer.
   2. `cmd = IsPng ? 102 : 101`.
   3. `GetBaseCmdBuf(cmd)`: 500-B header `[0]=cmd, [2]=0x1A, [3]=0x6D, [4..7]=LE timestamp_lo32`.
   4. Header `[8..11]` = BE `bytes.Length` (full uint32 — though the Lancool 207 path also uses `[8..11]` for body length, the HS II RGB path at `PushRgbData=252` uses `[12..15]` instead).
   5. **`Encrypt(header)`** → DES-CBC with key `73 6C 76 33 74 75 7A 78` ("slv3tuzx"), IV = key, output 512 B (504-B input padded to multiple of 8). The trailing `[510]=0xA1`, `[511]=0x1A` are forced post-encryption — `WinUsbH2.cs:679-688`.
   6. `bytes.CopyTo(output, 512)` — body at offset 512.
   7. `writer.Write(output, 2000ms_timeout)` over WinUSB EP1 OUT.
4. **Stored boot-logo path.** `H2Controller.UploadLogo(filePath)` (`H2Controller.cs:1880-1899`): resizes the bitmap to `(Width=480, Height=480)`, encodes JPG quality 95, wraps in a `MemoryStream`, calls `winUsb.UpdateFile(MemoryStream, "/usr/data/boot.jpg")`. This is cmd `UpdateFireWare=0x28` with `header[8..11]=BE filename_len=20`, `header[12..15]=BE body_len`, `header[16..36]="/usr/data/boot.jpg"`.
5. **Firmware path.** `H2Controller.FirmwareUpdate(filePath)` → `winUsb.UpdateFile(filePath, "/usr/data/update.app")` (`WinUsbH2.cs:711-754`). Identical wire format to the boot-logo path, different filename. Followed by `Thread.Sleep(1000)` and `Reboot()`.
6. **MCU firmware path.** `H2Controller.UpdateAioFirmware(path)` → `winUsb.UpdateMcuFirmware(path)` (`WinUsbH2.cs:1156-1214`). Uses opcodes 247 + 248 (undeclared in `CmdType`), 4096-B chunks with CRC-16 trailer, 2 s inter-chunk sleep, last chunk marked with `0xBB` instead of `0xAA`. Sets `Updating = true` to gate the message-queue worker (the heartbeat / pump-sync polling stops while a firmware push is in flight).

#### HS II — H264 video-stream pipeline (transient, repeating)

`WinUsbH2.PlayH264(filePath, isRepeat)` (`WinUsbH2.cs:337-409`):

1. Open file as `FileStream`.
2. Read up to `bufLen = 202,752` B per iteration into `fileBuffer`.
3. Allocate output `byte[512 + len]`; copy file bytes to offset 512.
4. `GetBaseCmdBuf(121)` → cmd `StartPlay=0x79`. Header `[8..11]` = BE chunk len.
5. `Encrypt(header)` → 512 B at output offset 0.
6. `writer.Write(output)` — single transfer.
7. `await Task.Delay(30)` — 30 ms pacing, matching the default 30 fps `SetFrameRate`.
8. If `ret[8] > 3` → `await waitBuffer(2)` — back-pressure: poll `QueryBlock=0x7A` until device buffer drops to ≤ 2 outstanding blocks.
9. On EOF, if `isRepeat` → seek to 0 and continue. Else break.

The host-side `.h264` stream is produced by `FFmpegHelper.TransferH264` (FFmpeg.AutoGen, in `lcd207/FFMpeg/`) from arbitrary input video. Frame rate is derived from the file (`H264.GetAvFrameNum`) and forwarded via cmd `SetFrameRate=0x0F`.

#### Cmd-15-hypothesis answer for HS II

**Yes — lcd207 has a transient-vs-stored split at the wire level, and it mirrors slv3's pattern exactly.** Confirmed for HS II Circle, HS II Square, and Lancool 207 (the wired LCD families):

- **Transient JPG frame** = `PushJpg = 0x65` (101). Body length at header `[8..11]` BE; body at offset 512. No persistence.
- **Transient PNG frame** = raw byte `0x66` (102). Same wire format, different leading byte (no enum entry — it is a literal `cmd = IsPng ? 102 : 101` switch in `PushImage`).
- **Transient H264 chunk** = `StartPlay = 0x79` (121). Same envelope, body is a `.h264` elementary-stream slice.
- **Stored boot logo** = `UpdateFireWare = 0x28` (40), with target filename `/usr/data/boot.jpg`. *Same opcode as application firmware update, distinguished by filename in header `[16..]`.* This is the lcd207 equivalent of LCDV2 cmd 15 / TLLCD `WriteBootJPG`.
- **Stored firmware** = `UpdateFireWare = 0x28` (40), with filename `/usr/data/update.app`. Followed by `Reboot()`.
- **Stored MCU firmware (AIO controller)** = raw bytes 247 / 248 (undeclared in `CmdType`). Distinct from the application-firmware path.

The HS II per-family survey's opacity is **closed by this finding** — the HS II LCD wired HID transport that the survey could not reach is **actually WinUSB**, not HID; HID is only used out-of-band for SN discovery and `SetMonitorMode`. The actual frame-upload protocol is identical in spirit to slv3's wired-LCD pattern (DES-CBC-encrypted header + raw body at offset 512, stored variant via filename-keyed `UpdateFireWare`).

### Lancool 207 Digital pipeline

Separate from HS II — Lancool 207 has its own LCD path through `LCDController` and `WinUsb` (no H2 suffix). The wire format is **identical** to HS II's at the byte level (same `CmdType`, same DES envelope, same body-at-offset-512 layout). Differences are at the orchestration and dimension layer:

1. **Service-layer entry point.** `Lancool207DigitalController.cs:507` calls `LCDController.ApplyTemplate(theme)`. Theme assets are resolved through `LCDController.GetTemplate` (line 454) and rendered through `LCDController.GetBitmapByTheme` (lines 924, 1024-1025, 1168).
2. **Dimensions.** `LCDController.Width = 720`, `Height = 1472` (`LCDController.cs:72-74`) — Lancool 207 panel resolution. HS II Circle / Square are both `480 × 480`; UniversalScreen 8.8" is `480 × 1920` (`LEDController.cs:76-78`).
3. **Wire-byte sequence.** Identical to HS II:
   - `PushImage(bytes, IsPng)` → cmd 101 (JPG) or raw 102 (PNG).
   - `PlayH264(filePath, isRepeat, token)` → cmd `StartPlay=0x79` chunked.
   - `UpdateFile(filePath, "/usr/data/update.app")` → cmd `UpdateFireWare=0x28` for application FW.
   - Stored boot logo would use `UpdateFile(filePath, "/usr/data/boot.jpg")`. The Lancool 207 controller does not directly expose `UploadLogo` — that's an HS II convenience method — but `LCDController` inherits the same `UpdateFile` capability.
4. **Sensor / control commands.** Lancool 207 does **not** use `SetWthemeIndex=0xF9`, `GetH2Params=0xFA`, `SyncPumpFan=0xFB`, or `PushRgbData=0xFC` — these are HS II-specific because they encode AIO pump / fan / RGB state. Lancool 207 only uses the LCD-side opcodes (`PushJpg`, `StartPlay`, `QueryBlock`, `StopPlay`, `Rotate`, `BrigthSet`, `SetFrameRate`, `SetClock`, `StopClock`, `SwitchToDesktop`, `UpdateFireWare`, `DelFile`, `GetVer`, `Reboot`).
5. **Cross-process IPC.** The L-Connect-Service uses `copyImageMemory(processId, address, screenFrameBuffer)` (`HydroShiftIILCDController.cs:959-962, 3697, 3756`) to ingest pre-rendered desktop captures from the L-Connect 3 GUI process. The byte buffer travels Win32-shared-memory → `screenFrameBuffer` → JPG-encode → `PushJpg`. The lcd207 SDK itself is **not** involved in the IPC mechanism — it consumes already-rendered byte arrays. The Lancool 207 staging file's "trace this to a lcd207 SDK call" question therefore has a negative answer: lcd207 is below the IPC layer; the IPC happens entirely inside L-Connect-Service.

### Theme / graph rendering helpers

This is a host-side widget-rendering pipeline that produces a `Bitmap` for each frame, which is then JPG/PNG-encoded and uploaded via `PushImage`. It is **prior art for Phase 4's theme engine design** but is not itself part of the wire protocol.

#### Graph hierarchy

`GraphItem` (`GraphItem.cs:10`) is the abstract base for all renderable widgets. Concrete subclasses:

| Class | File | Purpose |
|---|---|---|
| `GraphImage` | `GraphImage.cs:8` | Static raster (PNG/JPG/BMP) at a position with size + opacity. Direct `Graphics.DrawImage`. |
| `GraphClock` | `GraphClock.cs:8` (extends `GraphImage`) | Analog or digital clock face. Renders hour/minute/second based on `DateTime.Now`. |
| `GraphSensor` | `GraphSensor.cs:9` | Renders a sensor value as text (CPU temp / GPU load / fan RPM / etc.) using a `FontConfig`. Sensor name → live value lookup via `M_Data.MaxVals` and `LCDController.stringDataNameValues` (`LEDController.cs:32-36`). |
| `GraphLine` | `GraphLine.cs:11` | Time-series line chart. Buffer of past samples per sensor. |
| `GraphArchBar` | `GraphArchBar.cs:8` | Arc-shaped progress bar (e.g. radial CPU-usage gauge). |
| `GraphStatuBar` | `GraphStatuBar.cs:9` | Linear horizontal/vertical progress bar with style baseline. |
| `GraphDynamicBar` | `GraphDynamicBar.cs:8` (extends `GraphStatuBar`) | Bar that adjusts colour or thickness based on the live value. |
| `GraphAnimation` | `GraphAnimation.cs:8` | Wrapper for an `.h264` clip — drives the `videoPath` field of the parent `Theme` and routes through `PlayH264` instead of being rasterized into the `Bitmap` layer. |

All concrete subclasses override `Render(Graphics g, bool isVisual, bool rebuildItem, bool CalMargin)` (`Graph*.cs` line ~50-150).

#### Theme model

`Theme` (`Theme.cs:12`) aggregates `GraphList: ObservableCollection<GraphItem>` (line 81) with a background `BgImg: Bitmap`, an optional `videoPath: string`, dimensions (`width × height` defaulting to `LCDController.Width × LCDController.Height`), front/back colours, a `name`, and three discriminator enums:

- `ThemeTypes` (`ThemeTypes.cs:3-8`) — `Template`, `ThemeInfo`, `Modulars`. Drives the file-extension on save (`.template`, `.theme`, `.modular`) — `H2Controller.cs:1266-1283`.
- `ModularTypes` (`ModularTypes.cs:3-9`) — `General`, `WireLessLcd`, `DataAndTime`, `CustomText`. Sub-discriminator for `Modulars`.
- `IsRemovable` / `isTempTheme` — distinguishes user-created vs built-in themes.

Render pipeline (per frame, host-side):

1. `LCDController.GetBitmapByTheme(theme)` allocates `Bitmap(width, height)`, draws `theme.BgImg` if set.
2. For each `GraphItem` in `theme.GraphList`, calls `item.Render(g, ...)`.
3. Sensor values are looked up via `M_Data.MaxVals: Dictionary<string, double>` (the live-sensor cache) and `stringDataNameValues` (string-typed sensor names like CPU model / GPU model). The L-Connect-Service refreshes these via `H2Controller.UpdateSensorValues(sensorValues)` and `UpdateStringDataNameValue(values)` (`H2SSDK.cs:177,182`).
4. Encode to JPG (quality 95, max 512000 B) or PNG via `Util.GetJpgBytes` / `BitmapHelper`.
5. Push via `PushJpg` / `PushPng`.

`ThemePreview.cs` is a separate render-once-to-disk path for catalog thumbnails; not a wire path.

#### Helper modules

- `BitmapHelper.cs` (1193 lines) — colour-space conversions, alpha-channel manipulation, resize / rotate / pad operations.
- `LockBitmap.cs` — fast pixel access via `BitmapData.LockBits` (a standard .NET pattern for high-FPS rendering).
- `FontConfig.cs` / `FontSet.cs` — font-family + size + style + colour bundle, with system-font enumeration helpers.
- `M_Data.cs` (490 lines) — sensor-data cache + UTF-8 multi-byte-string helpers (`stringDataNameValues` etc.). Acts as the bridge between the L-Connect-Service sensor pipeline and the renderer.
- `TransFormInfo.cs` — affine-transform descriptors (translate / rotate / scale).
- `PaddingF.cs`, `TextPaddingCalculator.cs`, `TextAlignment.cs` — text layout / metrics.
- `Contr/` subdirectory — `DrawStyle`, `GradientCircle`, `SensorInfo`, `ShapeInfo`, `StyleInfo`, plus `SenSorTypes` / `ShapTypes` / `StyleTypes` enums. These are style-bundle DTOs reused across `Graph*` items.
- `FFMpeg/` subdirectory — `FFmpegHelper`, `H264`, `H264VideoStreamEncoder`, `VideoFrameConverter`, `VideoStreamDecoder`. Wraps FFmpeg.AutoGen for video → `.h264` elementary-stream conversion ahead of `StartPlay`.

**Phase 4 design note:** the `(Theme, GraphItem*)` model is a sound starting reference for the Linux port's theme engine, but **do not** mechanically transcribe — `Graph*` classes are tightly coupled to `System.Drawing.Graphics` (GDI+) and `System.Windows.Media.Color`. A Linux port would naturally use Skia/Cairo/`tiny-skia` and need a re-design of the render trait. Reuse the *enum* set (`ThemeTypes`, `ModularTypes`, the eight graph-item variants) as the wire-compatible vocabulary for theme files; reuse the *layering* model (background → graph items → optional H264 overlay); but rewrite the renderer.

### Notable findings

1. **The "what does `lcd207.dll` actually do?" question is fully answered.** It's a four-product LCD-rendering and AIO-control SDK that wraps four transports (`HIDUsb`, `WinUsb`, `WinUsbH2`, `WinUsbH2S`, `WinUsbLed`) under one shared `CmdType` opcode space, one shared DES-CBC envelope, and one shared host-side theme-rendering pipeline. The four product families differ only in PID, panel dimensions, and which subset of `CmdType` opcodes they use. **HS II Circle and Square are byte-identical at the wire level** (only PIDs and the pump-RPM-to-PWM mapping differ). The "LCD207" filename is a misnomer — Lancool 207 is just one of four clients.

2. **The HS II per-family survey's wired-HID-transport opacity is closed.** HS II LCD does not use HID for frames — it uses WinUSB (PID 0xA021 / 0xA034) with DES-CBC-encrypted headers, exactly like Lancool 207. HID is only used for one-time SN discovery (`HIDUsb.GetHidLst`) and for the host-only "switch into desktop pixel-streaming mode" magic-byte signal (`SetMonitorMode`).

3. **lcd207 and slv3 share the same DES-CBC envelope, key, and IV.** Both use DES-CBC with key+IV = `73 6C 76 33 74 75 7A 78` ("slv3tuzx"); both place the encrypted 504-B header at output offset 0; both place the raw body at offset 512; both force trailing `0xA1 0x1A` post-encryption. lcd207's `Encrypt` (`WinUsb.cs:601-631`) is byte-for-byte identical to slv3's (`slv3.WinUsb.Encrypt`). This is **strong evidence that lcd207 was forked from slv3** and not independently written. The shared opcodes (`GetVer`, `Reboot`, `Rotate`, `BrigthSet`, `SetFrameRate`, `UpdateFireWare`, `PushJpg`, `StartPlay`, `QueryBlock`, `StopPlay`, `GetPosIndex`, `FactoryH264Test`) reinforce this. The fork added the AIO-specific opcodes (`SetWthemeIndex`, `GetH2Params`, `SyncPumpFan`, `PushRgbData`) plus `Rotate`, `DelFile`, `SetClock`, `StopClock`, `QueryDir`, `SwitchToDesktop`. Implication for the Linux port: a single shared-encryption module covers both SDKs.

4. **HS II Circle vs HS II Square wire-level differences are minimal.** A `diff WinUsbH2.cs WinUsbH2S.cs` shows: PID (0xA021 vs 0xA034), class names (`WinUsbH2` vs `WinUsbH2S`), log-string prefixes (`H2 SendAndRead` vs `H2S SendAndRead`), and the pump-RPM-to-PWM mapping LUT (H2: 6 segments covering 1600-2500 RPM; H2S: 8 segments covering 1600-3200 RPM). Nothing else. This argues for a single Rust codec serving both, parameterized by `(Pid, PumpRpmRange)`.

5. **lcd207 calls slv3 across SDKs.** `WinUsbH2.SendPumpFan` and `WinUsbH2S.SendPumpFan` call `slv3.RFController.GetAllGroup()` (`WinUsbH2.cs:1086`, `WinUsbH2S.cs:1095`) to skip wired pump-fan sync if a wireless waterblock peer is bound. This is the only behavioural cross-link: when the user has both a wired HS II AIO and a wireless waterblock paired through the LWireless hub, the wireless one wins. The Linux port will need to model this priority (probably via a `RFGroupSnapshot` shared between the two driver modules).

6. **The "MCU firmware" vs "application firmware" distinction is real.** lcd207 has two separate firmware-update paths: cmd `UpdateFireWare = 0x28` writes `/usr/data/update.app` (the embedded Linux application binary on the LCD's onboard SoC) and reboots; raw bytes 247 + 248 (undeclared in `CmdType`) do CRC-16-checksummed 4096-B chunked uploads to the AIO controller microcontroller (a separate chip). The MCU path uses `Updating = true` to gate the message-queue worker so heartbeat traffic doesn't interleave. **Phase-0 verification harness implication:** any HS II MCU FW path must NOT be invoked during normal testing — bricking risk is much higher than the application FW path because a failed MCU update can render the AIO non-responsive.

7. **`WinUsbLed` is the only family that speaks an unencrypted protocol over the same writer/reader.** UniversalScreen 8.8"'s LED-side endpoint (PID 0x8050 / VID 0x0416) bypasses the DES envelope: cmd bytes 14, 16, 17, 18, 21, 32-35, 37 (the `USB_CMD` enum, which is the **same enum as slv3**'s `USB_CMD.cs`) are sent as raw 64-B frames. The LCD-side endpoint of the same panel (PID 0xA088) uses the encrypted `CmdType` envelope inherited from `WinUsb`. Single physical board, two protocols on two USB endpoints — exactly mirroring slv3's wired-LCD-vs-RF split, except here both halves are on the same USB hub.

### Open questions / unclear from static analysis

- 🚧 **`CmdType.GetPosIndex = 201`** is declared but no caller located in the lcd207 source. Likely inherited from slv3's enum during the fork and never wired up.
- 🚧 **`CmdType.FactoryH264Test = 253`** declared, no caller. Same inheritance story.
- 🚧 **`Util.Compute(buffer, 0)` CRC algorithm** — not inspected line-by-line. Used in MCU firmware chunk trailer. Almost certainly CRC-16/MODBUS or CRC-16/CCITT; a Phase-0 verifier will need to confirm against captured bytes before implementing the MCU FW path.
- 🚧 **`WinUsb.cs` Lancool 207 path missing methods.** Lancool 207's `WinUsb` does not appear to inherit `QuerryDir` (only HS II's `WinUsbH2` / `WinUsbH2S` define it), but `LCDController` may rely on `WinUsbH2.QuerryDir` for slot-listing — needs verification at run-time. Looks like the `QueryDir` opcode is HS II-only at the host call layer even though the device firmware probably accepts it for Lancool 207 as well.
- 🚧 **`copyImageMemory` IPC source.** `HydroShiftIILCDController.copyImageMemory` is decompiled with `// Exception in: System.Boolean copyImageMemory(...)` — the L-Connect-Service decompiler choked on the implementation body. The shared-memory mechanism is almost certainly `OpenProcess` + `ReadProcessMemory` (Win32) given the `(processId, destAddress, sourceBuffer)` signature, but the protocol details (handshake, address negotiation) are not visible from this DLL. Out of scope for lcd207 — lives in L-Connect-Service.
- 🚧 **`HIDUsb.GetHidLst` SN-discovery probe payload.** The 3-byte probe `[0xAA, 0x55, 0x35]` (`HIDUsb.cs:82`) has no CmdType-style enum entry. The SN comes back at `Read.Data[2..18]` as ASCII. Origin / firmware-side handling unknown.
- 🚧 **`SetMonitorMode` magic 8 bytes** = `35 66 33 37 35 39 64 66` (`H2Controller.cs:1147`) decode as ASCII `"5f3759df"`. This is the famous "fast inverse square root" magic constant from Quake III. Cute easter-egg / signature byte sequence used as a host-authentication token to flip the device into pixel-streaming mode. Wire-format-wise it's just 8 raw HID bytes — but worth noting that the firmware probably checks for this exact value, so any Linux port replacing the protocol must ship those exact bytes.
- 🚧 **`H2SController.cs` was not read line-by-line.** The byte-level diff against `H2Controller.cs` (similar to the `WinUsbH2` vs `WinUsbH2S` diff) was assumed-symmetric; spot-check during Phase-1 implementation.
- 🚧 **`startmp4` static field on `WinUsbH2`** is shared across all H2 instances (it's `public static`); concurrent multi-device scenarios may have race conditions if a future user has two HS II Circles connected. Note as a quirk to *not* replicate in the Linux port.
- 🚧 **The `aioParamsTX[0] = 2080` initial value** in `WinUsbH2.InitDev` (`WinUsbH2.cs:476`) doesn't fall on any of the segment boundaries in the pump-RPM-to-PWM LUT. Likely an "off / not yet configured" sentinel that the LUT clamps to the 1600-2500 range on first tick. Worth verifying against device behaviour.
- 🚧 **Cross-link to slv3 SDK survey:** `slv3-sdk.md` documents `slv3.WinUsb` and `slv3.RFController`. lcd207 imports `slv3.RFController` but **does not** import `slv3.WinUsb`. The boundary is clean: lcd207 owns its own WinUSB enumeration; it only borrows slv3's RF-group state for the cross-SDK pump-fan-skip logic. This means a Phase-1 Linux implementation of lcd207 can stub out `RFController::GetAllGroup -> empty` and lose only the wireless-waterblock-suppresses-wired-pump-fan behaviour.
