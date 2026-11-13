#!/usr/bin/env python3
"""Build a production Linux/Arch payload; never install or start anything.

Python >=3.11, a provisioned build host and frozen frontend dependencies required.
Output must not exist. The generated PKGBUILD consumes only its adjacent archive.
"""
import argparse
import gzip
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
BINARIES = {"lconnectd", "lconnectctl", "lconnect-gui"}
SKIP = {"node_modules", ".svelte-kit", "build", "gen", "__pycache__",
        "test-results", "playwright-report"}


def run(args, **kwargs):
    return subprocess.run([str(arg) for arg in args], cwd=REPO, check=True, **kwargs)


def output(args):
    return run(args, stdout=subprocess.PIPE, text=True).stdout.strip()


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source_inputs():
    """Include dirty/untracked build inputs, independent of git staging."""
    paths = []
    for name in ["crates", "desktop", "integrations", "packaging", "xtask"]:
        for base, dirs, files in os.walk(REPO / name):
            dirs[:] = sorted(d for d in dirs if d not in SKIP)
            paths.extend(Path(base) / file for file in files)
    paths.extend(REPO / name for name in ["Cargo.toml", "Cargo.lock", "LICENSE",
                                          "tools/build-distribution.py", "tools/test-distribution.py",
                                          "tools/test-distribution-gui.py", "tools/test-distribution-mini.py",
                                          "tools/tests/test_distribution.py",
                                          ".github/workflows/linux.yml"])
    return {p.relative_to(REPO).as_posix(): digest(p) for p in sorted(paths) if p.is_file()}


def production_artifacts(messages):
    artifacts = {}
    for message in messages:
        if message.get("reason") != "compiler-artifact":
            continue
        name = message["target"]["name"]
        if name not in BINARIES or not message.get("executable"):
            continue
        features = message["features"]
        if "native-smoke" in features:
            raise ValueError("Refusing native-smoke artifact")
        if name == "lconnect-gui" and "custom-protocol" not in features:
            raise ValueError("GUI must embed the production frontend")
        if not message["profile"]["opt_level"] == "3" or message["profile"]["test"]:
            raise ValueError("Expected release binary")
        artifacts[name] = Path(message["executable"])
    if artifacts.keys() != BINARIES:
        raise ValueError(f"Missing production artifacts: {BINARIES - artifacts.keys()}")
    return artifacts


def require_device_ownership(rule):
    """The installed rule must grant the active session the Galahad hidraw node.

    udev ignores an entire rule when GROUP names a group the system does not
    have. The package does not create plugdev, so a grouped rule never
    registers device ownership on a clean Arch install.
    """
    required = (
        'SUBSYSTEM=="hidraw"',
        'ATTRS{idVendor}=="0416"',
        'ATTRS{idProduct}=="7395"',
        'TAG+="uaccess"',
        'MODE="0660"',
    )
    # udev continuations form one rule; comments and separate rules cannot
    # supply matches or assignments to the device rule.
    active_lines = [line.strip() for line in rule.splitlines()
                    if line.strip() and not line.lstrip().startswith("#")]
    logical_rules = "\n".join(active_lines).replace("\\\n", " ").splitlines()
    if not any(all(token in line for token in required) and "GROUP=" not in line
               for line in logical_rules):
        raise ValueError(
            "udev rule does not register Galahad II LCDV2 for the active session"
        )


def copy(source, stage, destination, mode=0o644):
    target = stage / destination
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    target.chmod(mode)


def archive_payload(stage, destination, epoch):
    """Stable paths, timestamps, ownership and gzip header for identical payloads."""
    with destination.open("wb") as raw:
        with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for path in [stage, *sorted(stage.rglob("*"))]:
                    if path.is_symlink() or not (path.is_dir() or path.is_file()):
                        raise ValueError(f"Unexpected staged file type: {path}")
                    info = archive.gettarinfo(str(path), "payload/" + path.relative_to(stage).as_posix())
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    info.mtime = epoch
                    info.mode = 0o755 if path.is_dir() else path.stat().st_mode & 0o777
                    if path.is_file():
                        with path.open("rb") as stream:
                            archive.addfile(info, stream)
                    else:
                        archive.addfile(info)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--gnome-shell-version", action="append", required=True,
                        help="Explicit candidate versions; not a qualification claim")
    args = parser.parse_args()
    destination = args.output.absolute()
    if destination.exists() or destination.is_symlink():
        parser.error("Output already exists; preserved")
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("This Arch payload currently targets Linux x86_64 only")
    if any(not v.isdecimal() or not 45 <= int(v) <= 999 for v in args.gnome_shell_version):
        parser.error("GNOME candidate versions must be integers >=45")
    version = tomllib.loads((REPO / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    for file in ["desktop/package.json", "desktop/src-tauri/tauri.conf.json"]:
        if json.loads((REPO / file).read_text())["version"] != version:
            parser.error(f"Version mismatch: {file}")
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", output(["git", "show", "-s", "--format=%ct", "HEAD"])))
    if not 0 <= epoch <= 0xFFFFFFFF:
        parser.error("SOURCE_DATE_EPOCH must fit unsigned 32 bits")
    before = source_inputs()
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Work beside output for atomic publication, never in a system install root.
    with tempfile.TemporaryDirectory(prefix=".lconnect-dist-", dir=destination.parent) as temporary:
        work = Path(temporary)
        stage = work / "payload"
        stage.mkdir()
        run(["pnpm", "--dir", "desktop", "build"])
        command = ["cargo", "build", "--locked", "--release", "-p", "lconnect-daemon",
                   "-p", "lconnect-cli", "-p", "lconnect-gui", "--bins", "--no-default-features",
                   "--features", "lconnect-gui/custom-protocol", "--message-format=json-render-diagnostics"]
        # Cargo diagnostics go to stderr; JSON artifacts go to a bounded disk log.
        cargo_log = work / "cargo-artifacts.jsonl"
        with cargo_log.open("w") as log:
            run(command, stdout=log)
        with cargo_log.open() as log:
            artifacts = production_artifacts(json.loads(line) for line in log)
        for name, source in artifacts.items():
            if source.read_bytes()[:4] != b"\x7fELF":
                raise ValueError(f"Expected ELF: {name}")
            copy(source, stage, f"usr/bin/{name}", 0o755)
        unit = (REPO / "packaging/systemd/lconnectd.service").read_text()
        old = "ExecStart=%h/.local/bin/lconnectd"
        if unit.count(old) != 1:
            raise ValueError("Unexpected manual service ExecStart")
        unit_path = work / "lconnectd.service"
        unit_path.write_text(unit.replace(old, "ExecStart=/usr/bin/lconnectd"))
        copy(unit_path, stage, "usr/lib/systemd/user/lconnectd.service")
        udev_rule = (REPO / "packaging/udev/70-lianli-hid.rules").read_text()
        require_device_ownership(udev_rule)
        udev_path = work / "70-lianli-hid.rules"
        udev_path.write_text(udev_rule)
        copy(udev_path, stage, "usr/lib/udev/rules.d/70-lianli-hid.rules")
        copy(REPO / "packaging/net.inkyquill.lconnect.desktop", stage,
             "usr/share/applications/net.inkyquill.lconnect.desktop")
        copy(REPO / "desktop/src-tauri/icons/icon.png", stage,
             "usr/share/icons/hicolor/256x256/apps/ll-control.png")
        copy(REPO / "desktop/src-tauri/icons/32x32.png", stage,
             "usr/share/icons/hicolor/32x32/apps/ll-control.png")
        copy(REPO / "desktop/static/ll-control.svg", stage,
             "usr/share/icons/hicolor/scalable/apps/ll-control.svg")
        copy(REPO / "LICENSE", stage, "usr/share/licenses/lconnect-linux/LICENSE")
        copy(REPO / "desktop/static/fonts/OFL.txt", stage,
             "usr/share/licenses/lconnect-linux/fonts/Exo2-OFL.txt")
        for source in sorted((REPO / "crates/lconnect-image/assets").glob("*.txt")):
            copy(source, stage, f"usr/share/licenses/lconnect-linux/fonts/{source.name}")
        # Both integrations are installed system-wide. Each shell discovers only
        # its own assets; no active-desktop detection or user-session modification.
        plasma = work / "plasma"
        run(["cmake", "-S", "integrations/plasma", "-B", plasma,
             "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_INSTALL_PREFIX=/usr",
             "-DCMAKE_INSTALL_LIBDIR=lib", "-DBUILD_TESTING=OFF"])
        run(["cmake", "--build", plasma, "--parallel", "4"])
        run(["cmake", "--install", plasma], env=dict(os.environ, DESTDIR=str(stage)))
        gnome = work / "gnome"
        run(["python3", "integrations/gnome/build.py", "--output", gnome,
             *[arg for value in args.gnome_shell_version for arg in ["--shell-version", value]]])
        extension = gnome / "lconnect-mini@inkyquill.net"
        (extension / ".lconnect-mini-build").unlink()
        shutil.copytree(extension, stage / "usr/share/gnome-shell/extensions/lconnect-mini@inkyquill.net")
        if source_inputs() != before:
            raise ValueError("Source inputs changed during build; output not published")
        for path in stage.rglob("*"):
            if path.is_dir():
                path.chmod(0o755)
            elif path.suffix != ".so" and path.parent != stage / "usr/bin":
                path.chmod(0o644)
        files = {p.relative_to(stage).as_posix(): {"sha256": digest(p), "mode": oct(p.stat().st_mode & 0o777)}
                 for p in sorted(stage.rglob("*")) if p.is_file()}
        manifest = {"schema": 1, "version": version, "platform": "linux-x86_64-arch",
                    "source_revision": output(["git", "rev-parse", "HEAD"]),
                    "source_dirty": bool(output(["git", "status", "--porcelain"])),
                    "source_date_epoch": epoch, "source_inputs": before,
                    "tools": {name: output(cmd) for name, cmd in {
                        "rustc": ["rustc", "--version"], "node": ["node", "--version"],
                        "pnpm": ["pnpm", "--version"],
                        "qt_qml": ["pkg-config", "--modversion", "Qt6Qml"],
                        "gstreamer": ["pkg-config", "--modversion", "gstreamer-1.0"]}.items()},
                    "gui_features": ["custom-protocol"],
                    "gnome_candidate_versions": args.gnome_shell_version, "files": files}
        manifest_file = work / "manifest.json"
        manifest_file.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        copy(manifest_file, stage, "usr/share/doc/lconnect-linux/manifest.json")
        archive = work / f"lconnect-linux-{version}-x86_64.tar.gz"
        archive_payload(stage, archive, epoch)
        recipe = (REPO / "packaging/arch/PKGBUILD.in").read_text()
        for key, value in {"VERSION": version, "ARCHIVE": archive.name, "SHA256": digest(archive)}.items():
            recipe = recipe.replace(f"@{key}@", value)
        (work / "PKGBUILD").write_text(recipe)
        published = work / "published"
        published.mkdir()
        for source in [archive, manifest_file, work / "PKGBUILD"]:
            shutil.copyfile(source, published / source.name)
        (published / "SHA256SUMS").write_text("".join(
            f"{digest(p)}  {p.name}\n" for p in sorted(published.iterdir())))
        # mkdir is exclusive: a concurrently created destination is preserved.
        destination.mkdir()
        for path in published.iterdir():
            os.replace(path, destination / path.name)
    print(f"Distribution: {destination}")


if __name__ == "__main__":
    main()
