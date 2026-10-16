#!/usr/bin/env python3
"""Read-only GUI preflight in a private Xvfb/session/XDG with MockBackend."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside' not in sys.argv:
 env=dict(os.environ,LCONNECT_FIRMWARE_ISOLATED='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside',*sys.argv[1:]],env=env))
assert os.environ.get('LCONNECT_FIRMWARE_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-firmware-'));(root/'fixture-marker').write_text('MockBackend only\n')
language='ru-RU' if '--russian' in sys.argv else 'en-US';has_date='--with-date' in sys.argv
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_FIRMWARE='1',LCONNECT_NATIVE_SPEC='0',LCONNECT_NATIVE_FIRMWARE_DATE='1' if has_date else '0',LCONNECT_NATIVE_DESKTOP_PHASE=('ru' if language=='ru-RU' else 'en')+('-with-date' if has_date else '-without-date'),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
processes=[];logs=[];fixture=None
def launch(command,name):
 log=(root/(name+'.log')).open('w');logs.append(log);process=subprocess.Popen(command,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(process);return process
def wait(test,label,seconds=30):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(label)
  time.sleep(.05)
def rpc(method,params=None):
 with socket.socket(socket.AF_UNIX) as conn:
  conn.settimeout(15);conn.connect(str(root/'lconnectd.sock'));stream=conn.makefile('rwb');stream.write((json.dumps(dict(jsonrpc='2.0',id=1,method=method,params=params or {}))+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:assert 'error' not in reply,reply;return reply['result']
try:
 launch(['openbox'],'wm');fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
 wait(lambda:(root/'lconnectd.sock').exists() and (root/'stats.json').exists(),'fixture socket')
 settings=rpc('get_display_settings');settings['interface_language']=language;rpc('set_display_settings',{'settings':settings})
 wait(lambda:len(rpc('list_devices'))==1,'mock attachment')
 gui=launch([repo/'target/debug/lconnect-gui'],'gui')
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native firmware workflow',90)
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 wait(lambda:json.loads((root/'stats.json').read_text()).get('firmware_queries')==1,'one firmware query')
 stats=json.loads((root/'stats.json').read_text());assert stats['firmware_update_writes']==0 and stats['firmware_activations']==0,stats
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(gui.pid),'--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/'gui.log').read_text()
 for marker in ['native close: request admitted','native close: all workers joined']:assert marker in close_log,marker
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0
 result.update(firmware_queries=1,firmware_update_writes=0,firmware_activations=0,normal_close=True,hardware_access=False,host_installation=False)
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('PASS native firmware preflight/explicit read/EN-RU/one cmd134/no flash/no settings mutation:',root)
finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for process in reversed(processes):
  if process.poll() is None:
   try:process.wait(timeout=3)
   except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGTERM);process.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
