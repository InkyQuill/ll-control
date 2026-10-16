#!/usr/bin/env python3
"""Private WebKit/Tauri FPS settings and restart, zero-device MockBackend only."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside' not in sys.argv:
 env=dict(os.environ,LCONNECT_FPS_ISOLATED='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside'],env=env))
assert os.environ.get('LCONNECT_FPS_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-fps-'));(root/'fixture-marker').write_text('MockBackend; zero device; private settings\n')
log_path=root/'Настройки game.csv';log_path.write_text('fps,frametime,cpu_load,cpu_power,gpu_load,cpu_temp,gpu_temp,gpu_core_clock,gpu_mem_clock,gpu_vram_used,gpu_power,ram_used,swap_used,process_rss,cpu_mhz,elapsed\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_FPS='1',LCONNECT_NATIVE_FPS_LOG=str(log_path),LCONNECT_NATIVE_SPEC='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None);env.pop('LCONNECT_MANGOHUD_PER_FRAME_LOG',None)
processes=[];logs=[]
def launch(args,name):
 stream=(root/(name+'.log')).open('w');logs.append(stream);p=subprocess.Popen(args,env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p
def wait(test,label,seconds=40):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(label)
  time.sleep(.05)
def rpc(method):
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(10);s.connect(str(root/'lconnectd.sock'));stream=s.makefile('rwb');stream.write((json.dumps(dict(jsonrpc='2.0',id=1,method=method,params={}))+'\n').encode());stream.flush()
  while True:
   result=json.loads(stream.readline())
   if result.get('id')==1:assert 'error' not in result,result;return result['result']
try:
 launch(['openbox'],'wm');results=[]
 for phase in ['1','2']:
  for name in ['stop','lconnectd.sock','stats.json','result.json']:(root/name).unlink(missing_ok=True)
  env['LCONNECT_NATIVE_DESKTOP_PHASE']=phase
  fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture-'+phase)
  wait(lambda:(root/'lconnectd.sock').exists() and (root/'stats.json').exists(),'private fixture')
  assert rpc('list_devices')==[]
  gui=launch([repo/'target/debug/lconnect-gui'],'gui-'+phase)
  wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native FPS workflow',90)
  result=json.loads((root/'result.json').read_text());assert result['ok'],result;results.append(result)
  stats=json.loads((root/'stats.json').read_text());assert stats['devices_known']==0 and stats['hid_write_count']==0,stats
  window=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(gui.pid),'--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
  subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
  close=(root/('gui-'+phase+'.log')).read_text();assert 'native close: all workers joined' in close
  (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0
 assert log_path.read_text()=='fps,frametime,cpu_load,cpu_power,gpu_load,cpu_temp,gpu_temp,gpu_core_clock,gpu_mem_clock,gpu_vram_used,gpu_power,ram_used,swap_used,process_rss,cpu_mhz,elapsed\n','settings mutated external log'
 (root/'acceptance.json').write_text(json.dumps(dict(ok=True,phases=results,no_hardware=True,no_external_log_writes=True),ensure_ascii=False,indent=2)+'\n')
 print(root,flush=True)
finally:
 for p in reversed(processes):
  if p.poll() is None:
   try:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
   except ProcessLookupError:pass
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
 for f in logs:f.close()
