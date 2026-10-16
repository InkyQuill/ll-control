#!/usr/bin/env python3
"""Native screenshot acceptance with synthetic portal, private display and MockBackend."""
import json, os, signal, socket, struct, subprocess, sys, tempfile, time, zlib
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    env=dict(os.environ,LCONNECT_KEYBOARD_ISOLATED='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
    raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb'],env=env))
assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-screenshot-'));(root/'fixture-marker').write_text('MockBackend and synthetic portal only\n')
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
rows=(b'\x00'+bytes([255,0,0])*32+bytes([0,0,255])*32)*32
(root/'capture.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',64,32,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(rows))+chunk(b'IEND',b''))
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1')
env.pop('WAYLAND_DISPLAY',None)
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
def portal():return json.loads((root/'portal.json').read_text())
try:
    launch(['openbox'],'wm')
    fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
    wait(lambda:(root/'lconnectd.sock').exists() or fixture.poll() is not None,'private daemon');assert fixture.poll() is None
    provider=launch([sys.executable,repo/'desktop/scripts/native-screenshot-portal.py'],'portal')
    wait(lambda:(root/'portal-ready').exists() or provider.poll() is not None,'synthetic portal');assert provider.poll() is None
    gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_SCREENSHOT='1',LCONNECT_NATIVE_MEDIA='1'))
    sequence=0;deadline=time.monotonic()+60;close_sent=False
    while gui.poll() is None:
        if time.monotonic()>deadline:raise TimeoutError('native screenshot workflow')
        step_file=root/'media-step.json'
        if step_file.exists():
            step=json.loads(step_file.read_text())
            if step['sequence']>sequence:
                assert step['sequence']==sequence+1;sequence=step['sequence']
                expected={'screenshot_cancel_ready':3,'screenshot_close_ready':4}[step['action']]
                wait(lambda:len(portal()['requests'])==expected,'owned request reached portal')
                if expected==4:assert portal()['requests'][2]['path'] in portal()['closed']
                (root/'media-ack.tmp').write_text(str(sequence));(root/'media-ack.tmp').replace(root/'media-ack')
        result_file=root/'result.json'
        if result_file.exists() and not close_sent:
            result=json.loads(result_file.read_text());assert result['ok'],result
            window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
            subprocess.run(['xdotool','windowactivate','--sync',window],env=env,check=True,timeout=5)
            subprocess.run(['xdotool','key','--clearmodifiers','alt+F4'],env=env,check=True,timeout=5);close_sent=True
        time.sleep(.02)
    gui.wait(timeout=5);assert gui.returncode==0
    result=json.loads((root/'result.json').read_text());assert result['ok'] and close_sent
    observed=portal();assert len(observed['requests'])==4
    assert all(r['path'] in observed['closed'] for r in observed['requests'][2:]),observed
    png=bytes(rpc('read_theme_asset_for_edit',{'key':result['saved_key']}));assert png[:8]==b'\x89PNG\r\n\x1a\n'
    assert struct.unpack('>II',png[16:24])==(480,480)
    result.update(portal=observed,saved_png_bytes=len(png),normal_close_exit=gui.returncode)
    (root/'acceptance.json').write_text(json.dumps(result,indent=2))
    print('PASS native screenshot; evidence:',root)
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
