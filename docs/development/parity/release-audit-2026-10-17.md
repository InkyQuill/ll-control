# LCDV2 implementation and release audit — 2026-10-17

> Historical R0 audit. R1 base canvas gaps described here were subsequently closed
> at `f004cf2`; see the [canvas contract](r1-canvas-editor.md) and
> [current milestones](release-milestones.md). Other gaps retain their stated gates.

This audit separates **implemented capability**, **remaining implementation**, and
**qualification**. It is a source/evidence audit, not a new runtime, Windows, or
hardware test. The release plan is [release milestones](release-milestones.md);
[root roadmap](../../../roadmap.md) is the scheduling authority.

## Decisions confirmed with Pavel during this audit

1. Ship a usable LCDV2 release before attempting full parity.
2. First release requires basic direct canvas editing: selection, dragging,
   resizing and layer ordering. Forms plus a preview are insufficient.
3. First real-session acceptance target is **KDE Plasma on Wayland**.
4. Windows imports must be faithful; Linux-authored themes need equivalent
   capabilities, not identical Windows typography or screen organization.
5. Firmware flashing, extra effects and optional integrations follow the first release.
6. **Windows-compatible export is also required**, as a later milestone.

The baseline remains Windows L-Connect 3 **2.1.28.0**, Galahad II LCDV2
**0416:7395**, square **480×480**. Full parity is scoped to applicable capabilities
of this SKU, with documented Linux adaptations. It does not mean all Lian Li
hardware, all Windows versions, or identical proprietary artwork. First release
and full parity are two distinct completion gates.

## What was checked

- Current source entry points, component wiring, profile conversion/storage,
  renderer/editor contracts, versioned Windows page selection and acceptance documents.
- All **845 recorded build inputs** match the Realtime candidate snapshot and
  its manifest. This includes the fan-preset endpoint fix; the old roadmap's
  claim that it was outside the package was stale.
- Archive and manifest SHA-256 match their saved evidence. All nine referenced
  verification logs still exist and match their recorded hashes.
- All ten original Windows assemblies match the versioned reference manifest;
  it records GUI assembly version 2.1.28.0. The Windows page switch still
  enumerates 23 applicable pages. No new Windows execution was performed.
- The working tree is dirty. HEAD alone does not identify the tested source;
  the source-input manifest does. Untracked files outside that manifest are
  not implicitly qualified.

[Machine-readable audit](evidence/2026-10-17-release-audit.json).
[Candidate evidence](evidence/2026-10-17-realtime-shadows.json).

The saved candidate reports **1,373 Rust passes / 9 ignored**, four example
checks, 758 browser passes, Svelte 0/0, 1,144 static translations, 39 theme
contracts and three localization contracts. Frontend/browser/native divider
results were carried from the unchanged preceding frontend; the renderer change
had its own Rust, production package and GUI checks. These are retained results,
not tests rerun by this audit. A short MockBackend video + four sensors + dividers
run produced 23.40 fps against a nominal 24 fps and a minimum 20 fps gate;
723 frames decoded. That is neither a long-run nor a physical LCD result.

## Assessment

The project already has most core software workflows. It is not yet release
qualified, and the user's required direct canvas workflow is genuinely missing.
The immediate path is to finish that editor interaction, stabilize the existing
core, and qualify an exact package on Plasma Wayland and LCDV2. It is not to
finish every shader, font raster difference, peripheral provider or firmware path.

There is no defensible single readiness percentage. A feature may have a complete
GUI and persistence path while its hardware, Windows-equivalence or failure-path
acceptance remains open. Seven of 23 Windows pages have implementation counterparts
(two basic backgrounds and five effects); that ratio is **not** application parity.

## Implemented inventory and remaining work

“Implemented” below means source and recorded software evidence exist. Physical
acceptance is open unless specifically described as historical.

| Area | Implemented now | Remaining implementation / research | Remaining qualification | Milestone |
|---|---|---|---|---|
| Device shell | Device-first Svelte/Tauri studio; capability-aware sections; distinct disconnected/unsupported states; LCDV2 geometry | Canvas interaction below; other devices are outside this release | Real Plasma Wayland, navigation/focus/scaling and disconnected recovery | R1–R3 |
| LCD transport | JPEG and firmware-gated H.264; brightness, four rotations, stop/replace/LocalUI; daemon-owned replay | Legacy FW<1.5 support remains unqualified; offline content applicability unresolved | Current production pipeline on LCD, transitions, cold boot, USB and sleep/wake | R2–R3; R10 research |
| LCD composition | Image, text, sensor, cycle, clock, watermark, gradients, layered styles, fonts, dividers; live/one-shot preview and explicit Apply | Direct canvas manipulation; single-sensor rebinding; authoring coverage described below | Preview/output consistency, persistence, recovery and accessible operation | R1–R3 |
| Ready-made basic layouts | Focus, Duo, Triple, Quad, Cycle and Clock produce editable drafts | Linux layouts need capability equivalence; no need to clone Windows coordinates | Real values, missing/stale sources, units, saved/reopened layouts | R1–R3 |
| Basic Windows editor state | Independent current runtime and editor; two background banks × seven overlays; inactive state/resource history retained; existing-bank selector | Fresh native bank creation is not provided by the existing selector; exact imported intrinsic grid reflow remains missing | Genuine Windows exports; all background/overlay permutations and restart | R4; fresh native bank convenience R9 |
| Fonts/text | Bundled licensed fonts; managed TTF/OTF import; shaping/bidi/wrapping, line boxes, glyph fallback, separate sensor parts | Imported Windows layout/fallback/raster discrepancies; six proprietary families require user-supplied licensed files | Same font bytes, locale and content against reference; native missing-font recovery | R2/R4 |
| Image editing | EXIF, square crop, ordered rotate/flip, all six filters, preview, gallery re-edit and contextual replacement | Exact Windows WPF/Telerik sampling/DPI behavior is not qualified | Remaining native crop/focus/errors, original retention, decoded output and LCD | R2–R4 |
| Video editing | GIF/MP4 preparation, H.264, cut intervals, crop, source scrubbing, prepared playback, brightness/contrast, AutoFill, gallery re-edit | Windows FPS/end-frame/VFR/colorspace semantics still require reference comparison | Long/large inputs, short/repeated intervals, corrupted source/playback, cancellation and uncertain Save | R2/R4/R6 |
| Galleries/assets | Image/video collections, thumbnails, rename/delete, protected defaults, imported fonts, reference protection and cleanup | No blanket “gallery missing” implementation task | Lost reply/disconnect during publication/delete; plain/video/replacement membership variants; quota/resources and reset | R2 |
| Realtime LCD | Sensors/clock update without GUI; video overlays, rotation, replay | Optimize only measured failures against declared workloads | Thirty-minute combined LCD/cooling/RGB workload, sensor age, queues/CPU/RSS, physical timing | R2–R3 |
| Cooling | Fan presets/custom curves, pump duty/RPM/source, fallback/hysteresis, manual override, persisted modes; eight fan/three pump device banks; save/default/reset | Global Windows motherboard-sync caller unresolved; no fan MB-sync/start-stop requirement | Real pump response, source/fallback/manual/restart; fan only if wired to LCDV2 | R2–R3; R10 research |
| RGB | Six fan/16 pump mode catalogs, palettes/parameters, Static/Dynamic, source/unit profiles, sync, topology/LED counts and replay | Do not reimplement completed controls; resolve real coverage defects if found | All supported zones/modes, thresholds, unavailable sources, native reconnect/errors, combined load | R2–R3 |
| Thermal alerts | CPU/GPU priority/grace/override/base restore and persistence | Complete-zone behavior cannot be inferred from known direct-color prefix | Real zone coverage, overlapping alerts, manual change and restore while LCD runs | R2–R3 |
| Sensors/Spec | Typed identity/freshness cache, CPU/GPU/RAM/device readings; supported clocks/power/VRAM/voltage, host fan mapping, Spec | Additional CPU voltage rails, proprietary NVIDIA outside X, trustworthy Intel aggregate; field mapping gaps | Actual drivers/permissions/accuracy, multiple adapters, missing/stale/recovery | Core R3; broader R6 |
| Profiles/reset | Eight Linux screen slots, four fan slots, four extra pump slots; CRUD/load/save; independent Screen/FanAndLighting reset and native archive round-trip | Preserve distinction between Linux slots and Windows array positions | Crash/fsync/partial install/readback/lifecycle matrix, no repeated mutation on unknown result | R2; Windows R4–R5 |
| Windows import | ZIP inspect; explicit source/font/resource bindings; owned PNG/GIF/MP4 preparation; private preview; Control/Screen commit and recovery; current/editor separation | Intrinsic layout, unsupported combinations/defaults/effects; genuine-source compatibility | Actual Windows exports, managed resources/expiry/faults, all inactive-bank parameters and no silent conversion loss | R4, effects extension R8 |
| Windows export | Native Linux archives exist | **Windows-compatible export is not implemented; now explicitly required** | Windows must import output and preserve supported semantics/assets through round-trip | R5, effects extension R8 |
| Screenshot/record | Portal ownership, screenshot→editor→Save; record start/pause/resume/clear/finish, region, trim/review/re-edit, 180s limit; killable worker and cleanup fences | Fix concrete platform failures found in real sessions | Real portal consent/routing/DPI/parent/resize, capture resource limits and remaining native fault matrix | R6; release exposure decision R0/R3 |
| Desktop/settings | EN default, RU retained/required, global units, tray, autostart, single instance, joined exit, device settings | Release-quality core UX; no new standalone Mini window | Plasma Wayland login/tray/close/exit/restart, EN/RU errors, keyboard and speech | R2–R3 |
| Mini/FPS | Packaged Plasma widget/GNOME extension and cache-only CLI; opt-in MangoHud logged-frame mean and settings | FPS capture activation/app selection, Windows metric identity; other DEs deferred | Real shell/version/session, actual games, config/permissions/scaling | R6 |
| Firmware | Identity readback, preflight, pinned image inspector, internal transfer/quiescence/journal/guard, supervised jobs/reconnect, read-only recovery inspection | Product Inspect/Apply/progress, durable restart recovery, qualified resolution/restoration, serial-less/missed reset | Image/revision/ACK/cooling hold and recovery qualification; expanded fault/power/shutdown and eventual hardware | R7; ordinary identity R3 |
| Delivery | Arch x86_64 package/tar, GUI/CLI/daemon, systemd/udev/notices, both Mini stores, private install/upgrade/remove and production mock GUI | Release exposure/support declarations; CI execution and clean-host reproducibility | Actual Plasma Wayland install/login/upgrade/remove, retained data, exact-source release run | R0/R3 |

Source anchors: [main editor](../../../desktop/src/routes/+page.svelte),
[preview wrapper](../../../desktop/src/lib/LcdPreview.svelte),
[basic selector](../../../desktop/src/lib/BasicEditor.svelte),
[preset authoring](../../../desktop/src/lib/theme-presets.ts),
[page switching](../../../desktop/src/lib/lcd-pages.ts),
[Windows conversion](../../../crates/llcontrol-profile/src),
[stored editor](../../../crates/llcontrol-daemon/src/screen_editor.rs),
[Tauri command boundary](../../../desktop/src-tauri/src/main.rs).
Detailed behavior/evidence remains in the linked contracts in
[implementation status](implementation-status.md) and [acceptance checklist](basic-parity-checklist.md).

## LCD editor: three separate workstreams

### A. Base functionality and release usability — R1/R2

The preview is an image inside `LcdPreview`; it has no layer hit testing, selection
handles, drag or resize implementation. The form editor already adds/removes
layers and reorders them with buttons. Image/video crop pointer support does not
satisfy composition-canvas editing. This is a confirmed new implementation gap.

Single sensor rows expose a label, source ID, precision and temperature unit;
they do not expose source replacement, unlike sensor-cycle rows. R1 adds
rebinding without destroying the layer's style/position. Imported label/unit
parts also have a narrower form control set than the underlying TextStyle;
R1/R4 must enumerate editable properties and preserve untouched imported values.

Release scope: select/move/resize supported basic layers, order layers, inspect
and adjust properties, rebind sensors, preserve draft vs Save vs Apply, cancel
an unfinished gesture, and provide keyboard equivalents. Layout adaptation must
work at 480×480 and scaled preview sizes. Basic undo/redo is a proposed bounded
implementation aid in R1, not an existing feature or a Windows requirement.

### B. Additional theme transfer and authoring — R4/R8

The existing `BasicEditor` selects saved banks; it does not create Windows banks
for a fresh Linux theme. Linux presets are separate native templates. Neither
fact justifies calling all Windows theme authoring complete.

Two basic backgrounds and Rain, Snow, MeteorShower, Snow3D, Fireworks have
counterparts. Missing: **TwistHole, Kaleidoscope, Mandala, Blossom, Transform,
TimeTunnel, Whirlpool, RadiantRainbow, FlyInWording, SnowWording, Robot,
PlanetCity, Time, Steampunk, Atmosphere, Radio**. For each, transfer means its
controls, bindings, independent drafts/defaults, renderer, save/reload/reset,
preview/runtime, import/export mapping and acceptance—not just a sample animation.

Existing five effects still need visual/timing/background-video/fault acceptance.
The six Style Express pages require original replacement assets and their
sensor/clock/text bindings. [Effect catalog](lcd-effects.md).

### C. Polish — R9

Convenience snapping/guides, alignment/distribution, multi-select/grouping,
more presets, refined thumbnails, richer discovery, animations and extra locales
can follow release. They are proposals, not inferred missing Windows features.
Basic keyboard access, readable EN/RU, reliable errors, scaling needed for the
supported session and draft safety are release requirements, not cosmetic polish.

## Divergences and scope boundaries

| Classification | Decision / behavior | Consequence |
|---|---|---|
| Approved product direction | Device-first Linux UX; Windows is a capability reference | Do not clone Windows navigation or force non-LCD devices into an LCD shell |
| Confirmed this audit | Capability-equivalent Linux authoring; faithful Windows imports | Imported reflow/font/resource fidelity is R4; native preset coordinates need not match |
| Confirmed this audit | Usable release first; Plasma Wayland first | Additional environments/features do not silently join release blockers |
| Recorded adaptation | Mini uses Plasma/GNOME integration, not a standalone window; Sway deferred | Test actual shell integrations in R6; do not resurrect the removed window |
| Recorded adaptation | English default and required Russian; remaining locales deferred | EN/RU is release scope; de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW remain explicit language scope decisions for R9/R10 |
| Recorded safety/storage choice | Gallery re-edit replaces selected collection membership while retaining old referenced assets; explicit Apply and unknown-result readback | Do not copy Windows destructive asset deletion or replay an uncertain mutation |
| Recorded extensions | Four pump slots, WebP/BMP, 24h/seconds, flexible layers and local scene seeds | Keep compatible; they do not count toward closing Windows requirements |
| Recorded implementation difference | Linux preserves blank lines, uses its own resampling/gradient/shadow raster and bounded workloads | Fine for native authoring; imported-scene differences require R4 comparison/correction, not blanket waiver |
| Asset/font constraint | Own artwork; licensed bundled fonts and user import for proprietary families | Missing exact font must be explicit; no silent “faithful” fallback claim |
| Proven not applicable | HydroShift Screen Animation Editor; LCDV2 fan motherboard sync/start-stop | No implementation milestone unless new baseline evidence changes applicability |
| Unresolved, not waived | Cmd15/offline content; global pump MB-sync caller; legacy FW<1.5 qualification | R10 must resolve applicability/support with evidence; absence of evidence is not completion |
| Newly required | Windows-compatible export | R5 adds real outbound interoperability, not merely a renamed native archive |

Other hardware families remain future product scope. Architecture readiness or
round-screen metadata does not establish device support. Unavailable host sensors
remain unavailable; no fake zero, guessed rail or synthetic Intel aggregate.

## Documentation drift found

1. The previous roadmap says the fan endpoint fix was not packaged; the verified
   845-input candidate includes the current source. The implementation summary's
   package statement is the consistent one.
2. The checklist still calls the older Collections candidate current at its end.
   Realtime is the retained candidate for this audit; neither is the future release.
3. PAR-09 and SYS-001/002 retain “Screen/desktop conversion missing” wording;
   current import code and native evidence implement those paths with remaining
   limits. Treat those claims as superseded, not new implementation work.
4. Early image/font/video sections say native editing, VideoEdit, fonts or fallback
   are absent; later sections and current code implement them. Historical checkpoint
   language must not become the active backlog.
5. The matrix's old eight-Windows-slot interpretation is superseded by the
   current/editor identity investigation: Windows index0 is current Full Screen;
   positions1..7 are retained opaque history, not automatically Linux named slots.
6. Old M1/M2/basic plans overlap. Their unchecked totals cannot measure readiness.

The previous roadmap is preserved in
[the pre-cutover archive](archive/2026-10-17-roadmap-before-release-milestones.md).
Historical evidence is retained. The new milestones assign ownership to outstanding
requirements instead of deleting them or counting an implementation checkpoint as
hardware acceptance.
