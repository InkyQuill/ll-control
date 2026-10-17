#!/usr/bin/env python3
"""Private native gallery image editor EN/RU acceptance; no physical HID/session."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]


def wait(predicate, why, seconds=30):
    deadline = time.monotonic() + seconds
    while not predicate():
        if time.monotonic() > deadline:
            raise TimeoutError(why)
        time.sleep(.025)


def rpc(root, method, params=None):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(10)
        connection.connect(str(root / 'lconnectd.sock'))
        with connection.makefile('rwb') as stream:
            stream.write((json.dumps(dict(jsonrpc='2.0', id=1, method=method, params=params or {})) + '\n').encode())
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if reply.get('id') == 1:
                    assert 'error' not in reply, reply
                    return reply['result']


def case(root, language):
    root.mkdir(mode=0o700)
    (root / 'fixture-marker').write_text('MockBackend/native image editor only\n')
    (root / 'native-tmp').mkdir()
    inherited = {k: v for k, v in os.environ.items() if not k.startswith('LCONNECT_NATIVE_')}
    env = dict(inherited, LCONNECT_SMOKE_ROOT=str(root), LCONNECT_NATIVE_IMAGE_EDITOR='1',
               LCONNECT_NATIVE_MEDIA='1', LCONNECT_MOCK_EXPORT_FRAMES='1',
               XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root / 'config'),
               XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
               XDG_CONFIG_DIRS=str(root / 'system-config'), TMPDIR=str(root / 'native-tmp'),
               GDK_BACKEND='x11', GIO_USE_VFS='local', NO_AT_BRIDGE='0',
               AT_SPI_BUS_ADDRESS=f'unix:path={root}/a11y.sock',
               WEBKIT_DISABLE_DMABUF_RENDERER='1', LC_ALL='C.UTF-8')
    env.pop('WAYLAND_DISPLAY', None)
    processes, logs = [], []

    def launch(command, name):
        log = (root / (name + '.log')).open('w'); logs.append(log)
        child = subprocess.Popen([str(v) for v in command], env=env, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        processes.append(child)
        return child

    def run(command):
        return subprocess.run([str(v) for v in command], env=env, capture_output=True,
                              text=True, check=True, timeout=10).stdout

    try:
        launch(['dbus-daemon', '--config-file=/usr/share/defaults/at-spi2/accessibility.conf',
                f'--address={env["AT_SPI_BUS_ADDRESS"]}', '--nofork'], 'accessibility-bus')
        wait(lambda: (root / 'a11y.sock').exists(), 'private accessibility bus')
        launch(['openbox'], 'wm')
        fixture = launch([REPO / 'target/debug/examples/desktop-mock'], 'fixture')
        wait(lambda: (root / 'stats.json').exists() and (root / 'lconnectd.sock').exists()
             or fixture.poll() is not None, 'private fixture')
        assert fixture.poll() is None
        settings = rpc(root, 'get_display_settings'); settings['interface_language'] = language
        rpc(root, 'set_display_settings', dict(settings=settings))
        gui = launch([REPO / 'target/debug/lconnect-gui'], 'gui')
        deadline = time.monotonic() + 100
        acknowledged = closed = False
        receipt = None
        key_sequence = 0
        pointer_moves = []
        while gui.poll() is None:
            assert time.monotonic() < deadline, 'native image workflow deadline'
            keyboard = root / 'keyboard-step.json'
            if keyboard.exists():
                key_step = json.loads(keyboard.read_text())
                if key_step['sequence'] > key_sequence:
                    assert key_step['sequence'] == key_sequence + 1, key_step
                    assert key_step['key'] in {'space', 'Tab', 'Home', 'Return'}, key_step
                    window = run(['xdotool', 'search', '--onlyvisible', '--pid', gui.pid,
                                  '--name', '^L-Connect Linux$']).splitlines()[0]
                    run(['xdotool', 'windowactivate', '--sync', window])
                    probe = key_step.get('probe')
                    values = {'image_crop_x': '30', 'image_crop_y': '20', 'image_crop_side': '150'}
                    if probe in values:
                        run(['xdotool', 'key', '--clearmodifiers', 'ctrl+a'])
                        run(['xdotool', 'type', '--clearmodifiers', '--delay', '10', values[probe]])
                    elif probe == 'image_crop_pointer':
                        query = r"""
import gi, json, os, sys
from pathlib import Path
assert os.environ['AT_SPI_BUS_ADDRESS'] == 'unix:path=' + os.environ['LCONNECT_SMOKE_ROOT'] + '/a11y.sock'
assert (Path(os.environ['LCONNECT_SMOKE_ROOT']) / 'fixture-marker').is_file()
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
Atspi.set_timeout(500, 500); Atspi.init()
def find(node, depth=0):
    if depth > 30: return None
    if node.get_name() == sys.argv[1]:
        box = Atspi.Component.get_extents(node.get_component_iface(), Atspi.CoordType.SCREEN)
        return dict(x=box.x, y=box.y, width=box.width, height=box.height)
    for i in range(node.get_child_count()):
        child = node.get_child_at_index(i)
        if child:
            result = find(child, depth+1)
            if result: return result
    return None
result = find(Atspi.get_desktop(0)); assert result, 'native crop button missing'
print(json.dumps(result))
"""
                        name = ('Переместить область crop указателем; точные координаты доступны в полях выше'
                                if language == 'ru-RU' else 'Drag the crop region; exact coordinates are available in the fields above')
                        box = json.loads(run([sys.executable, '-c', query, name]))
                        assert box['width'] > 10 and box['height'] > 10, box
                        start = (box['x'] + box['width']//2, box['y'] + box['height']//2)
                        end = (box['x'] + box['width'] + 25, box['y'] + box['height'] + 25)
                        run(['xdotool', 'mousemove', '--sync', *start])
                        run(['xdotool', 'mousedown', '1']); time.sleep(.08)
                        run(['xdotool', 'mousemove', '--sync', *end]); time.sleep(.08)
                        run(['xdotool', 'mouseup', '1']); time.sleep(.08)
                        released_return = (box['x'] + 2, box['y'] + 2)
                        run(['xdotool', 'mousemove', '--sync', *released_return]); time.sleep(.08)
                        pointer_moves.append(dict(bounds=box, start=start, end=end, released_return=released_return))
                    else:
                        assert probe is None, key_step
                    run(['xdotool', 'key', '--clearmodifiers', key_step['key']])
                    key_sequence = key_step['sequence']
            step = root / 'media-step.json'
            result = root / 'result.json'
            if step.exists() and not acknowledged:
                value = json.loads(step.read_text())
                assert value == dict(sequence=1, action='editor_saved'), value
                dev = rpc(root, 'list_devices')[0]['id']
                theme = rpc(root, 'get_lcd_theme', dict(dev=dev))
                assert len(theme['layers']) == 1 and theme['layers'][0]['kind'] == 'image', theme
                expected = bytes(rpc(root, 'preview_lcd_theme', dict(theme=theme)))

                def matches():
                    nonlocal receipt
                    path = root / 'lcd-frame.json'
                    if not path.exists():
                        return False
                    receipt = json.loads(path.read_text())
                    return bytes(receipt['jpeg']) == expected

                wait(matches, 'actual mock-HID JPEG equals applied image theme')
                (root / 'applied.jpg').write_bytes(bytes(receipt['jpeg']))
                (root / 'applied-theme.json').write_text(json.dumps(theme, indent=2) + '\n')
                window = run(['xdotool', 'search', '--onlyvisible', '--pid', gui.pid,
                              '--name', '^L-Connect Linux$']).splitlines()[0]
                run(['import', '-window', window, root / 'applied.png'])
                (root / 'media-ack').write_text('1'); acknowledged = True
            if result.exists() and not closed:
                try:
                    outcome = json.loads(result.read_text())
                except json.JSONDecodeError:
                    continue
                if not outcome['ok']:
                    window = run(['xdotool', 'search', '--onlyvisible', '--pid', gui.pid,
                                  '--name', '^L-Connect Linux$']).splitlines()[0]
                    try:
                        run(['import', '-window', window, root / 'failure.png'])
                    except subprocess.CalledProcessError as error:
                        (root / 'failure-capture.log').write_text(error.stderr or str(error))
                assert outcome['ok'], outcome
                window = run(['xdotool', 'search', '--onlyvisible', '--pid', gui.pid,
                              '--name', '^L-Connect Linux$']).splitlines()[0]
                run(['xdotool', 'windowquit', window]); closed = True
            time.sleep(.025)
        assert gui.returncode == 0, (root / 'gui.log').read_text()
        outcome = json.loads((root / 'result.json').read_text())
        assert outcome['ok'] and closed and acknowledged, outcome
        assert outcome['trusted_key_steps'] == key_sequence == 9, outcome
        assert len(pointer_moves) == 1 and outcome['trusted_inputs'] >= 7, outcome
        assert 'native close: all workers joined' in (root / 'gui.log').read_text()
        assert not list((root / 'native-tmp').iterdir()), 'staging survives normal Close'
        dev = rpc(root, 'list_devices')[0]['id']
        assert rpc(root, 'get_lcd_state', dict(dev=dev))['source'] == 'Idle'
        stats = json.loads((root / 'stats.json').read_text())
        for key in ['fan_pwm_writes', 'pump_rgb_writes', 'firmware_update_writes', 'firmware_activations']:
            assert stats[key] == 0, (key, stats[key])
        assets = {}
        for kind in ['original', 'replacement']:
            data = bytes(rpc(root, 'read_theme_asset_for_edit', dict(key=outcome[kind])))
            path = root / (kind + '.png'); path.write_bytes(data)
            assets[kind] = dict(key=outcome[kind], sha256=hashlib.sha256(data).hexdigest(),
                                dimensions=run(['identify', '-format', '%wx%h', path]).strip())
        assert assets['replacement']['dimensions'] == '480x480'
        outcome.update(assets=assets, pointer_moves=pointer_moves, actual_mock_jpeg_match=True, sequence=receipt['sequence'],
                       normal_close=True, staging_empty=True, actual_hardware_access=False)
        (root / 'acceptance.json').write_text(json.dumps(outcome, indent=2) + '\n')
        return outcome
    finally:
        (root / 'stop').touch()
        for child in reversed(processes):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=5)
        for log in logs:
            log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inside', type=Path)
    args = parser.parse_args()
    if args.inside is None:
        root = Path(tempfile.mkdtemp(prefix='lconnect-image-editor-'))
        runtime = root / 'session'; runtime.mkdir(mode=0o700)
        env = dict(os.environ, LCONNECT_IMAGE_EDITOR_ISOLATED='1', XDG_RUNTIME_DIR=str(runtime),
                   XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
                   XDG_CACHE_HOME=str(root / 'cache'), DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
        for key in ['DISPLAY', 'WAYLAND_DISPLAY', 'AT_SPI_BUS_ADDRESS', 'DBUS_SESSION_BUS_ADDRESS']:
            env.pop(key, None)
        try:
            code = subprocess.call(['xvfb-run', '-a', '-s', '-screen 0 1600x1200x24',
                'dbus-run-session', '--', sys.executable, __file__, '--inside', str(root)], env=env)
        finally:
            target = runtime / 'doc'
            if any(row.split()[4] == str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines()):
                subprocess.run(['fusermount3', '-uz', '--', str(target)], check=True, timeout=10)
        print('Native image editor artifacts:', root)
        raise SystemExit(code)
    assert os.environ.get('LCONNECT_IMAGE_EDITOR_ISOLATED') == '1'
    outcomes = [case(args.inside / language, language) for language in ['en-US', 'ru-RU']]
    (args.inside / 'acceptance.json').write_text(json.dumps(dict(ok=True, cases=outcomes), indent=2) + '\n')
    print('PASS native gallery image editor EN/RU', flush=True)


if __name__ == '__main__':
    main()
