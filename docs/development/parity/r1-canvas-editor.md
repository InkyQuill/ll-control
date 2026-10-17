# R1 — base canvas editor

**Completed 2026-10-17 at `f004cf2` for software/native acceptance.**

Scope: edit the existing LCDV2 composition model. New themes, Windows grid
reconstruction and physical Plasma Wayland acceptance remain later milestones.

## Behavior and implementation

**Edit on canvas** enables automatic measured previews. Ordinary manual/live
preview remains available. `preview_lcd_editor` returns a JPEG and foreground
bounds from one renderer pass and one sensor/clock snapshot. The request uses
bounded daemon preparation jobs; video decoding retains its source pin through
render completion. It neither applies a scene nor changes saved profiles.

Selection uses shaped glyph bounds, combined sensor-part bounds, fitted image
rectangles and divider geometry. Shadows are excluded. Backgrounds and effects
have no transform handles. The layer chooser reaches obscured or clipped items;
the properties button moves keyboard focus to that layer's existing form.

Pointer coordinates convert from the displayed square into unrotated 480px
coordinates. The image and handles rotate together. Moving a sensor keeps its
label/value/unit offsets, and clamps the whole group at the format's anchor
limits. Text resizing scales font size, wrapping width, line height and part
positions around the main value anchor. Images resize proportionally and retain
fit/watermark semantics; native-size fitting still limits enlargement. Watermark
centers retain Windows percentage quantization, including the special zero value.
Dividers resize length and thickness separately.

A pointer gesture changes the draft only on release. Escape, lost capture,
pointer cancellation, window blur or a replaced draft discards an unfinished
gesture. History covers property edits and transforms, retaining at most 50 undo
entries and 8 million serialized characters of prior drafts. Undo/redo does not
write the device. Explicit profile Load and preset replacement remain draft
operations; Save and Apply retain their separate meanings.

Keyboard controls on a selected canvas layer:

| Input | Result |
|---|---|
| Arrows | Move in the visible direction |
| Ctrl/Command + arrows | Move by 10 native pixels |
| Shift + arrows | Resize; text font changes by one size step |
| Alt + arrows | Change layer order, respecting the fixed procedural background |
| Escape | Cancel an unfinished pointer gesture |

Watermark movement uses a 5px keyboard step to remain effective with percentage
coordinates. Numeric fields remain available for every supported geometry.
Single-sensor replacement changes identity and unit, preserving the authored
label, precision and styles. Missing identities stay explicit; no source is
silently substituted. Sensor label/unit styles expose the same controls as the
main value, including managed fonts, wrapping, anchors, opacity and shadows.

## Reproducible verification

- `desktop/tests/canvas-editor.spec.ts`: real browser pointer/keyboard actions,
  cancellation/capture loss, history, source replacement, overlapping layers,
  selection retained across image insertion,
  rotated coordinates, narrow desktop layout and no implicit Apply.
- `desktop/tests/canvas-geometry.spec.ts`: native coordinate conversion, group
  clamping and bounded resizing with retained imported fields, including Windows
  watermark percentage centers.
- `crates/lconnect-image/tests/theme.rs`: measured geometry against fitted assets
  and rendered sensor parts; the ordinary renderer retains identical pixels.
- `crates/lconnect-daemon/tests/theme_ipc.rs`: timed editor preview through IPC,
  unchanged active state and preserved existing theme lifecycle behavior.
- `media_composition` preview test: measured video preview shares the original
  decoder, sensor values and cycle position, and releases its source pin.

For native input acceptance, build the frontend, the `desktop-mock` daemon
example, and `lconnect-gui` with `native-smoke,custom-protocol`. From the repo root:

```sh
/usr/bin/python3 desktop/scripts/native-canvas.py
```

The harness creates private Xvfb, D-Bus, accessibility, XDG directories and a
MockBackend. It creates a four-layer composition through the GUI, performs
trusted pointer movement/resizing/cancellation and keyboard transforms, rebinds
a sensor, saves and reloads a profile, then explicitly applies it. Before Apply,
it checks that LCD, fan, RGB and firmware write counters remain zero. It decodes
actual mock-HID JPEG output and compares every RGB pixel with the preview.
After normal GUI close it changes the synthetic sensor and verifies rendering
continues. It restarts the daemon and GUI and verifies the saved scene/profile
and output are restored. Both EN and RU run independently.

Generated logs, screenshots, event records and output frames are local evidence,
not repository files. The harness prints its temporary evidence directory.
Native X11/WebKitGTK mock acceptance does not qualify real hardware or Plasma
Wayland; those remain R3 gates.

## Completion checks

- Broad regression at `7a36367`: 768 browser tests; 1,375 Rust tests passed,
  9 ignored. Strict workspace/native-feature Clippy, formatting and rustdoc passed.
- Final source `f004cf2`: the image-insertion selection regression failed before
  the fix and passed afterward. All 12 focused canvas checks passed; Svelte
  reported 0 errors/0 warnings and the production frontend/native GUI builds passed.
- Native EN/RU acceptance reran against that final rebuilt frontend: trusted
  pointer/keyboard actions, profile Save/load, explicit Apply, decoded output
  pixel equality, continued rendering after GUI close, and daemon/GUI restart passed.
- Localization: 1,160 static resources, no missing entries; 39 theme contracts
  passed. These unchanged resources were checked during R1 integration.

The final production change after the broad run only retains the selected layer
when inserting an image; it has focused regression and repeated native coverage.
Raw reports, hashes, frames and screenshots remain in ignored local evidence.
Reproducible tests and harnesses are committed. This is R1 completion, not R2
fault-matrix closure or R3 hardware/Wayland/package qualification.
