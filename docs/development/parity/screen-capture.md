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
preserves the previous editor draft. Errors preserve it too; another capture is
admitted only after the prior owner is resolved. A pending portal dialog prevents
duplicate capture. «Отменить снимок» and editor Close join checked cancellation;
failed Close keeps the editor and its recovery controls open. Unmount attempts
owned cleanup; unresolved cleanup remains fenced in Rust. Late replies cannot
restore pixels or cancel a newer capture. Portal Cancel remains available too.

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
sends Request.Close on error/timeout (2s close deadline). Service activation/owner
lookup has a separate10s budget. Screenshot and Close use the same pinned unique
service owner, rather than following later owners of the well-known portal name.

Unconfirmed Close retains the original connection/path/error in Rust and denies
new reservations. Explicit Cancel retries that original resource. A confirmed
UnknownObject from the original owner counts as an absent request. A missing
service error alone is insufficient. If Close fails, explicit Cancel queries the
original bus connection for the exact original unique name, with a separate2s
budget. Confirmed absence resolves the logical request as `owner_gone`; failed
inspection or an owner that remains connected retains the admission fence.
No Close is redirected to a replacement service, even at the same object path.
Cancel returns `closed` or `owner_gone`; shutdown joins the same checked cleanup.
The read-only `pending_screenshot_cleanup` command exposes only an opaque ID,
with its own narrow main-window capability and canonical build-manifest entry.
An editor whose begin is rejected can adopt quarantined cleanup for explicit
Cancel; it does not adopt another editor's active capture.

The UI retains its ID while Cancel is unconfirmed, coalesces duplicate cleanup,
and reports successful cancellation only after acknowledgement. Verified owner
loss has a distinct EN/RU notice; it does not claim that an external compositor
dialog closed. Unknown cancellation receipts retain ownership until explicit retry. An in-flight
reservation is joined before editor Close. Begin acknowledgement clears the
reservation pointer even if the original capture reply arrives much later.
This state is process-local, not persistent capture replay.

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
- [x] Permanent original unique-owner loss: private-bus verification, logical
  resolution, distinct UI notice and explicit fresh capture. A replacement owner
  receives no old Close. Original bus loss remains unknown and fenced.
- [ ] Real-session owner-loss behavior: verify desktop dialog disappearance and
  recovery with actual portal/backend processes; synthetic fixtures cannot prove it.
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
`cargo build -p llcontrol-gui --features native-smoke,custom-protocol` plus the
`desktop-mock` example. It creates its own Xvfb and bus; the portal always returns
a generated fixture and never captures the active desktop.

[Cleanup recovery checkpoint](evidence/2026-10-16-screenshot-cleanup-recovery.json):
137 GUI Rust tests passed/3ignored,8 private-bus/file tests,38 browser cases,
strict Clippy/docs/fmt, Svelte0/0, locales899/899 and production custom-protocol
check passed. Private Rust tests cover refused Close, retained admission fence,
explicit retry/shutdown, original-owner binding after well-known owner replacement,
and exact-request absence versus missing service owner. Browser EN/RU cases cover
failed Cancel/editor Close, preserved source/ID, late old reply versus new capture
and adoption of quarantined cleanup.

`python3 desktop/scripts/native-screenshot.py --recovery` runs two EN/RU workflows
against a synthetic private portal: Cancel and editor Close are rejected, then
an explicit Cancel succeeds. Each original request receives exactly three Close
attempts, no new request overlaps and no image is published. The ordinary native
pixel/Save/cancel/editor-close/WM-close workflow was repeated. Both harness modes
set private XDG before D-Bus activation and clean only their own document mount.
Real portal/DPI, never-settling bridge invokes and the remaining extended lifecycle
matrix are still open.

[Owner-loss checkpoint](evidence/2026-10-16-screenshot-owner-loss.json):139 GUI
Rust tests passed/3ignored, including10 screenshot tests;42 browser cases cover
both locales, `owner_gone` versus ordinary cancellation and unsupported receipts.
Native `--owner-loss` terminates the original provider after two rejected Close
attempts, starts a replacement, verifies logical recovery and a new explicit
capture/Cancel against the replacement. The original request never reaches it.
The ordinary `--recovery` EN/RU and normal pixel/Save/Close workflows were repeated.
All native captures use synthetic images/private portals/MockBackend, without
real desktop capture or physical LCD acceptance.
