#!/usr/bin/env python3
"""Real storage syscall failures on an owned MockBackend process; no physical HID."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix='lconnect-storage-fault-'))
(root / 'fixture-marker').write_text('MockBackend storage faults only\n')
library = root / 'storage-fault.so'
subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                REPO / 'desktop/scripts/storage-fault.c', '-o', library], check=True)
env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root), LD_PRELOAD=str(library),
           XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root/'config'),
           XDG_DATA_HOME=str(root/'data'), XDG_CACHE_HOME=str(root/'cache'))
for key in list(env):
    if key.startswith('LCONNECT_NATIVE_'):
        env.pop(key)
process = None
logs = []
results = []


def wait(predicate, why, seconds=12):
    deadline = time.monotonic() + seconds
    while not predicate():
        if time.monotonic() > deadline:
            raise TimeoutError(why)
        time.sleep(.025)


def rpc(method, params=None):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(10)
        connection.connect(str(root/'lconnectd.sock'))
        with connection.makefile('rwb') as stream:
            stream.write((json.dumps(dict(jsonrpc='2.0', id=1, method=method,
                                         params=params or {}))+'\n').encode())
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if reply.get('id') == 1:
                    return reply


def call(method, params=None):
    reply = rpc(method, params)
    assert 'error' not in reply, reply
    return reply['result']


def start():
    global process
    for name in ['stop', 'lconnectd.sock', 'stats.json']:
        (root/name).unlink(missing_ok=True)
    log = (root/f'daemon-{len(logs)}.log').open('w'); logs.append(log)
    process = subprocess.Popen([REPO/'target/debug/examples/desktop-mock'], env=env,
                               stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    wait(lambda: (root/'lconnectd.sock').exists() or process.poll() is not None, 'socket')
    assert process.poll() is None
    wait(lambda: len(call('list_devices')) == 1, 'mock attachment')
    return call('list_devices')[0]['id']


def stop():
    (root/'stop').touch()
    process.wait(timeout=15)
    assert process.returncode == 0


def arm(selector, error):
    (root/'storage-fault').write_text(f'{selector} {error}\n')


def disarm():
    (root/'storage-fault').unlink(missing_ok=True)


def persisted(dev):
    status = call('get_lcd_state', dict(dev=dev)).get('persistence') or {}
    return status.get('saved') and not status.get('pending') and not status.get('last_error')


try:
    dev = start()
    call('patch_lcd_config', dict(dev=dev, patch={'brightness_percent': 30}))
    wait(lambda: persisted(dev), 'initial settings saved')
    settings = list((root/'settings/devices').glob('*.toml'))
    assert len(settings) == 1
    for index, (selector, error, before_rename) in enumerate([
        ('settings-write', 'ENOSPC', True),
        ('settings-file-sync', 'EIO', True),
        ('settings-rename', 'ENOSPC', True),
        ('settings-directory-sync', 'EIO', False),
    ]):
        previous = settings[0].read_bytes()
        value = 61 + index
        arm(selector, error)
        call('patch_lcd_config', dict(dev=dev, patch={'brightness_percent': value}))
        wait(lambda: bool((call('get_lcd_state', dict(dev=dev)).get('persistence') or {}).get('last_error')),
             selector+' error visible')
        failed = call('get_lcd_state', dict(dev=dev))
        # `saved` means any successful sync in this session, not this revision.
        assert failed['persistence']['pending'] and failed['persistence']['last_error'], failed
        if before_rename:
            assert settings[0].read_bytes() == previous, 'previous durable settings lost'
        else:
            assert settings[0].read_bytes() != previous, 'post-rename fault did not reach new bytes'
        disarm()
        wait(lambda: persisted(dev), selector+' retry durable')
        stop(); dev = start()
        wait(lambda: call('get_lcd_state', dict(dev=dev))['requested']['brightness_percent'] == value,
             selector+' restart value')
        results.append(dict(case=selector, error=error, pending_reported=True,
                            old_bytes_retained=before_rename, recovered_brightness=value))
    # Exercise publication before and after its rename independently of settings retry.
    builtin = next(clip for clip in call('list_prepared_clips') if clip['is_default'])
    unit = call('load_media_first_frame', {'key': builtin['key']})
    for index, selector in enumerate(['media-file-sync', 'media-directory-sync']):
        key = f'{index+1:032x}'
        upload = lambda action, **extra: rpc('media_upload', {'request': {'action': action, 'id': key, **extra}})
        begin = upload('begin', name='Storage fault media', frames=1, encoded_bytes=len(unit['bytes']))
        assert 'result' in begin, begin
        assert 'result' in upload('chunk', index=0, unit=unit)
        arm(selector, 'EIO')
        finished = upload('finish')
        if selector == 'media-file-sync':
            assert finished.get('result', {}).get('phase') == 'failed', finished
            assert not any(c['key'] == key for c in call('list_prepared_clips'))
        else:
            # Post-publication sync uncertainty must not be reported as durable success.
            assert 'error' in finished or finished.get('result', {}).get('phase') != 'published', finished
        disarm()
        status = upload('status')
        if selector == 'media-file-sync':
            assert status['result']['phase'] == 'failed', status
            assert upload('cancel')['result']['phase'] == 'cancelled'
        else:
            assert status['result']['phase'] == 'published', status
            stop(); dev = start()
            assert upload('status')['result']['phase'] == 'published'
            assert len([c for c in call('list_prepared_clips') if c['key'] == key]) == 1
        results.append(dict(case=selector, finish=finished, readback=status))
    events = (root/'storage-fault-events.log').read_text().splitlines()
    assert len(events) >= 6, events
    (root/'acceptance.json').write_text(json.dumps(dict(ok=True, cases=results,
        syscall_failures=events, actual_hardware_access=False), indent=2)+'\n')
    print('PASS storage syscall failure/recovery:', root, flush=True)
finally:
    disarm()
    if process is not None and process.poll() is None:
        (root/'stop').touch()
        try: process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
    for log in logs: log.close()
    print('Storage fault artifacts:', root, flush=True)
