"""Production feature gate and deterministic archive consumer contracts."""
import importlib.util
import os
import tarfile
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("distribution", Path(__file__).parents[1] / "build-distribution.py")
dist = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dist)


def messages():
    return [dict(reason="compiler-artifact", target={"name": name}, executable=f"/release/{name}",
                 features=["custom-protocol"] if name == "llcontrol-gui" else [],
                 profile={"opt_level": "3", "test": False}) for name in sorted(dist.BINARIES)]


class DistributionTests(unittest.TestCase):
    def test_rejects_instrumented_and_development_gui(self):
        for features in [[], ["native-smoke"], ["native-smoke", "custom-protocol"]]:
            artifacts = messages()
            next(m for m in artifacts if m["target"]["name"] == "llcontrol-gui")["features"] = features
            with self.subTest(features=features), self.assertRaises(ValueError):
                dist.production_artifacts(artifacts)
        self.assertEqual(set(dist.production_artifacts(messages())), dist.BINARIES)

    def test_missing_or_debug_binary_is_not_reused(self):
        for items in [messages()[:-1], []]:
            with self.assertRaises(ValueError):
                dist.production_artifacts(items)
        items = messages()
        items[0]["profile"]["opt_level"] = "0"
        with self.assertRaises(ValueError):
            dist.production_artifacts(items)

    def test_archive_extracts_with_modes_and_stable_hash_despite_mtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = root / "payload"
            (payload / "usr/bin").mkdir(parents=True)
            binary = payload / "usr/bin/llcontrol"
            binary.write_bytes(b"test artifact\n")
            binary.chmod(0o755)
            first, second = root / "first.tar.gz", root / "second.tar.gz"
            dist.archive_payload(payload, first, 123456)
            os.utime(binary, (999999, 999999))
            dist.archive_payload(payload, second, 123456)
            self.assertEqual(dist.digest(first), dist.digest(second))
            with tarfile.open(first) as archive:
                archive.extractall(root / "extract", filter="data")
                for member in archive.getmembers():
                    self.assertEqual(member.uid, 0)
                    self.assertEqual(member.mtime, 123456)
                    self.assertFalse(member.name.startswith("/"))
            restored = root / "extract/payload/usr/bin/llcontrol"
            self.assertEqual(restored.read_bytes(), binary.read_bytes())
            self.assertEqual(restored.stat().st_mode & 0o777, 0o755)

    def test_package_rule_registers_galahad_without_a_missing_group(self):
        rule = (Path(__file__).parents[2] / "packaging/udev/70-lianli-hid.rules").read_text()
        dist.require_device_ownership(rule)
        for broken in (
            rule.replace('TAG+="uaccess"', ""),
            rule.replace('ATTRS{idProduct}=="7395"', 'ATTRS{idProduct}=="0000"'),
            rule.replace('MODE="0660"', 'MODE="0660", GROUP="plugdev"'),
            '# ' + rule.replace('\n', '\n# '),
            rule.replace('TAG+="uaccess", MODE="0660"', 'MODE="0660"')
            + 'SUBSYSTEM=="hidraw", TAG+="uaccess"\n',
        ):
            with self.subTest(broken=broken), self.assertRaises(ValueError):
                dist.require_device_ownership(broken)

    def test_archive_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage = root / "payload"
            stage.mkdir()
            (stage / "outside").symlink_to("/etc/passwd")
            with self.assertRaises(ValueError):
                dist.archive_payload(stage, root / "bad.tar.gz", 0)


if __name__ == "__main__":
    unittest.main()
