import assert from 'node:assert/strict';
import { translate } from '../src/lib/localization.ts';
assert.equal(translate('Settings', 'en-US'), 'Settings');
assert.equal(translate('Settings', 'ru-RU'), 'Настройки');
assert.equal(translate('  Settings\n', 'en-US'), '  Settings\n');
assert.equal(
  translate('A device supplied this message', 'en-US'),
  'A device supplied this message',
);
assert.equal(translate('Select a source', 'en-US'), 'Select a source');
assert.equal(translate('Unknown {0}', 'en-US', '<sensor> {1}'), 'Unknown <sensor> {1}');
assert.equal(translate('Файл {0}', 'ru-RU', 'Пользовательский файл'), 'Файл Пользовательский файл');
assert.equal(translate(null, 'en-US'), '');
console.log('Localization fallback, whitespace and parameter preservation passed');
assert.equal(
  translate('Enter an integer from {0} to {1}', 'en-US', 20, 100),
  'Enter an integer from 20 to 100',
);
assert.equal(translate('Для User выберите 5 датчиков', 'en-US'), 'Для User выберите 5 датчиков');
assert.equal(
  translate('Sensors required for {0}: {1}', 'en-US', 'Мой датчик {1}', 3),
  'Sensors required for Мой датчик {1}: 3',
);
