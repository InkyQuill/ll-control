import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import System from 'system';
const loop = new GLib.MainLoop(null, false);
let currentFeed=null,exitStatus=0;
const root = GLib.getenv('LCONNECT_MINI_TEST_ROOT');
const assert = (value, message) => {if (!value) throw new Error(message);};
const sleep = ms => new Promise(resolve => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {resolve(); return GLib.SOURCE_REMOVE;}));
async function until(test, message, milliseconds = 4000) {
    const end = GLib.get_monotonic_time() + milliseconds * 1000;
    while (!test()) {if (GLib.get_monotonic_time() > end) throw new Error(message); await sleep(20);}
}
function script(body) {
    const path = `${root}/bin/lconnectctl`;
    GLib.file_set_contents(path, `#!/usr/bin/python3\nimport sys,time,json,os\nassert sys.argv[1:]==['mini','watch']\n${body}\n`);
    Gio.File.new_for_path(path).set_attribute_uint32('unix::mode', 0o700, Gio.FileQueryInfoFlags.NONE, null);
}
async function main() {
    const {Feed} = await import(`file://${ARGV[0]}/feed.js`);
    script("line=(json.dumps({'version':1,'available':True,'data':{'name':'Настройки','sample':1}},ensure_ascii=False)+'\\n').encode()\ni=line.index('Настройки'.encode())+1\nsys.stdout.buffer.write(line[:i]);sys.stdout.flush();time.sleep(.05)\nsys.stdout.buffer.write(line[i:]+b'{\"version\":1,\"available\":true,\"data\":{\"sample\":2}}\\n');sys.stdout.flush();time.sleep(20)");
    let feed = (currentFeed = new Feed()), frames = [], updates = 0;
    feed.start(frame => {updates++; if (frame.available) frames.push(frame);});
    await until(() => feed.frame.data?.sample === 2, 'fragmented frames');
    assert(frames.some(f => f.data.name === 'Настройки'), 'UTF-8 source text changed');
    const pid = feed.helperPid;
    await feed.shutdown();
    const after = updates; await sleep(1100);
    assert(updates === after, 'UI callback after stop');
    assert(!GLib.file_test(`/proc/${pid}`, GLib.FileTest.EXISTS), 'owned helper not reaped');
    print('PASS fragmented UTF-8/multiple frames/disable/reaping/no callbacks');

    for (const bad of ["sys.stdout.write('not JSON\\n')", "sys.stdout.write('x'*(1024*1024+1))", "sys.stdout.buffer.write(b'\\xff\\n')"]) {
        script(`print('{"version":1,"available":true,"data":{"sample":1}}',flush=True)\ntime.sleep(.2)\n${bad}\nsys.stdout.flush();time.sleep(20)`);
        feed = (currentFeed = new Feed());feed.start(() => {});
        await until(() => feed.frame.available, 'valid sample before rejection');
        await until(() => !feed.frame.available, 'bad frame must clear old values');
        assert(feed.frame.data === null, 'old data retained');await feed.shutdown();
    }
    print('PASS malformed/oversized/invalid UTF-8 frames clear data');

    script("p=os.path.join(os.path.dirname(__file__),'count')\nn=int(open(p).read())+1 if os.path.exists(p) else 1\nopen(p,'w').write(str(n))\nprint(json.dumps({'version':1,'available':True,'data':{'sample':n}}),flush=True)\nif n>1:time.sleep(20)");
    feed = (currentFeed = new Feed());feed.start(() => {});
    await until(() => feed.frame.data?.sample === 2, 'restart after EOF', 6000);await feed.shutdown();
    print('PASS helper exit/restart recovers new frame');

    script("print('{\"version\":1,\"available\":true,\"data\":{\"sample\":1}}',flush=True)\ntime.sleep(5.6)\nprint('{\"version\":1,\"available\":true,\"data\":{\"sample\":2}}',flush=True)\ntime.sleep(20)");
    feed = (currentFeed = new Feed());feed.start(() => {});
    await until(() => feed.frame.available, 'initial sample');
    await until(() => !feed.frame.available, 'silent stream expiry', 7000);
    await until(() => feed.frame.data?.sample === 2, 'late fresh stream recovery');await feed.shutdown();
    print('PASS monotonic age expiry/late sample recovery');

    Gio.File.new_for_path(`${root}/bin/lconnectctl`).delete(null);
    feed = (currentFeed = new Feed());feed.start(() => {});await sleep(1100);assert(!feed.frame.available && feed.helperPid === null, 'missing CLI fabricated data');await feed.shutdown();
    print('PASS missing helper stays unavailable and stops retries');
}
main().then(() => loop.quit()).catch(async error => {console.error(error);await currentFeed?.shutdown();exitStatus=1;loop.quit();});
loop.run();
System.exit(exitStatus);
