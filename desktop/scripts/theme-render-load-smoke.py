#!/usr/bin/env python3
"""Actual raster/JPEG load against a private MockBackend; no GUI or host sensors."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix='llcontrol-render-load-'))
(root / 'fixture-marker').write_text('MockBackend only\n')
env = dict(os.environ, LLCONTROL_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root),
           XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
           XDG_CACHE_HOME=str(root / 'cache'))
env.pop('LD_PRELOAD', None)


class Session:
    def __enter__(self):
        self.socket = socket.socket(socket.AF_UNIX)
        self.socket.settimeout(45)
        self.socket.connect(str(root / 'llcontrold.sock'))
        self.stream = self.socket.makefile('rwb')
        self.seq = 0
        self.responses = {}
        return self

    def __exit__(self, *args):
        self.stream.close()
        self.socket.close()

    def send(self, method, params):
        self.seq += 1
        self.stream.write((json.dumps({'jsonrpc': '2.0', 'id': self.seq, 'method': method,
                                      'params': params}) + '\n').encode())
        self.stream.flush()
        return self.seq

    def receive(self, request):
        while request not in self.responses:
            line = self.stream.readline()
            assert line, 'IPC closed'
            response = json.loads(line)
            if 'id' in response:
                self.responses[response['id']] = response
        response = self.responses.pop(request)
        if 'error' in response:
            raise RuntimeError(response['error'])
        return response['result']

    def call(self, method, params):
        return self.receive(self.send(method, params))


def until(predicate, message, seconds=45):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.01)
    raise AssertionError(message)


def cpu_seconds(pid):
    # Only the process spawned by this fixture; no host hardware counters.
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')


def fresh_stats():
    path = root / 'stats.json'
    stamp = path.stat().st_mtime_ns if path.exists() else 0
    until(lambda: path.exists() and path.stat().st_mtime_ns > stamp, 'mock stats update', 5)
    return json.loads(path.read_text())


text = ('CPU 溫度 Привет سلام abcdefghijklmnop ' * 30)
assert len(text.encode()) <= 2048
heavy = {'version': 1, 'name': 'Raster load', 'background': [10, 20, 30], 'layers': [
    {'kind': 'text', 'text': text, 'style': {'x': 0, 'y': index % 8, 'size': 64,
        'line_box': True, 'max_width': 480, 'color': [230, 240, 250], 'align': 'left'}}
    for index in range(32)]}
results = []
with (root / 'daemon.log').open('w') as log:
    process = subprocess.Popen([repo / 'target/debug/examples/desktop-mock'], env=env,
                               stdout=log, stderr=subprocess.STDOUT)
    try:
        until(lambda: process.poll() is not None or (root / 'llcontrold.sock').exists(), 'startup', 5)
        assert process.poll() is None
        with Session() as session:
            until(lambda: bool(session.call('list_devices', {})), 'discovery', 5)
            dev = session.call('list_devices', {})[0]['id']
            for method in ['preview_lcd_theme', 'set_lcd_theme']:
                (root / 'cpu-hot').unlink(missing_ok=True)
                until(lambda: next(r for r in session.call('list_sensors', {}) if r['id'] == 'fixture:cpu')['value'] < 50, 'cool sensor baseline', 5)
                session.call('set_cooling_policy', {'dev': dev, 'target': 'fan', 'policy': {
                    'mode': 'curve', 'sensor_id': 'fixture:cpu',
                    'points': [{'celsius': 30, 'percent': 30}, {'celsius': 70, 'percent': 100}],
                    'hysteresis_c': 0, 'fallback_percent': 100}})
                until(lambda: session.call('get_cooling_state', {'dev': dev})['fan']['applied_percent'] < 70, 'curve baseline', 5)
                prior = fresh_stats()
                cpu_start = cpu_seconds(process.pid)
                start = time.monotonic()
                pending = session.send(method, {'dev': dev, 'theme': heavy})
                until(lambda: cpu_seconds(process.pid) - cpu_start >= .2, 'render CPU activity', 5)
                (root / 'cpu-hot').touch()
                def automatic_cooling():
                    state = session.call('get_cooling_state', {'dev': dev})['fan']
                    assert pending not in session.responses, 'render finished before autonomous cooling'
                    return state['applied_percent'] == 100 and not state['fallback_active']
                until(automatic_cooling, 'curve must react while rendering', 2)
                sensor = next(r for r in session.call('list_sensors', {}) if r['id'] == 'fixture:cpu')
                assert sensor['value'] == 95 and sensor['age_ms'] < 1000
                latencies = {}
                for name, params in [
                    ('get_lcd_state', {'dev': dev}),
                    ('list_sensors', {}),
                    ('set_fan_pwm', {'dev': dev, 'percent': 63}),
                    ('set_lighting', {'dev': dev, 'kind': 'pump', 'cfg': {
                        'mode': 'static', 'color': {'r': 10, 'g': 100, 'b': 200},
                        'brightness_percent': 75, 'speed_percent': 50,
                        'direction': 'forward', 'scope': None, 'disabled': False}}),
                    ('clear_lcd', {'dev': dev}),
                ]:
                    sent = time.monotonic()
                    session.call(name, params)
                    latencies[name] = time.monotonic() - sent
                    assert pending not in session.responses, 'render completed before controls'
                    assert latencies[name] < 1, (name, latencies)
                cpu_at_clear = cpu_seconds(process.pid)
                stats = fresh_stats()
                assert stats['fan_pwm_writes'] > prior['fan_pwm_writes']
                assert stats['pump_rgb_writes'] > prior['pump_rgb_writes']
                try:
                    result = session.receive(pending)
                    assert method == 'preview_lcd_theme' and result[:2] == [255, 216]
                except RuntimeError as error:
                    assert method == 'set_lcd_theme' and 'superseded' in str(error), str(error)
                cpu_after_clear = cpu_seconds(process.pid) - cpu_at_clear
                assert cpu_after_clear >= .1, 'no substantial render work remained after Clear'
                assert session.call('get_lcd_state', {'dev': dev})['source'] == 'Idle'
                assert fresh_stats()['jpeg_starts'] == prior['jpeg_starts']
                results.append({'method': method, 'wall_seconds': time.monotonic()-start,
                                'cpu_seconds': cpu_seconds(process.pid)-cpu_start,
                                'cpu_after_clear': cpu_after_clear, 'latencies': latencies,
                                'autonomous_curve_percent': 100, 'hot_sensor_age_ms': sensor['age_ms'],
                                'mock_pwm_rgb_writes': True, 'no_stale_jpeg': True})
            # Successfully apply the same expensive composition, then cancel an
            # actual periodic render. CPU continues after Clear until joined work ends.
            session.call('set_lcd_theme', {'dev': dev, 'theme': heavy})
            before = fresh_stats()
            cpu_start = cpu_seconds(process.pid)
            until(lambda: cpu_seconds(process.pid)-cpu_start >= .2, 'periodic render CPU activity', 5)
            sent = time.monotonic()
            session.call('clear_lcd', {'dev': dev})
            clear_latency = time.monotonic()-sent
            assert clear_latency < 1
            cpu_at_clear = cpu_seconds(process.pid)
            until(lambda: cpu_seconds(process.pid)-cpu_at_clear >= .1, 'periodic work after Clear', 5)
            # A new Apply waits on the asset/font work but reserves ordering while
            # the obsolete periodic frame is still being joined by its owner.
            recovery = {'version': 1, 'name': 'Recovered', 'background': [1, 2, 3], 'layers': []}
            session.call('set_lcd_theme', {'dev': dev, 'theme': recovery})
            session.call('clear_lcd', {'dev': dev})
            # Wait for the cancelled periodic CPU work to drain before checking
            # the final write count; only the recovery Apply may add a JPEG.
            quiet = 0
            last_cpu = cpu_seconds(process.pid)
            deadline = time.monotonic() + 45
            while quiet < 3:
                assert time.monotonic() < deadline, 'render did not drain'
                time.sleep(.2)
                current_cpu = cpu_seconds(process.pid)
                quiet = quiet + 1 if current_cpu-last_cpu < .03 else 0
                last_cpu = current_cpu
            final_frames = fresh_stats()['jpeg_starts']
            assert final_frames == before['jpeg_starts'] + 1, 'cancelled periodic frame was sent'
            # Shutdown joins any old render before the fixture process exits.
            results.append({'periodic_clear_latency': clear_latency,
                            'periodic_cpu_after_clear': cpu_seconds(process.pid)-cpu_at_clear,
                            'recovery_apply': True, 'no_stale_periodic_jpeg': True,
                            'jpeg_after_recovery': final_frames, 'jpeg_before_clear': before['jpeg_starts']})
    finally:
        (root / 'stop').touch()
        try:
            process.wait(timeout=45)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise
    assert process.returncode == 0
(root / 'acceptance.json').write_text(json.dumps({'ok': True, 'hardware': False,
    'build': 'debug', 'theme': heavy, 'scenarios': results}, indent=2))
print('PASS actual raster/JPEG → same-stream controls → Clear/recovery:', root)
