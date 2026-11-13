#!/usr/bin/env python3
"""Real asset-load ordering through Unix IPC and private MockBackend; no HID/GUI."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import tomllib

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix='llcontrol-preparation-'))
(root / 'fixture-marker').write_text('MockBackend only\n')
assets = root / 'settings/theme-assets'
assets.mkdir(parents=True)
key = '2' * 32
(assets / (key + '.png')).write_bytes((repo / 'desktop/src-tauri/icons/icon.png').read_bytes())
shim = root / 'read-gate.so'
subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                str(repo / 'desktop/scripts/theme-read-gate.c'), '-o', str(shim)], check=True)
env = dict(os.environ, LLCONTROL_SMOKE_ROOT=str(root), LD_PRELOAD=str(shim),
           XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root / 'config'),
           XDG_DATA_HOME=str(root / 'data'), XDG_CACHE_HOME=str(root / 'cache'))


class Session:
    def __enter__(self):
        self.connection = socket.socket(socket.AF_UNIX)
        self.connection.settimeout(5)
        self.connection.connect(str(root / 'llcontrold.sock'))
        self.stream = self.connection.makefile('rwb')
        self.sequence = 0
        self.responses = {}
        return self

    def __exit__(self, *args):
        self.stream.close()
        self.connection.close()

    def send(self, method, params):
        self.sequence += 1
        self.stream.write((json.dumps({'jsonrpc': '2.0', 'id': self.sequence,
                                      'method': method, 'params': params}) + '\n').encode())
        self.stream.flush()
        return self.sequence

    def receive(self, request_id):
        while request_id not in self.responses:
            line = self.stream.readline()
            assert line, 'unexpected IPC EOF'
            response = json.loads(line)
            if 'id' in response:
                self.responses[response['id']] = response
        response = self.responses.pop(request_id)
        if 'error' in response:
            raise RuntimeError(response['error'])
        return response['result']

    def call(self, method, params):
        return self.receive(self.send(method, params))


def rpc(method, params):
    with Session() as session:
        return session.call(method, params)


def until(predicate, description):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError(description)


def fresh_stats():
    path = root / 'stats.json'
    previous = path.stat().st_mtime_ns if path.exists() else 0
    until(lambda: path.exists() and path.stat().st_mtime_ns > previous, 'fresh mock HID counters')
    return json.loads(path.read_text())


baseline = {'version': 1, 'name': 'Existing', 'background': [12, 34, 56], 'layers': []}
held = {'version': 1, 'name': 'Delayed asset theme', 'background': [0, 0, 0], 'layers': [
    {'kind': 'image', 'asset': key, 'x': 0, 'y': 0, 'width': 480, 'height': 480,
     'fit': 'contain', 'opacity': 255}]}
results = []
with (root / 'daemon.log').open('w') as log:
    process = subprocess.Popen([repo / 'target/debug/examples/desktop-mock'], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        until(lambda: process.poll() is not None or (root / 'llcontrold.sock').exists(), 'fixture startup')
        assert process.poll() is None, 'fixture exited'
        until(lambda: bool(rpc('list_devices', {})), 'mock device discovered')
        dev = rpc('list_devices', {})[0]['id']
        jpeg = rpc('preview_lcd_theme', {'dev': dev, 'theme': baseline})
        for replacement in ['clear', 'image']:
            rpc('set_lcd_theme', {'dev': dev, 'theme': baseline})
            (root / 'read-entered').unlink(missing_ok=True)
            (root / 'hold-read').touch()
            with Session() as session:
                pending = [session.send('set_lcd_theme', {'dev': dev, 'theme': held})]
                try:
                    until(lambda: (root / 'read-entered').exists(), 'actual PNG read did not reach gate')
                    assert session.call('get_lcd_theme', {'dev': dev})['name'] == 'Existing'
                    # Admission reserves tickets in wire order; even a full theme
                    # queue must leave controls available on this same connection.
                    if replacement == 'clear':
                        pending += [session.send('set_lcd_theme', {'dev': dev, 'theme': held}) for _ in range(1)]
                        try:
                            session.call('set_lcd_theme', {'dev': dev, 'theme': held})
                            raise AssertionError('third pending daemon theme job was accepted')
                        except RuntimeError as error:
                            assert 'no permits available' in str(error), str(error)
                    prior_writes = fresh_stats()
                    session.call('set_fan_pwm', {'dev': dev, 'percent': 61})
                    cooling = session.call('get_cooling_state', {'dev': dev})
                    assert cooling['fan']['applied_percent'] == 61
                    session.call('set_lighting', {'dev': dev, 'kind': 'pump', 'cfg': {
                        'mode': 'static', 'color': {'r': 200, 'g': 30, 'b': 90},
                        'brightness_percent': 80, 'speed_percent': 50,
                        'direction': 'forward', 'scope': None, 'disabled': False}})
                    writes = fresh_stats()
                    assert writes['fan_pwm_writes'] > prior_writes['fan_pwm_writes']
                    assert writes['pump_rgb_writes'] > prior_writes['pump_rgb_writes']
                    if replacement == 'clear':
                        session.call('clear_lcd', {'dev': dev})
                    else:
                        session.call('set_lcd_image', {'dev': dev, 'jpeg': jpeg})
                    before = session.call('get_lcd_state', {'dev': dev})
                    frames = fresh_stats()['jpeg_starts']
                    assert before['source'] == ('Idle' if replacement == 'clear' else 'Jpeg')
                    assert (root / 'hold-read').exists()
                finally:
                    (root / 'hold-read').unlink(missing_ok=True)
                for request_id in pending:
                    try:
                        session.receive(request_id)
                        raise AssertionError('obsolete theme was accepted')
                    except RuntimeError as error:
                        assert 'superseded' in str(error), str(error)
                        rejected = str(error)
            after = rpc('get_lcd_state', {'dev': dev})
            assert after['source'] == before['source'] and after['generation'] == before['generation']
            assert after['last_error'] == before['last_error'], 'stale job contaminated current source error'
            assert rpc('get_lcd_theme', {'dev': dev}) is None
            assert fresh_stats()['jpeg_starts'] == frames, 'stale preparation emitted a frame'
            until(lambda: not rpc('get_lcd_state', {'dev': dev})['persistence']['pending'], 'settings flush')
            settings = list((root / 'settings/devices').glob('*.toml'))
            assert settings and all('theme' not in tomllib.loads(p.read_text()) for p in settings), 'stale theme persisted'
            recovery = rpc('set_lcd_theme', {'dev': dev, 'theme': held})
            assert recovery['source'] == 'Theme' and not recovery['last_error']
            assert rpc('get_lcd_theme', {'dev': dev})['name'] == held['name']
            results.append({'replacement': replacement, 'same_connection': True, 'cooling_applied': 61,
                            'rgb_write_completed': True, 'bounded_pending_requests': len(pending), 'read_gate_entered': True, 'stale_error': rejected,
                            'source_preserved': after['source'], 'generation_preserved': after['generation'],
                            'no_stale_frame': True, 'no_stale_persistence': True, 'explicit_recovery': True})
        # Preview is also deferred and cannot delay unrelated reads/control.
        (root / 'read-entered').unlink(missing_ok=True)
        (root / 'hold-read').touch()
        with Session() as session:
            pending = session.send('preview_lcd_theme', {'dev': dev, 'theme': held})
            try:
                until(lambda: (root / 'read-entered').exists(), 'preview read gate')
                session.call('clear_lcd', {'dev': dev})
                assert session.call('get_lcd_state', {'dev': dev})['source'] == 'Idle'
            finally:
                (root / 'hold-read').unlink(missing_ok=True)
            assert session.receive(pending)[:2] == [255, 216]
            assert session.call('get_lcd_theme', {'dev': dev}) is None
        results.append({'preview_same_connection': True, 'clear_before_preview_completion': True})
        rpc('clear_lcd', {'dev': dev})
    finally:
        (root / 'hold-read').unlink(missing_ok=True)
        (root / 'stop').touch()
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise
    assert process.returncode == 0
(root / 'acceptance.json').write_text(json.dumps({'ok': True, 'hardware': False, 'scenarios': results}, indent=2))
print('PASS real asset read → concurrent replacement → superseded/recovery:', root)
