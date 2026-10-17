#!/usr/bin/env python3
"""Native media import acceptance on private Xvfb, private IPC and MockBackend."""
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time

repo = Path(__file__).resolve().parents[2]
if '--inside-xvfb' not in sys.argv:
    # Portal activation inherits the bus environment, before per-GUI overrides.
    # Keep its runtime/config private as well as the display and IPC fixture.
    session = Path(tempfile.mkdtemp(prefix='lconnect-media-session-'))
    session.chmod(0o700)
    inherited = {key:value for key,value in os.environ.items() if not key.startswith('LCONNECT_NATIVE_')}
    env = dict(inherited, LCONNECT_KEYBOARD_ISOLATED='1', XDG_RUNTIME_DIR=str(session),
               XDG_CONFIG_HOME=str(session/'config'), XDG_DATA_HOME=str(session/'data'),
               XDG_CACHE_HOME=str(session/'cache'), DBUS_SYSTEM_BUS_ADDRESS=f'unix:path={session}/no-system-bus')
    for key in ['DISPLAY','WAYLAND_DISPLAY','AT_SPI_BUS_ADDRESS','DBUS_SESSION_BUS_ADDRESS']:
        env.pop(key,None)
    try:
        result = subprocess.call(['xvfb-run','-a','-s','-screen 0 1600x1200x24','dbus-run-session','--',sys.executable,__file__,'--inside-xvfb',*sys.argv[1:]],env=env)
    finally:
        target=session/'doc'
        mounted=lambda:any(row.split()[4]==str(target) for row in Path('/proc/self/mountinfo').read_text().splitlines())
        if mounted():
            cleanup=subprocess.run(['fusermount3','-uz','--',str(target)],capture_output=True,text=True,timeout=5)
            # The private portal may unmount concurrently after its bus exits.
            if cleanup.returncode and mounted():
                raise RuntimeError(f'Private document portal cleanup failed: {cleanup.stderr}')
    raise SystemExit(result)
assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED') == '1'
root = Path(tempfile.mkdtemp(prefix='lconnect-native-media-'))
(root / 'fixture-marker').write_text('MockBackend only\n')
(root / 'native-tmp').mkdir()
(root / 'bin').mkdir()
real_ffmpeg = shutil.which('ffmpeg')
assert real_ffmpeg
for name in ['fixture.mp4', 'fixture.gif']:
    subprocess.run([real_ffmpeg, '-nostdin', '-v', 'error', '-f', 'lavfi', '-i', 'color=c=red:s=320x160:r=24'+(',drawbox=x=0:y=0:w=160:h=160:color=blue:t=fill' if name.endswith('.mp4') else ''), *(['-frames:v','586','-c:v','libvpx-vp9','-deadline','realtime','-cpu-used','8'] if name.endswith('.mp4') else ['-t','0.5']), '-threads', '1', str(root/name)], check=True, timeout=15)
(root / 'broken.mp4').write_bytes(b'not media')
subprocess.run([real_ffmpeg,'-nostdin','-v','error','-f','lavfi','-i','color=c=blue:s=128x64:r=24:d=2','-c:v','libx264','-pix_fmt','yuv420p','-threads','1',str(root/'editor.mp4')],check=True,timeout=15)
# The GUI alone gets this wrapper. Normal cases exec the actual FFmpeg; only
# deliberate cancel/close cases pause before exec and expose their owned PID.
wrapper = root / 'bin/ffmpeg'
wrapper.write_text(f'''#!{sys.executable}
import json,os,pathlib,sys,time
root=pathlib.Path({str(root)!r})
args=sys.argv[1:]
if args[-1].endswith('clip.h264'):
    record={{'pid':os.getpid(),'source':args[args.index('-i')+1],'output':args[-1]}}
    p=root/'encoder.tmp';p.write_text(json.dumps(record));p.replace(root/'encoder.json')
    if (root/'slow-encode').exists(): time.sleep(30)
os.execv({real_ffmpeg!r}, [{real_ffmpeg!r}, *args])
''')
wrapper.chmod(0o700)
env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root), XDG_CONFIG_DIRS=str(root/'system-config'),
           XDG_CONFIG_HOME=str(root/'config'), XDG_CACHE_HOME=str(root/'cache'),
           XDG_DATA_HOME=str(root/'data'), GDK_BACKEND='x11', WEBKIT_DISABLE_DMABUF_RENDERER='1',
           LC_ALL='C.UTF-8', AT_SPI_BUS_ADDRESS=f'unix:path={root}/a11y.sock', NO_AT_BRIDGE='0')
env.pop('WAYLAND_DISPLAY', None)
gui_env = dict(env, LCONNECT_NATIVE_MEDIA='1', TMPDIR=str(root/'native-tmp'), PATH=f'{root}/bin:{env["PATH"]}')
processes, logs, dialogs, cancellations = [], [], [], []
fixture = None

def launch(command, name, process_env=env):
    log = (root/f'{name}.log').open('w'); logs.append(log)
    process = subprocess.Popen(command, env=process_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    processes.append(process)
    return process

def wait(predicate, why, seconds=10):
    deadline = time.monotonic()+seconds
    while not predicate():
        if time.monotonic()>deadline: raise TimeoutError(why)
        time.sleep(.02)

def xdo(*args):
    return subprocess.check_output(['xdotool', *args], env=env, text=True, timeout=5).strip()

def active():
    p = subprocess.run(['xdotool','getactivewindow'],env=env,text=True,capture_output=True,timeout=5)
    return p.stdout.strip() if p.returncode==0 else None

def rpc(method, params=None):
    with socket.socket(socket.AF_UNIX) as conn:
        conn.settimeout(10); conn.connect(str(root/'lconnectd.sock'))
        stream=conn.makefile('rwb')
        stream.write((json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})+'\n').encode());stream.flush()
        while True:
            result=json.loads(stream.readline())
            if result.get('id')==1:
                if 'error' in result: raise RuntimeError(result['error'])
                return result['result']

def start_fixture():
    global fixture
    (root/'stop').unlink(missing_ok=True)
    (root/'lconnectd.sock').unlink(missing_ok=True)
    fixture=launch([repo/'target/debug/examples/desktop-mock'],f'fixture-{len(processes)}')
    wait(lambda:(root/'lconnectd.sock').exists() or fixture.poll() is not None,'mock socket')
    assert fixture.poll() is None
    wait(lambda:len(rpc('list_devices'))==1,'mock attachment')
    # The scripted selectors below deliberately exercise Russian. Fresh stores
    # correctly default to English; choose the test locale through normal RPC.
    settings=rpc('get_display_settings')
    if settings.get('interface_language')!='ru-RU':
        settings['interface_language']='ru-RU'
        rpc('set_display_settings',{'settings':settings})

def stop_fixture():
    (root/'stop').touch();fixture.wait(timeout=10);assert fixture.returncode==0

def ack(sequence):
    (root/'media-ack.tmp').write_text(str(sequence));(root/'media-ack.tmp').replace(root/'media-ack')

def encoder():
    wait(lambda:(root/'encoder.json').exists(),'owned encoder PID')
    record=json.loads((root/'encoder.json').read_text())
    assert Path(f'/proc/{record["pid"]}').exists(), 'encoder already exited'
    for key in ['source','output']:
        assert Path(record[key]).is_relative_to(root/'native-tmp')
    return record

def assert_clean(record):
    wait(lambda:not Path(f'/proc/{record["pid"]}').exists(),'encoder reap')
    for key in ['source','output']:
        wait(lambda:not Path(record[key]).parent.exists(),'staging directory cleanup')

try:
    launch(['dbus-daemon','--config-file=/usr/share/defaults/at-spi2/accessibility.conf',f'--address={env["AT_SPI_BUS_ADDRESS"]}','--nofork'],'accessibility-bus')
    wait(lambda:(root/'a11y.sock').exists(),'private accessibility bus')
    # Keep an accessibility client registered across repeated chooser probes,
    # as in native-keyboard; short-lived clients alone can observe an empty registry.
    listener=launch([sys.executable,repo/'desktop/scripts/native-atspi-events.py'],'accessibility-events')
    wait(lambda:(root/'accessibility-events-ready').exists() or listener.poll() is not None,
         'persistent accessibility listener')
    assert listener.poll() is None
    launch(['openbox'],'wm')
    start_fixture()
    gui=launch([repo/'target/debug/lconnect-gui'],'gui',gui_env)
    window=None; sequence=0; closing=None
    deadline=time.monotonic()+150
    while gui.poll() is None:
        if time.monotonic()>deadline: raise TimeoutError('native media workflow')
        step_file=root/'media-step.json'
        if not step_file.exists(): time.sleep(.02);continue
        step=json.loads(step_file.read_text())
        if step['sequence']<=sequence: time.sleep(.02);continue
        assert step['sequence']==sequence+1
        sequence=step['sequence'];action=step['action']
        if window is None:
            window=xdo('search','--onlyvisible','--name','^L-Connect Linux$').splitlines()[0]
            xdo('windowactivate','--sync',window)
        if action.startswith('chooser_'):
            if action=='chooser_slow':
                (root/'slow-encode').touch();(root/'encoder.json').unlink(missing_ok=True)
            kind={'chooser_bad':'bad_media','chooser_gif':'gif','chooser_editor':'editor'}.get(action,'media')
            filename={'bad_media':'broken.mp4','gif':'fixture.gif','media':'fixture.mp4','editor':'editor.mp4'}[kind]
            xdo('key','--clearmodifiers','Return')
            wait(lambda:active() not in {None,window},'GTK chooser opening')
            dialog=active()
            props=subprocess.check_output(['xprop','-id',dialog,'WM_TRANSIENT_FOR','_NET_WM_WINDOW_TYPE'],env=env,text=True,timeout=5)
            assert '_NET_WM_WINDOW_TYPE_DIALOG' in props
            match=re.search(r'window id # (0x[0-9a-fA-F]+)',props)
            assert match and str(int(match[1],16))==window,props
            observation={'action':action,'title':xdo('getwindowname',dialog),'properties':props}
            if action=='chooser_cancel': xdo('key','--clearmodifiers','Escape')
            else:
                xdo('windowactivate','--sync',dialog)
                xdo('key','--clearmodifiers','ctrl+l')
                subprocess.check_output([sys.executable,repo/'desktop/scripts/native-file-chooser.py','location','0',kind],env=env,timeout=8)
                xdo('key','--clearmodifiers','ctrl+a')
                xdo('type','--clearmodifiers','--delay','1',str(root/filename))
                # A fresh AT-SPI client can briefly see an empty application tree.
                # Re-poll the same open chooser; never repeat typing or opening it.
                probe=[sys.executable,repo/'desktop/scripts/native-file-chooser.py','chooser','0',kind]
                try:
                    inspected=subprocess.check_output(probe,env=env,timeout=8)
                except subprocess.CalledProcessError:
                    inspected=subprocess.check_output(probe,env=env,timeout=8)
                observation['accessibility']=json.loads(inspected)
                xdo('windowactivate','--sync',dialog)
                xdo('key','--clearmodifiers','alt+o')
            wait(lambda:active()==window,'GTK chooser closing')
            dialogs.append(observation)
        elif action=='cancel_worker':
            record=encoder()
            if '--fail-after-encoder' in sys.argv: raise RuntimeError('Injected harness failure with active encoder')
            xdo('key','--clearmodifiers','Return');assert_clean(record)
            cancellations.append(record);(root/'slow-encode').unlink()
        elif action in {'editor_saved','editor_reedited'}:
            clips=rpc('list_prepared_clips')
            expected_frames=12 if action=='editor_saved' else 6
            expected_name='editor.mp4' if action=='editor_saved' else 'fixture.mp4'
            clip=next(c for c in clips if c['name']==expected_name and c['frames']==expected_frames)
            assert clip['frames']==expected_frames,clip
            unit=rpc('load_media_first_frame',{'key':clip['key']})
            encoded=root/'editor-first.h264';encoded.write_bytes(bytes(unit['bytes']))
            rgb=subprocess.run([real_ffmpeg,'-v','error','-i',str(encoded),'-frames:v','1','-threads','1','-pix_fmt','rgb24','-f','rawvideo','pipe:1'],capture_output=True,check=True,timeout=10).stdout
            assert len(rgb)==480*480*3
            center=list(rgb[3*(240*480+240):3*(240*480+240)+3])
            if action=='editor_saved': assert min(center)>245,center
            else: assert center[0]>230 and max(center[1:])<20,center
            (root/('editor-accepted.json' if action=='editor_saved' else 'gallery-editor-accepted.json')).write_text(json.dumps({'clip':clip,'center_rgb':center,'frames':expected_frames}))
        elif action=='editor_recorded':
            clips=[c for c in rpc('list_prepared_clips') if c['name']=='fixture.mp4' and 1<c['frames']<100]
            assert len(clips)==1,clips
            clip=clips[0]
            files=list((root/'settings').rglob(clip['key']+'.lcm'));assert len(files)==1
            stored=files[0].read_bytes();assert stored[:8]==b'LCMEDIA2'
            header_size=int.from_bytes(stored[8:12],'little')
            encoded=root/'recorded.h264';encoded.write_bytes(stored[12+header_size:])
            rgb=subprocess.run([real_ffmpeg,'-v','error','-i',str(encoded),'-threads','1','-vf','crop=2:2:240:240','-pix_fmt','rgb24','-f','rawvideo','pipe:1'],capture_output=True,check=True,timeout=15).stdout
            assert len(rgb)==clip['frames']*12,(len(rgb),clip)
            colors=[list(rgb[i:i+3]) for i in range(0,len(rgb),12)]
            blue=lambda c:c[2]>230 and max(c[:2])<20
            red=lambda c:c[0]>230 and max(c[1:])<20
            first_red=next((i for i,c in enumerate(colors) if red(c)),None)
            assert first_red is not None and first_red>0,colors
            assert all(blue(c) for c in colors[:first_red]) and all(red(c) for c in colors[first_red:]),colors
            (root/'recorded-accepted.json').write_text(json.dumps({'clip':clip,'center_colors':colors,'blue_frames':first_red,'red_frames':len(colors)-first_red}))
        elif action=='long_loop':
            device=rpc('list_devices')[0]['id']
            wait(lambda:rpc('get_lcd_playback_stats',{'dev':device})['sent_frames']>=587,'complete long-video loop',40)
            stats=rpc('get_lcd_playback_stats',{'dev':device})
            assert stats['failed_transactions']==0
            (root/'long-playback.json').write_text(json.dumps(stats))
        elif action=='seed_long_v2':
            encoded=root/'long-first.h264'
            subprocess.run([real_ffmpeg,'-nostdin','-v','error','-f','lavfi','-i','color=c=cyan:s=480x480:r=24','-frames:v','1','-c:v','libx264','-preset','ultrafast','-tune','zerolatency','-profile:v','baseline','-pix_fmt','yuv420p','-threads','1','-f','h264',str(encoded)],check=True,timeout=15)
            unit={'bytes':list(encoded.read_bytes()),'keyframe':True}
            key='d'*32
            rpc('media_upload',{'request':{'action':'begin','id':key,'name':'Long v2 cyan','frames':586,'encoded_bytes':len(unit['bytes'])*586}})
            for index in range(586):
                rpc('media_upload',{'request':{'action':'chunk','id':key,'index':index,'unit':unit}})
            receipt=rpc('media_upload',{'request':{'action':'finish','id':key}})
            assert receipt['phase']=='published' and receipt['asset']['frames']==586
            frame=rpc('load_media_first_frame',{'key':key});assert frame==unit
            (root/'v2-thumbnail-seed.json').write_text(json.dumps({'key':key,'frames':586,'first_au_bytes':len(unit['bytes']),'receipt':receipt}))
        elif action=='restart_daemon':
            before=[c for c in rpc('list_prepared_clips') if not c.get('is_default')];assert len(before)==2
            stop_fixture();start_fixture()
            assert [c for c in rpc('list_prepared_clips') if not c.get('is_default')]==before
        elif action=='close_worker':
            closing=encoder()
            ack(sequence)
            window=xdo('search','--onlyvisible','--name','^L-Connect Linux$').splitlines()[0]
            xdo('windowquit',window)
            break
        else: raise AssertionError(action)
        ack(sequence)
    gui.wait(timeout=15)
    result=json.loads((root/'result.json').read_text())
    assert gui.returncode==0 and result['ok'],result
    close_log=(root/'gui.log').read_text()
    for marker in ['native close: request admitted','native close: group worker joined (true)','native close: all workers joined']:assert marker in close_log,marker
    assert closing is not None,'close workflow not reached'
    assert_clean(closing)
    clips=[c for c in rpc('list_prepared_clips') if not c.get('is_default')];assert len(clips)==2
    assert sorted(c['key'] for c in clips)==sorted(result['keys'])
    # Managed imports must no longer depend on their original user files.
    for name in ['fixture.mp4', 'fixture.gif', 'editor.mp4']:
        (root/name).unlink()
    stop_fixture();start_fixture()
    assert [c for c in rpc('list_prepared_clips') if not c.get('is_default')]==clips
    dev=rpc('list_devices')[0]['id']
    for clip in clips:
        rpc('set_lcd_media',{'dev':dev,'key':clip['key']})
        wait(lambda:rpc('get_lcd_playback_stats',{'dev':dev})['sent_frames']>=2,
             'managed playback after original deletion')
        assert rpc('get_lcd_media',{'dev':dev})==clip['key']
        rpc('clear_lcd',{'dev':dev})
        assert rpc('get_lcd_media',{'dev':dev}) is None
    result['original_sources_removed_and_replayed']=True
    # Decode the persisted bytes themselves after the daemon restart.
    decoded=[]
    for clip in clips:
        files=list((root/'settings').rglob(clip['key']+'.lcm'))
        assert len(files)==1,files
        stored=files[0].read_bytes();assert stored[:8]==b'LCMEDIA2'
        header_size=int.from_bytes(stored[8:12],'little')
        header=json.loads(stored[12:12+header_size])
        assert len(header['units'])==clip['frames']
        encoded=root/(clip['key']+'.h264')
        encoded.write_bytes(stored[12+header_size:])
        all_frames=subprocess.run([real_ffmpeg,'-v','error','-i',str(encoded),'-threads','1','-f','framemd5','-'],capture_output=True,check=True,timeout=20).stdout.decode()
        count=sum(1 for line in all_frames.splitlines() if line and not line.startswith('#'))
        assert count==clip['frames']
        if clip['name'].endswith('.mp4'): assert count==586
        first=rpc('load_media_first_frame',{'key':clip['key']})
        assert bytes(first['bytes'])==encoded.read_bytes()[:header['units'][0]['length']]
        output=subprocess.run([real_ffmpeg,'-v','error','-i',str(encoded),'-frames:v','1','-threads','1','-pix_fmt','rgb24','-f','rawvideo','pipe:1'],capture_output=True,check=True,timeout=10).stdout
        assert len(output)==480*480*3
        top=output[3*240:3*240+3]
        if clip['name'].endswith('.mp4'): assert all(c<8 for c in top),top
        else: assert top[0]>230 and top[1]<15 and top[2]<15,top
        decoded.append({'name':clip['name'],'bytes':len(output),'top_pixel':list(top),'decoded_frames':count,'format':'v2'})
    result.update({'dialogs':dialogs,'cancel_reaped':cancellations,'close_reaped':closing,'decoded':decoded,'normal_close_exit':gui.returncode})
    (root/'acceptance.json').write_text(json.dumps(result,ensure_ascii=False,indent=2))
    print('PASS native media import; evidence:',root)
finally:
    (root/'stop').touch()
    for process in reversed(processes):
        # Every group was created by launch(start_new_session=True); kill the
        # owned descendants even if their GUI parent exited before the failure.
        try: os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError: pass
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired: pass
        try: os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError: pass
        process.wait(timeout=5)
    shutil.rmtree(root/'native-tmp', ignore_errors=True)
    for log in logs:log.close()
    print('Fixture directory:',root)
