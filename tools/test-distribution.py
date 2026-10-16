#!/usr/bin/env python3
"""Consume the real Arch artifact in a private pacman root; never use host DB/hooks.

No daemon, GUI, HID, service enable, or user configuration mutation. The private
database intentionally omits dependency resolution: host ABI is checked via ldd.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path


def run(args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify(root, manifest):
    for name, info in manifest["files"].items():
        path = root / name
        assert path.is_file() and not path.is_symlink(), name
        assert sha(path) == info["sha256"], name
        assert oct(path.stat().st_mode & 0o777) == info["mode"], name
    # System-wide Mini assets and its native plugin must survive both transactions.
    assert (root / "usr/share/plasma/plasmoids/net.inkyquill.lconnect.mini/metadata.json").is_file()
    assert (root / "usr/lib/qt6/qml/net/inkyquill/lconnect/qmldir").is_file()
    assert list((root / "usr/lib/qt6/qml/net/inkyquill/lconnect").glob("*.so"))
    extension = root / "usr/share/gnome-shell/extensions/lconnect-mini@inkyquill.net"
    assert json.loads((extension / "metadata.json").read_text())["shell-version"]
    assert (extension / "schemas/gschemas.compiled").is_file()
    assert "ExecStart=/usr/bin/lconnectd" in (root / "usr/lib/systemd/user/lconnectd.service").read_text()
    assert not (root / "usr/bin/verify-galahad").exists()


def transactions(work, first, second, manifest):
    root = work / "root"
    root.mkdir()
    assert root.is_absolute() and root != Path("/") and not root.is_symlink()
    for relative in ["db/local", "cache", "hooks", "gnupg"]:
        (work / relative).mkdir(parents=True)
    config = work / "pacman.conf"
    config.write_text("[options]\nArchitecture = x86_64\nSigLevel = Never\n")
    # Every pacman filesystem input/output points at this random private tree.
    base = ["pacman", "--root", root, "--dbpath", work / "db", "--cachedir", work / "cache",
            "--config", config, "--logfile", work / "pacman.log", "--gpgdir", work / "gnupg",
            "--hookdir", work / "hooks", "--noconfirm"]
    for package in [first, second]:
        run([*base, "--noscriptlet", "-Udd", package])
        verify(root, manifest)
        result = run([root / "usr/bin/lconnectctl", "--version"], capture_output=True, text=True)
        assert manifest["version"] in result.stdout
        for binary in ["lconnectctl", "lconnectd", "lconnect-gui"]:
            result = run(["ldd", root / "usr/bin" / binary], capture_output=True, text=True)
            assert "not found" not in result.stdout, result.stdout
    installed = run([*base, "-Q", "lconnect-linux"], capture_output=True, text=True).stdout
    assert installed.strip().endswith("-2"), installed
    # Simulated existing intent lives outside the package-owned paths.
    state = root / "home/test/.config/lconnect/sentinel"
    state.parent.mkdir(parents=True)
    state.write_text("existing user intent\n")
    run([*base, "--noscriptlet", "-Rdd", "lconnect-linux"])
    assert state.read_text() == "existing user intent\n"
    assert not any((root / name).exists() for name in manifest["files"])
    assert not (root / "usr/share/doc/lconnect-linux/manifest.json").exists()
    print("PASS private pacman install -> upgrade -1 to -2 -> uninstall; both Mini assets; user intent retained")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("distribution", type=Path)
    parser.add_argument("--transactions", type=Path)
    args = parser.parse_args()
    distribution = args.distribution.resolve()
    manifest = json.loads((distribution / "manifest.json").read_text())
    packages = list(distribution.glob("*.pkg.tar.zst"))
    assert len(packages) == 1, "Build the first package with makepkg before testing"
    first = packages[0]
    if args.transactions:
        second = next((args.transactions / "upgrade").glob("*.pkg.tar.zst"))
        transactions(args.transactions, first, second, manifest)
        return
    # Verify checksums before extraction and repack the *real* payload twice.
    for line in (distribution / "SHA256SUMS").read_text().splitlines():
        expected, name = line.split("  ", 1)
        assert Path(name).name == name and sha(distribution / name) == expected
    spec = importlib.util.spec_from_file_location("dist", Path(__file__).with_name("build-distribution.py"))
    dist = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(dist)
    with tempfile.TemporaryDirectory(prefix="lconnect-package-acceptance-") as temporary:
        work = Path(temporary)
        archive = next(distribution.glob("*.tar.gz"))
        with tarfile.open(archive) as tar:
            tar.extractall(work / "extract", filter="data")
        verify(work / "extract/payload", manifest)
        rebuilt = work / "rebuilt.tar.gz"
        dist.archive_payload(work / "extract/payload", rebuilt, manifest["source_date_epoch"])
        assert sha(rebuilt) == sha(archive), "Archive metadata is not reproducible"
        upgrade = work / "upgrade"
        upgrade.mkdir()
        shutil.copyfile(archive, upgrade / archive.name)
        recipe = (distribution / "PKGBUILD").read_text()
        assert recipe.count("pkgrel=1\n") == 1
        (upgrade / "PKGBUILD").write_text(recipe.replace("pkgrel=1\n", "pkgrel=2\n"))
        command = ["makepkg", "--nodeps", "--noconfirm"]
        if os.geteuid() == 0:
            # CI provisions this unprivileged build account; no host user creation here.
            import pwd
            account = pwd.getpwnam("builder")
            work.chmod(0o755)
            for path in [upgrade, *upgrade.rglob("*")]:
                os.chown(path, account.pw_uid, account.pw_gid)
            command = ["runuser", "-u", "builder", "--", *command]
        run(command, cwd=upgrade)
        command = [sys.executable, Path(__file__).resolve(), distribution, "--transactions", work]
        if os.geteuid() != 0:
            command = ["fakeroot", *command]
        run(command)


if __name__ == "__main__":
    main()
