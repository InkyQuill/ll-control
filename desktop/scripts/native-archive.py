#!/usr/bin/env python3
"""Native archives: private Xvfb, session D-Bus, FileChooser fixture and MockBackend."""
import hashlib,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='llcontrol-native-archive-'))
 env=dict(os.environ,LLCONTROL_ARCHIVE_ISOLATED='1',LLCONTROL_ARCHIVE_ROOT=str(root),
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
assert os.environ.get('LLCONTROL_ARCHIVE_ISOLATED')=='1'
root=Path(os.environ['LLCONTROL_ARCHIVE_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LLCONTROL_NATIVE_DESKTOP_PHASE='en' if '--english' in sys.argv else 'ru',LLCONTROL_SMOKE_ROOT=str(root),LLCONTROL_NATIVE_ARCHIVE_PORTAL='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
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
  conn.settimeout(20);conn.connect(str(root/'llcontrold.sock'));stream=conn.makefile('rwb');stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:assert 'error' not in reply,reply;return reply['result']

def start_fixture():
 global fixture
 (root/'stop').unlink(missing_ok=True);(root/'llcontrold.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}');wait(lambda:((root/'llcontrold.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==1,'mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0

try:
 launch(['openbox'],'wm');start_fixture()
 display=rpc('get_display_settings');display['interface_language']='en-US' if '--english' in sys.argv else 'ru-RU';rpc('set_display_settings',{'settings':display})
 gui=launch([repo/'target/debug/llcontrol-gui'],'gui',dict(env,LLCONTROL_NATIVE_ARCHIVE='1'))
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native archive workflow',seconds=120);assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 exported=root/'exported archive.lcp';assert exported.read_bytes().startswith(b'LCPROF01') and exported.stat().st_size>0
 events=[json.loads(line) for line in (root/'archive-portal-events.jsonl').read_text().splitlines()];assert [e['method'] for e in events]==['SaveFile','SaveFile','OpenFile'],events
 assert [e['title'] for e in events]==(['Export LL-Control profile', 'Export LL-Control profile', 'Import LL-Control profile'] if '--english' in sys.argv else ['Экспорт профиля LL-Control', 'Экспорт профиля LL-Control', 'Импорт профиля LL-Control']),events
 assert all(e['filters']==[[('LL-Control profile' if '--english' in sys.argv else 'Профиль LL-Control'),[[0,'*.lcp']]]] for e in events),events
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowactivate','--sync',window],env=env,check=True,timeout=5);subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/'gui.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker
 journal=root/'settings/import-publications'
 assert journal.is_dir() and not any(p.is_dir() for p in journal.iterdir())
 stop_fixture();start_fixture();library=rpc('list_profiles');assert library==result['library'];assert rpc('get_lcd_theme',{'dev':result['dev']}) is None
 recovered=rpc('profile_archive',{'request':{'action':'status','id':result['operation']}});assert recovered['state']['phase']=='imported' and recovered['state']['committed_revision']==2
 imported=next(p for p in library['profiles'] if p['slot']==7)['content']['value'];preview=bytes(rpc('preview_lcd_theme',{'theme':imported}));assert len(preview)>100
 assert rpc('get_lcd_theme',{'dev':result['dev']}) is None
 receipt=next(r for r in library['archive_imports'] if r['id']==result['operation'])
 assert receipt['source_sha256']==exported.read_bytes()[12:44].hex()
 assert not any(p.is_dir() for p in journal.iterdir())
 result.update(publication_journal_retired=True,receipt_manifest_sha256=receipt['source_sha256'],post_restart_preview_bytes=len(preview),post_restart_preview_sha256=hashlib.sha256(preview).hexdigest(),restart_replay=True,normal_close=True,portal_events=events,exported_bytes=exported.stat().st_size,gui_exit=gui.returncode);(root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('PASS native archive FileChooser/WebKitGTK/Unix IPC/MockBackend/restart:',root)
finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
