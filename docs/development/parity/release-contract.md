# R0 release contract — 2026-10-17

The first release is a usable **Galahad II LCDV2 0416:7395 / 480×480** application
on **KDE Plasma Wayland**, packaged initially for **Arch Linux x86_64**. EN/RU are
required. Windows L-Connect3 2.1.28.0 is the capability reference.

The first-release cut does not limit product scope: applicable parity for other
devices and integration of all functions of the separate third-party
[ll-connect3](https://github.com/joeytroy/ll-connect3), including UNI FAN SL-INF,
are required goals. These are unverified unless device- and operation-specific
evidence says otherwise; use of unverified hardware is at the user's own risk.
Unimplemented functions must be identified as unavailable, not as usable merely
because they are planned. The user's LCDV2 does not imply UNI FAN TL ownership or
validation. See the [reference inventory](../2026-09-08-reference-2.1.28-and-ll-connect3.md).

## Immutable starting point

Commit **299fe6c3c20bf042e48f3bd88049227afd3324d9**, branch
`codex/r0-release-baseline-20261017`, contains exactly the 845 build inputs of
`target/distribution-20261017-realtime`. It was created with a private Git index;
the working branch, index and unrelated dirty work were not staged or changed.
[Commit/tree/input manifest](evidence/2026-10-17-r0-baseline.json).

This is the immutable **release-development baseline**, not the final release.
The package predates the snapshot commit and retains its original truthful manifest.
R3 must assign a new release commit/package after R1/R2, rather than reuse this
qualification for changed code. No release tag or remote publication is implied.

## Feature disposition

| Entry point | First-release disposition | Required qualification |
|---|---|---|
| Device overview, sensors, Spec, units and EN/RU | Core candidate, not yet hardware-qualified | R2/R3 supported providers, stale/unavailable, session/accessibility |
| LCD image/video playback, basic composition, presets, fonts, galleries | Core candidate; R1 direct canvas editing implemented | R1/R2/R3 input, persistence, output and resources |
| Cooling, manual/dynamic RGB, thermal alerts, topology | Core candidate | R2/R3 physical channels, source fallback, lifecycle and zones |
| Native profile save/load/backup/restore and group resets | Core candidate | R2/R3 data retention, errors and recovery |
| Tray/autostart/single instance/install/upgrade/remove | Core candidate | R3 real Plasma Wayland session |
| Screenshot / ScreenRecord | Experimental, retained; not supported-capture claim | R2 isolation; R6 real portal/routing/resource/fault matrix |
| Windows Control/Screen ZIP import and imported editor | Experimental, retained | R2 isolation; R4 genuine exports, fidelity and faults |
| Rain/Snow/MeteorShower/Snow3D/Fireworks | Experimental, retained | R2 runtime isolation; R8 reference/authoring/physical acceptance |
| Plasma Mini / GNOME Mini | Experimental, installed but user enables/places | R6 exact shell/version/session acceptance; no automatic panel changes |
| MangoHud FPS source/settings | Experimental, opt-in logged-frame mean | R6 app activation/identity, real-game metric and session qualification |
| Firmware identity / saved recovery inspection | Read-only core candidate | R3 device identity/latency; no flash or recovery-clear implied |
| Firmware flashing / clear quarantine / product updater | Unavailable in public GUI | R7; existing command allowlist denies these writes |
| Remaining 16 LCD pages / Windows-compatible export | Unavailable, not advertised as implemented | R8 / R5 |
| Other parity devices / ll-connect3 functions | Product goals; implementation varies and hardware is unverified | Implement missing functions and qualify each device and operation separately |
| Other DEs / legacy FW<1.5 | Unqualified, outside initial support | Separate evidence/support decision |

Experimental features remain accessible to preserve existing functionality. The
README and this table label the entire current application a **software preview**;
R3 must make the per-feature disposition visible in release UI/help before promoting
any package to a supported release. Retention never permits data loss or leaked jobs.

## Versions and evidence boundary

Historical physical observations concern LCDV2 FW1.6 (version string V2.01.041),
JPEG and diagnostic H.264. They are not current canvas or cooling acceptance.
No Plasma/Wayland version has passed R3 yet; no version range is claimed.
GNOME51 is a candidate, not supported. Recorded package platform is Arch x86_64.
Actual shell, firmware, host ABI and supported source/provider versions must be
recorded during R3, with fan wiring and known restoration settings established.

R0 contract/baseline and R1 software/native canvas acceptance are complete.
R2 integrated stability and R3 package/session/hardware acceptance remain open.
[R1 implementation and verification](r1-canvas-editor.md).
