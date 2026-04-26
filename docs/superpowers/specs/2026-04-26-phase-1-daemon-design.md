# Phase 1 — Daemon design

- **Status:** approved (brainstorming complete) — ready for Phase 0e + Phase 1 plans
- **Date:** 2026-04-26
- **Author:** brainstorming session with user
- **Scope:** the first user-facing binary distribution of L-Connect Linux,
  wrapping every codec verified in Phase 0 behind a stable IPC contract
- **Supersedes:** Phase 1 / 3 / 4 sections of
  [`2026-04-25-lconnect-linux-design.md`](2026-04-25-lconnect-linux-design.md)
  — original spec separated daemon-MVP, lighting, and LCD-config into
  three phases; this design folds them all into Phase 1

---

## 1. Context

Phase 0 (a/b/c/d) verified `lconnect-protocol` against the user's
Galahad II LCDV2 (`0416:7395`, FW 1.6) for sensors, firmware, fan/pump
PWM, lighting (cmd 20/131/133), LCD config (cmd 12/23), and LCD JPEG
(cmd 14 live, cmd 15 codec-only). The `lconnect-image` crate prepares
arbitrary input images as 480×480 JPEG. Everything currently runs through
the `verify-galahad` CLI directly against the device.

Phase 1 promotes the verified codecs into a daemon with a stable IPC
surface, a CLI client, persistence for user settings, and packaging.

A short, focused **Phase 0e** runs first to survey LConnect3's command
landscape across all device families and cross-validate against
OpenRGB. The findings inform the daemon's framing-class abstraction and
the cmd-15 investigation hypotheses.

## 2. Scope

### In scope

- `lconnectd` daemon binary (single process, `systemd --user`)
- `lconnectctl` CLI client
- `lconnect-ipc` crate — `DaemonRpc` trait + JSON-RPC 2.0 transport
- `lconnect-hid` crate — `HidBackend` trait + `hidapi-rs` impl + mock
- `CCommandPacket` framing in `lconnect-protocol` (codec only)
- Timeboxed cmd-15 (`UploadJpeg`) activation investigation (1 day max)
- Persistence: per-device TOML for fan/pump/lighting/LCD config (small
  settings only — LCD JPEG is **not** persisted)
- Continuous re-issue timers for lighting (cmd 131/133) and LCD (cmd 14)
- Hot-plug detection (500 ms polling)
- udev rule, systemd unit, AUR PKGBUILD skeleton

### Out of scope

GUI, theme engine, fan curves, host sensors (CPU/GPU temps), H.264
streaming pipeline (codec only), `SendH264Frame512` live verification,
theme files, hot-reload, tray icon, Flatpak/AppImage, system-bus
support.

### Hardware coverage

Only Galahad II LCDV2. The daemon's per-device actor tree and
device-family registry are designed for multi-device but only one family
is registered. Phase 3 picks the next device based on the Phase-0e
overlap score.

## 3. Five load-bearing design decisions

The brainstorm settled five axes that shape every other section:

| Axis | Choice | Rationale |
|---|---|---|
| **Phase 1 scope perimeter** | wrap *everything* Phase 0 verified, not just sensors+PWM | Phase 0 went well past the original spec's Phase 0; deferring lighting/LCD into separate later phases would leave verified codecs unused |
| **LCD IPC model** | daemon abstracts the cmd-14 refresh loop; IPC exposes `set_lcd_image` / `clear_lcd` only | clients should never need to know cmd 14 reverts after ~20 s; the internal "frame source" seam is the explicit hook for Phase 4's theme engine |
| **Persistence model** | hybrid — small settings persist (fan/pump/lighting/LCD config); LCD JPEG bytes do **not** | matches user expectations for a settings service without disk-thrashing JPEGs once theme rendering arrives |
| **Lighting refresh policy** | mirror C# service's continuous re-issue timer (every ~1.5 s) | shipping software does this and there's an empirically-observed buffering quirk on FW 1.6; Lian Li knows their hardware |
| **Protocol additions** | strict wrap + CCommandPacket framing (codec only) + 1-day timeboxed cmd-15 investigation | keeps Phase 1 focused on daemon plumbing while unblocking known follow-ups |

## 4. Daemon process model

```
lconnectd (single process, tokio multi-threaded runtime)
├── ipc_server                  1 task; listens on $XDG_RUNTIME_DIR/lconnectd.sock,
│                               accepts long-lived JSON-RPC connections
├── device_manager              1 task; polls HidApi::device_list() every 500ms,
│                               consults the device-family registry, spawns/reaps
│                               per-device actor trees on hot-plug
└── per-device actor tree       (one tree per attached device)
    ├── hid_io                  owns the HidDevice; consumes a Command mpsc;
    │                           every read/write runs on spawn_blocking
    ├── sensor_poll             1 Hz; sends GetHandshakeInfo → hid_io;
    │                           publishes SensorSnapshot via tokio::sync::watch
    ├── lighting_keeper         re-issues current lighting state every ~1.5s
    │                           (mirrors C# service); replays from persistent
    │                           state on actor spawn / device reconnect
    ├── lcd_keeper              the "frame source" abstraction — consumes a
    │                           current-frame slot (Option<Bytes>); when Some,
    │                           re-emits to hid_io every ~15s; stops at None.
    │                           Phase 4's theme engine plugs in by writing
    │                           generated frames to the same slot at any
    │                           cadence.
    └── command_dispatch        services one-off IPC requests
                                (set_pwm, set_lighting, set_lcd_image, ...)
```

### Three invariants enforced project-wide

1. **No lock held across `.await`.** Snapshot fanout uses
   `tokio::sync::watch`, never `Mutex<Snapshot>`.
2. **All HID I/O on `spawn_blocking`.** `hidapi-rs` is synchronous;
   the blocking executor pool isolates stalls.
3. **Per-device serialization.** All HID writes for one device flow
   through `hid_io`'s mpsc — no concurrent writes, no echo-mismatch
   races, no torn multi-packet transfers.

### Hot-plug behavior

500 ms polling of `HidApi::device_list()`. On disconnect, the device's
actor tree exits cleanly; persisted state stays on disk; on reconnect,
the actor tree respawns and replays lighting + PWM + LCD config from
persistence (LCD image is **not** replayed — per the persistence model).
A future `udev` netlink subscription is a polish-phase upgrade, not
Phase 1 work.

## 5. IPC contract

### Transport

- Wire: line-delimited JSON-RPC 2.0 over a Unix domain socket at
  `$XDG_RUNTIME_DIR/lconnectd.sock`
- Auth: `$XDG_RUNTIME_DIR` is `0700` per user; the socket inherits that
- Connection model: long-lived; one client process = one connection.
  Server may emit notifications for subscribed events on the same
  connection.
- Versioning: server sends a `hello` notification on connect with
  `{ daemon_version, ipc_version }`; major-version mismatch → client
  reconnects with a compatible build

### `DaemonRpc` trait — Phase 1 surface

```rust
#[async_trait]
pub trait DaemonRpc: Send + Sync + 'static {
    // ── Discovery ──────────────────────────────────────────────────
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>>;
    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo>;

    // ── Sensors ────────────────────────────────────────────────────
    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot>;
    async fn subscribe_snapshots(&self, dev: DeviceId)
        -> Result<BoxStream<'static, SensorSnapshot>>;

    // ── PWM ────────────────────────────────────────────────────────
    async fn set_fan_pwm(&self, dev: DeviceId,
                         percent: Pwm) -> Result<()>;
    async fn set_pump_pwm(&self, dev: DeviceId,
                          percent: Pwm,
                          source: PwmSource) -> Result<()>;

    // ── Lighting ───────────────────────────────────────────────────
    async fn get_lighting_info(&self, dev: DeviceId, kind: LightingKind)
        -> Result<LightingInfo>;
    async fn set_lighting(&self, dev: DeviceId, kind: LightingKind,
                          cfg: LightingConfig) -> Result<()>;
    async fn set_led_colors(&self, dev: DeviceId, target: LedTarget,
                            colors: Vec<Rgb>) -> Result<()>;

    // ── LCD ────────────────────────────────────────────────────────
    async fn is_lcd_available(&self, dev: DeviceId) -> Result<bool>;
    async fn set_lcd_config(&self, dev: DeviceId,
                            cfg: LcdConfig) -> Result<()>;
    async fn set_lcd_image(&self, dev: DeviceId,
                           jpeg: Vec<u8>) -> Result<()>;
    async fn clear_lcd(&self, dev: DeviceId) -> Result<()>;

    // ── Maintenance / debug ───────────────────────────────────────
    async fn request_reset_token(&self, dev: DeviceId) -> Result<ResetToken>;
    async fn reset_device(&self, dev: DeviceId,
                          confirm: ResetToken) -> Result<()>;
    async fn raw_send(&self, dev: DeviceId, framing: Framing,
                      bytes: Vec<u8>) -> Result<Vec<u8>>;
    async fn daemon_stats(&self) -> Result<DaemonStats>;
}
```

### Server-pushed notifications

- `device_attached(DeviceDescriptor)`
- `device_detached(DeviceId)`
- `device_error(DeviceId, ErrorBrief)` — non-fatal HID read errors
- `snapshot(DeviceId, SensorSnapshot)` — only to clients that called
  `subscribe_snapshots`

### Device addressing

`DeviceId` is a stable opaque string assigned by the daemon, formatted
`"galahad2-lcdv2:0001"` for the first attached LCDV2. The mapping
`(vid, pid, hid_path) → DeviceId` is stable for the lifetime of the
daemon process. `DeviceDescriptor` carries the underlying tuple so
clients can re-resolve after a daemon restart.

### Error codes

| Code | Meaning |
|---|---|
| `-32600`…`-32603` | standard parse / method / params / internal |
| `-32000` | device not found |
| `-32001` | device busy / IO error |
| `-32002` | invalid argument (out-of-range PWM, oversize JPEG, etc.) |
| `-32003` | not implemented |
| `-32004` | confirmation token required (for `reset_device`) |
| `-32005` | protocol mismatch (response echo wrong, etc.) |

### Safety gates

- `reset_device` requires a fresh `ResetToken` from
  `request_reset_token` (one-shot, 30 s TTL). Defense against
  accidental CLI typos, not a security boundary.
- `raw_send` is gated behind `daemon.allow_raw_send = false` by default;
  intended for protocol diagnosis, not production use.

## 6. Persistence

### Layout

```
$XDG_CONFIG_HOME/lconnectd/                 # ~/.config/lconnectd/
├── daemon.toml                             # global daemon config
└── devices/
    └── galahad2-lcdv2_0416-7395_<serial>.toml

$XDG_STATE_HOME/lconnectd/                  # ~/.local/state/lconnectd/
└── last-known/<device-key>.toml            # informational; never read on startup

$XDG_RUNTIME_DIR/lconnectd.sock              # IPC socket (0700)
```

If `<serial>` is unavailable, fall back to `0416-7395_default.toml`.

### `daemon.toml`

```toml
[daemon]
log_level         = "info"               # error | warn | info | debug | trace
allow_raw_send    = false

[poll]
hotplug_ms        = 500
sensor_hz         = 1

[lcd]
refresh_ms        = 15000                # cmd-14 re-emit cadence

[lighting]
reissue_ms        = 1500                 # cmd-131/133 continuous re-emit cadence
```

### Per-device settings file

```toml
schema_version  = 1

[fan]
pwm_percent     = 50

[pump]
pwm_percent     = 75
source          = "mcu"

[lighting.fan]
mode            = "Rainbow"
brightness      = 4
speed           = 4
direction       = "Forward"
color           = "#3aa1ff"
disabled        = false

[lighting.pump]
scope           = "Outer"
mode            = "StaticColor"
brightness      = 4
speed           = 4
direction       = "Forward"
color           = "#FF00FF"
disabled        = false

[lcd]
brightness      = 50
rotation        = "Rotate0"
control_mode    = "LocalUI"              # mode at startup; image NOT persisted
```

### Write semantics

- All writes atomic: write-temp + `fsync` + rename
- Writes debounced ~250 ms to absorb future GUI slider drags
- Writes happen **after** the device successfully accepts the change;
  HID failure → no persistence update
- `schema_version = 1` ships in v1; unknown versions warn and fall back
  to defaults

### Replay on startup / reconnect

When a device's actor tree spawns, in order with ~50 ms inter-write
delay: lighting → fan PWM → pump PWM → LCD config. `lighting_keeper`
begins continuous re-issue. `lcd_keeper` starts in the empty-slot state.

## 7. CLI surface

```sh
# Discovery
lconnectctl devices
lconnectctl firmware [--device ID]

# Sensors
lconnectctl snapshot [--device ID]
lconnectctl watch    [--device ID]              # streaming, redraws in place

# PWM
lconnectctl fan  <PERCENT> [--device ID]
lconnectctl pump <PERCENT> [--device ID] [--source mcu|application]

# Lighting
lconnectctl lighting info <fan|pump> [--device ID]
lconnectctl lighting set  <fan|pump>            # cmd 131/133, preset modes
       --mode <Rainbow|RainbowMorph|StaticColor|Breathing|...>
       [--color '#RRGGBB']
       [--speed 0..=100]
       [--brightness 0..=100]
       [--direction Forward|Reverse]
       [--scope Inner|Outer|All]                # pump only
       [--disable]
lconnectctl lighting clear <fan|pump> [--device ID]

lconnectctl led-colors set <fan|screen> --color '#RRGGBB' [--count N]
lconnectctl led-colors set <fan|screen> --from-file colors.txt

# LCD
lconnectctl lcd available    [--device ID]
lconnectctl lcd brightness <PERCENT>
lconnectctl lcd rotation <0|90|180|270>
lconnectctl lcd mode <LocalUI|Application|...>
lconnectctl lcd image <PATH>                    # set persistent image
       [--fit crop|letterbox|stretch]
       [--letterbox-bg '#RRGGBB']
       [--quality 1..=100]
lconnectctl lcd clear [--device ID]             # stop refreshing

# Maintenance
lconnectctl reset --yes-i-really-want-to-reset
lconnectctl logs [--follow] [--level debug]     # journalctl --user passthrough
lconnectctl raw <FRAMING:a|b|c> <HEX>           # only when daemon allows it

# Daemon control
lconnectctl daemon status
lconnectctl daemon version
```

`--device` defaults to the only attached device. With more than one,
the CLI prints the list and exits — no magic-first-device fallback.
Output is human-readable by default; `--json` emits machine-readable
output (NDJSON for `watch`).

## 8. Phase 0e — cross-device command survey

Phase 0e runs **before** Phase 1's daemon work. It produces
`docs/protocol/cross-device-survey.md`, a single document mapping
every command, framing class, and payload shape that LConnect3 uses
across all device families, cross-validated against OpenRGB.

### Sources

1. **Primary:** decompiled `L-Connect.Core/Products/<family>/`
2. **Cross-validator:** [OpenRGB's `Controllers/LianLiController/`](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers/LianLiController)
3. **Verified:** `docs/protocol/captures/` (LCDV2 only at this point)

OpenRGB is GPL-2.0-or-later; license-compatible for reading. The
survey doc cites OpenRGB by URL and file path; it does **not** copy
OpenRGB code into our tree. Protocol facts (command bytes, payload
offsets, framing) are documented in our own tables.

### Coverage

For each device family in `Products/`:

- TLFan
- Galahad2 (Trinity, LCD, LCDV2 — already done)
- HydroShift (I, II Circle, II Square — closest LCDV2 analog)
- TLLCD / TLV2LCD / SLV3LCD
- UniversalScreen 8.8"
- StrimerPlus
- Ene6K77Fan (AL, SL)
- LWireless (likely named-pipe, may not use HID)
- Lancool 207 / 217 Digital

Per family, document USB VID/PID, framing classes used, full command
table with citations, and presence/absence of subsystem patterns
(continuous re-issue, fan curves, lighting modes, LCD frame upload).

### Cross-reference matrix

Per shared subsystem (lighting, LCD frame, fan PWM, ...), one table
showing which families share exact command bytes + payloads, which
overlap by byte but mean different things, and which are unique. The
matrix gets a column for OpenRGB cross-validator status (✅ matches /
⚠ diverges / — not implemented).

### Findings — actionable outputs

- Daemon framing-class extension points (do any families use a fourth
  framing class beyond A/B/C?)
- Cmd-15 hypotheses — does HydroShift II's LCD-image command shape
  match cmd 15? If so, the activation path may be observable by static
  comparison alone
- Reusable codec opportunities — any byte-identical commands across
  families get factored into a `protocol/common/` module before the
  second device family lands
- Risky-area flagging for `hid-safety-reviewer`

### Targeted live probing on LCDV2 (optional, gated)

Read-only commands picked from HydroShift I/II that LCDV2 might
respond to (since the LCD-pump MCU may be shared firmware). Run via a
new `verify-galahad probe <cmd-hex> [--framing a|b|c]` subcommand —
never in the daemon.

- Read-only commands. No writes, no resets, no firmware operations.
- One probe at a time. `hid-safety-reviewer` dispatch on every probe
  definition.
- Output: `docs/protocol/captures/2026-04-26-cross-device-probes.txt`

### Estimated cost

Static survey: 1–2 days. OpenRGB cross-validator pass: ~½ day.
Optional probing: ~½ day. Total: 1.5–2.5 days.

## 9. Phase 1 — protocol-side additions

### CCommandPacket framing

Third packet class, parallels Phase 0d's BCommandPacket work.

- ReportID = `0x03`, output = 512 bytes, input = 512 bytes
- Header layout cited from the `Galahad2VisionDevice.cs` call site for
  `SendH264Frame512` (Phase-0 citation convention: every codec
  doc-comment names the C# `file:line` range it was derived from)
- Phase 0e's static survey provides additional cross-family validation
  that the framing class is indeed shared, not LCDV2-specific
- Payload max ≈ 501 bytes (header occupies 11 bytes)
- Owned-buffer iterator mirroring `BPacketChunks` (no shared-buffer
  reuse)
- Codec only — `SendH264Frame512` live verification waits on a real
  H.264 pipeline (Phase 4+)

### Cmd-15 (`UploadJpeg`) activation investigation

Hard timebox: **1 working day**. Whatever the outcome, write up
findings.

Hypothesis tree, in order:

1. Cmd 15 stores to flash for boot-time display — power-cycle the
   device after a successful 56-packet upload, see if the image
   appears at boot
2. Cmd 15 is consumed by `LCDControlMode::LocalAVI` or `::LocalH264` —
   toggle through each mode after upload
3. Cmd 15 needs a separate "activate stored image" command we haven't
   decompiled yet — grep the C# source for callers of `UploadJpeg`
   and trace
4. Cmd 15 is dead/legacy on FW 1.6 — accept and document

Output: `docs/protocol/captures/2026-04-26-cmd15-investigation.txt`
plus an updated entry in `galahad2-lcdv2.md`.

If activation found, expose via `lconnectctl lcd boot-image <PATH>` (or
whatever the activation semantics turn out to be). If not found, cmd
15 stays codec-implemented but unexposed in IPC and explicitly
documented as "no known activation path on FW 1.6."

## 10. Workspace layout

```
lconnect-linux/
├── crates/
│   ├── protocol/                  existing; gains CCommandPacket
│   ├── lconnect-image/            existing
│   ├── lconnect-hid/              NEW — HidBackend trait + hidapi-rs + mock
│   ├── lconnect-ipc/              NEW — DaemonRpc trait + JSON-RPC server/client
│   ├── lconnect-daemon/           NEW — lconnectd binary
│   └── lconnect-cli/              NEW — lconnectctl binary
├── tools/
│   └── verify-galahad/            existing; gains `probe` subcommand
├── packaging/                     NEW
│   ├── systemd/lconnectd.service
│   ├── udev/70-lianli-hid.rules
│   └── arch/PKGBUILD
└── xtask/                         NEW — generates the udev rule from a
                                    single device-family table
```

Boundary rules enforced by the Cargo dep graph:

- `protocol/` is pure, no I/O, `#![forbid(unsafe_code)]` (already true)
- `lconnect-hid/` is the only crate that depends on `hidapi-rs`
- `lconnect-cli/` depends only on `lconnect-ipc/` — cannot do HID I/O
- `lconnect-daemon/` is the only crate that wires everything together

## 11. Packaging

### udev — `packaging/udev/70-lianli-hid.rules`

```
# Galahad II LCDV2
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0416", ATTRS{idProduct}=="7395", \
  TAG+="uaccess", MODE="0660", GROUP="plugdev"
```

`TAG+="uaccess"` gives the logged-in user automatic ACL. Phase 1 ships
the LCDV2 entry only; the `xtask`-generated table adds new entries as
device families come online.

### systemd — `packaging/systemd/lconnectd.service`

```ini
[Unit]
Description=L-Connect Linux daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/lconnectd
Restart=on-failure
RestartSec=2
Environment=RUST_LOG=lconnectd=info

[Install]
WantedBy=default.target
```

User bus only. No PolicyKit, no system-bus.

### Distribution

- AUR PKGBUILD — primary path (CachyOS/Arch); builds all three
  binaries, installs udev rule + systemd unit, post-install
  `udevadm control --reload-rules`
- `cargo install --path crates/<daemon|cli>` — non-Arch users; drop in
  udev + systemd manually, documented in README
- No Flatpak, AppImage, or `.deb` in v1

## 12. Observability

- `tracing` crate end-to-end. Spans on per-device actor lifecycle,
  per-IPC-request, per-HID-write
- `tracing-journald` under systemd; `tracing-subscriber::fmt` to stdout
  when run interactively
- `RUST_LOG=lconnectd=debug` toggles verbosity at process start;
  SIGHUP reload is polish-phase
- Per-device counters (transient HID errors, decode failures, etc.)
  exposed via `daemon_stats` IPC method
- No metrics endpoint (Prometheus/etc.) in v1

## 13. Testing strategy

| Layer | Style | Hardware |
|---|---|---|
| `lconnect-protocol` (existing + CCommandPacket) | unit + table-driven | no |
| `lconnect-image` (existing) | round-trip integration | no |
| `lconnect-hid` discovery + retries | mock-hid (fake HidBackend impl) | no |
| `lconnect-ipc` JSON-RPC | in-memory transport integration | no |
| Daemon actor topology | tokio-test with mock-hid | no |
| End-to-end via real socket | spawn daemon + client integration test | no |
| Live device verification | manual `lconnectctl` smoke run | **yes** |

The `mock-hid` shim is a new piece — a `HidBackend` trait with a
`hidapi-rs` impl and a fake impl for tests. Most layers reach high
confidence without hardware. Live verification is the exit gate.

## 14. Acceptance criteria

- [ ] `cargo build --workspace` and `cargo build --workspace --release` succeed
- [ ] `cargo test --workspace` passes; ≥30 new tests over Phase 0d's 156
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo doc --workspace --no-deps` builds with no warnings
- [ ] `systemctl --user enable --now lconnectd` starts the daemon;
      `journalctl --user -u lconnectd` shows clean startup
- [ ] `lconnectctl devices` lists the LCDV2
- [ ] `lconnectctl watch` shows live coolant temp updating at 1 Hz
- [ ] `lconnectctl fan 50` / `pump 75 --source mcu` set PWM
      live-verifiably
- [ ] `lconnectctl lighting set pump --mode StaticColor --color '#FF0000'
      --scope outer` lights the pump cap red, **stays red after 10 s**
      (continuous re-issue working)
- [ ] `lconnectctl led-colors set screen --color '#00FF00'` lights the
      LED ring green
- [ ] `lconnectctl lcd image <some.jpg>` displays the JPEG, **still
      showing after 60 s** (refresh loop working)
- [ ] `lconnectctl lcd clear` reverts to LocalUI
- [ ] Disconnect + reconnect device → lighting and PWM replay from
      persistence; LCD does not (per persistence model)
- [ ] Daemon survives `verify-galahad` running concurrently against
      another USB path (no shared HID handle, no contention)
- [ ] Stop & restart daemon → settings replay; LCD does not
- [ ] `docs/protocol/cross-device-survey.md` covers all device families
      with cross-validator column for OpenRGB
- [ ] CCommandPacket framing codec lands with ≥10 unit tests and a
      `hid-safety-reviewer` dispatch in the plan transcript
- [ ] Cmd-15 investigation has a written outcome doc, regardless of
      whether activation was found
- [ ] AUR PKGBUILD builds successfully on a clean Arch container (CI
      or manual)
- [ ] Every HID write path landed in this phase has a
      `hid-safety-reviewer` dispatch recorded in the implementation
      plan

## 15. Risks & open questions

1. **`hidapi-rs` blocking semantics on `hidraw`** — `read_timeout`
   behavior under contention is known to be quirky. The daemon's
   `spawn_blocking` isolates this, but Phase 1 should benchmark actual
   read-timeout distribution during sustained refresh-loop operation
   and document any backend-specific workarounds.
2. **Hot-plug 500 ms poll cadence** — fine for typical use; a user
   power-cycling the AIO mid-session sees a noticeable gap. Acceptable
   for v1; netlink is a polish-phase upgrade.
3. **HID contention between LCD refresh and lighting re-issue** — LCD
   refresh (every 15 s, ~50 KiB) and lighting re-issue (every 1.5 s,
   64 bytes) share the device. Per-device serialization through
   `hid_io`'s mpsc handles correctness, but a brief `tracing` timing
   pass during implementation should verify the LCD write doesn't
   starve the lighting timer.
4. **`lconnectctl` UX before a GUI ships** — the CLI is the only
   interface for ~one phase. The `--watch` view is the most-likely-to-
   feel-rough piece; revisit during Phase 2 in light of GUI patterns.
5. **`mock-hid` divergence from real `hidraw`** — fake backend won't
   catch `hidapi-rs` quirks. Live `lconnectctl` smoke run is the real
   exit gate.
6. **OpenRGB cross-validator may return little useful overlap** —
   OpenRGB targets RGB strips and case fans; LCDV2 is an AIO cooler.
   Even a "no overlap" finding is useful — it reframes Phase 0e's
   scope to mostly C# decompilation.
7. **Cmd-15 investigation may dead-end** — possible the activation
   path doesn't exist on FW 1.6 at all. The 1-day timebox is the
   exit valve; we accept "documented unknown" as a valid outcome.

## 16. Phase ordering recap

| Phase | Deliverable | Size | HW |
|---|---|---|---|
| 0a–0d | Protocol verification for LCDV2 (**done**) | — | yes |
| **0e** | **Cross-device command survey + OpenRGB cross-validation + optional LCDV2 read-only probes** | small (1.5–2.5 days) | partial |
| **1** | **Daemon MVP — `lconnectd` + IPC + `lconnectctl`, full Phase-0 feature surface, persistence, refresh & lighting timers, CCommandPacket codec, cmd-15 investigation** | large | yes |
| 2 | GUI shell (egui, lag-free by construction, replaces broken scaffold) | small | yes |
| 3 | Second device family — picked from 0e survey by overlap score; protocol verification + adoption into daemon | medium | requires hardware |
| 4 | Theme engine v1 (TOML manifest + expr + tiny-skia + bindings + hot reload) | **large** | yes |
| 5 | Polish — fan curves, theme parameter forms, AUR PKGBUILD release, troubleshooting docs | medium | yes |

Original spec's "Phase 3 = Lighting" / "Phase 4 = LCD config + static
image" are absorbed into Phase 1 (per the scope decision). Original
"Phase 5 = Theme engine" becomes Phase 4.

The IPC contract is the load-bearing decision; `lcd_keeper`'s frame
slot is the explicit hook for Phase 4's theme engine; the
device-family registry is the explicit hook for Phase 3's second
device.
