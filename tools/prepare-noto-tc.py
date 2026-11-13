#!/usr/bin/env python3
"""Reproduce pinned Noto TC OTF assets offline; embed OFL without changing glyphs."""
import argparse
import hashlib
import json
from pathlib import Path
import fontTools
from fontTools.ttLib import TTFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('input', type=Path, help='Directory containing the two official pinned OTF files')
parser.add_argument('--output', type=Path, help='Output directory (defaults to bundled assets)')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1] / 'crates/llcontrol-image/assets'
manifest = json.loads((root / 'NotoSansTC-provenance.json').read_text())
if fontTools.__version__ != manifest['fonttools']:
    raise SystemExit('Use fonttools==' + manifest['fonttools'])
license_bytes = (root / 'NotoSansTC-OFL.txt').read_bytes()
if hashlib.sha256(license_bytes).hexdigest() != manifest['license_sha256']:
    raise SystemExit('License does not match pinned upstream')
# Validate both inputs before writing either output.
for name, source in manifest['inputs'].items():
    if hashlib.sha256((args.input / name).read_bytes()).hexdigest() != source['sha256']:
        raise SystemExit('Input does not match pinned upstream: ' + name)
output = args.output or root
output.mkdir(parents=True, exist_ok=True)
for name in manifest['inputs']:
    font = TTFont(args.input / name, recalcTimestamp=False)
    for record in font['name'].names:
        if record.nameID == 13:
            record.string = license_bytes.decode().encode(record.getEncoding())
    path = output / name
    font.save(path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != manifest['outputs'][name]['sha256']:
        raise SystemExit('Output differs from recorded build: ' + name)
