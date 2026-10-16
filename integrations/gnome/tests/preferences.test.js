import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
import Adw from 'gi://Adw?version=1';
import System from 'system';
const loop = new GLib.MainLoop(null, false);
const root = GLib.getenv('LCONNECT_MINI_TEST_ROOT');
const assert = (value, message) => {if (!value) throw new Error(message);};
const sleep = ms => new Promise(resolve => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {resolve(); return GLib.SOURCE_REMOVE;}));
async function until(test, message, milliseconds = 5000) {
    const end = GLib.get_monotonic_time() + milliseconds * 1000;
    while (!test()) {if (GLib.get_monotonic_time() > end) throw new Error(message); await sleep(20);}
}
let controller = null, window = null, exitStatus = 0;
async function main() {
    Adw.init();
    const [{PreferencesController}, Mini] = await Promise.all([import(`file://${ARGV[0]}/preferences-ui.js`), import(`file://${ARGV[0]}/mini.mjs`)]);
    const source = Gio.SettingsSchemaSource.new_from_directory(`${ARGV[0]}/schemas`, Gio.SettingsSchemaSource.get_default(), false);
    const schema = source.lookup('org.gnome.shell.extensions.lconnect-mini', false);
    assert(schema !== null, 'Compiled schema missing');
    const settings = new Gio.Settings({settings_schema: schema});
    const fresh = settings.get_string('slot-config');
    window = new Adw.PreferencesWindow({title: 'L-Connect Mini preferences test', default_width: 760, default_height: 850});
    controller = new PreferencesController(window, settings);window.present();
    await until(() => controller.rows[0].reading.model !== null && controller.feed.frame.available, 'real GTK controls and CLI frame');
    assert(controller.page.title === 'Six readings', 'English default');
    const row = controller.rows[0];
    row.reading.selected = Mini.kinds.indexOf('gpu_power') + 1;
    await until(() => row.source.model.get_n_items() === 3, 'GPU choices from native feed');
    row.source.selected = 2;
    assert(controller._draft.slots[0].source === 'gpu:B:power', 'Explicit GPU B selection');
    assert(settings.get_string('slot-config') === fresh, 'Selection wrote settings before Save');
    const sourceModel = row.source.model;
    await sleep(1400);
    assert(row.source.model === sourceModel, 'Polling rebuilt source model');
    assert(controller._draft.slots[0].source === 'gpu:B:power', 'Polling lost source draft');
    controller.saveButton.emit('clicked');
    const saved = JSON.parse(settings.get_string('slot-config'));
    assert(saved.slots[0].source === 'gpu:B:power', 'Explicit Save failed');
    const other = new Gio.Settings({settings_schema: schema});
    assert(JSON.parse(other.get_string('slot-config')).slots[0].source === 'gpu:B:power', 'Second settings reader did not see saved slots');
    print('PASS GTK source selection/draft isolation/stable models/Save/GSettings readback');

    row.reading.selected = Mini.kinds.indexOf('fps') + 1;
    await until(() => row.source.visible && row.source.model.get_n_items() === 2, 'FPS native source choices');
    assert(row.source.model.get_string(0) === 'No FPS source selected', 'FPS must not promise shared GPU selection');
    row.source.selected = 1;
    assert(controller._draft.slots[0].source === 'mangohud:2f67616d65:1:2:logged_frame_mean', 'FPS explicit source draft');
    controller.saveButton.emit('clicked');
    assert(JSON.parse(settings.get_string('slot-config')).slots[0].source === 'mangohud:2f67616d65:1:2:logged_frame_mean', 'FPS Save/readback');
    print('PASS GTK FPS source selection/Save/readback');

    row.reading.selected = Mini.kinds.indexOf('cpu_temperature') + 1;
    const external = Mini.defaultConfig();external.slots[0] = null;
    other.set_string('slot-config', JSON.stringify(external));
    await until(() => !controller.saveButton.sensitive, 'External edit must block stale Save');
    assert(controller._draft.slots[0].kind === 'cpu_temperature', 'External edit discarded local draft');
    controller.reloadButton.emit('clicked');
    assert(controller._draft.slots[0] === null, 'Explicit reload');
    const future = '{"version":9,"slots":[]}';other.set_string('slot-config', future);controller.reload();
    await sleep(1100);assert(settings.get_string('slot-config') === future && controller._draft === null, 'Future settings silently reset');
    assert(!controller.saveButton.sensitive, 'Invalid settings allowed Save');
    controller.resetButton.emit('clicked');assert(settings.get_string('slot-config') === future, 'Reset changed settings without Save');
    controller.saveButton.emit('clicked');assert(Mini.parseConfig(settings.get_string('slot-config')) !== null, 'Explicit reset/Save failed');
    print('PASS stale settings/draft preservation/reload/future config/explicit reset');

    GLib.file_set_contents(`${root}/language`, 'ru-RU');
    await until(() => controller.page.title === 'Шесть показаний', 'Confirmed Russian switches GTK labels');
    assert(controller.saveButton.label === 'Сохранить слоты', 'Russian action text');
    assert(settings.get_string('language-cache') === 'ru-RU', 'Confirmed language cache');
    assert(controller.rows[0].reading.model.get_string(1) === 'Загрузка CPU', 'Russian reading names');
    GLib.file_set_contents(`${root}/offline`, '1');
    await until(() => !controller.feed.frame.available, 'Offline clears cache data');
    assert(controller.page.title === 'Шесть показаний', 'Offline lost chosen Russian');
    print('PASS Russian labels/confirmed language cache/offline language preservation');
    const pid = controller.feed.helperPid;
    controller.destroy();await controller.feed.shutdown();window.close();
    assert(!GLib.file_test(`/proc/${pid}`, GLib.FileTest.EXISTS), 'Preferences close left helper running');
    Gio.Settings.sync();
    print('PASS close/cancelled read/helper reaping');
}
main().then(() => loop.quit()).catch(async error => {console.error(error);controller?.destroy();await controller?.feed.shutdown();window?.close();exitStatus=1;loop.quit();});
loop.run();
System.exit(exitStatus);
