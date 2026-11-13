#!/usr/bin/env python3
"""Measure the real optimized recording engine with synthetic sources, no GUI/HID.

Builds the lib test executable in release mode, or accepts an explicit prebuilt one.
Runs one explicitly ignored test, sequentially, with private temporary storage.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
TEST = "screen_record::resource_profile::profile_recording_engine_resources"
PHASE = "LLCONTROL_RECORD_PROFILE "
RESULT = "LLCONTROL_RECORD_PROFILE_RESULT "


def records(log, prefix):
    # libtest may prefix its first line with the test name.
    return [json.loads(line.split(prefix, 1)[1]) for line in log.splitlines(keepends=True) if prefix in line and line.endswith("\n")]


def sample(pid, start, phase):
    proc = Path(f"/proc/{pid}")
    status = dict(line.split(":", 1) for line in (proc / "status").read_text().splitlines())
    stat = (proc / "stat").read_text().rsplit(")", 1)[1].split()
    return dict(seconds=time.monotonic() - start, phase=phase,
                rss_kib=int(status.get("VmRSS", "0 kB").split()[0]),
                high_water_kib=int(status.get("VmHWM", "0 kB").split()[0]),
                threads=int(status["Threads"]), fds=len(list((proc / "fd").iterdir())),
                cpu_ticks=int(stat[11]) + int(stat[12]))


def summarize(samples, phase):
    selected = [s for s in samples if s["phase"] == phase and s["rss_kib"]]
    if not selected:
        return None
    first = selected[0]["seconds"]
    last = selected[-1]["seconds"]
    head = [s["rss_kib"] for s in selected if first + 3 <= s["seconds"] <= first + 8]
    tail = [s["rss_kib"] for s in selected if s["seconds"] >= last - 5]
    return dict(samples=len(selected), observed_seconds=last - first,
                peak_rss_mib=max(s["rss_kib"] for s in selected) / 1024,
                peak_threads=max(s["threads"] for s in selected),
                max_fds=max(s["fds"] for s in selected),
                mean_cpu_percent=(100 * (selected[-1]["cpu_ticks"] - selected[0]["cpu_ticks"]) /
                                  os.sysconf("SC_CLK_TCK") / (last - first)) if last > first else None,
                warm_to_tail_rss_mib=(statistics.median(tail) - statistics.median(head)) / 1024
                if head and tail else None)


def run_case(executable, root, dimensions, seconds):
    case = root / dimensions
    case.mkdir()
    staging = case / "staging"
    staging.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith("LLCONTROL_") and
           k not in {"DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS"}}
    env.update(TMPDIR=str(staging), XDG_RUNTIME_DIR=str(case),
               XDG_CONFIG_HOME=str(case / "config"), XDG_DATA_HOME=str(case / "data"),
               XDG_CACHE_HOME=str(case / "cache"),
               DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={case}/no-system-bus",
               LLCONTROL_RECORD_PROFILE_DIMENSIONS=dimensions,
               LLCONTROL_RECORD_PROFILE_SECONDS=str(seconds), LC_ALL="C.UTF-8")
    log_path = case / "engine.log"
    samples = []
    start = time.monotonic()
    with log_path.open("w") as log:
        child = subprocess.Popen([executable, TEST, "--exact", "--ignored", "--nocapture",
                                  "--test-threads=1"], stdout=log, stderr=subprocess.STDOUT,
                                 env=env, start_new_session=True, cwd=REPO)
        try:
            while child.poll() is None:
                if time.monotonic() - start > seconds + 90:
                    raise TimeoutError(f"Recording profile did not finish: {dimensions}")
                phases = records(log_path.read_text(), PHASE)
                try:
                    samples.append(sample(child.pid, start, phases[-1]["phase"] if phases else "startup"))
                except (FileNotFoundError, ProcessLookupError):
                    if child.poll() is None:
                        raise
                time.sleep(0.1)
            child.wait()
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            (case / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
    text = log_path.read_text()
    results = records(text, RESULT)
    report = dict(dimensions=dimensions, pid=child.pid, exit_code=child.returncode,
                  process_reaped=child.returncode is not None,
                  staging_empty=not list(staging.iterdir()),
                  phases=records(text, PHASE), result=results[-1] if results else None,
                  resources={p: summarize(samples, p) for p in
                             ["setup", "recording", "paused", "resumed", "finished", "released"]},
                  peak_rss_mib=max((s["high_water_kib"] for s in samples), default=0) / 1024)
    (case / "acceptance.json").write_text(json.dumps(report, indent=2) + "\n")
    assert child.returncode == 0 and results and report["staging_empty"], report
    print(f"PASS {dimensions}: peak RSS {report['peak_rss_mib']:.1f} MiB", flush=True)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-executable", type=Path, help="Use an explicit prebuilt release lib test executable")
    parser.add_argument("--seconds", type=int, default=30)
    parser.add_argument("--dimensions", choices=["320x240", "1920x1080", "3840x2160"],
                        nargs="+", default=["320x240", "1920x1080", "3840x2160"])
    args = parser.parse_args()
    if args.test_executable is None:
        build = subprocess.run(["cargo", "test", "--locked", "--release", "-p", "llcontrol-gui",
                                "--lib", "--no-run", "--message-format=json"],
                               cwd=REPO, check=True, stdout=subprocess.PIPE, text=True)
        artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
        matches = [Path(a["executable"]) for a in artifacts if a.get("reason") == "compiler-artifact"
                   and a.get("executable") and a["target"]["name"] == "llcontrol_gui"
                   and a.get("profile", {}).get("test")]
        assert len(matches) == 1, "Expected one release lib test artifact"
        args.test_executable = matches[0]
    executable = args.test_executable.resolve()
    assert executable.is_relative_to(REPO / "target/release/deps") and executable.is_file()
    assert 5 <= args.seconds <= 180
    listed = subprocess.run([executable, "--list"], check=True, capture_output=True, text=True).stdout
    assert f"{TEST}: test" in listed
    root = Path(tempfile.mkdtemp(prefix="llcontrol-recording-profile-"))
    root.chmod(0o700)
    print(f"Profile artifacts: {root}", flush=True)
    report = dict(scope="Synthetic release recording engine, no portal/GUI/physical HID",
                  kernel=platform.release(), machine=platform.machine(),
                  cpu_model=next((line.split(":", 1)[1].strip() for line in
                                  Path("/proc/cpuinfo").read_text().splitlines()
                                  if line.startswith("model name")), "unknown"),
                  cpu_count=os.cpu_count(), sample_interval_seconds=0.1,
                  cpu_unit="percent of one logical CPU; 100 means one occupied core",
                  excluded_resources="ffprobe decoder child and Tauri/portal/PipeWire overhead",
                  executable=str(executable), executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                  seconds_per_case=args.seconds, cases=[])
    try:
        for dimensions in args.dimensions:
            report["cases"].append(run_case(str(executable), root, dimensions, args.seconds))
    finally:
        (root / "acceptance.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
