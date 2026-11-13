# LCD editor live rendering

The canvas updates automatically. Apply remains the only route from editor changes
to the physical display. Screen rotation and brightness are pending device settings;
profile Save stores the theme and layer metadata, not mounting/backlight settings.

## Rendering contract

The Rust renderer owns text metrics, imported fonts, shadows and layer bounds.
`preview_lcd_editor` returns the authoritative JPEG, background and measured bounds.
A request with `layer_index` returns only that layer's transparent PNG. Two matte
renders recover alpha without replacing Rust typography with browser text layout.
During a gesture the frontend composites these surfaces with local transforms;
release commits the model and requests an authoritative frame. Escape cancels.

The frontend coalesces property changes, permits one preview request at a time,
fences stale generations and keeps at most64 cached layer surfaces. Video/effects
request up to24 frames/s; static clock/sensor scenes refresh once/s. Hidden windows
stop requesting frames. Pause freezes the preview timeline; edits still rerender.

The daemon keeps one matching runtime for30 seconds and one editor video decoder.
Video decodes on demand to the requested timeline frame, with a bounded frame
channel and a separate single-slot admission gate. Paused frames do not advance.
Resuming after idle cleanup seeks from the preceding keyframe with an AU0 parameter
primer; warmup is bounded to1024 frames and the whole request has a10-second deadline.
Two seconds without a video request closes the decoder. Procedural backgrounds
share the continuous output renderer and advance once per preview frame. Maintenance closes its
admission gate; cancellation joins the media reader and reaps FFmpeg before
releasing its permit and asset pin. This path never owns a HID writer.

## Reproduce checks

- `pnpm --dir desktop check` and `pnpm --dir desktop check:locales`.
- Run Playwright from `desktop`; do not rebuild/sync SvelteKit during the run,
  because development-server reloads invalidate in-progress interaction tests.
- Build `llcontrol-gui` with `custom-protocol,native-smoke` and the daemon's
  `desktop-mock` example, then run `desktop/scripts/native-canvas.py`.
  This checks native input, seven layers, explicit Apply, profiles and restart in
  English/Russian, and captures1280×800 and1024×768 windows on isolated X11.
- `native-editor-session.py --theme <theme-or-native-result.json>` runs the same
  frontend on the current Wayland session with private settings, bus and MockBackend.
  Use `--size`, `--language`, `--layers` for both pane captures, and optimized
  binaries with `--release`.
  `--video` generates a local video fixture and binds sensor layers to mock values.
  It records viewport geometry and observed canvas updates; it never tests hardware.

Raw logs, screenshots and fixture content belong outside Git. Record conclusions
and remaining acceptance gaps in [the UX milestone](lcd-editor-ux.md), not raw output.
