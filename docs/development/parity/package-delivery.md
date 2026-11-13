# Linux package delivery

Target: Linux x86_64 / Arch-family build host. Baseline application version0.1.0;
LCDV2 parity remains incomplete. GitHub is the selected CI platform; there is
a private GitHub repository at `InkyQuill/ll-control` and an observed CI run.
There is no published release. Run39063333105 failed because the browser runner
lacked FFmpeg; the workflow now installs that test dependency. A successful run
on the new source is still required before release qualification.

## Build and contents

Use Python >=3.11, Rust1.96.0 (qualified host, package MSRVs are not qualified),
Node26.2.0 and pnpm12.9.1, CMake/Qt6 Core/Qml development files, GTK3/WebKitGTK4.1,
AppIndicator, hidapi, GStreamer development files and plugins, FFmpeg/libx264,
`glib-compile-schemas`, `makepkg`, `fakeroot`, `desktop-file-validate`.
The workflow provisions its own Arch container; it does not install dependencies
on the developer's system. A rolling container is not a frozen sysroot.

```sh
pnpm --dir desktop install --frozen-lockfile
python3 tools/build-distribution.py --output target/distribution --gnome-shell-version 51
cd target/distribution
sha256sum --check SHA256SUMS
makepkg --nodeps --noconfirm
cd ../..
python3 tools/test-distribution.py target/distribution
```

Output must not already exist. GNOME versions are explicit **candidates**, not a
support promise;51 preserves the existing development candidate. No other shell
or standalone Mini window is packaged. The builder runs the frontend build and
locked release Cargo build itself. Cargo artifact messages must contain all
three production binaries, GUI `custom-protocol`, and no `native-smoke` feature.
It does not accept an arbitrary existing debug GUI or include `verify-galahad`.

ScreenRecord self-executes the installed GUI binary; there is no external worker
to locate beside it. Its descriptor isolation requires Linux5.11+ `close_range`
CLOEXEC. Unsupported kernels fail recording startup; this requirement does not
prevent ordinary GUI/daemon use. Real capture portals/plugins remain separate
acceptance gates.

The payload includes `/usr/bin/llcontrold`, `llcontrol`, `llcontrol-gui`, a desktop
launcher/icon, systemd **user** unit with `/usr/bin/llcontrold`, the LCDV2-only udev
rule, GPL3-or-later and bundled font notices. Both Mini integrations are included
automatically in the system-wide shell stores:

| Integration | Installed location |
|---|---|
| Plasma applet | `/usr/share/plasma/plasmoids/net.inkyquill.llcontrol.mini` |
| Compiled QML module/plugin | `/usr/lib/qt6/qml/net/inkyquill/llcontrol` |
| GNOME extension and compiled local schema | `/usr/share/gnome-shell/extensions/llcontrol-mini@inkyquill.net` |

There is no install scriptlet, desktop detection, service activation, extension
enable, panel modification or direct user-home install. Each installed shell
discovers its own assets. Qt runtime is a package dependency; shell hosts and
GNOME preferences runtime are optional dependencies. Installing the package
does not pull in both complete desktop environments. User placement/extension
enable and daemon enable are explicit session actions; real login/session and
system package-manager hooks still need qualification.

## Provenance and reproducibility boundary

The tar payload, generated local PKGBUILD and manifest have SHA256SUMS. The
manifest records exact source input hashes including dirty/untracked build
inputs, source revision, tools, candidate Shell versions and installed file
hashes/modes. The manifest itself is excluded from its own file-hash table.
Source mutation during the build rejects publication. Archive order, ownership,
mtime and gzip header are normalized using `SOURCE_DATE_EPOCH` (default HEAD
commit time). The generated PKGBUILD references its adjacent tar by exact SHA256;
it has no placeholder repository download or `SKIP` checksum.

Repacking identical extracted payload bytes must reproduce the tar checksum.
This does **not** establish bit-identical Rust/Qt/frontend rebuilds across clean
hosts, toolchains, dependency repository dates or build paths. Separate frozen
toolchain/sysroot qualification is required. ABI compatibility with other Linux
distributions and package formats is not claimed.

## Checks and remaining acceptance

`tools/test-distribution.py` checks checksums, extracts the real payload safely,
verifies installed file hashes/modes and both widget stores, repacks the archive,
builds a pkgrel2 upgrade and runs real pacman install→upgrade→remove in a random
private root/database/cache/hooks tree. Dependency resolution is disabled only
for this empty private root; `ldd` checks the three binaries against the current
host. It never starts the packaged daemon or writes host package state. A user
configuration sentinel survives removal.

`python3 tools/test-distribution-gui.py target/distribution` additionally consumes
the actual `.pkg.tar.zst` GUI. It requires a prebuilt `desktop-mock` example,
Xvfb/Openbox/session D-Bus, xdotool/ImageMagick and Python GI/AT-SPI. With private
XDG and accessibility buses and zero mock devices, it checks the embedded English
frontend and connected IPC through AT-SPI, captures pixels, closes normally and
tests rejection of a missing private recorder channel before GTK startup. It
uses the production executable without native-smoke or JS injection. This is
a launch/close check, not recorder playback or full GUI acceptance.

Add `--with-device` for one mock LCDV2: native accessible actions open Pump screen,
refresh the local preview without applying, explicitly Apply the clock theme,
observe repeated JPEG transfers through MockBackend, restore LocalUI and close.
Off-screen buttons are scrolled into view through AT-SPI. No JavaScript bridge,
real HID or host daemon is used. This does not qualify physical LCD playback,
screen-reader speech, portal capture or all editor workflows.

GitHub workflow source includes Rust fmt/strict Clippy/tests/docs, frontend
checks/locales/contracts/browser, production package build and the private
transaction test. Action references are full commits from the official
[checkout](https://github.com/actions/checkout),
[setup-node](https://github.com/actions/setup-node) and
[upload-artifact](https://github.com/actions/upload-artifact) projects.
Workflow syntax validation is separate from an actual remote run.

Remaining: GitHub execution after remote setup, clean-host binary reproducibility,
real KDE/GNOME enable/panel/login/accessibility and supported-version matrix,
host install/autostart/upgrade/uninstall with device replay, other package formats
and hardware acceptance. Do not close the broad M6 delivery gate from a private
transaction alone.

## Additional production GUI scenarios

`--sensor-theme` implies one mock LCDV2. Native controls edit sensor source,
name, label and Fahrenheit units. Full mock-HID JPEG bytes must match literal
text previews for valid/missing/aged-stale/recovery, after GUI Close and after
fixture restart/replay without Apply. The observer exists only in the mock.

`--cooling` exercises pump fixed PWM, curve, source, missing/stale fallback,
recovery and manual override through native controls and explicit Apply.
Requested/applied state must match successful mock cmd138 source/duty bytes.
Fixed policy may refresh every two seconds; check user setter RPC count and
current source/duty, not absence of all repeated HID writes. Both scenarios
finish with fixed61 and LocalUI in the private fixture when run together.
No fan policy is installed; this does not control motherboard-connected fans.

```sh
python3 tools/test-distribution-gui.py target/distribution --sensor-theme --cooling
```

## Current candidate — 2026-10-17

`target/distribution-20261017-realtime` supersedes `-dividers`:845 frozen inputs,
33 payload files plus embedded manifest, both system-wide Mini stores. Production
`custom-protocol` GUI excludes `native-smoke`. Includes current cooling banks,
Windows Control/Screen import, independent scene/editor/basic banks, private
preview/recovery and editable Three/Four Sensor dividers.
[Artifacts, hashes, commands and limits](evidence/2026-10-17-realtime-shadows.json).

1373 Rust passed/9ignored including doctests,4 example tests,758 browser,
39 theme contracts and3 localization tests; strict checks,
Svelte0/0/static1144/1144 and release passed. Final source/manifest hashes match
and remain unchanged through package/native acceptance.

Private install/upgrade/remove passed hashes/modes/ABI/retention. Actual packaged
GUI0/1mock without JS injection passed7 sensor phases and12 pump states, explicit
Apply/LocalUI, normal Close, GUI/daemon restart and replay without Apply. Packaged
Qt/model/GJS/preferences/actual CLI IPC and private Plasma host passed. Separate
native EN/RU checks edited fractional divider geometry, Save/separate Apply,
actual mock-HID JPEG and GUI/daemon restart.

**Minimum20fps software gate passed after text shadow optimization:** release
MockBackend video/four sensors/three dividers produced722 new frames in30.849s,
23.40fps at nominal24. Complete723-frame H.264 stream decodes480×480. Byte-exact
RGB reference tests preserve shadow output. Earlier19.75/19.64fps failures remain
in the Divider evidence. This is a short local software run, not physical-device,
Windows raster or long-run qualification. The only source delta from Divider is
image theme rendering; frontend758-browser evidence is retained for unchanged UI.

50 dynamic outputs are an inventory. Other native faults, real KDE/GNOME panel/
session/portal/Wayland/speech/scaling, hardware/Windows/provider, GitHub and
clean-host gates remain open. GNOME51 remains a package candidate version only.
Previous [Screen evidence](evidence/2026-10-17-screen-integrated-candidate.json)
and [older archive](package-checkpoints-through-2026-10-16.md) retain prior results.

## Packaged Mini acceptance — 2026-10-17

`python3 tools/test-distribution-mini.py target/distribution --gjs /usr/bin/gjs`
validates extracted payload hashes/modes and exercises its model, GJS feed,
actual production CLI/read-only Unix IPC, GTK preferences and persisted settings.
It also runs current Qt config/feed/model tests; the compiled model must equal
the packaged model. Optional `--plasma-native` loads the shipped applet/QML
libraries in a private host and verifies EN/RU/helper teardown. The normal CI
mode skips that host; source workflow now provisions its Qt/GJS/Xvfb dependencies.
[Evidence, actual runtime and boundaries](evidence/2026-10-17-packaged-mini.json).
The existing package is unchanged. No remote CI, actual GNOME Shell panel or
active user desktop installation is claimed; real session/version gates remain.
