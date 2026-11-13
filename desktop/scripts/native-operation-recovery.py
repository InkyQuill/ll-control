#!/usr/bin/env python3
"""Post-commit disconnect/deadline/close through native UI and private MockBackend."""
import importlib.util,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='llcontrol-native-operation-'))
 env=dict(os.environ,LLCONTROL_ARCHIVE_ISOLATED='1',LLCONTROL_OPERATION_ROOT=str(root),
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
root=Path(os.environ['LLCONTROL_OPERATION_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
operation,fault,language=[arg for arg in sys.argv[1:] if arg!='--inside-xvfb']
assert operation in ['image-delete','video-delete','profile-save','lcd-apply','lcd-clear','screen-reset']
assert fault in ['disconnect','timeout','close'] and language in ['en','ru']
env=dict(os.environ,LLCONTROL_NATIVE_LANGUAGE='1',LLCONTROL_NATIVE_DESKTOP_PHASE=f'core|{operation}|{fault}|{language}',LLCONTROL_SMOKE_ROOT=str(root),LLCONTROL_NATIVE_ARCHIVE_PORTAL='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
processes=[];logs=[];fixture=None

def launch(cmd,name,process_env=env):
 log=(root/f'{name}.log').open('w');logs.append(log);p=subprocess.Popen(cmd,env=process_env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p

def wait(test,why,seconds=20):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(why)
  time.sleep(.05)

def rpc(method,params=None,endpoint=None):
 with socket.socket(socket.AF_UNIX) as conn:
  conn.settimeout(20);conn.connect(str(endpoint or root/'llcontrold.sock'));stream=conn.makefile('rwb');stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:assert 'error' not in reply,reply;return reply['result']

def start_fixture():
 global fixture
 (root/'stop').unlink(missing_ok=True);(root/'llcontrold.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}');wait(lambda:((root/'llcontrold.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==1,'mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0

proxy=None
try:
 launch(['openbox'],'wm');start_fixture()
 dev=rpc('list_devices')[0]['id']
 settings=rpc('get_display_settings');settings['interface_language']='en-US' if language=='en' else 'ru-RU';rpc('set_display_settings',{'settings':settings})
 initial={'version':1,'name':'Existing scene','background':[21,32,43],'layers':[]}
 rpc('set_lcd_theme',{'dev':dev,'theme':initial})
 rpc('set_cooling_policy',{'dev':dev,'target':'fan','policy':{'mode':'fixed','percent':37}})
 lib=rpc('list_profiles');rpc('edit_profile',{'expected_revision':lib['revision'],'edit':{'action':'save','profile':{'slot':7,'name':'Keep neighbor','content':{'kind':'screen','value':initial}}}})
 rpc('migrate_screen_collections',{'dev':dev})
 image=rpc('import_theme_asset',{'bytes':list((repo/'crates/llcontrol-image/assets/defaults/midnight.png').read_bytes()),'name':'Delete me'})
 builtin=next(clip for clip in rpc('list_prepared_clips') if clip['is_default']);unit=rpc('load_media_first_frame',{'key':builtin['key']})
 clip='a'*32
 rpc('media_upload',{'request':{'action':'begin','id':clip,'name':'Delete video','frames':1,'encoded_bytes':len(unit['bytes'])}})
 rpc('media_upload',{'request':{'action':'chunk','id':clip,'index':0,'unit':unit}})
 assert rpc('media_upload',{'request':{'action':'finish','id':clip}})['phase']=='published'
 def snapshot(endpoint=None):
  return {'theme':rpc('get_lcd_theme',{'dev':dev},endpoint),'library':rpc('list_profiles',endpoint=endpoint),
          'images':rpc('list_named_theme_assets',endpoint=endpoint),'clips':rpc('list_prepared_clips',endpoint=endpoint),
          'fan':rpc('get_cooling_state',{'dev':dev},endpoint)['fan']['requested']}
 before=snapshot()
 backend=root/'backend.sock';(root/'llcontrold.sock').rename(backend)
 spec=importlib.util.spec_from_file_location('operation_proxy',repo/'desktop/scripts/native-fault-proxy.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 method={'image-delete':'delete_theme_asset','video-delete':'delete_prepared_clip','profile-save':'edit_profile','lcd-apply':'set_lcd_theme','lcd-clear':'clear_lcd','screen-reset':'reset_screen'}[operation]
 proxy=module.FaultProxy(root/'llcontrold.sock',backend,operation_method=method,operation_fault=fault)
 gui=launch([repo/'target/debug/llcontrol-gui'],'gui')
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native operation result',seconds=60)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 if fault=='close':
  assert result['ready'];wait(proxy.operation_ready.is_set,'committed operation before close')
 else:
  assert result['draft_preserved']
  if fault=='timeout':assert result['elapsed']>=9500
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^LL-Control$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/'gui.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker
 assert not proxy.errors,proxy.errors
 assert len(proxy.operation_requests)==1 and len(proxy.operation_results)==1,'operation automatically replayed or never committed'
 after=snapshot(backend)
 assert after['fan']==before['fan'],'neighbor cooling changed'
 if operation=='image-delete':
  assert [a for a in before['images'] if a['key']!=image]==after['images']
 else:assert after['images']==before['images']
 if operation=='video-delete':assert [c for c in before['clips'] if c['key']!=clip]==after['clips']
 else:assert after['clips']==before['clips']
 if operation=='profile-save':
  assert after['library']['revision']==before['library']['revision']+1
  saved=next(p for p in after['library']['profiles'] if p['slot']==0)
  assert saved['name']=='R2 saved draft' and saved['content']['value']['name']=='R2 local draft'
  assert [p for p in after['library']['profiles'] if p['slot']!=0]==before['library']['profiles']
 elif operation=='screen-reset':
  assert after['library']['revision']==before['library']['revision']+1
  receipt=rpc('get_screen_reset',{'dev':dev},backend)['receipt']
  assert receipt['operation_id']==proxy.operation_requests[0]['params']['operation_id']
 else:assert after['library']==before['library']
 if operation=='lcd-apply':assert after['theme']['name']=='R2 local draft'
 elif operation=='lcd-clear':assert after['theme'] is None
 elif operation!='screen-reset':assert after['theme']==before['theme']
 request=proxy.operation_requests[0];proxy.close();proxy=None
 stop_fixture();backend.unlink(missing_ok=True);start_fixture()
 wait(lambda:snapshot()==after,'committed state after daemon restart')
 result.update(before=before,after=after,request=request,normal_close=True,restart_readback=True,one_write=True)
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print('PASS native operation recovery:',operation,fault,language,root)
finally:
 if proxy is not None:proxy.close()
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
