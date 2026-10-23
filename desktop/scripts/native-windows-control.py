#!/usr/bin/env python3
"""Native Windows control import: private Xvfb, session D-Bus and MockBackend."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='lconnect-native-windows-control-'))
 env=dict(os.environ,LCONNECT_ARCHIVE_ISOLATED='1',LCONNECT_CONTROL_ROOT=str(root),
          XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),
          XDG_DATA_HOME=str(root/'data'),DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
 for key in list(env):
  if key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS'] or key.startswith('LCONNECT_NATIVE_'):env.pop(key,None)
 try:
  code=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb',*sys.argv[1:]],env=env)
 finally:
  target=root/'doc'
  def mounted():return any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines())
  if mounted():
   cleanup=subprocess.run(['fusermount3','-uz','--',str(target)],capture_output=True)
   if cleanup.returncode and mounted():raise RuntimeError(f'Private document mount remains: {target}')
 raise SystemExit(code)
assert os.environ.get('LCONNECT_ARCHIVE_ISOLATED')=='1'
root=Path(os.environ['LCONNECT_CONTROL_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),LCONNECT_NATIVE_ARCHIVE_PORTAL='1',LCONNECT_NATIVE_WINDOWS_PORTAL='1',LCONNECT_NATIVE_DESKTOP_LANGUAGE='en-US' if '--english' in sys.argv else 'ru-RU',GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
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
 (root/'stop').unlink(missing_ok=True);(root/'lconnectd.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}');wait(lambda:((root/'lconnectd.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==1,'mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0


def close_gui(gui,name):
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker

def result_for(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native Windows control workflow',seconds=120)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 return result

try:
 (root/'source controls.zip').write_bytes((repo/'crates/lconnect-profile/tests/fixtures/Control.zip').read_bytes())
 launch(['openbox'],'wm');start_fixture()
 display=rpc('get_display_settings');display['interface_language']='en-US' if '--english' in sys.argv else 'ru-RU'
 rpc('set_display_settings',{'settings':display})
 gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_WINDOWS_CONTROL='1'))
 first=result_for(gui);close_gui(gui,'gui');(root/'result.json').unlink()
 stop_fixture();start_fixture()
 assert rpc('list_profiles')==first['library']
 bank=rpc('get_cooling_modes',{'dev':first['dev']})
 for key in ['revision','fan','pump']:assert bank[key]==first['bank'][key],key
 assert rpc('get_lcd_state',{'dev':first['dev']})['requested']==first['lcd']
 (root/'source controls.zip').unlink()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui-recovery',dict(env,LCONNECT_NATIVE_WINDOWS_CONTROL='1',LCONNECT_NATIVE_WINDOWS_RECOVER='1'))
 second=result_for(gui);close_gui(gui,'gui-recovery')
 assert second['operation']==first['operation'] and second['library']==first['library']
 result=dict(initial=first,recovery=second,normal_close=True,desktop_and_daemon_restart=True)
 events=[json.loads(line) for line in (root/'archive-portal-events.jsonl').read_text().splitlines()]
 assert len(events)==2 and all(e['method']=='OpenFile' and e['filters']==[['Windows L-Connect3 ZIP',[[0,'*.zip']]]] for e in events),events
 assert all(e['title']==('Import Windows L-Connect3 settings' if '--english' in sys.argv else 'Импорт настроек Windows L-Connect3') for e in events),events
 result.update(language='en-US' if '--english' in sys.argv else 'ru-RU',portal_filters=True,source_removed_before_recovery=True)
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print('PASS native Windows control import/WebKitGTK/private FileChooser/Unix IPC/MockBackend/restart:',root)
finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
