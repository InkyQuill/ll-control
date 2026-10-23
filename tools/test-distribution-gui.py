#!/usr/bin/env python3
"""Production packaged GUI: private Xvfb/D-Bus/XDG and MockBackend.

Requires desktop-mock already built, xdotool, ImageMagick and Python GI/AT-SPI.
No native-smoke feature or JavaScript injection is used. Retains logs/screenshots.
"""
import argparse
import copy
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def run(args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def wait(predicate, label, seconds=40):
    deadline = time.monotonic() + seconds
    while not predicate():
        if time.monotonic() >= deadline:
            raise TimeoutError(label)
        time.sleep(0.1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("distribution", type=Path)
    parser.add_argument("--inside", type=Path)
    parser.add_argument("--with-device", action="store_true", help="exercise explicit LCD actions on one mock LCDV2")
    parser.add_argument("--sensor-theme", action="store_true", help="native sensor theme editing, JPEG contents, missing/recovery and replay (implies --with-device)")
    parser.add_argument("--cooling", action="store_true", help="native pump PWM/source/curve, sensor fallback/recovery, override and replay (implies --with-device)")
    args = parser.parse_args()
    args.with_device = args.with_device or args.sensor_theme or args.cooling
    distribution = args.distribution.resolve()
    if args.inside is None:
        root = Path(tempfile.mkdtemp(prefix="lconnect-production-gui-"))
        session = root / "session"
        session.mkdir(mode=0o700)
        env = dict(os.environ, XDG_RUNTIME_DIR=str(session), XDG_CONFIG_HOME=str(root / "config"),
                   XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"),
                   DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={root}/no-system-bus")
        for name in ["DISPLAY", "WAYLAND_DISPLAY", "AT_SPI_BUS_ADDRESS", "DBUS_SESSION_BUS_ADDRESS"]:
            env.pop(name, None)
        for name in list(env):
            if name.startswith("LCONNECT_NATIVE_"):
                env.pop(name)
        try:
            run(["xvfb-run", "-a", "-s", "-screen 0 1600x1200x24", "dbus-run-session", "--",
                 sys.executable, __file__, distribution, "--inside", root,
                 *(["--with-device"] if args.with_device else []),
                 *(["--sensor-theme"] if args.sensor_theme else []),
                 *(["--cooling"] if args.cooling else [])], env=env)
        finally:
            target = session / "doc"
            def mounted():
                return any(row.split()[4] == str(target) for row in Path("/proc/self/mountinfo").read_text().splitlines())
            if mounted():
                cleanup = subprocess.run(["fusermount3", "-uz", "--", str(target)], capture_output=True)
                if cleanup.returncode and mounted():
                    raise RuntimeError(f"Private document mount remains: {target}")
        print(f"Production native artifacts: {root}")
        return
    root = args.inside.resolve()
    assert os.environ["XDG_RUNTIME_DIR"] == str(root / "session")
    prefix = root / "installed"
    prefix.mkdir()
    packages = list(distribution.glob("*.pkg.tar.zst"))
    assert len(packages) == 1
    run(["bsdtar", "-xf", packages[0], "-C", prefix])
    binary = prefix / "usr/bin/lconnect-gui"
    env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root / "session"), LCONNECT_NATIVE_SPEC="0" if args.with_device else "1",
               LCONNECT_NATIVE_RECOVERY_STATE="none", GDK_BACKEND="x11", GIO_USE_VFS="local",
               WEBKIT_DISABLE_DMABUF_RENDERER="1", LC_ALL="C.UTF-8",
               AT_SPI_BUS_ADDRESS=f"unix:path={root}/session/a11y.sock")
    env.pop("NO_AT_BRIDGE", None)
    # The production GUI is unchanged; only the separately built mock fixture
    # exports its completed output packets for independent frame comparisons.
    env["LCONNECT_MOCK_EXPORT_FRAMES"] = "1" if args.sensor_theme else "0"
    (root / "session/fixture-marker").write_text(f"MockBackend; devices={int(args.with_device)}; production GUI\n")
    # Actual packaged self-exec entry rejects missing private channel before GTK.
    worker = subprocess.run([binary, "--internal-screen-record-worker"], env=env,
                            capture_output=True, text=True, timeout=5)
    assert worker.returncode == 1 and "Missing private recording control socket" in worker.stderr
    (root / "worker-entry.log").write_text(worker.stderr)
    processes, logs = [], []

    def launch(command, name):
        log = (root / f"{name}.log").open("w")
        logs.append(log)
        process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        processes.append(process)
        return process

    def rpc(method, params=None):
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(5)
            connection.connect(str(root / "session/lconnectd.sock"))
            with connection.makefile("rwb") as stream:
                stream.write((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params or {})) + "\n").encode())
                stream.flush()
                while True:
                    response = json.loads(stream.readline())
                    if response.get("id") == 1:
                        break
        assert "error" not in response, response
        return response["result"]

    def click(name, expected=None):
        # Accessible native actions only: the packaged webview has no test bridge.
        script = '''
import gi, sys
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.set_timeout(500, 500)
Atspi.init()
matches = []
def walk(node, depth=0):
    if node is None or depth > 25: return
    state = node.get_state_set()
    if (node.get_name() == sys.argv[1] and node.get_role() == Atspi.Role.PUSH_BUTTON
        and state.contains(Atspi.StateType.VISIBLE) and state.contains(Atspi.StateType.ENABLED)):
        matches.append(node)
    for i in range(node.get_child_count()): walk(node.get_child_at_index(i), depth + 1)
walk(Atspi.get_desktop(0))
# Overview also offers the same destination on its device card. The first
# matching button is the sidebar destination in document/accessibility order.
expected = int(sys.argv[2])
if len(matches) != expected: sys.exit(2)
if not matches[0].get_state_set().contains(Atspi.StateType.SHOWING):
    assert Atspi.Component.scroll_to(matches[0].get_component_iface(), Atspi.ScrollType.ANYWHERE)
assert Atspi.Action.do_action(matches[0].get_action_iface(), 0)
'''
        def action():
            count = expected if expected is not None else (2 if name == "Pump screen" else 1)
            result = subprocess.run([sys.executable, "-c", script, name, str(count)], env=env, capture_output=True, text=True, timeout=15)
            if result.returncode == 2:
                return False
            assert result.returncode == 0, result.stderr
            return True
        wait(action, f"accessible action: {name}")

    def field(name, value, select=False):
        script = '''
import gi, sys, json
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.set_timeout(500, 500)
Atspi.init()
matches=[]
details=[]
def walk(node, depth=0):
    if node is None or depth>25: return
    state=node.get_state_set()
    if node.get_name()==sys.argv[1] or state.contains(Atspi.StateType.EDITABLE):
        details.append((node.get_name(),str(node.get_role()),node.get_interfaces(),state.contains(Atspi.StateType.VISIBLE),state.contains(Atspi.StateType.EDITABLE)))
    if (node.get_name()==sys.argv[1] and "Component" in node.get_interfaces()
        and state.contains(Atspi.StateType.VISIBLE)
        and (node.get_role()==Atspi.Role.COMBO_BOX if sys.argv[3]=="select" else
             state.contains(Atspi.StateType.EDITABLE) or
             (node.get_role()==Atspi.Role.SPIN_BUTTON and "Value" in node.get_interfaces()))):
        matches.append(node)
    for i in range(node.get_child_count()): walk(node.get_child_at_index(i),depth+1)
walk(Atspi.get_desktop(0))
assert len(matches)==1, details
node=matches[0]
index=None
if sys.argv[3]=="select":
    indices=[]
    def choices(n):
        if "Selection" in n.get_interfaces():
            indices.extend(i for i in range(n.get_child_count())
                           if n.get_child_at_index(i).get_name()==sys.argv[2])
        for i in range(n.get_child_count()): choices(n.get_child_at_index(i))
    choices(node)
    assert len(indices)==1, indices
    index=indices[0]
assert Atspi.Component.scroll_to(node.get_component_iface(),Atspi.ScrollType.ANYWHERE)
assert Atspi.Component.grab_focus(node.get_component_iface())
bounds=Atspi.Component.get_extents(node.get_component_iface(),Atspi.CoordType.SCREEN)
print(json.dumps(dict(index=index,x=bounds.x+bounds.width//2,y=bounds.y+bounds.height//2)))
'''
        result = subprocess.run([sys.executable, "-c", script, name, value, "select" if select else "text"], env=env, capture_output=True, text=True, timeout=15)
        (root / f"field-{name.replace(' ', '-')}.log").write_text(result.stdout + result.stderr)
        assert result.returncode == 0, result.stderr
        control = json.loads(result.stdout)
        run(["xdotool", "mousemove", "--sync", str(control["x"]), str(control["y"]), "click", "1"], env=env)
        if select:
            run(["xdotool", "key", "Home"], env=env)
            for _ in range(control["index"]):
                run(["xdotool", "key", "Down"], env=env)
            run(["xdotool", "key", "Return"], env=env)
        else:
            run(["xdotool", "key", "--clearmodifiers", "ctrl+a", "BackSpace"], env=env)
            run(["xdotool", "type", "--clearmodifiers", "--delay", "10", value], env=env)

    try:
        launch(["openbox"], "wm")
        launch(["dbus-daemon", "--config-file=/usr/share/defaults/at-spi2/accessibility.conf",
                f'--address={env["AT_SPI_BUS_ADDRESS"]}', "--nofork"], "accessibility-bus")
        wait(lambda: (root / "session/a11y.sock").exists(), "private accessibility bus")
        fixture = launch([REPO / "target/debug/examples/desktop-mock"], "fixture")
        wait(lambda: (root / "session/lconnectd.sock").exists() or fixture.poll() is not None, "fixture")
        assert fixture.poll() is None
        gui = launch([binary], "gui")
        window = None

        def visible():
            nonlocal window
            assert gui.poll() is None, "Production GUI exited before showing frontend"
            result = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(gui.pid),
                                     "--name", "^LL-Control$"], env=env, capture_output=True, text=True)
            if result.returncode == 0:
                window = result.stdout.splitlines()[0]
            return window is not None

        wait(visible, "production window")

        def rendered():
            script = '''
import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
Atspi.set_timeout(500, 500)
Atspi.init()
rows = []
def walk(node, depth=0):
    if node is None or depth > 25 or len(rows) > 1000: return
    rows.append(node.get_name())
    if "Text" in node.get_interfaces():
        text = node.get_text_iface()
        rows.append(Atspi.Text.get_text(text, 0, min(Atspi.Text.get_character_count(text), 4096)))
    for i in range(node.get_child_count()): walk(node.get_child_at_index(i), depth + 1)
walk(Atspi.get_desktop(0))
print("\\n".join(rows))
'''
            text = run([sys.executable, "-c", script], env=env, capture_output=True, text=True, timeout=15).stdout
            (root / "main-accessibility.txt").write_text(text)
            return "Daemon connected" in text and "Settings" in text

        wait(rendered, "embedded English frontend with private daemon")
        run(["import", "-window", window, root / "main.png"], env=env)
        stats = json.loads((root / "session/stats.json").read_text())
        assert stats["devices_known"] == int(args.with_device)
        if args.with_device:
            device = rpc("list_devices")[0]
            assert device["family"] == "galahad2-lcdv2"
            params = dict(dev=device["id"])
            assert rpc("get_lcd_state", params)["source"] == "Idle"
            click("Pump screen")
            if args.sensor_theme:
                # Remove both default layers so clock changes cannot masquerade
                # as sensor updates in the output comparison.
                click("Remove layer", expected=2)
                click("Remove layer")
                field("Bind sensor", "Fixture CPU · celsius", select=True)
                click("Add")
                field("Name", "Production CPU sensor")
                field("Label", "CPU live")
                field("Unit", "°F", select=True)
                (root / "session/cpu-hot").touch()
            click("Refresh preview")
            wait(lambda: rendered() and "Preview updated. Nothing was sent to the device." in
                 (root / "main-accessibility.txt").read_text(), "English preview completion notice")
            assert rpc("get_lcd_state", params)["source"] == "Idle", "Preview applied a draft"
            click("Apply to LCD")
            wait(lambda: rpc("get_lcd_state", params)["source"] == "Theme", "explicit theme application")
            wait(lambda: rendered() and "Theme sent. Settings are queued for saving." in
                 (root / "main-accessibility.txt").read_text(), "English Apply completion notice")
            theme = rpc("get_lcd_theme", params)
            if args.sensor_theme:
                assert len(theme["layers"]) == 1 and theme["layers"][0]["kind"] == "sensor", theme
                layer = theme["layers"][0]
                assert theme["name"] == "Production CPU sensor" and layer["label"] == "CPU live", theme
                assert layer["sensor_id"] == "fixture:cpu" and layer["temperature_unit"] == "fahrenheit", layer
            else:
                assert any(layer["kind"] == "clock" for layer in theme["layers"])
            first = json.loads((root / "session/stats.json").read_text())["jpeg_starts"]
            wait(lambda: json.loads((root / "session/stats.json").read_text())["jpeg_starts"] >= first + 2,
                 "realtime clock frames through mock HID")
            run(["import", "-window", window, root / "applied.png"], env=env)
            (root / "applied-theme.json").write_text(json.dumps(theme, indent=2) + "\n")
            if args.sensor_theme:
                receipts = []
                def sensor_frame(name, text, after=0):
                    literal = copy.deepcopy(theme)
                    literal["layers"] = [dict(kind="text", text=text, style=layer["style"])]
                    expected = bytes(rpc("preview_lcd_theme", dict(theme=literal)))
                    path = root / "session/lcd-frame.json"
                    observed = None
                    def matches():
                        nonlocal observed
                        if not path.exists():
                            return False
                        observed = json.loads(path.read_text())
                        return observed["sequence"] > after and bytes(observed["jpeg"]) == expected
                    wait(matches, f"actual mock-HID JPEG equals literal {text!r}")
                    assert rpc("get_lcd_theme", params) == theme, "Sensor refresh mutated saved theme"
                    (root / f"sensor-{name}.jpg").write_bytes(bytes(observed["jpeg"]))
                    (root / f"sensor-{name}.json").write_text(json.dumps(observed) + "\n")
                    receipts.append(dict(phase=name, expected_text=text, sequence=observed["sequence"]))
                    return observed["sequence"]

                sequence = sensor_frame("hot", "CPU live 203.0°F")
                (root / "session/cpu-unavailable").touch()
                sequence = sensor_frame("missing", "CPU live —", sequence)
                (root / "session/cpu-unavailable").unlink()
                sequence = sensor_frame("returned", "CPU live 203.0°F", sequence)
                (root / "session/freeze-sensors").touch()
                wait(lambda: any(s["id"] == "fixture:cpu" and s["status"] == "stale"
                                 and s["value"] is None for s in rpc("list_sensors")), "real monotonic sensor age")
                sequence = sensor_frame("stale", "CPU live —", sequence)
                (root / "session/freeze-sensors").unlink()
                sequence = sensor_frame("fresh", "CPU live 203.0°F", sequence)

                # Closing the actual GUI must leave the daemon-owned producer
                # alive. No RPC mutation is used for replay or sensor changes.
                run(["xdotool", "windowquit", window], env=env)
                gui.wait(timeout=20)
                assert gui.returncode == 0
                sequence = sensor_frame("gui-closed", "CPU live 203.0°F", sequence + 1)
                (root / "session/stop").touch()
                fixture.wait(timeout=15)
                assert fixture.returncode == 0
                for name in ["stop", "stats.json", "lcd-frame.json", "lconnectd.sock"]:
                    (root / "session" / name).unlink(missing_ok=True)
                # Initial language seeding is a fresh-fixture action. Replay
                # must consume the saved settings, not re-save revision zero.
                env.pop("LCONNECT_NATIVE_RECOVERY_STATE", None)
                fixture = launch([REPO / "target/debug/examples/desktop-mock"], "fixture-restarted")
                wait(lambda: (root / "session/lconnectd.sock").exists() or fixture.poll() is not None,
                     "restarted fixture socket")
                assert fixture.poll() is None, (root / "fixture-restarted.log").read_text()
                wait(lambda: rpc("get_lcd_state", params)["source"] == "Theme", "saved theme replay")
                sensor_frame("restarted", "CPU live 203.0°F")
                gui = launch([binary], "gui-restarted")
                window = None
                wait(visible, "production GUI after restart")
                wait(rendered, "frontend after restart")
                click("Pump screen")
                click("Load active theme")
                wait(lambda: rendered() and "Production CPU sensor" in
                     (root / "main-accessibility.txt").read_text(), "saved theme loaded into editor")
                (root / "sensor-acceptance.json").write_text(json.dumps(dict(
                    phases=receipts, theme=theme, gui_closed_updates=True, replay_without_apply=True), indent=2) + "\n")
            if args.cooling:
                cooling_phases = []
                def pump_state(name, policy, duty, fallback=False, motherboard=False):
                    def accepted():
                        state = rpc("get_cooling_state", params)["pump"]
                        stats = json.loads((root / "session/stats.json").read_text())
                        packet = stats.get("last_pump_pwm_packet")
                        return (state["requested"] == policy and state["applied_percent"] == duty
                                and state["fallback_active"] == fallback and state["motherboard"] == motherboard
                                and state["last_error"] is None and packet is not None
                                and len(packet) == 64 and packet[:2] == [1, 138]
                                and packet[6:8] == [int(motherboard), duty])
                    wait(accepted, f"pump {name}: policy/state/raw cmd138")
                    state = rpc("get_cooling_state", params)
                    stats = json.loads((root / "session/stats.json").read_text())
                    assert state["fan"]["requested"] is None, "Pump editing changed the fan policy"
                    assert rpc("get_lcd_theme", params) == theme, "Cooling changed the LCD theme"
                    first = stats["jpeg_starts"]
                    wait(lambda: json.loads((root / "session/stats.json").read_text())["jpeg_starts"] > first,
                         "LCD frames continue during cooling")
                    if args.sensor_theme:
                        text = "CPU live —" if name in ["missing", "stale", "manual-override"] else "CPU live 203.0°F"
                        sensor_frame(f"cooling-{name}", text)
                    cooling_phases.append(dict(phase=name, state=state, stats=stats))

                click("Cooling")
                fixed = dict(mode="fixed", percent=61)
                field("PWM, %", "61")
                assert rpc("get_cooling_state", params)["pump"]["requested"] is None
                assert json.loads((root / "session/stats.json").read_text())["pump_pwm_writes"] == 0
                click("Apply cooling")
                pump_state("fixed", fixed, 61)
                field("Mode", "Temperature PWM curve", select=True)
                field("Temperature source", "Fixture CPU · valid", select=True)
                field("Fallback PWM, %", "87")
                (root / "session/cpu-hot").touch()
                curve = dict(mode="curve", sensor_id="fixture:cpu", hysteresis_c=2.0,
                             fallback_percent=87, points=[dict(celsius=30.0, percent=40), dict(celsius=70.0, percent=100)])
                assert rpc("get_cooling_state", params)["pump"]["requested"] == fixed, "Draft curve was applied"
                assert json.loads((root / "session/stats.json").read_text())["last_pump_pwm_packet"][6:8] == [0, 61]
                click("Apply cooling")
                pump_state("curve-hot", curve, 100)
                (root / "session/cpu-unavailable").touch()
                pump_state("missing", curve, 87, fallback=True)
                (root / "session/cpu-unavailable").unlink()
                pump_state("returned", curve, 100)
                (root / "session/freeze-sensors").touch()
                wait(lambda: any(s["id"] == "fixture:cpu" and s["status"] == "stale"
                                 for s in rpc("list_sensors")), "cooling sample monotonic age")
                pump_state("stale", curve, 87, fallback=True)
                field("Mode", "Fixed PWM", select=True)
                field("PWM, %", "61")
                click("Apply cooling")
                pump_state("manual-override", fixed, 61)
                (root / "session/freeze-sensors").unlink()
                wait(lambda: any(s["id"] == "fixture:cpu" and s["status"] == "valid"
                                 for s in rpc("list_sensors")), "fresh sample after manual override")
                pump_state("fresh-after-override", fixed, 61)
                # Fixed policies have a legitimate two-second refresh. Verify
                # that fresh samples cannot restore the old curve's output.
                assert json.loads((root / "session/stats.json").read_text())["last_pump_pwm_packet"][6:8] == [0, 61]
                field("Mode", "Motherboard source", select=True)
                field("PWM, %", "55")
                click("Apply cooling")
                pump_state("motherboard", dict(mode="motherboard", percent=55), 55, motherboard=True)
                field("Mode", "Temperature PWM curve", select=True)
                click("Apply cooling")
                pump_state("curve-before-restart", curve, 100)
                run(["xdotool", "windowquit", window], env=env)
                gui.wait(timeout=20)
                assert gui.returncode == 0
                pump_state("gui-closed", curve, 100)
                (root / "session/stop").touch()
                fixture.wait(timeout=15)
                assert fixture.returncode == 0
                for name in ["stop", "stats.json", "lcd-frame.json", "lconnectd.sock"]:
                    (root / "session" / name).unlink(missing_ok=True)
                env.pop("LCONNECT_NATIVE_RECOVERY_STATE", None)
                fixture = launch([REPO / "target/debug/examples/desktop-mock"], "fixture-cooling-restarted")
                wait(lambda: (root / "session/lconnectd.sock").exists() or fixture.poll() is not None,
                     "cooling replay fixture socket")
                assert fixture.poll() is None, (root / "fixture-cooling-restarted.log").read_text()
                wait(lambda: any(d["id"] == device["id"] for d in rpc("list_devices")), "replayed device discovery")
                pump_state("restarted", curve, 100)
                gui = launch([binary], "gui-cooling-restarted")
                window = None
                wait(visible, "production GUI cooling restart")
                wait(rendered, "cooling restart frontend")
                click("Cooling", expected=2)
                click("Load current policy")
                wait(lambda: rendered() and "Fallback PWM, %" in
                     (root / "main-accessibility.txt").read_text(), "saved curve loaded into editor")
                field("Mode", "Fixed PWM", select=True)
                field("PWM, %", "61")
                click("Apply cooling")
                pump_state("restore-fixed", fixed, 61)
                (root / "cooling-acceptance.json").write_text(json.dumps(dict(
                    phases=cooling_phases, no_fan_policy=True, replay_without_apply=True,
                    lcd_theme_unchanged=True), indent=2) + "\n")
                click("Pump screen", expected=1)
            click("Restore LocalUI")
            wait(lambda: rpc("get_lcd_state", params)["source"] == "Idle", "LocalUI restoration")
            wait(lambda: rendered() and "LocalUI requested. Automatic theme and video restoration is disabled." in
                 (root / "main-accessibility.txt").read_text(), "English LocalUI completion notice")
        else:
            assert stats["hid_write_count"] == 0
        run(["xdotool", "windowquit", window], env=env)
        gui.wait(timeout=20)
        assert gui.returncode == 0, f"Production close returned {gui.returncode}"
        assert "native close:" not in (root / "gui.log").read_text(), "Instrumented GUI was packaged"
        (root / "session/stop").touch()
        fixture.wait(timeout=15)
        assert fixture.returncode == 0
        (root / "acceptance.json").write_text(json.dumps(dict(ok=True, production_gui=True,
            instrumented=False, embedded_english_frontend=True, normal_close=True,
            mock_devices=int(args.with_device), actual_hardware_access=False,
            explicit_apply_and_localui=args.with_device, realtime_clock_frames=args.with_device and not args.sensor_theme,
            realtime_sensor_frames=args.sensor_theme,
            pump_cooling=args.cooling,
            worker_missing_channel_rejected=True), indent=2) + "\n")
    finally:
        (root / "session/stop").touch()
        for process in reversed(processes):
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait()
        for log in logs:
            log.close()


if __name__ == "__main__":
    main()
