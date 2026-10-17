#!/usr/bin/env python3
"""Offline recognition of the researched LCDV2 image; never opens a device.

This recognizes exact bytes, not arbitrary firmware formats or signatures.
Recognition does not authorize upload: ACK/recovery semantics remain unresolved.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat


@dataclass(frozen=True)
class Candidate:
    target: str
    size: int
    sha256: str
    version: str
    identity: str
    identity_offset: int


# Vendor catalog fetched 2026-10-15; hashes/offset independently recorded in
# docs/development/parity/evidence/2026-10-15-firmware-image-research.json. No vendor bytes
# are bundled. Filename, extension and user-supplied metadata are not trust inputs.
LCDV2 = Candidate(
    target="0416:7395",
    size=626212,
    sha256="68e2d11c409e75b1e25bf9c2e84b891bcdc18ac735bafa67a0ee25a218b2d9d3",
    version="1.6",
    identity="N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6",
    identity_offset=524324,
)


def inspect_file(path: Path, target: str, candidate: Candidate = LCDV2) -> dict:
    """Bound the read and reject unknown bytes before interpreting any identity."""
    if target != candidate.target:
        raise ValueError("This candidate is researched only for 0416:7395.")
    # Linux O_PATH pins the selected inode without opening a device driver or
    # blocking on a FIFO. Only reopen a verified regular inode for actual reads;
    # /proc/self/fd remains anchored even if the original pathname is replaced.
    pinned = os.open(path, os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(pinned)
        if not stat.S_ISREG(info.st_mode):
            raise ValueError("Expected a regular firmware file.")
        if info.st_size != candidate.size:
            raise ValueError(f"Unknown image size; expected {candidate.size} bytes.")
        fd = os.open(f"/proc/self/fd/{pinned}", os.O_RDONLY | os.O_CLOEXEC)
        with os.fdopen(fd, "rb") as stream:
            data = stream.read(candidate.size + 1)
    finally:
        os.close(pinned)
    if len(data) != candidate.size:
        raise ValueError("Image size changed during inspection.")
    digest = hashlib.sha256(data).hexdigest()
    if digest != candidate.sha256:
        raise ValueError("Unknown image SHA-256; no firmware recognized.")
    identity = candidate.identity.encode("ascii") + b"\0"
    if data[candidate.identity_offset:candidate.identity_offset + len(identity)] != identity:
        raise ValueError("Researched image identity does not match its recorded offset.")
    return {
        "status": "recognized_candidate",
        "target": candidate.target,
        "size": len(data),
        "sha256": digest,
        "version": candidate.version,
        "raw_token": candidate.identity,
        "identity_offset": candidate.identity_offset,
        "flash_ready": False,
        "remaining_gates": ["chunk_and_activation_ack", "interruption_recovery",
                            "exclusive_owner_update_job", "bound_reconnect_verification",
                            "hardware_revision_qualification"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("--target", default=LCDV2.target,
                        help="Expected USB VID:PID (default: 0416:7395).")
    args = parser.parse_args()
    try:
        print(json.dumps(inspect_file(args.image, args.target), indent=2))
    except (OSError, ValueError) as error:
        print(json.dumps({"status": "rejected", "flash_ready": False,
                          "error": str(error)}, indent=2))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
