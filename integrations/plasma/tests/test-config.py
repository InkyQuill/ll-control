#!/usr/bin/env python3
"""QML keyboard fixture: private config/runtime/session bus, synthetic CLI only."""
import json,os,subprocess,sys,tempfile
from pathlib import Path
build=Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='lconnect-mini-config-') as tmp:
 root=Path(tmp);(root/'bin').mkdir()
 def sensor(key,unit,value,label):return dict(id=key,unit=unit,value=value,label=label,status='valid',age_ms=0)
 a=sensor('gpu:A:power','watts',72,'GPU A');b=sensor('gpu:B:power','watts',99,'Настройки')
 frame=dict(version=1,available=True,data=dict(display=dict(temperature_unit='celsius',interface_language='en-US'),sensors=[a,b],defaults=dict(candidates=[dict(role='gpu_power',reading=a),dict(role='gpu_power',reading=b)],values=[])))
 frame['data']['sensors'].append(dict(id='mangohud:2f67616d65:1:2:logged_frame_mean',unit='frames_per_second',value=59.5,label='Test game',status='valid',age_ms=0))
 helper=root/'bin/lconnectctl';helper.write_text('#!/usr/bin/python3\nimport sys,time\nassert sys.argv[1:]==["mini","watch"]\nwhile True:\n print('+repr(json.dumps(frame))+',flush=True)\n time.sleep(1)\n');helper.chmod(0o700)
 env=dict(os.environ,PATH=str(root/'bin')+':'+os.environ.get('PATH',''),QT_QPA_PLATFORM='offscreen',QML_IMPORT_PATH=str(build/'qml'),XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),XDG_DATA_HOME=str(root/'data'))
 result=subprocess.run(['dbus-run-session','--','/usr/lib/qt6/bin/qmltestrunner','-input',str(build/'qml-tests'),'-import',str(build/'qml')],env=env,timeout=35)
 raise SystemExit(result.returncode)
