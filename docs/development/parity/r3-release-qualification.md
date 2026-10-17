# R3 release qualification

Status: in progress. Starting source: `267fd8a` (R0–R2 complete).
The [R3 milestone](release-milestones.md#r3--qualify-and-cut-the-first-usable-lcdv2-release)
is the acceptance contract. This ledger does not substitute software checks for
installed-session or physical observations.

| Gate | Current state | Required completion |
|---|---|---|
| Exact production package | Production payload built successfully from committed R2 source; four package unit tests passed | Verify manifest/source/payload hashes, production features/notices; build package and run transaction and packaged-GUI checks |
| Remote CI | User authorized creating a GitHub repository with gh; preparing private InkyQuill/lconnect-linux | Publish authorized source to the selected repository and inspect actual CI result |
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
