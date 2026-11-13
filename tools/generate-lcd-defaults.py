#!/usr/bin/env python3
"""Generate original LCD artwork; no Windows resources or external dependencies."""
import math
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1] / 'crates/llcontrol-image/assets/defaults'
SIZE = 480


def png(name, pixel):
    rows = bytearray()
    for y in range(SIZE):
        rows.append(0)
        for x in range(SIZE):
            rows.extend(pixel(x + .5, y + .5))
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    out = b'\x89PNG\r\n\x1a\n'
    out += chunk(b'IHDR', struct.pack('>2I5B', SIZE, SIZE, 8, 6, 0, 0, 0))
    out += chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b'')
    (ROOT / name).write_bytes(out)


def mark(x, y):
    # Two open orbits: a transparent watermark, with a soft one-pixel edge.
    dx, dy = x - 240, y - 240
    radius, angle = math.hypot(dx, dy), math.atan2(dy, dx)
    outer = abs(radius - 128) if not -.8 < angle < .15 else 999
    inner = abs(radius - 84) if not 2.3 < angle < 3.1 else 999
    alpha = round(255 * min(1, max(0, 9.5 - min(outer, inner))))
    return (98, 218, 237, alpha)


def background(x, y):
    # Dark radial field and offset teal/violet light; legible behind white data.
    teal = max(0, 1 - math.hypot(x - 90, y - 100) / 390) ** 2
    violet = max(0, 1 - math.hypot(x - 410, y - 400) / 340) ** 2
    return (round(9 + 25 * violet), round(16 + 39 * teal), round(29 + 48 * teal + 32 * violet), 255)


if __name__ == '__main__':
    ROOT.mkdir(parents=True, exist_ok=True)
    png('orbit-mark.png', mark)
    png('midnight.png', background)
