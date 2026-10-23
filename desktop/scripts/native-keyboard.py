#!/usr/bin/env python3
"""Trusted X11 keyboard input on a private Xvfb and MockBackend, never host HID."""
import json
import os
import re
import socket
from pathlib import Path
import subprocess
import sys
import tempfile
import time

repo = Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    root = Path(tempfile.mkdtemp(prefix='lconnect-native-keyboard-'))
    env = dict(os.environ, LCONNECT_KEYBOARD_ISOLATED='1', LCONNECT_KEYBOARD_ROOT=str(root),
        XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root/'config'),
        XDG_CACHE_HOME=str(root/'cache'), XDG_DATA_HOME=str(root/'data'),
        DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
    for key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS']:
        env.pop(key, None)
    try:
        code = subprocess.call(['xvfb-run', '-a', '-s', '-screen 0 1600x1200x24', 'dbus-run-session', '--', sys.executable, __file__, '--inside-xvfb'], env=env)
    finally:
        target = root/'doc'
        def mounted():
            return any(row.split()[4] == str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines())
        if mounted():
            subprocess.run(['fusermount3','-uz','--',str(target)],check=True)
            assert not mounted(), f'Private document mount remains: {target}'
    raise SystemExit(code)

assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED') == '1', 'Use the outer Xvfb launcher'
root = Path(os.environ['LCONNECT_KEYBOARD_ROOT'])
assert os.environ['XDG_RUNTIME_DIR'] == str(root)
(root / 'fixture-marker').write_text('MockBackend only\n')
(root / 'fixture-image.png').write_bytes((repo / 'desktop/src-tauri/icons/icon.png').read_bytes())
(root / 'fixture-font.ttf').write_bytes((repo / 'crates/lconnect-image/assets/DejaVuSansMono.ttf').read_bytes())
(root / 'fixture-invalid.ttf').write_bytes(b'not a font')
with (root / 'fixture-large.ttf').open('wb') as oversized:
    oversized.truncate(16 * 1024 * 1024 + 1)
env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root), LCONNECT_NATIVE_KEYBOARD='1', XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'), XDG_DATA_HOME=str(root / 'data'), GDK_BACKEND='x11', WEBKIT_DISABLE_DMABUF_RENDERER='1', LC_ALL='C.UTF-8')
env.pop('WAYLAND_DISPLAY', None)
# Opt-in OS font scaling in this private session, independent of WebKit zoom.
text_scale = os.environ.get('LCONNECT_NATIVE_TEXT_SCALE')
if text_scale is not None:
    assert text_scale in {'1', '2'}, 'Fixture text scale must be 1 or 2'
    env.pop('GDK_SCALE', None)
    env.pop('GDK_DPI_SCALE', None)
    env['GTK_THEME'] = 'Adwaita'
    settings_dir = root / 'config/gtk-3.0'
    settings_dir.mkdir(parents=True)
    (settings_dir / 'settings.ini').write_text(
        '[Settings]\ngtk-font-name=Sans 10\ngtk-xft-dpi=' + str(96 * 1024 * int(text_scale)) + '\n')
processes = []
logs = []
def launch(command, name):
    log = (root / (name + '.log')).open('w'); logs.append(log)
    process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
    processes.append(process)
    return process

def read_events():
    # The listener appends JSONL while the GUI is alive; defer an incomplete tail.
    lines = (root / 'accessibility-events.jsonl').read_bytes().split(b'\n')[:-1]
    return [json.loads(line) for line in lines]

def connection_events_ready(events):
    offline_seen = False
    for event in events:
        if event['live'] != 'polite':
            continue
        text = (event['delta'] or '') + event['text']
        if 'Нет связи с daemon' in text:
            offline_seen = True
        elif offline_seen and 'Daemon подключён' in text:
            return True
    return False

def xdo(*args):
    return subprocess.check_output(['xdotool', *args], env=env, text=True, timeout=5).strip()

def active_window():
    # A WM may briefly remove _NET_ACTIVE_WINDOW while mapping a modal dialog.
    result = subprocess.run(['xdotool', 'getactivewindow'], env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
    if result.returncode == 0:
        return result.stdout.strip()
    if '_NET_ACTIVE_WINDOW' in result.stderr:
        return None
    result.check_returncode()

try:
    atspi_enabled = os.environ.get('LCONNECT_NATIVE_ATSPI') == '1'
    dialogs_enabled = os.environ.get('LCONNECT_NATIVE_DIALOGS') == '1'
    assert not dialogs_enabled or atspi_enabled, 'Native dialog acceptance requires LCONNECT_NATIVE_ATSPI=1'
    if atspi_enabled:
        env['AT_SPI_BUS_ADDRESS'] = f'unix:path={root}/a11y.sock'
        env['NO_AT_BRIDGE'] = '0'
        launch(['dbus-daemon', '--config-file=/usr/share/defaults/at-spi2/accessibility.conf', f'--address={env["AT_SPI_BUS_ADDRESS"]}', '--nofork'], 'accessibility-bus')
        bus_deadline = time.monotonic() + 5
        while not (root / 'a11y.sock').exists():
            if time.monotonic() > bus_deadline: raise TimeoutError('Accessibility bus')
            time.sleep(.02)
        event_listener = launch([sys.executable, repo / 'desktop/scripts/native-atspi-events.py'], 'accessibility-events')
        listener_deadline = time.monotonic() + 5
        while not (root / 'accessibility-events-ready').exists():
            assert event_listener.poll() is None, 'AT-SPI event listener exited'
            if time.monotonic() > listener_deadline: raise TimeoutError('AT-SPI event listener')
            time.sleep(.02)
    gtk_settings = None
    if text_scale is not None:
        settings_file = root / 'xsettings.conf'
        settings_file.write_text('Gtk/FontName "Sans 10"\nXft/DPI ' + str(96 * 1024 * int(text_scale)) + '\n')
        settings_manager = launch(['xsettingsd', '-c', str(settings_file)], 'xsettings')
        settings_deadline = time.monotonic() + 5
        while True:
            assert settings_manager.poll() is None, 'Private XSettings manager exited'
            gtk_settings = json.loads(subprocess.check_output(
                [sys.executable, repo / 'desktop/scripts/native-gtk-settings.py'], env=env, text=True, timeout=5))
            if gtk_settings['dpi'] == 96 * 1024 * int(text_scale) and gtk_settings['font'] == 'Sans 10':
                break
            if time.monotonic() > settings_deadline:
                raise TimeoutError(f'Private GTK settings not applied: {gtk_settings}')
            time.sleep(.02)
    wm = launch(['openbox'], 'wm')
    fixture = launch([repo / 'target/debug/examples/desktop-mock'], 'fixture')
    deadline = time.monotonic() + 10
    while not (root / 'lconnectd.sock').exists():
        assert fixture.poll() is None, 'Mock fixture exited'
        if time.monotonic() > deadline: raise TimeoutError('Mock socket')
        time.sleep(.05)
    def rpc(method, params=None):
        with socket.socket(socket.AF_UNIX) as conn:
            conn.settimeout(15); conn.connect(str(root/'lconnectd.sock'))
            stream = conn.makefile('rwb')
            stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode()); stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if reply.get('id') == 1:
                    assert 'error' not in reply, reply
                    return reply['result']
    display = rpc('get_display_settings')
    display['interface_language'] = 'ru-RU'
    rpc('set_display_settings', {'settings':display})
    gui = launch([repo / 'target/debug/lconnect-gui'], 'gui')
    deadline = time.monotonic() + 180
    sequence = 0
    window = None
    window_switch = None
    atspi_results = {}
    file_dialogs = []
    while gui.poll() is None:
        if time.monotonic() > deadline: raise TimeoutError('Native keyboard')
        path = root / 'keyboard-step.json'
        if path.exists():
            step = json.loads(path.read_text())
            if step['sequence'] > sequence:
                assert step['sequence'] == sequence + 1
                assert step['key'] in {'Tab', 'shift+Tab', 'Return', 'space', 'Down', 'Up', 'Left', 'Right', 'End', 'Home'}
                if window is None:
                    window = xdo('search', '--onlyvisible', '--name', '^LL-Control$').splitlines()[0]
                    xdo('windowsize', '--sync', window, '1280', '960')
                    xdo('windowactivate', '--sync', window)
                # The first End is requested with Settings brightness focused.
                # Only this private X server has the GUI and the helper window.
                if step['key'] == 'End' and window_switch is None:
                    helper = launch(['xmessage', '-title', 'Keyboard focus fixture', 'Isolated focus target'], 'focus-helper')
                    helper_window = xdo('search', '--sync', '--onlyvisible', '--name', '^Keyboard focus fixture$').splitlines()[0]
                    xdo('windowactivate', '--sync', window)
                    time.sleep(.1)
                    xdo('key', '--clearmodifiers', 'alt+Tab')
                    time.sleep(.15)
                    assert xdo('getactivewindow') == helper_window, 'Alt+Tab did not leave GUI'
                    xdo('key', '--clearmodifiers', 'alt+Tab')
                    time.sleep(.15)
                    assert xdo('getactivewindow') == window, 'Alt+Tab did not return to GUI'
                    window_switch = {'sequence': step['sequence'], 'gui': window, 'helper': helper_window, 'round_trip': True}
                if step.get('probe') == 'fixture_disconnect':
                    (root / 'stop').touch()
                    fixture.wait(timeout=5)
                elif step.get('probe') == 'fixture_reconnect':
                    assert fixture.poll() is not None, 'Previous fixture still running'
                    (root / 'stop').unlink()
                    fixture = launch([repo / 'target/debug/examples/desktop-mock'], 'fixture-reconnect')
                elif step.get('probe') in {'fixture_file_cancel', 'fixture_file_select', 'fixture_file_font_cancel', 'fixture_file_font_select', 'fixture_file_font_invalid', 'fixture_file_font_large'}:
                    xdo('key', '--clearmodifiers', 'Return')
                    dialog_deadline = time.monotonic() + 8
                    while (dialog := active_window()) in {None, window}:
                        assert gui.poll() is None and wm.poll() is None, 'GUI/WM exited while opening chooser'
                        if time.monotonic() > dialog_deadline: raise TimeoutError('Native file chooser opening')
                        time.sleep(.02)
                    title = xdo('getwindowname', dialog)
                    window_type = subprocess.check_output(['xprop', '-id', dialog, '_NET_WM_WINDOW_TYPE'], env=env, text=True, timeout=3)
                    assert '_NET_WM_WINDOW_TYPE_DIALOG' in window_type, ('Not a dialog', title, window_type)
                    transient = dialog
                    parent_chain = []
                    for _ in range(5):
                        properties = subprocess.check_output(['xprop', '-id', transient, 'WM_TRANSIENT_FOR'], env=env, text=True, timeout=3)
                        match = re.search(r'window id # (0x[0-9a-fA-F]+)', properties)
                        assert match, ('Dialog has no transient parent', title, properties)
                        transient = str(int(match[1], 16))
                        parent_chain.append(transient)
                        if transient == window: break
                    assert transient == window, ('Dialog does not belong to GUI', parent_chain)
                    file_dialogs.append({'action': step['probe'], 'window': dialog, 'title': title, 'parent_chain': parent_chain, 'type': window_type.strip()})
                    kind = {'fixture_file_font_invalid': 'invalid', 'fixture_file_font_large': 'large'}.get(step['probe'], 'font' if '_font_' in step['probe'] else 'image')
                    fixture_name = {'image': 'fixture-image.png', 'font': 'fixture-font.ttf', 'invalid': 'fixture-invalid.ttf', 'large': 'fixture-large.ttf'}[kind]
                    if step['probe'].endswith('_cancel'):
                        xdo('key', '--clearmodifiers', 'Escape')
                    else:
                        xdo('key', '--clearmodifiers', 'ctrl+l')
                        xdo('type', '--clearmodifiers', '--delay', '1', str(root / fixture_name))
                        ready = subprocess.check_output([sys.executable, repo / 'desktop/scripts/native-file-chooser.py', 'chooser', '0', kind], env=env, timeout=8)
                        file_dialogs[-1]['accessibility'] = json.loads(ready)
                        # GTK's Open action; Enter in the location field may only resolve the path.
                        xdo('key', '--clearmodifiers', 'alt+o')
                    dialog_deadline = time.monotonic() + 8
                    while True:
                        visible = dialog in xdo('search', '--onlyvisible', '--name', '.*').splitlines()
                        if not visible and active_window() == window: break
                        if time.monotonic() > dialog_deadline:
                            raise TimeoutError('Native file chooser closing')
                        time.sleep(.02)
                    if not step['probe'].endswith('_cancel'):
                        count = sum(item['action'] == step['probe'] for item in file_dialogs)
                        ready = subprocess.check_output([sys.executable, repo / 'desktop/scripts/native-file-chooser.py', 'imported', str(count), kind], env=env, timeout=8)
                        file_dialogs[-1]['import_finished'] = json.loads(ready)
                elif step.get('probe') == 'fixture_reset_fault':
                    (root / 'fail-lcd').touch()
                elif step.get('probe') == 'fixture_reset_recover':
                    (root / 'fail-lcd').unlink()
                elif step.get('probe') == 'fixture_events_ready':
                    if atspi_enabled:
                        event_deadline = time.monotonic() + 4
                        while True:
                            assert event_listener.poll() is None, 'AT-SPI event listener exited'
                            events = read_events()
                            if connection_events_ready(events):
                                break
                            if time.monotonic() > event_deadline:
                                raise TimeoutError('Connection live events before GUI exit')
                            time.sleep(.02)
                elif atspi_enabled and step.get('probe'):
                    page = step['probe']
                    assert page in {'settings', 'cooling', 'lighting', 'sensors', 'display', 'reset'}
                    tree = subprocess.check_output([sys.executable, repo / 'desktop/scripts/native-atspi.py', page], env=env, text=True, timeout=3)
                    observation = json.loads(tree)
                    result_key = page + ('_' + observation['variant'] if observation['variant'] != 'main' else '')
                    atspi_results[result_key] = observation
                    (root / f'accessibility-{result_key}.json').write_text(tree)

                xdo('key', '--clearmodifiers', step['key'])
                sequence = step['sequence']
        time.sleep(.02)
    result = json.loads((root / 'result.json').read_text())
    print(json.dumps(result, ensure_ascii=False))
    assert gui.returncode == 0 and result['ok'], result
    assert result['trusted_key_steps'] == sequence
    zoom = int(os.environ.get('LCONNECT_NATIVE_ZOOM', '1'))
    assert result['native_zoom'] == zoom, 'Wrong webview zoom'
    expected_reflow = {'settings', 'cooling', 'lighting', 'sensors', 'display'}
    if dialogs_enabled:
        expected_reflow |= {'font-error-invalid', 'font-error-large', 'font-imported'}
    assert {item['page'] for item in result['reflow']} == expected_reflow
    effective_scale = zoom * int(text_scale or '1')
    for item in result['reflow']:
        assert abs(item['dpr'] - effective_scale) < .05, ('OS scale and WebKit zoom not reflected in DPR', item)
        assert abs(item['viewport'] * effective_scale - 1280) <= effective_scale, ('Unexpected test viewport', item)
        assert item['width'] <= item['viewport'] + 1, item
    assert window_switch and result['native_window_focus'], 'Window focus scenario missing'
    result['window_switch'] = window_switch
    if dialogs_enabled:
        assert [item['action'] for item in file_dialogs] == ['fixture_file_cancel', 'fixture_file_select', 'fixture_file_select', 'fixture_file_font_cancel', 'fixture_file_font_invalid', 'fixture_file_font_large', 'fixture_file_font_select', 'fixture_file_font_select']
        keys = result['imported_image_assets']
        assert len(keys) == 2 and all(re.fullmatch('[0-9a-f]{32}', key) for key in keys)
        fonts = list((root / 'settings/theme-fonts').glob('*.font'))
        assert len(fonts) == 2 and all(font.read_bytes() == (root / 'fixture-font.ttf').read_bytes() for font in fonts), 'Failed imports must not create font assets; valid uploads preserve bytes'
        images = [(root / 'settings/theme-assets' / (key + '.png')).read_bytes() for key in keys]
        assert images[0] == images[1] and images[0].startswith(b'\x89PNG'), 'Same selected file must normalize to the same PNG bytes'
    else:
        assert not file_dialogs and not result['imported_image_assets']
    result['native_file_dialogs'] = file_dialogs
    result['gtk_text_settings'] = gtk_settings
    if atspi_enabled:
        assert set(atspi_results) == {'settings', 'cooling', 'cooling_curve', 'cooling_rpm', 'cooling_curve_rpm', 'lighting', 'lighting_thermal', 'lighting_dynamic_celsius', 'lighting_dynamic_invalid', 'lighting_dynamic_rpm', 'sensors', 'display', 'reset'}
        pwm = next(node for node in atspi_results['cooling']['nodes'] if node['name'] == 'PWM, %')
        assert pwm['value']['current'] == 79, 'AT-SPI value did not follow keyboard edit'
        result['native_atspi'] = atspi_results
        events = read_events()
        assert any(event['live'] == 'assertive' and 'Пороги температуры должны соответствовать целым' in ((event['delta'] or '') + event['text']) for event in events), 'Validation error did not emit an assertive AT-SPI text event'
        assert any(event['live'] == 'polite' and 'Настройки ожидают записи на диск.' in event['text'] for event in events), 'Pending save event missing'
        assert any(event['live'] == 'polite' and 'Последняя запись настроек завершена.' in event['text'] for event in events), 'Saved event missing'
        assert connection_events_ready(events), 'Ordered offline → reconnected live events missing'
        if dialogs_enabled:
            for error_text in ['expected a single TrueType/OpenType font', 'Шрифт должен быть размером от 1 байта до 16 MiB']:
                assert any(event['live'] == 'assertive' and error_text in ((event['delta'] or '') + event['text']) for event in events), ('Font error AT-SPI event missing', error_text)
        result['native_live_events'] = events

    (root / 'acceptance.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print('PASS native keyboard; evidence:', root)
finally:
    (root / 'stop').touch()
    for process in reversed(processes):
        if process.poll() is None:
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
    for log in logs: log.close()
    print('Fixture directory:', root)
