# GA2LCD ScreenRecord: current contract and acceptance

Baseline: L-Connect3 2.1.28.0 / Galahad II LCDV2. This is desktop recording,
separate from selecting intervals in an existing video with Record/Pause.

## Windows behavior confirmed from local reference

`L-Connect Editor/LConnectEditor/Views/Tools/ScreenRecordTool.cs`, outside this
repository:

- Lines304–343 select the capture rectangle in physical coordinates and start
  or resume ScnLib recording while recorded time is below180000ms.
- Lines347–358 pause, then clear with confirmation; clearing stops the recording.
- Lines389–418 update the displayed recording time, automatically pause at180s,
  then show VideoOperationDialog. Cancel returns to selection; confirmation
  stops recording and prepares the result.
- Lines481–489 pause and allow changing the capture area before resuming.
- Lines558–565 disable audio and select VP9 for GA2LCD import output. This is
  not evidence of a fixed24fps source capture rate; Linux LCD conversion remains
  its separately verified H.264/24fps output path.

## Linux ScreenCast negotiation implemented

`screencast.rs` implements the portal session layer, not the recorder or UI.
It checks advertised source types and requests one monitor/window source,
without persistent permissions. CreateSession → SelectSources → Start →
OpenPipeWireRemote holds a dedicated connection, session and owned file descriptor
until the recorder closes them. The node ID is retained for older portals;
v6 requires the stable `pipewire-serial` property to avoid following a reused
node ID. Stream metadata coordinates are not assumed to be physical pixel sizes.

Every request subscribes before making the method call. Cancellation and failure
close the active request/session with bounded calls. Session::Closed is watched
from before creation through selection and exposed to the recorder; it
interrupts pending selection. Explicit close drops the remote FD, joins the
monitor and calls Session.Close. Drop attempts cleanup as a fallback; the
recording owner must explicitly join shutdown rather than rely on Drop.

[Portal specification](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
provides the request/session lifecycle and v6 stable serial contract. The local
GStreamer pipewiresrc plugin exposes fd and target-object properties; that
property inspection does not establish capture or encode functionality.

## Recording engine implemented

`screen_record.rs` provides the blocking GStreamer engine, now run inside the
owned child process managed by `screen_record_process.rs`. It duplicates
only the supplied portal FD, targets the serial (legacy node path fallback
only when the negotiated portal supplies no serial), and monitors Session::Closed.
PipeWire must support40ms unchanged-frame keepalive. `imagefreeze` with live
output and replacement holds the latest full frame at24fps even during a
responsive delivery gap. Admitted frames receive consecutive CFR timestamps;
setup/pause buffers cannot spend saved-video time. No arbitrary default source
is selected. Explicit upstream EOS and Session::Closed remain errors.

The Linux recording policy is H.264 MP4 at24fps, raw input dimensions2…4096
per axis, even encoded dimensions, at most4320 frames/180s active time and
256MiB mux output. This is a Linux policy, not proof of Windows source FPS or
codec equivalence. Oversized source negotiation fails rather than capturing a
different source. Queue depth is2; encoder uses2 threads. MP4 staging belongs
to a TempDir; explicit finish transfers ownership to RecordingFile, and dropping
that result removes the file.

Setup/preview/region selection before explicit start, pause/resume/finish,
independent active-time budget excluding setup and pauses,
frame fence and automatic LimitReached pause are implemented without UI polling.
Resume at the limit is rejected. Stop prevents additional raw frames, drains
EOS downstream of the repeating element, checks mux/file budgets and transitions to Null. Cancel/Drop joins the
worker; errors remove staging. BUFFER_LIST is included in output accounting.
State/EOS waits have deadlines. Native plugin calls/teardown may still block
inside the child; the parent can kill and reap that process without waiting for
the plugin. The low-level library engine alone is not a kill boundary.

Synthetic tests use videotestsrc only: decoded MP4 frame count/duration,
pause gap exclusion, idempotent Resume, shortened automatic limit, sparse
updates, budget failure, cancellation and session-close notification. This
neither opens PipeWire nor captures the active desktop. Responsive stopped
delivery, held-frame duration/content, resumed updates and finite-source EOS
are also covered. Parent recovery from a nonresponsive/crashed process is synthetic-tested;
real plugin hangs, resource cost and PipeWire timestamp/resend semantics remain
qualification gates.

[Engine checkpoint](evidence/2026-10-12-screen-record-engine.json).

## Owned recorder process

The main GUI re-execs its own executable with an internal worker flag before
Tauri, single-instance handling or window initialization. No extra executable
is searched on PATH or exposed as a webview command. The parent owns the selected
portal Session, its FD and a private staging root. A UnixStream pair carries
bounded length-prefixed typed requests/replies; only the child copy of the
already-selected PipeWire FD and the private socket cross exec as4 and3.
Node/serial targeting and the existing recording engine are unchanged.

Unrelated descriptors ≥5 are marked CLOEXEC with close_range before exec;
this also preserves std's exec-error pipe until exec. Failure of that operation
fails startup instead of silently inheriting descriptors. This requires Linux
close_range with CLOSE_RANGE_CLOEXEC (kernel5.11+). Inherited socket/FD checks
precede taking ownership. Linux parent-death SIGKILL is armed before exec with
a parent identity check. This is process isolation for lifetime/cancellation,
not a sandbox against untrusted plugins.

Startup acknowledgement is bounded to15s; status/control/snapshot to5s; finish
to15s plus2s for successful process exit. Transfers check cancellation and
Session::Closed every25ms. Parent commands stay on the existing blocking owner;
the webview receives existing status/metadata and bounded JPEG, never native
paths, FDs or process IDs. Application errors such as an unavailable first
snapshot leave the live recorder usable; broken transport, malformed/oversized
messages and deadlines latch failure and kill/reap it. Reply allocation is
bounded to9MiB, covering the existing2MiB JPEG limit encoded as JSON bytes.

Cancel/Drop/GUI shutdown kill and reap the worker before releasing staging and
closing the parent portal session. They do not ask a hung plugin to execute
teardown. Failed/crashed/partly finalized output never becomes Ready. Finish
moves the completed MP4 within the parent's private root; only successful child
exit and regular-file/length/frame bounds permit an owned RecordingFile. Review
and HTTP leases retain that file through capture cancellation as before.
EN/RU explains timeout/exit/invalid-response/source-close/startup failures and
allows Clear followed by a new source selection; it never silently resumes a
failed source or saves an incomplete recording.

Tests exercise a real synthetic GStreamer child, finalized decodable MP4,
setup/crop/pause/resume, selected FD transfer, exclusion of a deliberately
non-CLOEXEC sentinel, process crash/oversized reply, startup cancellation,
nonresponsive command timeout, portal-close during finish and owner Cancel/
Shutdown. Reaping is checked before staging release. These prove software
ownership and recovery, not actual PipeWire capture or a specific vendor plugin
failure. SIGKILL cannot force a task out of an uninterruptible kernel wait;
filesystem cleanup may also block in the kernel. Those platform/resource cases
remain in acceptance. [Checkpoint](evidence/2026-10-16-screen-record-process.json).

## Desktop recording owner and commands

`screen_record_jobs.rs` reserves one caller-known32-character ID before opening
capture. Repeated begin/start recover a lost reply; a reservation without Start
expires after10s. A dedicated blocking owner opens the portal, runs recorder
controls, retains the ready file, and closes recorder before session. Admission
is held until the worker joins. Controls use a bounded queue; cancellation uses
an independent signal. Lost control acknowledgements do not abandon work, and
repeated Finish in Ready does not produce another recording.

Both Tauri builders register `screen_record_begin/start/status/control/cancel`
and the main-window capability allows only these typed operations. Begin/Start
are async wrappers so task spawning runs inside the runtime. Status returns
camelCase metadata, never a caller-selectable native path. Closing the window
joins recording shutdown alongside existing media/capture owners.

Cancel/shutdown guarantee local resource release. If remote Session.Close fails,
it is retained as Failed status and logged; local FD/worker release permits exit
without indefinitely retaining a broken portal. Actual portal Close failure,
late replies, selection timeout and native window-close qualification remain.
Ready recordings support the admission/review bridge and original-source HTTP
preview below. The GUI workflow includes Setup, recording and explicit Save.

[Owner checkpoint](evidence/2026-10-12-screen-record-owner.json).

## Recording to prepared review

`screen_record_admit` synchronously pins a Ready recording into MediaImports
using its cached finalized length. It neither writes a dummy staging file nor
starts encoding. Immutable gallery/recording leases and mutable uploads are
separate InputStorage variants. The caller must await admission, or recover its
known ID through media_status after a lost reply, before preparing/cancelling.
Cancellation before calling admission does not create a tombstone for future IDs.

`screen_record_prepare` only prepares an already-admitted ID in an owned task.
The source lease survives capture cancellation, then releases after preparation.
Output reaches the existing ReviewReady/HTTP preview path; only the existing
explicit media_confirm publishes it. Normal review cancellation removes the
prepared result. Fit and ordered crop segments use existing validation/encoding;
no filesystem path or video bytes are supplied by the webview.

Synthetic regression records a real MP4, admits it, cancels capture, verifies
source retention until review, frame-count preservation and no published asset
key. Another test cancels admission before a delayed Prepare and proves it
cannot start encoding. These are native library tests, not a GUI/daemon save
workflow qualification.

[Review bridge checkpoint](evidence/2026-10-12-screen-record-review.json).

## Ready recording source preview

Finalization inspects the completed MP4 with FFprobe before publishing Ready
metadata. The existing token-protected loopback HTTP server accepts a recording
source ID and bounded Range reads through a Ready-only lease. No native path is
exposed. Cancel clears metadata and revokes new reads; an already-admitted read
retains its immutable lease until completion. Cancellation also signals the
metadata probe; remote session cleanup errors remain observable.

The synthetic test verifies metadata, exact206 response bytes, reader bounds
and404 after cancellation. It does not test WebKit playback or deterministic
cancellation inside the metadata probe. [Checkpoint](evidence/2026-10-12-screen-record-preview.json).

## Current GUI and ownership

`RecordingSession` owns a caller-known ID and recovers ambiguous admission/start
replies using that same ID. Closing waits for admission then cancels the owned
capture; cleanup failure is observable. Generation/revision guards reject stale
status and snapshot responses after clear, resume or unmount.

Source selection opens Setup. The full-source snapshot and numeric/pointer
region controls are available before explicit Start and again on Pause. Apply
has readback; invalid/unapplied drafts block Start/Resume. Setup contributes
no saved frames or time. Snapshot is bounded JPEG (maximum640px per axis and
2MiB), not a live or pixel-exact representation of encoded output.

Manual Finish pauses first and offers Cancel/Edit/Import. The180s limit opens
the same operation choice automatically. Cancel retains the recording;
Resume is rejected at the limit. Edit supports ordered trim/crop fragments,
fit and source preview. Import prepares the whole recording with Stretch.
Prepared review can return to editing and prepare again. Failed/cancelled review
and observable preparation errors also offer Return to Edit. The editor waits
for review cleanup before reopening the retained source; cleanup failure offers
an explicit retry. Capture, name and crop/trim drafts remain. Publishing/complete
reviews cannot be discarded through this path. Publication requires explicit
Save and then gallery membership. Clear removes the unsaved capture.
Native one-shot admission/Prepare error responses, actual admitted review
cancellation, reprepare/Save and normal Close are
[accepted](evidence/2026-10-13-screen-record-review-recovery.json).

## Remaining implementation and acceptance

Only open requirements are listed here. Historical checkpoint statements are
preserved in [the checkpoint archive](screen-record-checkpoints-2026-10-12.md).

- [ ] Real X11/Wayland ScreenCast portal/PipeWire FD/serial targeting, unchanged
  frames, timestamps/segments, source loss/resizing and compositor cancellation.
- [ ] Real deadlocked plugin/PipeWire recovery and resource profiling, including
  stalled kernel/filesystem operations. Responsive delivery gaps, held-frame
  content and resumed updates are synthetic-tested; explicit source EOS is an error.
- [ ] Region physical-pixel mapping across monitors and fractional scaling;
  distinguish monitor/window selection from crop and backend capabilities.
- [ ] Expanded native crop/edit/limit failure matrix: lost replies, device switch,
  publication disconnect/close, keyboard/speech and scaling. Happy paths do not
  qualify failures or screen-reader speech.
- [ ] Portal negotiation failures beyond current private-bus fixtures: backend
  unavailability, delayed replies, version/type differences, malformed handles/FD
  and remote Close failure. Private-bus fixtures already cover cancellation,
  real FD passing, v6 serial and Session::Closed during Start.

## Reproducible software qualification

Build the current frontend and the isolated native binary before running the
harness; the JS fixture is embedded at Rust build time. Neither harness opens
real PipeWire nor uses the physical LCD. It creates private Xvfb/D-Bus/XDG/TMPDIR
and MockBackend, checks explicit publication, normal app Close and empty staging.

```sh
cd desktop
npm run build
cd ..
cargo build -p lconnect-gui --features native-smoke,custom-protocol
python3 desktop/scripts/native-recording.py
python3 desktop/scripts/native-recording.py --full-limit
```

`--full-limit` uses the production180s budget and a300s outer deadline.
The ordinary harness retains its60s deadline. The separate full engine test is:

```sh
cargo test -p lconnect-gui --lib --locked full_production_limit -- --ignored --nocapture
```

Recent evidence (each file records its exact scope and limitations):

| Scope | Evidence |
|---|---|
| Full180s native automatic dialog/Cancel/Import/review/Save | [Native limit](evidence/2026-10-12-screen-record-native-full-limit.json) |
| Setup, visual/numeric selection, native Save | [Setup](evidence/2026-10-12-screen-record-setup-ui.json) |
| Responsive stalls/recovery/EOS and current full engine limit | [Held frame](evidence/2026-10-12-screen-record-held-frame.json) |
| Manual Cancel/Edit/Import and full-source snapshot | [Operations](evidence/2026-10-12-screen-record-operation-choice.json), [snapshot](evidence/2026-10-12-screen-record-source-snapshot.json) |

All hardware acceptance remains deferred. No software checkpoint establishes
real portal interoperability or the complete Windows editor parity.
