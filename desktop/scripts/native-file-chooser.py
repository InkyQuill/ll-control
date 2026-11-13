#!/usr/bin/env python3
"""Wait for the owned GTK image/font chooser to receive its typed fixture path."""
import json
import os
import re
import sys
from pathlib import Path
import time

import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi, GLib

root = Path(os.environ['LLCONTROL_SMOKE_ROOT'])
assert os.environ.get('LLCONTROL_KEYBOARD_ISOLATED') == '1'
assert (root / 'fixture-marker').is_file()
assert os.environ['AT_SPI_BUS_ADDRESS'] == f'unix:path={root}/a11y.sock'
kind = sys.argv[3] if len(sys.argv) > 3 else 'image'
fixture_names = {'image': 'fixture-image.png', 'font': 'fixture-font.ttf', 'invalid': 'fixture-invalid.ttf', 'large': 'fixture-large.ttf', 'editor': 'editor.mp4', 'media': 'fixture.mp4', 'gif': 'fixture.gif', 'bad_media': 'broken.mp4'}
assert kind in fixture_names
expected = str(root / fixture_names[kind])
mode = sys.argv[1] if len(sys.argv) > 1 else 'chooser'
assert mode in {'chooser', 'imported', 'location'}
expected_images = int(sys.argv[2]) if mode == 'imported' else 0
assert mode != 'imported' or expected_images in {1, 2}
Atspi.init()
Atspi.set_timeout(500, 500)

def collect(node, rows, depth=0):
    if depth > 25 or len(rows) > 1200:
        raise RuntimeError('Unexpected chooser accessibility tree')
    node.clear_cache()
    state = node.get_state_set()
    row = {'name': node.get_name(), 'role': node.get_role_name(),
           'enabled': state.contains(Atspi.StateType.ENABLED),
           'focused': state.contains(Atspi.StateType.FOCUSED)}
    if 'Text' in node.get_interfaces():
        interface = node.get_text_iface()
        row['text'] = Atspi.Text.get_text(interface, 0, min(Atspi.Text.get_character_count(interface), 1024))
    if mode == 'chooser' and os.environ.get('LLCONTROL_NATIVE_TEXT_SCALE'):
        if 'Component' in node.get_interfaces():
            rect = node.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
            row['rect'] = {key: getattr(rect, key) for key in ('x', 'y', 'width', 'height')}
        if row.get('text'):
            rect = Atspi.Text.get_character_extents(node.get_text_iface(), 0, Atspi.CoordType.SCREEN)
            row['first_character'] = {key: getattr(rect, key) for key in ('x', 'y', 'width', 'height')}
            row['text_attributes'] = dict(Atspi.Text.get_default_attributes(node.get_text_iface()))
    rows.append(row)
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child:
            collect(child, rows, depth + 1)

rows = []
observed = []
context = GLib.MainContext.default()
deadline = time.monotonic() + 5
while time.monotonic() < deadline:
    # Deliver registry/window events between synchronous reads. Without a main
    # context iteration a freshly reopened chooser can remain absent in cache.
    for _ in range(100):
        if not context.pending():
            break
        context.iteration(False)
    observed = []
    applications = []
    desktop = Atspi.get_desktop(0)
    desktop.clear_cache()
    for index in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(index)
        if not app:
            continue
        app.clear_cache()
        application_name=app.get_name()
        applications.append(application_name)
        if application_name != 'llcontrol-gui':
            continue
        if mode == 'imported':
            if kind == 'font' and len(list((root / 'settings/theme-fonts').glob('*.font'))) != expected_images:
                continue
            rows = []
            collect(app, rows)
            if kind in {'invalid', 'large'}:
                error_text = 'expected a single TrueType/OpenType font' if kind == 'invalid' else 'Шрифт должен быть размером от 1 байта до 16 MiB'
                error_visible = any(error_text in row.get('text', '') for row in rows)
                picker = next((row for row in rows if row['role'] == 'button' and row['name'] == 'Загрузить шрифт'), None)
                if error_visible and picker and picker['enabled']:
                    print(json.dumps({'error': error_text, 'picker_enabled': True, 'picker_focused': picker['focused']}))
                    raise SystemExit(0)
                continue
            if kind == 'font':
                notice = any('Шрифт загружен.' in row.get('text', '') for row in rows)
                picker = next((row for row in rows if row['role'] == 'button' and row['name'] == 'Загрузить шрифт'), None)
                if notice and picker and picker['enabled']:
                    print(json.dumps({'font_imported': True, 'picker_enabled': True, 'picker_focused': picker['focused']}))
                    raise SystemExit(0)
                continue
            image_count = sum(row['role'] == 'heading' and bool(re.fullmatch(r'\d+\. Изображение', row['name'])) for row in rows)
            notice = any(row.get('text') == 'Изображение добавлено в черновик темы.' for row in rows)
            picker = next((row for row in rows if row['role'] == 'button' and row['name'] == '+ Изображение'), None)
            if image_count == expected_images and notice and picker and picker['enabled']:
                print(json.dumps({'image_layers': image_count, 'picker_enabled': True, 'picker_focused': picker['focused']}))
                raise SystemExit(0)
            continue
        for child_index in range(app.get_child_count()):
            chooser = app.get_child_at_index(child_index)
            if not chooser:
                continue
            chooser.clear_cache()
            observed.append({'role': chooser.get_role_name(), 'name': chooser.get_name(), 'showing': chooser.get_state_set().contains(Atspi.StateType.SHOWING)})
            if chooser.get_role_name() != 'file chooser' or chooser.get_name() != 'Select File' or not chooser.get_state_set().contains(Atspi.StateType.SHOWING):
                continue
            rows = []
            collect(chooser, rows)
            if mode == 'location':
                focused = [row for row in rows if row['role'] == 'text' and row['focused']]
                if focused:
                    print(json.dumps({'location_focused': True, 'text': focused[0].get('text', '')}))
                    raise SystemExit(0)
                continue
            path_ready = any(row.get('text') == expected and row['role'] == 'text' and row['focused'] for row in rows)
            open_ready = any(row['name'] == 'Open' and row['role'] == 'button' and row['enabled'] for row in rows)
            if path_ready and open_ready:
                result = {'path': expected, 'focused_path_verified': path_ready, 'open_enabled': open_ready}
                if os.environ.get('LLCONTROL_NATIVE_TEXT_SCALE'):
                    controls = [next(row for row in rows if row.get('text') == expected and row['focused']),
                                next(row for row in rows if row['name'] == 'Open' and row['role'] == 'button')]
                    for control in controls:
                        rect = control['rect']
                        assert rect['width'] > 0 and rect['height'] > 0, control
                        assert rect['x'] >= 0 and rect['y'] >= 0, control
                        assert rect['x'] + rect['width'] <= 1600 and rect['y'] + rect['height'] <= 1200, control
                    assert controls[0]['first_character']['height'] > 0, controls[0]
                    result['scaled_controls'] = controls
                print(json.dumps(result))
                raise SystemExit(0)
    time.sleep(.05)
raise TimeoutError(f'Chooser path/Open not ready: applications={applications}; windows={observed}; controls={rows}')
