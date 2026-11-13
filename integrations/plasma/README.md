# LL-Control Mini for Plasma

Plasma 6 applet with six configurable cached readings.

**Experimental / Экспериментально:** Shell integration and FPS compatibility are
not fully verified; use at your own risk. / Интеграция с оболочкой и совместимость
FPS проверены не полностью; используйте на свой риск.

The widget owns its CLI helper and does not need the Tauri GUI. English is the default; confirmed Russian
in the daemon switches the widget and its settings to Russian. Other languages
and environments are deferred.

Build (Qt 6 development libraries, CMake, Node, installed desktop dependencies):

```sh
cmake -S integrations/plasma -B /tmp/lconnect-mini-plasma-build -DCMAKE_BUILD_TYPE=Debug
cmake --build /tmp/lconnect-mini-plasma-build -j4
ctest --test-dir /tmp/lconnect-mini-plasma-build --output-on-failure
```

The shared TypeScript model is compiled to an ES module targeting ES2017 for
Qt's JavaScript engine. The same source can be used by the future GNOME adapter.
The native `net.inkyquill.lconnect` QML module launches only
`lconnectctl mini watch`, without a shell or a configurable command. Each applet
or open configuration view has one owned helper. Removing the view kills and
reaps that helper, never the daemon. Missing CLI, invalid/oversized frames,
helper crashes and stale data become unavailable readings; the helper retries
on process failure. No device discovery or hardware collection is performed.

The desktop representation shows all six cards. The panel representation shows
up to two available values and opens the full card view when activated; actual
panel placement and keyboard/popup behavior remain session acceptance gates.

The applet stores its six-slot config with Plasma KConfig. Global defaults follow
the selected CPU/GPU. An explicit source stays bound to that ID; loss does not
select another GPU. Invalid or future config is preserved until an explicit
reset. The configuration dialog edits only its draft until Plasma applies it.
Network RX/TX and RAM values require complete fresh pairs. FPS supports an explicitly selected daemon MangoHud per-frame source;
without a configured fresh source it remains unavailable. See the
[FPS contract](../../docs/development/parity/fps-source.md).

The package requires both the compiled QML module and `lconnectctl` on PATH.
Installing only the applet directory is insufficient. Release packaging must
place the module under Qt's QML import path and the applet under Plasma's package
path. No installer is run against the user's session by the development tests.

An isolated staged-package check:

```sh
cmake --install /tmp/lconnect-mini-plasma-build --prefix /tmp/lconnect-mini-plasma-build/stage
python3 integrations/plasma/tests/native-plasma.py /tmp/lconnect-mini-plasma-build --staged
```

This launches a Plasma widget test host inside private Xvfb/session D-Bus/XDG,
installs the staged package only in that private root, uses the real CLI against
a synthetic read-only Unix peer, captures English/Russian screenshots and checks
that closing the host reaps its one helper. It does not change the active Plasma
session or prove real desktop/panel/login/upgrade behavior.

Development validation used Plasma 6.7.5 / Qt 6.12.0. The source declares Plasma 6
API and Qt >=6.6 build requirements; these are not a fully qualified supported
version matrix. GNOME Shell, FPS capture/game qualification and real session acceptance remain open in the
[Mini contract](../../docs/development/parity/mini-system-info.md).

References: [Plasma configuration](https://develop.kde.org/docs/plasma/widget/configuration/),
[widget setup](https://develop.kde.org/docs/plasma/widget/setup/),
[QML JavaScript modules](https://doc.qt.io/qt-6/qtqml-javascript-imports.html).

FPS source selection uses the shared daemon cache and explicit file-session ID;
see the [source contract](../../docs/development/parity/fps-source.md). Capture configuration,
real-game compatibility and full shell/session qualification remain open.


## Verify the shipped package

From the repository root, after building an Arch package, run
`python3 tools/test-distribution-mini.py target/distribution --gjs /usr/bin/gjs --plasma-native`.
This checks the extracted production CLI, shared model and QML module without
installing into the active desktop. `--plasma-native` requires plasmawindowed,
kpackagetool6, Openbox, Xvfb, xdotool and ImageMagick. The native fixture accepts
`--package-root PREFIX --cli BINARY` to select the exact shipped inputs; its
private XDG directories are set before session D-Bus starts. It verifies both
packaged QML libraries are mapped, EN/RU and helper teardown. The default CI
mode omits the Plasma host and still runs Qt config/feed/model and GJS checks.
Real panels, GNOME Shell and version/session qualification remain separate.
