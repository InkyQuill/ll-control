#!/usr/bin/env python3
"""Validate product versions and inherited workspace lock entries without writes."""
import argparse
import json
import tomllib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
root = parser.parse_args().root
workspace = tomllib.loads((root / "Cargo.toml").read_text())
version = workspace["workspace"]["package"]["version"]
assert (root / "version.txt").read_text().strip() == version
for name in ("desktop/package.json", "desktop/src-tauri/tauri.conf.json"):
    assert json.loads((root / name).read_text())["version"] == version, name
names = set()
for member in workspace["workspace"]["members"]:
    package = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
    assert package["version"] == {"workspace": True}, member
    names.add(package["name"])
local = [entry for entry in tomllib.loads((root / "Cargo.lock").read_text())["package"] if "source" not in entry]
assert {entry["name"] for entry in local} == names, "workspace lock entries"
assert all(entry["version"] == version for entry in local), "workspace lock versions"
print(f"Validated product version {version} and {len(local)} workspace lock entries")
