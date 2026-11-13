#!/usr/bin/env python3
"""Qualify inherited PipeWire FD with a private server and synthetic video only.

No portal, compositor, physical device or session manager is started. Artifacts
are retained under /tmp; every process started by the wrapper is joined.
"""
import argparse
import hashlib
import platform
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
TEST = "screen_record_process::pipewire_tests::selected_pipewire_fd_records_real_plugin_frames"
SERVER = """context.properties = { core.daemon = true core.name = llcontrol-private support.dbus = false }
context.spa-libs = { support.* = support/libspa-support }
context.modules = [
 { name = libpipewire-module-protocol-native }
 { name = libpipewire-module-spa-node-factory }
 { name = libpipewire-module-client-node }
 { name = libpipewire-module-link-factory }
 { name = libpipewire-module-access }
]
context.objects = [
 { factory = spa-node-factory args = { factory.name = support.node.driver node.name = Dummy-Driver node.group = pipewire.dummy priority.driver = 20000 } }
]
"""
CLIENT = """context.properties = { support.dbus = false }
context.spa-libs = { support.* = support/libspa-support video.convert.* = videoconvert/libspa-videoconvert }
context.modules = [
 { name = libpipewire-module-protocol-native }
 { name = libpipewire-module-client-node }
 { name = libpipewire-module-adapter }
]
"""


def stop(child):
    if child.poll() is None:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-executable", type=Path)
    parser.add_argument("--disconnect-phase", choices=["setup", "recording", "paused"], default="recording")
    args = parser.parse_args()
    if args.test_executable is None:
        built = subprocess.run(["cargo", "test", "--locked", "-p", "llcontrol-gui", "--lib", "--no-run",
                                "--message-format=json"], cwd=REPO, check=True, stdout=subprocess.PIPE, text=True)
        artifacts = [json.loads(line) for line in built.stdout.splitlines() if line.startswith("{")]
        matches = [Path(a["executable"]) for a in artifacts if a.get("reason") == "compiler-artifact"
                   and a.get("executable") and a["target"]["name"] == "llcontrol_gui"
                   and a.get("profile", {}).get("test")]
        assert len(matches) == 1
        args.test_executable = matches[0]
    executable = args.test_executable.resolve()
    assert executable.is_file() and executable.is_relative_to(REPO / "target")
    listed = subprocess.run([str(executable), "--list"], check=True, capture_output=True, text=True).stdout
    assert f"{TEST}: test" in listed, "test executable does not contain this qualification"
    root = Path(tempfile.mkdtemp(prefix="llcontrol-private-pipewire-"))
    root.chmod(0o700)
    print(f"PipeWire artifacts: {root}", flush=True)
    (root / "pipewire.conf").write_text(SERVER)
    (root / "client.conf").write_text(CLIENT)
    (root / "fixture-marker").write_text("synthetic-only\n")
    staging = root / "staging"
    staging.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith(("LLCONTROL_", "PIPEWIRE_"))
           and k not in {"DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS"}}
    env.update(XDG_RUNTIME_DIR=str(root), PIPEWIRE_RUNTIME_DIR=str(root), PIPEWIRE_CONFIG_DIR=str(root),
               PIPEWIRE_REMOTE="llcontrol-private", XDG_CONFIG_HOME=str(root / "config"),
               XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"), TMPDIR=str(staging),
               DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={root}/no-system-bus")

    def graph():
        result = subprocess.run(["pw-dump"], env=env, capture_output=True, text=True, timeout=5, check=True)
        objects = json.loads(result.stdout)
        assert not any(o["type"].endswith(":Device") for o in objects), "unexpected physical device"
        return objects

    with (root / "server.log").open("w") as server_log, (root / "recorder.log").open("w") as test_log:
        server = subprocess.Popen(["pipewire", "-c", str(root / "pipewire.conf")], env=env,
                                  stdout=server_log, stderr=subprocess.STDOUT, start_new_session=True)
        recorder = None
        producer = None
        linked = False
        links = []
        try:
            deadline = time.monotonic() + 5
            while not (root / "llcontrol-private").exists():
                assert server.poll() is None and time.monotonic() < deadline
                time.sleep(.05)
            producer_log = (root / "producer.log").open("w")
            producer = subprocess.Popen(["gst-launch-1.0", "-q", "videotestsrc", "is-live=true", "pattern=smpte", "!",
                "video/x-raw,format=RGB,width=320,height=240,framerate=24/1", "!", "pipewiresink", "mode=provide",
                "stream-properties=properties,node.name=llcontrol-pattern,media.class=Video/Source"], env=env,
                stdout=producer_log, stderr=subprocess.STDOUT, start_new_session=True)
            deadline = time.monotonic() + 5
            while True:
                initial = graph()
                if any(o.get("info", {}).get("props", {}).get("node.name") == "llcontrol-pattern" for o in initial):
                    break
                assert producer.poll() is None and time.monotonic() < deadline, "source unavailable"
                time.sleep(.05)
            (root / "initial-graph.json").write_text(json.dumps(initial, indent=2))
            source = next(o for o in initial if o.get("info", {}).get("props", {}).get("node.name") == "llcontrol-pattern")
            serial = source["info"]["props"]["object.serial"]
            test_env = env | dict(LLCONTROL_PRIVATE_PIPEWIRE=str(root), LLCONTROL_PRIVATE_NODE=str(2**32 - 2),
                                  LLCONTROL_PRIVATE_SERIAL=str(serial), LLCONTROL_PRIVATE_LOSS_PHASE=args.disconnect_phase, PIPEWIRE_REMOTE="nonexistent-recorder-remote")
            recorder = subprocess.Popen([str(executable), TEST, "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                                        env=test_env, cwd=REPO, stdout=test_log, stderr=subprocess.STDOUT, start_new_session=True)
            deadline = time.monotonic() + 30
            while recorder.poll() is None:
                assert time.monotonic() < deadline, "private recording timeout"
                if (root / "disconnect-server").exists():
                    stop(server)
                    time.sleep(.05)
                    continue
                objects = graph()
                inputs = [o for o in objects if o["type"].endswith(":Node")
                          and o.get("info", {}).get("props", {}).get("media.class") == "Stream/Input/Video"]
                if inputs:
                    assert len(inputs) == 1
                    sink = inputs[0]
                    assert str(sink["info"]["props"]["target.object"]) == str(serial)
                    outputs = [o for o in objects if o["type"].endswith(":Port") and o["info"]["props"].get("node.id") == source["id"]]
                    ports = [o for o in objects if o["type"].endswith(":Port") and o["info"]["props"].get("node.id") == sink["id"]]
                    existing = [o for o in objects if o["type"].endswith(":Link")
                                and o["info"]["output-node-id"] == source["id"]
                                and o["info"]["input-node-id"] == sink["id"]]
                    if outputs and ports and not existing:
                        assert len(outputs) == len(ports) == 1
                        link = subprocess.run(["pw-link", str(outputs[0]["id"]), str(ports[0]["id"])], env=env, capture_output=True, text=True, timeout=5)
                        (root / "link.log").write_text(link.stdout + link.stderr)
                        assert link.returncode == 0, link.stderr
                        linked = True
                        links.append(dict(source_node=source["id"], source_serial=serial,
                                          sink_node=sink["id"], sink_serial=sink["info"]["props"]["object.serial"]))
                        (root / "linked-graph.json").write_text(json.dumps(graph(), indent=2))
                time.sleep(.05)
            recorder.wait()
            assert recorder.returncode == 0 and linked, (root / "recorder.log").read_text()
            assert not list(staging.iterdir()), "retained recording staging"
            print("PASS real pipewiresrc: inherited FD, serial target property, crop, pause/resume, decode, remote loss, cleanup", flush=True)
        finally:
            if recorder is not None:
                stop(recorder)
            if producer is not None:
                stop(producer)
                producer_log.close()
            stop(server)
            (root / "acceptance.json").write_text(json.dumps(dict(
                scope="private PipeWire and real GStreamer plugins, synthetic source, no desktop portal or HID",
                linked=linked, links=links, disconnect_phase=args.disconnect_phase, kernel=platform.release(),
                executable=str(executable), executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                pipewire_version=subprocess.run(["pipewire", "--version"], capture_output=True, text=True, check=True).stdout,
                gstreamer_version=subprocess.run(["gst-launch-1.0", "--version"], capture_output=True, text=True, check=True).stdout,
                server_pid=server.pid, server_exit=server.returncode,
                producer_pid=producer.pid if producer else None,
                producer_exit=producer.returncode if producer else None,
                recorder_pid=recorder.pid if recorder else None,
                recorder_exit=recorder.returncode if recorder else None,
                staging_empty=not list(staging.iterdir())), indent=2) + "\n")


if __name__ == "__main__":
    main()
