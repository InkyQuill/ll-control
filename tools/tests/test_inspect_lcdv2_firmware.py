"""Offline file recognition guards; synthetic bytes, no HID or vendor fixture."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "inspect-lcdv2-firmware.py"
spec = importlib.util.spec_from_file_location("firmware_inspector", SCRIPT)
inspector = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = inspector
spec.loader.exec_module(inspector)


class FirmwareInspection(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="lcdv2-inspect-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.path = self.root / "arbitrary-name.dat"
        self.data = b"test-prefix:SYNTHETIC-ID\0test-suffix"
        self.path.write_bytes(self.data)
        self.candidate = inspector.Candidate("0416:7395", len(self.data),
            hashlib.sha256(self.data).hexdigest(), "test-version", "SYNTHETIC-ID", 12)

    def inspect(self, path=None, target="0416:7395"):
        return inspector.inspect_file(path or self.path, target, self.candidate)

    def test_exact_content_recognized_independent_of_filename_but_not_flash_ready(self):
        result = self.inspect()
        self.assertEqual(result["status"], "recognized_candidate")
        self.assertEqual(result["raw_token"], "SYNTHETIC-ID")
        self.assertFalse(result["flash_ready"])
        self.assertIn("interruption_recovery", result["remaining_gates"])

    def test_target_is_checked_before_opening_a_path(self):
        with self.assertRaisesRegex(ValueError, "0416:7395"):
            self.inspect(self.root / "does-not-exist", "0416:7398")

    def test_truncated_empty_oversize_and_same_length_corruption_rejected(self):
        for data in [b"", self.data[:-1], self.data + b"x", b"X" + self.data[1:]]:
            with self.subTest(data=data):
                self.path.write_bytes(data)
                with self.assertRaises(ValueError):
                    self.inspect()

    def test_identity_string_and_filename_cannot_authorize_unknown_image(self):
        path = self.root / "HH015CHS_N9H26HSSQ_GALAHADII-Vision_V201041_pid7395.bin"
        path.write_bytes(self.data[:-1] + b"X")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.inspect(path)

    def test_identity_offset_consistency_is_checked_after_hash(self):
        wrong = inspector.Candidate("0416:7395", len(self.data),
            self.candidate.sha256, "test", "SYNTHETIC-ID", 0)
        with self.assertRaisesRegex(ValueError, "offset"):
            inspector.inspect_file(self.path, "0416:7395", wrong)

    def test_symlink_fifo_and_directory_are_rejected_without_blocking(self):
        link = self.root / "link"; link.symlink_to(self.path)
        with self.assertRaises((OSError, ValueError)):
            self.inspect(link)
        fifo = self.root / "pipe"; os.mkfifo(fifo)
        for path in [fifo, self.root]:
            with self.subTest(path=path), self.assertRaises((OSError, ValueError)):
                self.inspect(path)

    def test_real_cli_never_accepts_synthetic_or_missing_input(self):
        for path in [self.path, self.root / "missing"]:
            result = subprocess.run([sys.executable, SCRIPT, str(path)],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 1)
            report = json.loads(result.stdout)
            self.assertEqual(report["status"], "rejected")
            self.assertFalse(report["flash_ready"])

    def test_replaced_path_cannot_redirect_the_pinned_regular_file_read(self):
        original_open = os.open
        fifo = self.root / "pipe"; os.mkfifo(fifo)

        def replace_after_pin(path, flags):
            fd = original_open(path, flags)
            if path == self.path:
                self.path.rename(self.root / "old-inode")
                self.path.symlink_to(fifo)
            return fd

        with patch.object(inspector.os, "open", side_effect=replace_after_pin):
            self.assertEqual(self.inspect()["status"], "recognized_candidate")


if __name__ == "__main__":
    unittest.main()
