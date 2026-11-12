# LL-Control comprehensive application review

**Review date:** 2026-11-12  
**Source:** local `master` at `5749fc6` (`feat(branding): adopt LL-Control name and dial icons`)  
**Review type:** read-only source audit plus isolated software tests and non-hardware runtime probes  
**Hardware changes:** none. No physical HID command, firmware operation, host package installation, service mutation, or active-desktop modification was performed.

## 1. Executive assessment

LL-Control is a substantial, thoughtfully engineered software preview, not an empty shell or a collection of stubs. The strongest parts are its typed protocol boundaries, daemon-owned hardware actors, explicit draft/Save/Apply separation, bounded media pipelines, durable revision-checked persistence, and unusually broad fault-injection tests. The desktop bridge has a narrow command allowlist, managed-media HTTP uses opaque IDs and a per-process token, and production dependencies have no known frontend advisory at review time.

It is **not ready to be promoted to a hardware-supported stable release**. Two issues require immediate attention:

1. A second daemon can take over the IPC socket and start a second set of hardware actors.
2. The project's own physical acceptance records still show a failed static-JPEG rotation test and incomplete cooling/RGB/lifecycle qualification.

The next release-blocking fixes are the Arch udev rule, which is skipped on systems without `plugdev`, and an exact-source green CI/package qualification. Experimental features also need visible in-app labeling. The later Windows, capture, firmware, and effects milestones are real feature work, but they should not distract from those core issues.

### Priority definitions used here

- **P0 — Critical:** potential unsafe/competing hardware control or a critical stable-release acceptance gate.
- **P1 — Important:** a confirmed core defect, install blocker, misleading support state, or required production qualification.
- **P2 — Medium:** meaningful robustness, localization, interoperability, or expected post-preview functionality.
- **P3 — Low:** maintainability, documentation, research, and lower-impact robustness work that can follow core completion.
- **P4 — Nice to have:** optional expansion only after the supported core and required parity work are complete.

### Findings count

| Priority | Count | Main themes |
|---|---:|---|
| P0 | 2 | Competing daemon owners; incomplete critical hardware/lifecycle acceptance |
| P1 | 5 | Physical static-JPEG rotation, udev, current CI/package, experimental labeling, editor performance acceptance |
| P2 | 11 | CLI/IPC robustness, localization, font process bounds, Windows interoperability, integrations, sensors, firmware, RPC maintainability |
| P3 | 6 | Frontend maintainability/bundle, documentation drift, branding, config failure policy, Rust advisory automation, unresolved research |
| P4 | 4 | Remaining effects, editor/locales polish, broader platforms, authoring conveniences |

## 2. Confirmed P0 findings

### P0-01 — A second daemon can replace the socket and become a competing hardware owner

**Status:** confirmed from source and by an isolated Unix-socket semantic probe.

`crates/lconnect-ipc/src/server.rs:87-93` unconditionally removes the socket path before binding. `crates/lconnect-daemon/src/main.rs:65-83` starts the device manager, sensor collector, and power monitor **before** `server.serve()` is entered at `main.rs:97-107`. The manager opens devices and replays persisted settings during attachment (`crates/lconnect-daemon/src/manager.rs:593-637`).

Consequences when a manual daemon, duplicate service, or restart race occurs:

- the second process unlinks the live daemon's pathname;
- existing clients remain connected to the first daemon, while new clients connect to the second;
- both managers can open the same HID device and replay or apply competing cooling/RGB/LCD policies;
- the first process can later remove the second process's socket during shutdown (`main.rs:119-122`).

The isolated probe confirmed the operating-system behavior: after unlinking listener A's path, listener B bound successfully and new clients reached B while A remained open. No application hardware was involved in that probe.

**Required action:** acquire an exclusive daemon lock in the runtime directory before persistence, HID, or manager initialization. Probe the existing socket and fail loudly if a live daemon answers; remove a socket only after proving it stale. Track socket identity before cleanup. Add a two-process regression proving the loser exits before any HID call or replay and that the winner's pathname/clients remain intact.

### P0-02 — The critical R3 hardware and lifecycle gate is still open

**Status:** documented acceptance gap, not a new source inference.

`docs/development/parity/release-milestones.md:148-186` makes R3 the critical first-release gate. Its unchecked requirements include exact release qualification, real Plasma lifecycle, physical display/cooling/RGB, and a 30-minute combined workload followed by restart, cold boot, sleep/wake, and USB reconnect.

The more recent R3 ledger records useful partial observations, but not closure:

- physical static-JPEG rotation failed (`r3-release-qualification.md:112-122`);
- intermediate backlight levels remain inconclusive;
- RGB restoration/qualification had not run (`r3-release-qualification.md:71-74`);
- installed-app checkpoints are process/telemetry confirmations, not final physical or user acceptance (`lcd-editor-ux.md:291-299,339-348`).

MockBackend and codec tests cannot establish actual pump safety, LED zones, display visibility, or suspend/reconnect behavior. A stable release should remain labeled **software preview** until the declared R3 matrix is complete on the exact package.

**Required action:** finish the R3 evidence matrix with one HID owner, recorded restore settings and fan wiring, physical pump/RGB checks, all four display orientations, brightness steps, a measured 30-minute combined soak, and lifecycle restoration. Publish failures as failures; do not substitute requested/applied wire state for visible output.

## 3. P1 findings

### P1-01 — Static-JPEG rotation physically fails; intermediate brightness is not accepted

**Status:** confirmed historical physical failure on the current implementation lineage; no later physical pass is recorded.

After the first protocol fix, the rebuilt daemon still left the strawberry upright despite a requested 90° rotation (`r3-release-qualification.md:112-116`). Current code continues to use `LCDSetting` and the firmware rotation field for JPEG sources (`crates/lconnect-daemon/src/actor/lcd_keeper.rs:1084-1122`). Later editor and branding commits do not record a physical retest. The same ledger says 30% could not be distinguished from full brightness, while 0% visibly darkened the panel.

This is not merely missing test coverage: static rotation has a recorded physical failure. The preview and mock-HID tests do not detect it.

**Required action:** trace the Windows mode/order semantics for static JPEG configuration. If firmware rotation remains unreliable, evaluate rotating the final static JPEG in software while preserving exactly-once rotation across theme/media transitions. Separately qualify intermediate backlight levels. Acceptance must cover all four angles on static and animated sources, 0/30/50/100% brightness, restart, replacement, and physical observation.

### P1-02 — The packaged udev rule is skipped when `plugdev` is absent

**Status:** reproduced with `udevadm test` on the review host.

Both the generated rule (`packaging/udev/70-lianli-hid.rules:3-4`) and its generator (`xtask/src/main.rs:38-42`) specify `GROUP="plugdev"`. The Arch package does not create that group. On this host, udev logged:

```text
Failed to resolve group 'plugdev', ignoring: Unknown group
```

A matching synthetic test rule with the same group, a distinctive tag, and mode was skipped entirely: its tag and mode never appeared in the processed event. Thus this is not just cosmetic log noise. The legacy host rule can mask the problem on an already-configured machine.

**Required action:** prefer the self-contained `TAG+="uaccess"` rule without a distribution-specific group, or deliberately provision a documented group through packaging. Regenerate the rule through `xtask`, add a clean-Arch permission test, and qualify a fresh install/reboot rather than relying on legacy rules.

### P1-03 — There is no current green CI result or qualified package for HEAD

**Status:** release-evidence gap.

The current local HEAD `5749fc6` has no remote commit/check run. The latest listed workflow run, [39115166579](https://github.com/InkyQuill/lconnect-linux/actions/runs/39115166579), had a successful browser job but a failed Arch job; its Arch log was unavailable through `gh` during review. The local all-feature Rust and frontend checks are green, but they are not a remote CI/package result.

The exact current package was not built during this review because CMake is unavailable on the host. GJS is also unavailable, so full GNOME and Plasma integration suites could not be run. `docs/development/parity/package-delivery.md:61-76,112-116` still requires clean-host/reproducibility and real delivery qualification.

**Required action:** publish the intended source through the project's cleaned-history process, make both CI jobs green, inspect and retain the actual Arch failure cause if it recurs, build the exact package, run package/consumer checks, and perform a clean-host comparison. Do not release from historical candidate hashes.

### P1-04 — Experimental features are not identified as experimental in the application UI

**Status:** confirmed release-contract/UI mismatch.

`docs/development/parity/release-contract.md:39-42` says R3 must make per-feature disposition visible in release UI/help. The README labels capture/recording, Windows import, procedural effects, and Mini/FPS experimental, but the corresponding UI presents them as ordinary workflows:

- recording: `desktop/src/lib/MediaLibrary.svelte:148-160`;
- Windows import: `desktop/src/lib/FullGroupArchive.svelte:106-121`;
- FPS: `desktop/src/lib/FpsSettings.svelte:41-55`;
- procedural effect editors under `desktop/src/lib/WeatherEditor.svelte` and `PaletteEditor.svelte`.

A search of production frontend sources found no experimental-status copy. Installed users should not need to read repository documentation to discover support boundaries.

**Required action:** add concise EN/RU badges and just-in-time explanations to every experimental entry point and the Mini install/help surfaces. State what is unverified, not only what is implemented. Add browser assertions for the release disposition table.

### P1-05 — Production editor latency, 32-layer behavior, and prolonged resource use remain unqualified

**Status:** missing required UX2 acceptance, not a measured performance failure.

`docs/development/parity/lcd-editor-ux.md:165-176` leaves rendering-session proof and production performance measurement open. Later checkpoints report only a 10-second 20.79 fps playback measurement and explicitly defer exact-package latency and prolonged resource checks (`lcd-editor-ux.md:255-274`).

The current preview path can issue a full editor preview followed by per-layer preview RPCs (`desktop/src/routes/+page.svelte:279-331`) and rebuilds dynamic-layer cache signatures from the complete sensor list. That design deserves measurement at the declared limits; source inspection alone does not prove a defect.

**Required action:** measure the production app on the target session with seven and 32 layers, imported fonts, transparent images, independent sensor parts, video, and a procedural background. Record gesture/property p95, frame rate, CPU, RSS, queues, and sensor age during a prolonged edit. Optimize only if measured budgets fail.

## 4. P2 findings

### P2-01 — `lconnectctl logs` reports success when `journalctl` fails

**Status:** reproduced.

`crates/lconnect-cli/src/commands/maintenance.rs:56-71` discards both spawn errors and the child exit status. Running with an invalid priority produced `journalctl: Unknown log level not-a-real-priority` while `lconnectctl` exited **0**.

**Impact:** scripts and users can treat missing logs or invalid filters as successful diagnostics.

**Action:** propagate spawn errors and require `status.success()`. Add CLI tests for invalid priority, missing executable, and journalctl failure.

### P2-02 — Ordinary CLI connections and RPC calls have no deadline

**Status:** confirmed from source.

`crates/lconnect-cli/src/connection.rs:17-24` awaits connection/hello without a timeout, and ordinary command calls use the unbounded `IpcClient::call`. `crates/lconnect-ipc/src/client.rs:75-113,252-282` has no internal timeout. The GUI adds its own deadlines and Mini adds a three-second read deadline, but most CLI commands do not.

**Impact:** a stalled daemon or peer can hang hardware-control commands and scripts indefinitely.

**Action:** add configurable connect and per-call deadlines with explicit ambiguous-mutation reporting. Never automatically replay a write after timeout. Test stalled hello, stalled response, and committed-but-lost response behavior.

### P2-03 — One slow subscription can block every response on an IPC client

**Status:** confirmed from source.

The single reader task in `crates/lconnect-ipc/src/client.rs:165-177` awaits `s.send(...)` for each subscriber. A full 32-item subscriber queue stops the reader, so unrelated RPC responses and subscriptions on that connection stop too. There is no slow-consumer regression test.

**Impact:** a slow `watch` or media-export consumer can cause unrelated calls on the same client to time out or hang.

**Action:** use bounded nonblocking fan-out with an explicit drop/close policy, or isolate subscriptions from the response reader. Test a full subscriber queue while an unrelated RPC completes.

### P2-04 — English UI still contains Russian-owned validation errors, and the locale checker misses them

**Status:** confirmed from source; missing test coverage.

Examples include:

- `desktop/src/lib/api.ts:68-70`: `Изображение превышает 16 MiB`;
- `desktop/src/lib/weather-settings.ts:13-23`: several effect validation errors;
- `desktop/src/lib/lcd-pages.ts:36`: layer-limit error.

These strings are absent from `en-US.json`. The image error is wrapped as raw diagnostic data, so it remains Russian in an English session. `desktop/scripts/check-locales.mjs:11-20` inventories `t(...)` and `ValidationError(...)`, but not `new Error(...)`, which creates a false sense of complete coverage. The current static result is therefore 1179/1179 inventoried resources, not proof that all user-facing messages are localized.

**Action:** use `ValidationError` for all app-owned user-facing failures, add the missing resources, extend the AST audit to `Error`, and add EN/RU runtime tests for oversized images and every effect-validation path.

### P2-05 — Fontconfig enumeration has no timeout or output bound

**Status:** confirmed from source.

`desktop/src-tauri/src/system_fonts.rs:46-54` uses synchronous `Command::output()` for `fc-list`. It has no deadline and accumulates all stdout before parsing. Both listing and reading a selected font rerun the catalog (`system_fonts.rs:56-76`). The Tauri command waits on that blocking worker without its own deadline.

**Impact:** a broken fontconfig setup or hung subprocess can stall font selection indefinitely; excessive output is not bounded like other desktop inputs.

**Action:** reuse the bounded/killable subprocess pattern used by sensor/media providers, cap output, and add stalled/large-output tests. Cache a validated catalog briefly if profiling shows repeated enumeration is expensive.

### P2-06 — Faithful Windows import is only a supported subset, not complete R4 behavior

**Status:** documented feature gap.

Owned fixtures and private previews are implemented, but genuine Windows exports and reference renders have not closed the compatibility contract. Remaining work includes intrinsic Three/Four-grid reflow, font/fallback/raster behavior, image/video timing/VFR/colorspace, unsupported combinations, inactive-bank fidelity, and expanded resource/fault cases (`release-milestones.md:188-218`).

**Action:** finish R4 using real Windows 2.1.28 exports with versioned provenance. Record explicit rejection for unsupported cases before mutation; never silently flatten or approximate an imported scene while calling it faithful.

### P2-07 — Windows-compatible export is not implemented

**Status:** confirmed absent feature (R5).

Native `.lcprof`/`.lcg` backup exists, but it is not a Windows export. There is no typed Windows writer, GUI/CLI compatibility report, or Linux→Windows→Linux semantic round-trip (`release-milestones.md:220-244`).

**Action:** implement versioned Screen and FanAndLighting serializers only after R4 identities are stable. Linux-only features need explicit rejection or informed conversion. Validate actual Windows import and both semantic round-trips.

### P2-08 — Capture, shell integrations, and FPS lack real-environment completion

**Status:** implementation exists; platform qualification and selected behavior are incomplete (R6).

The private PipeWire test passes, but it does not exercise portal consent/routing, real monitor/window/region selection, scaling, source loss, or long capture resource behavior. GNOME Shell panel enable/disable and real Plasma panel/login behavior remain open. FPS is a logged-frame mean, not yet qualified app activation/identity or Windows PresentMon-equivalent measurement (`release-milestones.md:246-276`).

**Action:** qualify the exact Plasma Wayland portal/PipeWire workflow, then separately named GNOME/X11 versions. Complete shell install/restart/accessibility tests and real-game FPS identity/metric checks. Keep the current honest “logged frame mean” label until semantics are proven.

### P2-09 — Sensor/provider breadth and physical accuracy are incomplete

**Status:** documented platform gap.

The code deliberately returns unavailable rather than inventing values, which is good. Remaining items include selected CPU voltage rails, proprietary NVIDIA outside X, trustworthy Intel aggregate semantics, GPU multiplicity, permissions, Spec mappings, and physical accuracy/recovery (`release-milestones.md:262-270`; `docs/development/parity/lcdv2-baseline-gaps.md:58`).

**Action:** add providers only with confirmed rail identity and units. Build a real-driver matrix for supported CPU/GPU combinations, stale/recovery, multiple adapters, and source rebinding. Preserve unavailable behavior where the OS cannot expose the Windows measurement.

### P2-10 — A safe product firmware updater is not available

**Status:** internal scaffolding exists; user-facing R7 feature is intentionally unavailable.

The public GUI exposes identity and read-only recovery inspection, not Inspect→Apply→progress→verify. Durable restart recovery, serial-less/missed-reset policy, qualified cooling hold, activation, and physical recovery remain open (`release-milestones.md:278-300`).

**Action:** keep flashing and quarantine clearing denied until R7 is complete. Connect the existing owned engine only after durable recovery and failure semantics are proven; qualify with separately authorized physical equipment and a recovery path.

### P2-11 — Manual RPC duplication makes a large change surface increasingly risky

**Status:** code-quality risk, not a demonstrated runtime bug.

Examples are `crates/lconnect-ipc/src/server.rs` (1,318 lines), `crates/lconnect-daemon/src/ipc_server.rs` (2,646), and `desktop/src-tauri/src/main.rs` (1,433). Method names, parameter structs, trait methods, desktop allowlists, and hand-written TypeScript interfaces are maintained separately. The 39 theme contracts are valuable, but they do not generate the full API.

**Action:** incrementally introduce a versioned method registry/schema shared by server dispatch, desktop allowlists, CLI, and TypeScript types. Split command groups rather than rewriting working actors. Add unknown-field and request/response contract tests per method.

## 5. P3 findings

### P3-01 — Frontend readability and bundle structure need maintenance work

The production frontend emits a 661.41 kB page chunk (197.68 kB gzip), triggering Vite's chunk-size warning. `desktop/src/routes/+page.svelte` eagerly imports all major workspaces. Several components and `studio.css` use dense one-line markup/style that is difficult to review and merge. There is no formatter or lint rule for accessibility, stale async cleanup, or localization-safe error construction.

**Action:** format existing code mechanically before deeper refactoring, add Svelte/TypeScript linting and the localization check described in P2-04, and lazy-load heavyweight archive/recording/editor workspaces where measurement supports it. Avoid a cosmetic rewrite of working behavior.

### P3-02 — Comments and historical documents contain stale implementation statements

Examples:

- `crates/lconnect-cli/src/main.rs:8-11,219-222` still describes future stubs/placeholders although commands are implemented;
- `crates/lconnect-cli/src/commands/mod.rs` retains a stub-era comment;
- `docs/development/protocol/galahad2-lcdv2.md:83-86` says C framing is not implemented, while C packet/H.264 code and tests exist;
- `crates/lconnect-hid/src/hidapi_backend.rs:52-53` links a retired udev filename.

The milestone documents generally mark historical context carefully, but the number of dated ledgers makes current truth harder to discover than necessary.

**Action:** sweep comments after behavior changes, maintain one short current support/status entry point, and preserve history in clearly archived documents rather than repeating stale absence claims in active source comments.

### P3-03 — Mini visible branding was not migrated to LL-Control

`integrations/gnome/metadata.in.json:3-4`, `integrations/gnome/src/extension.js:16-17,51-52`, `integrations/plasma/package/metadata.json:4-5`, and `integrations/plasma/package/contents/ui/main.qml:20-21` still show “L-Connect” or “L-Connect Mini,” while the product decision requires visible LL-Control branding.

**Action:** update user-visible names/descriptions and screenshots/accessibility expectations. Keep package, service, schema, and configuration identifiers stable for compatibility.

### P3-04 — Invalid daemon configuration falls back to defaults with only a log warning

`crates/lconnect-daemon/src/main.rs:43-46` logs a warning and continues with `Config::default()` for every load error, including malformed existing configuration. Defaults currently disable raw send, which is safe, but polling/replay timing can still differ from user intent and the GUI has no indication of the fallback.

**Action:** fail startup for an existing invalid file, or expose a persistent explicit warning requiring acknowledgement. Preserve a missing-file default. Test malformed, unreadable, and unsupported configuration separately.

### P3-05 — Rust advisory auditing is not part of the available verification path

`pnpm audit --prod` reported no known frontend vulnerabilities. `cargo-audit` is not installed locally and the GitHub workflow has no Rust advisory-audit step. This is a visibility gap, not a claim that a Rust vulnerability exists.

**Action:** add a locked `cargo audit` or `cargo deny check advisories` CI job and a reviewed update process. Keep native-library/sysroot qualification separate from Rust advisory results.

### P3-06 — Several applicability questions remain research items, not completed exclusions

R10 still owes startup/offline-content (Cmd15) applicability, global pump motherboard-sync caller resolution, and a legacy firmware <1.5 support declaration (`release-milestones.md:350-371`). These should not be guessed from codec presence or excluded from a “full parity” claim by omission.

**Action:** resolve each with versioned Windows caller evidence or physical proof, then implement, explicitly adapt, or document an evidence-based exclusion. Keep current firmware support scoped to what has actually been observed.

## 6. P4 findings — defer until the above is complete

### P4-01 — Remaining 16 LCD pages/effects

TwistHole, Kaleidoscope, Mandala, Blossom, Transform, TimeTunnel, Whirlpool, RadiantRainbow, FlyInWording, SnowWording, Robot, PlanetCity, Time, Steampunk, Atmosphere, and Radio are not implemented (`release-milestones.md:302-328`). Existing Rain/Snow/MeteorShower/Snow3D/Fireworks also need final visual/timing/fault acceptance.

Implement page-by-page only after core and other functional expansion, with original assets, complete controls, drafts, runtime, reset, import/export, and physical evidence—not static screenshots.

### P4-02 — Editor conveniences and additional languages

Snapping/guides, align/distribute, multi-select/grouping, richer discovery, refined motion, and de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW are optional R9 work. EN/RU correctness and accessibility are not optional and should be closed first.

### P4-03 — Additional hardware, desktop environments, and package formats

Other Lian Li families, Sway/other desktops, non-Arch packages, and broader legacy-device support require independent capability registries and hardware/session acceptance. Metadata for another display shape is not device support.

### P4-04 — Optional native authoring conveniences

Fresh independent Windows-style background/overlay bank creation and richer preset/thumbnail discovery are useful, but existing native presets already support authoring. Keep them behind core fidelity, import/export, and usability completion.

## 7. Verification performed for this review

### Passed locally

| Check | Result |
|---|---|
| `cargo fmt --all --check` | Pass |
| Strict Clippy, workspace/all-targets, locked | Pass |
| Strict Clippy, workspace/all-targets/all-features, locked | Pass |
| Workspace tests, default and all-features | **1,388 passed; 0 failed; 11 routine ignores** each run |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --locked --no-deps` | Pass |
| One-hour media boundary (`hour_boundary --ignored`) | Pass; all 86,400 output frames decoded |
| Real 60-second group recovery deadline | Pass; no second Apply |
| Full 180-second recording limit | Pass |
| Optimized 1080p and 4K recording resource profiles | Both passed with cleanup |
| Optimized 30-second LCD pacing test | 720 frames; 0 frames over one period late; pass |
| `svelte-check` | 0 errors, 0 warnings |
| Full Playwright browser suite | **778 passed in 17.7 minutes; 0 failed** |
| Frontend production build | Pass; one large-chunk warning |
| Static locales | 1179/1179 inventoried resources; P2-04 explains the blind spot |
| Theme contracts | 39 frontend themes validated against Rust schema |
| Localization unit tests | 3 passed |
| Distribution/tool Python tests | 19 passed |
| Shared Mini model tests | 4 passed |
| Native canvas EN/RU, isolated MockBackend | Pointer/keyboard, profiles, decoded output, and restart passed |
| Private synthetic PipeWire transport | FD inheritance, target serial, crop, pause/resume, decode, remote loss, cleanup passed |
| `pnpm audit --prod` | No known vulnerabilities |

### Not run or environment-limited

- **Physical HID, cooling, RGB, LCD, firmware, USB, sleep/cold boot:** not run; this review did not authorize hardware mutation.
- **Current Arch package build:** not run because CMake is unavailable.
- **Full Plasma/Qt and GJS/GNOME suites:** not run because CMake and GJS are unavailable. The shared Mini model and private PipeWire checks did run.
- **Current remote CI:** no run exists for local HEAD; the latest listed Arch job was red.
- **Rust advisory audit:** `cargo-audit` is not installed.
- **Real Windows exports/round-trips, real capture portals, real games, real shell sessions:** remain open as described above.
- **Four vendor-image cases:** not run because they require separately acquired pinned firmware images. One ignored subprocess entry point is a helper, not a missing standalone scenario. All other routine-ignore scenarios were either exercised directly or through their maintained private harness.

The routine 11 ignores should not be read as 11 failed tests. Long boundary tests executed separately in this review passed; other ignored cases have explicit external-fixture or resource-qualification reasons.

## 8. Test and acceptance gaps by subsystem

| Subsystem | Strong existing evidence | Missing acceptance that still matters |
|---|---|---|
| Protocol/HID | Typed packet tests, MockBackend, frame validation, historical physical observations | Duplicate-process ownership, current static output, all supported physical controls |
| Persistence/recovery | Atomic writes, CAS, crash/fsync tests, lost-reply/readback matrices | Exact current package lifecycle and real power/session events |
| LCD editor | Browser and native pointer/keyboard, profiles, output/restart | Production p95, 32-layer/prolonged workloads, real scaling/speech, physical rotation/brightness |
| Media/capture | Real codecs, one-hour/180-second boundaries, cleanup, private PipeWire | Real portal routing/consent/DPI/source loss, worst-case real capture resources |
| Cooling/RGB | Policies, fallback, thermal arbitration, packet assertions | Actual pump duty/RPM/source, LED zones/modes, combined physical soak and restore |
| Sensors/Mini/FPS | Typed freshness, shared model, helper ownership, read-only feeds | Real drivers, shell panels/login, real games and metric identity |
| Windows | Owned import fixtures, private preview, native Linux archives | Genuine Windows imports, reference fidelity, outbound Windows export |
| Delivery | Local strict checks, private historical package tests | Current remote CI, exact current package, clean-host and installed-session qualification |

## 9. Recommended execution order

1. **Stop competing ownership:** implement and test the daemon singleton lock before any more hardware qualification (P0-01).
2. **Fix clean-install access:** remove or provision `plugdev`, then prove the packaged udev rule on clean Arch (P1-02).
3. **Close physical correctness:** resolve static-JPEG rotation, qualify backlight, cooling/RGB, and complete the measured R3 lifecycle matrix (P1-01, P0-02).
4. **Make the release artifact real:** current-source CI, exact package, clean-host comparison, installed Plasma lifecycle (P1-03).
5. **Tell users the truth in-product:** experimental labels and remaining EN/RU validation errors (P1-04, P2-04).
6. **Measure before optimizing:** production editor/resource budgets and IPC/CLI hangs (P1-05, P2-02, P2-03, P2-05).
7. **Then expand interoperability:** faithful Windows import/export, real capture/sensors/shells, and only afterward the firmware product workflow (P2-06 through P2-10).
8. **Last:** effects, extra languages, new hardware/platforms, and convenience polish (P4).

## 10. What should be preserved

- Explicit local draft versus Save versus hardware Apply.
- No automatic mutation replay after ambiguous replies.
- Revision-checked durable state and conservative resource retention.
- Bounded media input/output, child ownership, cancellation, and cleanup fences.
- Typed unavailable/stale sensors rather than fabricated zeroes.
- Narrow Tauri command permissions and scoped media serving.
- The project's candid distinction between source-derived, mock/native, historical physical, and current physical evidence.

These are strong foundations. The correct next step is not a broad rewrite; it is to close ownership, install permissions, physical output, and exact-release acceptance before expanding the feature catalog.
