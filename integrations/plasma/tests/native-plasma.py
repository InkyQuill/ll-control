#!/usr/bin/env python3
"""Render the real Plasma package in a private Xvfb/session with a Unix RPC peer.
No installation in the user's session, no daemon/hardware collector, no Tauri.
"""
import json,os,signal,socket,subprocess,sys,tempfile,threading,time
from pathlib import Path
repo=Path(__file__).resolve().parents[3]
build=Path(sys.argv[1]).resolve()
if '--inside' not in sys.argv:
 env=dict(os.environ,LCONNECT_PLASMA_ISOLATED='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1400x1000x24','dbus-run-session','--',sys.executable,__file__,str(build),'--inside']+(['--staged'] if '--staged' in sys.argv else []),env=env))
assert os.environ.get('LCONNECT_PLASMA_ISOLATED')=='1'
root=Path(tempfile.mkdtemp(prefix='lconnect-mini-plasma-native-'));(root/'bin').mkdir();(root/'bin/lconnectctl').symlink_to(repo/'target/debug/lconnectctl')
stage=build/'stage'
package=stage/'share/plasma/plasmoids/net.inkyquill.lconnect.mini' if '--staged' in sys.argv else build/'package'
qml_path=next(stage.glob('lib*/qt6/qml')) if '--staged' in sys.argv else build/'qml'
assert (package/'contents/code/mini.mjs').is_file()
env=dict(os.environ,PATH=str(root/'bin')+':'+os.environ.get('PATH',''),QML_IMPORT_PATH=str(qml_path),LCONNECTCTL_SOCKET=str(root/'peer.sock'),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),QT_QPA_PLATFORM='xcb',QT_QUICK_BACKEND='software',LC_ALL='C.UTF-8');env.pop('WAYLAND_DISPLAY',None)
stop=threading.Event();calls=[];language='en-US';processes=[];logs=[];workers=[];errors=[]
def sensor(key,unit,value,label=None):return dict(id=key,unit=unit,value=value,label=label or key,status='valid',age_ms=0,observed_unix_ms=int(time.time()*1000))
sensors=[sensor('cpu:load','percent',38),sensor('cpu:power','watts',80),sensor('cpu:voltage','volts',1.25),sensor('gpu:A:load','percent',15),sensor('gpu:A:power','watts',72),sensor('gpu:A:voltage','volts',1.05)]
roles=['cpu_load','cpu_power','cpu_voltage','gpu_load','gpu_power','gpu_voltage']
config=dict(version=1,revision=0,cpu=dict(load='cpu:load',power='cpu:power',voltage='cpu:voltage'),gpu=dict(device_id='A',load='gpu:A:load',power='gpu:A:power',voltage='gpu:A:voltage'))
def serve(conn):
 try:
  conn.settimeout(2);stream=conn.makefile('rwb');stream.write((json.dumps(dict(jsonrpc='2.0',method='hello',params={}))+'\n').encode());stream.flush()
  while not stop.is_set():
   try:line=stream.readline()
   except (TimeoutError,OSError):break
   if not line:break
   request=json.loads(line);method=request['method'];calls.append(method)
   if request.get('params')!={}:raise AssertionError(request)
   if method=='list_sensors':value=sensors
   elif method=='get_display_settings':value=dict(version=1,revision=0,temperature_unit='fahrenheit',interface_language=language)
   elif method=='get_sensor_defaults':value=dict(config=config,recommended=config,candidates=[dict(role=role,gpu_id='A' if role.startswith('gpu') else None,reading=s) for role,s in zip(roles,sensors)],values=[dict(role=role,sensor_id=s['id'],reading=s) for role,s in zip(roles,sensors)])
   else:raise AssertionError('Forbidden method: '+method)
   stream.write((json.dumps(dict(jsonrpc='2.0',id=request['id'],result=value))+'\n').encode());stream.flush()
 except (BrokenPipeError,ConnectionResetError):pass
 except Exception as e:errors.append(str(e))
 finally:conn.close()
listener=socket.socket(socket.AF_UNIX);listener.bind(str(root/'peer.sock'));listener.listen();listener.settimeout(.2)
def accept():
 while not stop.is_set():
  try:conn,_=listener.accept()
  except TimeoutError:continue
  except OSError:break
  t=threading.Thread(target=serve,args=(conn,),daemon=True);workers.append(t);t.start()
server=threading.Thread(target=accept,daemon=True);server.start()
def launch(cmd,name):
 log=(root/(name+'.log')).open('w');logs.append(log);p=subprocess.Popen(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p
def wait(test,label,seconds=20):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(label)
  time.sleep(.05)
def windows(pid):
 p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(pid)],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=5)
 return p.stdout.splitlines() if p.returncode==0 else []
try:
 launch(['openbox'],'wm')
 installed=subprocess.run(['kpackagetool6','--type','Plasma/Applet','--packageroot',str(root/'data/plasma/plasmoids'),'--install',str(package)],env=env,text=True,capture_output=True,timeout=15);(root/'package-install.log').write_text(installed.stdout+installed.stderr);assert installed.returncode==0,installed.stderr
 gui=launch(['plasmawindowed','net.inkyquill.lconnect.mini'],'widget')
 wait(lambda:len(calls)>=6 or gui.poll() is not None,'real CLI feed');assert gui.poll() is None
 wait(lambda:bool(windows(gui.pid)),'Plasma package window');window=windows(gui.pid)[0]
 subprocess.run(['xdotool','windowsize',window,'760','420'],env=env,check=True,timeout=5)
 # Wait for another complete CLI cycle after resize before recording pixels.
 before=len(calls);wait(lambda:len(calls)>=before+3,'English refresh')
 subprocess.run(['import','-window',window,str(root/'english.png')],env=env,check=True,timeout=10)
 language='ru-RU';before=len(calls);wait(lambda:len(calls)>=before+6,'Russian refresh')
 subprocess.run(['import','-window',window,str(root/'russian.png')],env=env,check=True,timeout=10)
 helper_ids=subprocess.run(['pgrep','-P',str(gui.pid),'lconnectctl'],text=True,stdout=subprocess.PIPE).stdout.splitlines();assert len(helper_ids)==1,helper_ids
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=15);assert gui.returncode==0
 for pid in helper_ids:assert not Path('/proc/'+pid).exists(),'Helper survived widget destruction'
 assert 'languageCache=ru-RU' in (root/'config/plasmawindowedrc').read_text(),'Confirmed Russian cache was not persisted by Plasma'
 log=(root/'widget.log').read_text();fatal=['ReferenceError','TypeError','SyntaxError','is not installed','Error loading QML file','Script file:']
 assert not any(item in log for item in fatal),log
 assert not errors,errors
 (root/'acceptance.json').write_text(json.dumps(dict(package_loaded=True,real_cli=True,readonly_methods=sorted(set(calls)),calls=len(calls),helper_count=1,helper_reaped=True,language_cache_persisted=True,english_screenshot='english.png',russian_screenshot='russian.png',hardware_access=False,host_installation=False,staged_package='--staged' in sys.argv),indent=2)+'\n')
 print('PASS private Plasma package/real CLI/read-only Unix peer/helper teardown:',root)
finally:
 stop.set();listener.close();server.join(timeout=2)
 for worker in workers:worker.join(timeout=3)
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=2)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
