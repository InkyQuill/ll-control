#!/usr/bin/env python3
"""Native System Spec: private Xvfb, session D-Bus and MockBackend."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 env=dict(os.environ,LCONNECT_SPEC_ISOLATED='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb'],env=env))
assert os.environ.get('LCONNECT_SPEC_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-system-spec-'));(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_SPEC='1',XDG_CONFIG_DIRS=str(root/'system-config'),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
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
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}');wait(lambda:((root/'lconnectd.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==0,'no mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0


def close_gui(gui,name):
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker

def result_for(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native System Spec',seconds=120)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 return result

# Write a bounded, synthetic Linux inventory before starting the fixture daemon.
def write(relative,value):
 path=root/'spec'/relative;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(value)
write('proc/cpuinfo','processor : 0\nmodel name : Fixture CPU\nphysical id : 0\ncpu cores : 1\n\n')
write('proc/meminfo','MemTotal: 33554432 kB\n')
for bus,device in [('01','1234'),('02','5678')]:
 for field,value in [('class','0x030000'),('vendor','0xabcd'),('device','0x'+device)]:write(f'sys/bus/pci/devices/0000:{bus}:00.0/{field}',value)
write('pci.ids','abcd  Fixture Vendor\n\t1234  Fixture GPU A\n\t5678  Fixture GPU B\n')
write('sys/class/dmi/id/board_name','Fixture Board')
write('sys/block/sda/device/model','Fixture SSD');write('sys/block/sda/size','2097152')
try:
 launch(['openbox'],'wm');start_fixture()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui')
 result=result_for(gui);close_gui(gui,'gui')
 (root/'acceptance.json').write_text(json.dumps(dict(result,normal_close=True),ensure_ascii=False,indent=2)+'\n')
 print('PASS native System Spec/WebKitGTK/Unix IPC/no attached devices/private inventory:',root)

finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
