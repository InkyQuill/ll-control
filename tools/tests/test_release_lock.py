"""Release lock refresh preserves third-party resolutions and fails on drift."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("release_lock", Path(__file__).parents[1] / "sync-release-lock.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReleaseLockTests(unittest.TestCase):
    def fixture(self, root, version):
        (root / "Cargo.toml").write_text('[workspace]\nmembers = ["crate"]\n[workspace.package]\nversion = "' + version + '"\n')
        (root / "crate").mkdir(exist_ok=True)
        (root / "crate/Cargo.toml").write_text('[package]\nname = "local"\nversion.workspace = true\n')
        for file in ("desktop/package.json", "desktop/src-tauri/tauri.conf.json"):
            path = root / file
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"version": version}))
        (root / "version.txt").write_text(version + "\n")

    def test_first_and_subsequent_release_only_modify_local_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            third_party = '\n[[package]]\nname = "remote"\nversion = "0.1.0"\nsource = "registry+example"\nchecksum = "unchanged"\n'
            original = '# generated\nversion = 4\n\n[[package]]\nname = "local"\nversion = "0.1.0"\ndependencies = ["remote"]\n' + third_party
            (root / "Cargo.lock").write_text(original)
            for version in ("0.1.0", "0.1.1", "0.2.0", "0.3.0"):
                self.fixture(root, version)
                MODULE.synchronize(root, write=True)
                self.assertEqual((root / "Cargo.lock").read_text(), original.replace('name = "local"\nversion = "0.1.0"', f'name = "local"\nversion = "{version}"'))
                self.assertFalse(MODULE.synchronize(root))

    def test_version_drift_fails_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, "0.2.0")
            (root / "desktop/package.json").write_text('{"version":"0.1.0"}')
            with self.assertRaisesRegex(ValueError, "differs"):
                MODULE.synchronize(root, write=True)

    def test_missing_local_entry_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, "0.2.0")
            (root / "Cargo.lock").write_text("version = 4\n")
            with self.assertRaisesRegex(ValueError, "Missing workspace"):
                MODULE.synchronize(root, write=True)
