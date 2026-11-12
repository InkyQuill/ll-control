#!/usr/bin/env python3
"""Check product version agreement; refresh only workspace Cargo.lock versions."""
import argparse
import json
import re
import tomllib
from pathlib import Path


def synchronize(root: Path, write: bool = False) -> bool:
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    version = workspace["workspace"]["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Release version must be stable SemVer")
    for name in ("desktop/package.json", "desktop/src-tauri/tauri.conf.json"):
        if json.loads((root / name).read_text())["version"] != version:
            raise ValueError(f"{name} differs from workspace version {version}")
    if (root / "version.txt").read_text().strip() != version:
        raise ValueError("version.txt differs from workspace version")
    names = set()
    for member in workspace["workspace"]["members"]:
        # Explicit members keep this validator independent of filesystem globbing.
        package = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
        if package["version"] != {"workspace": True}:
            raise ValueError(f"{member} must inherit the workspace version")
        names.add(package["name"])

    lock = root / "Cargo.lock"
    original = lock.read_text()
    seen = set()

    def update(match):
        block = match.group()
        package = tomllib.loads(block)["package"][0]
        if "source" in package or package["name"] not in names:
            return block
        if package["name"] in seen:
            raise ValueError(f"Duplicate local lock entry: {package['name']}")
        seen.add(package["name"])
        updated, count = re.subn(r'^version = "[^"\n]+"$', f'version = "{version}"', block, flags=re.M)
        if count != 1:
            raise ValueError("Expected one version in local Cargo.lock entry")
        return updated

    updated = re.sub(r"(?ms)^\[\[package\]\]\n.*?(?=^\[\[package\]\]|\Z)", update, original)
    if seen != names:
        raise ValueError(f"Missing workspace lock entries: {sorted(names - seen)}")
    changed = updated != original
    if changed and write:
        lock.write_text(updated)
    return changed


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    if synchronize(args.root, args.write) and not args.write:
        parser.exit(1, "Cargo.lock workspace versions need synchronization\n")
