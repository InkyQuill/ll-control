#!/usr/bin/env python3
"""Native ScreenRecord acceptance with synthetic recording, private display and MockBackend."""
import json, os, signal, socket, subprocess, sys, tempfile, time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    env=dict(os.environ,LCONNECT_KEYBOARD_ISOLATED='1',LCONNECT_NATIVE_RECORDING_LIMIT='1' if '--full-limit' in sys.argv else '0');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
    raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb'],env=env))
assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-recording-'));(root/'fixture-marker').write_text('MockBackend and synthetic recording only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1')
env.pop('WAYLAND_DISPLAY',None)
(root/'native-tmp').mkdir();env['TMPDIR']=str(root/'native-tmp')
processes=[];logs=[]
def launch(command,name,child_env=env):
    log=(root/(name+'.log')).open('w');logs.append(log)
    child=subprocess.Popen(command,env=child_env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(child);return child
def wait(predicate,why,seconds=15):
    deadline=time.monotonic()+seconds
    while not predicate():
        if time.monotonic()>deadline:raise TimeoutError(why)
        time.sleep(.02)
def rpc(method,params=None):
    with socket.socket(socket.AF_UNIX) as conn:
        conn.settimeout(10);conn.connect(str(root/'lconnectd.sock'));stream=conn.makefile('rwb')
        stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
        while True:
            result=json.loads(stream.readline())
            if result.get('id')==1:
                assert 'error' not in result,result
                return result['result']
try:
    launch(['openbox'],'wm')
    fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
    wait(lambda:(root/'lconnectd.sock').exists() or fixture.poll() is not None,'private daemon');assert fixture.poll() is None
    display=rpc('get_display_settings');display['interface_language']='ru-RU';rpc('set_display_settings',{'settings':display})
    gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_RECORDING='1'))
    full_limit=env.get('LCONNECT_NATIVE_RECORDING_LIMIT')=='1'
    deadline=time.monotonic()+(300 if full_limit else 60);close_sent=False
    while gui.poll() is None:
        if time.monotonic()>deadline:raise TimeoutError('native recording workflow')
        result_file=root/'result.json'
        if result_file.exists() and not close_sent:
            result=json.loads(result_file.read_text());assert result['ok'],result
            window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
            subprocess.run(['xdotool','windowactivate','--sync',window],env=env,check=True,timeout=5)
            subprocess.run(['xdotool','key','--clearmodifiers','alt+F4'],env=env,check=True,timeout=5);close_sent=True
        time.sleep(.02)
    gui.wait(timeout=5);assert gui.returncode==0
    result=json.loads((root/'result.json').read_text());assert result['ok'] and close_sent
    assert not list((root/'native-tmp').iterdir()), 'native staging remains after close'
    assert result['saved_key'] in [clip['key'] for clip in rpc('list_prepared_clips')]
    result.update(normal_close_exit=gui.returncode)
    (root/'acceptance.json').write_text(json.dumps(result,indent=2))
    print('PASS native recording; evidence:',root)
finally:
    (root/'stop').touch()
    for child in reversed(processes):
        try:os.killpg(child.pid,signal.SIGTERM)
        except ProcessLookupError:pass
        try:child.wait(timeout=5)
        except subprocess.TimeoutExpired:pass
        try:os.killpg(child.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        child.wait(timeout=5)
    for log in logs:log.close()
    print('Fixture directory:',root)
