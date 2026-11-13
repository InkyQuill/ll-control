# LL-Control Mini for GNOME Shell

Initial GNOME Shell extension: a panel summary and six readings in its menu.

**Experimental / Экспериментально:** Shell integration and FPS compatibility are
not fully verified; use at your own risk. / Интеграция с оболочкой и совместимость
FPS проверены не полностью; используйте на свой риск.

There is no separate Mini application window. The standard extension preferences
process edits six slots and explicit source IDs through GSettings. English is the
fresh default; confirmed Russian is cached for offline use. Other languages and
Sway/other environments remain deferred.

The extension and preferences use the same pure TypeScript model as Plasma,
compiled to `mini.mjs`. The fixed `lconnectctl mini watch` helper reads the existing
daemon cache; it does not collect hardware data or require the main GUI. Install
`lconnectctl` in the session PATH separately. No arbitrary command setting exists.
Missing CLI/daemon, malformed frames, lost sources and stale readings show
unavailable; FPS uses an explicitly selected opt-in daemon MangoHud per-frame source.

## Build a candidate package

Requires Python 3, Node, the project's installed `desktop/node_modules/typescript`,
and `glib-compile-schemas`:

```sh
python3 integrations/gnome/build.py --output /tmp/lconnect-mini-gnome-build --shell-version 51
```

The output is `/tmp/lconnect-mini-gnome-build/lconnect-mini@inkyquill.net`.
The builder compiles the shared model strictly, compiles schemas and syntax-checks
JavaScript. It neither installs nor enables the extension. Each explicit
`--shell-version` is a candidate for later qualification, **not a tested support
promise**. The extension uses the ES-module API introduced in GNOME 45; no final
supported-version matrix has been selected. Do not install into an active desktop
as part of build/test.

## Isolated checks

```sh
LCONNECT_MINI_MODEL=/tmp/lconnect-mini-gnome-build/lconnect-mini@inkyquill.net/mini.mjs node --test integrations/common/tests/mini.test.mjs
python3 integrations/gnome/tests/run-feed.py /tmp/lconnect-mini-gnome-build/lconnect-mini@inkyquill.net --gjs /path/to/gjs
python3 integrations/gnome/tests/run-ipc.py /tmp/lconnect-mini-gnome-build/lconnect-mini@inkyquill.net --gjs /path/to/gjs --cli target/debug/lconnectctl
python3 integrations/gnome/tests/run-preferences.py /tmp/lconnect-mini-gnome-build/lconnect-mini@inkyquill.net --gjs /path/to/gjs
```

The feed test runs actual GJS/Gio against a synthetic fixed-name CLI. It checks
fragmented UTF-8, multiple frames, malformed/oversized input, exit/retry, silent
expiry/recovery and helper reaping with no late UI callbacks.
The preferences test requires GTK4, libadwaita, dconf, Xvfb and dbus-run-session.
It runs actual GTK controls/signals with private XDG directories and a private
session bus. It checks explicit GPU/source selection, no writes before Save,
stable source models, stale-setting detection with draft preservation, Reload,
future-schema preservation and explicit Reset/Save, EN/RU and offline language,
and helper shutdown. A second process/private bus verifies persisted slots and
language. Stale-setting detection is not an atomic cross-process CAS operation.
The IPC test uses the real CLI binary and a private synthetic Unix peer: raw labels,
Fahrenheit, loss/reconnect/new Russian sample, owned helper reaping and exactly
three read-only methods. Build the CLI separately if its binary is missing.
All server data is synthetic; tests do not start a production daemon or access HID.

## Acceptance still required

GJS/GTK tests do not run the panel code inside GNOME Shell. Enable/disable,
panel/menu keyboard and accessibility, scaling, session restart, package install/update/removal and supported
Shell versions remain open. Preferences tests invoke real controls and signals,
but do not qualify full pointer/keyboard/screen-reader flows. Full parity also
requires FPS capture/game qualification and the remaining shared sensor coverage.

FPS source selection uses the shared daemon cache and explicit file-session ID;
see the [source contract](../../docs/development/parity/fps-source.md). Capture configuration,
real-game compatibility and full shell/session qualification remain open.


## Verify the shipped package

The maintained package-consumer command is
`python3 tools/test-distribution-mini.py target/distribution --gjs /usr/bin/gjs`
from the repository root. It extracts a built Arch package to a private prefix,
checks file hashes and runs the packaged shared model, GJS feed, real production
CLI/read-only Unix peer, GTK preferences and cross-process persistence checks.
Qt config/feed tests run in the same command. Add `--plasma-native` only where
its test-host dependencies are available. GJS may be an explicit private-runtime
wrapper; no runtime or system package is installed by this tool. GitHub workflow
source invokes the default mode. This does not run GNOME Shell panel code.
