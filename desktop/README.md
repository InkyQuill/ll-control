# L-Connect Linux desktop

Tauri 2 + SvelteKit/Svelte 5, Linux. Current pages: LCD composition editor, sensor catalog, Cooling, Lighting and
Settings. Cooling presets use the Rust reference functions; UI edits require
explicit Apply. Thermal rules load from daemon before editing.

```sh
pnpm install --frozen-lockfile
pnpm check
pnpm check:contracts
pnpm build
pnpm exec playwright install chromium
pnpm exec playwright test
pnpm tauri dev
```

For installation and recovery see the [quickstart](../docs/quickstart.md).
Run these commands from `desktop/`. `tauri dev` opens a real window and connects
to the existing daemon; it never starts a daemon or accesses HID itself.
During development verification use headless Playwright mocks or the
[isolated native smoke](../docs/parity/native-desktop-smoke.md); hardware tests
are deferred. The daemon can replay saved settings when started separately.

The bridge resolves the shared XDG runtime socket and keeps a connection.
Only explicit Apply/LocalUI actions mutate the screen; polling reads sensors
and state, never writes settings. After an ambiguous timeout mutations are
not retried automatically. Read state before reapplying. The bridge excludes
raw commands, reset and firmware operations and grants its command only to
local `main` window content. No filesystem/shell plugins are enabled.

Themes: name/background, text, clock, sensor bindings, image layer, position,
size/alignment/colors, alpha, fit and layer order. Preview supports one-shot and live daemon rendering; it is not proof of
physical output. Apply starts the daemon-owned 1Hz theme and queues persistence.
The UI reports pending/write errors and the last completed save separately. GUI can close while daemon continues rendering. Device changes
keep the current local draft; loading the active theme is explicit. A missing
selected device is retained instead of selecting another device automatically.
Device actions are disabled until it returns or you explicitly select another.
Late theme loads cannot overwrite edits made while waiting; stale status from
a previously selected device is discarded.

Image imports accept <=16 MiB and normalize in Rust to <=960px PNG before
the bounded daemon upload. Built-in editable layouts: Focus (1 sensor), Duo (2), Triple (3), Quad (4), Cycle and Clock.
Clock uses separate editable date/time/AM-PM layers with independent styles;
legacy clock layers keep their previous format. Sensor order is explicit. Sensor/Cycle label and unit can have independent
styles; image contain_native keeps small logos at their managed size. Complex PNGs fall back to native 480px to fit the wire
budget. Font selection (Sans/Serif/Mono), text opacity/shadows, image alpha shadows and Windows watermark center placement, profile slots and
conservative asset cleanup are available. Input validation errors are reported
by daemon. Native happy paths passed for Display/Sensors/Cooling/Lighting/Settings,
including control restart/applied. Profile/cleanup and reconnect/error slices have dedicated acceptance records;
OS file-picker interaction and remaining accessibility cases are still open.
See the [current checklist](../docs/parity/basic-parity-checklist.md).

The icon is an original provisional geometric L; it is not a Lian Li asset.

Text blocks also support optional wrapping width, line height and top/center/
bottom vertical anchor. Legacy themes retain previous spacing and placement.
Wrapping preserves explicit blank lines and Unicode grapheme clusters.

Settings suppresses responses from a previous device/connection without replacing
local brightness/rotation inputs. Profile lists must be refreshed after reconnect;
late edit results cannot restore a stale library. Asset cleanup scans expire when
the draft asset set or connection changes. These UI guards do not cancel commands
already accepted by the daemon; refresh reads the resulting persisted state.
Browser regressions live in `tests/settings-library-reconnect.spec.ts`.

Lighting shares an in-flight mutation guard between manual/thermal controls and
Dynamic RGB. Apply buttons cannot issue overlapping writes from this panel;
local draft fields remain editable. Manual mutations invalidate the Dynamic RGB
status until a fresh poll completes. This is UI coordination, not an exclusive
lock against other IPC clients.
