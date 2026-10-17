> Historical design reference. Current scope/order: [roadmap](../../../../roadmap.md); current execution: [plan index](../plans/README.md). Older phase status and architecture must be reconciled with the M1/M2 daemon contracts before implementation.

# Phase 2 — GUI (Tauri 2 + SvelteKit) Design

- **Status:** approved (brainstorming complete) — ready for Phase 2 plan
- **Date:** 2026-04-27
- **Author:** brainstorming session with user
- **Scope:** the L-Connect Linux desktop GUI client, replacing the original
  spec's egui-based "Phase 2 GUI shell" with a Tauri 2 + SvelteKit + Svelte 5
  app aiming for L-Connect-3-equivalent polish using an original "Aurora"
  visual language
- **Supersedes:** Section 9 ("GUI client") and Phase 2 entry of
  [`2026-04-25-lconnect-linux-design.md`](2026-04-25-lconnect-linux-design.md);
  the original committed to `eframe + egui`, now replaced

---

## 1. Context

The original design picked egui for the GUI on grounds of single-language
Rust-everywhere and rapid scaffolding. The user's polish target is closer
to L-Connect 3 (the official Windows app, which itself uses CefSharp /
embedded Chromium) — egui's immediate-mode constraints and built-in widget
look make that target very expensive. A native shell + web UI is the same
paradigm L-Connect 3 already uses, and Tauri 2 + SvelteKit + Svelte 5 is
the modern, Linux-friendly form of it.

Phase 1 (the daemon + CLI + IPC) is currently mid-execution. **Phase 2 GUI
work begins after Phase 1 lands** — it consumes the daemon's stable
JSON-RPC contract over `$XDG_RUNTIME_DIR/lconnectd.sock`. Phase 1 is the
sole producer of HID I/O; the GUI is a stateless client.

Phase numbering after this spec:

- Phase 0a-0e: protocol verification (done)
- Phase 1: daemon + CLI MVP (in progress)
- **Phase 2: GUI (this spec)** — was "egui shell"
- Phase 3: second device family (was "lighting")
- Phase 4: theme engine v1 (was "Phase 5")
- Phase 5: polish (was "Phase 6")

## 2. Scope

### In scope

- `lconnect-gui` Tauri 2 app (Rust shell + SvelteKit/Svelte 5 webview UI)
- Five top-level pages: Dashboard, Cooling, Lighting, Display, Settings
- Live sensor view (1 Hz, push-driven from daemon)
- Fan/pump PWM control with editable curves
- Lighting control: preset modes (cmd 131/133), per-pixel LEDs (cmd 20)
- LCD config + image upload via `lconnect-image`
- Daemon health + log-level adjustment in Settings
- Original "Aurora" design language: cosmos palette, Inter font, glass
  cards, lavender/pink accents
- Handcrafted component layer on Tailwind v4 + `bits-ui` headless primitives
- Tauri events fed by `subscribe_snapshots` for push-driven UI
- AUR PKGBUILD distribution

### Out of scope

- Theme engine UI (Phase 4) — Display page is designed with room for it
  but ships without
- Profiles / saved settings bundles (deferred — daemon already persists
  current state per-device; profiles are a UX layer on top)
- Multi-device UI affordances (Phase 1 supports only LCDV2; UI displays
  one device, Phase 3+ gains a device picker)
- Tray icon, system-wide hotkeys, notifications
- Light theme (Phase 2 ships dark only; light follow-up is polish-tier)
- Internationalization (English only in v1)
- AppImage / Flatpak / `.deb` distribution (AUR-only Phase 2)

### Hardware coverage

Galahad II LCDV2 only — same as Phase 1. The GUI displays a single device
and assumes its presence; "no device" state is handled but not emphasized.

## 3. Six load-bearing design decisions

| Axis | Choice | Rationale |
|---|---|---|
| **UI stack** | Tauri 2 + SvelteKit + Svelte 5 + Tailwind v4 | Polish target requires a real UI toolkit; Tauri 2 is the modern Linux-native shell; Svelte 5 + runes for reactive state; SvelteKit for routing and SSR-ish dev ergonomics |
| **IPC topology** | Tauri commands wrap a Rust `lconnect-ipc` client + Tauri events for push | Reuses Phase 1's IPC client crate verbatim; auto-generated TypeScript bindings via `tauri-specta`; full type safety end-to-end; Svelte components stay simple |
| **Real-time updates** | One long-lived `subscribe_snapshots` per attached device, re-emitted as Tauri events into a Svelte store | Matches the daemon's actor model exactly; populated even on non-Dashboard pages so navigation is instant; cheap per the daemon's existing 1 Hz cadence |
| **Component primitives** | Handcrafted components on Tailwind v4 + `bits-ui` headless lib | Tightest visual control for the Aurora language; smallest dep footprint; the brand identity lives in the last 30% that pre-built libs would fight |
| **Distribution** | AUR PKGBUILD only (Phase 2) | Mirrors Phase 1's primary path; non-Arch users build from source. AppImage / Flatpak deferred to polish-tier; Flatpak particularly fights the `$XDG_RUNTIME_DIR` socket sandbox |
| **Testing** | Vitest (component logic) + Playwright (E2E with mocked daemon via Phase 1's in-memory transport) | Mirrors Phase 1's testing layering; hardware-free for CI |

## 4. App architecture

```
   ┌──────────────────────────────────────────────┐
   │            lconnect-gui (Tauri 2)            │
   │                                              │
   │  ┌──────────────┐    ┌───────────────────┐   │
   │  │  Rust shell  │    │ Svelte 5 webview  │   │
   │  │              │◄──►│                   │   │
   │  │  - Tauri     │    │  - 5 page routes  │   │
   │  │    commands  │    │  - Tauri events   │   │
   │  │  - Tauri     │    │    → stores       │   │
   │  │    events    │    │  - bits-ui +      │   │
   │  │  - lconnect- │    │    Tailwind v4    │   │
   │  │    ipc       │    │                   │   │
   │  │    client    │    │                   │   │
   │  └──────┬───────┘    └───────────────────┘   │
   │         │                                    │
   └─────────┼────────────────────────────────────┘
             │
             │ JSON-RPC 2.0
             │ over Unix socket
             ▼
   ┌──────────────────────────┐
   │     lconnectd            │  (Phase 1)
   │     (daemon)             │
   └──────────────────────────┘
             │
             ▼ hidraw
   ┌──────────────────────────┐
   │  Galahad II LCDV2        │
   └──────────────────────────┘
```

### Tauri shell responsibilities (Rust)

- Hold one `lconnect-ipc::IpcClient` connection to `lconnectd`
- Track attached devices, spawn one `subscribe_snapshots` task per device
- Re-emit each snapshot as a Tauri event scoped by `device_id`
- Translate Tauri commands (typed via `tauri-specta`) into JSON-RPC requests
- Handle daemon disconnects + reconnect with exponential backoff
- Surface daemon connection state as a Tauri event for the UI

### Svelte UI responsibilities (TS)

- Five top-level pages on file-based routes (`/`, `/cooling`, `/lighting`,
  `/display`, `/settings`)
- One reactive store per device, populated from the snapshot Tauri event
- One reactive store for daemon connection state
- All HID-touching actions go through Tauri commands; the UI never
  imports anything that talks to the OS
- Form-state managed locally with Svelte 5 runes; commit on explicit
  action (slider release, button click) — no live-write on every drag

### Three architectural invariants enforced by the dep graph

1. **Svelte side never touches HID.** The Svelte tree depends only on
   `@tauri-apps/api` (for `invoke` / `listen`) — no node-hid, no socket
   client, no daemon imports.
2. **Rust shell never owns persistent state.** All state lives either in
   the daemon (per-device persistence) or in Svelte stores (UI-local).
   The Tauri shell is a stateless adapter.
3. **`tauri-specta` is the type bridge.** Every Tauri command + event has
   a generated TypeScript binding; the Svelte side has zero hand-written
   IPC types.

### Reconnect / startup contract

1. App starts → Tauri shell tries to connect to socket.
2. If daemon unreachable: emit `daemon-state: disconnected`. UI shows
   recovery affordance ("Start lconnectd via systemctl --user start
   lconnectd" with a button that runs the command).
3. On connect → emit `daemon-state: connected`. Read `list_devices`. For
   each: spawn a subscription task.
4. On daemon disconnect: emit `daemon-state: reconnecting`. Retry with
   exponential backoff (1s, 2s, 5s, 10s, 30s, capped). UI degrades
   gracefully — last-known sensor values remain visible but greyed.

## 5. Visual design language ("Aurora")

### Palette

Derived from the user's space/cosmos brief; tokens defined in Tailwind
config and exposed as CSS variables.

```
--bg-deepest:  #060818
--bg-base:     #0d0e2a
--bg-elev:     #1a1740
--bg-glow:     #2a1655        (radial-gradient seed top-right)

--text-base:   #f3eeff
--text-muted:  #b6abd1
--text-faint:  #9484bd
--text-dim:    #6a5fa1

--accent-pink:    #ff9fcd
--accent-pink-hi: #ffd5f0
--accent-lav:     #c4a8ff
--accent-lav-hi:  #d4baff
--accent-violet:  #b18bff
--accent-sky:     #92d4ff       (used sparingly for "info" state only)
--border-subtle:  rgba(255, 200, 255, 0.10)
--border-active:  rgba(255, 164, 209, 0.5)
```

App-level background: radial gradient from `--bg-glow` at 80%/0% fading
into `--bg-base` and finally `--bg-deepest`. Cards use a 180° linear
gradient with low-opacity violet base and `backdrop-filter: blur(8px)`.

### Typography

Inter only (300/400/500/600/700 weights). System font stack as fallback.

- H1 page titles: 22-24px, 600 weight, `letter-spacing: -0.2px`
- Section labels (uppercase): 10-11px, 1.4px tracking, `--text-faint`
- Body: 13px, 400 weight
- Numeric readouts: 28-32px, 600 weight, `font-variant-numeric:
  tabular-nums`

### Layout shell

Two-column `240px sidebar + 1fr content`. Sidebar is glassy
(`backdrop-filter: blur(10px)` over `--bg-base` with low alpha overlay).
Content scrolls vertically; sidebar is fixed.

### Motion

Restrained. Tauri webview is fast; use this budget on:
- 200ms ease for sidebar nav-item active transitions
- Snapshot value updates: no animation by default (let the number tick)
- Page transitions: 150ms cross-fade at most; no slide or zoom

Avoid:
- Continuously-animating background gradients (eats GPU)
- Pulsing/glowing UI elements (that's what gamer-RGB looks like)
- Confetti / spring physics on form actions

### Iconography

Lucide icons, ~16-18px stroke 1.5. Sidebar nav items use icons + text.
No custom icon set in v1.

## 6. Pages

### 6.1 Dashboard (`/`)

Three sensor cards in a 3-column grid: Coolant, Pump RPM, Fan RPM. Each
shows the current value, a sub-line with PWM%/source/etc, and a tiny
60-second sparkline. Below, a single "Quick actions" row: "Open Cooling
page" / "Switch lighting preset" buttons.

State source: `device.snapshot` Svelte store.

### 6.2 Cooling (`/cooling`)

Two side-by-side cards: Fan, Pump. Each card holds:
- Current RPM + PWM% readout (large, tabular)
- Editable curve: SVG with draggable control points; X-axis = coolant
  temp 20-70 °C, Y-axis = PWM 0-100%
- Manual override slider (used when curve is "Manual" mode)
- For pump only: MCU / Application source toggle

Curve editing is local (Svelte runes). "Apply" button commits to daemon
via `set_fan_pwm` / `set_pump_pwm` (Phase 1 doesn't have curve upload —
the GUI computes effective PWM from the curve at the current temp and
sends a single PWM value, debounced 300ms; full curve upload waits for a
later phase).

### 6.3 Lighting (`/lighting`)

Tab toggle: Pump / Fan (changes between cmd 131 vs cmd 133 surface).
For Pump: scope toggle (Inner / Outer / All).

Mode chip grid: all modes from `get_lighting_info` for the selected
target. Selecting a mode shows the relevant controls below (color picker,
brightness, speed, direction). Color picker is a custom component — no
HSV-wheel mess; just a hex input + a sample square + 8 swatch presets
drawn from the Aurora palette + custom colors the user has used before.

Apply button commits via `set_lighting`.

A separate "LED ring" panel below the preset-mode area exposes per-pixel
mode (cmd 20) — this is the screen LED ring, not preset modes. v1 ships
a flat color picker that drives all LEDs to one color via
`set_led_colors`. Multi-LED gradient or pattern editing is Phase 4
theme-engine territory.

### 6.4 Display (`/display`)

Phase 2 surface:
- 480×480 round preview of current LCD content (rendered from the JPEG
  the daemon last pushed — daemon should expose `get_lcd_image()` or the
  GUI keeps a copy of what it set)
- Drop zone for image upload (PNG/JPEG/WebP/BMP/GIF)
- `lconnect-image` runs in the Rust shell to convert to 480×480 JPEG;
  result sent via `set_lcd_image`
- Brightness slider (0-100%)
- Rotation toggle (0°/90°/180°/270°)
- Mode dropdown (LocalUI / Application — Phase 2 only exposes the safe
  subset; LocalAVI/LocalH264 hidden until verified)

**Phase 4 expansion (foreshadowed in design):** the right side of the
Display page becomes a theme editor — sidebar of widget types (clock,
sensor, image, arc, text, chart) on the left of the content area, theme
canvas in the center, properties panel on the right. The Aurora layout
(2-col shell) leaves room for that without redesign — the inner "card"
content swaps from "drop zone + sliders" to the editor.

### 6.5 Settings (`/settings`)

Sections:
- **Daemon** — connection state, socket path (read-only), version, log
  level dropdown (Error/Warn/Info/Debug/Trace; calls a tiny IPC
  extension `set_log_level`)
- **About** — version, contributors, license link
- **Reset device** — button with two-step confirm (token gate from
  Phase 1)
- **Open config dir** — button that opens `$XDG_CONFIG_HOME/lconnectd`
  in the user's file manager

No theme switching, no language picker, no keyboard shortcuts editor in
v1.

## 7. Tauri command surface

One Tauri command per Phase 1 `DaemonRpc` method, plus a few GUI-only
helpers. All auto-generated TypeScript types via `tauri-specta`.

```rust
// crates/lconnect-gui/src-tauri/src/commands.rs (sketch)

#[tauri::command]
#[specta::specta]
async fn list_devices(rpc: State<RpcClient>) -> Result<Vec<DeviceDescriptor>, IpcError>;

#[tauri::command]
#[specta::specta]
async fn get_snapshot(rpc: State<RpcClient>, device: DeviceId) -> Result<SensorSnapshot, IpcError>;

#[tauri::command]
#[specta::specta]
async fn set_fan_pwm(rpc: State<RpcClient>, device: DeviceId, percent: u8) -> Result<(), IpcError>;

// ... one per Phase 1 IPC method: set_pump_pwm, set_lighting, set_led_colors,
//     set_lcd_config, set_lcd_image, clear_lcd, request_reset_token,
//     reset_device, get_firmware, is_lcd_available, daemon_stats, etc.

// GUI-only helpers:
#[tauri::command]
#[specta::specta]
async fn prepare_jpeg(path: PathBuf, fit: FitMode, quality: u8) -> Result<Vec<u8>, ImageError>;
// runs lconnect-image in the Rust shell; returns ready-to-upload JPEG bytes

#[tauri::command]
#[specta::specta]
async fn open_path(path: PathBuf) -> Result<(), String>;
// xdg-open wrapper for "Open config dir" button

#[tauri::command]
#[specta::specta]
async fn daemon_connection_state() -> DaemonConnectionState;
// "Connected" | "Connecting" | "Reconnecting" | "Disconnected"
```

### Tauri events (server → client push)

- `daemon-state` — `DaemonConnectionState` payload
- `device-attached` — `DeviceDescriptor`
- `device-detached` — `DeviceId`
- `snapshot:<device-id>` — `SensorSnapshot` (one event channel per device)
- `device-error` — `{ device_id, message }` for transient errors

## 8. Workspace layout

The GUI is a workspace addition. Rust shell + Svelte UI live together
under `crates/lconnect-gui/`.

```
lconnect-linux/
├── Cargo.toml                    workspace adds lconnect-gui
├── crates/
│   ├── protocol/                 (existing)
│   ├── lconnect-image/           (existing)
│   ├── lconnect-hid/             (Phase 1)
│   ├── lconnect-ipc/             (Phase 1) — consumed by GUI
│   ├── lconnect-daemon/          (Phase 1)
│   ├── lconnect-cli/             (Phase 1)
│   └── lconnect-gui/             NEW
│       ├── Cargo.toml            (Tauri Rust shell)
│       ├── package.json          (SvelteKit + Vite)
│       ├── svelte.config.js
│       ├── tailwind.config.ts
│       ├── tsconfig.json
│       ├── playwright.config.ts
│       ├── vitest.config.ts
│       ├── tauri.conf.json
│       ├── src/                  Svelte source
│       │   ├── app.html
│       │   ├── app.css           Tailwind v4 + Aurora tokens
│       │   ├── lib/
│       │   │   ├── api/          tauri-specta-generated bindings
│       │   │   ├── stores/       device.ts, daemon.ts, ui.ts
│       │   │   ├── components/   handcrafted Aurora primitives
│       │   │   │   ├── Card.svelte
│       │   │   │   ├── Slider.svelte
│       │   │   │   ├── TogglePill.svelte
│       │   │   │   ├── ColorPicker.svelte
│       │   │   │   ├── ModeChipGrid.svelte
│       │   │   │   ├── CurveEditor.svelte
│       │   │   │   ├── DropZone.svelte
│       │   │   │   ├── DeviceCard.svelte
│       │   │   │   └── SidebarNav.svelte
│       │   │   └── format/       formatters (rpm, temp, hex)
│       │   └── routes/
│       │       ├── +layout.svelte         shell + sidebar
│       │       ├── +page.svelte           Dashboard
│       │       ├── cooling/+page.svelte
│       │       ├── lighting/+page.svelte
│       │       ├── display/+page.svelte
│       │       └── settings/+page.svelte
│       ├── src-tauri/            Rust shell
│       │   ├── Cargo.toml
│       │   ├── tauri.conf.json
│       │   └── src/
│       │       ├── main.rs
│       │       ├── commands.rs
│       │       ├── events.rs
│       │       ├── ipc_state.rs  the long-lived IpcClient + subs
│       │       └── reconnect.rs  exponential-backoff reconnect logic
│       └── tests/
│           ├── unit/             Vitest
│           └── e2e/              Playwright (against mocked daemon)
└── packaging/
    └── arch/
        └── PKGBUILD              extends Phase 1's PKGBUILD with lconnect-gui
```

### Boundary rules (Cargo + tsconfig)

- `lconnect-gui/src-tauri` depends on `lconnect-ipc`, `lconnect-image`,
  `tauri`, `tauri-specta`, `serde`, `tokio`. **Never** on `lconnect-hid`
  (the daemon mediates HID).
- Svelte side has no Cargo deps. Imports from `@tauri-apps/api` and
  `bits-ui` only (plus generated `lib/api/` bindings).

## 9. Component library — handcrafted primitives

Phase 2 builds these Svelte 5 components on top of `bits-ui`:

| Component | Backed by | Notes |
|---|---|---|
| `Card.svelte` | div | Aurora glass card; takes `--variant` for accent |
| `Button.svelte` | bits-ui Button | three variants: primary (gradient), secondary (outline), ghost |
| `Slider.svelte` | bits-ui Slider | gradient fill track, glow thumb |
| `TogglePill.svelte` | bits-ui ToggleGroup | the pill-style segmented control from mockups |
| `ModeChipGrid.svelte` | div + button | grid of selectable mode chips |
| `ColorPicker.svelte` | bits-ui Popover + custom HSL | hex input + swatch grid + recently-used row |
| `CurveEditor.svelte` | SVG + custom drag | draggable control points on a polyline; emits `change` with point array |
| `DropZone.svelte` | input[type=file] | DnD + click-to-browse; emits `file` event |
| `SidebarNav.svelte` | nav + a | sidebar with Lucide icon + label items |
| `Sparkline.svelte` | SVG | tiny trend line for Dashboard cards |
| `DeviceCard.svelte` | div | device summary tile (Phase 3+ multi-device) |

`bits-ui` handles accessibility (focus rings, keyboard nav, ARIA roles)
on the interactive primitives. We add the Aurora visual layer on top.

## 10. State management — Svelte 5 runes + stores

- **Per-device state:** one `$state` object per device, structured as
  `{ snapshot, settings, status }`. Updated by the snapshot Tauri event.
- **Daemon state:** one `$state` for connection status + version.
- **UI-local state:** form drafts (slider values mid-drag, curve points
  being edited) live in component-scoped `$state`. They commit via Tauri
  commands on explicit user action.
- **Persistent UI state:** sidebar collapsed/expanded, current page —
  stored in `localStorage` via SvelteKit's `$page` and a small wrapper.

No Svelte 4-style `writable` stores in new code. Use Svelte 5 runes
universally.

## 11. Build + dev workflow

### Dev mode

```bash
cd crates/lconnect-gui
pnpm install
pnpm tauri dev
# - Vite serves Svelte at localhost:1420 with HMR
# - Tauri shell connects to the running lconnectd
# - Or to a mock daemon: pnpm tauri dev:mock
```

`dev:mock` uses an in-memory daemon (`lconnect-ipc::transport_memory` from
Phase 1) seeded with a fake device. Lets the GUI develop without a real
LCDV2 attached.

### Production build

```bash
pnpm tauri build
# Produces:
# - target/release/lconnect-gui (binary)
# - target/release/bundle/appimage/ (skipped in v1)
# - target/release/bundle/deb/      (skipped in v1)
```

The AUR PKGBUILD picks up the binary and installs it to
`/usr/bin/lconnect-gui`.

## 12. Packaging

### AUR PKGBUILD

Phase 2 extends Phase 1's `packaging/arch/PKGBUILD`:

```bash
# additions
makedepends=(... 'pnpm' 'nodejs' 'webkit2gtk-4.1')
depends=(... 'gtk3' 'webkit2gtk-4.1' 'libappindicator-gtk3')

build() {
  # Phase 1 build steps unchanged
  cargo build --release --workspace --bin lconnectd --bin lconnectctl --bin verify-galahad

  # Phase 2 GUI build
  cd crates/lconnect-gui
  pnpm install --frozen-lockfile
  pnpm tauri build
}

package() {
  # Phase 1 install steps unchanged
  install -Dm755 target/release/lconnect-gui "$pkgdir/usr/bin/lconnect-gui"

  # Desktop entry
  install -Dm644 packaging/desktop/lconnect-gui.desktop \
    "$pkgdir/usr/share/applications/lconnect-gui.desktop"

  # Icon
  install -Dm644 packaging/icons/lconnect-gui.png \
    "$pkgdir/usr/share/icons/hicolor/256x256/apps/lconnect-gui.png"
}
```

### Desktop entry

```ini
[Desktop Entry]
Name=L-Connect Linux
Comment=Control panel for Lian Li hardware
Exec=lconnect-gui
Icon=lconnect-gui
Terminal=false
Type=Application
Categories=System;HardwareSettings;
```

## 13. Observability

- Tauri shell uses the same `tracing` setup as the daemon. Logs to stderr
  in dev mode; in production the GUI app's stderr is captured by systemd
  if launched as a user service, otherwise lost (acceptable — daemon is
  the system-of-record for state).
- `RUST_LOG=lconnect_gui=debug pnpm tauri dev` raises shell verbosity
- Webview console is accessible via right-click → Inspect Element in dev
  builds; disabled in release builds via `tauri.conf.json`

## 14. Testing strategy

| Layer | Style | Hardware |
|---|---|---|
| Svelte stores + formatters | Vitest unit | no |
| Components in isolation | Vitest + Svelte Testing Library | no |
| Tauri command bridge | Vitest mocking `@tauri-apps/api` | no |
| End-to-end | Playwright against assembled Tauri app + mocked daemon | no |
| Live device verification | manual smoke run | **yes** |

The end-to-end suite uses `lconnect-ipc::transport_memory` from Phase 1
to spin up a fake daemon in-process; Playwright drives the Tauri app and
asserts that user actions produce the expected `set_*` calls in the
mock. No hidapi dependency anywhere in CI.

## 15. Acceptance criteria

- [ ] `pnpm tauri build` succeeds; binary launches and connects to a
  running `lconnectd` on first run
- [ ] All 5 pages render with the Aurora design language without layout
  bugs at 1280×720, 1600×900, 1920×1080
- [ ] Dashboard shows live coolant temp updating at 1 Hz when daemon is
  attached
- [ ] Cooling page: drag-edit fan curve, release, verify daemon receives
  the resulting PWM via journalctl
- [ ] Cooling page: pump source toggle (MCU vs Application) sends the
  expected `set_pump_pwm` with the right `source` field
- [ ] Lighting page: pick a mode + color, click Apply, see the LED on
  the device change
- [ ] Lighting page: per-pixel LED ring color picker drives `set_led_colors`
- [ ] Display page: drop a JPEG, see it on the LCD within 2 seconds; it
  stays for 60+ seconds (refresh loop alive in daemon)
- [ ] Display page: brightness slider commits `set_lcd_config`; rotation
  toggle commits `set_lcd_config`
- [ ] Settings page: log level dropdown changes daemon log level
- [ ] Settings page: Reset device button shows two-step confirm; on
  confirm, device drops + reappears as expected
- [ ] Daemon disconnect → UI shows reconnecting state; reconnect →
  state restored without app restart
- [ ] `cargo build --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --workspace`
  all clean (Rust shell)
- [ ] `pnpm lint`, `pnpm test`, `pnpm test:e2e` all clean (Svelte side)
- [ ] AUR PKGBUILD builds successfully on a clean Arch container

## 16. Risks & open questions

1. **`lconnect-image` in Tauri Rust vs WASM-in-webview** — Phase 2 plans
   to call `prepare_jpeg` as a Tauri command, running the existing Rust
   crate in the shell. If image conversion ever gets slow enough to
   block the UI, we move it to a `tauri::async_runtime::spawn_blocking`.
   Not a real concern for 480×480 JPEG.
2. **Curve upload protocol** — Phase 1 doesn't have a fan-curve-upload
   IPC method. Phase 2 GUI computes effective PWM client-side from the
   curve at the current temp and sends a single `set_fan_pwm`. This is
   coarser than what L-Connect 3 does (which uploads the curve to the
   firmware). Acceptable for v1; full curve upload is a Phase 1.x
   addition.
3. **Per-pixel LED multi-color editing** — v1 ships only a single-color
   picker for the LED ring (cmd 20 with one color repeated across LEDs).
   Multi-LED gradient or pattern editing is Phase 4 theme-engine
   territory.
4. **Reconnect storm** — if the daemon crashes-and-restarts repeatedly,
   the GUI's exponential-backoff caps at 30s. After 5 failed attempts
   the UI offers a "Stop trying" button.
5. **Webview rendering on older WebKitGTK** — Tauri 2 requires
   `webkit2gtk-4.1`. CachyOS / Arch are fine; Debian-stable users on the
   future `.deb` path may need backports. Document in v2 polish phase.
6. **Bundle size** — Tauri app target is 10-15 MB compressed; SvelteKit
   build with no SSR is small, Tailwind v4 ships only used utilities.
   Compares well to L-Connect 3's much larger footprint.

## 17. Phase 4 readiness — Display tab evolution

The Display page is **the entry point for Phase 4's theme engine**. The
Aurora layout shell leaves room for that without redesign:

- Phase 2: content area = "drop zone + brightness/rotation sliders"
- Phase 4: content area becomes a theme editor — left rail of widget
  types, center canvas (480×480), right properties panel. The
  outer shell (sidebar + Display nav item) is unchanged.

The IPC seam for theme content is the same `set_lcd_image` from Phase 2.
Phase 4's theme renderer produces JPEGs at the canvas refresh rate; the
daemon's `lcd_keeper` already handles the multi-rate write loop.

The Phase 2 plan should keep `Display.svelte` modest in scope and
factor any reusable canvas / preview / image-pipeline components into
`lib/components/` so Phase 4 inherits them.

## 18. Phase 2 → Phase 3 / Phase 4 / Phase 5 handoffs

- **Phase 3 (second device family)** inherits the device picker code
  path. Phase 2 ships with the device-picker designed for N≥1 but
  with N=1 always (single LCDV2). The sidebar / device card system
  scales without redesign.
- **Phase 4 (theme engine)** inherits the Display page shell as
  described in §17.
- **Phase 5 (polish)** picks up: light theme, AppImage / Flatpak / .deb,
  internationalization, tray icon, fan curve upload protocol.
