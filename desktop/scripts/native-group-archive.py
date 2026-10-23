#!/usr/bin/env python3
"""Native archives: private Xvfb, session D-Bus, FileChooser fixture and MockBackend."""
import hashlib,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='lconnect-native-group-archive-'))
 env=dict(os.environ,LCONNECT_GROUP_ISOLATED='1',LCONNECT_GROUP_ROOT=str(root),
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
assert os.environ.get('LCONNECT_GROUP_ISOLATED')=='1'
root=Path(os.environ['LCONNECT_GROUP_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_NATIVE_DESKTOP_PHASE='en' if '--english' in sys.argv else 'ru',LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_GROUP_KIND='fan_and_lighting' if '--fan' in sys.argv else 'screen',LCONNECT_NATIVE_ARCHIVE_PORTAL='1',LCONNECT_NATIVE_GROUP_PORTAL='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
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

try:
 launch(['openbox'],'wm');start_fixture()
 display=rpc('get_display_settings');display['interface_language']='en-US' if '--english' in sys.argv else 'ru-RU';rpc('set_display_settings',{'settings':display})
 gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_GROUP_ARCHIVE='1'))
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native archive workflow',seconds=120);assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 publication_root=root/'settings/import-publications';assert publication_root.is_dir() and not list(publication_root.iterdir()),'completed GUI transfer left ownership records or recovery notices'
 exported=root/'exported group.lcg';assert exported.read_bytes().startswith(b'LCGROUP1') and exported.stat().st_size>0
 events=[json.loads(line) for line in (root/'archive-portal-events.jsonl').read_text().splitlines()];assert [e['method'] for e in events]==['SaveFile','OpenFile'],events
 assert [e['title'] for e in events]==(['Export LL-Control settings group', 'Import LL-Control settings group'] if '--english' in sys.argv else ['Экспорт группы настроек LL-Control', 'Импорт группы настроек LL-Control']),events
 assert all(e['filters']==[[('LL-Control settings group' if '--english' in sys.argv else 'Группа настроек LL-Control'),[[0,'*.lcg']]]] for e in events),events
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
 # windowquit sends WM_DELETE_WINDOW (not XDestroyWindow/windowclose). This
 # qualifies the application's normal close path independently of WM key grabs.
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/'gui.log').read_text();assert 'native close: request admitted' in close_log and 'native close: group worker joined (true)' in close_log and 'native close: all workers joined' in close_log
 stop_fixture();start_fixture();library=rpc('list_profiles');assert library==result['library']
 recovered=rpc('read_group_import',{'dev':result['dev'],'group':result['group'],'operation_id':result['operation']});assert recovered['receipt']==result['receipt'] and recovered['selected_content_current']
 if result['group']=='screen':assert rpc('get_lcd_state',{'dev':result['dev']})['requested']['brightness_percent']==64
 else:assert rpc('get_cooling_state',{'dev':result['dev']})['fan']['requested']['percent']==64
 preview=bytes(rpc('preview_lcd_theme',{'theme':next(p for p in library['profiles'] if p['slot']==7)['content']['value']}));assert len(preview)>100
 assert not list(publication_root.iterdir()),'restart left publication ownership records'
 result.update(publication_journal_retired=True,post_restart_preview_bytes=len(preview),post_restart_preview_sha256=hashlib.sha256(preview).hexdigest(),restart_replay=True,normal_close=True,close_method='WM_DELETE_WINDOW via xdotool windowquit',portal_events=events,exported_bytes=exported.stat().st_size,gui_exit=gui.returncode);(root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('PASS native full-group FileChooser/WebKitGTK/Unix IPC/MockBackend/restart:',root)
finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
