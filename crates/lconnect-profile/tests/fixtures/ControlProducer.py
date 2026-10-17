#!/usr/bin/env python3
"""Combine owned .NET cooling/lighting DTO fixtures; not a vendor export."""
import base64
import json
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent

def backup(name):
    with zipfile.ZipFile(root / name) as archive:
        return json.loads(archive.read('backup'))

cooling, lighting = backup('Cooling.zip'), backup('Lighting.zip')
profile = json.loads(base64.b64decode(cooling['Datas'][1]['Data']))
profile.update(json.loads(base64.b64decode(lighting['Datas'][0]['Data'])))
cooling['Datas'][1]['Data'] = base64.b64encode(
    json.dumps(profile, separators=(',', ':'), ensure_ascii=False).encode()
).decode()
entry = zipfile.ZipInfo('backup', date_time=(2026, 10, 17, 0, 0, 0))
entry.compress_type = zipfile.ZIP_DEFLATED
with zipfile.ZipFile(root / 'Control.zip', 'w') as archive:
    archive.writestr(entry, json.dumps(cooling, separators=(',', ':'), ensure_ascii=False))
