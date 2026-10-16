import * as Mini from './mini.mjs';
const strings = {
    'en-US': {
        preferences: 'Preferences', save: 'Save slots', reload: 'Reload saved slots',
        saved: 'Slots saved.', changed: 'Settings changed elsewhere. Reload before saving.',
        failed: 'Could not confirm saved settings. Reload before retrying.',
        description: 'Six cached readings. Saving slots does not change LCD, cooling or lighting.',
    },
    'ru-RU': {
        preferences: 'Настройки', save: 'Сохранить слоты', reload: 'Загрузить сохранённые слоты',
        saved: 'Слоты сохранены.', changed: 'Настройки изменились в другом окне. Загрузите их перед сохранением.',
        failed: 'Сохранение настроек не подтверждено. Загрузите состояние перед повтором.',
        description: 'Шесть показаний из кеша. Сохранение слотов не меняет LCD, охлаждение или подсветку.',
    },
};
export function text(key, language = 'en-US') {
    return strings[language]?.[key] ?? strings['en-US'][key] ?? Mini.text(key, language);
}
