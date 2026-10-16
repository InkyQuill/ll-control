# GA2LCD capture: Windows contract and Linux implementation

Baseline: Windows L-Connect3 2.1.28.0, Galahad II LCDV2. Screenshot and
ScreenRecord are separate applicable actions (LCD-011). A screenshot does not
establish video recording parity.

## Reference

Outside the repository, `L-Connect Editor/LConnectEditor/Views/Tools/ScreenCaptureTool.cs`
lines 184–217 hide the capture overlay, capture selected rectangles, then show
ImageOperationDialog. Cancel discards the captured result and returns to selection;
confirmation returns ScreenCaptureToolResult. Shared Gallery dispatch and publication
are recorded in [LCD-011](lcdv2-2.1.28-matrix.md).

Linux uses the [Screenshot portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html):
`interactive=true`, `modal=true`; selection UI belongs to the desktop backend.
Interactive selection is a version-2 hint; region/window/monitor choices depend
on the installed backend. No claim that every backend provides all three.

## Implemented path

Image editor → «Снять экран…» → portal choice → local image inspection →
existing crop/filter preview → explicit Save → existing gallery publication.
Capture alone never calls daemon publication or LCD Apply. Portal cancellation
preserves the previous editor draft. Errors preserve it too and permit another
attempt. A pending portal dialog prevents duplicate capture. «Отменить снимок», editor
Close and unmount cancel the owned operation; late replies cannot restore pixels
to a closed editor. Portal Cancel remains available too.

The desktop command accepts no path from the webview. One capture is admitted
at a time. GUI first reserves an opaque ID, then starts capture with that ID.
Cancellation before start removes the reservation; a late capture is rejected.
A lost begin reply expires the unused reservation after10s. Once started, an
owned task survives a dropped invoke future. Cancel waits for its completion,
including an already running bounded file decode. Shutdown denies new reservations
and joins capture alongside the existing media owners.

It listens on the predicted request path before calling Screenshot,
so a Response preceding the method reply is retained. Cancellation is response1;
other nonzero responses are errors. The response wait has a120s deadline and
sends Request.Close on error/timeout (2s close deadline).

Only local file URIs without query/fragment are read. O_NOFOLLOW/O_NONBLOCK,
regular-file and16MiB checks, a bounded read and the existing image decoder
budgets run before raw bytes cross IPC. The portal-returned file is not deleted
by this application: ownership and lifetime belong to the portal provider.
No general filesystem command or shell execution is exposed to the webview.

## Remaining acceptance and implementation

- [ ] Actual X11 and Wayland portals: selection/cancel/deny, missing backend,
  fractional DPI/multiple monitors, screenshot colors and region coordinates.
- [ ] Actual compositor screenshot → editor → gallery → LCD preview; native
  raw response/editor/Save and editor/app Close already pass with the synthetic
  private portal. This does not capture or qualify the real desktop.
- [ ] Parent-window identifiers/modality and hiding the application when desired.
- [ ] Device-switch/unmount and real-compositor app-close cancellation;
  owned-task cancellation, early reservation revocation and shutdown joins are
  implemented and private-bus/browser-tested. Verify no orphaned desktop dialogs,
  including portal disconnect/Close failure and delayed method replies.
- [ ] Portal versions/backends without interactive selection: explicit capability
  handling rather than assuming selectable regions.
- [ ] [ScreenRecord](screen-record.md): session/FD negotiation реализован;
  recorder, source/region selection, start/pause/resume/clear/stop,
 180s limit, review, explicit publication, process/stream teardown, X11/Wayland.

Private-bus tests verify success before method reply, cancellation, rejection
and timeout Close. File tests cover valid bytes, malformed content, symlinks,
directories and oversized files. Browser tests verify preview/Save separation,
cancel and error draft preservation. These do not prove compositor integration.

[Software checkpoint](evidence/2026-10-12-screenshot-portal.json):54 library +2 binary
Rust tests,10 browser tests, strict Clippy/docs/fmt and frontend check/build passed.

[Cancellation checkpoint](evidence/2026-10-12-screenshot-cancellation.json):56 library +2
binary Rust tests,12 browser tests, strict Clippy/docs/fmt and frontend check/build.

[Native checkpoint](evidence/2026-10-12-native-screenshot.json): real Tauri/WebKitGTK
raw IPC, red/blue preview pixels, explicit480×480 PNG publication, portal cancel,
editor Close/focus and WM close/Request.Close passed. Harness:
`python3 desktop/scripts/native-screenshot.py`, after building frontend and
`cargo build -p lconnect-gui --features native-smoke,custom-protocol` plus the
`desktop-mock` example. It creates its own Xvfb and bus; the portal always returns
a generated fixture and never captures the active desktop.
