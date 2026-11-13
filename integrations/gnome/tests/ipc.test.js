import GLib from 'gi://GLib';
import System from 'system';
const root = GLib.getenv('LLCONTROL_MINI_TEST_ROOT');
const loop = new GLib.MainLoop(null, false);
let feed = null;
let exitStatus = 0;
const sleep = ms => new Promise(resolve => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {resolve(); return GLib.SOURCE_REMOVE;}));
async function until(test, message) {
    const end = GLib.get_monotonic_time() + 10000000;
    while (!test()) {if (GLib.get_monotonic_time() > end) throw new Error(message); await sleep(20);}
}
async function main() {
    const {Feed} = await import(`file://${ARGV[0]}/feed.js`);
    const Mini = await import(`file://${ARGV[0]}/mini.mjs`);
    feed = new Feed();
    feed.start(() => {});
    const config = Mini.defaultConfig();
    config.slots[0] = {kind: 'cpu_temperature', source: 'cpu:temperature'};
    const view = () => Mini.render(feed.frame, config, feed.elapsedMs);
    await until(() => view().cards[0].values[0] === '104.0 °F', 'Real CLI initial Fahrenheit frame');
    if (view().cards[0].sourceLabel !== 'Настройки') throw new Error('Raw source label changed');
    GLib.file_set_contents(`${root}/disconnect`, '1');
    await until(() => !feed.frame.available && !view().cards[0].available, 'IPC loss did not clear data');
    GLib.file_set_contents(`${root}/recover`, '1');
    await until(() => view().cards[0].values[0] === '122,0 °F' && view().language === 'ru-RU', 'New sample/Russian after reconnect');
    const pid = feed.helperPid;
    await feed.shutdown();
    if (GLib.file_test(`/proc/${pid}`, GLib.FileTest.EXISTS)) throw new Error('Real CLI helper not reaped');
    print('PASS GJS/real CLI/private Unix IPC/raw labels/Fahrenheit/offline/reconnect/new Russian sample/helper reaping');
}
main().then(() => loop.quit()).catch(async error => {console.error(error);await feed?.shutdown();exitStatus = 1;loop.quit();});
loop.run();
System.exit(exitStatus);
