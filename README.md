# L-Connect Linux

Linux-native client for Lian Li L-Connect 3 hardware. **Phase 1
complete: `lconnectd` daemon + `lconnectctl` CLI live-verified
against Galahad II LCDV2.** GUI lands in Phase 2.

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
- ✅ Phase 0e: cross-device protocol survey
  (`docs/protocol/cross-device-survey.md`) — 13 device families +
  slv3 / lcd207 shared SDKs catalogued with cross-validation against
  OpenRGB. 6 read-only probes against LCDV2
  (`docs/protocol/captures/2026-04-26-cross-device-probes.txt`)
  confirmed asymmetric per-framing parser behavior (A-class permissive
  echo, B-class strict-on-unknown). New `verify-galahad probe`
  subcommand for cross-device experimentation.
- ✅ **Phase 1**: `lconnectd` daemon + `lconnectctl` CLI live-verified
  against Galahad II LCDV2 (FW 1.6, 2026-04-27). Sensors at 1 Hz,
  fan/pump PWM, lighting (preset modes + per-LED via cmd 20), LCD
  config + JPEG image with daemon-managed 15 s refresh loop, and
  persistence replay across daemon restart and device hot-plug.
  `CCommandPacket` framing codec landed. systemd `--user` unit + udev
  rule + AUR PKGBUILD skeleton in place. Cmd-15 investigation deferred
  (procedure preserved at
  `docs/protocol/captures/2026-04-27-cmd15-investigation.txt`). See
  `docs/protocol/captures/2026-04-27-phase-1-smoke-run.txt` for the
  live smoke transcript.
- 🚧 GUI, theme engine — Phases 2–6.

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
cargo run -p verify-galahad -- probe 0x86 --framing a --yes-read-only               # cmd byte probe (read-only; safety boundary is the curated probe set)
```

## udev permissions

For non-root access to the Galahad II LCDV2 hidraw node, install the rule
from `docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules`
(a proper multi-device rule lands with the daemon — see below):

```bash
sudo install -m 0644 \
  docs/protocol/captures/legacy-scaffold/99-lian-li-galahad2.rules \
  /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
# unplug and replug the device
```

## Daemon installation

Phase 1 ships `lconnectd` (the background daemon) and `lconnectctl` (the
control CLI). The daemon owns the HID handles and exposes a JSON-RPC
surface over `$XDG_RUNTIME_DIR/lconnect/daemon.sock`.

```bash
# Build + install binaries
cargo install --path crates/lconnect-daemon
cargo install --path crates/lconnect-cli

# udev rule for non-root device access (multi-device)
sudo install -m 0644 packaging/udev/70-lianli-hid.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger

# systemd --user unit
mkdir -p ~/.config/systemd/user
cp packaging/systemd/lconnectd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now lconnectd

# Confirm
lconnectctl daemon status
lconnectctl devices
```

Arch users: an AUR PKGBUILD skeleton is at `packaging/arch/PKGBUILD`.

## License

GPL-3.0-or-later.
