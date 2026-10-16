import Gio from 'gi://Gio';
import System from 'system';
const source = Gio.SettingsSchemaSource.new_from_directory(`${ARGV[0]}/schemas`, Gio.SettingsSchemaSource.get_default(), false);
const settings = new Gio.Settings({settings_schema: source.lookup('org.gnome.shell.extensions.lconnect-mini', false)});
const {parseConfig, defaultConfig} = await import(`file://${ARGV[0]}/mini.mjs`);
const config = parseConfig(settings.get_string('slot-config'));
if (JSON.stringify(config) !== JSON.stringify(defaultConfig()) || settings.get_string('language-cache') !== 'ru-RU') {
    console.error('Private dconf settings did not survive separate process/session restart');
    System.exit(1);
}
print('PASS saved slots/Russian cache persist across separate process/private bus restart');
