#!/usr/bin/env python3
"""Weather GUI/slot acceptance using private Xvfb, D-Bus and MockBackend only."""
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
 root=Path(tempfile.mkdtemp(prefix='llcontrol-native-weather-'))
 env=dict(os.environ,LLCONTROL_WEATHER_ISOLATED='1',LLCONTROL_WEATHER_ROOT=str(root),
          XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),
          XDG_DATA_HOME=str(root/'data'),DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
 for key in list(env):
  if key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS'] or key.startswith('LLCONTROL_NATIVE_'):env.pop(key,None)
 try:
  code=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb',*sys.argv[1:]],env=env)
 finally:
  target=root/'doc'
  def mounted():return any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines())
  if mounted():
   cleanup=subprocess.run(['fusermount3','-uz','--',str(target)],capture_output=True)
   if cleanup.returncode and mounted():raise RuntimeError(f'Private document mount remains: {target}')
 raise SystemExit(code)
assert os.environ.get('LLCONTROL_WEATHER_ISOLATED')=='1'
root=Path(os.environ['LLCONTROL_WEATHER_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
(root/'weather-cpu-load').touch()
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
    settings=rpc('get_display_settings');settings['interface_language']='ru-RU'
    rpc('set_display_settings',{'settings':settings})
    gui = launch([repo/'target/debug/llcontrol-gui'],'gui',dict(env,LLCONTROL_NATIVE_WEATHER='1'))
    wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native weather workflow',seconds=120)
    assert (root/'result.json').exists(), f'GUI exited {gui.returncode}'
    result = json.loads((root/'result.json').read_text());assert result['ok'], result
    gui.wait(timeout=15);assert gui.returncode==0
    close_log=(root/'gui.log').read_text()
    for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:
        assert marker in close_log,marker
    params = {'dev':result['dev']}
    assert rpc('get_lcd_theme',params)==result['theme']
    wait(lambda:rpc('get_lcd_state',params).get('persistence',{}).get('saved') is True,'theme saved')
    before = json.loads((root/'stats.json').read_text())['lcd_video_packets']
    wait(lambda:json.loads((root/'stats.json').read_text())['lcd_video_packets']>before,'video stream continues after GUI exit')
    stop_fixture();start_fixture()
    wait(lambda:rpc('get_lcd_theme',params)==result['theme'],'weather restart replay')
    assert rpc('list_profiles')==result['profiles'],'slot ranges survive restart'
    state = rpc('get_lcd_state',params);assert state['source']=='Theme' and not state['last_error'],state
    first = json.loads((root/'stats.json').read_text())['lcd_video_packets']
    wait(lambda:json.loads((root/'stats.json').read_text())['lcd_video_packets']>first,'weather stream resumes after restart')
    result.update(restart_replay=True,profiles_restart=True,stream_after_gui_exit=True,normal_close_joined=True,gui_exit=gui.returncode)
    (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print('PASS native Weather WebKitGTK → Unix IPC → MockBackend; evidence:',root)
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
