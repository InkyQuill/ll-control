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
        connection.connect(str(root / 'lconnectd.sock'))
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
    args = parser.parse_args()
    assert os.environ.get('WAYLAND_DISPLAY'), 'Run in the target Wayland desktop session'
    root = Path(tempfile.mkdtemp(prefix='lconnect-editor-wayland-'))
    (root / 'fixture-marker').touch()
    wayland = Path(os.environ['WAYLAND_DISPLAY'])
    if not wayland.is_absolute():
        wayland = Path(os.environ['XDG_RUNTIME_DIR']) / wayland
    env = {k:v for k,v in os.environ.items() if not k.startswith('LCONNECT_NATIVE_')}
    env.update(LCONNECT_SMOKE_ROOT=str(root), XDG_RUNTIME_DIR=str(root),
               XDG_CONFIG_HOME=str(root/'config'), XDG_DATA_HOME=str(root/'data'),
               XDG_CACHE_HOME=str(root/'cache'), WAYLAND_DISPLAY=str(wayland),
               GDK_BACKEND='wayland', WEBKIT_DISABLE_DMABUF_RENDERER='1',
               LCONNECT_NATIVE_EDITOR_VIEW='1', LCONNECT_NATIVE_EDITOR_SIZE=args.size,
               LCONNECT_NATIVE_DESKTOP_LANGUAGE=args.language)
    binaries=REPO/'target'/('release' if args.release else 'debug')
    children = []
    with (root/'session.log').open('w') as log:
        try:
            children.append(subprocess.Popen([binaries/'examples/desktop-mock'],env=env,stdout=log,stderr=log,start_new_session=True))
            deadline=time.monotonic()+20
            while not (root/'lconnectd.sock').exists():
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
            rpc(root,'set_lcd_theme',dict(dev=device,theme=theme))
            children.append(subprocess.Popen(['dbus-run-session','--',str(binaries/'lconnect-gui')],env=env,stdout=log,stderr=log,start_new_session=True))
            deadline=time.monotonic()+90
            while not (root/'result.json').exists():
                assert time.monotonic()<deadline,'native view timeout'
                time.sleep(.1)
            result=json.loads((root/'result.json').read_text())
            assert result['ok'],result
            subprocess.run(['spectacle','-b','-n','-a','-o',str(root/'editor.png')],check=True,timeout=30)
            if args.layers:
                deadline=time.monotonic()+10
                while json.loads((root/'result.json').read_text()).get('pane')!='layers':
                    assert time.monotonic()<deadline,'layer pane capture timeout'
                    time.sleep(.1)
                subprocess.run(['spectacle','-b','-n','-a','-o',str(root/'layers.png')],check=True,timeout=30)
            print(json.dumps(dict(root=str(root),language=args.language,size=args.size,result=result)),flush=True)
        finally:
            (root/'stop').touch()
            for child in reversed(children):
                os.killpg(child.pid, signal.SIGTERM)
                try:child.wait(timeout=10)
                except subprocess.TimeoutExpired:os.killpg(child.pid, signal.SIGKILL);child.wait()


if __name__ == '__main__':
    main()
