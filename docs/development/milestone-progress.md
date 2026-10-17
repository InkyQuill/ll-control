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

## R1 — in progress: shared renderer geometry

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
