# Milestone progress

## R0 — release baseline and repository organization

Completed the source audit, immutable build-input baseline and release feature
contract. Initial target: LCDV2 / Arch x86_64 / Plasma Wayland, with EN/RU and
basic direct canvas editing required. Final release qualification remains R3.

The R0 commit also records the substantial pre-existing implementation in the
working tree. It does not claim that work was newly developed in this milestone.
Its source baseline and earlier verification were checked before documentation
migration; no hardware run occurred here.

Developer contracts, plans and reference material are under `docs/development/`.
Users have short install/use/troubleshooting guides under `docs/user/`. Raw run
reports, screenshots, captures and checkpoint logs are Git-ignored and preserved
locally. Reproducible tests, fixtures, tools and license provenance stay tracked.
Historical Git objects were not rewritten.

Validation: local document links checked; tracked evidence inventory checked;
production edits in this organization step are reference-path comments only.
The existing candidate's saved checks are historical evidence, not a new suite run.
R1 implementation and interaction acceptance are next and receive their own commit.

## R1 implementation history

### Shared renderer geometry (intermediate state)

The renderer now returns layer foreground bounds alongside the same rendered
frame. The read-only `preview_lcd_editor` request exposes that frame and geometry
through the existing bounded preparation jobs, including decoded video previews.
Text uses actual shaped glyph bounds; sensor parts are combined; fitted images
retain Windows watermark placement. Backgrounds and effects have no handles.
The original preview request remains compatible.

Added native-coordinate/rotation conversion and draft-only move/resize helpers.
Sensor parts move together, resizing retains imported styles, and all transforms
respect the existing format limits. These helpers are not yet wired into the UI.

Verified: image crate suite, two added renderer geometry regressions, three theme
IPC scenarios, the video preview/cache/pin scenario, four transform checks,
Svelte checking and Clippy for affected Rust packages/all targets. Raw logs remain
local and ignored. These are foundational checks, not pointer/keyboard acceptance.

Remaining R1: canvas component and selection/property integration, history/cancel,
sensor rebinding and property reachability, translated controls, real browser and
native input, profile/output/restart acceptance, then the completion commit.

### R1 canvas integration

Canvas mode now uses renderer bounds for selection, pointer move/resize and
rotation-aware keyboard transforms. A layer chooser reaches obscured layers;
selection highlights the property form. Escape, pointer cancellation/loss and
window blur discard an unfinished gesture. Draft undo/redo is bounded and never
applies hardware changes. Source replacement preserves a sensor's label/style;
label/unit parts now expose the same text controls as the main value.

Browser verification covers drag/resize, cancellation, undo/redo, keyboard move
and resize, rotated coordinates, obscured-layer selection, reordering and the
explicit Apply boundary. Existing preview recovery, basic banks and font flows
also passed in the focused regression run. Native WebKitGTK input and complete
profile/output/restart acceptance still remain; R1 is not complete.

### R1 native acceptance

Added a reproducible EN/RU WebKitGTK harness using private Xvfb, accessibility,
D-Bus and MockBackend instances. Trusted pointer and keyboard input passed move,
resize, Escape cancellation and history checks. The authored four-layer scene
survived source replacement, profile Save/load, Apply and daemon/GUI restart.
Decoded mock-HID output matched preview pixels; sensor-driven rendering continued
after normal GUI close. Before Apply, LCD/fan/RGB/firmware counters remained zero.
The initial Escape failure was traced to the test driver's modifier-clearing
option releasing the held button; the corrected driver records the input order.

The [canvas contract](parity/r1-canvas-editor.md) documents implementation semantics
and runnable verification. Broad regression suites are still running, so the
milestone completion audit and final R1 status update remain open.

## R1 complete

Completed at `f004cf2` on 2026-10-17. The intermediate pending statements above
record the implementation sequence; this completion entry supersedes them.
All eight R1 work items are closed for their software/native scope.

Direct canvas editing, selection/property synchronization, rotation-aware
pointer/keyboard transforms, ordering, sensor rebinding and bounded draft history
are implemented. Existing forms, Ready-made themes, Composition, Library and
Save/preview/Apply boundaries are preserved. Review found and a failing browser
regression confirmed selection drifting after image insertion; the final fix
keeps the same layer selected and verifies subsequent edits affect that layer.

Validation: 768 broad browser checks and 1,375 Rust passes/9 ignored; strict
Clippy/fmt/rustdoc; localization and theme contracts. After the final selection
fix, 12 focused canvas checks, Svelte0/0 and production frontend/native builds
passed. EN/RU native input/profile/output/close/restart acceptance passed again
on the final source. Exact scope and reproducible commands are in the
[canvas contract](parity/r1-canvas-editor.md#completion-checks).

Implementation commits: `b8d3183`, `c3c36bc`, `7a36367`, `f004cf2`.
R0 organization/audit commit: `840a9a8`; immutable baseline: `299fe6c3`.
All raw evidence stays outside tracked files. Next is R2 integrated core
stabilization; R3 real Plasma Wayland/LCDV2/package gates remain open.


## R2 in progress

Started from `245d5eb`. The [R2 acceptance ledger](parity/r2-core-qualification.md)
maps all eight milestone requirements to current checks and remaining qualification.
Native image/video runs use isolated MockBackend instances; no physical hardware
or real desktop acceptance is implied. R2 remains open until the full ledger closes.


R2 first batch: native transparent-image/gallery and GIF/MP4 workflows passed;
managed video survived original-source deletion and daemon restart. Both edited
and plain image import passed seven readback faults in EN/RU (28 cases). Twenty
focused Rust storage/media tests passed. The media harness now retains an
accessibility listener and awaits device discovery after restart. Raw passing and
failed-run evidence is ignored. Remaining R2 gates are listed in the ledger;
none are waived by these passes.


R2 storage batch: added fixture-scoped real write/rename/fsync error injection.
Six settings/media publication scenarios passed, including pending-write recovery,
restart and uncertain post-publication readback without duplicate assets. No
production defect was found in these scenarios. Group journal/import ownership
I/O boundaries remain open; see the R2 ledger for the exact scope.


R2 second batch: 42 native EN/RU cases passed for image/video replacement and
ordinary GIF publication readback. Original resources, adjacent membership,
recovery diagnostics and no implicit Apply were checked. Two added real fsync +
SIGKILL transaction cases passed with the earlier six syscall scenarios; startup
preserved uncommitted state or completed committed intent exactly once. Existing
journal/transaction suites passed 22 tests, with one subprocess fixture excluded
from direct execution. Transport loss/reconnect and remaining R2 gates stay open.


R2 transport batch: 20 native EN/RU disconnect/timeout cases passed across five
media import/replacement flows. Actual committed-state checks confirm one
publication, preserved adjacent state, read reconnection and no implicit Apply.
The video test waits for the application's recovery diagnostic before verification
reads. Daemon-crash recovery and remaining R2 ledger gates are still open.


R2 cooling/lighting batch: 57 focused backend tests passed with no ignores.
Native catalog/palette/source-unit/topology checks passed, including disabled
dynamic mode across navigation and daemon restart while inactive profiles remain
intact. Private-session isolation and obsolete driver locale/label assumptions
were corrected. Native cooling/reset/accessibility and other R2 gates remain open.


R2 keyboard/reset batch: native Russian 1× keyboard/AT-SPI/chooser workflow
passed, including cooling, dynamic validation, font/image recovery, screen-reset
output failure/reapply and daemon reconnect. Full control reset passed across
desktop/daemon restart with newer intent preserved. 42 reset/transaction tests
passed, no ignores. Updated obsolete driver expectations and private-session
isolation; raw outputs remain ignored. R2 remains open for its unqualified gates.
