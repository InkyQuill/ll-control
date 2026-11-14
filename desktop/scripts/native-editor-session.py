#!/usr/bin/env python3
"""Capture the production frontend on the current Wayland session, using only MockBackend.

Pass a seven-layer theme JSON (or a native-canvas result containing `theme`).
Raw screenshots and measurements are written to a temporary directory outside Git.
The fixture uses a private daemon, bus, settings and assets; it never opens HID.
"""
import argparse
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[2]


def rpc(root, method, params):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(30)
        connection.connect(str(root / 'llcontrold.sock'))
        with connection.makefile('rwb') as stream:
            stream.write((json.dumps(dict(jsonrpc='2.0', id=1, method=method, params=params))+'\n').encode())
            stream.flush()
            while True:
                reply = json.loads(stream.readline())
                if reply.get('id') == 1:
                    assert 'error' not in reply, reply
                    return reply['result']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--theme', type=Path, required=True)
    parser.add_argument('--layers', action='store_true', help='Also capture the full Layers pane')
    parser.add_argument('--video', action='store_true', help='Use a generated 480px video background and available mock temperature sensors')
    parser.add_argument('--release', action='store_true', help='Use optimized binaries for performance measurements')
    parser.add_argument('--language', choices=['en-US','ru-RU'], default='en-US')
    parser.add_argument('--size', choices=['1280x800','1024x768'], default='1280x800')
    parser.add_argument('--stress-assets', action='store_true', help='Replace two decorative layers with translucent PNG and fireworks')
    parser.add_argument('--soak-seconds', type=int, default=0)
    parser.add_argument('--no-screenshots', action='store_true')
    args = parser.parse_args()
    assert os.environ.get('WAYLAND_DISPLAY'), 'Run in the target Wayland desktop session'
    root = Path(tempfile.mkdtemp(prefix='llcontrol-editor-wayland-'))
    (root / 'fixture-marker').touch()
    wayland = Path(os.environ['WAYLAND_DISPLAY'])
    if not wayland.is_absolute():
        wayland = Path(os.environ['XDG_RUNTIME_DIR']) / wayland
    env = {k:v for k,v in os.environ.items() if not k.startswith('LLCONTROL_NATIVE_')}
    env.update(LLCONTROL_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root),
               XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
               XDG_CACHE_HOME=str(root/'cache'), WAYLAND_DISPLAY=str(wayland),
               GDK_BACKEND='wayland',
               LLCONTROL_NATIVE_EDITOR_VIEW='1', LLCONTROL_NATIVE_EDITOR_SIZE=args.size,
               LLCONTROL_NATIVE_DESKTOP_LANGUAGE=args.language)
    env['LLCONTROL_NATIVE_EDITOR_SOAK'] = '1' if args.soak_seconds else '0'
    binaries=REPO/'target'/('release' if args.release else 'debug')
    children = []
    with (root/'session.log').open('w') as log:
        try:
            children.append(subprocess.Popen([binaries/'examples/desktop-mock'],env=env,stdout=log,stderr=log,start_new_session=True))
            deadline=time.monotonic()+20
            while not (root/'llcontrold.sock').exists():
                assert time.monotonic()<deadline,'fixture startup timeout'
                time.sleep(.05)
            theme=json.loads(args.theme.read_text());theme=theme.get('theme',theme)
            devices=rpc(root,'list_devices',{})
            while not devices:
                assert time.monotonic()<deadline,'mock device discovery timeout'
                time.sleep(.05)
                devices=rpc(root,'list_devices',{})
            device=devices[0]['id']
            settings=rpc(root,'get_display_settings',{})
            settings['interface_language']=args.language
            rpc(root,'set_display_settings',dict(settings=settings))
            if args.video:
                clip=root/'source.h264'
                subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i','testsrc2=s=480x480:r=24','-frames:v','48','-c:v','libx264','-preset','ultrafast','-tune','zerolatency','-profile:v','baseline','-pix_fmt','yuv420p','-g','24','-threads','1',str(clip)],check=True)
                packets=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_entries','packet=pos,size,flags','-of','json',str(clip)]))['packets']
                data=clip.read_bytes()
                units=[dict(bytes=list(data[int(packet['pos']):int(packet['pos'])+int(packet['size'])]),keyframe='K' in packet['flags']) for packet in packets]
                asset=rpc(root,'import_prepared_clip',dict(name='Editor session video',clip=dict(fps=24,units=units)))
                theme['background_video']=asset['key']
                temperatures=[sensor for sensor in rpc(root,'list_sensors',{}) if sensor['unit']=='celsius']
                assert temperatures,'mock temperature sensor required'
                for layer in theme['layers']:
                    if layer['kind']=='sensor':layer['sensor_id']=temperatures[0]['id']
            if args.stress_assets:
                theme['version'] = 3
                png = subprocess.check_output(['ffmpeg','-v','error','-f','lavfi','-i',
                    'color=c=red@0.5:s=32x32,format=rgba','-frames:v','1','-c:v','png','-f','image2pipe','pipe:1'])
                key = rpc(root,'import_theme_asset',dict(bytes=list(png),name='Translucent overlay'))
                theme['layers'][-1] = dict(kind='image',asset=key,x=20,y=20,width=48,height=48,fit='stretch',opacity=100)
                theme['layers'][-2] = dict(kind='fireworks',effect=dict(intensity=1,colors=[[0,255,0]]*6,
                    dynamic=False,sensor=None,seed=42))
                theme['layers'].insert(0, theme['layers'].pop(-2))
            rpc(root,'set_lcd_theme',dict(dev=device,theme=theme))
            children.append(subprocess.Popen(['dbus-run-session','--',str(binaries/'llcontrol-gui')],env=env,stdout=log,stderr=log,start_new_session=True))
            deadline=time.monotonic()+90
            while not (root/'result.json').exists():
                assert time.monotonic()<deadline,'native view timeout'
                time.sleep(.1)
            result=json.loads((root/'result.json').read_text())
            assert result['ok'],result
            if not args.no_screenshots:
                subprocess.run(['spectacle','-b','-n','-a','-o',str(root/'editor.png')],check=True,timeout=30)
            if args.layers and not args.no_screenshots:
                deadline=time.monotonic()+10
                while json.loads((root/'result.json').read_text()).get('pane')!='layers':
                    assert time.monotonic()<deadline,'layer pane capture timeout'
                    time.sleep(.1)
                subprocess.run(['spectacle','-b','-n','-a','-o',str(root/'layers.png')],check=True,timeout=30)
            if args.soak_seconds:
                samples = []
                deadline = time.monotonic() + args.soak_seconds
                while time.monotonic() < deadline:
                    sample = dict(time=time.time(),playback=rpc(root,'get_lcd_playback_stats',dict(dev=device)),
                                  sensors=rpc(root,'list_sensors',{}),processes=[])
                    for proc in Path('/proc').iterdir():
                        if not proc.name.isdigit():
                            continue
                        try:
                            environment=(proc/'environ').read_bytes()
                            if str(root).encode() not in environment:
                                continue
                            stat=(proc/'stat').read_text().rsplit(')',1)[1].split()
                            sample['processes'].append(dict(pid=int(proc.name),ticks=int(stat[11])+int(stat[12]),
                                rss=int(stat[21])*os.sysconf('SC_PAGE_SIZE')))
                        except (PermissionError,FileNotFoundError,ProcessLookupError):
                            continue
                    samples.append(sample)
                    (root/'soak.json').write_text(json.dumps(samples))
                    time.sleep(5)
                result=json.loads((root/'result.json').read_text())
                assert result.get('edits',0) >= args.soak_seconds//4, result
            print(json.dumps(dict(root=str(root),language=args.language,size=args.size,result=result)),flush=True)
        finally:
            (root/'stop').touch()
            for child in reversed(children):
                os.killpg(child.pid, signal.SIGTERM)
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:os.killpg(child.pid, signal.SIGKILL);child.wait()


if __name__ == '__main__':
    main()
