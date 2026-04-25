# `LConnectCore.Crypto.CryptoHelper` — usage map

**Status (post Phase 0b investigation):** transport-irrelevant. The handshake
(`Galahad2VisionDevice.SendGetHandshakeInfo` -> cmd 129) works unencrypted on
real hardware as verified in `docs/protocol/captures/2026-04-25-handshake-live.txt`.
This rules out `CryptoHelper` participating in `ACommandPacket` framing.

## Methodology

The task spec scoped the search to the shared library:

```bash
cd /home/inky/Development/lian-li/L-Connect.Core/LConnectCore && \
  grep -rn "CryptoHelper\." . --include='*.cs' | grep -v 'Crypto/CryptoHelper.cs'
```

This returned **zero matches** — `CryptoHelper` has no callers anywhere inside
`LConnectCore/`. Out of caution, the search was widened to the entire
`/home/inky/Development/lian-li/` tree (covering `L-Connect3/`, `L-Connect-Service/`,
`L-Connect Editor/`, and `L-Connect-Service-Watcher/`):

```bash
find /home/inky/Development/lian-li -name "*.cs" | xargs grep -l "CryptoHelper"
```

That surfaced exactly one external caller, in `L-Connect-Service`. No callers
of the related `EncryptedPacket` value type exist anywhere outside its own file
(further confirming `CryptoHelper.Encrypt` is the sole entry point and it has
exactly one consumer).

Each call site below has been opened and read in context (±20 lines) before
categorisation.

## Callers (one row per call site)

| File:line | Caller | Category |
|---|---|---|
| `L-Connect-Service/LConnectService/Products/TLFan/TLFanController.cs:571` | `TLFanController.handleHandshakeInfoRecord` (dispatched via `HandleRequestData` for IPC `RequestType == 9`) | License / activation (diagnostic telemetry export) |

That's the entire list. Inside `LConnectCore/` itself: zero callers.

## Categories

### Theme/profile bundles

None observed. Theme and profile JSON files in this codebase are persisted
unencrypted via `LConnectCore/JsonFileSettings.cs` and the per-product
`SettingType` enums. No `CryptoHelper` involvement.

### Firmware upload

None observed. The decompiled tree contains no caller that wraps a firmware
blob through `CryptoHelper.Encrypt` before handing it to `Cmd::UploadFirmware`
(11) or `Cmd::ActiveFirmware` (10). If firmware images are pre-encrypted, that
encryption happens at build time on Lian-Li's side and ships as already-cooked
bytes inside `Resources` or a downloaded `.bin`; the runtime helper is not on
that path. Phase 0d will revisit this when a `Cmd::UploadFirmware` codec is
added — at that point we'll know empirically from the on-wire bytes whether the
firmware payload is plaintext or pre-encrypted.

### License / activation

`L-Connect-Service/LConnectService/Products/TLFan/TLFanController.cs:571` —
`handleHandshakeInfoRecord` is the IPC handler invoked when a client sends
`RequestData.Query["type"] == "9"` to the Windows service. It serialises the
last 30 buffered raw HID handshake responses (`HandshakeInfoRecord[] {
Data: byte[], RecordTimeUtc: DateTime }`) to JSON via `System.Text.Json`,
then wraps the JSON in a `CryptoHelper.Encrypt(...)` `EncryptedPacket`
(AES-256 session key, RSA-encrypted with Lian-Li's bundled public key).

Because the asymmetric half is RSA-public-only (the private key is not in the
binary), the resulting ciphertext can only be decrypted by Lian-Li themselves.
This is a classic **phone-home diagnostic export** pattern: the consumer is
either a Lian-Li support tool that uploads these blobs, or a "send diagnostics"
button in the WPF app that POSTs them to a Lian-Li-controlled endpoint. It is
not a control-flow gate — the payload runs *after* the HID handshake has
already succeeded and read 30 samples, and nothing in the device-control
path depends on the encrypted output. From the Linux client's perspective this
is pure outbound telemetry that we have no obligation to reproduce.

(Categorised under "License / activation" rather than its own "telemetry"
bucket because the cryptographic shape — RSA-pinned-pubkey wrapping an AES
session-encrypted JSON blob, with only Lian-Li holding the decrypt key — is
the same activation/upload pattern the helper was designed for, and the four
buckets in the task spec do not include a separate telemetry category.)

### HID transport

**None observed.** This is the load-bearing finding for Phase 0b/0c.

No code path in the decompiled tree wraps an `ACommandPacket` or
`BCommandPacket` payload through `CryptoHelper.Encrypt` (or any other
crypto API) before calling `HidDevice.Write`. The single caller above is
in the opposite direction (encrypting data already received from the device)
and lives behind a service IPC dispatcher, not on the HID write path.

This static-analysis result is consistent with — and corroborates — Phase 0a's
empirical proof: the unencrypted `GetHandshakeInfo` (cmd 129) succeeded on real
hardware end-to-end (write + read) as captured in
`docs/protocol/captures/2026-04-25-handshake-live.txt`. If transport encryption
existed, an unencrypted handshake would have been rejected (or returned
nonsense bytes), and a transport-level caller would necessarily appear here.

## Helper internals

`L-Connect.Core/LConnectCore/Crypto/CryptoHelper.cs` (read for verification):

- `Encrypt(byte[] plain) -> EncryptedPacket` — generates a fresh AES-256 key
  + IV per call (`Aes.Create(); GenerateKey(); GenerateIV();`), AES-encrypts
  the plaintext via a `CryptoStream`, then RSA-encrypts the AES key and IV
  with Lian-Li's bundled public key.
- `Decrypt(...)` — **not present in the decompilation.** The class only
  exposes encryption. This is consistent with the helper being
  upload-only/phone-home-only: clients seal data they will never need to
  re-open themselves.
- `init()` — lazily loads the RSA public key from `Resources.PublicKey`,
  which is GZip-compressed XML (`decompressRSAXmlKey`). No private-key
  material is bundled.
- `EncryptedPacket` (`L-Connect.Core/LConnectCore/Crypto/EncryptedPacket.cs`)
  is a plain DTO with three properties: `Data` (AES ciphertext),
  `Key` (RSA-encrypted AES key), `IV` (RSA-encrypted IV).

Implication of "no `Decrypt`": even the firmware-upload theory now looks
weaker. If the runtime needed to decrypt a firmware image before pushing
it to the device, `CryptoHelper.Decrypt` would have to exist somewhere
(either here or in a sibling helper). It doesn't.

## Implications for the Rust port

- **Phase 0b/0c (`ACommandPacket` commands):** none. The protocol crate stays
  I/O-free and crypto-free. No `CryptoHelper`-equivalent dependency is required
  for `Cmd::SetFanPWM`, `Cmd::SetPumpPWM`, `Cmd::ResetDevice`,
  `Cmd::GetFirmwareVersion`, or any other Phase 0b/0c codec.
- **Phase 0d firmware upload:** likely also crypto-free. The absence of any
  `Decrypt` entry point and the absence of any `Encrypt` caller anywhere near
  the firmware upload path strongly suggest firmware blobs ship as opaque
  bytes (possibly pre-encrypted at Lian-Li's build server, but with the device
  itself doing the verification via signed bootloader). **Decision deferred
  to Phase 0d** — confirm by inspecting the on-wire bytes of a real firmware
  upload before deciding whether to support firmware flashing on Linux.
- **Theme/profile bundles:** out of scope per design spec §2 (Linux client uses
  a native theme format, not the Windows JSON bundles).
- **Diagnostic telemetry export:** explicitly out of scope. We do not
  reproduce the `RequestType == 9` IPC handler. The Linux client has no
  equivalent of "phone home to Lian-Li support with the last 30 handshake
  packets." Users who need that workflow can keep the Windows app installed
  for diagnostics; the Linux client owns device control only.
