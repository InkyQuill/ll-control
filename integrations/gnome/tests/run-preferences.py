#!/usr/bin/env python3
"""Private GTK/GSettings fixture, no GNOME session or extension installation."""
import argparse,json,os,signal,subprocess,tempfile
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('package',type=Path);parser.add_argument('--gjs',required=True);args=parser.parse_args()
with tempfile.TemporaryDirectory(prefix='llcontrol-mini-gjs-preferences-') as tmp:
 root=Path(tmp);(root/'bin').mkdir()
 def sensor(key,value,label):return dict(id=key,label=label,unit='watts',value=value,status='valid',age_ms=0)
 a=sensor('gpu:A:power',72,'GPU A');b=sensor('gpu:B:power',99,'Настройки')
 frame=dict(version=1,available=True,data=dict(display=dict(temperature_unit='fahrenheit',interface_language='en-US'),sensors=[a,b],defaults=dict(candidates=[dict(role='gpu_power',reading=a),dict(role='gpu_power',reading=b)],values=[])))
 frame['data']['sensors'].append(dict(id='mangohud:2f67616d65:1:2:logged_frame_mean',unit='frames_per_second',value=59.5,label='Test game',status='valid',age_ms=0))
 helper=root/'bin/llcontrol';helper.write_text('#!/usr/bin/python3\nimport sys,time,json,os\nassert sys.argv[1:]==["mini","watch"]\nroot='+repr(str(root))+'\nframe=json.loads('+repr(json.dumps(frame))+')\nwhile True:\n frame["data"]["display"]["interface_language"]=open(root+"/language").read() if os.path.exists(root+"/language") else "en-US"\n print(json.dumps(dict(version=1,available=False,data=None) if os.path.exists(root+"/offline") else frame),flush=True)\n time.sleep(1)\n');helper.chmod(0o700)
 env=dict(os.environ,LLCONTROL_MINI_TEST_ROOT=str(root),PATH=str(root/'bin')+':'+os.environ.get('PATH',''),GSETTINGS_BACKEND='dconf',GTK_A11Y='none',GIO_USE_PORTALS='0',GDK_BACKEND='x11',GSK_RENDERER='cairo',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'));env.pop('WAYLAND_DISPLAY',None);env.pop('DISPLAY',None)
 process=subprocess.Popen(['xvfb-run','-a','-s','-screen 0 1400x1000x24','dbus-run-session','--',args.gjs,'-m',str(Path(__file__).with_name('preferences.test.js')),str(args.package.resolve())],env=env,start_new_session=True)
 try:code=process.wait(timeout=35)
 finally:
  try:os.killpg(process.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  if process.poll() is None:process.wait(timeout=5)
 if code==0:
  code=subprocess.run(['dbus-run-session','--',args.gjs,'-m',str(Path(__file__).with_name('persistence.test.js')),str(args.package.resolve())],env=env,timeout=10).returncode
 raise SystemExit(code)
