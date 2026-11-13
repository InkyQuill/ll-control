#!/usr/bin/env python3
"""Prepare an LCDV2 clip from GIF/MP4, or generate the M1 diagnostic pattern."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
from typing import Any


FPS = 24
FRAME_COUNT = 48
MAX_ACCESS_UNITS = 240
MAX_ACCESS_UNIT_BYTES = 128 * 1024
MAX_CLIP_BYTES = 1024 * 1024
MAX_INPUT_BYTES = 256 * 1024 * 1024
ENCODE_FILE_LIMIT = MAX_CLIP_BYTES + MAX_ACCESS_UNIT_BYTES


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(argv, check=True, text=True, capture_output=True, timeout=60)


def tool_version(tool: str) -> str:
    output = run([tool, "-version"]).stdout.splitlines()
    return output[0] if output else "unknown"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def annex_b_nal_types(data: bytes) -> list[int]:
    """Return NAL types from the narrow Annex-B form accepted by M1."""
    offset = 0
    nal_types: list[int] = []
    while offset < len(data):
        if data.startswith(b"\x00\x00\x00\x01", offset):
            start_len = 4
        elif data.startswith(b"\x00\x00\x01", offset):
            start_len = 3
        else:
            raise RuntimeError(f"expected Annex-B start code at byte {offset}")
        header = offset + start_len
        if header >= len(data):
            raise RuntimeError("truncated Annex-B start code")
        nal_types.append(data[header] & 0x1F)

        next_three = data.find(b"\x00\x00\x01", header + 1)
        next_four = data.find(b"\x00\x00\x00\x01", header + 1)
        candidates = [position for position in (next_three, next_four) if position >= 0]
        if not candidates:
            break
        offset = min(candidates)
    return nal_types


def validate_units(units: list[dict[str, Any]]) -> None:
    if not 1 <= len(units) <= MAX_ACCESS_UNITS:
        raise RuntimeError(f"expected 1..={MAX_ACCESS_UNITS} access units")
    total = 0
    first_types: list[int] = []
    for index, unit in enumerate(units):
        data = bytes(unit["bytes"])
        if not 1 <= len(data) <= MAX_ACCESS_UNIT_BYTES:
            raise RuntimeError(
                f"access unit {index} must contain 1..={MAX_ACCESS_UNIT_BYTES} bytes"
            )
        total += len(data)
        if total > MAX_CLIP_BYTES:
            raise RuntimeError(f"clip exceeds {MAX_CLIP_BYTES} bytes")
        nal_types = annex_b_nal_types(data)
        has_idr = 5 in nal_types
        if bool(unit["keyframe"]) != has_idr:
            raise RuntimeError(
                f"access unit {index} keyframe marker does not match IDR NAL"
            )
        if index == 0:
            first_types = nal_types
    if not units[0]["keyframe"] or not {5, 7, 8}.issubset(first_types):
        raise RuntimeError(
            "first access unit must be a keyframe containing SPS, PPS, and IDR NALs"
        )


def input_arguments(source: Path | None, fit: str) -> tuple[list[str], dict[str, Any]]:
    if source is None:
        return ["-f", "lavfi", "-i", "testsrc2=size=480x480:rate=24", "-t", "2"], {
            "kind": "diagnostic",
        }
    source = source.resolve(strict=True)
    if not source.is_file() or not 0 < source.stat().st_size <= MAX_INPUT_BYTES:
        raise ValueError("input must be a nonempty regular file no larger than 256 MiB")
    # Force the demuxer, and do not permit network protocols or playlists.
    demuxer = {".gif": "gif", ".mp4": "mov"}.get(source.suffix.lower())
    if demuxer is None:
        raise ValueError("input must be a GIF or MP4 file")
    base = ["-protocol_whitelist", "file", "-f", demuxer]
    probe = json.loads(run([
        "ffprobe", "-v", "error", *base, "-select_streams", "v:0",
        "-show_entries", "stream=width,height:format=duration", "-of", "json",
        str(source),
    ]).stdout)
    streams = probe.get("streams", [])
    if len(streams) != 1:
        raise ValueError("input must contain a readable video stream")
    width, height = int(streams[0]["width"]), int(streams[0]["height"])
    if not (0 < width <= 4096 and 0 < height <= 4096):
        raise ValueError("input dimensions must be within 1..4096 pixels")
    duration = probe.get("format", {}).get("duration")
    if duration is not None and (
        not math.isfinite(float(duration)) or not 0 < float(duration) <= 10
    ):
        raise ValueError("current clip runtime accepts at most 10 seconds; edit the source first")
    if demuxer == "gif":
        base += ["-ignore_loop", "1"]
    if fit == "contain":
        transform = "scale=480:480:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad=480:480:(ow-iw)/2:(oh-ih)/2:color=black"
    else:
        transform = "scale=480:480:force_original_aspect_ratio=increase:force_divisible_by=2:reset_sar=1,crop=480:480"
    # fps follows source timestamps (including GIF frame delays). Ignore embedded
    # GIF loop count: the daemon owns looping. The extra frame detects overflow.
    return [*base, "-i", str(source), "-map", "0:v:0", "-vf",
            f"{transform},setsar=1,fps=24", "-frames:v", str(MAX_ACCESS_UNITS + 1)], {
        "kind": "file", "name": source.name, "fit": fit,
        "width": width, "height": height,
    }

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--input", type=Path, help="local GIF/MP4; omit for diagnostic pattern")
    parser.add_argument("--fit", choices=("contain", "cover"), default="contain")
    args = parser.parse_args()
    output: Path = args.output
    metadata = output.with_name(f"{output.name}.metadata.json")
    for path in (output, metadata):
        if path.exists():
            parser.error(f"refusing to overwrite existing file: {path}")
    output.parent.mkdir(parents=True, exist_ok=True)

    inputs, source_info = input_arguments(args.input, args.fit)
    ffmpeg_args = [
        "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error",
        "-threads", "1", "-filter_threads", "1", *inputs, "-an", "-sn", "-dn",
        "-c:v", "libx264", "-profile:v", "baseline", "-level:v", "3.0",
        "-preset", "ultrafast", "-crf", "25", "-g", "24", "-bf", "0",
        "-pix_fmt", "yuv420p", "-threads", "1", "-fs", str(ENCODE_FILE_LIMIT), "-f", "h264",
    ]
    probe_args = [
        "ffprobe", "-v", "error", "-select_streams", "v:0",
        "-show_entries", "packet=pos,size,flags", "-of", "json",
    ]

    with tempfile.TemporaryDirectory(prefix="llcontrol-lcdv2-") as directory:
        encoded_path = Path(directory) / "diagnostic.h264"
        run([*ffmpeg_args, str(encoded_path)])
        if encoded_path.stat().st_size >= ENCODE_FILE_LIMIT:
            raise ValueError("encoded clip exceeds runtime size limit")
        probe = json.loads(run([*probe_args, str(encoded_path)]).stdout)
        encoded = encoded_path.read_bytes()

    packets: list[dict[str, Any]] = probe.get("packets", [])
    if args.input is None and len(packets) != FRAME_COUNT:
        raise RuntimeError(f"expected {FRAME_COUNT} packets, got {len(packets)}")
    units: list[dict[str, Any]] = []
    expected_pos = 0
    for index, packet in enumerate(packets):
        pos = int(packet["pos"])
        size = int(packet["size"])
        if pos != expected_pos:
            raise RuntimeError(
                f"packet {index} is not contiguous: expected {expected_pos}, got {pos}"
            )
        end = pos + size
        if size <= 0 or end > len(encoded):
            raise RuntimeError(f"packet {index} has invalid range {pos}..{end}")
        units.append({
            "bytes": list(encoded[pos:end]),
            "keyframe": "K" in str(packet.get("flags", "")),
        })
        expected_pos = end
    if expected_pos != len(encoded):
        raise RuntimeError(
            f"packet coverage ends at {expected_pos}, stream has {len(encoded)} bytes"
        )
    validate_units(units)

    manifest = {"fps": FPS, "units": units}
    manifest_bytes = (json.dumps(manifest, separators=(",", ":")) + "\n").encode()
    sidecar = {
        "schema_version": 1,
        "source": source_info,
        "manifest": output.name,
        "frame_count": len(units),
        "encoded_bytes": len(encoded),
        "sha256": {
            "annex_b_stream": sha256(encoded),
            "manifest": sha256(manifest_bytes),
        },
        "tools": {
            "ffmpeg": tool_version("ffmpeg"),
            "ffprobe": tool_version("ffprobe"),
        },
        "commands": {
            "ffmpeg": [*ffmpeg_args, "<temporary-output.h264>"],
            "ffprobe": [*probe_args, "<temporary-output.h264>"],
        },
    }
    # Exclusive creation also protects against a destination appearing while encoding.
    created: list[Path] = []
    try:
        for path, data in [(output, manifest_bytes),
                           (metadata, (json.dumps(sidecar, indent=2) + "\n").encode())]:
            with path.open("xb") as stream:
                created.append(path)
                stream.write(data)
    except BaseException:
        for path in created:
            path.unlink(missing_ok=True)
        raise


if __name__ == "__main__":
    main()
