#!/usr/bin/env python3
"""Real CLI + GJS feed against a private read-only Unix peer, without daemon/HID."""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('package', type=Path)
parser.add_argument('--gjs', required=True)
parser.add_argument('--cli', type=Path, required=True)
args = parser.parse_args()
stop = threading.Event()
errors, calls, workers = [], [], []

with tempfile.TemporaryDirectory(prefix='lconnect-mini-gjs-ipc-') as tmp:
    root = Path(tmp)
    (root / 'bin').mkdir()
    (root / 'bin/lconnectctl').symlink_to(args.cli.resolve(strict=True))
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(root / 'peer.sock'))
    listener.listen()
    listener.settimeout(.1)

    def serve(conn):
        try:
            with conn:
                conn.settimeout(.3)
                stream = conn.makefile('rwb')
                stream.write(b'{"jsonrpc":"2.0","method":"hello","params":{}}\n')
                stream.flush()
                while not stop.is_set():
                    if (root / 'disconnect').exists() and not (root / 'recover').exists():
                        return
                    try:
                        line = stream.readline()
                    except (TimeoutError, OSError):
                        return
                    if not line:
                        return
                    request = json.loads(line)
                    method = request['method']
                    calls.append(method)
                    assert request.get('params') == {}, request
                    recovered = (root / 'recover').exists()
                    reading = dict(id='cpu:temperature', label='Настройки', unit='celsius',
                                   value=50 if recovered else 40, status='valid', age_ms=0,
                                   observed_unix_ms=int(time.time() * 1000))
                    config = dict(version=1, revision=0, cpu=dict(temperature='cpu:temperature'), gpu=None)
                    if method == 'list_sensors':
                        value = [reading]
                    elif method == 'get_sensor_defaults':
                        value = dict(config=config, recommended=config,
                                     candidates=[dict(role='cpu_temperature', gpu_id=None, reading=reading)],
                                     values=[dict(role='cpu_temperature', sensor_id=reading['id'], reading=reading)])
                    elif method == 'get_display_settings':
                        value = dict(version=1, revision=0, temperature_unit='fahrenheit',
                                     interface_language='ru-RU' if recovered else 'en-US')
                    else:
                        raise AssertionError('Forbidden method: ' + method)
                    stream.write((json.dumps(dict(jsonrpc='2.0', id=request['id'], result=value)) + '\n').encode())
                    stream.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception as error:
            errors.append(repr(error))

    def accept():
        while not stop.is_set():
            try:
                conn, _ = listener.accept()
            except TimeoutError:
                continue
            except OSError:
                break
            worker = threading.Thread(target=serve, args=(conn,), daemon=True)
            workers.append(worker)
            worker.start()

    server = threading.Thread(target=accept, daemon=True)
    server.start()
    env = dict(os.environ, LCONNECT_MINI_TEST_ROOT=str(root),
               PATH=str(root / 'bin') + ':' + os.environ.get('PATH', ''),
               LCONNECTCTL_SOCKET=str(root / 'peer.sock'), XDG_RUNTIME_DIR=str(root),
               XDG_CONFIG_HOME=str(root / 'config'), XDG_DATA_HOME=str(root / 'data'),
               XDG_CACHE_HOME=str(root / 'cache'))
    process = subprocess.Popen([args.gjs, '-m', str(Path(__file__).with_name('ipc.test.js')),
                                str(args.package.resolve())], env=env, start_new_session=True)
    try:
        code = process.wait(timeout=35)
    finally:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        if process.poll() is None:
            process.wait(timeout=5)
        stop.set()
        listener.close()
        server.join(timeout=2)
        for worker in workers:
            worker.join(timeout=2)
    assert not errors, errors
    assert set(calls) == {'list_sensors', 'get_sensor_defaults', 'get_display_settings'}, calls
    if code == 0:
        print('PASS exact read-only IPC method set; no mutation calls')
    raise SystemExit(code)
