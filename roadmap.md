# Roadmap: usable LCDV2 release, then full parity

Updated **2026-11-12** for product scope; milestone evidence below is historical.
**ll-control** (UI: **LL-Control**) is an independent Rust application based on the
original .NET L-Connect 3 behavior. The user's physical target is Galahad II LCDV2
(`0416:7395`). Integrating all functions of the separate third-party
[ll-connect3](https://github.com/joeytroy/ll-connect3), including its UNI FAN SL-INF
coverage, and parity for other applicable devices are product goals. These functions
and devices are unverified until independently tested; users operate unverified
hardware at their own risk. No UNI FAN TL ownership or validation follows from the
LCDV2 target. Distinguish unimplemented functions from implemented but unverified
ones, and record hardware evidence per device and operation. The milestones below
prioritize LCDV2; they do not exclude the broader scope.

The original roadmap was updated **2026-10-17** after a source/evidence audit and decisions confirmed with
Pavel. Baseline: **Windows L-Connect 3 2.1.28.0**, **Galahad II LCDV2 0416:7395**,
square LCD **480×480**.

**R0, R1 and R2 are historical completed milestones; R3 release qualification remains open.** The immediate work sequence is the review follow-up below, starting with documentation and then software work that does not require observation. Exact Windows grid typography is a later import-fidelity task;
it does not block native Linux authoring or the initial release.

- [Complete implementation/parity audit](docs/development/parity/release-audit-2026-10-17.md)
- [Milestone plans, dependencies and exit criteria](docs/development/parity/release-milestones.md)
- [Implemented behavior and retained evidence](docs/development/parity/implementation-status.md)
- [Detailed acceptance inventory](docs/development/parity/basic-parity-checklist.md)
- [Windows source provenance/scenarios](docs/development/parity/lcdv2-2.1.28-matrix.md)

## Agreed release cutover

Ship a usable LCDV2 release first: **core editing, cooling, RGB and persistence**,
with **basic direct canvas selection/move/resize/layer ordering**, on **KDE Plasma
Wayland**. Existing core playback, sensors, EN/RU, profiles/reset and installed
lifecycle are included. Firmware flashing, extra effects and optional integrations
follow later. Experimental entry points require an explicit release disposition
and must not compromise core state.

Windows imports must be faithful. Linux-authored layouts need equivalent
capabilities, not cloned Windows geometry. **Windows-compatible export is required
later**, in addition to native Linux backup/restore.

The initial release is not a claim of full parity. It requires current physical
and real-session acceptance; while hardware testing remains deferred, an artifact
can be a software preview only. This roadmap does not authorize hardware tests.

## Milestones

| Milestone | Priority | Result / gate |
|---|---|---|
| R0 — Release contract and frozen baseline | Critical; complete | Exact source/package identity, support table and optional-feature exposure decisions |
| R1 — Base LCD visual editor | Important; complete | Direct canvas workflow, sensor rebinding, keyboard equivalents, preserved draft/Save/Apply |
| R2 — Integrated core stability | Critical; complete | Existing media/cooling/RGB/profiles recover reliably; native/software checks on one snapshot |
| R3 — First release qualification and cutover | Critical | Actual Plasma Wayland install/lifecycle/input, LCDV2 control/display/soak and release package |
| R4 — Faithful Windows imports | Expected to have | Genuine exports, intrinsic layout/resources, current/editor fidelity, safe unsupported cases |
| R5 — Windows-compatible export | Expected to have | Actual Windows import and both semantic round-trips; no silent loss of Linux-only features |
| R6 — Capture and platform integrations | Expected to have | Real portals/capture, Mini/FPS, additional sessions and supported sensor coverage |
| R7 — Product firmware updater | Expected for full parity | Inspect/Apply/progress, durable recovery/resolution and separately qualified physical update |
| R8 — Additional theme transfer and authoring | Nice to have initially; full-parity requirement | Remaining 16 pages, controls/defaults/state, own assets, runtime and import/export |
| R9 — Polish and chosen languages | Polish | Bounded authoring conveniences, discovery/visual refinement and explicitly chosen locale breadth |
| R10 — Full-parity closure | Final gate | Every applicable requirement qualified or explicitly adapted/excluded with evidence |
| Additional device and third-party function integration | Product scope; scheduling open | Implement applicable device parity and all ll-connect3 functions; track implementation and device/function validation separately |

**Release path: R0 → R1 + R2 → R3.** R2 can advance alongside R1; final acceptance
includes it. R4–R9 do not block first release unless they expose a shipped core
failure. R4 precedes base export R5; R8 extends both to effects. Additional effects
stay last among functional expansion. [Detailed plans](docs/development/parity/release-milestones.md)
contain work items, affected areas, dependencies, exit scenarios and legacy mapping.

## Review follow-up and execution order

The [2026-11-12 comprehensive review](docs/development/2026-11-12-comprehensive-app-review.md) is a historical snapshot at `5749fc6`; its findings and evidence remain intact. The table below records subsequent fixes and remaining work. Host-side implementation and tests do not establish physical, real-session, clean-host or exact-package acceptance; reassess each disposition against the final source and recorded verification.

1. **Documentation and scope first:** current scope and review dispositions are recorded here, and all review findings are registered as issues. Remaining historical documentation and comment drift stays open. [Repository rename (#29)](https://github.com/InkyQuill/lconnect-linux/issues/29) plans the separate GitHub rename to `ll-control`; prepare and then verify project metadata, README badges, clone URLs, issue/action links, package references and redirects. Preserve external project and historical evidence links where appropriate. Issue #34 additionally authorizes renaming crate, binary, service, schema and configuration identities without compatibility aliases; this is a pre-release cutover.
2. **Next, software work without physical observation:** verify the daemon ownership and installation fixes, then address UI disclosures, diagnostic/IPC robustness, localization and other bounded fixes. Investigate static-JPEG rotation semantics and develop software tests without representing them as physical acceptance. Build and qualify an exact source/package snapshot and CI as their environments become available.
3. **Deferred observation and release gate:** physical device, real desktop session, clean-host and Windows interoperability observations remain separately scheduled and authorized. Do not run hardware tests under this roadmap. Stable LCDV2 release requires the R3 evidence on the exact package; until then the product is a software preview.
4. **Expansion:** pursue required Windows interoperability, integrations, device/function coverage and parity after the relevant core dependencies. Additional effects remain last among functional expansion. The ll-connect3 functions and other applicable devices are product goals, not proof of implementation or validation; track each device and operation independently.

| Review ID | Current disposition and next work |
|---|---|
| [P0-01](https://github.com/InkyQuill/lconnect-linux/issues/1) | Startup ownership, bounded occupied-socket probing and stale-inode cleanup are implemented with regression coverage. Host-side qualification on 2026-11-13 passed the IPC suite and a production-executable losing-startup regression with isolated sockets; see [evidence and reproduction](docs/development/parity/singleton-qualification-2026-11-13.md). Repeat on the combined release revision; package acceptance remains under P1-03. |
| [P0-02](https://github.com/InkyQuill/lconnect-linux/issues/2) | Open R3 gate: physical cooling, RGB, display, soak and lifecycle acceptance on the exact package. Deferred observation. |
| [P1-01](https://github.com/InkyQuill/lconnect-linux/issues/3) | Open: recorded physical static-JPEG rotation failure; intermediate brightness inconclusive. Software investigation first; physical retest required. |
| [P1-02](https://github.com/InkyQuill/lconnect-linux/issues/4) | Fixed in `b599d92`: generated udev access no longer depends on the missing `plugdev` group. The packaging validator checks the complete logical device rule; clean Arch installation and active-session ACL acceptance remain open. |
| [P1-03](https://github.com/InkyQuill/lconnect-linux/issues/5) | Open: exact-source green CI, package and clean-host qualification are missing. Retain failure evidence and qualify the intended snapshot. |
| [P1-04](https://github.com/InkyQuill/lconnect-linux/issues/6) | Fixed in `ec55a47`: EN/RU experimental and own-risk disclosures cover recording, Windows import, FPS, procedural effects, Mini preferences and unverified device controls. Real-session and hardware acceptance remain separate. |
| [P1-05](https://github.com/InkyQuill/lconnect-linux/issues/7) | Open acceptance gap, not a proven performance defect: measure production editor latency and sustained resources at stated layer limits. |
| [P2-01](https://github.com/InkyQuill/lconnect-linux/issues/8) | Merged in PR #30 (`3db4d906`), issue closed: CLI propagates `journalctl` spawn and exit failures. Package qualification remains under P1-03. |
| [P2-02](https://github.com/InkyQuill/lconnect-linux/issues/9) | Fixed in `ced4959`: configurable connect/hello and per-call deadlines report ambiguous outcomes without automatic replay; Unix-socket regressions cover stalled and committed-but-lost responses. |
| [P2-03](https://github.com/InkyQuill/lconnect-linux/issues/10) | Merged in PR #30 (`3db4d906`), issue closed: slow multiplexed subscribers are evicted so unrelated responses can proceed; dedicated subscriptions retain reliable delivery. Package qualification remains under P1-03. |
| [P2-04](https://github.com/InkyQuill/lconnect-linux/issues/11) | Fixed in `cd6c506`: app-owned user-facing failures retain localized resources, the strict audit includes `Error`, and EN/RU tests cover image, effect and layer-limit validation. |
| [P2-05](https://github.com/InkyQuill/lconnect-linux/issues/12) | Backend fix merged in PR #30 (`3db4d906`), issue closed: fontconfig enumeration has a three-second deadline and 4 MiB output limit. Font picker follow-up remains local; package qualification remains under P1-03. |
| [P2-06](https://github.com/InkyQuill/lconnect-linux/issues/13) | Open R4: Windows import is a supported subset; genuine exports and fidelity qualification remain. |
| [P2-07](https://github.com/InkyQuill/lconnect-linux/issues/14) | Open R5: Windows-compatible export and semantic round-trips remain unimplemented. |
| [P2-08](https://github.com/InkyQuill/lconnect-linux/issues/15) | Open R6: capture, shell integrations and FPS require remaining implementation and real-environment qualification. |
| [P2-09](https://github.com/InkyQuill/lconnect-linux/issues/16) | Open: sensor breadth, identity, accuracy and recovery require evidence; unavailable values stay unavailable. |
| [P2-10](https://github.com/InkyQuill/lconnect-linux/issues/17) | Open R7: product firmware update and durable recovery are not available; physical qualification requires separate authorization. |
| [P2-11](https://github.com/InkyQuill/lconnect-linux/issues/18) | Fixed in `74f3f3e`: a versioned registry validates device, sensor and display requests/responses across server dispatch, desktop allowlists, CLI and generated TypeScript. Extend the same contract incrementally to remaining method groups. |
| [P3-01](https://github.com/InkyQuill/lconnect-linux/issues/19) | Fixed in `efdfc3c`: mechanical formatting, lint/format checks and targeted cleanups are in place; secondary workspaces lazy-load and the main entry chunk is below 500 kB with no build warnings. |
| [P3-02](https://github.com/InkyQuill/lconnect-linux/issues/20) | Fixed in `b16f0b0`: active CLI/HID comments and the LCDV2 protocol document match the implemented C-packet/H.264 codecs; historical evidence remains unchanged. |
| [P3-03](https://github.com/InkyQuill/lconnect-linux/issues/21) | Fixed in `6172ca3`: visible Mini branding became LL-Control; issue #34 now completes the pre-release package, service, schema and configuration rename. Real-shell acceptance remains under R6. |
| [P3-04](https://github.com/InkyQuill/lconnect-linux/issues/22) | Fixed in `e7dbe97`: existing invalid configuration aborts startup explicitly, missing configuration retains defaults, and failure cases have host-side coverage. |
| [P3-05](https://github.com/InkyQuill/lconnect-linux/issues/23) | Merged in PR #30 (`3db4d906`), issue closed: cargo-audit CI and policy added. Follow-up `e08fa05` removes three warnings and documents 11 deferred warnings with owners and review dates in [#31](https://github.com/InkyQuill/lconnect-linux/issues/31). This does not qualify the package; P1-03 remains open. |
| [P3-06](https://github.com/InkyQuill/lconnect-linux/issues/24) | Open R10 research: offline content, global pump sync caller and legacy firmware applicability. |
| [P4-01](https://github.com/InkyQuill/lconnect-linux/issues/25) | Open R8: remaining LCD effects and existing-effect acceptance; schedule last among functional expansion. |
| [P4-02](https://github.com/InkyQuill/lconnect-linux/issues/26) | Open R9: optional editor polish and additional locales; EN/RU correctness remains core. |
| [P4-03](https://github.com/InkyQuill/lconnect-linux/issues/27) | Open: additional hardware, desktops and package formats need per-device/session implementation and qualification; broader device and ll-connect3 function coverage remain product goals. |
| [P4-04](https://github.com/InkyQuill/lconnect-linux/issues/28) | Open: optional native authoring conveniences after core and required parity work. |

## Current baseline, not the future release

`target/distribution-20261017-realtime`: **845 build inputs**, **33 payload files**,
production GUI, both Mini stores. This audit rechecked input/manifest consistency,
package hashes and saved verification-log hashes. It includes the fan-preset endpoint
fix, Windows imports/current-editor/private preview and editable dividers.

Recorded results: 1,373 Rust passes/9 ignored, four example checks, 758 browser
passes, Svelte0/0, 1,144 static translations, 39 theme and three localization
contracts. Browser/native divider evidence concerns the unchanged preceding
frontend; new renderer binaries received separate package/production checks.
Short MockBackend video+four-sensors+dividers throughput: **23.40fps** at nominal24,
minimum20fps passed, 723 frames decoded. No new runtime/hardware test was run by
this audit. [Audit verification](docs/development/parity/evidence/2026-10-17-release-audit.json),
[candidate evidence](docs/development/parity/evidence/2026-10-17-realtime-shadows.json).

The core software and [base canvas editor](docs/development/parity/r1-canvas-editor.md)
now exist. R1 passed browser/native input, profile/output and restart acceptance
at `f004cf2`; this did not rebuild or qualify the historical candidate above.
R2 integrated core qualification passed at `53ef291`: see the
[qualification ledger](docs/development/parity/r2-core-qualification.md) for
behavior coverage, measured budgets and explicit limitations. Real sessions/hardware,
clean-host/CI release gates and full Windows fidelity remain open. Counts and
historical hardware observations do not close them.

## Scope discipline

The installed-app review identified a required [LCD editor UX addendum](docs/development/parity/lcd-editor-ux.md):
compact layers/inspector, responsive live canvas, and exposed display orientation.
UX1–UX4 are approved R3 delivery slices; UX4 covers icon actions, floating tools,
flatter surfaces and installed fonts. Live editing retains explicit hardware Apply.

LCD work has three tracks: **base editor usability (R1/R2)**, **theme transfer and
authoring (R4/R8)**, **polish (R9)**. Reset reliability, ordinary RGB, cooling and
safe persistence stay core even when additional effects are deferred.

Unknown offline-content applicability, global pump motherboard-sync caller and
legacy firmware support remain explicit research/support questions in R10.
HydroShift's separate animation editor and LCDV2 fan MB-sync/start-stop are
source-confirmed exclusions. No guessed sensor rails or artificial readings.

The previous roadmap is [archived unchanged](docs/development/parity/archive/2026-10-17-roadmap-before-release-milestones.md).
Old M1/M2/basic plans remain acceptance inventories; their overlapping checkbox
counts are not milestones or a readiness percentage. New **R0–R10** plans own
scheduling and cutover. Historical checkpoint statements do not reopen implemented
features; use the [audit drift register](docs/development/parity/release-audit-2026-10-17.md#documentation-drift-found).
