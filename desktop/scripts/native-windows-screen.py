#!/usr/bin/env python3
"""Native Windows Screen workflow: private Xvfb, session D-Bus and MockBackend."""
import json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
repo=Path(__file__).resolve().parents[2]
if any(flag in sys.argv for flag in ['--gif','--corrupt-video']) and '--video' not in sys.argv:sys.argv.append('--video')
if '--inside-xvfb' not in sys.argv:
 root=Path(tempfile.mkdtemp(prefix='lconnect-native-windows-screen-'));root.chmod(0o700)
 env=dict(os.environ,LCONNECT_ARCHIVE_ISOLATED='1',LCONNECT_SCREEN_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={root}/no-system-bus')
 for key in list(env):
  if key in ['DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','AT_SPI_BUS_ADDRESS'] or key.startswith('LCONNECT_NATIVE_'):env.pop(key,None)
 try:code=subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb',*[arg for arg in ['--english','--video','--gif','--corrupt-video','--release-daemon'] if arg in sys.argv]],env=env)
 finally:
  target=root/'doc'
  if any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines()):subprocess.run(['fusermount3','-uz','--',str(target)],check=True,timeout=10)
 raise SystemExit(code)
assert os.environ.get('LCONNECT_ARCHIVE_ISOLATED')=='1'
root=Path(os.environ['LCONNECT_SCREEN_ROOT']);assert os.environ['XDG_RUNTIME_DIR']==str(root)
(root/'fixture-marker').write_text('MockBackend only\n')
env=dict(os.environ,LCONNECT_NATIVE_ARCHIVE_PORTAL='1',LCONNECT_NATIVE_WINDOWS_PORTAL='1',LCONNECT_SMOKE_ROOT=str(root),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'),LCONNECT_NATIVE_DESKTOP_LANGUAGE='en-US' if '--english' in sys.argv else 'ru-RU',GDK_BACKEND='x11',GIO_USE_VFS='local',WEBKIT_DISABLE_DMABUF_RENDERER='1',LC_ALL='C.UTF-8',NO_AT_BRIDGE='1');env.pop('WAYLAND_DISPLAY',None)
if '--video' in sys.argv:env['LCONNECT_MOCK_EXPORT_VIDEO']='1'
if '--corrupt-video' in sys.argv:env['LCONNECT_MOCK_EXPORT_FRAMES']='1'
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
 (root/'lcd-video.h264').unlink(missing_ok=True);(root/'lcd-video.json').unlink(missing_ok=True);
 (root/'stop').unlink(missing_ok=True);(root/'lconnectd.sock').unlink(missing_ok=True);(root/'stats.json').unlink(missing_ok=True)
 fixture=launch([repo/('target/release/examples/desktop-mock' if '--release-daemon' in sys.argv else 'target/debug/examples/desktop-mock')],f'fixture-{len(processes)}');wait(lambda:((root/'lconnectd.sock').exists() and (root/'stats.json').exists()) or fixture.poll() is not None,'mock socket');assert fixture.poll() is None;wait(lambda:len(rpc('list_devices'))==1,'mock device')

def stop_fixture():
 (root/'stop').touch();fixture.wait(timeout=15);assert fixture.returncode==0


def close_gui(gui,name):
 def visible():
  found=subprocess.run(['xdotool','search','--onlyvisible','--name','^L-Connect Linux$'],env=env,text=True,capture_output=True,timeout=5)
  return found.stdout.splitlines() if found.returncode==0 else []
 wait(lambda:gui.poll() is not None or bool(visible()),'mapped native window before Close')
 assert gui.poll() is None,'GUI exited before explicit Close'
 window=visible()[0]
 subprocess.run(['xdotool','windowquit',window],env=env,check=True,timeout=5);gui.wait(timeout=20);assert gui.returncode==0
 close_log=(root/f'{name}.log').read_text()
 for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker

def result_for(gui):
 wait(lambda:(root/'result.json').exists() or gui.poll() is not None,'native basic editor workflow',seconds=120)
 assert (root/'result.json').exists(),f'GUI exited {gui.returncode}'
 result=json.loads((root/'result.json').read_text());assert result['ok'],result
 return result


try:
 import base64,zipfile
 with zipfile.ZipFile(repo/'crates/lconnect-profile/tests/fixtures/ScreenBanks.zip') as z:backup=json.loads(z.read('backup'))
 profile=json.loads(base64.b64decode(backup['Datas'][0]['Data']))
 extension='gif' if '--gif' in sys.argv else 'mp4'
 source_name='corrupt.mp4' if '--corrupt-video' in sys.argv else f'motion.{extension}'
 path=r'C:\owned\art.png';uri=f'file:///C:/owned/{source_name}' if '--video' in sys.argv else 'file:///C:/owned/art.png'
 profile['ScreenConfigJson']=json.dumps({'Style':'Full','Screens':[{'Theme':{'Type':'Image','Path':uri,'BackgroundColor':[3,5,7]},'Text':{'Type':'None'},'Watermark':{'HasImage':False,'Image':'','Horizontal':50,'Vertical':50,'Shadow':50,'Opacity':50}}]})
 if '--video' in sys.argv:
  runtime=json.loads(profile['ScreenConfigJson'])
  runtime['Screens'][0]['Text']={'Type':'SliderSensor','Sensors':['CPUTemperature'],'LoopSecond':10,'Layout':[{'FontFamily':'HyperspaceRace_BoldItalic','FontSize':43,'FontColors':[[255,255,255],[255,255,255],[255,255,255]],'HorizontalAlignment':'Center','VerticalAlignment':'Center','TextAlignment':'Center','Shadow':20,'Opacity':100}]}
  profile['ScreenConfigJson']=json.dumps(runtime)
 profile['ScreenProfiles']=[{} for _ in range(8)];profile['ScreenEffectType']=[2]*8
 for key in ['ScreenImageFileInfos','ScreenImageVideoFileInfos','ScreenWatermarkFileInfos']:profile[key]=[{'FilePath':path,'IsDefaultFile':False}]
 backup['Datas'][0]['Data']=base64.b64encode(json.dumps(profile).encode()).decode()
 motion=None
 if '--video' in sys.argv:
  motion=root/f'owned-motion.{extension}'
  command=['ffmpeg','-nostdin','-v','error','-f','lavfi','-i','testsrc2=s=64x48:r=6','-t','1','-threads','1','-filter_threads','1']
  if extension=='mp4':command+=['-c:v','libx264','-pix_fmt','yuv420p']
  if '--corrupt-video' in sys.argv:motion.write_bytes(b'owned invalid MP4')
  else:subprocess.run([*command,str(motion)],check=True,timeout=20)
 with zipfile.ZipFile(root/'source controls.zip','w') as z:
  z.writestr('backup',json.dumps(backup));z.writestr('wmsq/art.png',(repo/'crates/lconnect-profile/tests/fixtures/ScreenImage.png').read_bytes())
  if motion is not None:z.writestr(f'wmsq/{source_name}',motion.read_bytes())
 launch(['openbox'],'wm');start_fixture()
 display=rpc('get_display_settings');display['interface_language']='en-US' if '--english' in sys.argv else 'ru-RU';rpc('set_display_settings',{'settings':display})
 rejection_baseline=None
 if '--corrupt-video' in sys.argv:
  dev=rpc('list_devices')[0]['id']
  prior_scene={'version':1,'name':'Existing LCD scene','background':[37,53,71],'layers':[]}
  rpc('set_lcd_theme',{'dev':dev,'theme':prior_scene})
  wait(lambda:json.loads((root/'stats.json').read_text())['jpeg_starts']>=1,'existing mock LCD scene before malformed import')
  rejection_baseline=json.loads((root/'stats.json').read_text())
  rejection_frame=json.loads((root/'lcd-frame.json').read_text())['jpeg']
 gui=launch([repo/'target/debug/lconnect-gui'],'gui',dict(env,LCONNECT_NATIVE_WINDOWS_SCREEN='1'))
 first=result_for(gui)
 if '--corrupt-video' in sys.argv:
  assert first['source_rejected']
  released=False
  try:rpc('group_upload',{'dev':first['dev'],'group':'screen','request':{'action':'status','id':first['operation']}})
  except AssertionError as error:
   assert 'group upload unavailable or expired' in str(error),error
   released=True
  assert released,'failed private upload was not released'
  assert first['active']==prior_scene
  stats=json.loads((root/'stats.json').read_text());assert stats['lcd_video_packets']==rejection_baseline['lcd_video_packets']
  assert json.loads((root/'lcd-frame.json').read_text())['jpeg']==rejection_frame,'existing LCD frame changed during rejected import'
  close_gui(gui,'gui');stop_fixture()
  result={'initial':first,'normal_close':True,'language':'en-US' if '--english' in sys.argv else 'ru-RU','source_format':'invalid_mp4','no_new_video_stream':True,'existing_frame_bytes_preserved':True,'existing_active_scene_preserved':True,'private_upload_released':True,'portal_filters':True}
  events=[json.loads(line) for line in (root/'archive-portal-events.jsonl').read_text().splitlines()]
  assert len(events)==2 and all(e['method']=='OpenFile' and e['filters']==[['Windows L-Connect3 ZIP',[[0,'*.zip']]]] for e in events),events
  (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
  print('PASS native corrupted Windows Screen media rejected without Apply:',root)
  raise SystemExit(0)
 video_evidence=None
 if '--video' in sys.argv:
  assert first['video_source'] and first['active']['background_video']
  wait(lambda:json.loads((root/'stats.json').read_text())['lcd_video_packets']>=24,'native imported video packets')
  initial_packets=json.loads((root/'stats.json').read_text())['lcd_video_packets']
  wait(lambda:json.loads((root/'stats.json').read_text())['lcd_video_packets']>=initial_packets+24,'native imported video continues across loop')
  video_evidence={'initial_packets':initial_packets,'later_packets':json.loads((root/'stats.json').read_text())['lcd_video_packets'],'prepared_clips':rpc('list_prepared_clips')}
  start_frames=json.loads((root/'lcd-video.json').read_text())['frames']
  started=time.monotonic()
  wait(lambda:json.loads((root/'lcd-video.json').read_text())['frames']>=start_frames+48,'two complete video cycles after preview')
  elapsed=time.monotonic()-started
  sampled_frames=json.loads((root/'lcd-video.json').read_text())['frames']-start_frames
  video_evidence.update(sampled_frames=sampled_frames,sampled_seconds=elapsed,observed_fps=sampled_frames/elapsed)
  assert video_evidence['observed_fps']>=20,video_evidence
 close_gui(gui,'gui');(root/'result.json').unlink()
 stop_fixture()
 if '--video' in sys.argv:
  receipt=json.loads((root/'lcd-video.json').read_text());assert receipt['frames']>=48
  probe=subprocess.run(['ffprobe','-v','error','-select_streams','v:0','-count_frames','-show_entries','stream=width,height,nb_read_frames','-of','json',str(root/'lcd-video.h264')],check=True,capture_output=True,text=True,timeout=20)
  decoded=json.loads(probe.stdout)['streams'][0]
  assert decoded['width']==480 and decoded['height']==480 and int(decoded['nb_read_frames'])==receipt['frames'],(decoded,receipt)
  # FFmpeg must actually decode every access unit, not only identify metadata.
  subprocess.run(['ffmpeg','-nostdin','-v','error','-xerror','-i',str(root/'lcd-video.h264'),'-f','null','-'],check=True,capture_output=True,timeout=20)
  video_evidence.update(completed_stream=receipt,decoded=decoded)
  (root/'lcd-video.h264').rename(root/'initial-lcd-video.h264')
 start_fixture()
 if '--video' in sys.argv:
  wait(lambda:(root/'lcd-video.json').exists() and json.loads((root/'lcd-video.json').read_text())['frames']>=24,'imported video restarts from saved settings')
 assert rpc('list_profiles')==first['library']
 assert rpc('get_lcd_theme',{'dev':first['dev']})==first['active']
 assert rpc('get_imported_lcd_editor',{'dev':first['dev']})==first['editor']
 (root/'source controls.zip').unlink()
 if motion is not None:motion.unlink()
 gui=launch([repo/'target/debug/lconnect-gui'],'gui-recovery',dict(env,LCONNECT_NATIVE_WINDOWS_SCREEN='1',LCONNECT_NATIVE_WINDOWS_RECOVER='1'))
 second=result_for(gui);close_gui(gui,'gui-recovery')
 if '--video' in sys.argv:
  stop_fixture()
  restart_receipt=json.loads((root/'lcd-video.json').read_text());assert restart_receipt['frames']>=24
  probe=subprocess.run(['ffprobe','-v','error','-select_streams','v:0','-count_frames','-show_entries','stream=width,height,nb_read_frames','-of','json',str(root/'lcd-video.h264')],check=True,capture_output=True,text=True,timeout=20)
  restart_decoded=json.loads(probe.stdout)['streams'][0]
  assert restart_decoded['width']==480 and restart_decoded['height']==480 and int(restart_decoded['nb_read_frames'])==restart_receipt['frames'],(restart_decoded,restart_receipt)
  subprocess.run(['ffmpeg','-nostdin','-v','error','-xerror','-i',str(root/'lcd-video.h264'),'-f','null','-'],check=True,capture_output=True,timeout=20)
  video_evidence.update(restart_stream=restart_receipt,restart_decoded=restart_decoded)
 assert second['operation']==first['operation'] and second['active']==first['active'] and second['editor']==first['editor'] and second['library']==first['library']
 events=[json.loads(line) for line in (root/'archive-portal-events.jsonl').read_text().splitlines()]
 assert len(events)==2 and all(e['method']=='OpenFile' and e['filters']==[['Windows L-Connect3 ZIP',[[0,'*.zip']]]] for e in events),events
 result=dict(initial=first,recovery=second,video=video_evidence,source_format=extension if '--video' in sys.argv else 'png',normal_close=True,desktop_and_daemon_restart=True,language='en-US' if '--english' in sys.argv else 'ru-RU',source_removed_before_recovery=True,portal_filters=True)
 (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 print('PASS native Windows Screen/WebKitGTK/FileChooser/Unix IPC/MockBackend/imported editor/restart:',root)

finally:
 if fixture is not None and fixture.poll() is None:(root/'stop').touch()
 for p in reversed(processes):
  if p.poll() is None:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=5)
 for log in logs:log.close()
 print('Fixture directory:',root)
