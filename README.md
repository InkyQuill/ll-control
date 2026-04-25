# L-Connect Linux

Linux-native client for Lian Li L-Connect 3 hardware. Currently wrapping
**Phase 0d** — protocol verification only. No daemon, no GUI yet.

## Status

(Legend: ✅ live-verified against real hardware; 🌗 codec-tested only;
🚧 not started.)

- ✅ Cargo workspace set up
- ✅ `lconnect-protocol` crate, with verified `ACommandPacket` framing
- ✅ Galahad II LCDV2 sensors (`GetHandshakeInfo`), firmware identity
  (`GetFirmwareVersion`), fan/pump PWM control (`SetFanPWM`, `SetPumpPWM`),
  and soft-reset codec (`ResetDevice`) — all verified against real hardware
  (or codec-tested with safety gate, in the case of reset)
- ✅ Crypto-not-used-at-transport confirmed (`docs/protocol/crypto-usage.md`)
- ✅ Lighting subsystem (`Get/Set {Fan,Pump}Lighting`) live-verified on
  the LCDV2. Pump-cap LED ring drives via `--scope outer`;
  fan ring via `set-fan-lighting`. See `docs/protocol/galahad2-lcdv2.md`
  for the LCDV2-specific scope semantics and a note on transient
  buffering during rapid-write bursts.
- ✅ LCD subsystem foundation (Phase 0d): `BCommandPacket` framing,
  LCD lifecycle (`IsLCDAvailable` / `LCDControl`), `SetLEDColors` for
  per-pixel RGB on the LED ring, and **JPEG upload to the pump-cap
  LCD** via `SendJpeg` (cmd 14, live-verified as transient
  single-frame with ~2 s / ~20 s auto-revert depending on
  `LCDControlMode`). `UploadJpeg` (cmd 15) is codec-verified but its
  activation path is unknown on FW 1.6 — bytes are accepted but no
  visible LCD change. The `lconnect-image` workspace crate handles
  arbitrary input → 480×480 JPEG conversion. End-to-end demo:
  `cargo run -p verify-galahad -- upload-jpeg --path any.png --yes`
  displays the image on the pump-cap LCD.
- 🌗 `SendH264Frame` (cmd 13) — Phase 0d stretch goal, not landed.
- 🚧 Daemon (with refresh loop for sustained LCD display), GUI,
  theme engine — Phases 1–6

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
cargo run -p verify-galahad -- get-pump-lighting       # read pump modes & wire-limits
cargo run -p verify-galahad -- set-pump-lighting       # static red on pump cap (defaults; add --color/--brightness/--mode to override)
cargo run -p verify-galahad -- get-fan-lighting        # read fan modes & wire-limits
cargo run -p verify-galahad -- set-fan-lighting --color '#0000FF'
cargo run -p verify-galahad -- set-led-colors --color '#FF0000' --yes
cargo run -p verify-galahad -- upload-jpeg --path any.png --yes  # display image on pump-cap LCD
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
