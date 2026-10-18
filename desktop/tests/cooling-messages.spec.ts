import { test, expect, type Page } from '@playwright/test';

type Language = 'en-US' | 'ru-RU';
type Operation = 'apply' | 'preset' | 'poll' | 'runtime' | 'validation';
interface Fixture {
  language: Language; revision: number; calls: string[]; failed: boolean;
  policy: unknown; percent: number;
}
declare global { interface Window { coolingMessages: Fixture } }
const names = {
  'en-US': { cooling: 'Cooling', apply: 'Apply cooling', source: 'Temperature source', mode: 'Mode' },
  'ru-RU': { cooling: 'Охлаждение', apply: 'Применить охлаждение', source: 'Источник температуры', mode: 'Режим' },
};
const contexts = {
  'en-US': { apply: 'Cooling application was not confirmed:', preset: 'Could not load the cooling preset:',
    poll: 'Could not refresh cooling state:', runtime: 'Cooling control error:' },
  'ru-RU': { apply: 'Применение охлаждения не подтверждено:', preset: 'Не удалось загрузить preset охлаждения:',
    poll: 'Не удалось обновить состояние охлаждения:', runtime: 'Ошибка управления охлаждением:' },
};
async function setup(page: Page, language: Language, operation: Operation) {
  await page.addInitScript(({ language, operation }) => {
    localStorage.setItem('lconnect.interface-language', language);
    const f: Fixture = { language, revision: 0, calls: [], failed: false, policy: null, percent: 80 };
    const channel = () => ({ requested: f.policy, applied_percent: f.percent, motherboard: false,
      fallback_active: false, last_error: operation === 'runtime' ? 'Настройки' : null });
    Object.assign(window, { coolingMessages: f, __TAURI_INTERNALS__: {
      invoke: async (command: string, args: Record<string, unknown>) => {
        if (command === 'desktop_set_language') return null;
        if (command === 'cooling_preset') {
          f.calls.push(command);
          if (!f.failed) { f.failed = true; throw new Error('Настройки'); }
          return { mode: 'curve', sensor_id: 'cpu', hysteresis_c: 2, fallback_percent: 100,
            points: [{ celsius: 30, percent: 40 }, { celsius: 70, percent: 100 }] };
        }
        const request = args as { method: string; params: { policy: unknown } };
        f.calls.push(request.method);
        switch (request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
          case 'get_display_settings': return { version: 1, revision: f.revision, interface_language: f.language, temperature_unit: 'celsius' };
          case 'list_devices': return [{ id: 'lcd', display_name: 'My pump', family: 'galahad2-lcdv2' }];
          case 'list_sensors': return [{ id: 'cpu', label: 'Настройки', unit: 'celsius', value: 45, status: 'valid', age_ms: 0 }];
          case 'get_lcd_state': return { source: 'Idle', requested: {}, last_error: null };
          case 'get_cooling_state':
            if (operation === 'poll' && !f.failed) throw new Error('Настройки');
            return { pump: channel(), fan: channel() };
          case 'set_cooling_policy':
            f.policy = structuredClone(request.params.policy); f.percent = 61;
            if (operation === 'apply' && !f.failed) { f.failed = true; throw new Error('Настройки'); }
            return { pump: channel(), fan: channel() };
          case 'list_profiles': return { version: 1, revision: 0, profiles: [] };
          default: return null;
        }
      },
    } });
  }, { language, operation });
  await page.goto('/?view=display');
  await page.getByRole('button', { name: names[language].cooling, exact: true }).click();
}
async function language(page: Page, value: Language) {
  await page.evaluate(value => { window.coolingMessages.language = value; window.coolingMessages.revision++; }, value);
  await expect(page.locator('html')).toHaveAttribute('lang', value === 'en-US' ? 'en' : 'ru');
}
for (const initial of ['en-US', 'ru-RU'] as const) {
  for (const operation of ['apply', 'preset', 'poll', 'runtime'] as const) {
    test(`${operation} retains raw cooling diagnostic and reacts to language (${initial})`, async ({ page }) => {
      await setup(page, initial, operation);
      const pwm = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
      await pwm.fill('61');
      if (operation === 'apply') await page.getByRole('button', { name: names[initial].apply, exact: true }).click();
      if (operation === 'preset') {
        await page.getByRole('combobox', { name: names[initial].source, exact: true }).selectOption('cpu');
        await page.getByRole('button', { name: 'quiet', exact: true }).click();
      }
      const alert = page.getByRole('alert').filter({ hasText: contexts[initial][operation] });
      await expect(alert).toContainText('Настройки');
      const other = initial === 'en-US' ? 'ru-RU' : 'en-US';
      await language(page, other);
      await expect(page.getByRole('alert').filter({ hasText: contexts[other][operation] })).toContainText('Настройки');
      await expect(pwm).toHaveValue('61');
      const writes = () => page.evaluate(() => window.coolingMessages.calls.filter(c => c === 'set_cooling_policy'));
      expect(await writes()).toHaveLength(operation === 'apply' ? 1 : 0);
      if (operation === 'apply') {
        expect(await page.evaluate(() => window.coolingMessages.policy)).toEqual({ mode: 'fixed', percent: 61 });
        await page.getByRole('button', { name: names[other].apply, exact: true }).click();
        await expect(page.getByRole('alert')).toHaveCount(0);
        expect(await writes()).toHaveLength(2);
      } else if (operation === 'preset') {
        await page.getByRole('button', { name: 'quiet', exact: true }).click();
        await expect(page.getByRole('alert')).toHaveCount(0);
        await expect(page.getByRole('combobox', { name: names[other].mode, exact: true })).toHaveValue('curve');
        expect(await writes()).toHaveLength(0);
      } else if (operation === 'poll') {
        await page.evaluate(() => { window.coolingMessages.failed = true; });
        await expect(page.getByRole('alert')).toHaveCount(0);
        await expect(pwm).toHaveValue('61');
      }
    });
  }
  test(`local cooling ranges keep parameters and language without a write (${initial})`, async ({ page }) => {
    await setup(page, initial, 'validation');
    await page.getByRole('spinbutton', { name: 'PWM, %', exact: true }).fill('19');
    await page.getByRole('button', { name: names[initial].apply, exact: true }).click();
    await expect(page.getByRole('alert')).toContainText(initial === 'en-US' ? 'Enter an integer from 20 to 100' : 'Введите целое значение 20–100');
    const other = initial === 'en-US' ? 'ru-RU' : 'en-US';
    await language(page, other);
    await expect(page.getByRole('alert')).toContainText(other === 'en-US' ? 'Enter an integer from 20 to 100' : 'Введите целое значение 20–100');
    await page.getByRole('combobox', { name: names[other].mode, exact: true }).selectOption('curve');
    await page.getByRole('combobox', { name: names[other].source, exact: true }).selectOption('cpu');
    await page.getByRole('spinbutton', { name: 'Fallback PWM, %', exact: true }).fill('19');
    await page.getByRole('button', { name: names[other].apply, exact: true }).click();
    await expect(page.getByRole('alert')).toContainText(other === 'en-US' ? 'Fallback PWM must be an integer from 20 to 100%' : 'Fallback PWM должен быть целым от 20 до 100%');
    expect(await page.evaluate(() => window.coolingMessages.calls.filter(c => c === 'set_cooling_policy'))).toEqual([]);
    await page.getByRole('button',{name:other === 'en-US' ? 'Saved profiles' : 'Сохранённые профили',exact:true}).click();
    await page.getByRole('button', { name: other === 'en-US' ? 'Refresh profile list' : 'Обновить список профилей', exact: true }).click();
    await page.getByLabel(other === 'en-US' ? 'Profile name' : 'Имя профиля', { exact: true }).fill('Invalid curve');
    await page.getByRole('button', { name: other === 'en-US' ? 'Save draft to slot' : 'Сохранить черновик в слот', exact: true }).click();
    await expect(page.getByRole('dialog',{name:/^(Saved profiles|Сохранённые профили)$/,exact:true}).locator('[role="alert"]')).toContainText(other === 'en-US' ? 'Fallback PWM must be an integer from 20 to 100%' : 'Fallback PWM должен быть целым от 20 до 100%');
    await language(page, initial);
    await expect(page.getByRole('dialog',{name:/^(Saved profiles|Сохранённые профили)$/,exact:true}).locator('[role="alert"]')).toContainText(initial === 'en-US' ? 'Fallback PWM must be an integer from 20 to 100%' : 'Fallback PWM должен быть целым от 20 до 100%');
    expect(await page.evaluate(() => window.coolingMessages.calls.filter(c => c === 'edit_profile'))).toEqual([]);
  });
}
