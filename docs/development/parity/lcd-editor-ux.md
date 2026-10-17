# LCD editor redesign brief

Status: approved for implementation on2026-10-17; implementation in progress.
Live canvas / explicit device Apply confirmed
by Pavel on 2026-10-17. Based on source at `1d61c2a` and the installed-app feedback.
This is development planning, not an implementation or hardware acceptance claim.

## Outcome

Make a seven-layer theme easy to edit without searching through seven expanded
forms. Preserve the graphite/lilac Device Studio identity, all current authoring
capabilities, faithful imported layouts, EN/RU, and the square 480×480 LCD.
The canvas is the editor from entry. Editing never writes to hardware until Apply.

## Verified problems

- `desktop/src/routes/+page.svelte`: canvas editing and continuous preview are
  separate opt-ins. Every layer renders a full expanded form. The selected-layer
  shortcut merely scrolls to that form. Common profile/background/effect controls
  compete with layer editing in one long column.
- Its preview scheduler waits250ms after changes and refreshes live frames at
  one-second intervals, with integer elapsed seconds. This is a snapshot preview,
  not a smooth editor playback loop.
- `desktop/src/lib/CanvasEditor.svelte`: dragging updates the selection rectangle;
  the actual layer data changes on pointer release. `ready` depends on matching
  daemon-rendered bounds, coupling interaction to rendering latency.
- `desktop/src/lib/TextStyleControls.svelte`: wrapping, line metrics, shadow and
  basic text settings occupy the same level of importance.
- `desktop/src/lib/canvas-geometry.ts`: selection, translation, resizing, keyboard
  manipulation and screen-coordinate rotation exist. They should be retained.
- The GUI reads requested screen rotation but provides no editor control for it.
  The daemon exposes `patch_lcd_config` for orientation and brightness; theme Apply
  is a separate operation. `LcdTheme` does not store screen orientation.
- `crates/lconnect-ipc/src/theme.rs`: layers have no common stable ID, user name,
  hidden or locked metadata. A Photoshop-like eye/lock/name UI needs actual model,
  persistence and renderer work; these controls cannot be decorative.
- Physical video-composition rotation worked. Static-JPEG rotation failed. A0%
  backlight setting visibly darkened the display; intermediate brightness was
  inconclusive. These remain distinct from editor design and preview accuracy.

## Proposed workspace

```text
Theme name       Open / Save                  Unsaved changes     Apply to LCD
─────────────────────────────────────────────────────────────────────────────
Add layer   Undo / Redo          Fit / 100%       Orientation: 0° 90° 180° 270°
┌────────────────────────────────────────────┬──────────────────────────────┐
│                                            │ Layers                       │
│                 LIVE CANVAS                │ eye lock  CPU temperature    │
│                                            │ eye lock  Label              │
│  direct selection, move and resize         │ eye lock  Image              │
│  actual content follows the gesture        │ …compact rows…               │
│                                            │ Background                   │
│                                            ├──────────────────────────────┤
│                                            │ Selected layer               │
│                                            │ Content                      │
│                                            │ Position / size              │
│                                            │ Appearance                   │
│                                            │ ▸ Advanced                   │
└────────────────────────────────────────────┴──────────────────────────────┘
Draft status / connection / render error           Display settings / LocalUI
```

Keep device navigation outside this workspace. Ready-made themes and Library
remain equally discoverable entry points; selecting content opens the same editor.
Do not require a mode checkbox, Refresh, or Continuous preview checkbox.

The canvas and inspector stay in place. The layer stack and long inspector scroll
independently. At narrower desktop widths, keep a usable canvas alongside a single
switchable Layers / Properties pane; at the smallest supported window, dock that
pane below. Do not shrink all controls or create horizontal page scrolling.
Verify at1280×800 and1024×768 logical pixels and the user's115% Plasma scaling.

## Layer workflow

- List topmost content first, with a background row pinned last. Correctly map this
  visual order to the renderer's current order and procedural-background rules.
- Each compact row identifies content (text excerpt, sensor label or asset name),
  type, visibility, lock state and selection. Seven rows fit without page scrolling;
  up to32 layers use a bounded stack. Long names truncate with accessible full text.
- Clicking a row or canvas object selects the same layer and updates one inspector.
  Selection never scrolls to a different form. Preserve it through reorder and undo.
- Add, duplicate, delete, rename, hide/show, lock/unlock and drag reorder are explicit
  actions. Provide keyboard reorder and visible controls as alternatives to dragging.
  Adding/duplicating selects the new layer; deletion selects a sensible neighbour.
- Hidden layers remain editable in the stack and persist in saved drafts. Locked
  layers cannot be moved/resized accidentally but can be selected and unlocked.
  Locking is an editor affordance; visibility affects rendering and Apply.
- Basic transforms: move, proportional image resize, text frame/font sizing with
  clear labels, numeric position/size, and current keyboard gestures. Undo groups
  one drag or committed numeric edit, not every pointer sample. Escape cancels.
- Screen orientation is0/90/180/270° for the entire output. It is not individual
  layer rotation. Arbitrary per-layer rotation, groups and multi-selection are
  separate future capabilities and must not appear as nonfunctional controls.

## Inspector coverage

| Selection | Immediately available | Progressive detail |
| --- | --- | --- |
| Text | Text, font, size, colour, alignment, position | Wrapping, line height, vertical anchor, opacity, shadow, imported layout metrics |
| Sensor | Source, label, unit, precision, position, value styling | Separate label/value/unit styles, wrapping and shadows |
| Sensor cycle | Sources and order, interval, formatting | Per-source labels/units and independent part styles |
| Clock/date | Format,12/24h, seconds, font, position | Wrapping, opacity and shadow |
| Image/watermark | Asset, fit, position, size, opacity | Windows watermark positioning and shadow |
| Divider | Axis, length, thickness, position, colour | Opacity and shadow |
| Background/effect | Current solid/image/video/effect choice and relevant controls | Existing effect parameters and page-specific state |

Advanced settings are collapsed initially, show a concise indication when used,
and retain their values when collapsed. Imported values never reset on selection.
Basic/template and effect-page state must survive navigation; the inspector is a
new presentation of those capabilities, not a flattened replacement of their model.
Image/video crop tools and media management remain separate contextual workflows.

## Live canvas and Apply

- Local interaction must not wait for a USB command or round trip. Pixels follow
  pointer movement, not just the outline. Hit testing uses the same geometry.
- Preserve the Rust renderer as the layout authority for Windows imports. First
  validate a bounded editor rendering session exposing reusable layer surfaces and
  geometry, with local transforms during gestures and authoritative rerender after
  commits. Do not silently substitute browser text metrics for imported raster layout.
- Cache unchanged layers/assets. Coalesce rapid input, reject stale generations,
  bound outstanding work and dispose sessions/textures on navigation. A slow render
  must not disable selection or discard the draft. Show a small recoverable error.
- Video/effects/clocks/sensors animate automatically. A compact Pause animation
  control freezes time-dependent preview only; editing still works. Suspend work
  when the window is hidden. Missing sensors stay unavailable rather than becoming0.
- Proposed performance acceptance: gesture feedback within50ms at p95; typical
  committed text/property update within150ms at p95; at least20fps for the existing
  video+four-sensor reference scene. Measure on the target session; these are goals,
  not current results. Retain bounded CPU/memory/queue checks for prolonged editing.
- Orientation is a local pending device setting, immediately reflected on canvas.
  Brightness is a device setting, not theme opacity: expose it under Display settings
  and do not darken the authoring canvas to pretend to reproduce physical backlight.
- Apply captures a consistent theme + pending display-setting revision for the
  selected device. Existing separate RPCs need explicit coordinated semantics:
  validate first, report partial/unknown outcomes truthfully, preserve the draft,
  and read back state before retry. Do not announce full success after one half fails.
- Theme Save preserves content/editor metadata; physical mounting orientation and
  backlight remain device settings rather than being silently added to theme files.
  Label pending device changes separately from unsaved theme changes.
- Device selection/reconnect must not apply a draft to another device. Offline
  editing retains cached content; unavailable asset rendering shows a recoverable
  state without clearing the document. Apply stays unavailable until safe.

## Delivery milestones within first-release qualification

These address observed core usability failures. They are an R3 acceptance addendum,
not a reopening of historical R1 results and not deferred wholesale to R9 polish.

### UX1 — Compact authoring workspace (critical)

- [ ] Split route responsibilities into canvas toolbar, layer stack and inspector.
- [ ] Make canvas the default; remove duplicated preview modes/refresh workflow.
- [ ] Implement compact selection/reorder/add/duplicate/delete and contextual fields.
- [ ] Add stable layer identity and persisted names/visibility/lock with explicit
      schema compatibility; update renderer, validation, profiles and import handling.
- [ ] Preserve every inspector capability in the table and existing page/editor state.
- [ ] Add local screen orientation and brightness drafts with truthful Apply outcomes.

Exit: author a seven-layer scene, find/edit any layer, reorder/duplicate/hide/lock,
undo/redo, save/reload and apply without expanding seven forms or losing settings.
Old native profiles and Windows imports still load without silent conversion loss.

### UX2 — Responsive live rendering (critical)

- [ ] Prove the rendering-session approach with text, imported font, transparent
      image, independent sensor parts, video and procedural background.
- [ ] Implement local pixel movement/resizing plus authoritative render reconciliation.
- [ ] Replace one-second snapshots with bounded live playback and coalesced changes.
- [ ] Handle stale work, disconnect, missing assets, cancellation and hidden windows.
- [ ] Measure the stated latency/frame-rate/resource targets on the production app.

Exit: text/image pixels follow dragging, typing and numeric edits feel immediate,
live content advances without an opt-in, and no edit writes to the physical LCD.
A seven-layer and a32-layer stress case both remain navigable and recoverable.

### UX3 — Target-session acceptance (important; required for cutover)

- [ ] Test keyboard-only authoring, focus retention, shortcuts, EN/RU and long labels.
- [ ] Test desktop sizes and actual115% Plasma Wayland scaling with a production build.
- [ ] Check screen orientation controls against visible output on static and animated
      sources; close the static-JPEG defect before claiming universal orientation.
- [ ] Check Save/reload/restart, Apply failure, reconnect and active-output replacement.
- [ ] Run one batched visual review and at most one refinement confirmation round;
      keep screenshots/logs outside Git and track results in the qualification ledger.
- [ ] Update concise user guidance and commit each implementation milestone separately.

Exit: Pavel can complete the seven-layer task in the installed app without hunting
for controls; automated checks and real-session observations agree at their scopes.

### Later — Optional authoring conveniences (R9)

Snapping/guides, align/distribute, multi-select/groups, arbitrary layer rotation,
advanced history and timeline editing stay out of this bounded redesign unless a
verified parity requirement promotes them. Additional themes remain R8.

## UX1 implementation checkpoint

Compact stack and contextual inspector are implemented, with default canvas mode,
layer duplication/reordering, persisted optional identity/name/visibility/lock,
hidden-layer rendering, advanced typography disclosure, and staged screen controls.
Existing themes without metadata remain readable. Seven existing canvas workflows
and a new seven-layer/metadata/staged-Apply scenario pass in the browser. Svelte
checks report0 errors/warnings; strict Rust Clippy passed before the added regression
test, and the focused hidden-layer/legacy-JSON test passes.

This is not UX1 or release closure: remaining work includes broader affected-flow
regressions, native/visual review, live pixels during gestures, rendering sessions,
performance qualification and physical orientation acceptance. Screenshots/logs
remain ignored; user documentation will change when the shipped workflow is tested.
