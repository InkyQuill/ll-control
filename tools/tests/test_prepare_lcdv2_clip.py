"""Real FFmpeg → manifest → decoder/CLI contract; never opens a device."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools/prepare-lcdv2-clip.py"


def run(*args):
    return subprocess.run(args, capture_output=True, timeout=90)


class MediaPreparation(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="llcontrol-media-test-")
        self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name)

    def source(self, extension, duration="1", size="320x160"):
        path = self.directory / f"source.{extension}"
        result = run("ffmpeg", "-nostdin", "-v", "error", "-f", "lavfi", "-i",
                     f"color=red:size={size}:rate=12", "-t", duration,
                     "-threads", "1", str(path))
        self.assertEqual(result.returncode, 0, result.stderr)
        return path

    def prepare(self, source=None, fit="contain", name="clip.json"):
        path = self.directory / name
        args = [sys.executable, str(SCRIPT), "--output", str(path), "--fit", fit]
        if source:
            args += ["--input", str(source)]
        return run(*args), path

    def decode(self, manifest):
        stream = self.directory / "encoded.h264"
        stream.write_bytes(b"".join(bytes(unit["bytes"]) for unit in manifest["units"]))
        probe = run("ffprobe", "-v", "error", "-show_streams", "-of", "json", str(stream))
        self.assertEqual(probe.returncode, 0, probe.stderr)
        info = json.loads(probe.stdout)["streams"][0]
        self.assertEqual((info["width"], info["height"], info["pix_fmt"]), (480, 480, "yuv420p"))
        self.assertIn("Baseline", info["profile"])
        frame = run("ffmpeg", "-v", "error", "-i", str(stream), "-frames:v", "1",
                    "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1")
        self.assertEqual(frame.returncode, 0, frame.stderr)
        self.assertEqual(len(frame.stdout), 480 * 480 * 3)
        return frame.stdout

    def test_mp4_fits_without_stretching_and_cli_accepts_manifest(self):
        source = self.source("mp4")
        for fit in ("contain", "cover"):
            with self.subTest(fit=fit):
                result, path = self.prepare(source, fit, fit + ".json")
                self.assertEqual(result.returncode, 0, result.stderr)
                manifest = json.loads(path.read_text())
                self.assertEqual(manifest["fps"], 24)
                self.assertEqual(len(manifest["units"]), 24)
                self.assertTrue(manifest["units"][0]["keyframe"])
                pixels = self.decode(manifest)
                corner = pixels[:3]
                center = pixels[(240*480+240)*3:(240*480+240)*3+3]
                self.assertGreater(center[0], 200)
                if fit == "contain":
                    self.assertLess(max(corner), 10)
                else:
                    self.assertGreater(corner[0], 200)
                self.assert_cli_contract(path, manifest)

    def assert_cli_contract(self, path, manifest):
        # Only a private Unix socket; explicit device avoids enumeration.
        endpoint = self.directory / "fake.sock"
        endpoint.unlink(missing_ok=True)
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(endpoint))
            listener.listen()
            listener.settimeout(10)
            requests, errors = [], []
            def serve():
                try:
                    conn, _ = listener.accept()
                    with conn:
                        conn.settimeout(10)
                        conn.sendall(b'{"jsonrpc":"2.0","method":"hello","params":{"daemon_version":"0.1.0","ipc_version":"1.0"}}\n')
                        request = json.loads(conn.makefile("rb").readline())
                        requests.append(request)
                        conn.sendall((json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": {}})+"\n").encode())
                except Exception as error:
                    errors.append(error)
            thread = threading.Thread(target=serve)
            thread.start()
            env = dict(os.environ, LLCONTROL_SOCKET=str(endpoint))
            result = subprocess.run([str(ROOT / "target/debug/llcontrol"), "lcd", "play-clip",
                                     str(path), "--loops", "2", "--device", "galahad2-lcdv2:fixture"],
                                    env=env, capture_output=True, timeout=15)
            thread.join(12)
            self.assertFalse(thread.is_alive())
            self.assertFalse(errors, errors)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(requests[0]["method"], "play_lcd_clip")
            self.assertEqual(requests[0]["params"]["clip"], manifest)
            self.assertEqual(requests[0]["params"]["loops"], 2)

    def test_gif_embedded_loop_is_not_expanded(self):
        result, path = self.prepare(self.source("gif"))
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads(path.read_text())
        self.assertEqual(len(manifest["units"]), 24)
        self.decode(manifest)

    def test_non_square_pixels_keep_display_aspect_ratio(self):
        source = self.directory / "wide-pixels.mp4"
        result = run("ffmpeg", "-v", "error", "-f", "lavfi", "-i",
                     "color=red:size=320x160:rate=24", "-t", "1",
                     "-vf", "setsar=2", "-threads", "1", str(source))
        self.assertEqual(result.returncode, 0, result.stderr)
        result, path = self.prepare(source)
        self.assertEqual(result.returncode, 0, result.stderr)
        pixels = self.decode(json.loads(path.read_text()))
        # DAR4:1 → 480x120, not 480x240. y150 must still be padding.
        self.assertLess(max(pixels[(150*480+240)*3:(150*480+240)*3+3]), 10)
        self.assertGreater(pixels[(200*480+240)*3], 200)

    def test_animated_gif_preserves_changing_frames_and_delays(self):
        source = self.directory / "moving.gif"
        result = run("ffmpeg", "-v", "error", "-f", "lavfi", "-i",
                     "testsrc2=size=64x64:rate=2", "-t", "1", "-threads", "1", str(source))
        self.assertEqual(result.returncode, 0, result.stderr)
        # GIF Graphics Control Extension delay in hundredths: .25s + .75s.
        data = source.read_bytes()
        marker = b"\x21\xf9\x04"
        positions = [i for i in range(len(data)) if data.startswith(marker, i)]
        self.assertEqual(len(positions), 2)
        edited = bytearray(data)
        for position, delay in zip(positions, (25, 75)):
            edited[position+4:position+6] = delay.to_bytes(2, "little")
        source.write_bytes(edited)
        result, path = self.prepare(source)
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads(path.read_text())
        self.assertEqual(len(manifest["units"]), 24)
        self.decode(manifest)
        result = run("ffmpeg", "-v", "error", "-i", str(self.directory / "encoded.h264"),
                     "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1")
        self.assertEqual(result.returncode, 0, result.stderr)
        size = 480*480*3
        frames = [result.stdout[i:i+size] for i in range(0, len(result.stdout), size)]
        self.assertEqual(len(frames), 24)
        # Compare mean absolute error to distinguish content changes from
        # inter-frame lossy encoding differences near the expected .25s boundary.
        def distance(a, b):
            return sum(abs(x-y) for x, y in zip(a, b))/len(a)
        self.assertLess(distance(frames[0], frames[4]), 1)
        self.assertGreater(distance(frames[0], frames[8]), 2)
        self.assertLess(distance(frames[8], frames[-1]), distance(frames[0], frames[-1]) / 2)

    def test_diagnostic_is_still_48_frames(self):
        result, path = self.prepare()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(json.loads(path.read_text())["units"]), 48)

    def test_long_or_malformed_input_does_not_publish(self):
        for source in [self.source("mp4", "11"), self.directory / "bad.gif"]:
            if source.name == "bad.gif":
                source.write_bytes(b"not a GIF")
            result, path = self.prepare(source)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(path.exists())
            self.assertFalse(path.with_name(path.name + ".metadata.json").exists())

    def test_existing_output_or_sidecar_is_preserved(self):
        for suffix in ("", ".metadata.json"):
            with self.subTest(suffix=suffix):
                existing = self.directory / ("clip.json" + suffix)
                existing.write_bytes(b"preserve me")
                result, _ = self.prepare()
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(existing.read_bytes(), b"preserve me")
                existing.unlink()


if __name__ == "__main__":
    unittest.main()
