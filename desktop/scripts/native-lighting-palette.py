#!/usr/bin/env python3
"""Lighting palette GUI/replay acceptance using private Xvfb, D-Bus and MockBackend only."""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time

repo = Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    root = Path(tempfile.mkdtemp(prefix='llcontrol-native-lighting-palette-'))
    env = dict(os.environ, LLCONTROL_PALETTE_ISOLATED='1', LLCONTROL_PALETTE_ROOT=str(root),
        XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root/'config'),
        XDG_CACHE_HOME=str(root/'cache'), XDG_DATA_HOME=str(root/'data'),
        DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
    for key in list(env):
        if key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS'] or key.startswith('LLCONTROL_NATIVE_'):
            env.pop(key, None)
    try:
        code = subprocess.call(['xvfb-run', '-a', '-s', '-screen 0 1600x1200x24',
            'dbus-run-session', '--', sys.executable, __file__, '--inside-xvfb'], env=env)
    finally:
        target = root/'doc'
        def mounted():
            return any(row.split()[4] == str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines())
        if mounted():
            subprocess.run(['fusermount3','-uz','--',str(target)],check=True)
            assert not mounted(), f'Private document mount remains: {target}'
    raise SystemExit(code)
assert os.environ.get('LLCONTROL_PALETTE_ISOLATED') == '1'
root = Path(os.environ['LLCONTROL_PALETTE_ROOT'])
assert os.environ['XDG_RUNTIME_DIR'] == str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env = dict(os.environ, LLCONTROL_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root),
    XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'),
    XDG_DATA_HOME=str(root/'data'), GDK_BACKEND='x11', WEBKIT_DISABLE_DMABUF_RENDERER='1',
    LC_ALL='C.UTF-8', NO_AT_BRIDGE='1')
env.pop('WAYLAND_DISPLAY', None)
processes, logs = [], []
fixture = None

def launch(command, name, process_env=env):
    log = (root/f'{name}.log').open('w'); logs.append(log)
    process = subprocess.Popen(command, env=process_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    processes.append(process)
    return process

def wait(predicate, why, seconds=15):
    deadline = time.monotonic()+seconds
    while not predicate():
        if time.monotonic()>deadline: raise TimeoutError(why)
        time.sleep(.05)

def rpc(method, params=None):
    with socket.socket(socket.AF_UNIX) as conn:
        conn.settimeout(15); conn.connect(str(root/'llcontrold.sock'))
        stream = conn.makefile('rwb')
        stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
        while True:
            response = json.loads(stream.readline())
            if response.get('id') == 1:
                assert 'error' not in response, response
                return response['result']

def start_fixture():
    global fixture
    (root/'stop').unlink(missing_ok=True);(root/'llcontrold.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
    fixture = launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}')
    wait(lambda:((root/'llcontrold.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket')
    assert fixture.poll() is None
    wait(lambda:len(rpc('list_devices'))==1,'mock device registration')

def stop_fixture():
    (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0

try:
    launch(['openbox'],'wm'); start_fixture()
    display = rpc('get_display_settings')
    display['interface_language'] = 'ru-RU'
    rpc('set_display_settings', {'settings': display})
    gui = launch([repo/'target/debug/llcontrol-gui'],'gui',dict(env,LLCONTROL_NATIVE_LIGHTING_PALETTE='1'))
    wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native lighting workflow',seconds=120)
    assert (root/'result.json').exists(), f'GUI exited {gui.returncode}'
    result = json.loads((root/'result.json').read_text());assert result['ok'] and result.get('native_base_load') and result.get('applied_mode_profiles') and result.get('native_fan_topology') and result.get('native_dynamic_profiles') and result.get('native_dynamic_selection'), result
    gui.wait(timeout=15);assert gui.returncode==0
    def check_packets():
        stats=json.loads((root/'stats.json').read_text())
        expected=[1,2,3,4,5,6,7,8,9,10,11,12]
        return (stats.get('last_pump_rgb_packet') or [])[10:22]==expected and (stats.get('last_fan_rgb_packet') or [])[9:21]==expected and (stats.get('last_fan_rgb_packet') or [])[24:26]==[1,16]
    wait(check_packets,'both native palette packets')
    stop_fixture()
    import tomllib
    settings=tomllib.loads(next((root/'settings/devices').glob('*.toml')).read_text())
    assert settings['lighting']['fan_topology']=={'sync_to_pump':True,'size':'280','manual_led_count':31}
    for kind in ('fan','pump'):
        assert settings['lighting'][kind]['color']=='#010203'
        assert settings['lighting'][kind]['additional_colors']==['#040506','#070809','#0a0b0c']
    for kind in ('fan','pump'):
        assert set(settings['lighting'][kind+'_modes'])==({'meteor','runway','static'} if kind=='pump' else {'meteor','runway'})
        assert settings['lighting'][kind+'_modes']['runway']['color']=='#112233'
        assert settings['lighting'][kind+'_modes']['runway']['additional_colors']==['#445566']
    start_fixture();wait(check_packets,'palette packet replay')
    for kind in ('fan','pump'):
        state=rpc('get_lighting_info',{'dev':result['dev'],'kind':kind})
        runway=next(c for c in state['profiles'] if c['mode']=='runway')
        assert runway['color']=={'r':17,'g':34,'b':51} and runway['additional_colors']==[{'r':68,'g':85,'b':102}]

    assert rpc('get_lighting_info',{'dev':result['dev'],'kind':'fan'})['fan_topology']==settings['lighting']['fan_topology']
    restored=rpc('get_dynamic_rgb',{'dev':result['dev']})
    assert restored['requested'] is None and restored['profiles']==result['dynamic_profiles']
    assert next(p for p in restored['profiles']['modes'] if p['mode']=='static')['enabled'] is True
    assert next(p for p in restored['profiles']['modes'] if p['mode']=='meteor')['enabled'] is False
    assert rpc('get_lighting_info',{'dev':result['dev'],'kind':'pump'})['base']['dynamic_mode'] is False

    result.update(restart_replay=True,settings_palettes=settings['lighting'],gui_exit=gui.returncode)
    (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print('PASS native lighting palettes WebKitGTK/Unix IPC/MockBackend/restart:',root)

finally:
    if fixture is not None and fixture.poll() is None:
        (root/'stop').touch()
    for process in reversed(processes):
        try: os.killpg(process.pid,signal.SIGTERM)
        except ProcessLookupError: pass
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try: os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError: pass
            process.wait(timeout=5)
    for log in logs:log.close()
    print('Fixture directory:',root)
