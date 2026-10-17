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
