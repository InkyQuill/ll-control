#!/usr/bin/env python3
"""Native ScreenRecord acceptance with synthetic recording, private display and MockBackend."""
import json, os, signal, socket, subprocess, sys, tempfile, time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    # Portal activation inherits the bus environment, not the per-GUI one.
    # Retain the private session directory as diagnostic data; unmount only its
    # own document portal after the bus has exited.
    language=sys.argv[sys.argv.index('--language')+1] if '--language' in sys.argv else 'ru-RU'
    assert language in {'ru-RU','en-US'}, 'supported language required'
    assert '--full-limit' not in sys.argv or language=='ru-RU', 'full-limit workflow currently supports ru-RU only'
    session=Path(tempfile.mkdtemp(prefix='llcontrol-recording-session-'));session.chmod(0o700)
    inherited={key:value for key,value in os.environ.items() if not key.startswith('LLCONTROL_NATIVE_')}
    env=dict(inherited,LLCONTROL_KEYBOARD_ISOLATED='1',LLCONTROL_RECORDING_LANGUAGE=language,LLCONTROL_NATIVE_RECORDING_LIMIT='1' if '--full-limit' in sys.argv else '0',XDG_RUNTIME_DIR=str(session),XDG_CONFIG_HOME=str(session/'config'),XDG_DATA_HOME=str(session/'data'),XDG_CACHE_HOME=str(session/'cache'),DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={session}/no-system-bus')
    for key in ['DISPLAY','WAYLAND_DISPLAY','AT_SPI_BUS_ADDRESS','DBUS_SESSION_BUS_ADDRESS']:env.pop(key,None)
    try:result=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb'],env=env)
    finally:
        target=session/'doc'
        if any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines()):
            subprocess.run(['fusermount3','-uz','--',str(target)],check=True,timeout=5)
    raise SystemExit(result)
assert os.environ.get('LLCONTROL_KEYBOARD_ISOLATED')=='1'
language=os.environ['LLCONTROL_RECORDING_LANGUAGE']
root=Path(tempfile.mkdtemp(prefix='llcontrol-native-recording-'));(root/'fixture-marker').write_text('MockBackend and synthetic recording only\n')
env=dict(os.environ,LLCONTROL_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='0',AT_SPI_BUS_ADDRESS=f'unix:path={root}/a11y.sock')
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
        conn.settimeout(10);conn.connect(str(root/'llcontrold.sock'));stream=conn.makefile('rwb')
        stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
        while True:
            result=json.loads(stream.readline())
            if result.get('id')==1:
                assert 'error' not in result,result
                return result['result']
try:
    launch(['dbus-daemon','--config-file=/usr/share/defaults/at-spi2/accessibility.conf',f'--address={env["AT_SPI_BUS_ADDRESS"]}','--nofork'],'accessibility-bus')
    wait(lambda:(root/'a11y.sock').exists(),'private accessibility bus')
    launch(['openbox'],'wm')
    fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
    wait(lambda:(root/'llcontrold.sock').exists() or fixture.poll() is not None,'private daemon');assert fixture.poll() is None
    display=rpc('get_display_settings');display['interface_language']=language;rpc('set_display_settings',{'settings':display})
    gui=launch([repo/'target/debug/llcontrol-gui'],'gui',dict(env,LLCONTROL_NATIVE_RECORDING='1'))
    full_limit=env.get('LLCONTROL_NATIVE_RECORDING_LIMIT')=='1'
    deadline=time.monotonic()+(300 if full_limit else 90);close_sent=False;key_sequence=0;pointer_moves=[]
    while gui.poll() is None:
        if time.monotonic()>deadline:raise TimeoutError('native recording workflow')
        step=root/'keyboard-step.json'
        if step.exists():
            request=json.loads(step.read_text())
            if request['sequence']>key_sequence:
                assert request==dict(sequence=key_sequence+1,key='Home',probe='capture_region_pointer' if key_sequence<3 else 'video_region_pointer'),request
                window=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(gui.pid),'--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
                subprocess.run(['xdotool','windowactivate','--sync',window],env=env,check=True,timeout=5)
                query="""
import gi,json,os,sys
from pathlib import Path
assert os.environ['AT_SPI_BUS_ADDRESS']=='unix:path='+os.environ['LLCONTROL_SMOKE_ROOT']+'/a11y.sock'
assert (Path(os.environ['LLCONTROL_SMOKE_ROOT'])/'fixture-marker').is_file()
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
Atspi.set_timeout(500,500);Atspi.init()
def find(node,depth=0):
 if depth>30:return None
 if node.get_name()==sys.argv[1]:
  b=Atspi.Component.get_extents(node.get_component_iface(),Atspi.CoordType.SCREEN)
  return dict(x=b.x,y=b.y,width=b.width,height=b.height)
 for i in range(node.get_child_count()):
  child=node.get_child_at_index(i)
  if child:
   result=find(child,depth+1)
   if result:return result
result=find(Atspi.get_desktop(0));assert result,'native region button missing'
print(json.dumps(result))
"""
                labels=('Выбор области захвата на кадре','Выбор области видео') if language=='ru-RU' else ('Capture region selection on frame','Video region selection')
                box=json.loads(subprocess.check_output([sys.executable,'-c',query,labels[0 if key_sequence<3 else 1]],env=env,text=True,timeout=10))
                assert box['width']>10 and box['height']>10,box
                def mouse(*args):subprocess.run(['xdotool',*map(str,args)],env=env,check=True,timeout=5);time.sleep(.08)
                def move(x,y):mouse('mousemove','--sync',round(box['x']+box['width']*x),round(box['y']+box['height']*y))
                move(.2,.2);mouse('mousedown',1)
                if (request['sequence']-1)%3==0:move(1.05,1.05)
                else:
                    move(.6,.6)
                    if (request['sequence']-1)%3==1:move(.8,.8)
                mouse('mouseup',1);move(.4,.4)
                mouse('key','--clearmodifiers','Home')
                pointer_moves.append(dict(sequence=request['sequence'],bounds=box))
                key_sequence=request['sequence']
        result_file=root/'result.json'
        if result_file.exists() and not close_sent:
            result=json.loads(result_file.read_text());assert result['ok'],result
            window=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(gui.pid),'--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
            subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);close_sent=True
        time.sleep(.02)
    gui.wait(timeout=5);assert gui.returncode==0
    result=json.loads((root/'result.json').read_text());assert result['ok'] and close_sent
    assert not list((root/'native-tmp').iterdir()), 'native staging remains after close'
    assert result['saved_key'] in [clip['key'] for clip in rpc('list_prepared_clips')]
    if not full_limit:
        assert result['trusted_region_steps']==key_sequence==6,result
        assert result['language']==language,result
    result.update(normal_close_exit=gui.returncode,pointer_moves=pointer_moves)
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
