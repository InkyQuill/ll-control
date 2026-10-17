# R2 — integrated core qualification

Status: **in progress**. Starting source: `245d5eb` (R0/R1 complete).
The eight exit requirements in [the milestone plan](release-milestones.md#r2--stabilize-the-existing-core-as-one-application)
remain authoritative. Historical passes locate coverage; they do not qualify this snapshot.

## Acceptance ledger

| Requirement | Existing reproducible coverage to inspect/run | Remaining qualification |
|---|---|---|
| Media workflow | `native-image-editor.py`, `native-media.py`, `native-basic-editors.py`; image/media store and codec tests | Fresh native import/edit/review/Save/gallery/preview/Apply/replace/stop; transparent/EXIF/GIF/MP4; original source removal; thumbnail/output; protected defaults |
| Interrupted operations | Native fault proxy, interface-language, group-archive/control-reset and keyboard harnesses; browser interrupted-operation tests | Extend image-only membership readback injection to plain/video/replacement and reconnect; lost reply/timeout/close for publication/delete/profiles/both resets; no automatic duplicate writes |
| Storage | `asset_publication/journal_tests.rs`, persistence acceptance, media upload/crash, screen reset storage | Inspect exact assertions; exercise real sync/link/rename/disk-full failures and partial installation; startup/crash/stale revision recovery; preserve uncertain outcomes |
| Cooling/RGB | Cooling IPC/mode-save, dynamic RGB IPC, lighting owner and thermal tests; native palette/keyboard/control-reset | Current native navigation/restart, mode/default/source units, manual override/fallback, thermal grace/priority, sync/topology and persisted Disable |
| Bounded work | Media reader/file/upload tests; theme preparation and render-load harnesses | Long/large supported media, slow first read, pre-frame cancellation, mid-playback corruption, joined cleanup; declare CPU/RSS/queue/sensor-age budgets before acceptance; nominal24 video+four sensors must achieve at least20fps |
| EN/RU/accessibility | Native keyboard/AT-SPI, interface-language, display-settings; browser recovery messages | Current error/recovery/focus checks for changed and newly covered flows; real speech/Wayland remain R3 |
| Optional isolation | Native recording faults, archive/Windows imports, weather; owner and cancellation tests | Retained experimental paths cannot corrupt core state or leak workers when cancelled/closed |
| Final source | Rust fmt/strict Clippy/tests/rustdoc; Svelte/build/locales/theme contracts; browser/native | Execute on integrated final implementation; record ignored cases and precise limitations |

## Execution order

1. Recheck native image/video happy paths and inventory actual assertions.
2. Extend missing native interruption/recovery scenarios and fix confirmed defects.
3. Close storage and media lifetime/resource boundaries with targeted fault checks.
4. Qualify cooling/RGB, EN/RU recovery and optional-flow isolation together.
5. Run integrated final-source checks, reconcile every row with its exit evidence,
   then commit completion. A passing test count alone does not close a row.

All runs use private MockBackend instances, private configuration and owned
processes. Physical device/session/package qualification remains R3. Generated
logs, hashes, screenshots and captures live under ignored local evidence or
private temporary directories. Tests, fixtures, harnesses, fixes and this ledger
are tracked; no run output is committed.


## First qualification batch

- Native gallery image editing passed in EN/RU: transparent source, crop and
  transforms/filters, preview pixels, replacement, explicit Apply/output and close.
- Native GIF/MP4 workflow passed, including a 586-frame video, gallery thumbnails,
  profile Save/load/Apply, protected references, deletion/reimport, encoder cancel
  and joined close. The harness now deletes the original GIF/MP4/editor files,
  restarts the daemon, and confirms each managed clip still plays and stops.
- Plain image import now has the same seven native collection-readback faults as
  edited import: stale/missing/null/pending/pending-detail/error/malformed. EN/RU
  checks preserve exactly one catalog asset, unchanged draft and adjacent membership,
  original filenames, and reactive recovery messages without duplicate writes.
- The fault proxy identifies the changed collection member from actual before/after
  state, and verifies the complete real committed collection before corrupting its
  reply. Both edited/plain EN/RU matrices passed against this updated proxy
  (28 cases). This prepares video/replacement coverage; it does not claim those flows pass.
- Twenty targeted Rust storage/media tests passed: persistence errors/retry/shutdown,
  screen reset atomicity/revisions, media format/reader, upload/reply loss and crash.
  These do not close the still-open precise I/O failure matrix.

The media driver twice observed an empty accessibility registry in short-lived
chooser probes. A persistent listener, already used by the keyboard harness,
keeps a client registered throughout the media run. Chooser path/Open assertions
remain unchanged. A separate newly added restart check exposed discovery lag
behind socket readiness; the driver now explicitly awaits the mock attachment.
Both are harness changes, not evidence of a production media defect. Failed logs
are retained alongside passing runs, outside Git.

Reproduce the added plain-image matrix:

```sh
cargo build -p lconnect-gui --features native-smoke,custom-protocol --locked
/usr/bin/python3 desktop/scripts/native-interface-language.py --collection-plain-errors
```

R2 remains in progress. Video/replacement/reconnect interruption coverage, storage
I/O boundaries, declared resource budgets, integrated cooling/RGB/optional isolation
and final-source qualification still require completion.


## Storage syscall qualification

`storage-fault-smoke.py` compiles a fixture-only preload library and launches the
owned MockBackend with a private root. A marker scopes injected failures to that
root's settings/media paths; no production daemon or host device is used.

Six scenarios passed through real Unix IPC and failed syscalls:

- Settings write ENOSPC, file-sync EIO and rename ENOSPC preserve the previous
  durable file, expose pending/error status, then recover and survive restart.
- Settings directory-sync EIO occurs after rename: new bytes exist but persistence
  remains pending. Removing the fault completes sync; restart restores the value.
- Media file-sync EIO publishes nothing; status reports failure and cancellation
  remains available.
- Media directory-sync EIO leaves an uncertain published file. Finish does not
  claim durable success; subsequent Status after recovery confirms publication,
  restart retains the receipt, and the catalog contains exactly one asset.

`SettingsSaveState.saved` means **any** successful sync in the current session;
`pending` and `last_error` describe the current unfinished write. The harness
checks that documented distinction rather than treating `saved` as a revision ACK.

```sh
/usr/bin/python3 desktop/scripts/storage-fault-smoke.py
```

The C fixture compiles with warnings treated as errors. This closes these six
boundaries only; multi-file group journal/import ownership and remaining precise
crash/link/partial-cleanup boundaries are still open in R2.


## Replacement and video publication readback

Native WebKitGTK/Unix IPC checks now cover image replacement, video replacement
and ordinary GIF import under the same seven committed-readback faults in EN/RU
(42 additional cases). Replacements retain the original asset, publish exactly
one derivative and preserve adjacent collection membership. Each video derivative
is decoded in native review before Save. Faults leave the LCD unapplied, retain
translated diagnostics and never initiate another publication. Image replacement
also checks that the existing composition draft remains unchanged.

The GIF check uses semantic object equality: JSON member order is not part of the
collection contract. A first assertion compared serialized field order; saved
state showed correct membership. The corrected comparison passed all 14 cases.
No production defect is claimed from that test-driver error.

Reproduce with `native-interface-language.py` and respectively
`--collection-replacement-errors`, `--collection-video-errors`, or
`--collection-video-import-errors` after rebuilding the native-smoke GUI.
Socket loss, timeout and reconnect timing still need separate qualification.

## Transaction crash boundaries

The storage syscall harness now also injects actual EIO during journal staging
and during profile staging after the device settings file has changed. It kills
the fixture with SIGKILL while the fault is armed, then starts a new daemon.
Before journal publication, profiles remain unchanged. After partial transaction
installation, startup recovers both files and retires the journal. Repeated
readback/restart keeps one library revision increment, not a second commit.
All eight syscall scenarios passed together. The existing publication journal
suite passed 18 tests (one subprocess entry point ignored by the ordinary runner)
and the shared state transaction suite passed four tests.


## Transport recovery qualification

All five media flows (edited/plain image import, image/video replacement and GIF
import) passed disconnect and response-timeout injection in EN/RU: 20 cases.
The proxy confirms the collection write actually committed before dropping the
connection or withholding its reply. Subsequent reads reconnect and confirm one
publication, preserved adjacent membership and no implicit LCD Apply. Native
close joins workers. The production bridge does not automatically replay writes.

Run the corresponding collection flag above together with
`--collection-transport-errors`. The video replacement test now waits for the
application's recovery diagnostic before making verification reads; a test read
issued during the intentional socket drop was itself rejected in the first run.
The corrected run passed without retrying mutations. RPC test errors now retain
the method and structured diagnostic. This is a harness sequencing change, not
a claimed production fix. Failed and passing run evidence remains ignored.

This qualifies bridge socket loss and its next-read reconnection only. Daemon
death/restart during an operation, other operation families, resource budgets
and the remaining ledger requirements stay open. R2 is not complete.


## Cooling and lighting qualification batch

The native lighting palette workflow passed on the current source, exercising
all 16 pump and six fan catalog entries and their control/default shapes,
per-mode palettes, readback, global fan topology, and CPU Celsius/GPU percent/RPM
dynamic bindings. It verifies source-specific thresholds/colors/brightness/direction,
mode switching, explicit Disable, navigation away/back, and daemon restart.
Disabled Meteor stays disabled; the inactive Static/RPM profile remains enabled.
Persisted settings and emitted mock packets agree after restart.

The driver now creates the private XDG root before launching D-Bus/portals,
cleans its document mount, explicitly selects Russian, and uses the current °C
label. The first two runs exposed obsolete driver language/label assumptions;
no production defect is claimed. Passing and failed logs remain ignored.

57 targeted backend tests passed across `cooling`, `cooling_ipc`,
`cooling_mode_save`, `dynamic_rgb`, `dynamic_rgb_ipc`, `lighting_owner`,
`lighting_palette`, and `thermal`, with no ignored tests. Inspected assertions
cover stale-source fallback, manual cancellation, mode revision/default storage,
restart, invalid source units, thermal precedence over dynamic RGB, renewed
manual grace, latest-base restoration and sync topology restoration. These
backend assertions supplement the native workflow; they do not substitute for
the remaining current native cooling/default/reset and accessibility acceptance.

Reproduce with `native-lighting-palette.py` after building the native-smoke GUI,
and `cargo test -p lconnect-daemon --locked --test cooling --test cooling_ipc
--test cooling_mode_save --test dynamic_rgb --test dynamic_rgb_ipc
--test lighting_owner --test lighting_palette --test thermal`.


## Native keyboard and reset qualification

The current Russian native keyboard workflow passed at 1× scale with real
X11 input: 861 trusted key steps, 13 AT-SPI control states and eight native
chooser interactions. Assertions verify cooling PWM, fixed/curve RPM and
unapplied drafts; thermal grace/thresholds; dynamic Celsius/RPM fields and
invalid-input recovery; image/font cancel/import/reselect and decoder/size
errors; native focus/reflow; and screen reset cancel, durable commit with
injected output failure, readback, explicit reapply without another commit,
preserved files/draft and restored focus. Daemon stop/restart restores controls
and emits ordered offline/reconnected accessibility events. Pending/saved and
validation/font-error live events were also checked.

The full control-reset native workflow passed separately: read-only preview,
cancel, complete default cooling banks/selection, one slot revision, neighboring
screen/draft preservation, explicit reapply without another commit, and both
desktop/daemon restart readback refusing to overwrite newer manual intent.

42 targeted tests passed with no ignores in `control_group_commit`,
`control_group_barrier`, `screen_reset_actor`, `screen_reset_lifecycle`, and
`screen_reset_storage`. Inspected assertions cover lost replies, two-owner
barriers, commit-before-output, unknown outcomes, stale revisions, transport
retry without storage replay, shutdown/sleep joining and neighbor preservation.

Harness corrections create private XDG roots before session services, set the
keyboard fixture language explicitly, update obsolete title/unit/error strings,
enter test speeds independently of mode defaults, retain distinct per-source
RPM thresholds, and navigate Library/Composition for font actions. Earlier
failures were driver assumptions; no production fix is claimed. Failed runs
are retained locally alongside the pass.

Reproduce: build the native-smoke GUI, then run
`LCONNECT_NATIVE_ATSPI=1 LCONNECT_NATIVE_DIALOGS=1 /usr/bin/python3 desktop/scripts/native-keyboard.py`
and `/usr/bin/python3 desktop/scripts/native-control-reset.py`. The Rust suites
above run with `cargo test -p lconnect-daemon --locked --test <suite>`.

This is Russian, 1×, isolated X11 acceptance. It does not close EN/scale coverage,
all core operation interruption matrices, real speech or Plasma Wayland.
Resource budgets and the remaining R2 requirements remain open.


## Media resource and reader lifetime batch

The [fixed resource contract](r2-media-resource-contract.md) and reproducible
workload were committed as `65cddf0` before measurement. The optimized MockBackend
run passed all budgets: 1,407 new completed frames in 60.1886 seconds (23.3765fps),
0.4948 average CPU cores including codec descendants, 151,932,928 bytes sampled
peak combined RSS, and maximum sensor age 201ms. Every captured frame (1,433
including warmup/tail) decoded at 480×480. Clear joined all codec children.
No concurrent build/test workload was run during measurement.

Six `media_reader::tests` passed: bounded prefetch/order, blocked-first-read
cancellation retaining its owner until join, read failure without stale frame
repetition, panic join, drop under backpressure, and reserved send cancellation.
These directly assert queue/ownership behavior rather than inferring it from RSS.

This closes the declared short-workload budget measurement only. Maximum-size/
duration inputs, quota boundaries, long soak and the remaining integrated R2
requirements still need qualification. It is not physical playback acceptance.


## Long-media, storage limits and quota batch

Two new storage boundary cases write, publish and reopen the exact maximums:
86,400 index entries (one hour at 24fps), and 256MiB encoded payload. Each rejects
the next unit with the specific resource-budget error, then publishes the valid
prefix unchanged; indexed reads at start/middle/end and staging cleanup pass.
The padded byte-limit units are structural fixtures, not claimed decodable video.

A separate real-codec qualification creates an owned one-hour MP4, runs the
production preparer, deletes the source, reads every prepared unit and decodes
the entire H.264 stream. All 86,400 frames decode at 480×480 with no FFprobe error
diagnostics. The final run passed in 18.07 seconds; this is processing time, not
a one-hour real-time playback soak. The source is intentionally simple (32×32
blue at 1fps), while output preparation still produces 480×480 at 24fps.

Run this costly boundary explicitly:

```sh
cargo test -p lconnect-media --locked --test hour_boundary -- --ignored
```

It is excluded from routine test execution with an explicit reason, but was
executed and passed here. Ten daemon media-file/reader/upload tests also passed,
including four-session admission, oversized declarations/AUs, cancel/retry,
64-asset quota failure without partial publication, and receipt recovery.
Formatting, strict Clippy for both changed test targets and both crates' rustdoc
passed. Review tightened specific error and decoder-diagnostic assertions.

This qualifies host preparation at the duration limit and storage at the byte
limit; it does not imply worst-case source complexity, one-hour hardware playback
or every remaining storage I/O/operation-interruption boundary. R2 remains open.
