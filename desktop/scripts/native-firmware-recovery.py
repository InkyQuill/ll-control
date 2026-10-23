#!/usr/bin/env python3
"""Saved-state inspection only: private Xvfb/D-Bus/XDG, no device or real HID."""
import errno,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
def release_document_mount(session):
 # Only a mount inside this runner's own random private runtime directory.
 target=str(Path(session)/'doc')
 for line in Path('/proc/self/mountinfo').read_text().splitlines():
  if line.split()[4]==target:
   result=subprocess.run(['fusermount3','-uz','--',target],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
   # The portal may unmount between mountinfo and fusermount execution.
   if result.returncode and any(row.split()[4]==target for row in Path('/proc/self/mountinfo').read_text().splitlines()):
    raise RuntimeError('Private document portal mount could not be released')
 # Portal shutdown and FUSE unmount can finish just after the bus exits.
 deadline=time.monotonic()+5
 while True:
  try:os.stat(target);return
  except FileNotFoundError:return
  except OSError as error:
   if error.errno!=errno.ENOTCONN or time.monotonic()>deadline:raise
   time.sleep(.05)
if '--inside' not in sys.argv:
 # Activation services inherit the bus launch environment, not each GUI case.
 # Give them private XDG paths too, and no route to the host system bus.
 with tempfile.TemporaryDirectory(prefix='lconnect-recovery-session-') as session:
  env=dict(os.environ,LCONNECT_RECOVERY_ISOLATED='1',XDG_RUNTIME_DIR=session,XDG_CONFIG_HOME=str(Path(session)/'config'),XDG_DATA_HOME=str(Path(session)/'data'),XDG_CACHE_HOME=str(Path(session)/'cache'),DBUS_SYSTEM_BUS_ADDRESS='unix:path='+str(Path(session)/'no-system-bus'))
  env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
  try:result=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside'],env=env)
  finally:release_document_mount(session)
 raise SystemExit(result)
assert os.environ.get('LCONNECT_RECOVERY_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-recovery-'));results=[]
def wait(test,label,seconds=50):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(label)
  time.sleep(.05)
def snapshot(directory):return {str(p.relative_to(directory)):p.read_bytes() for p in directory.rglob('*') if p.is_file()}
for mode,language,sealed in [('none','en-US',True),('guarded','en-US',True),('guard_only','ru-RU',False),('unreadable','ru-RU',False)]:
 case=root/mode;case.mkdir();(case/'fixture-marker').write_text('MockBackend; zero devices; saved files only\n')
 env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(case),LCONNECT_NATIVE_SPEC='1',LCONNECT_NATIVE_RECOVERY='1',LCONNECT_NATIVE_RECOVERY_STATE=mode,LCONNECT_NATIVE_RECOVERY_RUSSIAN='1' if language=='ru-RU' else '0',LCONNECT_NATIVE_RECOVERY_SEALED='1' if sealed else '0',LCONNECT_NATIVE_DESKTOP_PHASE=f'{mode}:{language}:{"sealed" if sealed else "open"}',XDG_RUNTIME_DIR=str(case),XDG_CONFIG_HOME=str(case/'config'),XDG_DATA_HOME=str(case/'data'),XDG_CACHE_HOME=str(case/'cache'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
 processes=[];logs=[];fixture=None
 def launch(args,name):
  log=(case/(name+'.log')).open('w');logs.append(log);p=subprocess.Popen(args,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p
 try:
  launch(['openbox'],'wm');fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
  wait(lambda:(case/'lconnectd.sock').exists() and (case/'stats.json').exists() or fixture.poll() is not None,'private fixture')
  assert fixture.poll() is None,'fixture failed'
  before=snapshot(case/'settings');gui=launch([repo/'target/debug/lconnect-gui'],'gui')
  wait(lambda:(case/'result.json').exists() or gui.poll() is not None,'native recovery read')
  result=json.loads((case/'result.json').read_text());assert result['ok'],result
  stats=json.loads((case/'stats.json').read_text());assert stats['devices_known']==0 and stats['hid_write_count']==0 and stats['firmware_queries']==0 and stats['firmware_update_writes']==0 and stats['firmware_activations']==0,stats
  assert snapshot(case/'settings')==before,'saved firmware files changed'
  window=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(gui.pid),'--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
  subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
  assert 'native close: all workers joined' in (case/'gui.log').read_text()
  (case/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0
  assert snapshot(case/'settings')==before,'saved files changed on close'
  result.update(hid_writes=0,firmware_queries=0,saved_files_unchanged=True,normal_close=True);results.append(result)
 finally:
  (case/'stop').touch()
  for p in reversed(processes):
   if p.poll() is None:
    os.killpg(p.pid,signal.SIGTERM)
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  for log in logs:log.close()
(root/'acceptance.json').write_text(json.dumps(dict(ok=True,cases=results,no_hardware=True),ensure_ascii=False,indent=2)+'\n');print('PASS native saved firmware recovery:',root)
