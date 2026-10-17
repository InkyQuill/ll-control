#!/usr/bin/env python3
"""Signal actual owned recorder children in private native EN/RU acceptance.

No physical device, real PipeWire, host session or production daemon is used.
SIGSTOP is a process-stall fixture, not a specific vendor plugin deadlock.
Cleanup mode injects one frontend invoke rejection; it does not signal the child.
"""
import argparse
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
GUI = REPO / "target/debug/lconnect-gui"
WORKER_FLAG = b"--internal-screen-record-worker"


def run(args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, timeout=10, **kwargs)


def wait(predicate, label, seconds=20):
    deadline = time.monotonic() + seconds
    while not predicate():
        if time.monotonic() > deadline:
            raise TimeoutError(label)
        time.sleep(0.025)


def workers(parent):
    result = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            status = (entry / "status").read_text()
            if f"PPid:\t{parent}\n" not in status:
                continue
            args = (entry / "cmdline").read_bytes().split(b"\0")
            if len(args) > 1 and args[1] == WORKER_FLAG and (entry / "exe").resolve() == GUI.resolve():
                result.append(int(entry.name))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    return result


def rpc(root, method, params=None):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(5)
        connection.connect(str(root / "lconnectd.sock"))
        with connection.makefile("rwb") as stream:
            stream.write((json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params or {})) + "\n").encode())
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if reply.get("id") == 1:
                    assert "error" not in reply, reply
                    return reply["result"]


def inspect_process(pid):
    values = {}
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        key, _, value = line.partition(":")
        if key in {"VmRSS", "Threads", "State"}:
            values[key] = value.strip()
    values["fd_count"] = len(list(Path(f"/proc/{pid}/fd").iterdir()))
    return values


def case_run(root, mode, language):
    case = root / f"{mode}-{language}"
    case.mkdir(mode=0o700)
    (case / "fixture-marker").write_text("MockBackend/synthetic source; external recorder signal fixture\n")
    temporary = case / "native-tmp"
    temporary.mkdir()
    # Other acceptance selectors take precedence in the native-smoke builder.
    # A caller's prior test must not choose a different injected workflow.
    inherited = {key: value for key, value in os.environ.items() if not key.startswith("LCONNECT_NATIVE_")}
    env = dict(inherited, LCONNECT_SMOKE_ROOT=str(case), LCONNECT_NATIVE_RECORDING="1",
               LCONNECT_NATIVE_RECORDING_FAULTS="1", LCONNECT_NATIVE_MEDIA="1",
               LCONNECT_NATIVE_DESKTOP_PHASE=mode, XDG_RUNTIME_DIR=str(case),
               XDG_CONFIG_HOME=str(case / "config"), XDG_DATA_HOME=str(case / "data"),
               XDG_CACHE_HOME=str(case / "cache"), TMPDIR=str(temporary), GDK_BACKEND="x11",
               GIO_USE_VFS="local", WEBKIT_DISABLE_DMABUF_RENDERER="1", LC_ALL="C.UTF-8", NO_AT_BRIDGE="1")
    env.pop("WAYLAND_DISPLAY", None)
    processes, logs, signals = [], [], []
    signalled_pid = None
    signal_at = None
    close_at = None
    observed = set()

    def launch(command, name):
        log = (case / f"{name}.log").open("w")
        logs.append(log)
        child = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        processes.append(child)
        return child

    def window():
        return run(["xdotool", "search", "--onlyvisible", "--pid", gui.pid, "--name", "^L-Connect Linux$"],
                   env=env, capture_output=True, text=True).stdout.splitlines()[0]

    try:
        launch(["openbox"], "wm")
        fixture = launch([REPO / "target/debug/examples/desktop-mock"], "fixture")
        wait(lambda: (case / "lconnectd.sock").exists() and (case / "stats.json").exists()
             or fixture.poll() is not None, "private mock daemon")
        assert fixture.poll() is None
        settings = rpc(case, "get_display_settings")
        settings["interface_language"] = language
        rpc(case, "set_display_settings", {"settings": settings})
        wait(lambda: json.loads((case / "stats.json").read_text())["uptime_seconds"] >= 1, "mock baseline")
        baseline = json.loads((case / "stats.json").read_text())
        gui = launch([GUI], "gui")
        deadline = time.monotonic() + 100
        close_sent = False
        while gui.poll() is None:
            assert time.monotonic() < deadline, "native recording fault workflow deadline"
            step_file = case / "media-step.json"
            if step_file.exists():
                step = json.loads(step_file.read_text())
                if step["sequence"] not in observed:
                    observed.add(step["sequence"])
                    action = step["action"]
                    if action == "recording_" + ("cancel" if mode.startswith("cleanup") else mode):
                        children = workers(gui.pid)
                        assert len(children) == 1, children
                        signalled_pid = children[0]
                        before_signal = inspect_process(signalled_pid)
                        assert "t" not in before_signal["State"].split()[0].lower(), before_signal
                        if not mode.startswith("cleanup"):
                            os.kill(signalled_pid, signal.SIGKILL if mode == "crash" else signal.SIGSTOP)
                        if mode not in {"crash", "cleanup", "cleanup-review", "cleanup-review-lost"}:
                            wait(lambda: "T" in inspect_process(signalled_pid)["State"], "worker actually stopped", 2)
                        signal_at = time.monotonic()
                        signals.append(dict(action=action, pid=signalled_pid, before_signal=before_signal))
                    elif action in {"recording_reaped", "recording_recovered"}:
                        assert signalled_pid is not None and not Path(f"/proc/{signalled_pid}").exists(), "faulted worker not reaped"
                        assert not workers(gui.pid), "recording child remains"
                        assert not list(temporary.iterdir()), "staging remains after worker teardown"
                        elapsed = time.monotonic() - signal_at
                        if action == "recording_reaped":
                            assert elapsed < (30 if mode.startswith("cleanup-review") else 8 if mode == "stall" else 5), elapsed
                            run(["import", "-window", window(), case / "recovery.png"], env=env)
                        signals.append(dict(action=action, elapsed_since_signal=elapsed, no_children=True, staging_empty=True))
                    else:
                        raise AssertionError(f"Unexpected fixture step: {step}")
                    (case / "media-ack").write_text(str(step["sequence"]))
            result_file = case / "result.json"
            if result_file.exists() and not close_sent:
                try:
                    result = json.loads(result_file.read_text())
                except json.JSONDecodeError:
                    # native_smoke_result writes a small file without rename;
                    # an observer can see it between create and write.
                    time.sleep(0.025)
                    continue
                assert result["ok"], result
                run(["import", "-window", window(), case / "final.png"], env=env)
                close_at = time.monotonic()
                run(["xdotool", "windowquit", window()], env=env)
                close_sent = True
            if close_sent:
                assert time.monotonic() - close_at < 10, "GUI close stalled"
            time.sleep(0.025)
        gui.wait(timeout=5)
        assert gui.returncode == 0, (case / "result.json").read_text() if (case / "result.json").exists() else (case / "gui.log").read_text()
        result = json.loads((case / "result.json").read_text())
        assert result["ok"] and close_sent, result
        assert not Path(f"/proc/{signalled_pid}").exists() and not workers(gui.pid)
        assert not list(temporary.iterdir()), "staging survived GUI close"
        assert "native close: all workers joined" in (case / "gui.log").read_text()
        after = json.loads((case / "stats.json").read_text())
        # hid_write_count includes normal sensor requests; it is not a count
        # of mutations. Capture must not send media, PWM, RGB or firmware.
        for key in ["fan_pwm_writes", "pump_rgb_writes", "jpeg_starts", "lcd_video_packets",
                    "firmware_update_writes", "firmware_activations"]:
            assert after[key] == baseline[key], (key, baseline[key], after[key])
        if mode != "close":
            assert result["saved_key"] in [clip["key"] for clip in rpc(case, "list_prepared_clips")]
        result.update(signals=signals, normal_close=True, close_seconds=time.monotonic()-close_at,
                      all_workers_reaped=True, staging_empty=True, mock_control_writes_unchanged=True,
                      mock_hid_requests_delta=after["hid_write_count"]-baseline["hid_write_count"],
                      actual_hardware_access=False)
        (case / "acceptance.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        return result
    finally:
        (case / "stop").touch()
        # Prefer the fixture's joined shutdown; never signal a host daemon.
        if len(processes) > 1 and processes[1].poll() is None:
            try:
                processes[1].wait(timeout=10)
            except subprocess.TimeoutExpired:
                pass
        for child in reversed(processes):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
        for log in logs:
            log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["stall", "crash", "cancel", "close", "cleanup", "cleanup-review", "cleanup-review-lost"], action="append")
    parser.add_argument("--language", choices=["en-US", "ru-RU"], action="append")
    parser.add_argument("--inside", type=Path)
    args = parser.parse_args()
    if args.inside is None:
        root = Path(tempfile.mkdtemp(prefix="lconnect-recording-faults-"))
        runtime = root / "session"
        runtime.mkdir(mode=0o700)
        env = dict(os.environ, LCONNECT_RECORDING_FAULTS_ISOLATED="1", XDG_RUNTIME_DIR=str(runtime),
                   XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
                   XDG_CACHE_HOME=str(root / "cache"), DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={root}/no-system-bus")
        for name in ["DISPLAY", "WAYLAND_DISPLAY", "AT_SPI_BUS_ADDRESS", "DBUS_SESSION_BUS_ADDRESS"]:
            env.pop(name, None)
        try:
            result = subprocess.call(["xvfb-run", "-a", "-s", "-screen 0 1600x1200x24", "dbus-run-session", "--",
                sys.executable, __file__, *sys.argv[1:], "--inside", str(root)], env=env)
        finally:
            target = runtime / "doc"
            if any(row.split()[4] == str(target) for row in Path("/proc/self/mountinfo").read_text().splitlines()):
                run(["fusermount3", "-uz", "--", target])
        print("Native recording fault artifacts:", root)
        raise SystemExit(result)
    assert os.environ.get("LCONNECT_RECORDING_FAULTS_ISOLATED") == "1"
    results = []
    for mode in args.mode or ["stall", "crash", "cancel", "close", "cleanup", "cleanup-review", "cleanup-review-lost"]:
        for language in args.language or ["en-US", "ru-RU"]:
            result = case_run(args.inside, mode, language)
            results.append(result)
            print("PASS", mode, language, flush=True)
    (args.inside / "acceptance.json").write_text(json.dumps(dict(ok=True, cases=results), ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
