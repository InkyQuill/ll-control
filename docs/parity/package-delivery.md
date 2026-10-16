# Linux package delivery

Target: Linux x86_64 / Arch-family build host. Baseline application version0.1.0;
LCDV2 parity remains incomplete. GitHub is the selected CI platform; there is
currently no configured remote, published release, or observed GitHub run.

## Build and contents

Use Python >=3.11, Rust1.96.0 (qualified host, workspace MSRV1.82 is not qualified),
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

The payload includes `/usr/bin/lconnectd`, `lconnectctl`, `lconnect-gui`, a desktop
launcher/icon, systemd **user** unit with `/usr/bin/lconnectd`, the LCDV2-only udev
rule, GPL3-or-later and bundled font notices. Both Mini integrations are included
automatically in the system-wide shell stores:

| Integration | Installed location |
|---|---|
| Plasma applet | `/usr/share/plasma/plasmoids/net.inkyquill.lconnect.mini` |
| Compiled QML module/plugin | `/usr/lib/qt6/qml/net/inkyquill/lconnect` |
| GNOME extension and compiled local schema | `/usr/share/gnome-shell/extensions/lconnect-mini@inkyquill.net` |

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
