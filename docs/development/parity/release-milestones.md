# Release and full-parity milestones

Created **2026-10-17** from the [verified audit](release-audit-2026-10-17.md)
and explicit user decisions. Baseline: Windows **2.1.28.0**, LCDV2 **0416:7395**.
These **R** milestones replace old M1–M6 scheduling; old plans remain acceptance
sources. This document authorizes planning, not new hardware testing or flashing.

The LCDV2-first milestones prioritize the user's physical device; they do not
exclude other applicable parity devices or the requirement to integrate all
functions of the separate third-party [ll-connect3](https://github.com/joeytroy/ll-connect3),
including UNI FAN SL-INF. Plan and track that work separately by function and device,
distinguishing unimplemented from implemented but unverified. Hardware remains
unverified until evidence covers the particular device and operation, and its use
is at the user's own risk. LCDV2 ownership does not establish UNI FAN TL ownership
or validation. See the [reference inventory](../2026-09-08-reference-2.1.28-and-ll-connect3.md).

## Priority and completion rules

| Priority | Meaning | Planned work |
|---|---|---|
| Critical | Necessary for trustworthy control, saved data, lifecycle or an installable release | R0, R2, R3 |
| Important | Required normal user workflow, even if the backend already works | R1 basic direct canvas editing |
| Expected to have | Required for the agreed eventual parity/product, independently deliverable after first release | R4 faithful import, R5 Windows export, R6 capture/platform integrations, R7 updater |
| Nice to have for first release | Explicitly deferred from first release; still mandatory for full applicable catalog parity | R8 additional themes/effects and their authoring |
| Polish | Convenience/visual refinement; accessibility and correctness are not demoted here | R9 |
| Full-parity gate | Reconcile all applicable requirements against a qualified final snapshot | R10 |

A milestone is complete only when its listed exit evidence exists. Track each
work item as **implementation**, **qualification**, **research** or **decision**;
track results as pass/fail/blocked/not-run/not-applicable. Do not use “partial”
alone, test totals, or a successful tool invocation as an acceptance result.

First release path: **R0 → R1 + R2 → R3**. R2 can proceed while R1 is developed;
its final integrated checks depend on R1. No R4–R9 feature blocks that cutover
unless it exposes a defect in a shipped core path. Full parity requires R4–R8,
applicable R9 requirements and R10. R4 precedes basic R5; R8 extends both to effects.
R6 and R7 are independent after the core is stable. An unavailable hardware or
Windows reference gate blocks its acceptance, not independent software work in
other milestones. Additional effects stay last
among functional expansion, honoring the existing decision.

## R0 — Freeze the release contract and evidence baseline

**Critical. Status:** release-development contract and immutable baseline complete; final release identity/qualification remains R3. [R0 contract](release-contract.md).
**Owner areas:** roadmap/product documents, release manifest, CI/package metadata.

- [x] Verify baseline references, candidate hashes and current recorded inputs;
  distinguish implementation gaps from acceptance gaps.
- [x] Record first-release, direct canvas, Plasma Wayland, import fidelity and
  outbound Windows export decisions.
- [x] Record the exact release-development baseline commit plus source manifest; preserve unrelated
  work and historical evidence. Do not treat the current dirty HEAD as that commit.
- [x] Publish a feature/support table: core LCD editing/playback, sensors, cooling,
  RGB, profiles/reset, EN/RU, tray/autostart and installed lifecycle are supported
  only after R3. List precise firmware/hardware/session versions actually tested.
- [x] Assign every existing optional entry point a release disposition: qualified
  and supported, explicitly experimental, or unavailable in the release build.
  Capture, Windows migration, Mini/FPS and procedural effects may remain outside
  supported scope; their mere presence must not imply qualification. Any retained
  path must still pass isolation/data-safety checks in R2.
- [x] Use the existing Arch x86_64 artifact as the first packaging track; this is
  a plan based on current delivery, not a promise of other distribution support.
  Record the actual tested Plasma/Wayland versions in R3, not guessed versions.

**Exit:** immutable baseline commit `299fe6c3c20bf042e48f3bd88049227afd3324d9`, candidate identity, declared support/experimental table and
no contradictory release labels. The baseline audit is already recorded in
[evidence](evidence/2026-10-17-release-audit.json).

## R1 — Finish the base LCD visual editor

**Important; first-release blocker. Status:** complete for the R1 software/native scope at `f004cf2`; physical Plasma Wayland acceptance remains R3.
**Type:** implementation + qualification.
**Depends on:** R0 contracts. **Owner areas:** `desktop/src/routes/+page.svelte`,
`desktop/src/lib/LcdPreview.svelte`, editor state/presets, theme geometry and tests.

- [x] Introduce selectable canvas layers and selection-to-properties synchronization.
  Use the existing theme as the source of truth; do not build a second renderer
  with subtly different layout/fit/rotation semantics.
- [x] Add pointer move/resize for basic image/watermark, text, clock, sensor/cycle
  and divider geometry. Define text resize as font-size/box editing explicitly;
  preserve sensor label/value/unit relationships. Backgrounds/effects need explicit
  fixed/full-canvas behavior rather than misleading drag handles.
- [x] Convert preview coordinates to native 480×480 consistently at varying
  window size, display scale and rotation. Handle overlap/selection order,
  clipped layers, pointer capture/loss and Escape cancellation.
- [x] Keep layer ordering via accessible list/buttons and expose selection on
  canvas; dragging must not accidentally apply or reorder a procedural background.
- [x] Add single-sensor source replacement retaining style/position, with units,
  unavailable sources and device changes handled explicitly.
- [x] Audit base-layer property reachability, including separate sensor parts;
  preserve non-edited imported values. Do not silently reset hidden style fields.
- [x] Retain numeric input and keyboard move/resize/order equivalents, visible
  focus and accessible labels. Add bounded local undo/redo for gestures/property
  changes if used to provide predictable recovery; no history action writes HID.
- [x] Keep Ready-made themes, Composition and Library equally reachable. Selecting
  a preset replaces a draft explicitly; Save, preview and Apply remain distinct.

**Exit scenarios:** create an image + sensor + clock + text composition; select
an obscured layer, move/resize/reorder with mouse and keyboard, change a sensor,
interrupt a gesture, undo/cancel, Save/reopen, Apply, close GUI and restart daemon.
Preview must match decoded output geometry; draft actions emit no device writes.
Run focused browser behavior tests and native WebKitGTK input; Plasma Wayland
physical/scaling acceptance closes in R3. Existing crop tests are not substitutes.

**Completion:** [implementation and verification](r1-canvas-editor.md); the final
selection regression preserves the selected object when an image is inserted.
[Milestone record](../milestone-progress.md#r1-complete). No hardware gate is closed.

**Excluded:** new effects, multi-select/grouping, snapping/distribution, exact
Windows intrinsic grid reconstruction, new hardware and a timeline editor.

## R2 — Stabilize the existing core as one application

**Critical; first-release blocker. Type:** qualification + fixes demonstrated by failures.
**Depends on:** R0; final pass includes R1. **Owner areas:** daemon actors/storage,
IPC/Tauri jobs, existing UI, native harnesses. Do not rewrite working subsystems.

- [x] Cover image/video import→edit→review→Save→gallery→preview→Apply→replace/stop;
  ordinary, transparent/EXIF, GIF and MP4 inputs; source deletion after managed
  import; thumbnail/output agreement and protected defaults.
- [x] Close core lost-reply/disconnect/timeout/close cases for publication,
  replacement, delete, profile Save/Apply and both group resets. Preserve draft,
  active state and references; read back committed operations before any retry.
  Extend the already-passed image membership cases to plain/video/replacement
  and reconnect timing, rather than rerunning only the same image fixture.
- [x] Complete storage boundaries: atomicity, fsync failures, partial installation,
  disk-full/quotas, crash/startup recovery and stale revisions. Keep unknown outcomes
  visible, retain recovery resources, and avoid duplicate commits.
- [x] Qualify existing cooling modes/defaults, source fallback/manual override,
  RGB catalogs/parameters/dynamic source-unit settings, thermal priority/grace,
  sync/topology and persisted Disable across GUI navigation and daemon restart.
- [x] Check long/large media within advertised bounds, slow first read, cancellation
  before first frame, corruption during playback and joined cleanup. Establish
  measurable CPU/RSS/queue/sensor-age budgets for the release workload before
  acceptance; preserve the existing minimum20fps reference for the nominal24
  video+four-sensor case, and report actual rates rather than rounding to target.
- [x] Finish supported EN/RU errors and recovery states, keyboard/focus, readable
  diagnostics and meaningful accessible state. Inventory counts are not coverage.
- [x] Retained optional flows must not corrupt core state or leave daemon workers
  behind when cancelled/closed. Experimental status never excuses a core safety bug.
- [x] Run exact-source Rust fmt/strict Clippy/tests/rustdoc, frontend checks/build,
  localization/theme contracts and browser/native core flows. Record ignored tests
  and limitations; only broaden testing for changed paths or unresolved failures.

**Complete:** qualified at `53ef291` on 2026-10-17 for software/native MockBackend
scope. All eight requirements are reconciled in the [core ledger](r2-core-qualification.md)
and [operation matrix](r2-recovery-coverage.md). Physical devices, real Plasma
Wayland and the release package remain R3; full Windows parity remains R4–R10.

**Exit:** reproducible integrated core flows and bounded failure recovery on one
source snapshot; every open core defect resolved or shown outside the release
contract. Core faults and accessibility issues cannot be deferred as polish.
Detailed matrices: [checklist](basic-parity-checklist.md),
[media](persistent-media-v2.md), [recovery](import-publication-recovery.md),
[RGB](rgb-software-acceptance.md), [accessibility](accessibility-acceptance.md).

## R3 — Qualify and cut the first usable LCDV2 release

**Editor UX addendum:** [live-editor brief and UX1–UX3](lcd-editor-ux.md) address
observed installed-app friction. The interaction design is proposed; live canvas
with explicit Apply is confirmed. Track these core usability gaps here, separately
from optional R9 conveniences and additional R8 themes.

**Critical. Type:** real-platform/hardware/package qualification.
**Depends on:** R0–R2. **Owner areas:** package/CI, desktop lifecycle, hardware acceptance.

- [ ] Build the exact release snapshot with production features, notices and
  source/payload hashes; run remote CI and clean-host build comparison. Distinguish
  deterministic repack from binary reproducibility; investigate discrepancies.
- [ ] On actual KDE Plasma Wayland, verify installation, udev access, daemon launch,
  tray, single instance, hide/show/exit, login autostart, upgrade and uninstall with
  data retention. Record shell/compositor/toolchain versions and package dependencies.
- [ ] Perform R1 flows with real pointer/keyboard, window resize/fractional scaling,
  native choosers and screen-reader speech. Xvfb/AT-SPI tests remain supporting evidence.
- [ ] When hardware testing is authorized, record current device/firmware identity,
  single HID owner, known working pump/source/RGB restore settings and actual fan
  wiring. Firmware flashing is not part of this milestone.
- [ ] Qualify LCD image/video/overlays, stale/recovery, units, brightness/four
  rotations, LocalUI, stop/replace and operation after GUI close.
- [ ] Qualify pump duty/RPM/source/presets/fallback and available fan channels;
  prove RGB zones/modes/sync/count/thermal restore physically. Motherboard-wired
  fans are not a passing LCDV2 fan test; record unavailable and the support limit.
- [ ] Run 30-minute combined LCD+sensor+cooling+RGB acceptance with measured resource
  and timing budgets, then daemon restart, cold boot, sleep/wake and authorized
  USB reconnect. Record fresh data, stable identity and absence of stale frames.
- [ ] Restore cooling/RGB and finish at LocalUI; save pass/fail/not-run evidence.
- [ ] Publish release notes and the actual support/limitations table. Full Windows
  parity must not be claimed for this release.

**Exit / cutover:** R0–R2 complete and these gates pass for the declared target.
If physical acceptance remains deferred, label the artifact a software preview;
that does not close R3 or justify a hardware-ready claim. Defects in shipped core
reopen the relevant gate; absent optional effects/updater/Mini do not.
[Hardware protocol](lcdv2-hardware-acceptance.md),
[cooling protocol](lcdv2-m2-cooling-acceptance.md), [delivery](package-delivery.md).

## R4 — Faithful Windows import and base editor compatibility

**Expected to have; post-release. Type:** reference research + implementation + qualification.
**Depends on:** stable R2. **Owner areas:** `llcontrol-profile`, image/text renderer,
Windows import UI, stored editor and archive owner.

- [ ] Use genuine Windows exports and reference renders, with exact baseline,
  fonts/locale/values/resources recorded. Keep proprietary source assets outside
  the repository; store reproducible owned fixtures and observations.
- [ ] Complete ImageVideo/BackgroundColor 2×7 overlay mapping, intrinsic font/value
  dependent Three/Four grid reflow, wrapping/anchors/shadows/gradients and resource
  defaults. Static dividers already exist; they do not supply dynamic grid layout.
- [ ] Compare imported image/video timing, FPS/last-frame/VFR/GIF/colorspace and
  WPF image/filter behavior. Define per-scenario fidelity acceptance from reference
  before changing algorithms; do not silently loosen tolerance to pass results.
- [ ] Preserve independent current runtime/editor, inactive bank values, provenance
  and resources. Windows positions1..7 remain opaque until their semantics are
  established; do not map them to Linux named slots by array index.
- [ ] Enumerate all source curve/unit/mode combinations against the converters;
  implement applicable unsupported cases or report an explicit rejected case before
  mutation. Do not assume every RPM curve is absent or supported from old prose.
- [ ] Qualify managed fonts/missing resources/source replacement/expiry, corrupted
  archives, cancel/shutdown, stale target and uncertain commit without double import.
- [ ] Ensure imported properties are editable without losing untouched values;
  final scene and independently loaded editor both survive restart without sources.

**Exit:** faithful import of the declared base catalog from real Windows exports,
Control and Screen, with explicit per-case comparisons and no silent downgrade.
Effect-specific import completion belongs to R8. Exact Windows raster is not imposed
on Linux-authored templates. [Windows contract](windows-profiles.md),
[fonts](font-catalog.md), [editor](ga2lcd-editor.md), [video](video-editor.md).

## R5 — Windows-compatible export and round-trip

**Expected to have; explicitly requested. Type:** new research/implementation.
**Depends on:** R4 schema/resource identities; extends again in R8.
**Owner areas:** `llcontrol-profile`, archive/export storage, CLI/Tauri/desktop.

- [ ] Trace Windows import reader as well as exporter: ZIP layout, metadata/version,
  serializers, current/editor banks, resource URI rules and required defaults.
  Produce a versioned write contract with genuine accepted Windows examples.
- [ ] Define representable Linux features. For Linux-only constructs, offer explicit
  rejection or an informed conversion choice; never silently flatten a sensor,
  discard a layer, alter a curve, or pretend an image preserves editability.
- [ ] Implement bounded typed serializers and owned asset packaging for both
  Screen and FanAndLighting. Keep export read-only with respect to device state.
- [ ] Provide chooser→compatibility report→explicit export in CLI/GUI, with EN/RU
  diagnostics; validate paths, partial output cleanup and cancellation.
- [ ] On Windows2.1.28 import the generated ZIP and verify controls, images/video,
  bindings and rendered output. Export it again and compare semantic state with
  the Linux source, including inactive banks and resources.
- [ ] Test native archive interoperability separately; LCPROF01/LCGROUP1 are not
  Windows ZIP and must not be renamed to claim compatibility.

**Exit:** Linux→Windows→Linux and Windows→Linux→Windows semantic round-trips for
supported base cases; nonrepresentable cases have explicit reviewed outcomes.
Windows runtime access is an evidence dependency, not satisfied by local fixtures.

## R6 — Capture, desktop integrations and platform breadth

**Expected to have; post-release. Type:** mostly qualification, with identified gaps.
**Depends on:** stable R2/R3 support contract. **Owner areas:** capture workers,
portal bridge, sensors/FPS, desktop/Mini integrations.

- [ ] Qualify Screenshot and ScreenRecord on real Plasma Wayland portal/PipeWire:
  consent/deny/cancel, monitor/window/region, parent window, DPI/scaling, resize,
  source disappearance, portal restart, pause/resume/clear/180s limit and Save.
- [ ] Measure real capture, long/large sources and worst-case preparation; finish
  native crop/review/return-to-edit/limit/error accessibility and stalled-plugin
  cleanup. Private synthetic PipeWire nodes do not establish portal routing.
- [ ] Add qualified X11 and GNOME sessions/version ranges as separate support rows;
  do not claim universal Linux desktop support. Mini must load from installed stores,
  preserve six slots, survive login/restart and leave panel/extension activation
  under user control. Sway/other DEs remain a separate future decision.
- [ ] Finish FPS capture activation/app selection and precise metric identity;
  compare actual games with Windows PresentMon semantics. Keep logged-frame mean
  named honestly if it is not the same measure.
- [ ] Resolve additional voltage providers only with confirmed rail/identity/unit
  evidence; obtain Intel Windows source reports/workloads before claiming aggregate
  load. Qualify permissions, GPU multiplicity, stale/recovery and Spec field coverage.
- [ ] If a platform cannot expose a Windows measurement, document the actual
  limitation and seek an explicit scope adaptation; unavailable is correct behavior
  but does not prove equivalent measurement coverage.

**Exit:** independently named tested platform/provider/integration rows, complete
user workflows and truthful unavailable behavior. New environments do not invalidate
a qualified initial target unless a shared regression is found.
[Capture](screen-capture.md), [record](screen-record.md), [Mini](mini-system-info.md),
[FPS](fps-source.md), [providers](sensor-provider-inventory.md).

## R7 — Complete the safe product firmware updater

**Expected to have for full parity; excluded from first release.**
**Depends on:** target/image/recovery research and stable core ownership.
**Owner areas:** firmware coordinator/jobs/guards, IPC/CLI and firmware UI.

- [ ] Complete image/revision and ACK/activation/recovery qualification. A matching
  image hash and mock chunk count alone are insufficient.
- [ ] Connect product Inspect→explicit Apply→progress/cancel/status to the existing
  owned engine; ensure target-bound exclusive ownership and checked cooling hold.
- [ ] Add durable restart job/status/recovery, serial-less and missed-fast-reset
  policy, bounded deadlines, qualified guard/journal resolution and restoration
  only after proven no-effects or verified new identity/version.
- [ ] Close supervisor/panic/power/shutdown/publication-drain matrix, including
  failures before/between chunks, lost ACK, activation/close/reconnect and corrupt
  journals. Unknown remains quarantined; never automatically reflash/unlock.
- [ ] Qualify EN/RU recovery and diagnostics without requiring a connected device.
- [ ] Only after recovery prerequisites and separate hardware authorization, run
  physical update/reconnect/verification with appropriate recovery equipment.

**Exit:** usable qualified update and recovery workflow, not only transfer helpers.
Ordinary device identity/read-only recovery remains available before this milestone.
[Firmware contract and detailed checklist](firmware-lcdv2.md).

## R8 — Additional LCD themes, their authoring and interoperability

**Nice to have for first release; required for full catalog parity.**
**Depends on:** base editor/runtime and R4/R5 contracts; scheduled after core and
other functional expansion. **Owner areas:** effects schema/renderers, page editors,
assets, profile conversion and native acceptance.

- [ ] First finish visual/timing/background-video/fault acceptance for existing
  Rain, Snow, MeteorShower, Snow3D and Fireworks without rewriting working paths.
- [ ] Transfer geometric effects: TwistHole, Kaleidoscope, Mandala, Blossom,
  Transform, TimeTunnel, Whirlpool and RadiantRainbow.
- [ ] Transfer text effects: FlyInWording and SnowWording, including editable text,
  fonts, motion parameters and language behavior.
- [ ] Transfer Style Express: Robot, PlanetCity, Time, Steampunk, Atmosphere and
  Radio, with original assets and confirmed CPU/GPU/RAM/power/clock bindings.
- [ ] For **each** page: inventory applicable Windows settings/defaults/ranges,
  build original renderer/assets, expose all authoring controls, preserve independent
  drafts/inactive state, preview, Save/Load/reset, realtime/restart/rotation and
  sensor missing/stale behavior. Complete Windows import and export mappings.
- [ ] For **each** page: compare versioned reference output/behavior, test boundary
  values, native edit/save/Apply/restart, cancellation/failure and physical display;
  include representative combined load and resource budgets.

**Exit:** a 23-row page ledger with per-page authoring/runtime/import/export/reset
and evidence, no missing applicable settings. A named menu item, static screenshot
or decorative replacement without its authoring controls is not completion.
[Catalog](lcd-effects.md). Work page-by-page in bounded slices, not one giant port.

## R9 — Polish and explicitly chosen language breadth

**Polish; not a first-release blocker except defects in supported usability.**
**Depends on:** stable base workflows. **Owner areas:** UI/presets/locales/docs.

- [ ] Prioritize observed friction: optional snapping/guides, align/distribute,
  multi-select/grouping, better preset previews/discovery, refined motion and layout.
  These are proposals; do not add them as mandatory parity without a source/user need.
- [ ] Consider creating independent native background/overlay banks from scratch
  as an authoring convenience; existing preset creation already enables native layouts.
- [ ] Keep EN/RU complete as features grow. Resolve language scope explicitly for
  Windows de-DE/es-ES/fr-FR/ko-KR/zh-CN/zh-TW before claiming full language parity;
  add chosen locales with native strings, grammar, fonts, switching and persistence.
- [ ] Refresh onboarding, error wording, release/help documents and theme previews.
  Recheck keyboard/scaling/speech whenever changes affect those workflows.

**Exit:** selected improvements have observable user benefits and regression checks;
optional convenience work has a bounded backlog, not an endless release gate.
All-Windows-language support is either qualified or an explicit scope adaptation.

## R10 — Full-parity closure and final release qualification

**Final gate. Depends on:** R4–R8, relevant R9, and prior core acceptance.

- [x] Trace the Windows 2.1.28 motherboard-sync caller and document the per-device
  Linux pump policy. Declare observed playback support on FW 1.6 and record the
  unqualified legacy path. See the [versioned source audit](lcdv2-applicability-2.1.28.md).
- [ ] Confirm LCD-016 offline/startup-content applicability if a Windows caller,
  capture or physical demonstration becomes available. The audited product API
  exposes streaming only. Offline upload remains outside current support.
- [ ] Reconcile every LCD/COOL/RGB/SYS/FW row and platform requirement: implemented
  and qualified, explicitly not applicable, or documented agreed adaptation.
  Unresolved applicability and missing evidence are open, never silently waived.
- [ ] Confirm all 23 pages, authoring and both Windows interoperability directions;
  close sensor/desktop/capture/firmware requirements for the declared supported matrix.
- [ ] Review divergences with evidence. Missing host measurement, omitted language
  or unrepresentable Windows export is a scope decision, not “full parity” by fiat.
- [ ] Qualify a final exact-source package: complete software/native checks,
  reference/Windows round-trips, real sessions and current hardware/lifecycle/soak.
- [ ] Archive superseded scheduling only after all requirements are mapped; preserve
  raw evidence and publish an accurate supported-capability matrix/release notes.

**Exit:** no unowned or unexplained applicable requirement. Full parity refers to
this version/SKU/support contract with stated adaptations; additional convenience
ideas and new hardware are separate roadmap work.

## Traceability: existing requirements keep an owner

| Existing requirement | New owner |
|---|---|
| PAR-01; LCD-004–008/014; typography | R1 base authoring, R2/R3 operation, R4 imported fidelity |
| PAR-02; core GUI/accessibility | R1/R2/R3; optional platform expansion R6 |
| PAR-03; LCD-001/002/009–011 | R2/R3 core media; R4 reference semantics; R6 capture |
| PAR-04; LCD-003/015 | R8 effects; R2/R3 reset (not deferred with effects) |
| PAR-05; voltage/Intel/Spec/FPS | R3 supported core sources; R6 broader mapping/providers |
| PAR-06; COOL-001–004 | R2/R3; caller applicability R10 |
| PAR-07/08; RGB-001–005 | R2/R3 core control/thermal/physical coverage |
| PAR-09; SYS-001–003 | R2/R3 native persistence/reset; R4 import; R5 export; R8 effect mappings |
| PAR-10; SYS-004; LCD-012/013 | R2/R3 lifecycle/brightness/rotation; future-feature retest in its milestone |
| PAR-11; FW-001–003 | R3 identity/current transport; R7 updater; R10 legacy support |
| PAR-12; desktop/package | R0/R3 initial target; R6 optional shells/Mini; R9 locales |
| LCD-016 / LCD-017 | R10 unresolved offline research / retain proven HydroShift exclusion |
| Old basic plan | R3 core accessibility/final checks; R4 text fidelity; R6 providers |
| Old M1 plan | R3 physical display/reconnect/boot/sleep/transport recovery |
| Old M2 plan | R3 cooling/RGB/combined hardware; R6 provider gaps |
| New user requirements | R1 canvas; R3 Plasma Wayland; R4 faithful imports; R5 Windows export |

## Working and cutover discipline

Each implementation slice names scenario, source evidence, affected modules,
dependencies and exit check before coding. A newly found issue is assigned to its
owner milestone and classified by actual release impact. No dates or effort estimates
are invented before the remaining work is sized. After R3 scope freeze, only core
correctness/data-loss/lifecycle/accessibility/release blockers enter the release
branch; feature additions go to subsequent milestones. Keep working features and
user data intact. Planning completion never closes hardware or Windows acceptance.
