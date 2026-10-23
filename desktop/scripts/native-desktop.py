#!/usr/bin/env python3
"""Private Xvfb/D-Bus watcher, GTK tray/menu, actual Tauri commands; MockBackend."""
import json,os,re,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 language=sys.argv[sys.argv.index('--language')+1] if '--language' in sys.argv else 'ru-RU'
 assert language in {'en-US','ru-RU'}
 env=dict(os.environ,LCONNECT_DESKTOP_ISOLATED='1',LCONNECT_NATIVE_DESKTOP_LANGUAGE=language);env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 raise SystemExit(subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb'],env=env))
assert os.environ.get('LCONNECT_DESKTOP_ISOLATED')=='1'
language=os.environ['LCONNECT_NATIVE_DESKTOP_LANGUAGE']
show_label='Show LL-Control' if language=='en-US' else 'Показать LL-Control'
exit_label='Exit application' if language=='en-US' else 'Выйти из приложения'
root=Path(tempfile.mkdtemp(prefix='lconnect-native-desktop-'));(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_TRAY='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CONFIG_DIRS=str(root/'system-config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
processes=[];logs=[]
def launch(cmd,name,extra=None):
 log=(root/f'{name}.log').open('w');logs.append(log);p=subprocess.Popen(cmd,env=dict(env,**(extra or {})),stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p

def wait(test,why,seconds=25):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(why)
  time.sleep(.05)

def rpc(method,params=None):
 with socket.socket(socket.AF_UNIX) as conn:
  conn.settimeout(10);conn.connect(str(root/'lconnectd.sock'));stream=conn.makefile('rwb')
  stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:
    assert 'error' not in reply,reply
    return reply['result']

def visible():
 return subprocess.run(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL).returncode==0

def close_window():
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5)

def call(service,path,method,*args):
 return subprocess.check_output(['gdbus','call','--session','--dest',service,'--object-path',path,'--method',method,'--',*args],env=env,text=True,stderr=subprocess.DEVNULL,timeout=5)

def menu(label,activate=True):
 items=json.loads((root/'tray-items.json').read_text())
 for item in reversed(items):
  service,_,path=item.partition('/');path='/'+path if path else '/StatusNotifierItem'
  try:
   prop=call(service,path,'org.freedesktop.DBus.Properties.Get','org.kde.StatusNotifierItem','Menu')
   menu_path=re.search(r"objectpath '([^']+)'",prop).group(1)
   layout=call(service,menu_path,'com.canonical.dbusmenu.GetLayout','0','-1','[]')
   matches=re.findall(r"\((\d+), \{([^{}]*)\}",layout)
   chosen=next(int(identifier) for identifier,properties in matches if repr(label) in properties)
   if activate:call(service,menu_path,'com.canonical.dbusmenu.Event',str(chosen),'clicked','<0>','0')
   return chosen
  except (subprocess.CalledProcessError,StopIteration,AttributeError):pass
 raise AssertionError(f'Tray menu item unavailable: {label}')

def menu_present(label):
 try:return menu(label,False) is not None
 except AssertionError:return False

def joined(gui,name):
 gui.wait(timeout=25);assert gui.returncode==0
 text=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in text,marker

def phase_result(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'desktop result',seconds=120)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 (root/'result.json').unlink();return result

try:
 launch(['openbox'],'wm');fixture=launch([repo/'target/debug/examples/desktop-mock'],'fixture')
 wait(lambda:(root/'lconnectd.sock').exists() and (root/'stats.json').exists(),'mock daemon')
 display=rpc('get_display_settings')
 if language=='ru-RU':
  display['interface_language']=language;rpc('set_display_settings',{'settings':display})
 else:assert display['interface_language']=='en-US','fresh installation must default to English'
 results=[]
 for phase in [1,2,3]:
  if phase==3:(root/'no-tray-host').touch()
  if phase==2:(root/'lconnectd.sock').rename(root/'daemon-offline.sock')
  (root/'desktop-step').unlink(missing_ok=True);(root/'desktop-ack').unlink(missing_ok=True)
  name=f'gui-{phase}';gui=launch([repo/'target/debug/lconnect-gui',*(['--autostart'] if phase>1 else [])],name,{'LCONNECT_NATIVE_DESKTOP':'1','LCONNECT_NATIVE_DESKTOP_PHASE':str(phase)})
  wait(lambda:(root/'desktop-step').exists() or (root/'result.json').exists() or gui.poll() is not None,'desktop phase ready',seconds=60)
  if (root/'result.json').exists():
   unexpected=json.loads((root/'result.json').read_text());assert unexpected.get('ok'),unexpected
  assert (root/'desktop-step').exists(),f'GUI exited before desktop step: {gui.poll()}'
  assert (root/'desktop-step').read_text()==str(phase)
  if phase<3:wait(lambda:menu_present(show_label) and menu_present(exit_label),'localized native menu')
  if phase==1:
   before_language=json.loads((root/'stats.json').read_text())
   other='ru-RU' if language=='en-US' else 'en-US'
   display=rpc('get_display_settings');display['interface_language']=other;rpc('set_display_settings',{'settings':display})
   wait(lambda:menu_present('Показать LL-Control' if other=='ru-RU' else 'Show LL-Control') and menu_present('Выйти из приложения' if other=='ru-RU' else 'Exit application'),'runtime menu language switch')
   display=rpc('get_display_settings');display['interface_language']=language;rpc('set_display_settings',{'settings':display})
   wait(lambda:menu_present(show_label) and menu_present(exit_label),'restore selected native language')
   after_language=json.loads((root/'stats.json').read_text())
   for key in ['pump_pwm_writes','fan_pwm_writes','firmware_update_writes']:assert before_language[key]==after_language[key],key
  if phase==1:
   wait(visible,'initial window visible');entry=root/'config/autostart/net.inkyquill.lconnect.desktop';assert entry.exists()
   checked=subprocess.run(['desktop-file-validate',entry],env=env,capture_output=True,text=True);assert checked.returncode==0,checked.stderr
   close_window();wait(lambda:not visible(),'close hides to tray');assert gui.poll() is None
   auto_duplicate=launch([repo/'target/debug/lconnect-gui','--autostart'],'autostart-duplicate',{'LCONNECT_NATIVE_DESKTOP':'1'});auto_duplicate.wait(timeout=15);assert auto_duplicate.returncode==0;assert not visible(),'autostart duplicate raised the hidden window'
   # A normal duplicate launch activates the existing hidden window and exits.
   duplicate=launch([repo/'target/debug/lconnect-gui'],'duplicate',{'LCONNECT_NATIVE_DESKTOP':'1'});duplicate.wait(timeout=15);assert duplicate.returncode==0;wait(visible,'duplicate activates existing window')
   close_window();wait(lambda:not visible(),'hide before host loss');(root/'no-tray-host').touch();wait(visible,'host loss restores hidden window')
   (root/'no-tray-host').unlink();wait(lambda:(root/f'{name}.log').read_text().count('native desktop: tray-ready true')>=2,'host returns')
   close_window();wait(lambda:not visible(),'hide again');menu(show_label);wait(visible,'tray Show opens window')
  elif phase==2:
   wait(lambda:not visible(),'minimized startup');menu(show_label);wait(visible,'show minimized startup')
  else:wait(visible,'no-host startup fallback',seconds=10)
  (root/'desktop-ack').write_text(str(phase));result=phase_result(gui);results.append(result)
  if phase<3:menu(exit_label)
  # Phase3 requests app.exit in the smoke command, exercising ExitRequested.

  joined(gui,name)
  if phase==2:(root/'daemon-offline.sock').rename(root/'lconnectd.sock')
 assert not (root/'config/autostart/net.inkyquill.lconnect.desktop').exists()
 assert results[-1]['hostStatusLocalized'] and results[-1]['state']['trayErrorCode']=='host_unconfirmed'
 result=dict(ok=True,language=language,runtime_menu_language=True,offline_cached_restart=True,no_control_writes_on_language_change=True,phases=results,hide_show=True,host_loss_restore=True,minimized_start=True,no_host_fallback=True,single_instance_activation=True,autostart_duplicate_stays_hidden=True,exit_requested_event=True,joined_exit=True)
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print('PASS native desktop tray/menu/preferences/autostart/single-instance:',root)
finally:
 (root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
