# L-Connect Linux — Design

- **Status:** approved (brainstorming complete) — ready for Phase 0 plan
- **Date:** 2026-04-25
- **Author:** brainstorming session with user
- **Supersedes:** none (`PROTOCOL_DOCUMENTATION_RU.md` is kept as legacy reference, not replaced by this doc)

---

## 1. Project context

L-Connect Linux is a Linux-native rewrite of the Lian Li L-Connect 3 Windows
application. The Windows source has been decompiled and lives alongside this
project at:

- `/home/inky/Development/lian-li/L-Connect.Core/` — protocol logic (primary
  reference for Phase 0 verification)
- `/home/inky/Development/lian-li/L-Connect3/` — WPF UI (informational only)
- `/home/inky/Development/lian-li/L-Connect-Service/` — Windows service
  (informational only; we mirror its architecture, not its code)

A scaffolding attempt exists at `/home/inky/Development/lian-li/lconnect-linux/`
(Rust + egui, ~6 source files). It targets the right device but has
architectural bugs that produce permanent UI lag and hangs (see §3). Most of
that code is discarded; the Cargo project skeleton is reused.

A 958-line Russian protocol document
(`PROTOCOL_DOCUMENTATION_RU.md`) was written 2025-02-08 against an incorrect
assumption about the device's PID. It is **not** authoritative and will be
superseded by the Phase 0 deliverable.

## 2. Scope

### Hardware — Phase 1 (the only target until v1 ships)

- **Galahad II LCDV2** — `0x0416:0x7395`, firmware `1.6`, marketed as "Galahad
  II Vision". This is the user's hardware. Decompiled enum identity:
  `ControllerTypeID.Galahad2LCDV2` (`KnownDevices.cs`).
- **Note on naming:** the decompiled folder `Products/Galahad2Vision/`
  appears to implement multiple SKUs. Phase 0 verification confirms exactly
  which folder/files implement `Galahad2LCDV2` versus other variants
  (`Galahad2LCD` `0x7391`, `Galahad2TrinityRegular` `0x7383`,
  `Galahad2TrinityPerformance` `0x7381`).

### Hardware — long-term (out of scope for v1, in scope for the architecture)

Full coverage of every product family in `KnownDevices.cs`: TLFan, SL/AL
fans, StrimerPlus, Galahad II Trinity, HydroShift LCD/RGB/TL, HydroShift II
LCD (Circle/Square), TLLCD, SLV3LCD, TLV2LCD, UniversalScreen 8.8", L-Wireless
hub, Lancool 207/217 Digital. Architecture must accommodate adding a new
device family without rewrites — one new module under `protocol/<sku>/`,
one new udev rule line, no changes to daemon/IPC/GUI/theme cores.

### LCD feature scope — T5 (full theme engine)

The user owns the device for the LCD. Theme engine is the centerpiece, with:

- multiple animated layouts switchable at runtime,
- configurable text, colors, and sensor bindings,
- conditionals for threshold-driven visuals (hard requirement: user explicitly
  asked for conditionals).

### Explicit non-goals (v1)

- No video / GIF playback on the LCD.
- No screen mirroring.
- No 3D, no shaders, no Lua/Rhai/JS/Python scripting.
- No cross-theme transitions; switching themes is a hard cut.
- No firmware update support (too risky for v1).
- No Windows-theme-file compatibility (read or write).
- No GUI theme editor — themes are hand-edited TOML in v1.
- No tray icon.
- No Flatpak / AppImage in v1 — AUR + tarball + `cargo install` only.
- No multi-user / system-bus story; daemon is `systemd --user` only.

## 3. Lag/hang root-cause in the existing scaffold

`lconnect-linux/src/app.rs` has at least four overlapping bugs that
guarantee a stalled UI:

1. **Blocking HID I/O on the UI thread.** `update_device_data()` runs inside
   `eframe::App::update()`, the paint thread, and calls
   `device.read_timeout(buf, 1000)` — up to 1 second of blocking I/O per
   paint.
2. **Repaint storm during input.** egui repaints on every input event. Because
   step 1 runs unconditionally, every mouse motion over a slider triggers a
   fresh HID round-trip, each potentially waiting 1 s. They queue.
3. **Contradictory blocking mode.** `set_blocking_mode(false)` followed by
   `read_timeout(...)`. `read_timeout` is defined for blocking mode; behavior
   in non-blocking mode is implementation-defined and platform-dependent.
4. **No state cache.** Every paint re-fetches sensors from hardware. There's
   nothing for the GUI to read between polls except the device itself.

**Cure (architectural, not a patch):** the daemon owns HID; the GUI never
touches it. Sensors are cached in `tokio::sync::watch` and pushed to clients
over IPC. `update()` becomes pure and microsecond-fast. This is enforced by
the workspace structure — the GUI crate doesn't depend on `hidapi`.

## 4. Architecture decision

**Daemon + GUI client**, single-language Rust workspace.

```
   ┌────────────┐  ┌────────────┐  ┌────────────┐
   │ lconnect-  │  │ lconnect-  │  │  future    │
   │   gui      │  │   ctl (CLI)│  │  clients   │
   └──────┬─────┘  └──────┬─────┘  └──────┬─────┘
          │ JSON-RPC      │               │
          └───────────────┴───────────────┘
                          │  $XDG_RUNTIME_DIR/lconnectd.sock
                  ┌───────▼────────┐
                  │   lconnectd    │
                  │  (the daemon)  │
                  └───────┬────────┘
                          │ hidraw
                       ┌──▼──┐
                       │ HID │  Lian Li device(s)
                       └─────┘
```

- **Daemon** owns every HID handle. Tokio runtime. Per-device actor tree.
- **GUI** is a stateless egui client of the daemon's IPC.
- **CLI** (`lconnectctl`) is a stateless command-line client of the same IPC.
- **IPC**: JSON-RPC 2.0 over Unix socket, line-delimited.
- **Migration hook to DBus later**: the entire IPC surface lives behind one
  `DaemonRpc` trait; swapping JSON-RPC for `zbus` is a transport-layer change
  with no callsite churn in GUI/CLI.

### Why Rust end-to-end (Approach 1)

- Already chosen by the user's prior scaffolding work.
- Single language across daemon/GUI/CLI minimises cognitive load.
- `hidapi-rs`, `tokio`, `tiny-skia`, `serde_json`, `eframe/egui` are all
  battle-tested and work fine on Linux.
- The existing scaffold's `Cargo.toml` and project skeleton are reused; the
  buggy `app.rs` and direct-HID `galahad.rs` are removed.

Approaches 2 (GTK4 + DBus) and 3 (Tauri + web UI) were explicitly considered
and deferred. Both are reachable from Approach 1 without rewriting the
daemon — only the IPC transport and the UI process change.

## 5. Phase 0 — Protocol verification

Phase 0 produces a single deliverable, **`docs/protocol/galahad2-lcdv2.md`**,
that becomes the only source of truth used by implementation phases. After
Phase 0 lands, no implementation crate reads decompiled C# directly.

### Coverage

For Galahad II LCDV2, every command we will use:

| Subsystem | Verification target |
|---|---|
| Discovery | `lsusb -v`, `udevadm info`, raw HID descriptor; confirm only `0x0416:0x7395` interfaces. |
| Handshake / sensors | Exact request/response framing for `GetHandshakeInfo` (cmd byte, payload offsets for fan-RPM, pump-RPM, coolant temp, validity flag). |
| Firmware version | Confirm `cmd 134` and the `N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6` token format. Tokens between commas matter for SKU branching. |
| Fan PWM | Confirm `cmd 139` payload layout. The current scaffold's `packet[2] = percentage` is suspicious — most Lian-Li products use a fan-group-indexed payload. |
| Pump PWM | Confirm `cmd 138` payload layout. |
| Fan curve | Find the curve-upload command (the current scaffold treats curves as a TODO comment). |
| Lighting | Find lighting-mode commands (mode, speed, direction, brightness, per-LED RGB). |
| LCD config | Brightness, rotation, screen on/off, sleep mode. |
| LCD frame upload | The big one — chunking, frame headers, JPEG vs raw-RGB, max payload size, flow control. |
| Crypto | Determine whether `LConnectCore/Crypto/CryptoHelper.cs` is used on transport (per-packet) or only on theme files. If transport-encrypted, the protocol crate gains a transform layer. |

### Verification methodology

For each command, three independent artefacts:

1. **Static citation:** `file:line` references from the decompiled C#
   proving the wire format. Stored inline in the protocol doc.
2. **Live capture:** the user runs the Windows L-Connect 3 app under their
   dual-boot setup with `Wireshark + usbmon` (or equivalent) and captures
   each command being issued. PCAPNGs go in `docs/protocol/captures/`.
3. **Round-trip test:** a `verify_<cmd>.rs` test in `crates/protocol/tests/`
   that issues the command from Linux against the live device and validates
   the response shape. Doubles as a regression test forever after.

The dual-boot capture is highly recommended for the LCD frame protocol; for
all other commands, static analysis + experimentation from Linux is enough.

### Output

`docs/protocol/galahad2-lcdv2.md` with, per command:

- name, command byte, direction (host→device or device→host),
- full byte layout with offsets,
- response shape,
- `file:line` citations from the decompiled C#,
- a `verified | inferred | unknown` status flag,
- known firmware-version dependencies (if any),
- a link to the corresponding `verify_*` test.

## 6. Daemon design (`lconnectd`)

### Process model

- Single binary, managed by `systemd --user`.
- No root required — HID access via udev rule with `TAG+="uaccess"`.
- Tokio multi-threaded runtime.

### Task / actor topology

```
lconnectd (tokio runtime)
├── ipc_server                  (1 task)  listens on $XDG_RUNTIME_DIR/lconnectd.sock
├── device_manager              (1 task)  polls device list every 500 ms,
│                                         spawns/reaps Device actors on hotplug
└── per-Device actor (one tree per attached device):
    ├── hid_io_task              owns the HidDevice; consumes a Command mpsc;
    │                            does writes + reads on spawn_blocking
    ├── sensor_poll_task         1 Hz; sends GetHandshakeInfo to hid_io;
    │                            publishes SensorSnapshot via tokio::sync::watch
    ├── theme_render_task        event-driven (see §8); reads bindings from
    │                            watch channels; renders frame to JPEG;
    │                            pushes via hid_io
    └── command_dispatch         handles one-off commands from IPC
                                 (set fan PWM, switch theme, etc.)
```

### Three invariants enforced everywhere

1. **No lock is ever held across `.await`.**
2. **All HID writes/reads happen on `spawn_blocking`.** `hidapi-rs` is
   synchronous; the blocking executor pool isolates any stall.
3. **Sensor snapshots use `tokio::sync::watch`**, never `Mutex<Snapshot>`.
   Latest-wins semantics; readers are lock-free.

### Channels per device

- `Command` mpsc into `hid_io_task` —
  `{ GetHandshake(reply: oneshot), SetFanPwm(u8), SetPumpPwm(u8),
     UploadFrame(Bytes), SetLighting(LightingConfig), ... }`
- `SensorSnapshot` watch —
  `{ coolant_c: f32, fan_rpm: u16, pump_rpm: u16, temp_valid: bool,
     fw_version: String, last_seen: Instant }`

### Hotplug

- `device_manager` polls `HidApi::device_list()` every 500 ms.
- Detect known VID/PID → open → spawn actor tree.
- HID I/O error indicating device-gone (`ENODEV`, `-1`) → actor exits clean,
  manager respawns on next detection.
- Theme/state persists across reconnects, keyed by `vid:pid:serial`.

### Persistence

- Config: `$XDG_CONFIG_HOME/lconnectd/`
  - `daemon.toml` — global settings
  - `devices/<vid>_<pid>_<serial>.toml` — per-device fan/pump/lighting/
    active-theme
  - `themes/<theme-id>/` — user theme bundles
- Runtime state: `$XDG_STATE_HOME/lconnectd/last-snapshot.json` (best-effort).
- All writes atomic (write-temp + rename).

## 7. IPC surface

### Transport

- **Wire:** Unix domain socket at `$XDG_RUNTIME_DIR/lconnectd.sock`
  (`/run/user/$UID/lconnectd.sock`).
- **Encoding:** line-delimited JSON-RPC 2.0; one object per `\n`.
- **Connection model:** long-lived; one connection per client.
  Subscriptions are server-pushed JSON-RPC notifications on the same
  connection.
- **Auth:** UNIX permissions on `$XDG_RUNTIME_DIR` (`0700`). No tokens,
  no TLS.
- **Versioning:** `hello` handshake on connect; server refuses on major
  version mismatch.

### Trait abstraction (the migration hook to DBus later)

```rust
#[async_trait]
pub trait DaemonRpc: Send + Sync + 'static {
    async fn list_devices(&self) -> Result<Vec<DeviceDescriptor>>;
    async fn get_snapshot(&self, dev: DeviceId) -> Result<SensorSnapshot>;
    async fn subscribe_snapshots(&self, dev: DeviceId)
        -> Result<BoxStream<'static, SensorSnapshot>>;
    async fn set_fan_pwm(&self, dev: DeviceId, percent: u8) -> Result<()>;
    async fn set_pump_pwm(&self, dev: DeviceId, percent: u8) -> Result<()>;
    async fn set_fan_curve(&self, dev: DeviceId,
                           points: Vec<(f32, u8)>) -> Result<()>;
    async fn set_lighting(&self, dev: DeviceId,
                          cfg: LightingConfig) -> Result<()>;
    async fn list_themes(&self) -> Result<Vec<ThemeDescriptor>>;
    async fn set_active_theme(&self, dev: DeviceId,
                              theme: ThemeId) -> Result<()>;
    async fn set_theme_param(&self, dev: DeviceId,
                             key: String, val: ParamValue) -> Result<()>;
    async fn upload_theme_asset(&self, theme: ThemeId,
                                path: String, bytes: Bytes) -> Result<()>;
    async fn set_lcd_config(&self, dev: DeviceId,
                            cfg: LcdConfig) -> Result<()>;
    async fn get_firmware(&self, dev: DeviceId) -> Result<FirmwareInfo>;
}
```

### Server-pushed notifications

- `device_attached`, `device_detached`, `device_error`, `snapshot`
  (per-device, only to subscribed clients).

### Error codes (JSON-RPC 2.0)

- `-32000` device not found
- `-32001` device busy / IO error
- `-32002` invalid theme / asset
- `-32003` not implemented
- standard `-32600` … `-32603` for parse / method / params / internal.

## 8. Theme engine

### Theme = directory bundle

```
~/.config/lconnectd/themes/coolant-arc/
├── theme.toml      # manifest
├── background.png
├── ring.png
└── fonts/InterTight-Bold.ttf
```

Built-in themes ship in `/usr/share/lconnectd/themes/` (read-only); user
themes shadow built-ins by `id`.

### Manifest model

A theme is a stack of typed layers rendered back-to-front. No scripting.

```toml
[theme]
id        = "coolant-arc"
name      = "Coolant Arc"
device    = "galahad2-lcdv2"   # or "any"
canvas    = { width = 480, height = 480, shape = "circle" }
animation = "event"            # "event" | "fixed:NN_hz"

[[layer]]
type = "image"
src  = "background.png"
fit  = "cover"

[[layer]]
type      = "arc"
center    = ["50%", "50%"]
radius    = "44%"
thickness = 18
start_deg = 135
sweep_deg = { binding = "device.coolant_c",
              map = [[20, 0], [70, 270]] }
color     = { binding = "device.coolant_c",
              gradient = [[20, "#3aa1ff"],
                          [50, "#ffd23a"],
                          [70, "#ff3a3a"]] }

[[layer]]
type    = "text"
font    = "fonts/InterTight-Bold.ttf"
size    = 96
color   = { expr = "coolant_c >= 70 ? '#ff3a3a' \
                  : (coolant_c >= 50 ? '#ffd23a' : '#3aa1ff')" }
binding = "device.coolant_c"
format  = "{:.0}°"
visible = { expr = "device.fan_rpm > 0" }
anchor  = ["center", "center"]
```

### Bindings

| Namespace | Examples |
|---|---|
| `device.*` | `device.coolant_c`, `device.fan_rpm`, `device.pump_rpm` |
| `host.cpu.*` | `host.cpu.temp_c`, `host.cpu.load`, `host.cpu.freq_ghz` |
| `host.gpu.*` | `host.gpu.temp_c`, `host.gpu.load`, `host.gpu.power_w` |
| `host.mem.*` | `host.mem.used_pct`, `host.mem.used_gb` |
| `host.time.*` | `host.time.hhmm`, `host.time.unix` |

### Expression layer (conditionals — hard requirement)

Anywhere a layer field accepts a binding, it also accepts
`{ expr = "..." }`. Supported:

- **Operators:** arithmetic `+ - * / %`, comparison `== != < <= > >=`,
  boolean `&& || !`, ternary `cond ? a : b`, string concat `+`, parens.
- **Literals:** numbers, strings (`'...'` / `"..."`), hex colors
  (`'#rgb'` / `'#rrggbb'`), `true`, `false`, `null`.
- **Identifiers:** any binding namespace shorthand
  (`coolant_c`, `cpu.temp_c`, `gpu.load`, `time.unix`); full path also OK.
- **Whitelisted functions:** `min`, `max`, `clamp`, `abs`, `round`, `floor`,
  `ceil`, `lerp(a,b,t)`, `map(x, in_lo, in_hi, out_lo, out_hi)`,
  `rgb(r,g,b)`, `rgba(r,g,b,a)`, `mix(c1, c2, t)`.

Out: assignments, function definitions, loops, side effects, async, I/O.
Pure functions of bindings → result.

Implementation: `evalexpr` crate, with a custom context that exposes the
binding registry. Fallback to ~300 LOC `chumsky` parser if `evalexpr`'s
color-literal handling proves awkward; choice locked in Phase 0.

Each `expr` is parsed once at theme load into an AST; per-render is a tree
walk over current binding values. Type errors are reported at theme load,
not at render time.

### Renderer

- `tiny-skia` — vector + raster CPU renderer, plenty fast for 480×480 @ 10 Hz.
- `cosmic-text` — text shaping (preferred over `fontdue` for i18n).
- `image` — PNG/JPG asset loading.
- JPEG encoding via the `image` crate at quality ~85, then pushed as a frame
  to `hid_io_task`.
- All rendering on a dedicated single-threaded `LocalSet` per device.

### Animation handling

- `animation = "event"` (default): subscribe to bindings; re-render on
  any change. Idle = zero CPU.
- `animation = "fixed:10_hz"`: theme contains time-driven elements;
  renderer ticks at the requested rate while this theme is active.
- Per-layer `tick = true` flag forces inclusion in the animation tick set
  even when no time binding is detected.

### Hot reload

Daemon watches the active theme's directory via `notify` crate. Saves
trigger a re-render. No GUI editor in v1; users hand-edit TOML.

### Open verification dependency

Canvas size (480×480 is a placeholder), shape (circle vs square), JPEG vs
raw RGB, and chunk size are determined in Phase 0. The manifest schema
does not change when these are confirmed; only the renderer's output
target does.

## 9. GUI client (`lconnect-gui`)

### Stack

- `eframe` + `egui` 0.27 (continues the existing scaffold's framework).
- One shared `tokio` runtime per process.
- The GUI crate does **not** depend on `hidapi`. The buggy
  `lconnect-linux/src/device/galahad.rs` and `test_hid*.rs` are
  archived to `docs/protocol/captures/` as reference for Phase 0 and
  removed from the source tree.

### Threading rules (the lag cure, codified)

1. `update()` is pure and synchronous, microseconds per call. Reads from
   `tokio::sync::watch::Receiver`s via `.borrow()`.
2. Zero HID calls in the GUI process. Enforced by Cargo dependency graph.
3. One background tokio task (`ipc_runtime`) holds a long-lived JSON-RPC
   connection, runs `subscribe_snapshots` for visible devices, pushes
   results into the watch channels the UI reads.
4. `ctx.request_repaint()` is called from the IPC task only when new data
   arrives. No `request_repaint_after(...)` polling.
5. Disconnected daemon is a first-class state, not an error. UI offers
   `systemctl --user start lconnectd` as a one-click recovery.

### Pages (v1)

Single window, sidebar + content:

- **Sensors** — live readouts + 60 s rolling chart (egui `plot`).
- **Fan / Pump** — Manual / Curve mode; slider + point-edit graph;
  debounced apply.
- **Lighting** — mode / color / speed / direction / brightness.
- **LCD** — brightness, rotation, screen on/off, sleep timeout.
- **Theme** — picker (radio list with thumbnails), per-theme parameter form
  for any layer marked `editable = true`.
- **Settings** — socket path, log level, "open config dir".

No tray icon, no theme editor, no system-wide hotkeys in v1.

## 10. CLI client (`lconnectctl`)

Same workspace, separate binary; uses the same `DaemonRpc` trait the GUI
uses. Examples:

```sh
lconnectctl devices               # list attached devices
lconnectctl snapshot              # one-shot sensor read
lconnectctl fan 60                # set fan PWM 60 %
lconnectctl pump 80               # set pump PWM 80 %
lconnectctl theme set coolant-arc # switch theme
lconnectctl theme reload          # re-read manifest
lconnectctl logs --follow         # tail daemon log
lconnectctl raw <hex>             # dev: send raw bytes (Phase 0 only)
```

The CLI is invaluable for testing — every Phase 0 protocol verification
step has a CLI invocation as its smoke test.

## 11. Cargo workspace layout

```
lconnect-linux/
├── Cargo.toml                       # [workspace]
├── README.md
├── docs/
│   ├── protocol/
│   │   ├── galahad2-lcdv2.md        # Phase 0 deliverable
│   │   ├── captures/                # usbmon dumps
│   │   └── decompiled-source-map.md # which C# files map to what
│   ├── architecture.md              # this design, post-implementation
│   └── superpowers/specs/
│       └── 2026-04-25-lconnect-linux-design.md
├── crates/
│   ├── protocol/                    # PURE: types + framing, no I/O
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── galahad2_lcdv2/      # one module per supported SKU
│   │   │   │   ├── mod.rs
│   │   │   │   ├── packets.rs
│   │   │   │   ├── commands.rs
│   │   │   │   ├── handshake.rs
│   │   │   │   ├── lighting.rs
│   │   │   │   ├── lcd.rs
│   │   │   │   └── firmware.rs
│   │   │   └── common/
│   │   │       ├── packet.rs
│   │   │       └── crypto.rs        # iff transport-encrypted
│   │   └── tests/                   # round-trip + golden-byte tests
│   ├── hid/                         # hidapi wrapper, hotplug, retries
│   ├── theme/                       # manifest + expr + render + reload
│   ├── ipc/                         # DaemonRpc trait + JSON-RPC impls
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── jsonrpc/{server,client}.rs
│   │       └── dbus/                # stub crate, behind feature flag
│   ├── host_sensors/                # /sys/class/hwmon, /proc, NVML, …
│   ├── daemon/                      # binary: lconnectd
│   ├── cli/                         # binary: lconnectctl
│   └── gui/                         # binary: lconnect-gui
├── packaging/
│   ├── systemd/lconnectd.service
│   ├── udev/70-lianli-hid.rules
│   ├── arch/PKGBUILD
│   └── desktop/lconnect-gui.desktop
└── xtask/                           # cargo xtask install / verify-protocol
```

Boundary rules (enforced by the Cargo dep graph):

- `protocol/` is pure, no I/O, `no_std`-friendly. Fully unit-testable
  without hardware.
- `hid/` is the only crate that depends on `hidapi-rs`.
- `gui/` and `cli/` depend only on `ipc/`. They cannot do HID I/O.
- `daemon/` is the only crate that wires everything together.

## 12. Packaging

### udev rule — `packaging/udev/70-lianli-hid.rules`

One entry per known VID/PID from `KnownDevices.cs`, generated from a single
table in `xtask`. Phase 1 ships only the LCDV2 entry; later phases add as
they support new devices.

```
# Galahad II LCDV2
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0416", ATTRS{idProduct}=="7395", \
  TAG+="uaccess", MODE="0660", GROUP="plugdev"
```

`TAG+="uaccess"` gives the logged-in user automatic ACL — no group
membership ritual. The existing
`lconnect-linux/99-lian-li-galahad2.rules` is replaced and renamed (the
`99-` priority isn't required).

### systemd — `packaging/systemd/lconnectd.service`

```ini
[Unit]
Description=L-Connect Linux daemon
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/lconnectd
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
```

Install: `systemctl --user enable --now lconnectd`. User-bus only; no
PolicyKit needed.

### Distribution targets (v1)

- AUR PKGBUILD (`packaging/arch/PKGBUILD`) — primary, since user is on
  CachyOS.
- `cargo install --path crates/daemon` etc. for non-Arch users who can
  build from source.
- Tarball release with prebuilt binaries (later, post-v1).
- **No** Flatpak / AppImage in v1 (sandboxing + `hidraw` is fiddly).

## 13. Logging

- `tracing` crate everywhere.
- Daemon writes to `journald` automatically when run under systemd; stdout
  when run manually.
- Levels: `error` (visible to user), `warn` (anomaly), `info` (state
  transitions), `debug` (HID frames), `trace` (every byte).
- `RUST_LOG=lconnectd=debug` toggles verbosity at process start.
- SIGHUP → reload config + log level: stretch goal.

## 14. Testing strategy

| Layer | Style | Hardware needed |
|---|---|---|
| `protocol/` encode/decode | unit tests, table-driven | no |
| `theme/` expr eval, manifest parsing, layer rendering | unit + golden-image tests | no |
| `ipc/` request/response | integration test using in-memory transport | no |
| `hid/` discovery + retries | mock-hid (fake `HidDevice` impl) | no |
| End-to-end verification suite | `cargo xtask verify-protocol` | **yes** |
| GUI | manual smoke test against a stub `DaemonRpc` impl | no |

Most layers reach high confidence without hardware. The verification suite
is the only thing requiring the device; it is run once per protocol change.

## 15. Phase plan

| Phase | Deliverable | HW required | Size |
|---|---|---|---|
| **0. Verify** | `docs/protocol/galahad2-lcdv2.md` (every command annotated, cited, status-flagged); `protocol/` crate encode/decode + tests; `lconnectctl raw` byte-dumper | yes (device + dual-boot capture for LCD) | medium |
| **1. Daemon MVP** | `lconnectd` running under systemd; sensors + fan/pump PWM via JSON-RPC; `lconnectctl`; udev rule installed. **No LCD, no theme, no GUI.** | yes | medium |
| **2. GUI shell** | egui app connecting to daemon; live sensors; fan/pump controls. **Lag-free by construction.** Replaces broken existing app. | yes | small |
| **3. Lighting** | RGB control end-to-end (protocol → daemon → CLI → GUI). | yes | small-medium |
| **4. LCD config + static image** | LCD on/off, brightness, rotation, push-static-JPEG. Validates the chunked-upload protocol. No theme engine yet. | yes | medium |
| **5. Theme engine v1** | TOML themes + `expr` + `tiny-skia` renderer + hot reload + bindings. Ship 2–3 built-in themes. | yes | **large** |
| **6. Polish** | Fan curves UI, theme parameter forms, AUR PKGBUILD, README rewrite, troubleshooting doc. | yes | medium |
| 7. (later) Second product | Pick next device family, repeat 0 → 5 for it. Theme engine, GUI, IPC are reused. | yes | per-product medium |

Each phase ends with a working binary; nothing is "build everything then turn
it on". Each phase gets its own implementation plan via the **writing-plans**
skill when its predecessor lands. The first plan to be drafted is **Phase 0**.

## 16. Open questions / risks

- **Crypto on transport.** If `LConnectCore/Crypto/CryptoHelper.cs` wraps
  packets (rather than just theme files), the protocol crate gains a
  transform layer. Phase 0 verifies this; if true, scope of Phase 0 grows by
  ~30%.
- **LCD frame protocol shape.** Chunking, flow control, and frame headers
  are the highest-risk unknowns. Dual-boot capture is highly recommended
  before implementation.
- **Folder-vs-SKU mapping in decompiled source.** `Products/Galahad2Vision/`
  may implement multiple `ControllerTypeID`s. Phase 0 produces an explicit
  mapping in `docs/protocol/decompiled-source-map.md`.
- **`evalexpr` color-literal handling.** Whether the off-the-shelf crate
  copes with `'#rrggbb'` cleanly or whether we need a hand-rolled parser is
  decided in Phase 0.
- **Firmware-version branching.** `LianLi-GA_II-LCD_v1.6` suggests `v1.6` is
  firmware revision. Other firmwares may have different command layouts.
  Phase 0 documents what we observe; future firmware revisions are a
  follow-up sub-project.
- **`hidapi-rs` blocking on Linux `hidraw`.** `read_timeout` semantics on
  the `hidraw` backend are known to be quirky. The daemon design isolates
  this on `spawn_blocking`, but Phase 1 should benchmark the actual
  behaviour and document any backend-specific workarounds.
