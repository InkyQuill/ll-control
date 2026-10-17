# Roadmap: usable LCDV2 release, then full parity

Updated **2026-10-17** after a source/evidence audit and decisions confirmed with
Pavel. Baseline: **Windows L-Connect 3 2.1.28.0**, **Galahad II LCDV2 0416:7395**,
square LCD **480×480**. Other hardware remains future scope.

**Next work: R0 release contract, R1 basic direct canvas editing, and R2 core
stabilization.** Exact Windows grid typography is a later import-fidelity task;
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
| R0 — Release contract and frozen baseline | Critical | Exact source/package identity, support table and optional-feature exposure decisions |
| R1 — Base LCD visual editor | Important; release blocker | Direct canvas workflow, sensor rebinding, keyboard equivalents, preserved draft/Save/Apply |
| R2 — Integrated core stability | Critical; release blocker | Existing media/cooling/RGB/profiles recover reliably; native/software checks on one snapshot |
| R3 — First release qualification and cutover | Critical | Actual Plasma Wayland install/lifecycle/input, LCDV2 control/display/soak and release package |
| R4 — Faithful Windows imports | Expected to have | Genuine exports, intrinsic layout/resources, current/editor fidelity, safe unsupported cases |
| R5 — Windows-compatible export | Expected to have | Actual Windows import and both semantic round-trips; no silent loss of Linux-only features |
| R6 — Capture and platform integrations | Expected to have | Real portals/capture, Mini/FPS, additional sessions and supported sensor coverage |
| R7 — Product firmware updater | Expected for full parity | Inspect/Apply/progress, durable recovery/resolution and separately qualified physical update |
| R8 — Additional theme transfer and authoring | Nice to have initially; full-parity requirement | Remaining 16 pages, controls/defaults/state, own assets, runtime and import/export |
| R9 — Polish and chosen languages | Polish | Bounded authoring conveniences, discovery/visual refinement and explicitly chosen locale breadth |
| R10 — Full-parity closure | Final gate | Every applicable requirement qualified or explicitly adapted/excluded with evidence |

**Release path: R0 → R1 + R2 → R3.** R2 can advance alongside R1; final acceptance
includes it. R4–R9 do not block first release unless they expose a shipped core
failure. R4 precedes base export R5; R8 extends both to effects. Additional effects
stay last among functional expansion. [Detailed plans](docs/development/parity/release-milestones.md)
contain work items, affected areas, dependencies, exit scenarios and legacy mapping.

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

The core software exists; missing direct canvas editing, real sessions/hardware,
remaining native failures, clean-host/CI release gates and full Windows fidelity
remain open. Counts and historical hardware observations do not close them.

## Scope discipline

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
