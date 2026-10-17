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
