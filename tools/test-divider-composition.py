#!/usr/bin/env python3
"""Release-only owned video/four-sensor/divider acceptance on MockBackend; no HID."""
import argparse,json,os,socket,subprocess,tempfile,time,hashlib
from pathlib import Path
repo=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--without-dividers',action='store_true',help='Measure the same four live sensors without divider layers')
args=parser.parse_args()
root=Path(tempfile.mkdtemp(prefix='lconnect-divider-composition-'));root.chmod(0o700)
(root/'fixture-marker').write_text('MockBackend only\n')
env={key:value for key,value in os.environ.items() if not key.startswith('LCONNECT_')}
env.update(LCONNECT_SMOKE_ROOT=str(root),LCONNECT_MOCK_EXPORT_VIDEO='1',XDG_RUNTIME_DIR=str(root),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'))
def wait(test,label):
 end=time.monotonic()+45
 while not test():
  if process.poll() is not None:raise RuntimeError('fixture exited: '+str(process.returncode))
  if time.monotonic()>end:raise TimeoutError(label)
  time.sleep(.05)
def rpc(method,params=None):
 with socket.socket(socket.AF_UNIX) as peer:
  peer.settimeout(20);peer.connect(str(root/'lconnectd.sock'));stream=peer.makefile('rwb');stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
  while True:
   reply=json.loads(stream.readline())
   if reply.get('id')==1:
    if 'error' in reply:raise RuntimeError(str(reply))
    return reply['result']
log=(root/'daemon.log').open('w')
process=subprocess.Popen([repo/'target/release/examples/desktop-mock'],env=env,stdout=log,stderr=subprocess.STDOUT)
try:
 wait(lambda:(root/'lconnectd.sock').exists(),'mock socket');wait(lambda:len(rpc('list_devices'))==1,'mock device')
 dev=rpc('list_devices')[0]['id'];clip=next(c for c in rpc('list_prepared_clips') if c['is_default'])
 style=lambda x,y,size,align,color:{'x':x,'y':y,'size':size,'align':align,'color':color,'line_box':True,'shadow':{'x':0,'y':3,'blur':2,'color':[0,0,0],'opacity':20}}
 layers=[]
 for index,(x,y) in enumerate([(133,34),(347,34),(133,270),(347,270)]):
  layers.append({'kind':'sensor','sensor_id':'fixture:cpu','label':f'CPU{index}','unit':'celsius','temperature_unit':'celsius','decimals':1,'style':style(x,y,114,'center',[235,245,255]),'parts':{'label':style(x-8,y+114,46,'right',[71,201,137]),'unit':style(x+8,y+114,46,'left',[71,201,137])}})
 for axis,x,y,length in [('vertical',239,53,169),('horizontal',34,242,412),('vertical',239,264,166)]:
  layers.append({'kind':'divider','axis':axis,'x':x,'y':y,'length':length,'thickness':2,'color':[71,201,137],'opacity':100,'shadow':{'x':0,'y':0,'blur':5,'color':[0,0,0],'opacity':100}})
 theme={'version':2,'name':'Owned live quad with dividers','background':[0,0,0],'background_video':clip['key'],'layers':layers}
 if args.without_dividers:theme['layers']=[layer for layer in layers if layer['kind']!='divider']
 rpc('set_lcd_theme',{'dev':dev,'theme':theme});wait(lambda:(root/'lcd-video.json').exists(),'completed video unit')
 count=lambda:json.loads((root/'lcd-video.json').read_text())['frames']
 initial=count();started=time.monotonic();wait(lambda:count()>=initial+720,'720 paced complete units');elapsed=time.monotonic()-started;sampled=count()-initial
 active=rpc('get_lcd_theme',{'dev':dev});assert len([l for l in active['layers'] if l['kind']=='divider'])==(0 if args.without_dividers else 3)
 (root/'stop').touch();process.wait(timeout=20);assert process.returncode==0
 receipt=json.loads((root/'lcd-video.json').read_text())
 probe=subprocess.run(['ffprobe','-v','error','-select_streams','v:0','-count_frames','-show_entries','stream=width,height,nb_read_frames','-of','json',str(root/'lcd-video.h264')],check=True,capture_output=True,text=True,timeout=30)
 decoded=json.loads(probe.stdout)['streams'][0];assert (decoded['width'],decoded['height'])==(480,480) and int(decoded['nb_read_frames'])==receipt['frames']
 subprocess.run(['ffmpeg','-nostdin','-v','error','-xerror','-i',str(root/'lcd-video.h264'),'-f','null','-'],check=True,capture_output=True,timeout=30)
 report={'ok':sampled/elapsed>=20,'hardware':False,'release':True,'target_fps':24,'minimum_fps':20,'without_dividers':args.without_dividers,'sampled_frames':sampled,'seconds':elapsed,'observed_fps':sampled/elapsed,'completed_stream':receipt,'decoded':decoded,'stream_sha256':hashlib.sha256((root/'lcd-video.h264').read_bytes()).hexdigest(),'theme':active,'scope':'Actual MockBackend H.264 writes; video with four live CPU reads and optional three shadowed dividers; short software acceptance, not hardware/long-run qualification'}
 (root/'acceptance.json').write_text(json.dumps(report,indent=2)+'\n')
 assert report['ok'],(sampled,elapsed)
 print('PASS release divider/video composition:',root,flush=True)
finally:
 if process.poll() is None:
  (root/'stop').touch()
  try:process.wait(timeout=20)
  except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=5)
 log.close();print('Artifacts:',root,flush=True)
