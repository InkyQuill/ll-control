#!/usr/bin/env python3
"""Fixed-budget 60-second release workload; owned MockBackend and codec children only."""
import json, os, socket, subprocess, tempfile, time
from pathlib import Path
repo = Path(__file__).resolve().parents[2]
root = Path(tempfile.mkdtemp(prefix='lconnect-media-resources-'))
(root/'fixture-marker').write_text('MockBackend only\n')
budgets = dict(seconds=60, minimum_fps=20, average_cpu_cores=2, peak_rss_bytes=512*1024*1024, sensor_age_ms=2000)
env = dict(os.environ, LCONNECT_SMOKE_ROOT=str(root), LCONNECT_MOCK_EXPORT_VIDEO='1',
    XDG_RUNTIME_DIR=str(root), XDG_CONFIG_HOME=str(root/'config'),
    XDG_CACHE_HOME=str(root/'cache'), XDG_DATA_HOME=str(root/'data'))
env.pop('LD_PRELOAD', None)
def rpc(method, params=None):
    with socket.socket(socket.AF_UNIX) as conn:
        conn.settimeout(30); conn.connect(str(root/'lconnectd.sock'))
        stream=conn.makefile('rwb');stream.write((json.dumps(dict(jsonrpc='2.0',id=1,method=method,params=params or {}))+'\n').encode());stream.flush()
        while True:
            reply=json.loads(stream.readline())
            if reply.get('id')==1:
                assert 'error' not in reply,reply
                return reply['result']
def wait(test, label, seconds=30):
    end=time.monotonic()+seconds
    while not test():
        assert process.poll() is None, 'fixture exited'
        if time.monotonic()>end:raise TimeoutError(label)
        time.sleep(.05)
def resources(pid):
    # Include children forked by any Tokio worker thread, and exited/reaped CPU.
    pending=[pid];seen=set();ticks=0;rss=0
    while pending:
        current=pending.pop()
        if current in seen:continue
        seen.add(current)
        try:
            for task in Path(f'/proc/{current}/task').iterdir():
                pending.extend(map(int,(task/'children').read_text().split()))
            fields=Path(f'/proc/{current}/stat').read_text().rsplit(')',1)[1].split()
            ticks+=sum(int(fields[n]) for n in [11,12,13,14])
            rss+=int(fields[21])*os.sysconf('SC_PAGE_SIZE')
        except FileNotFoundError:
            pass
    return ticks/os.sysconf('SC_CLK_TCK'),rss,len(seen)-1
def receipt():return json.loads((root/'lcd-video.json').read_text())
result={'budgets':budgets,'actual_hardware_access':False,'theme':'r2-video-four-sensors.json'}
with (root/'daemon.log').open('w') as log:
    process=subprocess.Popen([repo/'target/release/examples/desktop-mock'],env=env,stdout=log,stderr=subprocess.STDOUT)
    try:
        wait(lambda:(root/'lconnectd.sock').exists(),'socket')
        wait(lambda:len(rpc('list_devices'))==1,'device')
        dev=rpc('list_devices')[0]['id']
        theme=json.loads((repo/'desktop/scripts/fixtures/r2-video-four-sensors.json').read_text())
        rpc('set_lcd_theme',{'dev':dev,'theme':theme})
        wait(lambda:(root/'lcd-video.json').exists() and receipt()['frames']>=24,'warmup')
        before=receipt()['frames'];cpu_start=resources(process.pid)[0];start=time.monotonic()
        samples=[]
        while time.monotonic()-start<budgets['seconds']:
            cpu,rss,children=resources(process.pid)
            sensors=rpc('list_sensors')
            sensor=next(row for row in sensors if row['id']=='fixture:cpu')
            assert sensor['status']=='valid' and sensor['age_ms'] is not None,sensor
            samples.append(dict(elapsed=time.monotonic()-start,cpu_seconds=cpu-cpu_start,rss=rss,children=children,sensor_age_ms=sensor['age_ms'],frames=receipt()['frames']))
            time.sleep(.2)
        elapsed=time.monotonic()-start;cpu=resources(process.pid)[0]-cpu_start
        frames=receipt()['frames']-before
        result.update(seconds=elapsed,frames=frames,fps=frames/elapsed,average_cpu_cores=cpu/elapsed,
            peak_rss_bytes=max(s['rss'] for s in samples),max_sensor_age_ms=max(s['sensor_age_ms'] for s in samples))
        rpc('clear_lcd',{'dev':dev})
        wait(lambda:resources(process.pid)[2]==0,'codec children joined')
        time.sleep(.3)  # Allow the fixture observer to flush the last completed write.
        completed=receipt()
        decoded=json.loads(subprocess.check_output(['ffprobe','-v','error','-count_frames','-select_streams','v:0','-show_entries','stream=width,height,nb_read_frames','-of','json',str(root/'lcd-video.h264')],text=True))['streams'][0]
        assert decoded['width']==480 and decoded['height']==480 and int(decoded['nb_read_frames'])==completed['frames'],decoded
        result.update(decoded=decoded,codec_children_joined=True)
        (root/'samples.json').write_text(json.dumps(samples,indent=2)+'\n')
        (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
        assert result['fps']>=budgets['minimum_fps'],result
        assert result['average_cpu_cores']<=budgets['average_cpu_cores'],result
        assert result['peak_rss_bytes']<=budgets['peak_rss_bytes'],result
        assert result['max_sensor_age_ms']<=budgets['sensor_age_ms'],result
        result['ok']=True
        (root/'acceptance.json').write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result),flush=True)
    finally:
        (root/'stop').touch()
        try:process.wait(timeout=15)
        except subprocess.TimeoutExpired:process.kill();process.wait();raise
        print('Fixture directory:',root,flush=True)
