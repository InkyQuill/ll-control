#!/usr/bin/env python3
"""Production packaged GUI: private Xvfb/D-Bus/XDG, zero-device MockBackend.

Requires desktop-mock already built, xdotool, ImageMagick and Python GI/AT-SPI.
No native-smoke feature or JavaScript injection is used. Retains logs/screenshots.
"""
import argparse
import json
import os
import signal
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
    args = parser.parse_args()
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
        try:
            run(["xvfb-run", "-a", "-s", "-screen 0 1600x1200x24", "dbus-run-session", "--",
                 sys.executable, __file__, distribution, "--inside", root], env=env)
        finally:
            target = session / "doc"
            if any(row.split()[4] == str(target) for row in Path("/proc/self/mountinfo").read_text().splitlines()):
                run(["fusermount3", "-uz", "--", target])
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
    env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root / "session"), LCONNECT_NATIVE_SPEC="1",
               LCONNECT_NATIVE_RECOVERY_STATE="none", GDK_BACKEND="x11", GIO_USE_VFS="local",
               WEBKIT_DISABLE_DMABUF_RENDERER="1", LC_ALL="C.UTF-8",
               AT_SPI_BUS_ADDRESS=f"unix:path={root}/session/a11y.sock")
    env.pop("NO_AT_BRIDGE", None)
    (root / "session/fixture-marker").write_text("zero-device MockBackend; production GUI\n")
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
                                     "--name", "^L-Connect Linux$"], env=env, capture_output=True, text=True)
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
        assert stats["devices_known"] == 0 and stats["hid_write_count"] == 0
        run(["xdotool", "windowquit", window], env=env)
        gui.wait(timeout=20)
        assert gui.returncode == 0, f"Production close returned {gui.returncode}"
        assert "native close:" not in (root / "gui.log").read_text(), "Instrumented GUI was packaged"
        (root / "session/stop").touch()
        fixture.wait(timeout=15)
        assert fixture.returncode == 0
        (root / "acceptance.json").write_text(json.dumps(dict(ok=True, production_gui=True,
            instrumented=False, embedded_english_frontend=True, normal_close=True, hid_writes=0,
            worker_missing_channel_rejected=True), indent=2) + "\n")
    finally:
        (root / "session/stop").touch()
        for process in reversed(processes):
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
        for log in logs:
            log.close()


if __name__ == "__main__":
    main()
