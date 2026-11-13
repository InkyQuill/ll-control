# LL-Control desktop

Tauri 2 + SvelteKit/Svelte 5, Linux. Device Studio starts with the device overview;
the selected device exposes its implemented Screen, Cooling and Lighting sections.
LCD Ready-made themes, Composition and Library are peer panels. Recording is in
Screen → Library → Video library. Sensors, System information and Settings remain
global. Only Galahad II LCDV2 currently has implemented device editors; the
prototype's other families do not declare hardware support. See [design](../DESIGN.md).
Cooling presets use the Rust reference functions; UI edits require
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

For installation and recovery see the [quickstart](../docs/development/quickstart.md).
Run these commands from `desktop/`. `tauri dev` opens a real window and connects
to the existing daemon; it never starts a daemon or accesses HID itself.
During development verification use headless Playwright mocks or the
[isolated native smoke](../docs/development/parity/native-desktop-smoke.md); hardware tests
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
See the [current checklist](../docs/development/parity/basic-parity-checklist.md).

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

LCD preview geometry comes from the implemented device capability registry: GALAHAD II LCDV2 is square, 480×480. The same metadata drives its inventory glyph; circular clipping is reserved for explicitly round displays. This does not enable an additional hardware family.

ScreenRecord has an opt-in real PipeWire transport check, independent of the
user's desktop and physical devices. From the repository root run
`python3 desktop/scripts/test-recording-pipewire.py`. It owns a private server
and synthetic video producer, verifies inherited FD transport, serial target
property, crop/codec output, pause/resume and remote-loss cleanup. It manually
links nodes and does not qualify real portal permissions or automatic routing.
See the [recording contract](../docs/development/parity/screen-record.md).

Native recording and region selection accept explicit UI language, using a private
X11 display, accessibility bus, synthetic source and MockBackend. From the
repository root, after building the GUI with `native-smoke,custom-protocol`, run
`python3 desktop/scripts/native-recording.py --language en-US` and then
`python3 desktop/scripts/native-recording.py --language ru-RU`. Run sequentially.
Each case checks actual pointer input for capture/video regions, injected capture
loss, draft ownership, crop retained through review recovery, decode, explicit
Save/membership and joined cleanup on normal close. The separate `--full-limit`
workflow remains Russian-only. This does not qualify real portal routing,
Wayland/scaling, physical hardware, or encoded pixel parity with Windows.

Native image-import membership readback faults use the same isolated language
harness. After building with `native-smoke,custom-protocol`, run from the root:
`python3 desktop/scripts/native-interface-language.py --collection-readback-errors`.
Seven faults run in EN/RU: stale revision, missing member, null collection,
pending with/without literal detail, raw RPC error and malformed membership.
The proxy forwards the actual collection write and alters only its following
readback. The harness verifies fourteen committed assets/revisions, preserved
adjacent membership, no duplicate write/Save, unchanged draft, zero LCD JPEG
writes and joined Close. Other fault modes are separate sequential invocations;
this is MockBackend evidence, with no physical hardware or real capture-portal qualification.

### Stored cooling-mode native acceptance

After the ordinary frontend build and native-smoke GUI/example builds, run
`LCONNECT_NATIVE_COOLING_MODES=1 python3 desktop/scripts/native-smoke.py` from the
repository root. This opt-in owned fixture seeds8 fan/3 pump modes once; it never
constructs a real HID backend. The native RU workflow reads modes, loads FixRPM2300
without applying, edits the stored snapshot to2400 RPM, loses the actual Save
acknowledgement and confirms the durable bank by reading without replaying Save.
Only a separate stored-mode Apply sends60% through MockBackend and records
FixRPM as requested identity. A dedicated fixture restart verifies the identity
and2400 RPM snapshot replay before other controls replace them. The full core
workflow and GUI-close/daemon-restart checks ensure the same banks survive.
Missing banks are never reseeded after the marker is created, so loss fails the
acceptance. Browser EN/RU, actual keyboard/speech/session and hardware scopes
remain separate. See the [contract](../docs/development/parity/profile-groups.md#device-modes-read-only-rpc-и-редактор--2026-10-17).

For a fresh-store initialization run, add `LCONNECT_NATIVE_COOLING_DEFAULTS=1`
to that command. The fixture then never seeds modes. The actual UI chooses
`fixture:cpu` and calls production Initialize; the proxy loses its post-commit
reply and asserts exactly one request. The bank contains8/3 factory modes, all
factory labels carry explicit localization intent, and FixRPM starts at2200 RPM.
Initialization must not alter requested policies/IDs. The remaining native
Save2400/Apply60%/identity restart/core flow then runs normally.
See [cooling modes](../docs/development/parity/cooling-modes.md).

### Windows FanAndLighting native acceptance

After `pnpm build` in desktop, build `lconnect-gui` with features
`native-smoke,custom-protocol` and the daemon `desktop-mock` example. Run from the
repository root, sequentially:

```sh
python3 desktop/scripts/native-windows-control.py --english
python3 desktop/scripts/native-windows-control.py
```

The private Xvfb/D-Bus/FileChooser/MockBackend scenario uses the owned Control.zip
fixture. It checks source cancellation, explicit record/six bindings, read-only
review, edit fence, Apply, preserved Linux slots/Screen/local draft, readback and
reapply without another commit. It closes normally, restarts the daemon, removes
the source ZIP and restarts the GUI to recover only the original receipt. A JSON
acceptance report and logs remain in the printed temporary directory. No real HID
backend or system FileChooser is exercised. Do not run frontend builds/checks or
edit frontend sources while a native harness is live.

### Shared media engine

GIF/MP4 preparation, video adjustments and range validation now live in
[lconnect-media](../crates/lconnect-media/README.md). Desktop module paths remain
API facades. Media/preview/source/import owners keep their existing admission,
cancellation/reaping and private-file lifetime semantics. The same engine is used
for offline Windows ZIP media preparation; its result does not publish assets.

## Basic LCD editor banks

Themes with saved basic editor banks expose Background preset and Text preset
in Composition. Each background retains its text settings; background/gradient
and source watermark edits are shared by the seven text presets of that bank.
Selection and Profile Load only change the draft; Save and device Apply remain
separate actions. Themes without banks keep the existing composition editor.

After the ordinary frontend build, build lconnect-gui with
`native-smoke,custom-protocol` and the daemon desktop-mock example. Run
`python3 desktop/scripts/native-basic-editors.py --english` and then
`python3 desktop/scripts/native-basic-editors.py` from the repository root.
Each isolated MockBackend run exercises14 renderer previews, explicit Save/Apply,
normal Close and cold daemon+GUI restart. Keep native harnesses sequential; do
not edit/build/check frontend while one is live. Physical output is separate.
[Contract](../docs/development/parity/windows-profiles.md#basic-editor-user-workflow).

### Windows Screen import and editor acceptance

Build the frontend, `lconnect-gui` with `native-smoke,custom-protocol`, and the
`lconnect-daemon` `desktop-mock` example. Run sequentially from the repository root:

```sh
python3 desktop/scripts/native-windows-screen.py --english
python3 desktop/scripts/native-windows-screen.py
```

The harness sets private XDG directories before starting D-Bus/Xvfb and uses only
MockBackend. It generates an owned PNG Screen ZIP, tests the private FileChooser,
explicit sensor/font/resource bindings, preflight without Apply, one group commit,
independent current scene/editor, explicit editor load and sensor preview, square
LCD geometry, normal Close and GUI/daemon restart. The source ZIP is removed
before cold receipt recovery. It does not qualify genuine Windows exports, moving
video import in the GUI, pixel/font parity, real portals or physical LCD output.

Windows Screen preparation offers **Show prepared frame** before Apply. Choose
current Windows scene or imported editor; preview uses private staged assets,
current sensor readings and the imported LCD rotation. Videos show the first
frame. Preview leaves managed catalogs, saved settings and the LCD unchanged.
Apply remains a separate explicit action and is disabled during rendering.
Native owned PNG/MP4/GIF EN/RU acceptance includes both previews and restart.
The MP4 fixture includes a live CPU overlay; actual complete H.264 writes decode
before and after restart. Other GUI source variants, expanded faults and hardware
acceptance remain pending.
See [evidence](../docs/development/parity/evidence/2026-10-17-windows-screen-private-preview.json).

For native Windows Screen video + sensor throughput acceptance, build the
optimized isolated daemon fixture and use it explicitly:

```sh
cargo build --release -p lconnect-daemon --example desktop-mock
python3 desktop/scripts/native-windows-screen.py --video --english --release-daemon
python3 desktop/scripts/native-windows-screen.py --video --release-daemon
python3 desktop/scripts/native-windows-screen.py --gif --english --release-daemon
python3 desktop/scripts/native-windows-screen.py --gif --release-daemon
python3 desktop/scripts/native-windows-screen.py --corrupt-video --english --release-daemon
python3 desktop/scripts/native-windows-screen.py --corrupt-video --release-daemon
```

Run these sequentially after the native GUI build. The debug daemon is too slow
for this sensor/shadow composition throughput gate. The harness reassembles a
bounded H.264 stream from completed MockBackend writes, checks a short steady
rate, decodes all frames and repeats decoding after restart without source files.
It does not open a real HID backend.

Malformed media cases seed an existing LCD theme and verify that rejection
preserves its emitted JPEG bytes, the open draft, profile library and resource
catalogs. Existing theme refresh continues; the failed import starts no video
stream and releases private preparation. See the
[native media matrix](../docs/development/parity/evidence/2026-10-17-windows-screen-media-matrix.json).

Divider layers support horizontal/vertical orientation, fractional coordinates
and length, thickness0.25–16px, color, opacity and an optional bounded shadow.
They remain local until Save/Apply and survive basic bank switching, portable
archives and restart. Windows Three/FourSensor conversion adds editable2px
separators with explicit native grid geometry. Source CSS row extents are used;
font/value-dependent flex placement and Windows shadow/pixel equivalence remain
separate qualification.

To observe the original frozen Windows Vue components/CSS with owned sensor data
in isolated Chromium, without copying proprietary assets into the project:

```sh
node tools/inspect-windows-grid.mjs /path/to/2.1.28/Assets/ga2v /tmp/lconnect-grid-observation
```

The helper checks the frozen bundle/stylesheet hashes and mounts the original
grid components in their actual Image/Color modes. It records loaded fonts,
computed line bounds and screenshots; it does not establish Windows/WPF or
hardware equivalence.

### Native canvas editing

Build the frontend, `lconnect-gui` with `native-smoke,custom-protocol`, and the
`lconnect-daemon` `desktop-mock` example. Then run:

```sh
/usr/bin/python3 desktop/scripts/native-canvas.py
```

This uses private Xvfb/D-Bus/MockBackend instances for EN/RU pointer and keyboard
editing, profile Save/load, explicit Apply, decoded output comparison, normal
GUI close and daemon/GUI restart. It does not access physical HID. See the
[R1 contract](../docs/development/parity/r1-canvas-editor.md) for controls and
acceptance boundaries; generated evidence stays in the printed temporary path.
