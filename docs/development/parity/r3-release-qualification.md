# R3 release qualification

Status: in progress. Starting source: `267fd8a` (R0–R2 complete).
The [R3 milestone](release-milestones.md#r3--qualify-and-cut-the-first-usable-lcdv2-release)
is the acceptance contract. This ledger does not substitute software checks for
installed-session or physical observations.

| Gate | Current state | Required completion |
|---|---|---|
| Exact production package | Production payload and Arch package built; four package unit tests, private install/upgrade/remove and packaged sensor/cooling GUI passed | Verify manifest/source/payload hashes, production features/notices; build package and run transaction and packaged-GUI checks |
| Remote CI | Private InkyQuill/ll-control created; first CI run 39060891301 is active | Publish authorized source to the selected repository and inspect actual CI result |
| Clean-host comparison | Local Docker endpoint unavailable | Provision an isolated build environment, record toolchain/dependencies and compare outputs; explain binary differences separately from deterministic repacking |
| Installed Plasma Wayland lifecycle | Current session is KDE Wayland; no installed daemon unit or running L-Connect process found | Install/upgrade/remove with retained data, udev access, tray/single instance/hide/show/exit and actual login autostart |
| Real input/accessibility | R1/R2 supporting tests exist | Actual compositor pointer/keyboard, fractional scaling, choosers and screen-reader speech |
| Hardware preflight | USB product reports LCDV2; this is not a firmware RPC or ownership audit | Establish current firmware/identity, single HID owner, backup, safe pump/source/RGB restore and fan wiring |
| Display and controls | Not run in R3 | Physical image/video/overlays/units/rotation/brightness/LocalUI, cooling and RGB observations |
| Combined lifecycle | Not run | Thirty-minute measured combined run, restart, cold boot, sleep/wake and agreed USB reconnect |
| Restoration and cutover | Not run | Restore agreed cooling/RGB and LocalUI; publish accurate release notes/support table only after gates pass |

## Initial host inspection

Read-only inspection found KDE Plasma Wayland, Plasma/KWin 6.8.0, kernel
7.2.9-2-cachyos and WebKitGTK 2.54.1. USB discovery sees `0416:7395`; its product
string is not accepted as freshly read firmware. No daemon was started, no
hardware configuration was changed and no package was installed by this inspection.
Pavel confirms he is available for physical observations and expects fans to be
connected to the motherboard. LCDV2 fan output therefore remains unavailable
unless wiring is established otherwise. Pump restore settings remain unknown;
no cooling mutation is qualified by an assumed default. Firmware flashing is
outside R3.

## Evidence and repository policy

Raw build/run outputs remain in ignored `evidence/r3/` and private temporary
folders. Reproducible tools, fixes, plans and qualification summaries are tracked.
User-facing installation/use/help remains under `docs/user`; this technical
qualification ledger belongs to development documentation.


## GitHub publication

[Repository](https://github.com/InkyQuill/lconnect-linux) is private. At Pavel's
request, publication used a separate filtered copy: 421 historical evidence paths
were removed, while all 195 master commits and the current source tree were
preserved. Local history was not rewritten. Legacy implementation sources were
retained. The private commit map is retained with ignored evidence; historical
short hashes in documentation identify the original local lineage.

The published R3-ledger commit is `b701acc` (local `302ebe4`). Future commits must
be transferred to that cleaned lineage before pushing; do not merge unfiltered
local history into it. CI run39060891301 is the first actual remote execution,
not yet a passing result.

## Live preflight

One isolated daemon was started with an empty configuration after backing up the
existing configuration and confirming no HID owner. Firmware RPC returned1.6;
initial telemetry was pump2400RPM, coolant36.1°C and fan0RPM. No saved firmware
operation/guard was present. Cooling requested/applied state is unset in this
session. Old host configuration contains pump80% MCU and fan60%, but is historical
intent, not proof of the present device setting; it was not replayed.

The radial-gradient and strawberry replacement passed Pavel's physical
observation. A71.15-second sample series kept pump2400–2430RPM and maximum
telemetry age0.76s. Brightness/rotation FAILED physical acceptance: commands
reported success, but the image switched to the Lian Li logo and returned without
visible dimming/rotation. Source inspection shows JPEG configuration patches send
the requested LocalUI mode while the JPEG keeper remains active. Resolution and
physical retest are required; machine success does not close this gate.

Pavel authorized pump80% MCU and static-blue pump LEDs as restore settings.
The80→90→80 pump sequence completed with continuous telemetry and restored80;
fan controls were not changed. LCD was cleared to LocalUI after the failed test.
RGB restoration/qualification has not yet run.

The candidate was installed on the host through pacman. Its service remains
disabled/inactive; the isolated daemon is the sole device owner. The native
Wayland GUI initially failed with compositor error: explicit synchronization
without an acquire point. `WEBKIT_DISABLE_DMABUF_RENDERER=1` keeps it running;
Pavel confirms the window is visible/responsive. Default-launch resolution is
still required. This is a real target-session defect, not covered by Xvfb passes.

Packaged Mini checks passed using the retained private GJS runtime, including
Qt/model/GJS/preferences/read-only CLI and private Plasma host. `/usr/bin/gjs`
was absent, so the initial invocation did not run tests; that failure is retained.

First remote Arch CI failed before tests: all-feature Rust checking enables
embedded frontend loading, but a fresh checkout has no desktop/build directory.
The workflow now builds the frontend before Rust checks. Local actionlint passed;
a remote rerun is required. Browser CI is still running.


## First target-session fixes — pending physical retest

JPEG/Theme settings now use the LCDSetting transaction instead of replaying
LocalUI. Configuration targets are explicit so replacement through Idle cannot
select the wrong mode; H264 retains host rotation and zero hardware rotation.
The regression first failed on actual mode bytes, then passed brightness/rotation,
continued image refresh and replacement at90°. All20 LCD-owner tests,42 actor
unit tests (one unrelated vendor-firmware case ignored),16 Screen-group and9
Screen-reset tests passed. Strict GUI/daemon all-target Clippy and fmt passed.
Visible rotation/brightness is still pending a rebuilt-daemon retest.

Production GUI startup now re-executes with the confirmed WebKit compatibility
setting on NVIDIA Wayland unless the user explicitly sets the renderer variable.
It preserves arguments and leaves recording-worker dispatch first. The manual
workaround is visually accepted; automatic startup needs a production build test
with the variable unset. Rust review found a replacement transition risk, now
covered by an explicit configuration target and regression. R3 is not complete.


## Physical retest and editor feedback

The rebuilt daemon still failed the static JPEG rotation test: Pavel observed the
strawberry upright and apparently at full brightness despite requested30%/90°.
Do not close that defect based on the LCDSetting wire regression.

The video-background composition with CPU, CCD, GPU and coolant readings visibly
rotated and updated its values. Pavel could not distinguish30% from full
brightness. Setting0% then visibly darkened the screen;100% was subsequently
requested to restore visibility. This confirms a physical backlight response,
not intermediate brightness accuracy or static-JPEG rotation.

The installed composition editor passed Pavel's add/select/drag/resize/undo and
window-resize check at the current desktop scaling. Usability remains open:
make canvas editing the default, consolidate live preview, and replace the long
stack of expanded layer forms with compact navigation and selected-layer controls.
Pavel explicitly requested Impeccable polish; preserve explicit Apply.

The clean7fd3058 production package passed the packaged GUI sensor/cooling test
with MockBackend and no instrumentation. This is package verification, not
physical hardware acceptance. Remote Arch/browser CI remains pending.
