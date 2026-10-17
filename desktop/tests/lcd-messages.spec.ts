import {loadControls} from './editor-controls';
import { test, expect, type Page } from '@playwright/test';
import type { Theme } from '../src/lib/api';

type Operation = 'apply' | 'load' | 'localui' | 'preview';
type Language = 'en-US' | 'ru-RU';
interface Fixture {
  settings: { version: number; revision: number; temperature_unit: string; interface_language: Language };
  sourceOverride?: string | null; sourceError?: string | null;
  calls: string[]; active: Theme | null; failed: boolean; offline: boolean; persistenceError: string | null;
}
declare global { interface Window { lcdMessages: Fixture } }
const methods = { apply: 'set_lcd_theme', load: 'get_lcd_theme', localui: 'clear_lcd', preview: 'preview_lcd_editor' };
const buttons = {
  'en-US': { apply: 'Apply to LCD', load: 'Load active theme', localui: 'Restore LocalUI', preview: 'Refresh preview' },
  'ru-RU': { apply: 'Применить к LCD', load: 'Загрузить активную тему', localui: 'Вернуть LocalUI', preview: 'Обновить preview' },
};
const instructions = {
  'en-US': { apply: 'Theme application was not confirmed', load: 'Could not load the active theme',
    localui: 'LocalUI restoration was not confirmed', preview: 'Could not render the preview' },
  'ru-RU': { apply: 'Применение темы не подтверждено', load: 'Не удалось загрузить активную тему',
    localui: 'Возврат LocalUI не подтверждён', preview: 'Не удалось создать preview' },
};
async function setup(page: Page, language: Language, operation: Operation = 'apply') {
  await page.addInitScript(({ language, operation, methods }) => {
    localStorage.setItem('lconnect.interface-language', language);
    const f: Fixture = { settings: { version: 1, revision: 0, temperature_unit: 'celsius', interface_language: language },
      calls: [], active: { version: 1, name: 'Active theme', background: [0, 0, 0], layers: [] },
      failed: false, offline: false, persistenceError: null };
    window.lcdMessages = f;
    Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (_: string, req: { method?: string; params?: { theme: Theme } }) => {
        if (_ === 'desktop_set_language') return null;
      const method = req.method;
      if (method) f.calls.push(method);
      if (f.offline && (method === 'list_devices' || method === 'list_sensors')) throw new Error('Настройки');
      switch (method) {
        case 'get_display_settings': return structuredClone(f.settings);
        case 'list_devices': return [{ id: 'lcd', display_name: 'My pump', family: 'galahad2-lcdv2' }];
        case 'list_sensors': return [{ id: 'cpu', label: 'Настройки', unit: 'celsius', value: 45, status: 'valid', age_ms: 0 }];
        case 'get_lcd_state': return { source: f.sourceOverride !== undefined ? f.sourceOverride : f.active ? 'Theme' : 'Idle', last_error: f.sourceError ?? null,
          requested: {}, persistence: { pending: !!f.persistenceError, saved: !f.persistenceError, last_error: f.persistenceError } };
        case 'set_lcd_theme': case 'get_lcd_theme': case 'clear_lcd': case 'preview_lcd_editor':
          // A lost reply can follow a successful daemon mutation.
          if (method === 'set_lcd_theme') f.active = structuredClone(req.params!.theme);
          if (method === methods[operation] && !f.failed) { if(operation!=='preview')f.failed = true; throw new Error('Настройки'); }
          if (method === 'get_lcd_theme') return structuredClone(f.active);
          if (method === 'clear_lcd') f.active = null;
          if (method === 'preview_lcd_editor') return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII='), c => c.charCodeAt(0));
          return { source: f.active ? 'Theme' : 'Idle', last_error: null };
        case 'list_prepared_clips': case 'list_theme_assets': case 'list_named_theme_assets': return [];
        case 'list_profiles': return { version: 1, revision: 0, profiles: [] };
        default: return null;
      }
    } } });
  }, { language, operation, methods });
  await page.goto('/?view=display');
  await expect(page.locator('html')).toHaveAttribute('lang', language === 'en-US' ? 'en' : 'ru');
  await expect(page.getByText(language === 'en-US' ? 'Daemon connected' : 'Daemon подключён', { exact: true })).toBeVisible();
}
for (const language of ['en-US', 'ru-RU'] as const) {
  for (const operation of ['apply', 'load', 'localui'] as const) {
    test(`${operation} error keeps raw diagnostic, follows language and retries explicitly (${language})`, async ({ page }) => {
      await setup(page, language, operation);
      if(operation!=='apply')await loadControls(page);
      const name = 'Русское имя {1} <CPU>';
      await page.getByLabel(language === 'en-US' ? 'Name' : 'Название', { exact: true }).fill(name);
      await page.getByRole('button', { name: buttons[language][operation], exact: true }).click();
      const alert = page.getByRole('alert').filter({ hasText: 'Настройки' });
      await expect(alert).toBeVisible();
      await expect(alert).toContainText(instructions[language][operation]);
      const other: Language = language === 'en-US' ? 'ru-RU' : 'en-US';
      await page.evaluate(other => { window.lcdMessages.settings.interface_language = other; window.lcdMessages.settings.revision++; }, other);
      await expect(page.locator('html')).toHaveAttribute('lang', other === 'en-US' ? 'en' : 'ru');
      await expect(alert).toContainText(instructions[other][operation]);
      await expect(page.getByLabel(other === 'en-US' ? 'Name' : 'Название', { exact: true })).toHaveValue(name);
      expect(await page.evaluate(method => window.lcdMessages.calls.filter(call => call === method).length, methods[operation])).toBe(1);
      await page.getByRole('button', { name: buttons[other][operation], exact: true }).click();
      await expect(alert).toHaveCount(0);
      await expect(page.getByLabel(other === 'en-US' ? 'Name' : 'Название', { exact: true })).toHaveValue(operation === 'load' ? 'Active theme' : name);
      expect(await page.evaluate(method => window.lcdMessages.calls.filter(call => call === method).length, methods[operation])).toBe(2);
      const writes = await page.evaluate(() => window.lcdMessages.calls.filter(call => ['set_lcd_theme', 'clear_lcd', 'set_lcd_media'].includes(call)));
      expect(writes).toEqual(operation === 'apply' ? ['set_lcd_theme', 'set_lcd_theme'] : operation === 'localui' ? ['clear_lcd', 'clear_lcd'] : []);
    });
  }
  test(`loading without an active theme preserves the draft and uses an app-owned message (${language})`, async ({ page }) => {
    await setup(page, language);
    await page.evaluate(() => { window.lcdMessages.active = null; });
    const title = page.getByLabel(language === 'en-US' ? 'Name' : 'Название', { exact: true });
    await title.fill('My unsent draft');
    await loadControls(page);await page.getByRole('button', { name: buttons[language].load, exact: true }).click();
    await expect(page.getByRole('alert')).toHaveText(language === 'en-US' ? 'There is no active theme on the device' : 'На устройстве нет активной темы');
    await expect(title).toHaveValue('My unsent draft');
    expect(await page.evaluate(() => window.lcdMessages.calls.filter(call => ['set_lcd_theme', 'clear_lcd', 'get_lcd_theme'].includes(call)))).toEqual(['get_lcd_theme']);
  });
  for (const kind of ['connection', 'persistence'] as const) {
    test(`${kind} diagnostics are data even when equal to a resource key (${language})`, async ({ page }) => {
      await setup(page, language);
      await page.evaluate(kind => { if (kind === 'connection') window.lcdMessages.offline = true; else window.lcdMessages.persistenceError = 'Настройки'; }, kind);
      const alert = page.getByRole('alert').filter({ hasText: 'Настройки' });
      await expect(alert).toBeVisible();
      if (language === 'en-US') await expect(alert).toContainText(kind === 'connection' ? 'Could not connect to the daemon' : 'Could not save settings');
      expect(await page.evaluate(() => window.lcdMessages.calls.filter(call => ['set_lcd_theme', 'clear_lcd', 'set_lcd_media'].includes(call)))).toEqual([]);
    });
  }
}


for (const language of ['en-US', 'ru-RU'] as const) {
  for (const source of ['Idle', 'Jpeg', 'DiagnosticClip', 'Theme', 'Настройки', '<future-stream>{1}', null]) {
    test(`LCD source labels follow language without translating raw data (${language}, ${source})`, async ({ page }) => {
      await setup(page, language);
      await page.evaluate(source => { window.lcdMessages.sourceOverride = source; window.lcdMessages.sourceError = 'Настройки'; }, source);
      const labels = { 'en-US': { Idle: 'No software stream', Jpeg: 'Image', DiagnosticClip: 'Video', Theme: 'LCD theme' },
        'ru-RU': { Idle: 'Нет программного потока', Jpeg: 'Изображение', DiagnosticClip: 'Видео', Theme: 'Тема LCD' } };
      const expected = (lang: Language) => `${lang === 'en-US' ? 'Source:' : 'Источник:'} ${source === null ? '—' : labels[lang][source as keyof typeof labels['en-US']] ?? source}`;
      const state = page.locator('.visual .status > span').first();
      await expect(state).toHaveText(expected(language));
      await expect(page.locator('.visual .status > span').nth(1)).toHaveText('Настройки');
      await page.getByLabel(language === 'en-US' ? 'Name' : 'Название', { exact: true }).fill('Literal draft {1}');
      const other: Language = language === 'en-US' ? 'ru-RU' : 'en-US';
      await page.evaluate(other => { window.lcdMessages.settings.interface_language = other; window.lcdMessages.settings.revision++; }, other);
      await expect(page.locator('html')).toHaveAttribute('lang', other === 'en-US' ? 'en' : 'ru');
      await expect(state).toHaveText(expected(other));
      await expect(page.locator('.visual .status > span').nth(1)).toHaveText('Настройки');
      await expect(page.getByLabel(other === 'en-US' ? 'Name' : 'Название', { exact: true })).toHaveValue('Literal draft {1}');
      expect(await page.evaluate(() => window.lcdMessages.calls.filter(call => ['set_lcd_theme', 'clear_lcd', 'set_lcd_media'].includes(call)))).toEqual([]);
    });
  }
}

for(const language of ['en-US','ru-RU'] as const)test(`live canvas errors retain raw diagnostics across language changes and recover (${language})`,async({page})=>{
  await setup(page,language,'preview');
  const alert=page.getByRole('alert').filter({hasText:'Настройки'});
  await expect(alert).toContainText(instructions[language].preview);
  const other=language==='en-US'?'ru-RU':'en-US';
  await page.evaluate<void,Language>((other)=>{window.lcdMessages.settings.interface_language=other;window.lcdMessages.settings.revision++;},other);
  await expect(alert).toContainText(instructions[other].preview);
  await page.evaluate(()=>window.lcdMessages.failed=true);
  await expect(alert).toHaveCount(0);await expect(page.locator('.canvas img')).toBeVisible();
  expect(await page.evaluate(()=>window.lcdMessages.calls.filter(method=>['set_lcd_theme','clear_lcd'].includes(method)))).toEqual([]);
});
