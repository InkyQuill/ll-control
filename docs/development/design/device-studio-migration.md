# Device studio migration

User-approved direction: `DESIGN.md`, demonstrated by `prototypes/device-studio/`.
Scope: production Svelte/Tauri interface, followed by Impeccable polish and distill. No device protocol or hardware support expansion.

## Completion requirements

- Device-first overview based on daemon inventory; capability-specific editors, no LCD assumption for unknown families.
- Preserve LCD composition, themes, media galleries/import/editing/recording, profiles and archives, cooling, lighting, sensors, firmware state and settings.
- Preserve explicit Apply, device identity, unavailable-state handling, stale-response guards, recovery and drafts.
- Shared approved graphite/lilac palette, locally bundled Exo headings, consistent controls and keyboard focus.
- EN/RU, desktop/compact reflow, sufficient field/text contrast and keyboard accessibility.
- Distill: separate composition from library without unmounting in-flight library operations; common title/profile actions; quiet collapsed secondary tools, no removed capabilities.
- Polish: verify complete paths and eliminate inconsistent styling, clipping, misleading state and focus regressions.
- Browser regressions and actual isolated Tauri/WebKitGTK MockBackend acceptance; no live hardware writes.
- Finish review, design documentation, final requirement-by-requirement audit.

## Implementation

Production shell and overview implemented. Registry currently enables only the daemon's implemented `galahad2-lcdv2` family; additional device families require their own backend/editor support. The prototype's SL-INFINITY example is not shipped as a real device.

LCD composition/library panels stay mounted across local tab changes. Theme name and profiles are shared. Ready-made themes, composition and library have equally visible entry tabs; creating a preset opens its editable composition. Loading a media profile opens Composition to expose its distinct Apply action. Whole-section navigation retains the existing top-level theme draft. The lighting studio stays mounted after first entry, suspends reads while inactive and retains its device/mode draft caches across overview and LCD navigation. Other nested components retain their pre-existing section lifecycle.

Lighting color diagram is explicitly a schematic, not a physical output or animation simulation. No RGB data values were changed by UI palette migration.

Initial browser run: 323 passed / 52 failed while adapting navigation fixtures and repairing discovered regressions. This is an intermediate diagnostic result, not final acceptance. Current verification is ongoing.


## Finish review corrections

The independent reviewer requested four material corrections: place capability links under their owning selected device, expose ready-made themes as a peer entry, retain RGB drafts across section navigation, and give lighting Apply the primary treatment. All four are implemented. The reviewer scored each resolved and returned `ship` for the four-item fix list; this verdict is not a second whole-surface audit. The named Impeccable reviewer role was unavailable, so a fresh default agent used its finish-review contract.

## Verification ledger

- Historical diagnostic browser suite: 377/378 passed. The remaining video-background test omitted navigation from Library to Composition; corrected test and related scenarios passed 9/9.
- Svelte: 0 errors / 0 warnings. Production frontend build passed; Vite reports the existing large single-page chunk (~500 kB before gzip).
- Final browser suite: **378/378 passed**, including RGB draft navigation, capability guards, EN/RU, focus, contrast, compact reflow and existing editing/recovery workflows. Additional studio capture run: **3/3 passed**.
- Strict translation coverage: **859/859**, zero missing static resources (dynamic daemon data retains its existing qualification scope).
- Impeccable detector: no findings, `.impeccable/review/production-detector.json`.
- Initial native Tauri/WebKitGTK + Unix IPC + isolated MockBackend acceptance passed in `/tmp/llcontrol-native-vv41_1cf`. The harness now explicitly seeds its Russian fixture language and follows the new overview/panel navigation. Final post-review native acceptance also passed; see `device-studio-verification.json` for the full readback and source hashes.
- No real HID device, hardware output, OS accessibility tooling, installation or release is claimed by these checks.


## Requirement audit

| Requirement | Evidence |
| --- | --- |
| Device-first inventory and capability guards | `DeviceOverview.svelte`, `device-capabilities.ts`, device-studio browser tests |
| Optional LCD architecture | Only implemented family receives its editors; unknown fixture has no LCD calls/actions |
| Complete existing editing flows | 378 passing browser tests cover existing profiles, media, recording, controls and recovery |
| Explicit Apply and draft continuity | Existing mutation-guard tests plus local-panel and RGB navigation regressions |
| Approved visual system | `studio.css`, locally licensed font, final production screenshots |
| EN/RU and accessible compact layouts | 859/859 resource coverage, keyboard/contrast/reflow suites, 800px capture |
| Polish and distill | Three peer LCD panels, shared profile actions, device-owned navigation; reviewer fixes resolved |
| Finish review and documentation | `.impeccable/review/production-verdict.md`, production `DESIGN.md` and JSON sidecar |
| Native software acceptance | Final Tauri/WebKitGTK → Unix IPC → MockBackend smoke passed, including reconnect, faults and restart replay; no hardware acceptance implied |

Final native evidence originated at `/tmp/llcontrol-native-tdlry0zg` and is preserved in [device-studio-verification.json](device-studio-verification.json). The native script also preserves already-open sections when returning to the retained lighting editor. All completion requirements above are satisfied within the software-only scope.

## LCD geometry correction — 2026-10-16

GALAHAD II LCDV2 uses a square 480×480 LCD. Preview and inventory glyph now share typed device geometry; only explicitly round displays receive circular clipping. The component fixture covers both shapes without registering another hardware family. All four corner hit tests, center visibility, disconnect continuity and compact reflow passed in the current **380/380** browser suite; Svelte reports zero errors/warnings. Desktop/compact captures and both geometry fixtures were inspected. This supersedes the earlier circular preview treatment, while preserving the approved studio design and all hardware qualification boundaries. [Evidence](../parity/evidence/2026-10-16-lcd-geometry.json).
