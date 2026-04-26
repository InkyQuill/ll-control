## Shared protocol layer: slv3 SDK

**Source:** decompiled `slv3.dll` at `/home/inky/Development/lian-li/slv3/slv3/`
        + `slv3.models.dll` at `/home/inky/Development/lian-li/slv3.models/slv3/models/`
**What it is:** the host-side **SL Wireless** protocol library used by Lian
Li's SL Wireless transmitter dongle and the wireless-capable subset of the
device catalog. The DLL filename (`slv3.dll`) and the namespace (`slv3` /
`slv3.models`) name the **SL Wireless** product family - not "SL Version 3"
or "Strimer Light V3". Earlier writeups in this survey that speculated on
the version-number reading have been corrected.
**Status:** all citations are inferred-static (decompiled artifacts: anonymous-method names retained, some raw IL hints in `WinUsb.cs:151-161,304` etc., but the wire-relevant code is intact).
**Used by these families:**

- **SLV3LCD** — `L-Connect-Service/LConnectService/Products/SLV3LCD/SLV3LCDController.cs:190-846` (consumes `STRController.GetAllControlDevice`, `ShowAndSaveImg`, `RefreshSTRPosInfo`, `RestLcdHub`, `PlayVGroupMedia`).
- **TLV2LCD** — `L-Connect-Service/LConnectService/Products/TLV2LCD/TLV2LCDController.cs:191-847` (same call surface as SLV3LCD; both LCD families pivot through `STRController` because they enumerate as VID 0x1CBE / PID 5 or 6).
- **HydroShift II LCD** — slv3 is the SL Wireless transport for HS II's
  **Wireless** control mode (one of three operator-selectable modes; see
  `hydroshift-ii-lcd.md` "Control modes" sub-section). In Wireless mode,
  image content is shipped via `RF_SendPic` chunks (chunked through
  `MasterDevice.SendRfData`); other host writes go through the same RF
  carrier. HS II's **USB Streaming** mode goes through `lcd207.dll`
  instead (see `lcd207-sdk.md`). In addition to the wireless transport,
  slv3 also supplies the *effect/style vocabulary* via `slv3.models` enums
  (`L-Connect.Core/.../HydroShiftIILCD/CLLightingEffectsSetting.cs:1`,
  `H2LightingEffectsSetting.cs:1`, `HydroShiftIILCDLightingSetting.cs:1`,
  `HydroShiftIILCDViewLightingEffectsSetting.cs:1`) - those enum imports
  are used in both Wireless and USB Streaming modes.
- **UniversalScreen 8.8"** — slv3 owns the **lighting board** side only
  (LED ring around the panel). *No* direct `STRController` usage in the
  controller layer (`LConnect3cs/Products/UniversalScreen8p8Inch/UniversalScreen8p8InchController.cs`
  does not import slv3). slv3 reaches this product through
  `WinUsb.Get88inch_LED()` / `WinUsb.Led_Send` on VID 1046 / PID 32848
  (0x0416 / 0x8050). The **panel** side (LCD frames) goes through
  `lcd207.dll`, not slv3 (see `lcd207-sdk.md`). One physical board, two
  SDKs - this matches the OpenRGB cross-validator finding that the
  lighting board is also visible to OpenRGB at PID 0x8050.
- **LWireless hub (RF transmitter dongle, plus all wireless device
  variants)** — the SL Wireless transmitter dongle is the host-side
  endpoint for every wireless write; slv3 owns the entire transport.
  `L-Connect.Core/LConnectCore/Products/LWireless/LWirelessDevice.cs:48-196`
  consumes `RFController.SetAioThemeIndex`, `Bind`, `GetAllGroup`, `Init`,
  `SetFansRPM`, `FgSync`, `SetAllFansRPMSyncMainBoard`, `SetAioParams`,
  `SetAioPumpSpeed`, `SetH2SAioPumpSpeed`, `SwitchAioLcdWirelessMode`.
- **Lancool 207 Digital** — *not* a direct slv3 caller. `Lancool207DigitalController.cs` has no `STRController`/`RFController`/`MasterDevice` references. The "Lancool 207" path inside slv3 (`DevTypes.LC217 = 65`, `RF_217CloseWifi = 35`, `Close217Wifi(macStr)`) is the **wireless** Lancool 217 case fronted by the LWireless hub, not the wired 207. Lancool 207 Digital is owned by L-Connect.Core directly.

### Transports

slv3 wraps three wire transports plus two non-wire integrations.

| Transport class | Wire protocol | VID / PID(s) | Endpoint / signature | Buffer / chunking | Used by |
|---|---|---|---|---|---|
| `HIDUsb.cs` | HID (HidLibrary) | VID 0x1A86 (6790), PID 0x2107 (8455) — `HIDUsb.cs:13-15` | 64-byte HID reports, ReportId = 0 — `HIDUsb.cs:79-83,217-219` | 64 B in, 64 B out (`Read()` strips the leading report-ID byte and copies 64 — `HIDUsb.cs:181-196`) | "Mac map" probe of CH340-class hubs (USB-to-serial bridges acting as USB-hub identity tokens). Provides `(USB-port → MAC)` table. Cmd 0x1C (= 28) is the only one used: it asks the bridge for a 6-byte MAC payload. The MAC table is consumed by `STRController` group-mapping logic. **Not** a per-device LCD transport. |
| `WinUsb.cs` | LibUsbDotNet WinUSB / interrupt endpoints | LCD: VID 0x1CBE (7358), PID 5 or 6 — `WinUsb.cs:26-30,234`; RF receiver: VID 0x0416 (1046) PID 0x8041 (32833) **or** VID 0x1A86 (6790) PID 0xE305 (58117) — `WinUsb.cs:38,42,486`; RF sender: VID 0x0416 PID 0x8040 (32832) **or** VID 0x1A86 PID 0xE304 (58116) — `WinUsb.cs:32,40,532`; 8.8" LED side: VID 0x0416 PID 0x8050 (32848) — `WinUsb.cs:46-48,576` | `WriteEndpointID.Ep01` writer + `ReadEndpointID.Ep01` interrupt reader, 512-byte read packets — `WinUsb.cs:600-603,615-635` | LCD path: 102,400-byte buffer, encrypted 504-byte header (DES-CBC, key `slv3tuzx` = `73 6C 76 33 74 75 7A 78`) at offset 0 followed by raw image bytes at offset 512 — `WinUsb.cs:730-755,1056-1081`. RF path: 64-byte raw frames, no encryption — `WinUsb.cs:808-859`. 8.8" LED path: raw write through `Led_Send` — `WinUsb.cs:763-779` | SLV3LCD, TLV2LCD, UniversalScreen 8.8" LED side, RF transmitter/receiver dongles for the LWireless hub. |
| `RFController.cs` (logical) over `WinUsb` (physical) | Lian-Li proprietary 2.4 GHz RF, encapsulated in 64-byte WinUSB frames | (uses RF sender dongle above) | `MasterDevice.SendRfData` builds a 64-byte USB frame: `[0]=0x10 (Usb_SendRf)`, `[1]=fragment_seq`, `[2]=channel`, `[3]=rx_type`, `[4..63]=60-byte RF-payload chunk` — `MasterDevice.cs:945-968`. RF-level payload (240 B or longer) is fragmented into 60-B chunks. | `MasterDevice.SyncMasterClock`, `SendAioInfo`, `SendPicDataOnce`, `Update`, `QuerryMasterMac`, etc. | LWireless hub (all wireless devices: Strimer wireless, TL/SLV3 wireless fans, wireless HS II LCD, Lancool 217, V150, Led88, etc.). |
| `OpenRGBController.cs` | OpenRGB SDK (TCP, "ORGB" magic) | TCP loopback, default port **6742** — `OpenRGBController.cs:187,231,262,328` | TCP socket; the SDK launches `OpenRGB.exe` (`openRgbPath`), connects to 127.0.0.1:6742, then speaks the OpenRGB SDK protocol (request/response with magic+device-id+command-id+length+payload). | One-shot RGB sync to third-party RGB devices (motherboard, DRAM, GPU, keyboard, etc.). | Universal RGB sync feature exposed via `RFController.SetOpenRgbUIEffect` (`RFController.cs:1153`). |
| `UdpServer.cs` | UDP server, port **61447** — `UdpServer.cs:24` | localhost UDP | 64-byte command frames; `UdpCmdTypes` enum: `GetLst=101, SetLightEffect=102, SaveEffect=103, StartRecord=104, StopRecord=105` — `UdpServer.cs:13-20` | Local IPC for in-process effect record/replay (probably the L-Connect Editor → service bridge). Not a device-facing wire format. |

`MasterDev.cs` is a 12-line stub (a marker / no behaviour). `MasterDevice.cs` (1534 lines) is the real RF orchestrator.

### USB command table (USB_CMD.cs)

Citations: `USB_CMD.cs:5-14`. The values are **flat USB-frame opcodes** sent over the `WinUsb` RF-dongle transport — they are *not* OS-USB-spec commands. Each is the leading byte of a 64-B frame. (LCD-side WinUSB uses the encrypted `CmdType` enum instead; see next section.)

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| 0x0E (14) | `Usb_SendRfLong` | host → dongle | (RGB effect machinery) | `USB_CMD.cs:14`, `RgbEffect.cs:64465,64897` | Long-form RF-payload variant. Used inside the bulk RGB-sync codepath of the giant `RgbEffect.cs` (88k LOC, mostly per-effect frame tables). |
| 0x10 (16) | `Usb_SendRf` | host → dongle | every RF write | `USB_CMD.cs:8`, `MasterDevice.cs:955` | Standard RF-fragment carrier. `[1]` = fragment seq, `[2]` = channel, `[3]` = rx_type, `[4..63]` = 60 B RF payload. |
| 0x11 (17) | `USB_GetMac` | host → dongle, dongle → host | `MasterDevice.QuerryMasterMac` | `USB_CMD.cs:7`, `MasterDevice.cs:209-238` | Reads the master dongle's own 6-byte MAC + 4-byte tmos clock + 2-byte version. `[1]=channel`. Response: `[1..6]=mac`, `[7..10]=tmos`, `[11..12]=ver`. |
| 0x12 (18) | `USB_SendHeart` | host → dongle | (heartbeat, not seen actively used) | `USB_CMD.cs:6` | Declared but the only place `array[0] = 18` appears in `MasterDevice.cs` is the **RF-payload** byte (where 18 = `RF_Select`), not the USB layer. Likely vestigial / firmware-side heartbeat. |
| 0x15 (21) | `USB_ResetAnother` | host → dongle | `MasterDevice.ResetRx` / `ResetTx` | `USB_CMD.cs:5`, `MasterDevice.cs:169,192` | Tells the local dongle to reset its peer (Tx asks Rx to reset; Rx asks Tx to reset). 64 B frame, only `[0]` set. |
| 0x16 (22) | (no enum entry; `CloseRecUsb` / `CloseSenderUsb`) | host → dongle | `MasterDevice.cs:973,981` | Close-current-USB-session. Not in `USB_CMD.cs`; raw byte. |
| 0x20 (32) | `Usb_FwErase` | host → dongle | `MasterDevice.Update` | `USB_CMD.cs:9`, `MasterDevice.cs:1453` | Step 1 of dongle firmware update. Response: `[0]=0x20`, `[1]=1` for ack. |
| 0x21 (33) | `Usb_FwUpload` | host → dongle | `MasterDevice.Update` | `USB_CMD.cs:10`, `MasterDevice.cs:1461-1487` | Step 2 of dongle firmware update. `[1]` = end-of-page flag (1 when last chunk in a 4096-B page or last chunk overall), `[2]` = chunk length, `[4..]` = up to 60 B firmware bytes. Each chunk acked with `[0]=0x21,[1]=1`. End-of-page (every 4080 B + 16 B → 4096 B aligned) sleeps 50 ms. |
| 0x22 (34) | `Usb_FwVerify` | host → dongle | (declared, not seen used) | `USB_CMD.cs:11` | Defined in enum; no call site located in `MasterDevice.cs`. |
| 0x23 (35) | `Usb_FwReboot` | host → dongle | `MasterDevice.Update` / `ResetAll` | `USB_CMD.cs:12`, `MasterDevice.cs:1504,1514` | Final step — dongle reboot into new firmware. Also used by `ResetAll`. |
| 0x25 (37) | `Usb_FwGetInfo` | host → dongle | (declared, not seen actively used) | `USB_CMD.cs:13` | Read firmware-info. No write-time call site in inspected files. |
| 0x1C (28) | (no enum entry; HID MAC probe) | host → CH340 hub | `HIDUsb.GetMac` | `HIDUsb.cs:79` | Only the HID transport uses 28; raw byte. Not in `USB_CMD.cs`. |

### LCD WinUSB command table (`slv3.models.CmdType`)

This is the **separate** opcode space used by the encrypted 504-B header on the LCD WinUSB transport (SLV3LCD, TLV2LCD, 8.8"). Citations: `CmdType.cs:5-17`. The opcode lives at `header[0]` *before* DES-CBC encryption.

| Cmd byte | Name | Direction | Used by | Citation | Notes |
|---|---|---|---|---|---|
| 0x0A (10) | `GetVer` | host → device, response | `WinUsb.GetVer`, `SendHeartBeat` | `CmdType.cs:5`, `WinUsb.cs:1219-1228,1238-1257` | Plain reply: `[0]=0x0A`, `[8..40]` = UTF-8 version string, NUL-trimmed. |
| 0x0B (11) | `Reboot` | host → device | `WinUsb.Reboot` | `CmdType.cs:6`, `WinUsb.cs:1206-1212` | |
| 0x0D (13) | `Rotate` | host → device | `STRController.Rotate` → `WinUsb.Send(CmdType.Rotate, byte)` | `CmdType.cs:7`, `STRController.cs:1337,1346`, `WinUsb.cs:1030` | `header[8]` = rotation enum (0–3). |
| 0x0E (14) | `BrigthSet` | host → device | `STRController.SetBright` (line 1413, not shown) | `CmdType.cs:8` | Single-byte payload at `header[8]`. |
| 0x0F (15) | `SetFrameRate` | host → device | `WinUsb.SetFrameRate` (always called with 120) | `CmdType.cs:9`, `WinUsb.cs:637,1233-1236` | Always sends 120 fps regardless of argument — `Send(CmdType.SetFrameRate, 120)` ignores the parameter. |
| 0x28 (40) | `UpdateFireWare` | host → device | `WinUsb.UpdateFile` (firmware **and** boot.jpg) | `CmdType.cs:10`, `WinUsb.cs:1083-1204` | **Repurposed as a generic filesystem-write opcode.** Header layout: `[0]=40`, `[8..11]=BE filename length`, `[12..15]=BE file body length`, `[16..]=ASCII filename`. Body at offset 512 of the outer 102,400-B buffer. Targets: `/usr/data/update.app` (FW), `/usr/data/boot.jpg` (stored boot logo). |
| 0x29 (41) | `StopClock` | host → device | (declared) | `CmdType.cs:11` | Not seen used in `WinUsb.cs` / `STRController.cs`. |
| 0x65 (101) | `PushJpg` | host → device | every transient frame | `CmdType.cs:12`, `WinUsb.cs:718-728,742`, `STRController.cs:186,263,321,1305,1924,1978,...` | Transient frame upload. Header `[8..11]` = BE body length, body at offset 512. **No persistence.** |
| 0x79 (121) | `StartPlay` | host → device | (declared) | `CmdType.cs:13` | Not seen called in inspected scope; likely on-device animation start. |
| 0x7A (122) | `QueryBlock` | host → device | (declared) | `CmdType.cs:14` | |
| 0x7B (123) | `StopPlay` | host → device | (declared) | `CmdType.cs:15` | |
| 0xC9 (201) | `GetPosIndex` | host → device, response | `WinUsb.GetPosIndex` | `CmdType.cs:16`, `WinUsb.cs:1259-1287` | Returns group index; reply `[0]=0xC9`, `[8]=group_index`. |
| 0xFD (253) | `FactoryH264Test` | host → device | `STRController.FactoryH264Test` | `CmdType.cs:17`, `STRController.cs:1391` | Factory diagnostic. |

### RF command table (RF_CMD.cs)

Citations: `RF_CMD.cs:5-19`. Each value is the **second byte** of the RF payload (the first byte is `0x12` = `RF_Select` in most packets, identifying it as a per-device-targeted RF frame; `array[1]` then carries the actual RF command). Tagged by `RF_*` consumers in `MasterDevice.cs` and `RFController.cs`.

| Cmd byte | Name | Used by | Citation | Notes |
|---|---|---|---|---|
| 0x10 (16) | `RF_Bind` | `RFController.Bind` / `BindAll` | `RF_CMD.cs:5`, `RFController.cs:1702,1794` | Pair a wireless device to the master MAC. |
| 0x12 (18) | `RF_Select` | every per-device write | `RF_CMD.cs:6`, `MasterDevice.cs:337,362,532,569,587,...,939` | Outermost RF "envelope" byte: `[0]=0x12`, `[1]=sub_cmd`, `[2..7]=device_mac`, `[8..13]=master_mac`, `[14]=target_rx_type`, `[15]=target_channel`, `[17]=cmd_seq`, `[18..]=payload`. |
| 0x13 (19) | `RF_Print` | (declared; debug print? no caller located) | `RF_CMD.cs:7` | Likely device-side log echo. |
| 0x14 (20) | `RF_ClockSync` | `MasterDevice.SyncMasterClock` | `RF_CMD.cs:8`, `MasterDevice.cs:939-942` | Broadcast: `[0]=0x12`, `[1]=20`, `[8..13]=master_mac`, `SendRfData(MasterChannel, 0xFF=broadcast, ..., syncClock=true)`. |
| 0x15 (21) | `RF_SaveCfg` | `MasterDevice.SaveConfig` (line 529) | `RF_CMD.cs:9` | Asks devices to flush in-RAM config to flash. |
| 0x16 (22) | `RF_RebootLcd` | `RFController.RebootLcdGroup` | `RF_CMD.cs:10`, `RFController.cs:1592` | |
| 0x17 (23) | `RF_UpdatePre` | `MasterDevice.Update` (sub-device FW) | `RF_CMD.cs:11`, `RFController.cs:1940` | Pre-flight before sub-device firmware streaming. |
| 0x18 (24) | `RF_UpdateData` | sub-device FW upload chunks | `RF_CMD.cs:12` | Chunk carrier for wireless device firmware update. |
| 0x19 (25) | `RF_SwitchWirelessTheme` | `RFController.SwitchAioLcdWirelessMode` | `RF_CMD.cs:13`, `RFController.cs:1607` | Toggles AIO LCD into the on-device "wireless theme" mode (i.e. the AIO renders frames internally instead of receiving them over RF). |
| 0x20 (32) | `RF_RgbSync` | `MasterDevice.SyncRgbData` | `RF_CMD.cs:14`, `MasterDevice.cs:831` | Bulk RGB-frame sync to LED-strip / fan / strimer wireless devices. |
| 0x21 (33) | `RF_UpdateAio` | `MasterDevice.SendAioInfo` | `RF_CMD.cs:15`, `MasterDevice.cs:362` | Sends 32-B `aio_param` block (CPU temp, GPU temp, fan speed, pump speed, brightness, rotation, three colours, etc. — see `AioParams.cs`). |
| 0x22 (34) | `RF_SendPic` | `MasterDevice.SendPicDataOnce` (called from `SendAioInfo` image branch) | `RF_CMD.cs:16`, `MasterDevice.cs:337,386,391` | **Wireless AIO LCD image upload.** See "Frame upload mechanism" §3 below. |
| 0x23 (35) | `RF_217CloseWifi` | `RFController.Close217Wifi` | `RF_CMD.cs:17`, `RFController.cs:174` | Lancool 217-specific: switch off the case's built-in WiFi receiver. |
| 0x24 (36) | `RF_V150MBSyncSwitch` | `RFController.MBSyncSwitch` | `RF_CMD.cs:18`, `RFController.cs:191` | V150-specific: enable/disable motherboard sync. |
| 0x26 (38) | `RF_V150LightSyncSwitch` | `RFController.LightSwitch` | `RF_CMD.cs:19`, `RFController.cs:209` | V150-specific: enable/disable lighting. |

### Sub-device RF command table (`slv3.models.RFCmdType`)

`RFCmdType` (`RFCmdType.cs:3-21`) is the **third-level** opcode used inside specific sub-protocols (fans, AIO heads, etc.). It is the byte-1 value when the outer `RF_Select` payload is targeting per-device fan/RGB control rather than master-level operations. Values 1–3 are queries (group-num / RPM / err); 17–29 are sets (PWM, FG sync, all-RGB, RGB, LCD reset, LE direction, group select/unselect, ordering, unbind, bind, set-mode, exit-mode).

### Frame upload mechanism (cmd-15 hypothesis answer)

**This resolves the LCD-frame-upload opacity flagged in the SLV3LCD, HydroShift II LCD, and UniversalScreen surveys.**

slv3 has *two distinct* image-upload pipelines depending on whether the target is a wired LCD or a wireless AIO. Both have an explicit **transient vs stored split**, but they use different opcodes than Galahad II Vision LCDV2 (cmd 14/15) and TLLCD (`WriteJPG` / `WriteBootJPG`).

#### Pipeline A: wired LCD (SLV3LCD / TLV2LCD)

1. **Entry point.** `STRController.ShowAndSaveImg(string filePath)` (`STRController.cs:1303-1318`).
2. **Transient send.** `STRController.PushImg(filePath)` → `Util.GetJpgBytes(ResizeImage(bmp, 400, 400), quality=95)` → `winUsb.PushJpg(jpgBytes)` → `WinUsb.Send(CmdType.PushJpg=0x65, bytes, IsImg=true)` (`STRController.cs:1908-1939`, `WinUsb.cs:718-728,730-761`).
3. **Wire format of `PushJpg`.** `WinUsb.Send(CmdType, byte[], IsImg)`:
   1. Allocate 102,400-B output buffer (`WinUsb.cs:737`).
   2. Build 504-B header via `GetBaseCmdBuf(0x65, out timestamp)`: `[0]=0x65 (PushJpg)`, `[2]=0x1A`, `[3]=0x6D` (constant magic), `[4..7]=little-endian uint32 ms-since-app-start` (`WinUsb.cs:1044-1054`).
   3. Encode body length BE into header: `[8..11] = jpgBytes.Length` (`WinUsb.cs:745-748`).
   4. `Buffer.BlockCopy(jpgBytes, 0, output, 512, jpgBytes.Length)` — body sits at offset 512 (`WinUsb.cs:749`).
   5. **`Encrypt(header)`** with DES-CBC, key = `73 6C 76 33 74 75 7A 78` ("slv3tuzx"), IV = same as key (`WinUsb.cs:1056-1081`). DES output is 8-byte aligned, so 504 B → 512 B (padded) and copies into `output[0..512]`.
   6. `writer.Write(output, 500ms_timeout)` over WinUSB EP1 OUT.
4. **Stored-logo send (delayed, conditional).** `ShowAndSaveImg` schedules a Task that sleeps 5000 ms, then if no newer `PushImg` has happened in those 5 s, calls `UploadLogo(lastPath)` → `winUsb.UpdateFile(MemoryStream, "/usr/data/boot.jpg")` (`STRController.cs:1308-1316,1282-1300`).
5. **Wire format of `UpdateFile`.** Different opcode: `GetBaseCmdBuf(0x28, ...)` (`UpdateFireWare = 40`). Header layout (`WinUsb.cs:1167-1204`): `[0]=0x28`, `[8..11]=BE filename_len`, `[12..15]=BE body_len`, `[16..16+filename_len]=ASCII filename`. Body at offset 512. Same DES-CBC encryption as `PushJpg`. Targets are filesystem paths on the LCD's embedded Linux: `/usr/data/update.app` (firmware), `/usr/data/boot.jpg` (stored boot logo). After firmware writes only, also calls `Reboot()`.

#### Pipeline B: wireless AIO LCD (over RF, no stored variant)

1. **Entry point.** `RFController.SendAioPic(macstr, byte[] imgData)` (`RFController.cs:723`).
2. **Internal queueing.** Sets `rfd.imageData = imgData` and `rfd.aio_send_image = true`. The 1-Hz poll in `MasterDevice.Run` (`MasterDevice.cs:267-291`) triggers `SendAioInfo()`.
3. **Chunking.** `SendAioInfo` (`MasterDevice.cs:351-403`): splits `imageData` into 220-B chunks; for each chunk calls `SendPicDataOnce(rfd, chunk_index, chunk_data)`. Final chunk is sent with `chunk_index = 0xFF` and the first two bytes carry **BE total-image-length** as a "we're done" sentinel.
4. **Wire format of `SendPicDataOnce`.** Builds a 240-B RF payload: `[0]=0x12 (RF_Select)`, `[1]=0x22 (RF_SendPic)`, `[2..7]=device_mac`, `[8..13]=master_mac`, `[14]=target_rx_type`, `[15]=target_channel`, `[17]=cmd_seq`, `[18]=chunk_index_n`, `[19..238]=220-B image chunk` (`MasterDevice.cs:331-348`). The retransmit loop spins until `rfd.cmd_seq` echoes `targe_cmd_seq` (i.e. ack-and-retry per chunk). Then `SendRfData(channel, rx, payload)` fragments into 60-B WinUSB chunks under cmd `0x10 Usb_SendRf`.
5. **No stored variant.** Pipeline B has no `boot.jpg` analogue at the slv3 level — wireless AIO LCDs persist their own state via `RF_SaveCfg` (cmd 0x15) on a periodic schedule (`MasterDevice.CheckSaveConfig`, `MasterDevice.cs:321-329`), not via a separate stored-frame upload.

#### Cmd-15-hypothesis answer

**Yes**, slv3 has a transient-vs-stored split for the wired-LCD families, but it uses a *different* mechanism than the Galahad II Vision LCDV2 / TLLCD pattern:

- **Transient frame** = `CmdType.PushJpg = 0x65` (101). Every frame for animation / video / sensor display goes here. No persistence.
- **Stored boot logo** = `CmdType.UpdateFireWare = 0x28` (40), with target filename `/usr/data/boot.jpg`. *Same opcode as firmware update, distinguished by filename in header `[16..]`*. This is the slv3 equivalent of LCDV2 cmd 15 / TLLCD `WriteBootJPG`.
- Trigger heuristic: 5 s of inactivity after the most recent `PushJpg` triggers one auto-`UpdateFile("/usr/data/boot.jpg")` per session (`STRController.cs:1308-1316`). The user does **not** explicitly choose to save — slv3 does it for them.

Wireless AIO LCDs (Pipeline B) have **no stored variant**: only transient `RF_SendPic` chunks; persistence happens via per-device on-board flash on `RF_SaveCfg`.

### Master-device abstraction

- `MasterDev.cs` is a 12-line stub (`MasterDev.cs:1-12`); it is *not* the device-management surface. Naming probably came from the C# class hierarchy planning.
- `MasterDevice.cs` (1534 lines) is the real model. A `MasterDevice` represents the **RF receiver/sender pair** (the two USB dongles plugged into the host), *not* an end device. It exposes:
  - `RFReceiver: WinUsb`, `RFSender: WinUsb` — opened in `MasterInite()` via `WinUsb.GetRFReciver()` / `GetRFSender()`.
  - `MasterMacAddr: byte[6]`, `MasterMacAddrStr: string` — the receiver dongle's own MAC, learned via cmd 0x11 `USB_GetMac`.
  - `MasterChannel: byte = 8` — RF channel; can be overridden via `GetChannel()` config.
  - `rfList: List<RfDevice>`, `masterList: ObservableCollection<RfDevice>` — bound peers and discovered foreign masters (channel-conflict detection).
  - `SysClock: long`, `time_tmos: long` — synced master clock used for RF timing alignment.
  - `Run()` — the 100-ms-cadence orchestrator that calls `RefreshList`, `SyncPwm`, `SyncRgbData`, `SendAioInfo`, `SyncMasterClock`, `CheckChannelConflict`, `SyncControlInfo`, `CheckUpdate`.

`RfDevice` (`RfDevice.cs:16-`, 1462 lines) is the per-peer model: `MacStr`, `MasterMacStr`, `BindStatus`, `RecType[]: DevTypes[4]`, `BindLcd[]: bool[4]`, `DevSize[]: int[4]`, `LcdGroup`, `Speeds`, `PWMS`, `aio_param: byte[32]`, `imageData: byte[]`, `targe_cmd_seq` / `cmd_seq` (RF ack-seq tracking).

`STRController` (`STRController.cs:19-`, 3135 lines) is a parallel device-management layer for **wired LCDs** (SLV3LCD / TLV2LCD). Static `Init(cachePath, setPath, assetDir, callback)` enumerates VID 0x1CBE PID 5 / 6 devices and creates one `STRController` per device with its own `WinUsb` handle. Group-mapping (`GroupingMap`, `LcdGroup`, `vGroupIndex`) ties the wired LCDs to the wireless RF fan groups they live on top of.

Callers obtain handles via:
- Wired LCDs: `STRController.GetAllControlDevice()` → static `List<STRController>`.
- Wireless devices: `RFController.GetAllGroup()` / `RFController.GetFansInfo(macStr)` → returns `RfDevice` from `MasterDevice.RFList`.

### slv3.models contents

44 `.cs` files (1002 lines total). All are wire-format DTOs and enum maps. Highlights:

- **Per-family RGB effect ID enums**: `Slv3Effects` (25 effects), `H2Effects`, `CLEffects`, `LC217Effects`, `SLINFEffects`, `StrEffects`, `StrSingleEffects`, `TLEffects`, `UIEffects`. These are byte values written into the RF payload and into `LightEffectSet` configs.
- **`LightEffectSet`** — generic light-effect carrier (effect-id, speed, direction, brightness, primary/secondary colours).
- **`AioParams`** (`AioParams.cs:5-69`) — 32-B AIO display params struct: per-CPU/GPU temp/load enables, fan speed, pump speed, loop interval, three colours (str / val / unit), brightness, rotation. Wire-formatted into `rf.aio_param[32]` and emitted via `RF_UpdateAio`.
- **`StyleInfo`** — six `Color` fields (Color1, Color2, BgColor, MainFontColor, FontTopColor, FontBottomColor) — palette for built-in themes.
- **`SensorInfo`**, **`SensorVideoInfo`**, **`SensorColors`**, **`SenSorTypes`** — sensor display config (CPU temp / GPU temp / etc.).
- **`DevTypes`** — `Strimer=1, WaterBlock=10, WaterBlock2=11, SLV3Fan=20, TLV2Fan=28, SLINF=36, RL120=40, CLV1=41, LC217=65, V150=66, Led88=88, OpenRgbDev=99` — the per-device-type discriminator that drives cmd selection.
- **`USBStatus`** (`UP, DOWN, REMOVED`), **`OPStatus`** (`Success, Fail, FileInUse, TypeErr`), **`HResult`**, **`ErrorCodes`**, **`SetValStatus`**, **`BindStatusType`**, **`MasterUpdateTypes`** (`TX, RX, ...`).
- **`WinDevice`**, **`WinDeviceExt`**, **`WinDeviceType`** — generic USB-device descriptors used between the SDK and L-Connect 3 service.
- **`SlvDevice`**, **`BindDevice`**, **`GroupingSet`**, **`RFGroupSortSet`** — bind / group persistence schemas (JSON-serialized to local config).
- **`RFEffectDirType`**, **`RFBrightnessType`**, **`RFLampEffectSpeedType`**, **`EffectDirections`**, **`EffectScopes`**, **`StyleTypes`**, **`RgbEffectTypes`** — small enums driving cross-device RGB config.
- **`WifiReceiver`** — Lancool 217 wireless-receiver descriptor (110 lines).
- **`CmdType`**, **`RFCmdType`** — already covered above.

These are the types L-Connect 3 imports via `using slv3.models;` even when it doesn't pull in `slv3` itself — they're the shared *vocabulary* even when the *transport* is different (e.g. HS II LCD wired path uses these enums but its own HID stack).

### Notable findings

1. **The "cmd 15" question for slv3 families is answered.** Wired LCDs (SLV3LCD / TLV2LCD) have transient = `PushJpg=0x65` and stored = `UpdateFireWare=0x28` with filename `/usr/data/boot.jpg`. Wireless AIO LCDs have transient only (`RF_SendPic=0x22` chunks under `RF_Select=0x12`) — persistence is on-device via `RF_SaveCfg=0x15`. The "stored" opcode is *not* a separate opcode but a reuse of the firmware-update opcode parameterized by filename — a meaningful difference from LCDV2 / TLLCD that the Linux port should preserve.

2. **DES-CBC encryption of the LCD command header is novel.** Every WinUSB write to SLV3LCD / TLV2LCD / 8.8" (LED) has its 504-B command header DES-CBC-encrypted with key `slv3tuzx` and IV equal to key. The image body at offset 512 is **not** encrypted. None of the other surveys flagged this. The Linux port for these families will need a `des` crate (or equivalent). Key + IV are static and shipped in plaintext in the DLL, so this is anti-tamper signalling rather than confidentiality.

3. **Dual-VID dongles.** RF receiver/sender each have *two* VID/PID variants. Older dongles: VID 0x0416 (1046, "Winbond") with PIDs 0x8040/0x8041/0x8050. Newer: VID 0x1A86 (6790, "QinHeng/CH340") with PIDs 0xE304/0xE305. `WinUsb.GetRFReciver()` / `GetRFSender()` (`WinUsb.cs:476-566`) opens whichever it finds. The Linux udev rule will need both pairs.

4. **OpenRGB is a first-class transport, not a workaround.** `OpenRGBController.cs` is 1599 LOC, bundles the OpenRGB SDK protocol client, and `RFController.SetOpenRgbUIEffect` (`RFController.cs:1153`) exposes it as just another RGB sync target. Lian-Li ships their own OpenRGB invocation. `DevTypes.OpenRgbDev=99` makes it a real device class.

5. **HID transport is *not* what we expected.** The HID path (VID 0x1A86 / PID 0x2107) is *not* a per-device LCD transport — it's a CH340-class USB hub MAC probe (cmd 0x1C → 6-byte MAC reply). It exists purely to identify which physical USB port a given LCD is plugged into, so wired LCDs can be group-mapped to RF fan groups by hub topology. This means **none of the slv3-using families speak HID** — they all speak WinUSB (LCD) or WinUSB-encapsulated RF (wireless).

6. **`SendRfData` is the universal RF chunker.** Every RF write (master clock, AIO params, AIO image, RGB sync, fan PWM, all 15 RF commands) is fragmented through `MasterDevice.SendRfData` into 64-B WinUSB frames with cmd `0x10 Usb_SendRf`, 60-B payload, 1-B fragment seq, channel + rx_type. This is a clean abstraction for the Linux port: implement one chunker, dispatch all 15 RF cmds through it.

7. **`MasterDev.cs` and `MasterDevice.cs` are unrelated despite the name.** `MasterDev.cs` is a 12-line stub. The real master abstraction is `MasterDevice.cs`. This is a classic decompilation artifact that suggests `MasterDev` was an obsolete planning class never deleted.

### Open questions / unclear from static analysis

- 🚧 **CmdType `StartPlay=121` / `StopPlay=123` / `QueryBlock=122`** — declared but no call sites found in `WinUsb.cs` / `STRController.cs` within the 3.1k inspected lines. May be in `RgbEffect.cs` (88k LOC, not exhaustively searched). Likely on-device animation playback control, but unconfirmed.
- 🚧 **Cmd `Usb_SendHeart=18`** declared in `USB_CMD.cs` but no USB-layer caller found. The collisions with RF-payload byte 18 (=`RF_Select`) make it hard to grep. Probably reserved or firmware-side.
- 🚧 **Cmd `Usb_FwVerify=34`** declared but no caller in `MasterDevice.Update` (which uses 32→33→35 only). Either obsolete or used in a code path not covered by `Update()`.
- 🚧 **Status of the encrypted header on RF transport** — the RF transport (`RfSend`) does **not** call `Encrypt`; only the LCD WinUSB transport does. Confirmed for read paths inspected, but worth re-verifying when implementing.
- 🚧 **`SaveConfig` (`MasterDevice.cs:529`) RF cmd byte** — references `RF_SaveCfg=21`, but the actual `array[1]` byte at the call site needs spot-check (line not inspected). Assumption rests on the enum mapping.
- 🚧 **8.8" LED transport (`Get88inch_LED`, `Led_Send`, `Led_Read`)** uses *unencrypted* raw writes — a different protocol from the LCD path on the same board. No opcode table located; the L-Connect.Core 8.8" controller is the authoritative reference for that side.
- 🚧 **`OpenRGBController.cs` request encoding** — only the connection setup (port 6742) was confirmed. The actual OpenRGB SDK protocol message structure (4-B magic "ORGB" + 4-B device id + 4-B cmd id + 4-B length + payload) was not verified line-by-line in this survey but is the documented OpenRGB SDK wire format.
- 🚧 **HS II LCD wired path opcodes** — confirmed that L-Connect.Core uses `slv3.models` enums but **not** `slv3.WinUsb` for HS II LCD's HID transport. So slv3 unlocks the *effect/style vocabulary* for HS II LCD but **not** the HID frame format. The HS II LCD per-family staging file's HID-frame opacity is **not** closed by this survey — it's a separate L-Connect.Core HID stack.
- 🚧 **UniversalScreen 8.8" panel-side transport** — same situation. slv3 only owns the LED side (`Get88inch_LED`). The panel image-frame transport is in L-Connect.Core's WinUSB stack, not here.
