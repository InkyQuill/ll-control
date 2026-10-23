#!/usr/bin/env python3
"""Native basic editor bank workflow: private Xvfb, session D-Bus and MockBackend."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='lconnect-native-basic-editors-'));root.chmod(0o700)
 env=dict(os.environ,LCONNECT_MOCK_EXPORT_FRAMES='1',LCONNECT_ARCHIVE_ISOLATED='1',LCONNECT_BASIC_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
 for key in list(env):
  if key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS'] or key.startswith('LCONNECT_NATIVE_'):env.pop(key,None)
 try:code=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb',*(['--english'] if '--english' in sys.argv else [])],env=env)
 finally:
  target=root/'doc'
  if any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines()):subprocess.run(['fusermount3','-uz','--',str(target)],check=True,timeout=10)
 raise SystemExit(code)
assert os.environ.get('LCONNECT_ARCHIVE_ISOLATED')=='1'
root=Path(os.environ['LCONNECT_BASIC_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),LCONNECT_NATIVE_DESKTOP_LANGUAGE='en-US' if '--english' in sys.argv else 'ru-RU',GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
processes=[];logs=[];fixture=None

def launch(cmd,name,process_env=env):
 log=(root/f'{name}.log').open('w');logs.append(log);p=subprocess.Popen(cmd,env=process_env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p

def wait(test,why,seconds=20):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(why)
  time.sleep(.05)

def rpc(method,params=None):
 with socket.socket(socket.AF_UNIX) as conn:
  conn.settimeout(20);conn.connect(str(root/'lconnectd.sock'));stream=conn.makefile('rwb');stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:assert 'error' not in reply,reply;return reply['result']

def start_fixture():
 global fixture
 (root/'lcd-frame.json').unlink(missing_ok=True);
 (root/'stop').unlink(missing_ok=True);(root/'lconnectd.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}');wait(lambda:((root/'lconnectd.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==1,'mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0


def close_gui(gui,name):
 def visible():
  found=subprocess.run(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,capture_output=True,timeout=5)
  return found.stdout.splitlines() if found.returncode==0 else []
 wait(lambda:gui.poll() is not None or bool(visible()),'mapped native window before Close')
 assert gui.poll() is None,'GUI exited before explicit Close'
 window=visible()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker

def result_for(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native basic editor workflow',seconds=120)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 return result

try:
 launch(['openbox'],'wm');start_fixture()
 display=rpc('get_display_settings');display['interface_language']='en-US' if '--english' in sys.argv else 'ru-RU'
 rpc('set_display_settings',{'settings':display})
 theme=json.loads((repo/'desktop/tests/fixtures/basic-editor-theme.json').read_text())
 rpc('edit_profile',{'expected_revision':0,'edit':{'action':'save','profile':{'slot':0,'name':'Owned banks','content':{'kind':'screen','value':theme}}}})
 gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_BASIC_EDITORS='1'))
 first=result_for(gui);assert first['previews']==14 and first['divider_edit']
 wait(lambda:(root/'lcd-frame.json').exists(),'actual mock divider frame')
 jpeg=bytes(json.loads((root/'lcd-frame.json').read_text())['jpeg']);(root/'divider-rendered.jpg').write_bytes(jpeg)
 decoded=subprocess.run(['ffmpeg','-nostdin','-v','error','-i',str(root/'divider-rendered.jpg'),'-threads','1','-pix_fmt','rgb24','-f','rawvideo','-'],check=True,capture_output=True,timeout=15).stdout
 assert len(decoded)==480*480*3
 green=lambda x,y:decoded[(y*480+x)*3+1]
 assert green(200,431)>green(200,421)+80,'edited divider missing from actual HID JPEG'
 close_gui(gui,'gui');(root/'result.json').unlink()
 stop_fixture();start_fixture()
 assert rpc('list_profiles')==first['library']
 assert rpc('get_lcd_theme',{'dev':first['dev']})==first['theme']
 gui=launch([repo/'target/debug/lconnect-gui'],'gui-recovery',dict(env,LCONNECT_NATIVE_BASIC_EDITORS='1'))
 second=result_for(gui);close_gui(gui,'gui-recovery')
 assert second['recovery'] and second['theme']==first['theme'] and second['library']==first['library']
 result=dict(initial=first,recovery=second,normal_close=True,actual_divider_jpeg=True,desktop_and_daemon_restart=True,language='en-US' if '--english' in sys.argv else 'ru-RU')
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print('PASS native basic editors/WebKitGTK/Unix IPC/MockBackend/14 previews/Save/Apply/restart:',root)
finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
