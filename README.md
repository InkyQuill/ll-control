# L-Connect Linux

Linux-native client for Lian Li L-Connect 3 hardware. **Phase 0d
complete; daemon work begins next.** Protocol verification only at
this stage — no daemon or GUI yet.

## Status

(Legend: ✅ live-verified against real hardware; 🌗 partially verified
— wire codec works but live behaviour has caveats; 🚧 not started.)

- ✅ Cargo workspace set up
- ✅ `lconnect-protocol` crate (151 tests + 6 doctests), with verified
  `ACommandPacket` (64-byte) and `BCommandPacket` (1024 out / 512 in)
  framing
- ✅ Galahad II LCDV2 sensors (`GetHandshakeInfo`), firmware identity
  (`GetFirmwareVersion`), fan/pump PWM control (`SetFanPWM`,
  `SetPumpPWM`), and soft-reset codec (`ResetDevice`)
- ✅ Crypto-not-used-at-transport confirmed
  (`docs/protocol/crypto-usage.md`)
- ✅ Lighting subsystem (`Get/Set {Fan,Pump}Lighting`) live-verified —
  pump cap drives visible RGB under `--scope outer`; fan ring drives via
  `set-fan-lighting`. See LCDV2 scope-semantics in
  `docs/protocol/galahad2-lcdv2.md`.
- ✅ LCD subsystem (Phase 0d): BCommandPacket framing,
  `IsLCDAvailable` (cmd 23, live-verified), `LCDControl` (cmd 12,
  codec-verified), **`SetLEDColors` (cmd 20, live-verified)** — drives
  per-pixel RGB on the LED ring, **`SendJpeg` (cmd 14, live-verified)**
  — displays a JPEG on the pump-cap LCD for ~20 seconds before
  auto-reverting to LocalUI default, `UploadJpeg` (cmd 15,
  codec-verified — bytes accepted, no visible activation found),
  `SendH264Frame` (cmd 13, codec-verified — wire shape confirmed
  against the C# call site).
- ✅ `lconnect-image` workspace crate (5 tests) — converts arbitrary
  input images (PNG/JPEG/WebP/BMP/GIF) to 480×480 JPEG ready for
  cmd 14/15.
- 🚧 Daemon, GUI, theme engine — Phases 1–6.
- 🚧 `CCommandPacket` (third framing class) — needed for
  `SendH264Frame512`. Phase 1+.

See [`docs/superpowers/specs/2026-04-25-lconnect-linux-design.md`](docs/superpowers/specs/2026-04-25-lconnect-linux-design.md)
for the full design and [`docs/superpowers/plans/`](docs/superpowers/plans/)
for phase-by-phase implementation plans.

## Hardware coverage (current)

| Device | VID:PID | Status |
|---|---|---|
| Galahad II LCDV2 ("Vision") | `0416:7395` | Phase 0d landing — sensors, PWM, lighting, LED ring, and pump-cap LCD JPEG upload all driveable |
| Everything else from L-Connect 3 | various | future phases |

## Building (developers only — there's nothing to use yet)

```bash
# Prerequisites
sudo pacman -S rust pkg-config systemd  # CachyOS / Arch
# or:  apt install build-essential libudev-dev pkg-config

# Workspace build
cargo build --workspace

# Run protocol unit tests
cargo test -p lconnect-protocol

# Phase-0 verification harness against a live device
cargo run -p verify-galahad -- list
cargo run -p verify-galahad -- handshake
cargo run -p verify-galahad -- firmware
cargo run -p verify-galahad -- fan --percent 50
cargo run -p verify-galahad -- pump --percent 75 --source mcu
# cargo run -p verify-galahad -- reset --yes-i-really-want-to-reset  # dangerous
cargo run -p verify-galahad -- get-pump-lighting
cargo run -p verify-galahad -- set-pump-lighting --color '#FF0000' --scope outer  # static red
cargo run -p verify-galahad -- get-fan-lighting
cargo run -p verify-galahad -- set-led-colors --color '#FF0000'                    # cmd 20, per-pixel LED
cargo run -p verify-galahad -- upload-jpeg --path my-image.png                      # cmd 14, JPEG to LCD (~20s display)
```

## udev permissions

For non-root access to the Galahad II LCDV2 hidraw node, install the rule
from `docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules`
(a proper multi-device rule lands in Phase 1):

```bash
sudo install -m 0644 \
  docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules \
  /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
# unplug and replug the device
```

## License

GPL-3.0-or-later.
