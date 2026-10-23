#!/usr/bin/env python3
"""Native canvas input, profile persistence and mock output acceptance; no physical HID."""
import argparse
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


AT_SPI_QUERY = """
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
Atspi.set_timeout(500,500); Atspi.init()
def find(node,depth=0):
    if depth>30:return None
    if node.get_name()==sys.argv[1]:
        box=Atspi.Component.get_extents(node.get_component_iface(),Atspi.CoordType.SCREEN)
        return dict(x=box.x,y=box.y,width=box.width,height=box.height)
    for i in range(node.get_child_count()):
        child=node.get_child_at_index(i)
        if child:
            result=find(child,depth+1)
            if result:return result
    return None
result=find(Atspi.get_desktop(0));assert result,'native canvas control missing'
print(json.dumps(result))
"""

def case(root, language):
    root.mkdir(mode=0o700)
    (root / 'fixture-marker').write_text('MockBackend/native canvas only\n')
    (root / 'cpu-hot').touch()
    (root / 'native-tmp').mkdir()
    inherited = {k: v for k, v in os.environ.items() if not k.startswith('LCONNECT_NATIVE_')}
    env = dict(inherited, LCONNECT_SMOKE_ROOT=str(root), LCONNECT_NATIVE_CANVAS='1',
               LCONNECT_NATIVE_DESKTOP_LANGUAGE=language, LCONNECT_MOCK_EXPORT_FRAMES='1',
               XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root / 'config'),
               XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'),
               XDG_CONFIG_DIRS=str(root / 'system-config'), TMPDIR=str(root / 'native-tmp'),
               GDK_BACKEND='x11', GIO_USE_VFS='local', NO_AT_BRIDGE='0',
               AT_SPI_BUS_ADDRESS=f'unix:path={root}/a11y.sock',
               WEBKIT_DISABLE_DMABUF_RENDERER='1', LC_ALL='C.UTF-8')
    env.pop('WAYLAND_DISPLAY', None)
    processes, logs, pointer_moves = [], [], []

    def launch(command, name):
        log = (root / (name + '.log')).open('w'); logs.append(log)
        child = subprocess.Popen([str(v) for v in command], env=env, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        processes.append(child)
        return child

    def run(command):
        return subprocess.run([str(v) for v in command], env=env, capture_output=True,
                              text=True, check=True, timeout=10).stdout

    def start_fixture():
        for name in ['stop', 'stats.json', 'lconnectd.sock', 'lcd-frame.json']:
            (root / name).unlink(missing_ok=True)
        fixture = launch([REPO / 'target/debug/examples/desktop-mock'], 'fixture-' + str(len(processes)))
        wait(lambda: (root / 'stats.json').exists() and (root / 'lconnectd.sock').exists()
             or fixture.poll() is not None, 'private fixture')
        assert fixture.poll() is None
        wait(lambda: len(rpc(root, 'list_devices')) == 1, 'mock device')
        return fixture

    def window(gui):
        return run(['xdotool', 'search', '--onlyvisible', '--pid', gui.pid,
                    '--name', '^LL-Control$']).splitlines()[0]

    def close(gui):
        run(['xdotool', 'windowquit', window(gui)])
        gui.wait(timeout=20)
        assert gui.returncode == 0

    def frame():
        path = root / 'lcd-frame.json'
        return bytes(json.loads(path.read_text())['jpeg']) if path.exists() else None

    def workflow(gui):
        key_sequence = 0
        deadline = time.monotonic() + 150
        while gui.poll() is None:
            assert time.monotonic() < deadline, 'native canvas workflow deadline'
            keyboard = root / 'keyboard-step.json'
            if keyboard.exists():
                step = json.loads(keyboard.read_text())
                if step['sequence'] > key_sequence:
                    assert step['sequence'] == key_sequence + 1, step
                    assert step['key'] in {'Home', 'Return', 'ctrl+Right', 'shift+Right'}, step
                    run(['xdotool', 'windowactivate', '--sync', window(gui)])
                    probe = step.get('probe')
                    if probe in {'canvas_move', 'canvas_resize', 'canvas_cancel'}:
                        name = ('Изменить размер выбранного слоя' if language == 'ru-RU' else 'Resize selected layer') if probe == 'canvas_resize' else ('Выбрать слой 2: Текст' if language == 'ru-RU' else 'Select layer 2: Text')
                        query = 'import gi,json,sys\n' + AT_SPI_QUERY
                        box = json.loads(run([sys.executable, '-c', query, name]))
                        assert box['width'] > 4 and box['height'] > 4, box
                        start = (box['x'] + box['width']//2, box['y'] + box['height']//2)
                        end = (start[0] + 24, start[1] + 16)
                        run(['xdotool', 'mousemove', '--sync', *start])
                        run(['xdotool', 'mousedown', '1']); time.sleep(.08)
                        run(['xdotool', 'mousemove', '--sync', *end]); time.sleep(.08)
                        if probe == 'canvas_cancel':
                            run(['xdotool', 'key', 'Escape'])
                        run(['xdotool', 'mouseup', '1']); time.sleep(.08)
                        pointer_moves.append(dict(probe=probe, bounds=box, start=start, end=end))
                    elif probe == 'canvas_no_writes':
                        stats = json.loads((root / 'stats.json').read_text())
                        for counter in ['jpeg_starts', 'lcd_video_packets', 'fan_pwm_writes', 'pump_rgb_writes', 'firmware_update_writes', 'firmware_activations']:
                            assert stats[counter] == 0, (counter, stats[counter])
                    else:
                        assert probe is None, step
                    run(['xdotool', 'key', '--clearmodifiers', step['key']])
                    key_sequence = step['sequence']
            if (root / 'result.json').exists():
                outcome = json.loads((root / 'result.json').read_text())
                run(['import', '-window', window(gui), root / ('recovery.png' if outcome.get('recovery') else 'canvas.png')])
                assert outcome['ok'], outcome
                if not outcome.get('recovery'):
                    for width, height in [(1280, 800), (1024, 768)]:
                        run(['xdotool', 'windowsize', window(gui), width, height])
                        time.sleep(.5)
                        run(['import', '-window', window(gui), root / f'canvas-{width}x{height}.png'])
                return outcome
            time.sleep(.025)
        raise AssertionError('GUI exited before acceptance result')

    try:
        launch(['dbus-daemon', '--config-file=/usr/share/defaults/at-spi2/accessibility.conf',
                f'--address={env["AT_SPI_BUS_ADDRESS"]}', '--nofork'], 'accessibility-bus')
        wait(lambda: (root / 'a11y.sock').exists(), 'private accessibility bus')
        launch(['openbox'], 'wm')
        fixture = start_fixture()
        settings = rpc(root, 'get_display_settings'); settings['interface_language'] = language
        rpc(root, 'set_display_settings', dict(settings=settings))
        gui = launch([REPO / 'target/debug/lconnect-gui'], 'gui')
        outcome = workflow(gui)
        dev, theme = outcome['dev'], outcome['theme']
        library = rpc(root, 'list_profiles')
        expected = bytes(rpc(root, 'preview_lcd_editor', dict(dev=dev, theme=theme))['jpeg'])
        wait(lambda: frame() == expected, 'mock HID output matches saved composition')
        (root / 'applied.jpg').write_bytes(frame())
        (root / 'preview.jpg').write_bytes(expected)
        decode = lambda name: subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-i', str(root / name), '-threads', '1', '-pix_fmt', 'rgb24', '-f', 'rawvideo', '-'], check=True, capture_output=True, timeout=15).stdout
        actual_pixels, preview_pixels = decode('applied.jpg'), decode('preview.jpg')
        assert len(actual_pixels) == 480*480*3 and actual_pixels == preview_pixels
        close(gui)
        assert 'native close: all workers joined' in (root / 'gui.log').read_text()
        (root / 'cpu-hot').unlink()
        wait(lambda: frame() != expected, 'sensor updates continue after GUI close')
        (root / 'cpu-hot').touch()
        wait(lambda: frame() == expected, 'daemon returns to stable source value')
        (root / 'stop').touch(); fixture.wait(timeout=20); assert fixture.returncode == 0
        fixture = start_fixture()
        wait(lambda: rpc(root, 'get_lcd_theme', dict(dev=dev)) == theme, 'applied scene replay after daemon restart')
        assert rpc(root, 'list_profiles') == library
        wait(lambda: frame() == expected, 'restarted daemon reproduces saved geometry')
        (root / 'first-result.json').write_text(json.dumps(outcome, indent=2)+'\n')
        (root / 'result.json').unlink(); (root / 'keyboard-step.json').unlink(missing_ok=True)
        gui = launch([REPO / 'target/debug/lconnect-gui'], 'gui-recovery')
        recovery = workflow(gui); assert recovery.get('recovery')
        close(gui)
        outcome.update(language=language, pointer_moves=pointer_moves, decoded_output_matches=True,
                       playback_after_close=True, daemon_restart=True, gui_reopen=True,
                       actual_hardware_access=False, profile=library, recovery=recovery)
        (root / 'acceptance.json').write_text(json.dumps(outcome, indent=2)+'\n')
        return outcome
    finally:
        (root / 'stop').touch()
        for child in reversed(processes):
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try: child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL); child.wait(timeout=5)
        for log in logs: log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inside', type=Path)
    args = parser.parse_args()
    if args.inside is None:
        root = Path(tempfile.mkdtemp(prefix='lconnect-canvas-'))
        runtime = root / 'session'; runtime.mkdir(mode=0o700)
        env = dict(os.environ, LCONNECT_CANVAS_ISOLATED='1', XDG_RUNTIME_DIR=str(runtime),
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
        print('Native canvas artifacts:', root)
        raise SystemExit(code)
    assert os.environ.get('LCONNECT_CANVAS_ISOLATED') == '1'
    outcomes = [case(args.inside / language, language) for language in ['en-US', 'ru-RU']]
    (args.inside / 'acceptance.json').write_text(json.dumps(dict(ok=True, cases=outcomes), indent=2) + '\n')
    print('PASS native canvas EN/RU input, profiles, output and restart', flush=True)


if __name__ == '__main__':
    main()
