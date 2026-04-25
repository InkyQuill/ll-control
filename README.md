# L-Connect Linux

Linux-native client for Lian Li L-Connect 3 hardware. Currently nearing the
end of **Phase 0c** — protocol verification only. No daemon, no GUI yet.

## Status

- ✅ Cargo workspace set up
- ✅ `lconnect-protocol` crate, with verified `ACommandPacket` framing
- ✅ Galahad II LCDV2 sensors (`GetHandshakeInfo`), firmware identity
  (`GetFirmwareVersion`), fan/pump PWM control (`SetFanPWM`, `SetPumpPWM`),
  and soft-reset codec (`ResetDevice`) — all verified against real hardware
  (or codec-tested with safety gate, in the case of reset)
- ✅ Crypto-not-used-at-transport confirmed (`docs/protocol/crypto-usage.md`)
- 🌗 Lighting subsystem (`Get/Set {Fan,Pump}Lighting`) — codecs and
  CLI complete and integration-tested (77 tests across the protocol
  crate); live-device verification pending
- 🚧 LCD config + frame upload — Phase 0d
- 🚧 Daemon, GUI, theme engine — Phases 1–6

See [`docs/superpowers/specs/2026-04-25-lconnect-linux-design.md`](docs/superpowers/specs/2026-04-25-lconnect-linux-design.md)
for the full design and [`docs/superpowers/plans/`](docs/superpowers/plans/)
for phase-by-phase implementation plans.

## Hardware coverage (current)

| Device | VID:PID | Status |
|---|---|---|
| Galahad II LCDV2 ("Vision") | `0416:7395` | Phase 0a in progress |
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
cargo run -p verify-galahad -- get-pump-lighting       # read modes & limits
cargo run -p verify-galahad -- set-pump-lighting       # static red on pump cap (default flags)
cargo run -p verify-galahad -- get-fan-lighting
cargo run -p verify-galahad -- set-fan-lighting --color '#0000FF'
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
