#!/usr/bin/env python3
"""Native interface language: private Xvfb, session D-Bus and MockBackend."""
import importlib.util,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='lconnect-native-interface-language-'))
 env=dict(os.environ,LCONNECT_LANGUAGE_ISOLATED='1',LCONNECT_LANGUAGE_ROOT=str(root),
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
assert os.environ.get('LCONNECT_LANGUAGE_ISOLATED')=='1'
root=Path(os.environ['LCONNECT_LANGUAGE_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_SMOKE_ROOT=str(root),LCONNECT_NATIVE_SPEC='1',LCONNECT_NATIVE_UNITS='1',LCONNECT_NATIVE_LANGUAGE='1',XDG_CONFIG_DIRS=str(root/'system-config'),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
processes=[];logs=[];fixture=None;proxy=None

def launch(cmd,name,process_env=env):
 log=(root/f'{name}.log').open('w');logs.append(log);p=subprocess.Popen(cmd,env=process_env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(p);return p

def wait(test,why,seconds=20):
 end=time.monotonic()+seconds
 while not test():
  if time.monotonic()>end:raise TimeoutError(why)
  time.sleep(.05)

def rpc(method,params=None,endpoint=None):
 with socket.socket(socket.AF_UNIX) as conn:
  conn.settimeout(20);conn.connect(str(endpoint or root/'lconnectd.sock'));stream=conn.makefile('rwb');stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:assert 'error' not in reply,reply;return reply['result']

def start_fixture(with_device=False):
 global fixture
 (root/'stop').unlink(missing_ok=True);(root/'lconnectd.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}',dict(env,LCONNECT_NATIVE_SPEC='0' if with_device else '1'));wait(lambda:((root/'lconnectd.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==(1 if with_device else 0),'fixture device count')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0


def close_gui(gui,name):
 window=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^L-Connect Linux$'],env=env,text=True,timeout=5).splitlines()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker

def result_for(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native interface language',seconds=120)
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
 if any(flag in sys.argv for flag in ['--lcd-errors','--image-errors','--library-errors','--cooling-errors','--collection-readback-errors','--collection-plain-errors','--collection-replacement-errors','--collection-video-errors','--collection-video-import-errors']):
  launch(['openbox'],'wm');start_fixture(with_device=True)
  collection_errors=any(flag in sys.argv for flag in ['--collection-readback-errors','--collection-plain-errors','--collection-replacement-errors','--collection-video-errors','--collection-video-import-errors'])
  plain_errors='--collection-plain-errors' in sys.argv
  replacement_errors='--collection-replacement-errors' in sys.argv
  video_errors='--collection-video-errors' in sys.argv
  video_import_errors='--collection-video-import-errors' in sys.argv
  image_errors='--image-errors' in sys.argv
  library_errors='--library-errors' in sys.argv
  cooling_errors='--cooling-errors' in sys.argv
  if image_errors or collection_errors:
   dev=rpc('list_devices')[0]['id']
   if replacement_errors:
    rpc('import_theme_asset',{'bytes':list((repo/'crates/lconnect-image/assets/defaults/midnight.png').read_bytes()),'name':'R2 original {0} <CPU>.png'})
   if video_errors:
    builtin=next(clip for clip in rpc('list_prepared_clips') if clip['is_default'])
    unit=rpc('load_media_first_frame',{'key':builtin['key']});key='a'*32
    rpc('media_upload',{'request':{'action':'begin','id':key,'name':'R2 source.mp4','frames':48,'encoded_bytes':len(unit['bytes'])*48}})
    for index in range(48):rpc('media_upload',{'request':{'action':'chunk','id':key,'index':index,'unit':unit}})
    assert rpc('media_upload',{'request':{'action':'finish','id':key}})['phase']=='published'
   rpc('migrate_screen_collections',{'dev':dev})
   original_collections=rpc('get_screen_collections',{'dev':dev})['collections']
  backend=root/'backend.sock';(root/'lconnectd.sock').rename(backend)
  spec=importlib.util.spec_from_file_location('lcd_fault_proxy',repo/'desktop/scripts/native-fault-proxy.py')
  proxy_module=importlib.util.module_from_spec(spec);spec.loader.exec_module(proxy_module)
  phase='collection-video-import-errors' if video_import_errors else 'collection-video-errors' if video_errors else 'collection-replacement-errors' if replacement_errors else 'collection-plain-errors' if plain_errors else 'collection-readback-errors' if collection_errors else 'cooling-errors' if cooling_errors else 'library-errors' if library_errors else 'image-errors' if image_errors else 'lcd-errors'
  transport_errors='--collection-transport-errors' in sys.argv
  if transport_errors:phase+='-transport'
  proxy=proxy_module.FaultProxy(root/'lconnectd.sock',backend,collection_transport=transport_errors,lcd_messages=not (image_errors or library_errors or cooling_errors or collection_errors),collection_readbacks=collection_errors,image_messages=image_errors,library_messages=library_errors,cooling_messages=cooling_errors)
  gui=launch([repo/'target/debug/lconnect-gui'],f'gui-{phase}',dict(env,LCONNECT_NATIVE_DESKTOP_PHASE=phase))
  result=result_for(gui);close_gui(gui,f'gui-{phase}')
  if collection_errors:
   count=len(proxy.collection_modes)*2
   assert len(result['cases'])==count and len(proxy.collection_writes)==count and len(proxy.collection_faults)==count
   assert not proxy.collection_pending,proxy.collection_pending
   assert [case['mode'] for case in proxy.collection_faults]==list(proxy.collection_modes)*2
   assert proxy.collection_writes==proxy.collection_faults
   current=rpc('get_screen_collections',{'dev':dev},backend)
   assert not current['persistence']['pending']
   expected=proxy.collection_writes[-1]['expected'] if replacement_errors or video_errors or video_import_errors else {**original_collections,'revision':original_collections['revision']+count,'images':original_collections['images']+[case['key'] for case in proxy.collection_writes]}
   assert expected['revision']==original_collections['revision']+count
   assert current['collections']==expected,(current,expected)
   assert [case['key'] for case in proxy.collection_writes]==[case['key'] for case in result['cases']]
   assert json.loads((root/'stats.json').read_text())['jpeg_starts']==0
   requests={'set_screen_collections':count,'injected_readbacks':count,'faults':proxy.collection_faults}
  elif cooling_errors:
   assert proxy.cooling_message_requests==4,proxy.cooling_message_requests
   wait(lambda:json.loads((root/'stats.json').read_text())['pump_pwm_writes']>=4,'observed cooling writes')
   stats=json.loads((root/'stats.json').read_text());assert stats['last_pump_pwm_packet'][6:8]==[0,64] and stats['fan_pwm_writes']==0 and stats['jpeg_starts']==0,stats
   requests={'set_cooling_policy':proxy.cooling_message_requests}
  elif library_errors:
   assert proxy.library_rename_requests==4,proxy.library_rename_requests
   assert json.loads((root/'stats.json').read_text())['jpeg_starts']==0
   requests={'rename_theme_asset':proxy.library_rename_requests}
  elif image_errors:
   assert proxy.image_message_requests==4,proxy.image_message_requests
   assert rpc('get_screen_collections',{'dev':dev},backend)['collections']==original_collections
   stats=json.loads((root/'stats.json').read_text());assert stats['jpeg_starts']==0,stats
   requests={'set_screen_collections':proxy.image_message_requests}
  else:
   expected={method:4 for method in ['set_lcd_theme','get_lcd_theme','clear_lcd','preview_lcd_theme']}
   assert proxy.lcd_message_requests==expected,proxy.lcd_message_requests
   requests=proxy.lcd_message_requests
  assert not proxy.errors,proxy.errors
  (root/'acceptance.json').write_text(json.dumps(dict(result=result,requests=requests,normal_close=True,actual_hardware_access=False),ensure_ascii=False,indent=2)+'\n')
  print(f'PASS native {phase}/EN-RU/publication boundaries/Unix IPC/MockBackend:',root)
  raise SystemExit(0)
 launch(['openbox'],'wm');start_fixture()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui')
 first=result_for(gui);close_gui(gui,'gui');(root/'result.json').unlink()
 stop_fixture();start_fixture()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui-recovery',dict(env,LCONNECT_NATIVE_DESKTOP_PHASE='2'))
 second=result_for(gui);close_gui(gui,'gui-recovery')
 stats=json.loads((root/'stats.json').read_text());assert stats['hid_write_count']==0 and stats['devices_known']==0,stats
 stop_fixture();start_fixture(with_device=True);(root/'result.json').unlink()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui-image',dict(env,LCONNECT_NATIVE_DESKTOP_PHASE='image'))
 image=result_for(gui);assert image['imageEditorPreview'] and image['imageMembershipReadback']['adjacentMembershipPreserved'] and not image['noDevices'];close_gui(gui,'gui-image')
 image_stats=json.loads((root/'stats.json').read_text());assert image_stats['devices_known']==1 and image_stats['jpeg_starts']==0,image_stats
 (root/'acceptance.json').write_text(json.dumps(dict(initial=first,recovery=second,image_editor=image,normal_close=True,daemon_restart=True,no_device_phase_zero_hid_writes=True,image_phase_mock_hid_writes=image_stats['hid_write_count']),ensure_ascii=False,indent=2)+'\n')
 print('PASS native interface language/WebKitGTK/Unix IPC/user draft/daemon restart/MockBackend image editor:',root)

finally:
 if proxy is not None:proxy.close()
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
