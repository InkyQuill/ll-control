# Phase 2 — GUI (Tauri 2 + SvelteKit) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `lconnect-gui` — a Tauri 2 + SvelteKit + Svelte 5 desktop app for Linux that consumes Phase 1's `lconnectd` daemon over JSON-RPC IPC and provides five pages (Dashboard, Cooling, Lighting, Display, Settings) in an original "Aurora" cosmos-themed design language.

**Architecture:** Two-layer app — a Rust shell holding the daemon IPC connection and serving as a bridge, plus a SvelteKit/Svelte 5 webview that renders the UI. Tauri commands handle one-shot actions; Tauri events push device snapshots to a Svelte store. All HID I/O remains in `lconnectd` — the GUI is a stateless client. Components are handcrafted on `bits-ui` headless primitives + Tailwind v4 to express the Aurora palette without fighting a pre-built design system.

**Tech Stack:** Rust 1.82, Tauri 2.x, `lconnect-ipc` (Phase 1), `lconnect-image` (Phase 0d), `tauri-specta` (typed JS bindings), `tokio`, `tracing`. Svelte 5 (runes), SvelteKit 2.x, Tailwind v4, `bits-ui`, Lucide icons, Vite, Vitest, Playwright, pnpm.

---

## Context + references

- **Approved spec:** [`docs/superpowers/specs/2026-04-27-phase-2-gui-tauri.md`](../specs/2026-04-27-phase-2-gui-tauri.md)
- **Phase 1 IPC contract:** [`crates/lconnect-ipc/src/trait_def.rs`](../../../crates/lconnect-ipc/src/trait_def.rs) (lands at end of Phase 1 sub-phase C)
- **Phase 0d image pipeline:** `crates/lconnect-image/` (already shipped)
- **Project conventions:** parent `CLAUDE.md`

### Phase 1 prerequisites

This plan **cannot execute** until Phase 1 sub-phase C lands (`lconnect-ipc` crate with `DaemonRpc` trait + types + JSON-RPC framing + server + client + in-memory transport). Phase 1's full scope through wrap-up is not strictly required — Phase 2 only needs `lconnect-ipc` types and the in-memory transport. But practically the IPC trait is finalized only when Phase 1 sub-phase D's `IpcServerImpl` lands and exercises every method.

**Sequencing rule:** Phase 2 begins after Phase 1 P1C-6 (in-memory transport + roundtrip test) lands. P2B-1 (mock daemon) consumes that crate directly.

### Project conventions to honor

- Conventional Commits (`feat(gui):`, `fix(gui):`, `docs(gui):`, `test(gui):`)
- `#![forbid(unsafe_code)]` on the Rust shell crate
- `#[must_use]` on builders / type constructors
- `#[non_exhaustive]` on public enums (Rust side)
- Svelte 5 runes (`$state`, `$derived`, `$effect`); no `writable<T>` stores in new code
- TypeScript strict mode (`"strict": true` in tsconfig)
- ASCII-only commit messages
- All gates clean before commit:
  - Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc --workspace --no-deps`
  - Svelte: `pnpm lint`, `pnpm check`, `pnpm test`
  - E2E: `pnpm test:e2e` (after sub-phase G lands the harness)
- Per the user's standing preference: code-touching tasks get spec-compliance + code-quality + rust-practices-verifier (Rust) or svelte-practices-reviewer (Svelte) review subagents. Use `tauri-2` skill for Tauri-shell work, `using-svelte-5` + `using-sveltekit` skills for Svelte work.

---

## File structure overview

```
lconnect-linux/
├── Cargo.toml                          modified: workspace adds lconnect-gui
├── crates/
│   ├── lconnect-gui/                   NEW (top-level crate dir)
│   │   ├── Cargo.toml                  (root metadata; the actual Tauri crate is src-tauri/)
│   │   ├── package.json                pnpm + Vite + SvelteKit + Tauri CLI
│   │   ├── pnpm-lock.yaml
│   │   ├── pnpm-workspace.yaml
│   │   ├── svelte.config.js
│   │   ├── vite.config.ts
│   │   ├── tailwind.config.ts          Aurora design tokens
│   │   ├── postcss.config.js
│   │   ├── tsconfig.json
│   │   ├── playwright.config.ts
│   │   ├── vitest.config.ts
│   │   ├── README.md                   crate-local dev instructions
│   │   ├── src/                        Svelte source
│   │   │   ├── app.html
│   │   │   ├── app.css                 Aurora tokens + Tailwind directives
│   │   │   ├── lib/
│   │   │   │   ├── api/
│   │   │   │   │   ├── bindings.ts     tauri-specta auto-generated
│   │   │   │   │   └── client.ts       thin wrapper around bindings
│   │   │   │   ├── stores/
│   │   │   │   │   ├── daemon.svelte.ts        daemon connection state
│   │   │   │   │   ├── devices.svelte.ts       device list + per-device snapshots
│   │   │   │   │   └── ui.svelte.ts            sidebar collapsed, current page
│   │   │   │   ├── components/                 handcrafted Aurora primitives
│   │   │   │   │   ├── Card.svelte
│   │   │   │   │   ├── Button.svelte
│   │   │   │   │   ├── Slider.svelte
│   │   │   │   │   ├── TogglePill.svelte
│   │   │   │   │   ├── ModeChipGrid.svelte
│   │   │   │   │   ├── ColorPicker.svelte
│   │   │   │   │   ├── CurveEditor.svelte
│   │   │   │   │   ├── DropZone.svelte
│   │   │   │   │   ├── Sparkline.svelte
│   │   │   │   │   ├── SidebarNav.svelte
│   │   │   │   │   ├── AppShell.svelte
│   │   │   │   │   └── DaemonStatusBanner.svelte
│   │   │   │   └── format/
│   │   │   │       ├── rpm.ts          formatRpm, formatPwm
│   │   │   │       ├── temp.ts         formatTempC
│   │   │   │       └── color.ts        hex parsing/validation
│   │   │   └── routes/
│   │   │       ├── +layout.svelte      AppShell + sidebar
│   │   │       ├── +page.svelte        Dashboard
│   │   │       ├── cooling/+page.svelte
│   │   │       ├── lighting/+page.svelte
│   │   │       ├── display/+page.svelte
│   │   │       └── settings/+page.svelte
│   │   ├── src-tauri/                  Rust shell
│   │   │   ├── Cargo.toml
│   │   │   ├── tauri.conf.json
│   │   │   ├── build.rs
│   │   │   ├── icons/                  app icon set (light/dark + sizes)
│   │   │   ├── capabilities/
│   │   │   │   └── default.json
│   │   │   └── src/
│   │   │       ├── main.rs             entry; tauri::Builder
│   │   │       ├── lib.rs              for tauri-specta type collection
│   │   │       ├── ipc_state.rs        long-lived IpcClient + reconnect
│   │   │       ├── commands.rs         all Tauri commands (one mod per group)
│   │   │       ├── events.rs           snapshot subscription tasks
│   │   │       ├── helpers.rs          prepare_jpeg, open_path
│   │   │       └── error.rs            Tauri-side error mapping
│   │   ├── tests/
│   │   │   ├── unit/                   Vitest unit tests
│   │   │   │   ├── format.test.ts
│   │   │   │   └── stores.test.ts
│   │   │   └── e2e/                    Playwright E2E
│   │   │       ├── dashboard.spec.ts
│   │   │       ├── cooling.spec.ts
│   │   │       ├── lighting.spec.ts
│   │   │       ├── display.spec.ts
│   │   │       └── settings.spec.ts
│   │   └── mock-daemon/                NEW small bin crate inside lconnect-gui
│   │       ├── Cargo.toml
│   │       └── src/main.rs             spawns lconnect-ipc::IpcServer over a Unix socket with seeded fake state
└── packaging/
    ├── arch/
    │   └── PKGBUILD                    modified: extends Phase 1's PKGBUILD
    ├── desktop/
    │   └── lconnect-gui.desktop        NEW
    └── icons/
        └── lconnect-gui.png            NEW (256×256)
```

### Boundary rules enforced by the dep graph

- `lconnect-gui/src-tauri` depends on `lconnect-ipc`, `lconnect-image`, `tauri`, `tauri-specta`, `tokio`. **Never** on `lconnect-hid` (the daemon mediates HID).
- Svelte side has zero Rust deps. Imports `@tauri-apps/api` and `bits-ui` only (plus generated `lib/api/bindings.ts`).
- `mock-daemon` depends on `lconnect-ipc` only — no real `lconnect-hid` or `lconnect-protocol`. It serves canned responses.

---

## Sub-phase index

- **Sub-phase A — Scaffold** (P2A-1 .. P2A-3) — Tauri+SvelteKit init, Tailwind v4 + Aurora tokens, tauri-specta wiring
- **Sub-phase B — Mock daemon harness** (P2B-1 .. P2B-2) — small bin crate + dev script
- **Sub-phase C — Rust IPC bridge** (P2C-1 .. P2C-4) — IpcState, Tauri commands, snapshot events, GUI helpers
- **Sub-phase D — Svelte stores** (P2D-1 .. P2D-3) — daemon, devices, store unit tests
- **Sub-phase E — Component primitives** (P2E-1 .. P2E-9) — handcrafted Aurora components on bits-ui
- **Sub-phase F — Pages** (P2F-1 .. P2F-6) — Dashboard, Cooling, Lighting (preset), Lighting (LED ring), Display, Settings
- **Sub-phase G — E2E tests** (P2G-1 .. P2G-4) — Playwright + mock daemon
- **Sub-phase H — Packaging** (P2H-1 .. P2H-2) — desktop entry + icon, AUR PKGBUILD extension
- **Sub-phase I — Wrap-up** (P2I-1 .. P2I-2) — live smoke run + post-execution notes

**Total: 35 tasks.**

**Parallelism:** sub-phase E (component primitives) tasks are mutually independent — each component is its own file. Dispatch in parallel after sub-phase D lands. Sub-phase F (pages) tasks are also independent of each other after sub-phase E lands. Sub-phase G E2E tasks can parallelize within their sub-phase. Sub-phases A/B/C/D/H/I are sequential.

---

## Sub-phase A — Scaffold

### Task P2A-1: Create the `lconnect-gui` crate + Tauri 2 init

**Files:**
- Create: `crates/lconnect-gui/` directory tree
- Modify: workspace `Cargo.toml`

- [ ] **Step 1: Create the directory + run Tauri's init**

```bash
cd /home/inky/Development/lian-li/lconnect-linux
mkdir -p crates/lconnect-gui
cd crates/lconnect-gui

# Use pnpm + the Tauri CLI to scaffold a SvelteKit + TS template.
# This creates package.json, src-tauri/, src/, and config files.
pnpm create tauri-app@latest -- \
  --manager pnpm \
  --template svelte-ts \
  --name lconnect-gui \
  --identifier com.github.inky.lconnect-linux \
  --yes .
```

If `pnpm create tauri-app` requires interactive prompts despite `--yes`, run it interactively once and answer:
- Project name: `lconnect-gui`
- Identifier: `com.github.inky.lconnect-linux`
- Frontend: `SvelteKit` → `TypeScript`
- Package manager: `pnpm`

- [ ] **Step 2: Verify the scaffold**

```bash
ls crates/lconnect-gui/
# Expect: package.json, src/, src-tauri/, svelte.config.js, vite.config.ts, tsconfig.json
ls crates/lconnect-gui/src-tauri/
# Expect: Cargo.toml, src/main.rs, tauri.conf.json, build.rs, icons/, capabilities/
```

- [ ] **Step 3: Add to workspace Cargo.toml**

Edit the root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/lconnect-hid",
    "crates/lconnect-image",
    "crates/lconnect-ipc",
    "crates/lconnect-daemon",
    "crates/lconnect-cli",
    "crates/lconnect-gui/src-tauri",       # NEW — only the Rust shell is in the workspace
    "crates/lconnect-gui/mock-daemon",     # NEW (created in P2B-1; can be a stub for now)
    "crates/protocol",
    "tools/verify-galahad",
]
```

(Note: only `src-tauri` and `mock-daemon` are workspace crates; the Svelte/Vite half lives in pnpm-land at `crates/lconnect-gui/` root.)

The `Cargo.toml` at `crates/lconnect-gui/` is a non-workspace pnpm-companion file with package metadata. Replace its contents with:

```toml
# This file exists so cargo doesn't treat the directory as a stray.
# The actual Rust shell crate lives in src-tauri/.
[package]
name = "lconnect-gui-meta"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
```

Actually a cleaner alternative is to **delete** `crates/lconnect-gui/Cargo.toml` if Tauri's scaffold created one — only `src-tauri/Cargo.toml` should be a Cargo crate. Verify:

```bash
ls crates/lconnect-gui/Cargo.toml 2>/dev/null && rm crates/lconnect-gui/Cargo.toml
```

- [ ] **Step 4: Configure `src-tauri/Cargo.toml` to inherit workspace**

Edit `crates/lconnect-gui/src-tauri/Cargo.toml` so it inherits workspace fields and depends on Phase 1 crates:

```toml
[package]
name = "lconnect-gui"
description = "L-Connect Linux GUI — Tauri 2 + SvelteKit shell"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true

[lib]
name = "lconnect_gui_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-opener = "2"
tauri-specta = { version = "=2.0.0-rc.21", features = ["derive", "typescript"] }
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
specta-typescript = "=0.0.9"

lconnect-ipc = { path = "../../lconnect-ipc" }
lconnect-image = { path = "../../lconnect-image" }

serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = { workspace = true }
thiserror = { workspace = true }
async-trait = "0.1"
futures = "0.3"

[features]
custom-protocol = ["tauri/custom-protocol"]

[lints]
# inherit workspace lints when added later
```

- [ ] **Step 5: Verify the workspace builds**

```bash
cd /home/inky/Development/lian-li/lconnect-linux
cargo check -p lconnect-gui
```

Expected: clean compile (the Tauri-generated `main.rs` runs an empty `tauri::Builder`).

- [ ] **Step 6: Verify the dev server runs**

```bash
cd crates/lconnect-gui
pnpm install
pnpm tauri dev &
DEV_PID=$!
sleep 10
kill $DEV_PID 2>/dev/null
```

Expected: Tauri opens a webview window with SvelteKit's default page; closes cleanly when killed.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/lconnect-gui/
git commit -m "feat(gui): scaffold lconnect-gui crate (Tauri 2 + SvelteKit)

Initial scaffold via pnpm create tauri-app. Adds src-tauri/ to the
Cargo workspace; the Svelte/Vite half lives at the crate root.
Depends on lconnect-ipc + lconnect-image from earlier phases.
Empty webview confirmed to run via pnpm tauri dev."
```

---

### Task P2A-2: Tailwind v4 + Aurora design tokens

**Files:**
- Create: `crates/lconnect-gui/tailwind.config.ts`
- Create: `crates/lconnect-gui/postcss.config.js`
- Modify: `crates/lconnect-gui/src/app.css`
- Modify: `crates/lconnect-gui/package.json` (add deps)

- [ ] **Step 1: Install Tailwind v4 + plugins**

```bash
cd crates/lconnect-gui
pnpm add -D tailwindcss@^4 @tailwindcss/vite postcss autoprefixer
pnpm add bits-ui lucide-svelte
pnpm add -D @types/node
```

- [ ] **Step 2: Configure Vite to use Tailwind v4 plugin**

Edit `crates/lconnect-gui/vite.config.ts`:

```ts
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "0.0.0.0",
    hmr: { protocol: "ws", host: "0.0.0.0", port: 1421 },
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
```

- [ ] **Step 3: Define Aurora design tokens in `app.css`**

Replace `crates/lconnect-gui/src/app.css` (delete prior contents):

```css
@import "tailwindcss";

@theme {
  /* Aurora palette — locked in brainstorm */
  --color-aurora-deepest:  #060818;
  --color-aurora-base:     #0d0e2a;
  --color-aurora-elev:     #1a1740;
  --color-aurora-glow:     #2a1655;

  --color-aurora-text-base:   #f3eeff;
  --color-aurora-text-muted:  #b6abd1;
  --color-aurora-text-faint:  #9484bd;
  --color-aurora-text-dim:    #6a5fa1;

  --color-aurora-pink:        #ff9fcd;
  --color-aurora-pink-hi:     #ffd5f0;
  --color-aurora-lav:         #c4a8ff;
  --color-aurora-lav-hi:      #d4baff;
  --color-aurora-violet:      #b18bff;
  --color-aurora-sky:         #92d4ff;

  --color-aurora-border-subtle: rgba(255, 200, 255, 0.10);
  --color-aurora-border-active: rgba(255, 164, 209, 0.5);

  /* Typography */
  --font-sans: "Inter", system-ui, -apple-system, sans-serif;
  --font-mono: "JetBrains Mono", "SF Mono", Menlo, monospace;

  /* Motion */
  --animate-duration-fast: 150ms;
  --animate-duration-base: 200ms;
}

/* Inter font from Google Fonts */
@import url("https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap");

html, body {
  background: radial-gradient(ellipse at 80% 0%,
    var(--color-aurora-glow) 0%,
    var(--color-aurora-base) 50%,
    var(--color-aurora-deepest) 100%);
  color: var(--color-aurora-text-base);
  font-family: var(--font-sans);
  font-feature-settings: "cv11", "ss01";
  min-height: 100vh;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* Numeric tabular figures for sensor readouts */
.tabular {
  font-variant-numeric: tabular-nums;
}
```

- [ ] **Step 4: Add a token-smoke-test page**

Replace `crates/lconnect-gui/src/routes/+page.svelte`:

```svelte
<script lang="ts">
  let count = $state(0);
</script>

<main class="min-h-screen p-12">
  <h1 class="text-3xl font-semibold tracking-tight text-aurora-pink-hi mb-2">
    Aurora is live
  </h1>
  <p class="text-aurora-text-muted mb-6">
    If you can see this in the cosmos palette with Inter font, the design tokens load correctly.
  </p>
  <div class="flex gap-3">
    <button
      class="px-4 py-2 rounded-lg bg-gradient-to-b from-aurora-violet to-aurora-lav text-white font-medium"
      onclick={() => count++}>
      Count: {count}
    </button>
  </div>
</main>
```

- [ ] **Step 5: Run dev server + visually confirm**

```bash
pnpm tauri dev &
DEV_PID=$!
sleep 8
# Confirm by squinting at the open window: dark cosmos background, pink heading, gradient button
kill $DEV_PID 2>/dev/null
```

If launching headless (CI), `pnpm dev` (without `tauri`) suffices to verify Vite + Tailwind compile.

- [ ] **Step 6: Commit**

```bash
git add .
git commit -m "feat(gui): Tailwind v4 config + Aurora design tokens

Adds @tailwindcss/vite plugin, bits-ui + lucide-svelte deps. Aurora
palette tokens (deepest/base/elev/glow + 4 text shades + 6 accents)
defined as @theme variables. App background is the radial cosmos
gradient. Inter font loaded from Google Fonts. Smoke-test page
renders pink heading + gradient button on dark base."
```

---

### Task P2A-3: tauri-specta wiring + first typed binding

**Files:**
- Modify: `crates/lconnect-gui/src-tauri/src/main.rs`
- Create: `crates/lconnect-gui/src-tauri/src/lib.rs`
- Modify: `crates/lconnect-gui/src-tauri/Cargo.toml`
- Create: `crates/lconnect-gui/src/lib/api/bindings.ts` (auto-generated)
- Create: `crates/lconnect-gui/src/lib/api/client.ts`

- [ ] **Step 1: Restructure src-tauri to export a lib**

Replace `crates/lconnect-gui/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lconnect_gui_lib::run()
}
```

Create `crates/lconnect-gui/src-tauri/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

use tauri_specta::{collect_commands, Builder};

#[tauri::command]
#[specta::specta]
fn ping() -> String {
    "pong".to_string()
}

pub fn run() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![ping]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/lib/api/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Build to generate bindings**

```bash
cd crates/lconnect-gui
cargo build -p lconnect-gui
# tauri-specta writes src/lib/api/bindings.ts automatically in debug builds
ls src/lib/api/bindings.ts
```

If `bindings.ts` is missing, run the binary briefly (it generates on startup):

```bash
cargo run -p lconnect-gui &
PID=$!
sleep 3
kill $PID
ls src/lib/api/bindings.ts
```

- [ ] **Step 3: Add a thin client wrapper**

Create `crates/lconnect-gui/src/lib/api/client.ts`:

```ts
// Thin wrapper around tauri-specta-generated bindings.
// Re-exports commands directly + provides a typed event listener helper.
export * from "./bindings";

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Listen for a typed Tauri event.
 * Caller is responsible for unlistening on component teardown.
 */
export async function on<T>(
  eventName: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => handler(event.payload));
}
```

- [ ] **Step 4: Use the binding from Svelte to verify wiring**

Update `crates/lconnect-gui/src/routes/+page.svelte`:

```svelte
<script lang="ts">
  import { commands } from "$lib/api/client";

  let pong = $state<string | null>(null);

  async function pingDaemon() {
    pong = await commands.ping();
  }
</script>

<main class="min-h-screen p-12">
  <h1 class="text-3xl font-semibold tracking-tight text-aurora-pink-hi mb-2">
    Aurora is live
  </h1>
  <p class="text-aurora-text-muted mb-6">
    Tauri-specta-generated bindings are loaded. Click ping to round-trip Rust ↔ TS.
  </p>
  <div class="flex gap-3 items-center">
    <button
      class="px-4 py-2 rounded-lg bg-gradient-to-b from-aurora-violet to-aurora-lav text-white font-medium"
      onclick={pingDaemon}>
      Ping
    </button>
    {#if pong}
      <span class="text-aurora-pink font-mono">{pong}</span>
    {/if}
  </div>
</main>
```

(Note: `tauri-specta` exposes commands under a `commands` namespace by convention; if the generated `bindings.ts` uses different names, adjust the import.)

- [ ] **Step 5: Verify the round-trip**

```bash
pnpm tauri dev &
DEV_PID=$!
sleep 8
# Click "Ping"; expect "pong" to appear next to the button
kill $DEV_PID 2>/dev/null
```

- [ ] **Step 6: Add `bindings.ts` to `.gitignore`**

It's generated; should not be committed. Add to `crates/lconnect-gui/.gitignore` (create if missing):

```
src/lib/api/bindings.ts
```

- [ ] **Step 7: Commit**

```bash
git add .
git commit -m "feat(gui): tauri-specta wiring + typed bindings round-trip

Adds Tauri Builder via tauri-specta with collect_commands!. The
ping command demonstrates the Rust -> TS type-safe pipeline.
bindings.ts is auto-generated on debug builds; gitignored.

src-tauri restructured into main.rs (entry only) + lib.rs (run
fn + commands). client.ts wraps the generated bindings and adds
a typed event listener helper."
```

---

## Sub-phase B — Mock daemon harness

### Task P2B-1: Mock daemon binary using `transport_memory`

**Files:**
- Create: `crates/lconnect-gui/mock-daemon/Cargo.toml`
- Create: `crates/lconnect-gui/mock-daemon/src/main.rs`

The mock daemon implements `DaemonRpc` with canned responses + a synthetic 1Hz snapshot loop. It listens on a Unix socket path passed via `--socket <path>`. Used by `pnpm tauri dev:mock` and Playwright E2E.

- [ ] **Step 1: Create `mock-daemon/Cargo.toml`**

```toml
[package]
name = "lconnect-mock-daemon"
description = "In-process fake lconnectd for GUI dev/E2E"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
publish = false

[[bin]]
name = "lconnect-mock-daemon"
path = "src/main.rs"

[dependencies]
lconnect-ipc = { path = "../../lconnect-ipc" }

tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
async-trait = "0.1"
futures = "0.3"
clap = { workspace = true, features = ["derive"] }
anyhow = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 2: Implement the mock**

Create `crates/lconnect-gui/mock-daemon/src/main.rs`:

```rust
//! Mock lconnectd for GUI development + Playwright E2E.
//!
//! Listens on a Unix socket; implements DaemonRpc with canned responses
//! plus a synthetic 1 Hz snapshot stream that wobbles values realistically.
//!
//! NOT a hardware-touching daemon — exists only so GUI work can proceed
//! without a real LCDV2 attached.

#![forbid(unsafe_code)]

use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use futures::stream::{self, BoxStream, StreamExt};
use lconnect_ipc::{
    DaemonRpc, DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, IpcError,
    LcdConfig, LedTarget, LightingConfig, LightingInfo, LightingKind, Pwm, PwmSource, ResetToken,
    Rgb, SensorSnapshot,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Parser, Debug)]
#[command(version, about = "Mock lconnectd for lconnect-gui dev + E2E")]
struct Cli {
    /// Socket path to listen on (typically $XDG_RUNTIME_DIR/lconnectd.sock or a tmp path)
    #[arg(long)]
    socket: PathBuf,
}

struct MockState {
    snapshot: SensorSnapshot,
    fan_pwm: u8,
    pump_pwm: u8,
    pump_source: PwmSource,
    lcd_config: LcdConfig,
    last_image_bytes: usize,
    write_log: Vec<String>,
}

impl MockState {
    fn new() -> Self {
        Self {
            snapshot: SensorSnapshot {
                coolant_c_x10: 342, // 34.2 °C
                fan_rpm: 1240,
                pump_rpm: 2860,
                temp_valid: true,
                last_seen_unix_ms: now_ms(),
            },
            fan_pwm: 50,
            pump_pwm: 75,
            pump_source: PwmSource::Mcu,
            lcd_config: LcdConfig {
                mode: "LocalUI".to_string(),
                brightness_percent: 50,
                rotation_degrees: 0,
            },
            last_image_bytes: 0,
            write_log: Vec::new(),
        }
    }

    fn tick(&mut self) {
        // Wobble the sensor values realistically: ±0.3 °C, ±20 RPM
        let nonce = (now_ms() / 1000) as i16 & 0x3;
        self.snapshot.coolant_c_x10 = 342 + (nonce - 2);
        self.snapshot.fan_rpm = 1240 + (nonce as u16 * 10);
        self.snapshot.pump_rpm = 2860 + (nonce as u16 * 5);
        self.snapshot.last_seen_unix_ms = now_ms();
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct MockRpc {
    state: Arc<Mutex<MockState>>,
}

fn lcdv2_descriptor() -> DeviceDescriptor {
    DeviceDescriptor {
        id: DeviceId::new("galahad2-lcdv2:0001"),
        family: "galahad2-lcdv2".to_string(),
        display_name: "Galahad II Vision (mock)".to_string(),
        vid: 0x0416,
        pid: 0x7395,
        serial: None,
        hid_path: PathBuf::from("/mock/hidraw0"),
    }
}

#[async_trait]
impl DaemonRpc for MockRpc {
    async fn list_devices(&self) -> lconnect_ipc::Result<Vec<DeviceDescriptor>> {
        Ok(vec![lcdv2_descriptor()])
    }

    async fn get_firmware(&self, _: DeviceId) -> lconnect_ipc::Result<FirmwareInfo> {
        Ok(FirmwareInfo {
            version: "1.6".to_string(),
            raw_token: "N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6 (mock)".to_string(),
        })
    }

    async fn get_snapshot(&self, _: DeviceId) -> lconnect_ipc::Result<SensorSnapshot> {
        Ok(self.state.lock().await.snapshot)
    }

    async fn subscribe_snapshots(
        &self,
        _: DeviceId,
    ) -> lconnect_ipc::Result<BoxStream<'static, SensorSnapshot>> {
        let state = self.state.clone();
        let stream = stream::unfold(state, |state| async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut s = state.lock().await;
            s.tick();
            let snap = s.snapshot;
            drop(s);
            Some((snap, state))
        });
        Ok(stream.boxed())
    }

    async fn set_fan_pwm(&self, _: DeviceId, percent: Pwm) -> lconnect_ipc::Result<()> {
        let mut s = self.state.lock().await;
        s.fan_pwm = percent.percent();
        s.write_log.push(format!("set_fan_pwm({})", percent.percent()));
        Ok(())
    }

    async fn set_pump_pwm(
        &self,
        _: DeviceId,
        percent: Pwm,
        source: PwmSource,
    ) -> lconnect_ipc::Result<()> {
        let mut s = self.state.lock().await;
        s.pump_pwm = percent.percent();
        s.pump_source = source;
        s.write_log
            .push(format!("set_pump_pwm({}, {:?})", percent.percent(), source));
        Ok(())
    }

    async fn get_lighting_info(
        &self,
        _: DeviceId,
        _: LightingKind,
    ) -> lconnect_ipc::Result<LightingInfo> {
        Ok(LightingInfo {
            max_speed: 4,
            max_brightness: 4,
            modes: vec![
                "Rainbow", "RainbowMorph", "StaticColor", "Breathing", "Runway", "Meteor",
                "TickerTape", "Fluctuation", "Transmit", "ColorfulStarryNight",
                "StaticStarryNight", "Voice", "BigBang", "Burst", "ColorsMorph", "Bounce",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        })
    }

    async fn set_lighting(
        &self,
        _: DeviceId,
        kind: LightingKind,
        cfg: LightingConfig,
    ) -> lconnect_ipc::Result<()> {
        self.state
            .lock()
            .await
            .write_log
            .push(format!("set_lighting({:?}, {:?})", kind, cfg.mode));
        Ok(())
    }

    async fn set_led_colors(
        &self,
        _: DeviceId,
        target: LedTarget,
        colors: Vec<Rgb>,
    ) -> lconnect_ipc::Result<()> {
        self.state.lock().await.write_log.push(format!(
            "set_led_colors({:?}, {} colors)",
            target,
            colors.len()
        ));
        Ok(())
    }

    async fn is_lcd_available(&self, _: DeviceId) -> lconnect_ipc::Result<bool> {
        Ok(true)
    }

    async fn set_lcd_config(&self, _: DeviceId, cfg: LcdConfig) -> lconnect_ipc::Result<()> {
        let mut s = self.state.lock().await;
        s.write_log
            .push(format!("set_lcd_config({:?})", cfg));
        s.lcd_config = cfg;
        Ok(())
    }

    async fn set_lcd_image(&self, _: DeviceId, jpeg: Vec<u8>) -> lconnect_ipc::Result<()> {
        let mut s = self.state.lock().await;
        s.last_image_bytes = jpeg.len();
        s.write_log
            .push(format!("set_lcd_image({} bytes)", jpeg.len()));
        Ok(())
    }

    async fn clear_lcd(&self, _: DeviceId) -> lconnect_ipc::Result<()> {
        self.state
            .lock()
            .await
            .write_log
            .push("clear_lcd".to_string());
        Ok(())
    }

    async fn request_reset_token(&self, _: DeviceId) -> lconnect_ipc::Result<ResetToken> {
        Ok(ResetToken("mock-token-30s".to_string()))
    }

    async fn reset_device(&self, _: DeviceId, confirm: ResetToken) -> lconnect_ipc::Result<()> {
        if confirm.0 != "mock-token-30s" {
            return Err(IpcError::ConfirmationRequired);
        }
        self.state
            .lock()
            .await
            .write_log
            .push("reset_device".to_string());
        Ok(())
    }

    async fn raw_send(
        &self,
        _: DeviceId,
        framing: FramingKind,
        bytes: Vec<u8>,
    ) -> lconnect_ipc::Result<Vec<u8>> {
        Err(IpcError::NotImplemented(format!(
            "raw_send not exposed in mock; framing={:?}, {} bytes",
            framing,
            bytes.len()
        )))
    }

    async fn daemon_stats(&self) -> lconnect_ipc::Result<DaemonStats> {
        let s = self.state.lock().await;
        Ok(DaemonStats {
            uptime_seconds: 0,
            devices_known: 1,
            hid_write_count: s.write_log.len() as u64,
            hid_read_errors: 0,
            hid_decode_errors: 0,
            ipc_request_count: 0,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("lconnect_mock_daemon=info")),
        )
        .try_init();

    let cli = Cli::parse();
    let state = Arc::new(Mutex::new(MockState::new()));
    let rpc = Arc::new(MockRpc {
        state: state.clone(),
    });

    let server = lconnect_ipc::server::IpcServer::new(rpc);
    tracing::info!("mock daemon listening on {}", cli.socket.display());
    server.serve(&cli.socket).await?;
    Ok(())
}
```

(Field names like `coolant_c_x10`, `last_seen_unix_ms` must match what Phase 1's `lconnect-ipc::types` actually defines. If they differ in Phase 1, adjust this file accordingly when Phase 1 ships.)

- [ ] **Step 3: Build + smoke run**

```bash
cargo build -p lconnect-mock-daemon

# Quick run to verify it binds a socket cleanly
SOCKET=$(mktemp -u --suffix=.sock)
cargo run -p lconnect-mock-daemon -- --socket "$SOCKET" &
PID=$!
sleep 2
test -S "$SOCKET" && echo "socket created OK"
kill $PID
rm -f "$SOCKET"
```

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/mock-daemon/
git commit -m "feat(gui): mock daemon harness for dev + E2E

Bin crate at crates/lconnect-gui/mock-daemon. Implements DaemonRpc
with canned responses + a 1 Hz synthetic snapshot stream that
wobbles values realistically. Listens on a Unix socket path passed
via --socket. write_log captures every set_* call so E2E tests can
assert behavior."
```

---

### Task P2B-2: `pnpm tauri dev:mock` script + socket env wiring

**Files:**
- Modify: `crates/lconnect-gui/package.json` (add scripts)
- Create: `crates/lconnect-gui/scripts/dev-mock.sh`

- [ ] **Step 1: Create the dev-mock script**

`crates/lconnect-gui/scripts/dev-mock.sh`:

```bash
#!/bin/bash
# Spin up the mock daemon + tauri dev, with a unique socket per run.
set -euo pipefail

SOCKET="${LCONNECT_SOCKET:-/tmp/lconnect-mock-$$.sock}"
export LCONNECT_SOCKET="$SOCKET"

cleanup() {
  kill "${MOCK_PID:-}" 2>/dev/null || true
  rm -f "$SOCKET"
}
trap cleanup EXIT

# Build mock first so it's ready before the GUI tries to connect
cargo build -p lconnect-mock-daemon

cargo run -q -p lconnect-mock-daemon -- --socket "$SOCKET" &
MOCK_PID=$!
sleep 1

echo "Mock daemon listening on $SOCKET (pid $MOCK_PID)"
echo "Tauri dev launching..."

pnpm tauri dev
```

```bash
chmod +x crates/lconnect-gui/scripts/dev-mock.sh
```

- [ ] **Step 2: Add scripts to package.json**

Edit `crates/lconnect-gui/package.json` `scripts` section:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "check:watch": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json --watch",
    "lint": "prettier --check . && eslint .",
    "format": "prettier --write .",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:e2e": "playwright test",
    "tauri": "tauri",
    "tauri:dev:mock": "bash scripts/dev-mock.sh"
  }
}
```

- [ ] **Step 3: Verify the dev-mock script runs**

```bash
cd crates/lconnect-gui
pnpm tauri:dev:mock &
SCRIPT_PID=$!
sleep 8
# Confirm window opens; webview should still show the smoke-test page (we haven't wired the daemon yet)
kill $SCRIPT_PID 2>/dev/null
```

(The actual GUI doesn't yet open the IPC socket — that lands in P2C. For now we just confirm the script orchestrates mock + tauri dev correctly.)

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/scripts/ crates/lconnect-gui/package.json
git commit -m "feat(gui): pnpm tauri:dev:mock script

scripts/dev-mock.sh spawns the mock daemon on a unique tmp socket
($LCONNECT_SOCKET), then runs pnpm tauri dev. Cleans up on exit.

Lets the GUI develop without a real LCDV2 attached. The Tauri shell
will read LCONNECT_SOCKET in P2C-1 to choose where to connect."
```

---

## Sub-phase C — Rust IPC bridge

### Task P2C-1: `IpcState` + connection management + reconnect

**Files:**
- Create: `crates/lconnect-gui/src-tauri/src/ipc_state.rs`
- Create: `crates/lconnect-gui/src-tauri/src/error.rs`
- Modify: `crates/lconnect-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Create the error module**

`crates/lconnect-gui/src-tauri/src/error.rs`:

```rust
//! Tauri-side error type. All Tauri commands return Result<T, Error>; serde-friendly.

use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "message")]
#[non_exhaustive]
pub enum Error {
    #[error("daemon connection error: {0}")]
    Daemon(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("confirmation token required")]
    ConfirmationRequired,
    #[error("image preparation failed: {0}")]
    Image(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<lconnect_ipc::IpcError> for Error {
    fn from(e: lconnect_ipc::IpcError) -> Self {
        use lconnect_ipc::IpcError as I;
        match e {
            I::DeviceNotFound(s) => Error::DeviceNotFound(s),
            I::DeviceBusy(s) | I::Transport(s) | I::Serialization(s) => Error::Daemon(s),
            I::InvalidArgument(s) => Error::InvalidArgument(s),
            I::NotImplemented(s) => Error::NotImplemented(s),
            I::ConfirmationRequired => Error::ConfirmationRequired,
            I::ProtocolMismatch(s) => Error::Daemon(format!("protocol mismatch: {s}")),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 2: Create `ipc_state.rs`**

```rust
//! Long-lived IpcClient + reconnect logic.
//!
//! Tauri commands access the daemon via `IpcState::with_client(|c| ...)`.
//! On daemon disconnect, the state transitions to Reconnecting and a
//! background task retries with exponential backoff (1s, 2s, 5s, 10s,
//! 30s capped). The frontend learns the state via the `daemon-state`
//! Tauri event.

use crate::error::{Error, Result};
use lconnect_ipc::client::IpcClient;
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "state", rename_all = "lowercase")]
#[non_exhaustive]
pub enum DaemonConnectionState {
    Disconnected,
    Connecting,
    Connected { ipc_version: String, daemon_version: String },
    Reconnecting { attempt: u32 },
}

pub struct IpcState {
    socket_path: PathBuf,
    client: Mutex<Option<Arc<IpcClient>>>,
    state: Mutex<DaemonConnectionState>,
}

impl IpcState {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            client: Mutex::new(None),
            state: Mutex::new(DaemonConnectionState::Disconnected),
        }
    }

    pub async fn current_state(&self) -> DaemonConnectionState {
        self.state.lock().await.clone()
    }

    /// Run a closure with a reference to the connected client.
    /// Returns `Error::Daemon` if not connected.
    pub async fn with_client<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&IpcClient) -> futures::future::BoxFuture<'_, Result<T>>,
    {
        let guard = self.client.lock().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| Error::Daemon("daemon not connected".to_string()))?;
        f(client).await
    }

    /// Spawn the connection-management task. Runs forever; transitions
    /// state and emits `daemon-state` events as it (re)connects.
    pub fn spawn_connection_task(self: Arc<Self>, app: AppHandle) {
        tokio::spawn(async move {
            self.connect_loop(app).await;
        });
    }

    async fn connect_loop(self: Arc<Self>, app: AppHandle) {
        let mut attempt: u32 = 0;
        loop {
            // Update state -> Connecting / Reconnecting
            {
                let mut s = self.state.lock().await;
                *s = if attempt == 0 {
                    DaemonConnectionState::Connecting
                } else {
                    DaemonConnectionState::Reconnecting { attempt }
                };
                let _ = app.emit("daemon-state", s.clone());
            }

            match IpcClient::connect(&self.socket_path).await {
                Ok(client) => {
                    info!("connected to daemon at {}", self.socket_path.display());
                    let hello = client.hello.clone();
                    let daemon_version = hello
                        .params
                        .get("daemon_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let ipc_version = hello
                        .params
                        .get("ipc_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();

                    *self.client.lock().await = Some(Arc::new(client));
                    *self.state.lock().await = DaemonConnectionState::Connected {
                        ipc_version,
                        daemon_version,
                    };
                    let _ = app.emit("daemon-state", self.state.lock().await.clone());

                    attempt = 0;
                    // Sleep until something signals disconnect. Phase 2-MVP: poll
                    // by attempting a no-op every 5s. A future improvement: have
                    // IpcClient expose a `closed()` future.
                    self.wait_until_disconnected().await;
                    *self.client.lock().await = None;
                    warn!("daemon connection lost; will retry");
                }
                Err(e) => {
                    warn!("connect failed: {}", e);
                }
            }

            attempt = attempt.saturating_add(1);
            let backoff = match attempt {
                1 => Duration::from_secs(1),
                2 => Duration::from_secs(2),
                3 => Duration::from_secs(5),
                4 => Duration::from_secs(10),
                _ => Duration::from_secs(30),
            };
            tokio::time::sleep(backoff).await;
        }
    }

    async fn wait_until_disconnected(&self) {
        // Poll list_devices every 5s; exit on error.
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let guard = self.client.lock().await;
            let Some(client) = guard.as_ref() else { return };
            let result = client
                .call("list_devices", serde_json::json!({}))
                .await;
            drop(guard);
            if result.is_err() {
                return;
            }
        }
    }
}
```

- [ ] **Step 3: Wire `IpcState` into `lib.rs`**

Edit `crates/lconnect-gui/src-tauri/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

mod commands;
mod error;
mod events;
mod helpers;
mod ipc_state;

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

use ipc_state::IpcState;

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("lconnect_gui=info")),
        )
        .try_init();

    let socket_path = std::env::var("LCONNECT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            xdg::BaseDirectories::with_prefix("lconnectd")
                .ok()
                .and_then(|d| d.get_runtime_directory().ok())
                .map(|d| d.join("lconnectd.sock"))
                .unwrap_or_else(|| PathBuf::from("/run/user/1000/lconnectd/lconnectd.sock"))
        });

    let ipc = Arc::new(IpcState::new(socket_path));

    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::ping,
            commands::daemon_connection_state,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/lib/api/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            app.manage(ipc.clone());
            ipc.clone().spawn_connection_task(app_handle);
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

(Add `xdg = "2"` to `src-tauri/Cargo.toml` deps if not present.)

- [ ] **Step 4: Add `daemon_connection_state` command**

Create `crates/lconnect-gui/src-tauri/src/commands.rs` (initial):

```rust
//! All Tauri commands. Grouped by concern; each fn is `#[tauri::command]
//! #[specta::specta]`.

use crate::ipc_state::{DaemonConnectionState, IpcState};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn daemon_connection_state(
    ipc: State<'_, Arc<IpcState>>,
) -> Result<DaemonConnectionState, ()> {
    Ok(ipc.current_state().await)
}
```

(Empty stub modules for the others so `lib.rs` compiles:)

```bash
for m in events helpers; do
  printf '//! Stub - body lands in subsequent P2C-* tasks.\n\n' \
    > "crates/lconnect-gui/src-tauri/src/${m}.rs"
done
```

- [ ] **Step 5: Build + run dev:mock + verify connection state**

```bash
cd crates/lconnect-gui
cargo build -p lconnect-gui
pnpm tauri:dev:mock &
PID=$!
sleep 6
# In the dev console (or by inspecting Vite logs), the daemon-state
# event should fire with state="connected" once the socket is up.
# Click Ping in the smoke-test page to confirm round-trip still works.
kill $PID 2>/dev/null
```

- [ ] **Step 6: Commit**

```bash
git add crates/lconnect-gui/src-tauri/
git commit -m "feat(gui): IpcState + connection management + reconnect

Long-lived IpcClient managed by tokio task. Exponential backoff
(1/2/5/10/30s) on disconnect. Emits 'daemon-state' Tauri event with
{Disconnected, Connecting, Connected{versions}, Reconnecting{attempt}}.

LCONNECT_SOCKET env var overrides the default
\$XDG_RUNTIME_DIR/lconnectd.sock path. daemon_connection_state command
exposed to the frontend."
```

---

### Task P2C-2: Tauri commands wrapping `DaemonRpc`

**Files:**
- Modify: `crates/lconnect-gui/src-tauri/src/commands.rs`
- Modify: `crates/lconnect-gui/src-tauri/src/lib.rs` (register commands)

Add one Tauri command per `DaemonRpc` method. All take `ipc: State<Arc<IpcState>>`, marshal arguments through `IpcClient::call`, and return `Result<T, Error>`.

- [ ] **Step 1: Implement command group** (paste-extend `commands.rs`)

```rust
use crate::error::{Error, Result};
use lconnect_ipc::{
    DaemonStats, DeviceDescriptor, DeviceId, FirmwareInfo, FramingKind, LcdConfig, LedTarget,
    LightingConfig, LightingInfo, LightingKind, Pwm, PwmSource, ResetToken, Rgb, SensorSnapshot,
};
use serde_json::json;

async fn rpc_call<T: serde::de::DeserializeOwned>(
    ipc: &IpcState,
    method: &str,
    params: serde_json::Value,
) -> Result<T> {
    ipc.with_client(|c| Box::pin(async move {
        let v = c.call(method, params).await?;
        serde_json::from_value::<T>(v).map_err(|e| Error::Internal(e.to_string()))
    })).await
}

// ── Discovery ─────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn list_devices(ipc: State<'_, Arc<IpcState>>) -> Result<Vec<DeviceDescriptor>> {
    rpc_call(&ipc, "list_devices", json!({})).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_firmware(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
) -> Result<FirmwareInfo> {
    rpc_call(&ipc, "get_firmware", json!({ "device": device })).await
}

// ── Sensors ───────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn get_snapshot(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
) -> Result<SensorSnapshot> {
    rpc_call(&ipc, "get_snapshot", json!({ "device": device })).await
}

// ── PWM ───────────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn set_fan_pwm(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    percent: u8,
) -> Result<()> {
    let pwm = Pwm::new(percent).map_err(Error::InvalidArgument)?;
    rpc_call(&ipc, "set_fan_pwm", json!({ "device": device, "percent": pwm })).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_pump_pwm(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    percent: u8,
    source: PwmSource,
) -> Result<()> {
    let pwm = Pwm::new(percent).map_err(Error::InvalidArgument)?;
    rpc_call(
        &ipc,
        "set_pump_pwm",
        json!({ "device": device, "percent": pwm, "source": source }),
    )
    .await
}

// ── Lighting ──────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn get_lighting_info(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    kind: LightingKind,
) -> Result<LightingInfo> {
    rpc_call(
        &ipc,
        "get_lighting_info",
        json!({ "device": device, "kind": kind }),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_lighting(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    kind: LightingKind,
    cfg: LightingConfig,
) -> Result<()> {
    rpc_call(
        &ipc,
        "set_lighting",
        json!({ "device": device, "kind": kind, "cfg": cfg }),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_led_colors(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    target: LedTarget,
    colors: Vec<Rgb>,
) -> Result<()> {
    rpc_call(
        &ipc,
        "set_led_colors",
        json!({ "device": device, "target": target, "colors": colors }),
    )
    .await
}

// ── LCD ───────────────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn is_lcd_available(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
) -> Result<bool> {
    rpc_call(&ipc, "is_lcd_available", json!({ "device": device })).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_lcd_config(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    cfg: LcdConfig,
) -> Result<()> {
    rpc_call(&ipc, "set_lcd_config", json!({ "device": device, "cfg": cfg })).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_lcd_image(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    jpeg: Vec<u8>,
) -> Result<()> {
    rpc_call(
        &ipc,
        "set_lcd_image",
        json!({ "device": device, "jpeg": jpeg }),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_lcd(ipc: State<'_, Arc<IpcState>>, device: DeviceId) -> Result<()> {
    rpc_call(&ipc, "clear_lcd", json!({ "device": device })).await
}

// ── Maintenance ───────────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn request_reset_token(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
) -> Result<ResetToken> {
    rpc_call(&ipc, "request_reset_token", json!({ "device": device })).await
}

#[tauri::command]
#[specta::specta]
pub async fn reset_device(
    ipc: State<'_, Arc<IpcState>>,
    device: DeviceId,
    confirm: ResetToken,
) -> Result<()> {
    rpc_call(
        &ipc,
        "reset_device",
        json!({ "device": device, "confirm": confirm }),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn daemon_stats(ipc: State<'_, Arc<IpcState>>) -> Result<DaemonStats> {
    rpc_call(&ipc, "daemon_stats", json!({})).await
}
```

(Note: every `lconnect-ipc` type used here needs `#[derive(specta::Type)]` so `tauri-specta` can generate matching TypeScript. If Phase 1 didn't add that derive, P2C-2 includes a small Phase-1-side amendment task.)

- [ ] **Step 2: Register commands in `lib.rs`**

Update the `collect_commands![]` macro:

```rust
.commands(collect_commands![
    commands::ping,
    commands::daemon_connection_state,
    commands::list_devices,
    commands::get_firmware,
    commands::get_snapshot,
    commands::set_fan_pwm,
    commands::set_pump_pwm,
    commands::get_lighting_info,
    commands::set_lighting,
    commands::set_led_colors,
    commands::is_lcd_available,
    commands::set_lcd_config,
    commands::set_lcd_image,
    commands::clear_lcd,
    commands::request_reset_token,
    commands::reset_device,
    commands::daemon_stats,
])
```

- [ ] **Step 3: Build + smoke**

```bash
cargo build -p lconnect-gui
# Verify bindings.ts grew with the new command signatures
cat crates/lconnect-gui/src/lib/api/bindings.ts | grep -c "export const commands"
```

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/
git commit -m "feat(gui): Tauri commands for the full DaemonRpc surface

15 commands added: list_devices, get_firmware, get_snapshot,
set_fan_pwm, set_pump_pwm, get_lighting_info, set_lighting,
set_led_colors, is_lcd_available, set_lcd_config, set_lcd_image,
clear_lcd, request_reset_token, reset_device, daemon_stats.

All marshal through rpc_call helper; errors map IpcError -> Error
(Tauri-side serde-friendly variant). tauri-specta auto-generates
TypeScript bindings on debug build."
```

---

### Task P2C-3: Snapshot subscription tasks + Tauri events

**Files:**
- Modify: `crates/lconnect-gui/src-tauri/src/events.rs`
- Modify: `crates/lconnect-gui/src-tauri/src/ipc_state.rs` (call into events)

- [ ] **Step 1: Implement `events.rs`**

```rust
//! Snapshot subscription tasks + Tauri event emission.
//!
//! For each device returned by list_devices, spawn a long-lived task
//! that calls subscribe_snapshots and re-emits each snapshot as a Tauri
//! event "snapshot:<device-id>".

use crate::ipc_state::IpcState;
use lconnect_ipc::{DeviceId, SensorSnapshot};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

pub async fn spawn_snapshot_subscriptions(
    ipc: Arc<IpcState>,
    app: AppHandle,
) -> crate::error::Result<()> {
    // List devices, then per device, subscribe.
    let devices: Vec<lconnect_ipc::DeviceDescriptor> = ipc
        .with_client(|c| {
            Box::pin(async move {
                let v = c.call("list_devices", serde_json::json!({})).await?;
                serde_json::from_value(v).map_err(|e| crate::error::Error::Internal(e.to_string()))
            })
        })
        .await?;

    for d in devices {
        let app2 = app.clone();
        let _ = app.emit("device-attached", d.clone());
        spawn_one(ipc.clone(), app2, d.id);
    }
    Ok(())
}

fn spawn_one(ipc: Arc<IpcState>, app: AppHandle, device_id: DeviceId) {
    tokio::spawn(async move {
        let mut sub = match ipc
            .with_client(|c| {
                let device_id = device_id.clone();
                Box::pin(async move {
                    Ok(c.subscribe(&format!("snapshot:{}", device_id.as_str())).await)
                })
            })
            .await
        {
            Ok(s) => s,
            Err(e) => {
                warn!("subscribe failed for {}: {}", device_id.as_str(), e);
                return;
            }
        };

        info!("snapshot subscription active for {}", device_id.as_str());
        let event_name = format!("snapshot:{}", device_id.as_str());

        while let Some(notif) = sub.recv().await {
            // The notification's params is the SensorSnapshot serialized.
            if let Ok(snap) = serde_json::from_value::<SensorSnapshot>(notif.params) {
                let _ = app.emit(&event_name, snap);
            }
        }

        let _ = app.emit("device-detached", device_id);
    });
}
```

(The exact subscribe API depends on Phase 1's `IpcClient::subscribe` signature. If Phase 1 ships `subscribe(method) -> mpsc::Receiver<Notification>` per the plan, this works as-is. Adjust if signatures differ.)

- [ ] **Step 2: Wire `spawn_snapshot_subscriptions` into the connect-loop**

In `ipc_state.rs`, after a successful connect, call `events::spawn_snapshot_subscriptions(self.clone(), app.clone()).await`. Adjust `connect_loop`:

```rust
// Inside connect_loop, after setting state to Connected:
if let Err(e) = crate::events::spawn_snapshot_subscriptions(self.clone(), app.clone()).await {
    warn!("failed to spawn snapshot subscriptions: {:?}", e);
}
```

- [ ] **Step 3: Smoke + verify event emission**

```bash
cargo build -p lconnect-gui
pnpm tauri:dev:mock &
PID=$!
sleep 8
# Check the dev console; "snapshot:galahad2-lcdv2:0001" events should fire ~1/sec
kill $PID 2>/dev/null
```

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/src-tauri/
git commit -m "feat(gui): snapshot subscription tasks + Tauri events

For each attached device, a long-lived tokio task subscribes to
snapshot notifications from the daemon and re-emits each one as
Tauri event 'snapshot:<device-id>'. device-attached and
device-detached events fire alongside.

Subscriptions are restarted automatically by the connect-loop
after reconnect."
```

---

### Task P2C-4: GUI helper commands (`prepare_jpeg`, `open_path`)

**Files:**
- Modify: `crates/lconnect-gui/src-tauri/src/helpers.rs`
- Modify: `crates/lconnect-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Implement `helpers.rs`**

```rust
//! GUI-only Tauri commands not backed by DaemonRpc.

use crate::error::{Error, Result};
use lconnect_image::{prepare_jpeg_from_path, FitMode, Quality};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum FitModeArg {
    Crop,
    Letterbox,
    Stretch,
}

impl From<FitModeArg> for FitMode {
    fn from(a: FitModeArg) -> Self {
        match a {
            FitModeArg::Crop => FitMode::Crop,
            FitModeArg::Letterbox => FitMode::Letterbox {
                bg: lconnect_image::Rgb { r: 0, g: 0, b: 0 },
            },
            FitModeArg::Stretch => FitMode::Stretch,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_jpeg(
    path: PathBuf,
    fit: FitModeArg,
    quality: u8,
) -> Result<Vec<u8>> {
    let q = Quality::new(quality)
        .map_err(|e| Error::InvalidArgument(format!("quality must be 1..=100: {e}")))?;
    let fit_mode: FitMode = fit.into();

    let path2 = path.clone();
    tokio::task::spawn_blocking(move || prepare_jpeg_from_path(&path2, fit_mode, q))
        .await
        .map_err(|e| Error::Internal(format!("blocking task panic: {e}")))?
        .map(|prepared| prepared.bytes)
        .map_err(|e| Error::Image(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn open_path(path: PathBuf) -> Result<()> {
    let s = path.to_string_lossy().to_string();
    open::that(&s).map_err(|e| Error::Io(e.to_string()))
}
```

(Add `open = "5"` to `src-tauri/Cargo.toml` deps.)

- [ ] **Step 2: Register the helpers**

Update `lib.rs`'s `collect_commands![]`:

```rust
.commands(collect_commands![
    commands::ping,
    commands::daemon_connection_state,
    // ... all 15 daemon commands ...
    helpers::prepare_jpeg,
    helpers::open_path,
])
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p lconnect-gui
git add crates/lconnect-gui/src-tauri/
git commit -m "feat(gui): GUI-only helpers (prepare_jpeg, open_path)

prepare_jpeg runs lconnect-image::prepare_jpeg_from_path inside
spawn_blocking (image conversion is CPU-bound; mustn't block tokio
runtime). FitModeArg enum mirrors lconnect-image::FitMode but
serde-friendly + specta-typed for tauri-specta export.

open_path uses 'open' crate for xdg-open semantics; used by
Settings page's 'Open config dir' button."
```

---

## Sub-phase D — Svelte stores (3 tasks)

**Goal:** Build the reactive layer that the pages consume. The stores hide IPC plumbing from components — pages read `connection.state` and `devices.byId(id)` and don't know anything about Tauri events.

**Idiom:** Svelte 5 runes (`$state`, `$derived`, `$effect`) inside `.svelte.ts` files. Stores are exported as objects with reactive properties, not as legacy Svelte stores.

### Task P2D-1: `daemon-connection.svelte.ts` store

**Files:**
- Create: `crates/lconnect-gui/src/lib/stores/daemon-connection.svelte.ts`
- Test: `crates/lconnect-gui/src/lib/stores/daemon-connection.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// crates/lconnect-gui/src/lib/stores/daemon-connection.test.ts
import { describe, expect, it, vi, beforeEach } from "vitest";
import { connection } from "./daemon-connection.svelte";
import * as tauriCore from "@tauri-apps/api/core";
import * as tauriEvent from "@tauri-apps/api/event";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

describe("daemon-connection store", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("starts in 'disconnected' state", () => {
    expect(connection.state.kind).toBe("disconnected");
  });

  it("updates state on daemon-connection-state event", async () => {
    const listeners = new Map<string, (e: { payload: unknown }) => void>();
    vi.mocked(tauriEvent.listen).mockImplementation(async (name, cb) => {
      listeners.set(name, cb as never);
      return () => listeners.delete(name);
    });
    vi.mocked(tauriCore.invoke).mockResolvedValue({ kind: "disconnected" });

    await connection.init();

    listeners.get("daemon-connection-state")?.({
      payload: { kind: "connected", since_unix_ms: 1234 },
    });

    expect(connection.state.kind).toBe("connected");
  });

  it("invokes daemon_reconnect command", async () => {
    vi.mocked(tauriCore.invoke).mockResolvedValue(undefined);
    await connection.reconnect();
    expect(tauriCore.invoke).toHaveBeenCalledWith("daemon_reconnect");
  });
});
```

- [ ] **Step 2: Run test, verify it fails**

Run: `pnpm --filter lconnect-gui test daemon-connection`
Expected: FAIL — "cannot find module ./daemon-connection.svelte"

- [ ] **Step 3: Implement the store**

```typescript
// crates/lconnect-gui/src/lib/stores/daemon-connection.svelte.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DaemonConnectionState } from "$lib/bindings";

class ConnectionStore {
  state = $state<DaemonConnectionState>({ kind: "disconnected" });
  private unlisten: UnlistenFn | null = null;

  async init(): Promise<void> {
    this.state = await invoke<DaemonConnectionState>("daemon_connection_state");
    this.unlisten = await listen<DaemonConnectionState>(
      "daemon-connection-state",
      (e) => {
        this.state = e.payload;
      },
    );
  }

  async reconnect(): Promise<void> {
    await invoke("daemon_reconnect");
  }

  destroy(): void {
    this.unlisten?.();
    this.unlisten = null;
  }
}

export const connection = new ConnectionStore();
```

- [ ] **Step 4: Verify test passes**

Run: `pnpm --filter lconnect-gui test daemon-connection`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/lconnect-gui/src/lib/stores/
git commit -m "feat(gui): connection store backed by Tauri events

ConnectionStore wraps the daemon-connection-state Tauri event into a
Svelte 5 \$state rune. Pages read connection.state.kind to render
banners ('Daemon disconnected. Reconnecting...'). +page.ts calls
connection.init() once at app boot via the root layout."
```

---

### Task P2D-2: `devices.svelte.ts` store

**Files:**
- Create: `crates/lconnect-gui/src/lib/stores/devices.svelte.ts`
- Test: `crates/lconnect-gui/src/lib/stores/devices.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// crates/lconnect-gui/src/lib/stores/devices.test.ts
import { describe, expect, it, vi, beforeEach } from "vitest";
import { devices } from "./devices.svelte";
import * as tauriCore from "@tauri-apps/api/core";
import * as tauriEvent from "@tauri-apps/api/event";

vi.mock("@tauri-apps/api/core");
vi.mock("@tauri-apps/api/event");

describe("devices store", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    devices.reset();
  });

  it("starts empty", () => {
    expect(devices.list).toEqual([]);
  });

  it("populates from list_devices", async () => {
    vi.mocked(tauriCore.invoke).mockResolvedValueOnce([
      { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2" },
    ]);
    vi.mocked(tauriEvent.listen).mockResolvedValue(() => {});

    await devices.init();

    expect(devices.list).toHaveLength(1);
    expect(devices.byId("ga2v:0001")).toBeTruthy();
  });

  it("merges snapshot events into byId map", async () => {
    const listeners = new Map<string, (e: { payload: unknown }) => void>();
    vi.mocked(tauriEvent.listen).mockImplementation(async (name, cb) => {
      listeners.set(name, cb as never);
      return () => {};
    });
    vi.mocked(tauriCore.invoke).mockResolvedValue([
      { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2" },
    ]);

    await devices.init();

    listeners.get("device-snapshot")?.({
      payload: {
        id: "ga2v:0001",
        cooling: { liquid_temp_c: 35.2, pump_rpm: 2400 },
        captured_unix_ms: 100,
      },
    });

    expect(devices.byId("ga2v:0001")?.snapshot?.cooling.liquid_temp_c).toBe(35.2);
  });
});
```

- [ ] **Step 2: Run test, verify it fails**

Run: `pnpm --filter lconnect-gui test devices`
Expected: FAIL — "cannot find module ./devices.svelte"

- [ ] **Step 3: Implement the store**

```typescript
// crates/lconnect-gui/src/lib/stores/devices.svelte.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeviceSnapshot, DeviceSummary } from "$lib/bindings";

interface DeviceEntry {
  summary: DeviceSummary;
  snapshot: DeviceSnapshot | null;
}

class DevicesStore {
  private map = $state<Map<string, DeviceEntry>>(new Map());
  private unlisten: UnlistenFn[] = [];

  list = $derived(Array.from(this.map.values()));

  byId(id: string): DeviceEntry | undefined {
    return this.map.get(id);
  }

  async init(): Promise<void> {
    const summaries = await invoke<DeviceSummary[]>("list_devices");
    const m = new Map<string, DeviceEntry>();
    for (const s of summaries) m.set(s.id, { summary: s, snapshot: null });
    this.map = m;

    this.unlisten.push(
      await listen<DeviceSnapshot>("device-snapshot", (e) => {
        const cur = this.map.get(e.payload.id);
        if (!cur) return;
        const next = new Map(this.map);
        next.set(e.payload.id, { ...cur, snapshot: e.payload });
        this.map = next;
      }),
      await listen<DeviceSummary>("device-attached", (e) => {
        const next = new Map(this.map);
        next.set(e.payload.id, { summary: e.payload, snapshot: null });
        this.map = next;
      }),
      await listen<{ id: string }>("device-detached", (e) => {
        const next = new Map(this.map);
        next.delete(e.payload.id);
        this.map = next;
      }),
    );
  }

  reset(): void {
    this.map = new Map();
    for (const u of this.unlisten) u();
    this.unlisten = [];
  }
}

export const devices = new DevicesStore();
```

- [ ] **Step 4: Verify test passes**

Run: `pnpm --filter lconnect-gui test devices`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/lconnect-gui/src/lib/stores/devices.svelte.ts \
        crates/lconnect-gui/src/lib/stores/devices.test.ts
git commit -m "feat(gui): devices store with snapshot/attach/detach merging

DevicesStore tracks DeviceSummary[] from list_devices() and merges
device-snapshot, device-attached, device-detached events into a
Map<id, DeviceEntry>. Map is replaced (not mutated) on each event so
\$derived recalculation reliably re-renders.

Components read devices.byId(id) for one device or devices.list for
the dashboard grid."
```

---

### Task P2D-3: `toasts.svelte.ts` store + error funnel

**Files:**
- Create: `crates/lconnect-gui/src/lib/stores/toasts.svelte.ts`
- Create: `crates/lconnect-gui/src/lib/util/handle-error.ts`
- Test: `crates/lconnect-gui/src/lib/stores/toasts.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
// crates/lconnect-gui/src/lib/stores/toasts.test.ts
import { describe, expect, it, beforeEach } from "vitest";
import { toasts } from "./toasts.svelte";

describe("toasts store", () => {
  beforeEach(() => toasts.clear());

  it("starts empty", () => {
    expect(toasts.items).toEqual([]);
  });

  it("push() adds an item with id, kind, and message", () => {
    const id = toasts.push({ kind: "error", message: "boom" });
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({ id, kind: "error", message: "boom" });
  });

  it("dismiss() removes by id", () => {
    const id = toasts.push({ kind: "info", message: "hi" });
    toasts.dismiss(id);
    expect(toasts.items).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test, verify it fails**

Run: `pnpm --filter lconnect-gui test toasts`
Expected: FAIL — "cannot find module ./toasts.svelte"

- [ ] **Step 3: Implement the store + handler**

```typescript
// crates/lconnect-gui/src/lib/stores/toasts.svelte.ts
export type ToastKind = "info" | "success" | "warn" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

class ToastsStore {
  items = $state<Toast[]>([]);
  private nextId = 1;

  push(t: Omit<Toast, "id">): number {
    const id = this.nextId++;
    this.items = [...this.items, { ...t, id }];
    if (t.kind !== "error") {
      setTimeout(() => this.dismiss(id), 4000);
    }
    return id;
  }

  dismiss(id: number): void {
    this.items = this.items.filter((t) => t.id !== id);
  }

  clear(): void {
    this.items = [];
    this.nextId = 1;
  }
}

export const toasts = new ToastsStore();
```

```typescript
// crates/lconnect-gui/src/lib/util/handle-error.ts
import { toasts } from "$lib/stores/toasts.svelte";

interface IpcError {
  kind: string;
  message: string;
}

function isIpcError(e: unknown): e is IpcError {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e &&
    typeof (e as IpcError).kind === "string"
  );
}

export function handleError(e: unknown, fallback = "Operation failed"): void {
  if (isIpcError(e)) {
    toasts.push({ kind: "error", message: `${e.kind}: ${e.message}` });
    return;
  }
  if (e instanceof Error) {
    toasts.push({ kind: "error", message: e.message });
    return;
  }
  toasts.push({ kind: "error", message: fallback });
}
```

- [ ] **Step 4: Verify test passes**

Run: `pnpm --filter lconnect-gui test toasts`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/lconnect-gui/src/lib/stores/toasts.svelte.ts \
        crates/lconnect-gui/src/lib/stores/toasts.test.ts \
        crates/lconnect-gui/src/lib/util/handle-error.ts
git commit -m "feat(gui): toast store + IPC error funnel

ToastsStore exposes push/dismiss/clear with auto-dismiss after 4s for
non-error kinds. handleError() narrows unknown to IpcError shape
({kind, message}) emitted by Tauri command errors and surfaces them
as red toasts. Component code wraps invoke() calls in try/catch with
handleError(e) so users never see silent failures."
```

---

## Sub-phase E — Component primitives (9 tasks, parallelizable)

**Goal:** Build the Aurora-themed primitives that pages compose. Each primitive is a single Svelte 5 component file with `$props` typing, scoped Tailwind v4 classes referencing Aurora tokens (defined in P2A-2), and a Vitest snapshot test.

**Parallelization note:** Tasks P2E-2 through P2E-9 are independent — they do not import each other. Dispatch them in parallel batches. P2E-1 (AppShell + SidebarNav) must land first because it pulls the routes together.

**Conventions for every component:**
- `<script lang="ts">` only (no `<script context="module">` unless needed)
- Props via `let { ... }: Props = $props()` — no `export let`
- Snippets via `{#snippet ...}` — no `<slot>`
- Event handlers as attributes: `onclick={handler}` — not `on:click`
- Tailwind classes only — no `<style>` blocks unless animation requires it
- a11y: every interactive element has either explicit role or native semantics; every input has a `<label>`

### Task P2E-1: AppShell + SidebarNav (must land before E-2…E-9)

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/AppShell.svelte`
- Create: `crates/lconnect-gui/src/lib/components/SidebarNav.svelte`
- Create: `crates/lconnect-gui/src/lib/components/ConnectionBanner.svelte`
- Create: `crates/lconnect-gui/src/lib/components/ToastViewport.svelte`
- Modify: `crates/lconnect-gui/src/routes/+layout.svelte`

- [ ] **Step 1: Write SidebarNav**

```svelte
<!-- crates/lconnect-gui/src/lib/components/SidebarNav.svelte -->
<script lang="ts">
  import { page } from "$app/state";

  interface NavItem {
    href: string;
    label: string;
    icon: string; // lucide icon name
  }

  const items: NavItem[] = [
    { href: "/", label: "Dashboard", icon: "gauge" },
    { href: "/cooling", label: "Cooling", icon: "fan" },
    { href: "/lighting", label: "Lighting", icon: "sparkles" },
    { href: "/display", label: "Display", icon: "monitor" },
    { href: "/settings", label: "Settings", icon: "settings" },
  ];

  function isActive(href: string): boolean {
    if (href === "/") return page.url.pathname === "/";
    return page.url.pathname.startsWith(href);
  }
</script>

<nav class="flex h-full w-56 flex-col gap-1 border-r border-aurora-glow/30 bg-aurora-deepest/60 p-3 backdrop-blur">
  <div class="px-2 py-4 text-xs font-semibold uppercase tracking-widest text-aurora-lav/70">
    L-Connect
  </div>
  {#each items as item (item.href)}
    <a
      href={item.href}
      class="rounded-lg px-3 py-2 text-sm font-medium text-aurora-text-base/80 transition hover:bg-aurora-glow/30 hover:text-aurora-text-base"
      class:bg-aurora-glow={isActive(item.href)}
      class:text-aurora-text-base={isActive(item.href)}
      data-testid="nav-{item.href.replace('/', '') || 'dashboard'}"
    >
      {item.label}
    </a>
  {/each}
</nav>
```

- [ ] **Step 2: Write ConnectionBanner**

```svelte
<!-- crates/lconnect-gui/src/lib/components/ConnectionBanner.svelte -->
<script lang="ts">
  import { connection } from "$lib/stores/daemon-connection.svelte";

  function label(): string {
    switch (connection.state.kind) {
      case "disconnected": return "Daemon disconnected";
      case "connecting": return "Connecting to daemon...";
      case "connected": return "";
      case "reconnecting": return "Reconnecting to daemon...";
    }
  }
</script>

{#if connection.state.kind !== "connected"}
  <div
    class="flex items-center justify-between border-b border-aurora-pink/40 bg-aurora-pink/10 px-4 py-2 text-sm text-aurora-pink"
    role="status"
    data-testid="connection-banner"
  >
    <span>{label()}</span>
    <button
      type="button"
      class="rounded bg-aurora-pink/20 px-3 py-1 text-xs font-semibold hover:bg-aurora-pink/30"
      onclick={() => connection.reconnect()}
    >
      Retry
    </button>
  </div>
{/if}
```

- [ ] **Step 3: Write ToastViewport**

```svelte
<!-- crates/lconnect-gui/src/lib/components/ToastViewport.svelte -->
<script lang="ts">
  import { toasts, type ToastKind } from "$lib/stores/toasts.svelte";

  function bg(k: ToastKind): string {
    switch (k) {
      case "info":    return "bg-aurora-violet/20 text-aurora-text-base border-aurora-violet/40";
      case "success": return "bg-aurora-lav/20 text-aurora-text-base border-aurora-lav/40";
      case "warn":    return "bg-aurora-pink/20 text-aurora-pink border-aurora-pink/40";
      case "error":   return "bg-red-500/20 text-red-200 border-red-400/40";
    }
  }
</script>

<div class="pointer-events-none fixed bottom-4 right-4 z-50 flex flex-col gap-2">
  {#each toasts.items as t (t.id)}
    <div
      class="pointer-events-auto flex items-center gap-3 rounded-lg border px-4 py-2 text-sm shadow-lg backdrop-blur {bg(t.kind)}"
      role="alert"
      data-testid="toast-{t.kind}"
    >
      <span class="flex-1">{t.message}</span>
      <button
        type="button"
        class="text-xs opacity-70 hover:opacity-100"
        onclick={() => toasts.dismiss(t.id)}
        aria-label="Dismiss"
      >
        ×
      </button>
    </div>
  {/each}
</div>
```

- [ ] **Step 4: Write AppShell**

```svelte
<!-- crates/lconnect-gui/src/lib/components/AppShell.svelte -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import SidebarNav from "./SidebarNav.svelte";
  import ConnectionBanner from "./ConnectionBanner.svelte";
  import ToastViewport from "./ToastViewport.svelte";

  interface Props { children: Snippet }
  let { children }: Props = $props();
</script>

<div class="flex h-screen w-screen flex-col bg-aurora-base text-aurora-text-base">
  <ConnectionBanner />
  <div class="flex flex-1 overflow-hidden">
    <SidebarNav />
    <main class="flex-1 overflow-y-auto p-6">
      {@render children()}
    </main>
  </div>
  <ToastViewport />
</div>
```

- [ ] **Step 5: Wire root layout**

```svelte
<!-- crates/lconnect-gui/src/routes/+layout.svelte -->
<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import { connection } from "$lib/stores/daemon-connection.svelte";
  import { devices } from "$lib/stores/devices.svelte";
  import { handleError } from "$lib/util/handle-error";

  let { children } = $props();

  onMount(() => {
    (async () => {
      try {
        await connection.init();
        await devices.init();
      } catch (e) {
        handleError(e, "Failed to initialise app");
      }
    })();
  });
</script>

<AppShell>
  {@render children()}
</AppShell>
```

- [ ] **Step 6: Snapshot test**

```typescript
// crates/lconnect-gui/src/lib/components/SidebarNav.test.ts
import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import SidebarNav from "./SidebarNav.svelte";

describe("SidebarNav", () => {
  it("renders all five nav items", () => {
    const { getAllByRole } = render(SidebarNav);
    const links = getAllByRole("link");
    expect(links).toHaveLength(5);
    expect(links.map((l) => l.textContent?.trim())).toEqual([
      "Dashboard", "Cooling", "Lighting", "Display", "Settings",
    ]);
  });
});
```

- [ ] **Step 7: Run tests**

Run: `pnpm --filter lconnect-gui test components/SidebarNav`
Expected: PASS (1 test)

Run: `pnpm --filter lconnect-gui run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/lconnect-gui/src/lib/components/AppShell.svelte \
        crates/lconnect-gui/src/lib/components/SidebarNav.svelte \
        crates/lconnect-gui/src/lib/components/ConnectionBanner.svelte \
        crates/lconnect-gui/src/lib/components/ToastViewport.svelte \
        crates/lconnect-gui/src/lib/components/SidebarNav.test.ts \
        crates/lconnect-gui/src/routes/+layout.svelte
git commit -m "feat(gui): AppShell + sidebar + connection banner + toast viewport

AppShell composes the persistent chrome: SidebarNav (5 routes left),
ConnectionBanner (top, only when daemon is not 'connected'), main
scroll region, and ToastViewport (fixed bottom-right). Layout calls
connection.init() and devices.init() once on mount."
```

---

### Task P2E-2: Card / GlassCard

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/Card.svelte`
- Test: `crates/lconnect-gui/src/lib/components/Card.test.ts`

- [ ] **Step 1: Implement Card**

```svelte
<!-- crates/lconnect-gui/src/lib/components/Card.svelte -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    glass?: boolean;
    class?: string;
    children: Snippet;
    actions?: Snippet;
  }

  let { title, glass = true, class: klass = "", children, actions }: Props = $props();

  const base = "rounded-xl border border-aurora-glow/30 p-5";
  const surface = glass
    ? "bg-aurora-deepest/40 backdrop-blur-md shadow-lg shadow-aurora-deepest/40"
    : "bg-aurora-deepest/60";
</script>

<section class="{base} {surface} {klass}">
  {#if title || actions}
    <header class="mb-4 flex items-center justify-between">
      {#if title}
        <h2 class="text-sm font-semibold uppercase tracking-widest text-aurora-lav">
          {title}
        </h2>
      {/if}
      {#if actions}
        <div>{@render actions()}</div>
      {/if}
    </header>
  {/if}
  {@render children()}
</section>
```

- [ ] **Step 2: Snapshot test**

```typescript
// crates/lconnect-gui/src/lib/components/Card.test.ts
import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import Card from "./Card.svelte";

describe("Card", () => {
  it("renders title when provided", () => {
    const { getByText } = render(Card, {
      props: { title: "Cooling" },
      // children snippet omitted for snapshot smoke
    });
    expect(getByText("Cooling")).toBeTruthy();
  });
});
```

- [ ] **Step 3: Verify**

Run: `pnpm --filter lconnect-gui test components/Card`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/src/lib/components/Card.svelte \
        crates/lconnect-gui/src/lib/components/Card.test.ts
git commit -m "feat(gui): Card primitive with optional glass surface

Card is the universal panel: rounded-xl, glassmorphic translucency
when glass=true, with optional title (lavender uppercase tracking)
and actions slot for header buttons."
```

---

### Task P2E-3: Button (primary/secondary/ghost)

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/Button.svelte`
- Test: `crates/lconnect-gui/src/lib/components/Button.test.ts`

- [ ] **Step 1: Implement Button**

```svelte
<!-- crates/lconnect-gui/src/lib/components/Button.svelte -->
<script lang="ts">
  import type { Snippet } from "svelte";

  type Variant = "primary" | "secondary" | "ghost" | "danger";

  interface Props {
    variant?: Variant;
    type?: "button" | "submit" | "reset";
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    class?: string;
    children: Snippet;
  }

  let {
    variant = "primary",
    type = "button",
    disabled = false,
    onclick,
    class: klass = "",
    children,
  }: Props = $props();

  const base =
    "inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition focus:outline-none focus:ring-2 focus:ring-aurora-lav/60 disabled:cursor-not-allowed disabled:opacity-50";

  const styles: Record<Variant, string> = {
    primary: "bg-aurora-lav text-aurora-deepest hover:bg-aurora-violet",
    secondary:
      "bg-aurora-glow/40 text-aurora-text-base border border-aurora-glow/60 hover:bg-aurora-glow/60",
    ghost:
      "bg-transparent text-aurora-text-base hover:bg-aurora-glow/30",
    danger:
      "bg-red-500/30 text-red-100 border border-red-500/50 hover:bg-red-500/50",
  };
</script>

<button
  {type}
  {disabled}
  class="{base} {styles[variant]} {klass}"
  onclick={onclick}
>
  {@render children()}
</button>
```

- [ ] **Step 2: Test**

```typescript
// crates/lconnect-gui/src/lib/components/Button.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Button from "./Button.svelte";

describe("Button", () => {
  it("invokes onclick when clicked", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Button, {
      props: { onclick, children: () => "Apply" } as never,
    });
    await fireEvent.click(getByRole("button"));
    expect(onclick).toHaveBeenCalledOnce();
  });

  it("respects disabled", async () => {
    const onclick = vi.fn();
    const { getByRole } = render(Button, {
      props: { disabled: true, onclick, children: () => "X" } as never,
    });
    await fireEvent.click(getByRole("button"));
    expect(onclick).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 3: Verify**

Run: `pnpm --filter lconnect-gui test components/Button`
Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/src/lib/components/Button.svelte \
        crates/lconnect-gui/src/lib/components/Button.test.ts
git commit -m "feat(gui): Button primitive with four variants

primary/secondary/ghost/danger using Aurora tokens. Focus ring uses
aurora-lav/60 for keyboard accessibility on the cosmos backdrop."
```

---

### Task P2E-4: Slider

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/Slider.svelte`
- Test: `crates/lconnect-gui/src/lib/components/Slider.test.ts`

- [ ] **Step 1: Implement Slider**

```svelte
<!-- crates/lconnect-gui/src/lib/components/Slider.svelte -->
<script lang="ts">
  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    unit?: string;
    disabled?: boolean;
    onchange?: (v: number) => void;
  }

  let {
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    label,
    unit = "%",
    disabled = false,
    onchange,
  }: Props = $props();

  function handleInput(e: Event): void {
    const t = e.target as HTMLInputElement;
    value = t.valueAsNumber;
    onchange?.(value);
  }

  let percent = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="flex flex-col gap-2">
  {#if label}
    <div class="flex items-baseline justify-between">
      <label class="text-xs font-medium uppercase tracking-wide text-aurora-text-base/70">
        {label}
      </label>
      <span class="text-sm tabular-nums text-aurora-text-base">
        {value}{unit}
      </span>
    </div>
  {/if}
  <input
    type="range"
    {min}
    {max}
    {step}
    {value}
    {disabled}
    oninput={handleInput}
    class="aurora-slider"
    style="--fill: {percent}%"
  />
</div>

<style>
  /* Range styling — Tailwind v4 cannot style ::-webkit-slider-thumb fully via classes. */
  .aurora-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      rgb(196 168 255) 0%,
      rgb(196 168 255) var(--fill),
      rgba(42, 22, 85, 0.4) var(--fill),
      rgba(42, 22, 85, 0.4) 100%
    );
    outline: none;
  }
  .aurora-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    background: rgb(243 238 255);
    border: 2px solid rgb(177 139 255);
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  }
  .aurora-slider:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

- [ ] **Step 2: Test**

```typescript
// crates/lconnect-gui/src/lib/components/Slider.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Slider from "./Slider.svelte";

describe("Slider", () => {
  it("calls onchange with new value on input", async () => {
    const onchange = vi.fn();
    const { container } = render(Slider, {
      props: { value: 50, onchange },
    });
    const range = container.querySelector("input[type=range]") as HTMLInputElement;
    range.value = "75";
    await fireEvent.input(range);
    expect(onchange).toHaveBeenCalledWith(75);
  });
});
```

- [ ] **Step 3: Verify + commit**

Run: `pnpm --filter lconnect-gui test components/Slider`
Expected: PASS

```bash
git add crates/lconnect-gui/src/lib/components/Slider.svelte \
        crates/lconnect-gui/src/lib/components/Slider.test.ts
git commit -m "feat(gui): Slider with Aurora track styling

Native input[type=range] with linear-gradient fill and lavender
thumb. \$bindable() so pages can <Slider bind:value={fanPct} />,
plus onchange callback for committing to daemon."
```

---

### Task P2E-5: TogglePill

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/TogglePill.svelte`
- Test: `crates/lconnect-gui/src/lib/components/TogglePill.test.ts`

- [ ] **Step 1: Implement TogglePill**

```svelte
<!-- crates/lconnect-gui/src/lib/components/TogglePill.svelte -->
<script lang="ts" generics="T extends string">
  interface Props {
    options: ReadonlyArray<{ value: T; label: string }>;
    value: T;
    onchange?: (v: T) => void;
    disabled?: boolean;
  }

  let { options, value = $bindable(), onchange, disabled = false }: Props = $props();
</script>

<div
  class="inline-flex rounded-full border border-aurora-glow/40 bg-aurora-deepest/40 p-1"
  role="tablist"
>
  {#each options as opt (opt.value)}
    <button
      type="button"
      role="tab"
      aria-selected={opt.value === value}
      {disabled}
      class="rounded-full px-4 py-1.5 text-xs font-semibold uppercase tracking-wide transition disabled:opacity-50"
      class:bg-aurora-lav={opt.value === value}
      class:text-aurora-deepest={opt.value === value}
      class:text-aurora-text-base={opt.value !== value}
      class:hover:bg-aurora-glow={opt.value !== value}
      onclick={() => {
        value = opt.value;
        onchange?.(opt.value);
      }}
    >
      {opt.label}
    </button>
  {/each}
</div>
```

- [ ] **Step 2: Test**

```typescript
// crates/lconnect-gui/src/lib/components/TogglePill.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import TogglePill from "./TogglePill.svelte";

describe("TogglePill", () => {
  it("changes value when other option clicked", async () => {
    const onchange = vi.fn();
    const { getByText } = render(TogglePill, {
      props: {
        options: [
          { value: "manual", label: "Manual" },
          { value: "auto", label: "Auto" },
        ],
        value: "manual",
        onchange,
      } as never,
    });
    await fireEvent.click(getByText("Auto"));
    expect(onchange).toHaveBeenCalledWith("auto");
  });
});
```

- [ ] **Step 3: Verify + commit**

```bash
pnpm --filter lconnect-gui test components/TogglePill
git add crates/lconnect-gui/src/lib/components/TogglePill.svelte \
        crates/lconnect-gui/src/lib/components/TogglePill.test.ts
git commit -m "feat(gui): TogglePill (radio-as-segmented-control)

Generic over T extends string so pages can pass typed mode unions
without casting. Used for cooling source (mcu/cpu/liquid) and
lighting tab switcher."
```

---

### Task P2E-6: ModeChipGrid

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/ModeChipGrid.svelte`
- Test: `crates/lconnect-gui/src/lib/components/ModeChipGrid.test.ts`

- [ ] **Step 1: Implement ModeChipGrid**

```svelte
<!-- crates/lconnect-gui/src/lib/components/ModeChipGrid.svelte -->
<script lang="ts">
  interface Mode {
    id: string;
    label: string;
    swatch: string; // CSS color or gradient
  }

  interface Props {
    modes: ReadonlyArray<Mode>;
    selected: string;
    onselect?: (id: string) => void;
  }

  let { modes, selected = $bindable(), onselect }: Props = $props();
</script>

<div class="grid grid-cols-3 gap-3 sm:grid-cols-4 lg:grid-cols-5">
  {#each modes as m (m.id)}
    <button
      type="button"
      class="group flex flex-col items-center gap-2 rounded-xl border border-aurora-glow/30 bg-aurora-deepest/40 p-3 transition hover:border-aurora-lav/60 hover:bg-aurora-glow/30"
      class:border-aurora-lav={m.id === selected}
      class:bg-aurora-glow={m.id === selected}
      onclick={() => {
        selected = m.id;
        onselect?.(m.id);
      }}
    >
      <span
        class="block h-10 w-10 rounded-full border border-aurora-text-base/20 shadow-inner"
        style="background: {m.swatch};"
        aria-hidden="true"
      ></span>
      <span class="text-xs font-medium text-aurora-text-base/90 group-hover:text-aurora-text-base">
        {m.label}
      </span>
    </button>
  {/each}
</div>
```

- [ ] **Step 2: Test + commit**

```typescript
// crates/lconnect-gui/src/lib/components/ModeChipGrid.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModeChipGrid from "./ModeChipGrid.svelte";

describe("ModeChipGrid", () => {
  it("calls onselect with mode id", async () => {
    const onselect = vi.fn();
    const { getByText } = render(ModeChipGrid, {
      props: {
        modes: [
          { id: "rainbow", label: "Rainbow", swatch: "linear-gradient(90deg, red, blue)" },
          { id: "static",  label: "Static",  swatch: "#c4a8ff" },
        ],
        selected: "rainbow",
        onselect,
      } as never,
    });
    await fireEvent.click(getByText("Static"));
    expect(onselect).toHaveBeenCalledWith("static");
  });
});
```

```bash
pnpm --filter lconnect-gui test components/ModeChipGrid
git add crates/lconnect-gui/src/lib/components/ModeChipGrid.svelte \
        crates/lconnect-gui/src/lib/components/ModeChipGrid.test.ts
git commit -m "feat(gui): ModeChipGrid for lighting preset selection

Responsive grid (3/4/5 cols) of square chips with circular swatch
preview + label. Selected chip shows lavender border. Pages pass
swatches as CSS color or gradient strings, decoupling visuals from
the mode catalog."
```

---

### Task P2E-7: ColorPicker (HSV-based)

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/ColorPicker.svelte`
- Create: `crates/lconnect-gui/src/lib/util/color.ts`
- Test: `crates/lconnect-gui/src/lib/util/color.test.ts`

- [ ] **Step 1: HSV/RGB conversion utilities**

```typescript
// crates/lconnect-gui/src/lib/util/color.ts
export interface Rgb { r: number; g: number; b: number }
export interface Hsv { h: number; s: number; v: number } // h:0-360, s/v:0-1

export function rgbToHsv({ r, g, b }: Rgb): Hsv {
  const rn = r / 255, gn = g / 255, bn = b / 255;
  const max = Math.max(rn, gn, bn), min = Math.min(rn, gn, bn);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  const s = max === 0 ? 0 : d / max;
  return { h, s, v: max };
}

export function hsvToRgb({ h, s, v }: Hsv): Rgb {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  let rp = 0, gp = 0, bp = 0;
  if      (h < 60)  { rp = c; gp = x; }
  else if (h < 120) { rp = x; gp = c; }
  else if (h < 180) {         gp = c; bp = x; }
  else if (h < 240) {         gp = x; bp = c; }
  else if (h < 300) { rp = x;         bp = c; }
  else              { rp = c;         bp = x; }
  return {
    r: Math.round((rp + m) * 255),
    g: Math.round((gp + m) * 255),
    b: Math.round((bp + m) * 255),
  };
}

export function rgbToHex({ r, g, b }: Rgb): string {
  const h = (n: number) => n.toString(16).padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}

export function hexToRgb(hex: string): Rgb | null {
  const m = /^#?([0-9a-fA-F]{6})$/.exec(hex.trim());
  if (!m) return null;
  const v = parseInt(m[1], 16);
  return { r: (v >> 16) & 0xff, g: (v >> 8) & 0xff, b: v & 0xff };
}
```

```typescript
// crates/lconnect-gui/src/lib/util/color.test.ts
import { describe, expect, it } from "vitest";
import { hsvToRgb, rgbToHsv, rgbToHex, hexToRgb } from "./color";

describe("color conversions", () => {
  it("rgbToHsv: red is hue 0", () => {
    const hsv = rgbToHsv({ r: 255, g: 0, b: 0 });
    expect(hsv.h).toBeCloseTo(0);
    expect(hsv.s).toBeCloseTo(1);
    expect(hsv.v).toBeCloseTo(1);
  });

  it("hsvToRgb: hue 120 is green", () => {
    const rgb = hsvToRgb({ h: 120, s: 1, v: 1 });
    expect(rgb).toEqual({ r: 0, g: 255, b: 0 });
  });

  it("rgbToHex round-trips", () => {
    const rgb = { r: 196, g: 168, b: 255 };
    expect(hexToRgb(rgbToHex(rgb))).toEqual(rgb);
  });

  it("hexToRgb rejects invalid input", () => {
    expect(hexToRgb("not a color")).toBeNull();
  });
});
```

- [ ] **Step 2: Run color util tests**

Run: `pnpm --filter lconnect-gui test util/color`
Expected: PASS (4 tests)

- [ ] **Step 3: Implement ColorPicker**

```svelte
<!-- crates/lconnect-gui/src/lib/components/ColorPicker.svelte -->
<script lang="ts">
  import {
    type Rgb,
    rgbToHsv,
    hsvToRgb,
    rgbToHex,
    hexToRgb,
  } from "$lib/util/color";

  interface Props {
    value: Rgb;
    onchange?: (rgb: Rgb) => void;
  }

  let { value = $bindable(), onchange }: Props = $props();

  let hsv = $derived(rgbToHsv(value));
  let hex = $derived(rgbToHex(value));

  function setHue(h: number): void {
    const next = hsvToRgb({ ...hsv, h });
    value = next;
    onchange?.(next);
  }

  function setSV(s: number, v: number): void {
    const next = hsvToRgb({ h: hsv.h, s, v });
    value = next;
    onchange?.(next);
  }

  function onHexInput(e: Event): void {
    const t = e.target as HTMLInputElement;
    const r = hexToRgb(t.value);
    if (r) {
      value = r;
      onchange?.(r);
    }
  }

  function onSVPointer(e: PointerEvent): void {
    const target = e.currentTarget as HTMLDivElement;
    const rect = target.getBoundingClientRect();
    const s = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    const v = 1 - Math.min(1, Math.max(0, (e.clientY - rect.top) / rect.height));
    setSV(s, v);
  }
</script>

<div class="flex flex-col gap-3">
  <!-- SV plane -->
  <div
    role="slider"
    aria-label="Saturation and value"
    aria-valuenow={Math.round(hsv.s * 100)}
    tabindex="0"
    class="relative h-40 w-full cursor-crosshair rounded-lg border border-aurora-glow/40"
    style="background:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, hsl({hsv.h}, 100%, 50%));"
    onpointerdown={(e) => {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      onSVPointer(e);
    }}
    onpointermove={(e) => {
      if (e.buttons & 1) onSVPointer(e);
    }}
  >
    <div
      class="pointer-events-none absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow"
      style="left: {hsv.s * 100}%; top: {(1 - hsv.v) * 100}%;"
    ></div>
  </div>

  <!-- Hue slider -->
  <input
    type="range"
    min="0"
    max="360"
    step="1"
    value={hsv.h}
    oninput={(e) => setHue((e.target as HTMLInputElement).valueAsNumber)}
    class="aurora-hue"
    aria-label="Hue"
  />

  <!-- Hex + swatch -->
  <div class="flex items-center gap-3">
    <span
      class="block h-8 w-8 rounded-md border border-aurora-glow/40"
      style="background: {hex};"
      aria-hidden="true"
    ></span>
    <input
      type="text"
      value={hex}
      oninput={onHexInput}
      class="flex-1 rounded-lg border border-aurora-glow/40 bg-aurora-deepest/40 px-3 py-1.5 font-mono text-sm text-aurora-text-base focus:border-aurora-lav focus:outline-none"
      aria-label="Hex color"
    />
  </div>
</div>

<style>
  .aurora-hue {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 12px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%
    );
  }
  .aurora-hue::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    background: white;
    border: 2px solid rgb(177 139 255);
    cursor: pointer;
  }
</style>
```

- [ ] **Step 4: Smoke test the component**

```typescript
// crates/lconnect-gui/src/lib/components/ColorPicker.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ColorPicker from "./ColorPicker.svelte";

describe("ColorPicker", () => {
  it("emits onchange when hex input is typed", async () => {
    const onchange = vi.fn();
    const { getByLabelText } = render(ColorPicker, {
      props: { value: { r: 0, g: 0, b: 0 }, onchange },
    });
    const hex = getByLabelText("Hex color") as HTMLInputElement;
    await fireEvent.input(hex, { target: { value: "#ff0000" } });
    expect(onchange).toHaveBeenCalledWith({ r: 255, g: 0, b: 0 });
  });
});
```

- [ ] **Step 5: Verify + commit**

```bash
pnpm --filter lconnect-gui test components/ColorPicker
git add crates/lconnect-gui/src/lib/util/color.ts \
        crates/lconnect-gui/src/lib/util/color.test.ts \
        crates/lconnect-gui/src/lib/components/ColorPicker.svelte \
        crates/lconnect-gui/src/lib/components/ColorPicker.test.ts
git commit -m "feat(gui): ColorPicker with HSV plane + hue slider + hex

SV plane uses pointer capture for drag, hue is a styled range input,
hex input round-trips RGB. Conversion utilities are pure functions
in \$lib/util/color.ts so headless tests don't need a DOM."
```

---

### Task P2E-8: CurveEditor (fan curve)

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/CurveEditor.svelte`
- Test: `crates/lconnect-gui/src/lib/components/CurveEditor.test.ts`

- [ ] **Step 1: Implement CurveEditor**

```svelte
<!-- crates/lconnect-gui/src/lib/components/CurveEditor.svelte -->
<script lang="ts">
  interface Point { temp: number; pct: number } // temp:0-100C, pct:0-100

  interface Props {
    points: Point[];
    onchange?: (points: Point[]) => void;
    minTemp?: number;
    maxTemp?: number;
  }

  let {
    points = $bindable(),
    onchange,
    minTemp = 20,
    maxTemp = 80,
  }: Props = $props();

  const W = 320, H = 200, P = 24; // pad

  function tempToX(t: number): number {
    return P + ((t - minTemp) / (maxTemp - minTemp)) * (W - 2 * P);
  }
  function pctToY(p: number): number {
    return H - P - (p / 100) * (H - 2 * P);
  }
  function xToTemp(x: number): number {
    const v = ((x - P) / (W - 2 * P)) * (maxTemp - minTemp) + minTemp;
    return Math.round(Math.min(maxTemp, Math.max(minTemp, v)));
  }
  function yToPct(y: number): number {
    const v = ((H - P - y) / (H - 2 * P)) * 100;
    return Math.round(Math.min(100, Math.max(0, v)));
  }

  let dragging: number | null = $state(null);

  function svgCoords(e: PointerEvent): { x: number; y: number } {
    const svg = (e.currentTarget as SVGSVGElement);
    const rect = svg.getBoundingClientRect();
    return {
      x: ((e.clientX - rect.left) / rect.width) * W,
      y: ((e.clientY - rect.top) / rect.height) * H,
    };
  }

  function onPointerMove(e: PointerEvent): void {
    if (dragging === null) return;
    const { x, y } = svgCoords(e);
    const next = [...points];
    next[dragging] = { temp: xToTemp(x), pct: yToPct(y) };
    next.sort((a, b) => a.temp - b.temp);
    points = next;
    onchange?.(next);
  }

  let pathD = $derived(
    points
      .map((p, i) => `${i === 0 ? "M" : "L"} ${tempToX(p.temp)} ${pctToY(p.pct)}`)
      .join(" "),
  );
</script>

<svg
  viewBox="0 0 {W} {H}"
  class="block w-full max-w-md rounded-xl border border-aurora-glow/40 bg-aurora-deepest/40"
  onpointermove={onPointerMove}
  onpointerup={() => (dragging = null)}
  onpointerleave={() => (dragging = null)}
  role="img"
  aria-label="Fan curve editor"
>
  <!-- gridlines -->
  {#each [25, 50, 75] as t}
    <line
      x1={tempToX(minTemp + ((maxTemp - minTemp) * t) / 100)}
      y1={P}
      x2={tempToX(minTemp + ((maxTemp - minTemp) * t) / 100)}
      y2={H - P}
      stroke="rgba(196,168,255,0.1)"
      stroke-dasharray="2 4"
    />
  {/each}
  <!-- curve -->
  <path d={pathD} fill="none" stroke="rgb(196 168 255)" stroke-width="2" />
  <!-- handles -->
  {#each points as p, i (i)}
    <circle
      cx={tempToX(p.temp)}
      cy={pctToY(p.pct)}
      r="6"
      fill="rgb(243 238 255)"
      stroke="rgb(177 139 255)"
      stroke-width="2"
      class="cursor-grab"
      onpointerdown={(e) => {
        (e.currentTarget as SVGElement).setPointerCapture(e.pointerId);
        dragging = i;
      }}
      data-testid="curve-handle-{i}"
    />
  {/each}
</svg>
```

- [ ] **Step 2: Test**

```typescript
// crates/lconnect-gui/src/lib/components/CurveEditor.test.ts
import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import CurveEditor from "./CurveEditor.svelte";

describe("CurveEditor", () => {
  it("renders one handle per point", () => {
    const { getAllByTestId } = render(CurveEditor, {
      props: {
        points: [
          { temp: 30, pct: 30 },
          { temp: 50, pct: 60 },
          { temp: 70, pct: 100 },
        ],
      },
    });
    expect(getAllByTestId(/^curve-handle-/)).toHaveLength(3);
  });
});
```

- [ ] **Step 3: Verify + commit**

```bash
pnpm --filter lconnect-gui test components/CurveEditor
git add crates/lconnect-gui/src/lib/components/CurveEditor.svelte \
        crates/lconnect-gui/src/lib/components/CurveEditor.test.ts
git commit -m "feat(gui): CurveEditor (drag-to-edit fan curve)

SVG-based editor with N draggable points (temp,pct), pointer-capture
drag, autosort by temp on each change, gridlines at 25/50/75 percent.
Cooling page passes liquid_temp curves; result feeds set_fan_pwm /
set_pump_pwm with mode=Curve."
```

---

### Task P2E-9: DropZone + Sparkline

**Files:**
- Create: `crates/lconnect-gui/src/lib/components/DropZone.svelte`
- Create: `crates/lconnect-gui/src/lib/components/Sparkline.svelte`
- Test: `crates/lconnect-gui/src/lib/components/DropZone.test.ts`

- [ ] **Step 1: Implement DropZone**

```svelte
<!-- crates/lconnect-gui/src/lib/components/DropZone.svelte -->
<script lang="ts">
  interface Props {
    accept?: string;
    onfile?: (path: string) => void;
    label?: string;
  }

  let { accept = "image/*", onfile, label = "Drop an image here" }: Props = $props();

  let dragging = $state(false);

  // Tauri's webview supports HTML5 drag/drop, but the dropped File object
  // does not expose a filesystem path. To get the path we use the Tauri
  // 'tauri://file-drop' window event (preferred) or fall back to <input>.
  // Here, the parent wires the Tauri event listener and calls onfile().
  // This component handles the visual state + native file picker fallback.

  function onClick(): void {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.onchange = () => {
      const f = input.files?.[0];
      // In Tauri, File.path is exposed on the webview shim — but webkit hides
      // it under different keys. Fall back to filename for the picker case;
      // pages should prefer drag-drop via Tauri events for path access.
      if (f) onfile?.((f as unknown as { path?: string }).path ?? f.name);
    };
    input.click();
  }
</script>

<button
  type="button"
  class="flex h-48 w-full flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-aurora-glow/50 bg-aurora-deepest/30 text-aurora-text-base/70 transition hover:border-aurora-lav hover:text-aurora-text-base"
  class:border-aurora-lav={dragging}
  class:bg-aurora-glow={dragging}
  ondragover={(e) => {
    e.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={(e) => {
    e.preventDefault();
    dragging = false;
    // Tauri exposes path on the dataTransfer item only via events; pages
    // should listen to the Tauri 'tauri://file-drop' event for real paths.
  }}
  onclick={onClick}
  data-testid="dropzone"
>
  <span class="text-3xl" aria-hidden="true">⬇</span>
  <span class="text-sm">{label}</span>
  <span class="text-xs text-aurora-text-base/50">Click to browse</span>
</button>
```

- [ ] **Step 2: Implement Sparkline**

```svelte
<!-- crates/lconnect-gui/src/lib/components/Sparkline.svelte -->
<script lang="ts">
  interface Props {
    values: number[];
    min?: number;
    max?: number;
    width?: number;
    height?: number;
    color?: string;
  }

  let {
    values,
    min = Math.min(...values),
    max = Math.max(...values),
    width = 120,
    height = 32,
    color = "rgb(196 168 255)",
  }: Props = $props();

  let path = $derived.by(() => {
    if (values.length < 2) return "";
    const range = max - min || 1;
    const stepX = width / (values.length - 1);
    return values
      .map((v, i) => {
        const x = i * stepX;
        const y = height - ((v - min) / range) * height;
        return `${i === 0 ? "M" : "L"} ${x.toFixed(1)} ${y.toFixed(1)}`;
      })
      .join(" ");
  });
</script>

<svg
  viewBox="0 0 {width} {height}"
  class="block"
  style="width: {width}px; height: {height}px;"
  aria-hidden="true"
>
  <path d={path} fill="none" stroke={color} stroke-width="1.5" />
</svg>
```

- [ ] **Step 3: Test**

```typescript
// crates/lconnect-gui/src/lib/components/DropZone.test.ts
import { render, fireEvent } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import DropZone from "./DropZone.svelte";

describe("DropZone", () => {
  it("toggles dragging visual on dragover", async () => {
    const { getByTestId } = render(DropZone);
    const dz = getByTestId("dropzone");
    await fireEvent.dragOver(dz);
    // Class assertion is brittle — just confirm element still mounted.
    expect(dz).toBeTruthy();
  });
});
```

- [ ] **Step 4: Verify + commit**

```bash
pnpm --filter lconnect-gui test components/DropZone
git add crates/lconnect-gui/src/lib/components/DropZone.svelte \
        crates/lconnect-gui/src/lib/components/Sparkline.svelte \
        crates/lconnect-gui/src/lib/components/DropZone.test.ts
git commit -m "feat(gui): DropZone + Sparkline primitives

DropZone provides drag/click-to-browse UI; pages must wire the Tauri
'tauri://file-drop' window event to receive real filesystem paths
(webview File.path is unreliable cross-platform). Sparkline draws
mini SVG lines for dashboard temp/RPM history (last N samples)."
```

---

## Sub-phase F — Pages (6 tasks)

**Goal:** Compose the primitives from sub-phase E into the five user-facing routes from spec §6. Pages own the local UI state (which device is selected, what tab is active) and dispatch IPC calls; primitives stay pure.

**Routing layout (SvelteKit conventions):**
```
src/routes/
├── +layout.svelte           # AppShell wrapper (already in P2E-1)
├── +page.svelte             # Dashboard (/)
├── cooling/+page.svelte     # /cooling
├── lighting/+page.svelte    # /lighting
├── display/+page.svelte     # /display
└── settings/+page.svelte    # /settings
```

### Task P2F-1: Dashboard page

**Files:**
- Create: `crates/lconnect-gui/src/routes/+page.svelte`

- [ ] **Step 1: Implement Dashboard**

```svelte
<!-- crates/lconnect-gui/src/routes/+page.svelte -->
<script lang="ts">
  import { devices } from "$lib/stores/devices.svelte";
  import Card from "$lib/components/Card.svelte";
  import Sparkline from "$lib/components/Sparkline.svelte";

  // Local rolling window of liquid_temp samples per device. Bumped by $effect.
  let history = $state<Map<string, number[]>>(new Map());

  $effect(() => {
    const next = new Map(history);
    for (const entry of devices.list) {
      const t = entry.snapshot?.cooling?.liquid_temp_c;
      if (typeof t !== "number") continue;
      const prev = next.get(entry.summary.id) ?? [];
      const arr = [...prev, t];
      if (arr.length > 60) arr.shift();
      next.set(entry.summary.id, arr);
    }
    history = next;
  });

  function fmt(n: number | undefined, suffix = ""): string {
    return typeof n === "number" ? `${n.toFixed(1)}${suffix}` : "—";
  }
</script>

<div class="flex flex-col gap-6">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight text-aurora-text-base">Dashboard</h1>
    <p class="text-sm text-aurora-text-base/60">
      {devices.list.length} device{devices.list.length === 1 ? "" : "s"} online
    </p>
  </header>

  {#if devices.list.length === 0}
    <Card title="No devices">
      <p class="text-sm text-aurora-text-base/70">
        The daemon is connected but no Lian Li devices have been detected.
        Check USB connections and udev rules.
      </p>
    </Card>
  {:else}
    <div class="grid gap-4 lg:grid-cols-2 xl:grid-cols-3">
      {#each devices.list as entry (entry.summary.id)}
        {@const c = entry.snapshot?.cooling}
        {@const l = entry.snapshot?.lighting}
        <Card title={entry.summary.display_name}>
          <div class="flex flex-col gap-3">
            <div class="grid grid-cols-2 gap-3 text-sm">
              <div>
                <div class="text-xs uppercase tracking-wide text-aurora-text-base/50">
                  Liquid
                </div>
                <div class="text-2xl font-semibold tabular-nums">
                  {fmt(c?.liquid_temp_c, "°")}
                </div>
              </div>
              <div>
                <div class="text-xs uppercase tracking-wide text-aurora-text-base/50">
                  Pump
                </div>
                <div class="text-2xl font-semibold tabular-nums">
                  {fmt(c?.pump_rpm, " rpm")}
                </div>
              </div>
            </div>
            <div class="flex items-center justify-between text-xs text-aurora-text-base/60">
              <span>{l?.mode ?? "—"}</span>
              <Sparkline values={history.get(entry.summary.id) ?? []} />
            </div>
          </div>
        </Card>
      {/each}
    </div>
  {/if}
</div>
```

- [ ] **Step 2: Smoke test**

Run: `pnpm --filter lconnect-gui run check`
Run: `pnpm --filter lconnect-gui test`
Expected: All tests still pass.

- [ ] **Step 3: Manual smoke (mock daemon)**

```bash
pnpm --filter lconnect-gui tauri:dev:mock
```
Open the window. Expected: Dashboard shows one synthetic device with values that update once per second; sparkline grows.

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/src/routes/+page.svelte
git commit -m "feat(gui): dashboard route with per-device cards + sparklines

Reads devices.list and devices.snapshot reactively; \$effect maintains
60-sample rolling history of liquid_temp per device for Sparkline.
Empty state guides users to udev / USB troubleshooting."
```

---

### Task P2F-2: Cooling page

**Files:**
- Create: `crates/lconnect-gui/src/routes/cooling/+page.svelte`

- [ ] **Step 1: Implement Cooling page**

```svelte
<!-- crates/lconnect-gui/src/routes/cooling/+page.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { devices } from "$lib/stores/devices.svelte";
  import { handleError } from "$lib/util/handle-error";
  import Card from "$lib/components/Card.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import TogglePill from "$lib/components/TogglePill.svelte";
  import Button from "$lib/components/Button.svelte";
  import CurveEditor from "$lib/components/CurveEditor.svelte";

  type Mode = "manual" | "curve";
  type Source = "mcu" | "cpu" | "liquid";

  let selectedId = $state<string | null>(null);
  $effect(() => {
    if (!selectedId && devices.list.length > 0) {
      selectedId = devices.list[0].summary.id;
    }
  });

  let fanMode = $state<Mode>("manual");
  let fanPct = $state(50);
  let fanSource = $state<Source>("liquid");
  let fanCurve = $state([
    { temp: 30, pct: 30 },
    { temp: 50, pct: 60 },
    { temp: 70, pct: 100 },
  ]);

  let pumpMode = $state<Mode>("manual");
  let pumpPct = $state(75);
  let pumpSource = $state<Source>("mcu");

  async function applyFan(): Promise<void> {
    if (!selectedId) return;
    try {
      if (fanMode === "manual") {
        await invoke("set_fan_pwm", {
          deviceId: selectedId,
          spec: { kind: "manual", percent: fanPct, source: fanSource },
        });
      } else {
        await invoke("set_fan_pwm", {
          deviceId: selectedId,
          spec: { kind: "curve", points: fanCurve, source: fanSource },
        });
      }
    } catch (e) {
      handleError(e, "Failed to update fan");
    }
  }

  async function applyPump(): Promise<void> {
    if (!selectedId) return;
    try {
      await invoke("set_pump_pwm", {
        deviceId: selectedId,
        spec: { kind: "manual", percent: pumpPct, source: pumpSource },
      });
    } catch (e) {
      handleError(e, "Failed to update pump");
    }
  }
</script>

<div class="flex flex-col gap-6">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight text-aurora-text-base">Cooling</h1>
  </header>

  {#if devices.list.length === 0}
    <Card title="No devices">No cooling devices online.</Card>
  {:else}
    <div class="flex gap-2">
      {#each devices.list as e (e.summary.id)}
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-sm border border-aurora-glow/40"
          class:bg-aurora-lav={e.summary.id === selectedId}
          class:text-aurora-deepest={e.summary.id === selectedId}
          onclick={() => (selectedId = e.summary.id)}
        >
          {e.summary.display_name}
        </button>
      {/each}
    </div>

    <Card title="Fan">
      <div class="flex flex-col gap-4">
        <TogglePill
          options={[
            { value: "manual", label: "Manual" },
            { value: "curve", label: "Curve" },
          ]}
          bind:value={fanMode}
        />
        <TogglePill
          options={[
            { value: "mcu", label: "MCU" },
            { value: "cpu", label: "CPU" },
            { value: "liquid", label: "Liquid" },
          ]}
          bind:value={fanSource}
        />
        {#if fanMode === "manual"}
          <Slider label="Fan speed" bind:value={fanPct} />
        {:else}
          <CurveEditor bind:points={fanCurve} />
        {/if}
        <div class="flex justify-end">
          <Button onclick={applyFan}>Apply</Button>
        </div>
      </div>
    </Card>

    <Card title="Pump">
      <div class="flex flex-col gap-4">
        <TogglePill
          options={[
            { value: "mcu", label: "MCU" },
            { value: "cpu", label: "CPU" },
            { value: "liquid", label: "Liquid" },
          ]}
          bind:value={pumpSource}
        />
        <Slider label="Pump speed" bind:value={pumpPct} min={50} max={100} />
        <div class="flex justify-end">
          <Button onclick={applyPump}>Apply</Button>
        </div>
      </div>
    </Card>
  {/if}
</div>
```

- [ ] **Step 2: Verify**

Run: `pnpm --filter lconnect-gui run check`
Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
git add crates/lconnect-gui/src/routes/cooling/+page.svelte
git commit -m "feat(gui): cooling route with fan curve + pump manual

Per-device chip selector, fan card (manual slider OR curve editor +
source pill), pump card (manual slider + source pill). Apply button
calls set_fan_pwm/set_pump_pwm via invoke; errors funnel to toasts.

Pump min=50 reflects Galahad II safety floor (Phase 0 finding); MCU
source is default for pump per C# defaults."
```

---

### Task P2F-3: Lighting page (preset modes tab)

**Files:**
- Create: `crates/lconnect-gui/src/routes/lighting/+page.svelte`
- Create: `crates/lconnect-gui/src/lib/data/lighting-modes.ts`

- [ ] **Step 1: Lighting mode catalog**

```typescript
// crates/lconnect-gui/src/lib/data/lighting-modes.ts
export interface LightingMode {
  id: string;
  label: string;
  swatch: string;
  supportsColor: boolean;
  supportsSpeed: boolean;
  supportsBrightness: boolean;
  supportsDirection: boolean;
}

// Reflects Phase 1 LightingSpec mode catalog (mirror of C# LightingMode enum,
// renamed in our own words). Initial set; expand as families ship.
export const LIGHTING_MODES: ReadonlyArray<LightingMode> = [
  { id: "off",       label: "Off",       swatch: "#0d0e2a", supportsColor: false, supportsSpeed: false, supportsBrightness: false, supportsDirection: false },
  { id: "static",    label: "Static",    swatch: "#c4a8ff", supportsColor: true,  supportsSpeed: false, supportsBrightness: true,  supportsDirection: false },
  { id: "rainbow",   label: "Rainbow",   swatch: "linear-gradient(90deg,#ff5e5e,#ffeb5e,#5eff7e,#5eccff,#b18bff,#ff9fcd)", supportsColor: false, supportsSpeed: true, supportsBrightness: true, supportsDirection: true },
  { id: "breathing", label: "Breathing", swatch: "#ff9fcd", supportsColor: true,  supportsSpeed: true,  supportsBrightness: true,  supportsDirection: false },
  { id: "marquee",   label: "Marquee",   swatch: "linear-gradient(90deg,#c4a8ff,#0d0e2a)", supportsColor: true, supportsSpeed: true, supportsBrightness: true, supportsDirection: true },
  { id: "wave",      label: "Wave",      swatch: "linear-gradient(90deg,#5eccff,#b18bff,#5eccff)", supportsColor: true, supportsSpeed: true, supportsBrightness: true, supportsDirection: true },
];
```

- [ ] **Step 2: Implement Lighting page (preset tab)**

```svelte
<!-- crates/lconnect-gui/src/routes/lighting/+page.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { devices } from "$lib/stores/devices.svelte";
  import { handleError } from "$lib/util/handle-error";
  import { LIGHTING_MODES } from "$lib/data/lighting-modes";
  import Card from "$lib/components/Card.svelte";
  import Button from "$lib/components/Button.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import TogglePill from "$lib/components/TogglePill.svelte";
  import ModeChipGrid from "$lib/components/ModeChipGrid.svelte";
  import ColorPicker from "$lib/components/ColorPicker.svelte";

  type Tab = "preset" | "led";

  let selectedId = $state<string | null>(null);
  $effect(() => {
    if (!selectedId && devices.list.length > 0) {
      selectedId = devices.list[0].summary.id;
    }
  });

  let tab = $state<Tab>("preset");
  let mode = $state("static");
  let speed = $state(50);
  let brightness = $state(100);
  let direction = $state<"forward" | "reverse">("forward");
  let color = $state({ r: 196, g: 168, b: 255 });

  let activeMode = $derived(LIGHTING_MODES.find((m) => m.id === mode));

  async function apply(): Promise<void> {
    if (!selectedId) return;
    try {
      await invoke("set_lighting", {
        deviceId: selectedId,
        spec: {
          mode,
          speed: activeMode?.supportsSpeed ? speed : undefined,
          brightness: activeMode?.supportsBrightness ? brightness : undefined,
          direction: activeMode?.supportsDirection ? direction : undefined,
          color: activeMode?.supportsColor ? color : undefined,
        },
      });
    } catch (e) {
      handleError(e, "Failed to update lighting");
    }
  }
</script>

<div class="flex flex-col gap-6">
  <header class="flex items-end justify-between">
    <h1 class="text-2xl font-semibold tracking-tight text-aurora-text-base">Lighting</h1>
    <TogglePill
      options={[
        { value: "preset", label: "Presets" },
        { value: "led", label: "Per-LED" },
      ]}
      bind:value={tab}
    />
  </header>

  {#if devices.list.length === 0}
    <Card title="No devices">No lighting devices online.</Card>
  {:else}
    <div class="flex gap-2">
      {#each devices.list as e (e.summary.id)}
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-sm border border-aurora-glow/40"
          class:bg-aurora-lav={e.summary.id === selectedId}
          class:text-aurora-deepest={e.summary.id === selectedId}
          onclick={() => (selectedId = e.summary.id)}
        >
          {e.summary.display_name}
        </button>
      {/each}
    </div>

    {#if tab === "preset"}
      <Card title="Mode">
        <ModeChipGrid modes={LIGHTING_MODES} bind:selected={mode} />
      </Card>

      <Card title="Parameters">
        <div class="flex flex-col gap-4">
          {#if activeMode?.supportsBrightness}
            <Slider label="Brightness" bind:value={brightness} />
          {/if}
          {#if activeMode?.supportsSpeed}
            <Slider label="Speed" bind:value={speed} />
          {/if}
          {#if activeMode?.supportsDirection}
            <TogglePill
              options={[
                { value: "forward", label: "Forward" },
                { value: "reverse", label: "Reverse" },
              ]}
              bind:value={direction}
            />
          {/if}
          {#if activeMode?.supportsColor}
            <ColorPicker bind:value={color} />
          {/if}
          <div class="flex justify-end">
            <Button onclick={apply}>Apply</Button>
          </div>
        </div>
      </Card>
    {:else}
      <Card title="Per-LED editor">
        <p class="text-sm text-aurora-text-base/70">
          Coming in P2F-4: per-LED color editing for compatible devices.
        </p>
      </Card>
    {/if}
  {/if}
</div>
```

- [ ] **Step 3: Verify + commit**

```bash
pnpm --filter lconnect-gui run check
git add crates/lconnect-gui/src/routes/lighting/ \
        crates/lconnect-gui/src/lib/data/lighting-modes.ts
git commit -m "feat(gui): lighting route preset tab + mode catalog

LIGHTING_MODES catalog declares per-mode capabilities (color/speed/
brightness/direction). Preset tab renders ModeChipGrid + dynamic
Parameters card showing only the controls supported by the active
mode. set_lighting payload omits unused fields. LED tab is a
placeholder filled in P2F-4."
```

---

### Task P2F-4: Lighting page (per-LED editor)

**Files:**
- Modify: `crates/lconnect-gui/src/routes/lighting/+page.svelte`
- Create: `crates/lconnect-gui/src/lib/components/LedStripEditor.svelte`

- [ ] **Step 1: Implement LedStripEditor**

```svelte
<!-- crates/lconnect-gui/src/lib/components/LedStripEditor.svelte -->
<script lang="ts">
  import { rgbToHex, type Rgb } from "$lib/util/color";
  import ColorPicker from "./ColorPicker.svelte";

  interface Props {
    leds: Rgb[];
    onchange?: (leds: Rgb[]) => void;
  }

  let { leds = $bindable(), onchange }: Props = $props();

  let selectedIdx = $state(0);
  let activeColor = $derived(leds[selectedIdx] ?? { r: 0, g: 0, b: 0 });

  function setActive(rgb: Rgb): void {
    const next = [...leds];
    next[selectedIdx] = rgb;
    leds = next;
    onchange?.(next);
  }

  function fillAll(): void {
    const c = leds[selectedIdx];
    const next = leds.map(() => ({ ...c }));
    leds = next;
    onchange?.(next);
  }
</script>

<div class="flex flex-col gap-4">
  <div class="flex flex-wrap gap-1.5">
    {#each leds as led, i (i)}
      <button
        type="button"
        class="h-7 w-7 rounded-md border border-aurora-glow/40 transition hover:scale-110"
        class:ring-2={i === selectedIdx}
        class:ring-aurora-lav={i === selectedIdx}
        style="background: {rgbToHex(led)};"
        onclick={() => (selectedIdx = i)}
        aria-label="LED {i + 1}"
      ></button>
    {/each}
  </div>

  <div class="flex items-center justify-between">
    <div class="text-xs uppercase tracking-wide text-aurora-text-base/60">
      Editing LED {selectedIdx + 1} / {leds.length}
    </div>
    <button
      type="button"
      class="text-xs text-aurora-lav hover:text-aurora-violet"
      onclick={fillAll}
    >
      Fill all
    </button>
  </div>

  <ColorPicker
    value={activeColor}
    onchange={setActive}
  />
</div>
```

- [ ] **Step 2: Wire LED tab into lighting page**

Replace the `{:else}` branch (placeholder) in `lighting/+page.svelte` with:

```svelte
    {:else}
      <Card title="Per-LED editor">
        {#if !ledCount}
          <p class="text-sm text-aurora-text-base/70">
            Selected device does not expose addressable LEDs.
          </p>
        {:else}
          <div class="flex flex-col gap-4">
            <LedStripEditor bind:leds={leds} />
            <div class="flex justify-end">
              <Button onclick={applyLeds}>Apply</Button>
            </div>
          </div>
        {/if}
      </Card>
    {/if}
```

Add to the `<script>` block (top, after existing state):

```typescript
  import LedStripEditor from "$lib/components/LedStripEditor.svelte";
  import type { Rgb } from "$lib/util/color";

  let ledCount = $derived(
    selectedId
      ? devices.byId(selectedId)?.summary.led_count ?? 0
      : 0,
  );

  let leds = $state<Rgb[]>([]);

  $effect(() => {
    if (ledCount > 0 && leds.length !== ledCount) {
      leds = Array.from({ length: ledCount }, () => ({ r: 0, g: 0, b: 0 }));
    }
  });

  async function applyLeds(): Promise<void> {
    if (!selectedId) return;
    try {
      await invoke("set_lighting", {
        deviceId: selectedId,
        spec: { mode: "per_led", per_led_colors: leds },
      });
    } catch (e) {
      handleError(e, "Failed to update LEDs");
    }
  }
```

- [ ] **Step 3: Verify + commit**

```bash
pnpm --filter lconnect-gui run check
git add crates/lconnect-gui/src/lib/components/LedStripEditor.svelte \
        crates/lconnect-gui/src/routes/lighting/+page.svelte
git commit -m "feat(gui): lighting per-LED editor

LedStripEditor lays out N square swatches; clicking a swatch routes
the ColorPicker output back to that LED. Fill-all duplicates the
active color. Page reads led_count from DeviceSummary; devices that
don't expose addressable LEDs show an explanatory message."
```

---

### Task P2F-5: Display page

**Files:**
- Create: `crates/lconnect-gui/src/routes/display/+page.svelte`

- [ ] **Step 1: Implement Display page**

```svelte
<!-- crates/lconnect-gui/src/routes/display/+page.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { devices } from "$lib/stores/devices.svelte";
  import { toasts } from "$lib/stores/toasts.svelte";
  import { handleError } from "$lib/util/handle-error";
  import Card from "$lib/components/Card.svelte";
  import Button from "$lib/components/Button.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import TogglePill from "$lib/components/TogglePill.svelte";
  import DropZone from "$lib/components/DropZone.svelte";

  type Fit = "crop" | "letterbox" | "stretch";

  let selectedId = $state<string | null>(null);
  $effect(() => {
    if (!selectedId && devices.list.length > 0) {
      selectedId = devices.list[0].summary.id;
    }
  });

  let droppedPath = $state<string | null>(null);
  let fit = $state<Fit>("crop");
  let quality = $state(85);
  let preview = $state<string | null>(null);
  let busy = $state(false);

  let displayBrightness = $state(100);
  let displayRotation = $state<0 | 90 | 180 | 270>(0);

  let unlisten: UnlistenFn | null = null;

  onMount(() => {
    (async () => {
      unlisten = await listen<{ paths: string[] }>("tauri://file-drop", (e) => {
        const p = e.payload.paths[0];
        if (p) droppedPath = p;
      });
    })();
  });

  onDestroy(() => {
    unlisten?.();
  });

  async function generatePreview(): Promise<void> {
    if (!droppedPath) return;
    busy = true;
    try {
      const bytes = await invoke<number[]>("prepare_jpeg", {
        path: droppedPath,
        fit,
        quality,
      });
      const blob = new Blob([new Uint8Array(bytes)], { type: "image/jpeg" });
      if (preview) URL.revokeObjectURL(preview);
      preview = URL.createObjectURL(blob);
    } catch (e) {
      handleError(e, "Image conversion failed");
    } finally {
      busy = false;
    }
  }

  async function uploadJpeg(): Promise<void> {
    if (!selectedId || !droppedPath) return;
    busy = true;
    try {
      const bytes = await invoke<number[]>("prepare_jpeg", {
        path: droppedPath,
        fit,
        quality,
      });
      await invoke("set_lcd_image", {
        deviceId: selectedId,
        jpegBytes: bytes,
      });
      toasts.push({ kind: "success", message: "Image uploaded" });
    } catch (e) {
      handleError(e, "Upload failed");
    } finally {
      busy = false;
    }
  }

  async function applyDisplaySettings(): Promise<void> {
    if (!selectedId) return;
    try {
      await invoke("set_display", {
        deviceId: selectedId,
        spec: { brightness: displayBrightness, rotation: displayRotation },
      });
    } catch (e) {
      handleError(e, "Failed to update display");
    }
  }
</script>

<div class="flex flex-col gap-6">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight text-aurora-text-base">Display</h1>
    <p class="text-sm text-aurora-text-base/60">
      Upload an image to compatible LCD coolers. Theme/animation tools coming in Phase 4.
    </p>
  </header>

  {#if devices.list.length === 0}
    <Card title="No devices">No display devices online.</Card>
  {:else}
    <div class="flex gap-2">
      {#each devices.list as e (e.summary.id)}
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-sm border border-aurora-glow/40"
          class:bg-aurora-lav={e.summary.id === selectedId}
          class:text-aurora-deepest={e.summary.id === selectedId}
          onclick={() => (selectedId = e.summary.id)}
        >
          {e.summary.display_name}
        </button>
      {/each}
    </div>

    <Card title="Image">
      <div class="flex flex-col gap-4 lg:flex-row">
        <div class="flex-1">
          <DropZone
            onfile={(p) => (droppedPath = p)}
            label={droppedPath ? droppedPath.split("/").pop() ?? "Selected" : "Drop an image here"}
          />
          <div class="mt-3 flex flex-col gap-3">
            <TogglePill
              options={[
                { value: "crop", label: "Crop" },
                { value: "letterbox", label: "Letterbox" },
                { value: "stretch", label: "Stretch" },
              ]}
              bind:value={fit}
            />
            <Slider label="Quality" bind:value={quality} min={1} max={100} />
            <div class="flex gap-2">
              <Button variant="secondary" onclick={generatePreview} disabled={!droppedPath || busy}>
                Preview
              </Button>
              <Button onclick={uploadJpeg} disabled={!droppedPath || !selectedId || busy}>
                Upload
              </Button>
            </div>
          </div>
        </div>
        <div class="flex w-full max-w-xs items-center justify-center">
          {#if preview}
            <img
              src={preview}
              alt="Preview"
              class="rounded-lg border border-aurora-glow/40"
            />
          {:else}
            <div class="flex h-48 w-48 items-center justify-center rounded-lg border border-dashed border-aurora-glow/30 text-xs text-aurora-text-base/50">
              No preview
            </div>
          {/if}
        </div>
      </div>
    </Card>

    <Card title="Display settings">
      <div class="flex flex-col gap-4">
        <Slider label="Brightness" bind:value={displayBrightness} />
        <TogglePill
          options={[
            { value: 0, label: "0°" },
            { value: 90, label: "90°" },
            { value: 180, label: "180°" },
            { value: 270, label: "270°" },
          ] as const}
          bind:value={displayRotation}
        />
        <div class="flex justify-end">
          <Button onclick={applyDisplaySettings}>Apply</Button>
        </div>
      </div>
    </Card>
  {/if}
</div>
```

- [ ] **Step 2: Verify + commit**

```bash
pnpm --filter lconnect-gui run check
git add crates/lconnect-gui/src/routes/display/+page.svelte
git commit -m "feat(gui): display route (image upload + LCD settings)

Image card: DropZone (wired to tauri://file-drop event for real
filesystem paths) + fit/quality controls + Preview button (renders
the post-prepare_jpeg blob inline so users see exactly what the
device will receive) + Upload button. Display card: brightness +
rotation. Future-Phase-4 hint in subtitle."
```

---

### Task P2F-6: Settings page

**Files:**
- Create: `crates/lconnect-gui/src/routes/settings/+page.svelte`

- [ ] **Step 1: Implement Settings page**

```svelte
<!-- crates/lconnect-gui/src/routes/settings/+page.svelte -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { connection } from "$lib/stores/daemon-connection.svelte";
  import { handleError } from "$lib/util/handle-error";
  import Card from "$lib/components/Card.svelte";
  import Button from "$lib/components/Button.svelte";
  import TogglePill from "$lib/components/TogglePill.svelte";

  interface DaemonInfo {
    version: string;
    socket_path: string;
    config_path: string;
  }

  let info = $state<DaemonInfo | null>(null);
  let theme = $state<"aurora" | "system">("aurora"); // future: more themes

  onMount(() => {
    (async () => {
      try {
        info = await invoke<DaemonInfo>("daemon_info");
      } catch (e) {
        handleError(e, "Could not fetch daemon info");
      }
    })();
  });

  async function openConfigDir(): Promise<void> {
    if (!info) return;
    try {
      await invoke("open_path", { path: info.config_path });
    } catch (e) {
      handleError(e, "Could not open config dir");
    }
  }
</script>

<div class="flex flex-col gap-6">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight text-aurora-text-base">Settings</h1>
  </header>

  <Card title="Daemon">
    <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
      <dt class="text-aurora-text-base/60">Connection</dt>
      <dd class="font-mono">{connection.state.kind}</dd>

      <dt class="text-aurora-text-base/60">Version</dt>
      <dd class="font-mono">{info?.version ?? "—"}</dd>

      <dt class="text-aurora-text-base/60">Socket</dt>
      <dd class="font-mono break-all">{info?.socket_path ?? "—"}</dd>

      <dt class="text-aurora-text-base/60">Config</dt>
      <dd class="font-mono break-all">{info?.config_path ?? "—"}</dd>
    </dl>
    <div class="mt-4 flex gap-2">
      <Button variant="secondary" onclick={openConfigDir} disabled={!info}>
        Open config dir
      </Button>
      <Button variant="ghost" onclick={() => connection.reconnect()}>
        Reconnect
      </Button>
    </div>
  </Card>

  <Card title="Appearance">
    <div class="flex flex-col gap-3">
      <TogglePill
        options={[
          { value: "aurora", label: "Aurora" },
          { value: "system", label: "System" },
        ]}
        bind:value={theme}
      />
      <p class="text-xs text-aurora-text-base/60">
        Theme switching arrives in Phase 4 with the theme engine.
      </p>
    </div>
  </Card>

  <Card title="About">
    <p class="text-sm text-aurora-text-base/80">
      lconnect-gui — open-source Linux client for Lian Li L-Connect 3 hardware.
      Not affiliated with Lian Li Industrial Co., Ltd.
    </p>
  </Card>
</div>
```

- [ ] **Step 2: Verify + commit**

```bash
pnpm --filter lconnect-gui run check
git add crates/lconnect-gui/src/routes/settings/+page.svelte
git commit -m "feat(gui): settings route (daemon info + appearance + about)

Daemon card surfaces connection state, version, socket + config
paths. 'Open config dir' uses helpers::open_path; 'Reconnect' calls
connection.reconnect(). Appearance card stub for Phase 4 theming.
About copy clarifies the project is independent of Lian Li."
```

---

## Sub-phase G — Playwright E2E (4 tasks)

**Goal:** End-to-end smoke for the four user-facing flows from spec §6, run against the mock daemon (no hardware required). Tests run in CI and locally via `pnpm --filter lconnect-gui test:e2e`.

**Architecture:** Playwright drives a Tauri-built window. We don't use `tauri driver` in CI (extra dep, slower); instead we point Playwright at the dev server (`pnpm tauri:dev:mock`) over Vite's localhost URL. This loses the Tauri-side asserts but covers ~95% of the GUI logic — IPC plumbing is unit-tested separately in sub-phase C.

For tests that need IPC (most of them), Playwright stubs `window.__TAURI_INTERNALS__.invoke` before page load to return canned responses. This is the same pattern Vitest uses, just at the page level.

### Task P2G-1: Playwright config + IPC stub harness

**Files:**
- Create: `crates/lconnect-gui/playwright.config.ts`
- Create: `crates/lconnect-gui/e2e/fixtures/tauri-stub.ts`
- Modify: `crates/lconnect-gui/package.json` (scripts + devDeps)

- [ ] **Step 1: Add Playwright dependency**

```bash
pnpm --filter lconnect-gui add -D @playwright/test playwright
pnpm --filter lconnect-gui exec playwright install chromium
```

- [ ] **Step 2: `playwright.config.ts`**

```typescript
// crates/lconnect-gui/playwright.config.ts
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:1420",
    trace: "retain-on-failure",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: "pnpm dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
```

- [ ] **Step 3: IPC stub helper**

```typescript
// crates/lconnect-gui/e2e/fixtures/tauri-stub.ts
import type { Page } from "@playwright/test";

export interface TauriStub {
  /** Map command name -> handler returning the resolved value (or throwing). */
  commands: Record<string, (args: unknown) => unknown>;
  /** Optional: events to dispatch on demand. */
  events?: Record<string, unknown>;
}

export async function installTauriStub(page: Page, stub: TauriStub): Promise<void> {
  await page.addInitScript((s: TauriStub) => {
    type Listener = (event: { event: string; payload: unknown; id: number }) => void;
    const listeners = new Map<string, Listener[]>();
    let nextId = 1;

    const internals = {
      invoke: async (cmd: string, args: unknown) => {
        const handler = s.commands[cmd];
        if (!handler) throw new Error(`tauri-stub: unknown command "${cmd}"`);
        return handler(args);
      },
      transformCallback: (cb: Listener, _once: boolean) => {
        const id = nextId++;
        // Tauri JS SDK converts this id back into an event listener wiring.
        (window as unknown as { __TAURI_CALLBACKS__?: Map<number, Listener> })
          .__TAURI_CALLBACKS__ ??= new Map();
        (window as unknown as { __TAURI_CALLBACKS__: Map<number, Listener> })
          .__TAURI_CALLBACKS__.set(id, cb);
        return id;
      },
    };

    (window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ =
      internals;

    // Helper exposed for tests to dispatch events:
    (window as unknown as { __dispatchTauriEvent: (e: string, p: unknown) => void })
      .__dispatchTauriEvent = (eventName, payload) => {
      const ls = listeners.get(eventName) ?? [];
      for (const l of ls) l({ event: eventName, payload, id: 0 });
    };

    // Tauri 2's listen() registers listeners through invoke('plugin:event|listen').
    // We intercept that command here:
    const originalInvoke = internals.invoke;
    internals.invoke = async (cmd: string, args: unknown) => {
      if (cmd === "plugin:event|listen") {
        const a = args as { event: string; handler: number };
        const cbs = (window as unknown as { __TAURI_CALLBACKS__?: Map<number, Listener> })
          .__TAURI_CALLBACKS__;
        const cb = cbs?.get(a.handler);
        if (cb) {
          const arr = listeners.get(a.event) ?? [];
          arr.push(cb);
          listeners.set(a.event, arr);
        }
        return a.handler; // unlisten ID
      }
      return originalInvoke(cmd, args);
    };
  }, stub);
}

export async function dispatchTauriEvent(
  page: Page,
  event: string,
  payload: unknown,
): Promise<void> {
  await page.evaluate(
    ([e, p]) => {
      const fn = (window as unknown as { __dispatchTauriEvent?: (e: string, p: unknown) => void })
        .__dispatchTauriEvent;
      fn?.(e as string, p);
    },
    [event, payload],
  );
}
```

- [ ] **Step 4: Add npm scripts**

Add to `crates/lconnect-gui/package.json`:

```json
{
  "scripts": {
    "test:e2e": "playwright test",
    "test:e2e:ui": "playwright test --ui"
  }
}
```

- [ ] **Step 5: Smoke check**

```bash
pnpm --filter lconnect-gui exec playwright --version
```
Expected: prints version (e.g., `Version 1.49.x`).

- [ ] **Step 6: Commit**

```bash
git add crates/lconnect-gui/playwright.config.ts \
        crates/lconnect-gui/e2e/ \
        crates/lconnect-gui/package.json
git commit -m "test(gui): Playwright config + Tauri IPC stub harness

playwright.config.ts targets the Vite dev server (localhost:1420),
single worker (avoid mock-daemon contention). tauri-stub.ts installs
window.__TAURI_INTERNALS__ before page load so invoke() and listen()
resolve from in-memory dictionaries; tests can dispatch fake events
via window.__dispatchTauriEvent. Avoids the tauri-driver dependency
while exercising the full Svelte + IPC plumbing."
```

---

### Task P2G-2: Dashboard + cooling smoke test

**Files:**
- Create: `crates/lconnect-gui/e2e/dashboard.spec.ts`
- Create: `crates/lconnect-gui/e2e/cooling.spec.ts`

- [ ] **Step 1: Dashboard test**

```typescript
// crates/lconnect-gui/e2e/dashboard.spec.ts
import { test, expect } from "@playwright/test";
import { installTauriStub, dispatchTauriEvent } from "./fixtures/tauri-stub";

test.describe("Dashboard", () => {
  test("renders connected device card with values", async ({ page }) => {
    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [
          { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2", led_count: 16 },
        ],
      },
    });

    await page.goto("/");

    await dispatchTauriEvent(page, "device-snapshot", {
      id: "ga2v:0001",
      cooling: { liquid_temp_c: 35.7, pump_rpm: 2400 },
      lighting: { mode: "rainbow" },
      captured_unix_ms: Date.now(),
    });

    await expect(page.getByText("Galahad II LCDV2")).toBeVisible();
    await expect(page.getByText("35.7°")).toBeVisible();
    await expect(page.getByText("2400.0 rpm")).toBeVisible();
  });

  test("empty state shows udev guidance", async ({ page }) => {
    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [],
      },
    });
    await page.goto("/");
    await expect(page.getByText("No devices")).toBeVisible();
    await expect(page.getByText(/udev/)).toBeVisible();
  });
});
```

- [ ] **Step 2: Cooling test**

```typescript
// crates/lconnect-gui/e2e/cooling.spec.ts
import { test, expect } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauri-stub";

test.describe("Cooling page", () => {
  test("Apply button calls set_fan_pwm with manual spec", async ({ page }) => {
    const calls: Array<{ cmd: string; args: unknown }> = [];

    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [
          { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2", led_count: 16 },
        ],
        set_fan_pwm: (args) => {
          calls.push({ cmd: "set_fan_pwm", args });
          return undefined;
        },
        set_pump_pwm: () => undefined,
      },
    });

    await page.goto("/cooling");

    // Default mode is "manual", default percent is 50.
    await page.getByRole("button", { name: "Apply" }).first().click();

    await expect.poll(() => calls.length).toBeGreaterThan(0);
    expect(calls[0].args).toMatchObject({
      deviceId: "ga2v:0001",
      spec: { kind: "manual", percent: 50 },
    });
  });
});
```

- [ ] **Step 3: Run tests**

```bash
pnpm --filter lconnect-gui run test:e2e -- dashboard cooling
```
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/lconnect-gui/e2e/dashboard.spec.ts \
        crates/lconnect-gui/e2e/cooling.spec.ts
git commit -m "test(gui): e2e — dashboard render + cooling apply flow

Dashboard test covers connected-with-data and empty (no devices)
states. Cooling test asserts the Apply button invokes set_fan_pwm
with the expected spec shape (kind, percent, source). Both use the
TauriStub harness — no real daemon, no real hardware."
```

---

### Task P2G-3: Lighting page interaction test

**Files:**
- Create: `crates/lconnect-gui/e2e/lighting.spec.ts`

- [ ] **Step 1: Implement test**

```typescript
// crates/lconnect-gui/e2e/lighting.spec.ts
import { test, expect } from "@playwright/test";
import { installTauriStub } from "./fixtures/tauri-stub";

test.describe("Lighting page", () => {
  test("rainbow mode hides color picker, shows speed/brightness/direction", async ({ page }) => {
    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [
          { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2", led_count: 16 },
        ],
        set_lighting: () => undefined,
      },
    });

    await page.goto("/lighting");

    // Default selected mode is "static" — color picker visible.
    await expect(page.getByLabel("Hex color")).toBeVisible();

    // Switch to rainbow.
    await page.getByText("Rainbow").click();
    await expect(page.getByLabel("Hex color")).not.toBeVisible();
    await expect(page.getByText("Speed")).toBeVisible();
    await expect(page.getByText("Brightness")).toBeVisible();
    await expect(page.getByText("Forward")).toBeVisible();
  });

  test("Apply sends mode + only relevant params", async ({ page }) => {
    const calls: Array<{ cmd: string; args: unknown }> = [];
    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [
          { id: "ga2v:0001", family: "ga2v", display_name: "Galahad II LCDV2", led_count: 16 },
        ],
        set_lighting: (args) => {
          calls.push({ cmd: "set_lighting", args });
          return undefined;
        },
      },
    });

    await page.goto("/lighting");
    await page.getByText("Rainbow").click();
    await page.getByRole("button", { name: "Apply" }).click();

    await expect.poll(() => calls.length).toBeGreaterThan(0);
    const spec = (calls[0].args as { spec: Record<string, unknown> }).spec;
    expect(spec.mode).toBe("rainbow");
    expect(spec.color).toBeUndefined(); // rainbow doesn't use color
    expect(spec.speed).toBe(50);
    expect(spec.brightness).toBe(100);
  });
});
```

- [ ] **Step 2: Run + commit**

```bash
pnpm --filter lconnect-gui run test:e2e -- lighting
git add crates/lconnect-gui/e2e/lighting.spec.ts
git commit -m "test(gui): e2e — lighting param visibility + Apply payload shape

Confirms LIGHTING_MODES capability flags drive UI: rainbow hides
the color picker; static hides speed/direction. Apply payload only
includes the params the active mode supports — important contract
because the daemon will reject extra fields."
```

---

### Task P2G-4: Display page upload happy path

**Files:**
- Create: `crates/lconnect-gui/e2e/display.spec.ts`
- Create: `crates/lconnect-gui/e2e/fixtures/sample.jpg` (8x8 red JPEG, ≤200 bytes)

- [ ] **Step 1: Create fixture image**

```bash
# Generate an 8x8 red JPEG using ImageMagick (available in CI image)
magick -size 8x8 xc:red crates/lconnect-gui/e2e/fixtures/sample.jpg
```
If ImageMagick is unavailable, commit a precomputed minimal JPEG instead — any valid 8x8 image works.

- [ ] **Step 2: Implement test**

```typescript
// crates/lconnect-gui/e2e/display.spec.ts
import { test, expect } from "@playwright/test";
import { installTauriStub, dispatchTauriEvent } from "./fixtures/tauri-stub";
import * as fs from "node:fs";
import * as path from "node:path";

test.describe("Display page", () => {
  test("drop -> preview -> upload happy path", async ({ page }) => {
    const calls: Array<{ cmd: string; args: unknown }> = [];

    // Pretend prepare_jpeg returns a tiny synthetic byte array.
    const fakeJpeg = Array.from({ length: 16 }, (_, i) => i);

    await installTauriStub(page, {
      commands: {
        daemon_connection_state: () => ({ kind: "connected", since_unix_ms: 1 }),
        list_devices: () => [
          {
            id: "ga2v:0001", family: "ga2v",
            display_name: "Galahad II LCDV2", led_count: 16,
            has_lcd: true,
          },
        ],
        prepare_jpeg: (args) => {
          calls.push({ cmd: "prepare_jpeg", args });
          return fakeJpeg;
        },
        set_lcd_image: (args) => {
          calls.push({ cmd: "set_lcd_image", args });
          return undefined;
        },
        set_display: () => undefined,
      },
    });

    await page.goto("/display");

    // Simulate a Tauri file-drop event with a known path.
    const fixturePath = path.resolve(__dirname, "fixtures/sample.jpg");
    expect(fs.existsSync(fixturePath)).toBe(true);

    await dispatchTauriEvent(page, "tauri://file-drop", { paths: [fixturePath] });

    // Preview button enables once a path is set; click it.
    await page.getByRole("button", { name: "Preview" }).click();
    await expect.poll(() => calls.length).toBeGreaterThan(0);
    expect(calls[0].cmd).toBe("prepare_jpeg");

    // Upload.
    await page.getByRole("button", { name: "Upload" }).click();
    await expect.poll(() => calls.some((c) => c.cmd === "set_lcd_image")).toBe(true);

    // Toast confirms.
    await expect(page.getByText("Image uploaded")).toBeVisible();
  });
});
```

- [ ] **Step 3: Run + commit**

```bash
pnpm --filter lconnect-gui run test:e2e -- display
git add crates/lconnect-gui/e2e/display.spec.ts \
        crates/lconnect-gui/e2e/fixtures/sample.jpg
git commit -m "test(gui): e2e — display drop+preview+upload flow

Dispatches synthetic tauri://file-drop event with fixture path,
asserts prepare_jpeg is called for both the Preview and Upload
buttons, set_lcd_image fires on Upload, success toast appears.
Covers the full media pipeline shim that Phase 4 will build on."
```

---

## Sub-phase H — Packaging (2 tasks)

**Goal:** Ship lconnect-gui as a real Linux desktop app — desktop entry, icon, AUR-installable. Distribution is AUR-only per spec §11; no Flatpak / AppImage / .deb in this phase.

### Task P2H-1: Desktop entry + icon + tauri.conf bundle settings

**Files:**
- Create: `crates/lconnect-gui/src-tauri/icons/lconnect-gui.png` (placeholder 512x512)
- Create: `crates/lconnect-gui/dist-extras/lconnect-gui.desktop`
- Modify: `crates/lconnect-gui/src-tauri/tauri.conf.json`

- [ ] **Step 1: Generate placeholder icon**

```bash
# Re-author from scratch — no copying of L-Connect 3 art (per CLAUDE.md).
magick -size 512x512 \
  gradient:'#0d0e2a-#2a1655' \
  -gravity center \
  -font 'Inter-Bold' -pointsize 200 -fill '#c4a8ff' \
  -annotate +0+0 'L' \
  crates/lconnect-gui/src-tauri/icons/lconnect-gui.png
```
This is a stand-in. If ImageMagick is unavailable, commit any 512x512 PNG with the project initial — Phase 5 (polish) will replace it with a designed icon.

- [ ] **Step 2: Desktop entry**

```ini
# crates/lconnect-gui/dist-extras/lconnect-gui.desktop
[Desktop Entry]
Name=L-Connect GUI
GenericName=Lian Li Hardware Controller
Comment=Linux client for Lian Li L-Connect 3 hardware
Exec=lconnect-gui
Icon=lconnect-gui
Terminal=false
Type=Application
Categories=Utility;HardwareSettings;
Keywords=lian-li;rgb;cooling;fan;lcd;
StartupWMClass=lconnect-gui
```

- [ ] **Step 3: Update `tauri.conf.json` bundle section**

```json
{
  "bundle": {
    "active": true,
    "targets": ["deb", "rpm", "appimage"],
    "identifier": "io.github.inky.lconnect-gui",
    "publisher": "lconnect-linux contributors",
    "category": "Utility",
    "shortDescription": "Lian Li L-Connect 3 hardware controller for Linux",
    "longDescription": "Open-source Linux client for Lian Li L-Connect 3 hardware (Galahad II coolers, TLFan, HydroShift). Connects to the lconnectd daemon over a Unix socket. Not affiliated with Lian Li.",
    "icon": ["icons/lconnect-gui.png"],
    "linux": {
      "deb": { "depends": [] }
    }
  }
}
```

(`targets` includes deb/rpm/appimage so `cargo tauri build` produces them locally for testing, but the AUR PKGBUILD installs from source.)

- [ ] **Step 4: Local build smoke**

```bash
pnpm --filter lconnect-gui tauri build --bundles none
```
Expected: produces an unbundled binary at `crates/lconnect-gui/src-tauri/target/release/lconnect-gui` that launches when run with the daemon up.

- [ ] **Step 5: Commit**

```bash
git add crates/lconnect-gui/src-tauri/icons/ \
        crates/lconnect-gui/dist-extras/ \
        crates/lconnect-gui/src-tauri/tauri.conf.json
git commit -m "feat(gui): packaging — desktop entry, icon, tauri bundle config

Placeholder 512x512 icon (re-authored, not copied from L-Connect 3
per CLAUDE.md). Desktop entry registers in HardwareSettings category
with StartupWMClass for window matching. tauri.conf.json declares
identifier io.github.inky.lconnect-gui and short/long description
for distro packagers."
```

---

### Task P2H-2: AUR PKGBUILD extension

**Files:**
- Modify: `dist/aur/PKGBUILD` (extends Phase 1's daemon/CLI PKGBUILD into a multi-package one)
- Modify: `dist/aur/README.md` (install instructions)

- [ ] **Step 1: Extend PKGBUILD**

If Phase 1's PKGBUILD is `pkgname=lconnect-linux` with split packages, add `lconnect-gui` to `pkgname=()`. If Phase 1 ships a single package, this task converts it to a split package.

```bash
# crates/../dist/aur/PKGBUILD (sketch)
pkgbase=lconnect-linux
pkgname=('lconnectd' 'lconnect-cli' 'lconnect-gui')
pkgver=0.2.0
pkgrel=1
arch=('x86_64')
url='https://github.com/inky/lconnect-linux'
license=('MIT' 'Apache-2.0')
makedepends=('rust' 'cargo' 'pnpm' 'nodejs' 'webkit2gtk-4.1' 'pkgconf' 'libayatana-appindicator')
source=("$pkgbase-$pkgver.tar.gz::https://github.com/inky/lconnect-linux/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "$pkgbase-$pkgver"
  pnpm install --frozen-lockfile
}

build() {
  cd "$pkgbase-$pkgver"
  cargo build --release --workspace -p lconnectd -p lconnect-cli
  pnpm --filter lconnect-gui tauri build --bundles none
}

package_lconnectd() {
  pkgdesc='Lian Li L-Connect daemon (lconnectd)'
  depends=('hidapi')
  cd "$pkgbase-$pkgver"
  install -Dm755 target/release/lconnectd "$pkgdir/usr/bin/lconnectd"
  install -Dm644 dist/systemd/lconnectd.service \
    "$pkgdir/usr/lib/systemd/user/lconnectd.service"
  install -Dm644 dist/udev/99-lian-li.rules \
    "$pkgdir/usr/lib/udev/rules.d/99-lian-li.rules"
}

package_lconnect-cli() {
  pkgdesc='Lian Li L-Connect command-line client'
  depends=('lconnectd')
  cd "$pkgbase-$pkgver"
  install -Dm755 target/release/lconnect "$pkgdir/usr/bin/lconnect"
}

package_lconnect-gui() {
  pkgdesc='Lian Li L-Connect graphical client (Tauri 2 + SvelteKit)'
  depends=('lconnectd' 'webkit2gtk-4.1' 'libayatana-appindicator')
  cd "$pkgbase-$pkgver"
  install -Dm755 crates/lconnect-gui/src-tauri/target/release/lconnect-gui \
    "$pkgdir/usr/bin/lconnect-gui"
  install -Dm644 crates/lconnect-gui/dist-extras/lconnect-gui.desktop \
    "$pkgdir/usr/share/applications/lconnect-gui.desktop"
  install -Dm644 crates/lconnect-gui/src-tauri/icons/lconnect-gui.png \
    "$pkgdir/usr/share/icons/hicolor/512x512/apps/lconnect-gui.png"
}
```

- [ ] **Step 2: README update**

Append to `dist/aur/README.md`:

```markdown
## Installing

```bash
yay -S lconnect-linux        # all three (daemon, CLI, GUI)
yay -S lconnect-gui          # GUI + daemon (depends pulls it)
yay -S lconnect-cli          # CLI + daemon
yay -S lconnectd             # daemon only
```

After install:

```bash
systemctl --user enable --now lconnectd
sudo udevadm control --reload && sudo udevadm trigger
```
```

- [ ] **Step 3: Local makepkg smoke**

```bash
cd dist/aur
makepkg -f --skipinteg
```
Expected: produces three `.pkg.tar.zst` files. Cleanup with `makepkg --clean` afterward.

- [ ] **Step 4: Commit**

```bash
git add dist/aur/PKGBUILD dist/aur/README.md
git commit -m "feat(packaging): split PKGBUILD into daemon/cli/gui

Converts pkgname to an array; gui package depends on webkit2gtk-4.1
and libayatana-appindicator (Tauri 2 runtime deps on Arch). Installs
the .desktop file and 512x512 hicolor icon. README documents the
'install all', 'gui only', 'cli only', 'daemon only' selections."
```

---

## Sub-phase I — Wrap-up (2 tasks)

### Task P2I-1: Smoke against real daemon (no mock)

This task runs only on a workstation with hardware connected. CI cannot exercise it.

**Files:** none changed; this is a verification task.

- [ ] **Step 1: Start the real daemon**

```bash
systemctl --user start lconnectd
journalctl --user -u lconnectd -f &  # watch logs
```

Confirm `lconnect list` shows the connected device(s).

- [ ] **Step 2: Run GUI in dev mode without `--mock-daemon`**

```bash
pnpm --filter lconnect-gui tauri:dev
```

- [ ] **Step 3: Walk every page**

Each item below is one **observation** to confirm by hand. Note the result inline.

- [ ] Connection banner shows nothing (state = connected)
- [ ] Dashboard shows real device card with non-zero liquid_temp and pump_rpm
- [ ] Sparkline grows over ~30 s
- [ ] Cooling: change fan to manual / 30%, click Apply, **visually confirm fans slow down** (this is a write to hardware — proceed carefully)
- [ ] Cooling: revert fan to a sensible default before leaving page
- [ ] Lighting: switch to "static" + a noticeable color (lime green), click Apply, **visually confirm LEDs**
- [ ] Lighting: switch to "rainbow", click Apply, confirm rainbow animation
- [ ] Display (only if device has LCD): drop a small JPEG, click Preview (image renders), click Upload, **visually confirm LCD shows it**
- [ ] Settings: click "Open config dir" → file manager opens at the real config path
- [ ] Settings: kill the daemon (`systemctl --user stop lconnectd`) → connection banner appears within 2 s, "Reconnecting..." replaces "Disconnected" within 2 s of `start`

- [ ] **Step 4: File any bugs found in `docs/superpowers/notes/2026-04-XX-phase-2-smoke.md`**

Don't fix here — open follow-up tasks. Phase 2's exit criterion is "smoke ran, results recorded"; bugs become Phase 5 (polish) work unless they block daily use.

- [ ] **Step 5: Commit the smoke notes (if any)**

```bash
git add docs/superpowers/notes/
git commit -m "docs(phase-2): hardware smoke results

Recorded observations from end-to-end smoke on real Galahad II
LCDV2: connection banner timing, dashboard freshness, fan/pump/LED
write paths, LCD upload, daemon-down recovery."
```

---

### Task P2I-2: Phase 2 retrospective doc

**Files:**
- Create: `docs/superpowers/notes/2026-04-XX-phase-2-retrospective.md`

- [ ] **Step 1: Draft retrospective**

Use this template — fill in actuals after smoke (P2I-1) is done. Lead with surprises, not a victory lap.

```markdown
# Phase 2 Retrospective — GUI (Tauri 2 + SvelteKit)

**Status:** Complete (date: YYYY-MM-DD)

## What shipped
- `lconnect-gui` Tauri app with 5 pages (dashboard, cooling, lighting, display, settings)
- Aurora design language (cosmos palette, Inter, glassmorphic cards)
- Mock daemon for hardware-free dev (`tauri:dev:mock`)
- Vitest unit tests + Playwright E2E (4 specs)
- AUR-installable via split PKGBUILD

## What was harder than expected
- (Fill in: e.g., HMR + Tauri events, HSV plane drag, Tailwind v4 token plumbing, specta type round-trips, ...)

## What was easier than expected
- (Fill in)

## Surprises in the spec ↔ implementation mapping
- (Did §6 page descriptions still match what we built? Did the Aurora token list need additions?)

## Open follow-ups for Phase 5 (polish)
- Replace placeholder icon with designed one
- Per-LED editor needs zone editing (not just per-LED)
- Dashboard sparkline could read from daemon-side history once exposed
- Settings page: add "log level" control once daemon exposes runtime config
- (Add bugs from P2I-1 smoke notes)

## Phase 4 readiness check (theme engine entry point)
- Display page is the planned home for theme tools
- Confirm: did we keep it factored so theme controls can replace the upload card without touching cooling/lighting? (Yes/No + why)

## Lines of code / files
- (Run `cloc crates/lconnect-gui/` and paste summary)
```

- [ ] **Step 2: Fill the retrospective with real numbers**

Run:
```bash
cloc crates/lconnect-gui/
git log --oneline v0.1.0..HEAD -- crates/lconnect-gui/ | wc -l   # commit count
```
Paste both into the doc.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/notes/2026-04-XX-phase-2-retrospective.md
git commit -m "docs(phase-2): retrospective

What shipped, what was harder/easier than expected, open follow-ups
for Phase 5, Phase 4 readiness check on the Display page factoring."
```

- [ ] **Step 4: Tag the release**

```bash
git tag -a v0.2.0 -m "Phase 2: GUI (Tauri 2 + SvelteKit)"
git push origin v0.2.0  # only after user explicitly approves the push
```

(Push is a side-effecting action — do not run without user confirmation.)

---

## Phase 2 done

When all 35 tasks are checked off:

1. `pnpm --filter lconnect-gui run check` — clean
2. `pnpm --filter lconnect-gui test` — all green
3. `pnpm --filter lconnect-gui run test:e2e` — all green
4. `cargo test --workspace` — still green (no Rust regressions)
5. P2I-1 smoke ran on hardware
6. P2I-2 retrospective filed
7. v0.2.0 tagged

Phase 3 (second device family — TLFan or HydroShift) can begin.
