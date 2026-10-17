import {addControls} from './editor-controls';
import { test, expect, type Page, type Locator } from '@playwright/test';
import { inspect } from './accessibility-helpers';

// Reach controls through the tab order, including hidden-page exclusion.
async function tabTo(page: Page, target: Locator) {
  await expect(target).toBeVisible();
  await expect(target).toBeEnabled();
  for (let n = 0; n < 150; n++) {
    if (await target.evaluate(el => el === document.activeElement)) {
      await expect(target).toBeFocused();
      expect(await target.evaluate(el => getComputedStyle(el).outlineStyle)).not.toBe('none');
      return;
    }
    await page.keyboard.press('Tab');
  }
  throw new Error('Control unreachable through keyboard tab order');
}
async function activate(page: Page, name: string) {
  await tabTo(page, page.getByRole('button', { name, exact: true }));
  await page.keyboard.press('Enter');
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const writes: { method: string; params: Record<string, unknown> }[] = [];
    const cpu = { temperature: null, clock: null, load: null, power: null, fan: null, voltage: null };
    let config = { version: 1, revision: 0, cpu, gpu: null };
    const reading = { id: 'cpu:test', label: 'CPU package', unit: 'celsius', value: 42, status: 'valid', age_ms: 0 };
    const channel = { requested: null, applied_percent: 80, motherboard: false, fallback_active: false, last_error: null };
    Object.assign(window, { keyboardWrites: writes, __TAURI_INTERNALS__: { invoke: async (_: string, request: { method: string; params: Record<string, unknown> }) => {
        if (_ === 'desktop_set_language') return null;
      if (request.method.startsWith('set_')) writes.push(structuredClone(request));
      switch (request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
        case 'list_devices': return [{ id: 'lcd:test', display_name: 'LCDV2', family:'galahad2-lcdv2' }];
        case 'list_sensors': return [reading];
        case 'get_lcd_state': return { source: 'Idle', last_error: null, requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 } };
        case 'get_cooling_state':
        case 'set_cooling_policy': return { pump: channel, fan: channel };
        case 'get_lighting_info': return { modes: ['static_color', 'breathing'] };
        case 'set_lighting': return {};
        case 'get_thermal_state': return { requested: { cpu: null, gpu: null }, active_source: null, degraded_sources: [], grace_remaining_ms: 0, blocked_reason: null, pump: { last_error: null }, fan: { last_error: null } };
        case 'get_dynamic_rgb': return { requested: null, base_mode: null, color_count: null, applied: null, degraded: false, last_error: null };
        case 'get_sensor_defaults': return { config, recommended: config, candidates: [{ role: 'cpu_temperature', gpu_id: null, reading }], values: [] };
        case 'set_sensor_defaults': config = { ...(request.params.config as typeof config), revision: config.revision + 1 }; return config;
        default: throw new Error('Unexpected keyboard fixture RPC: ' + request.method);
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
});
const writes = (page: Page) => page.evaluate(() => (window as unknown as { keyboardWrites: { method: string; params: Record<string, unknown> }[] }).keyboardWrites);

test('keyboard selects fan channel and applies fixed PWM only on explicit action', async ({ page }) => {
  await activate(page, 'Охлаждение');
  await tabTo(page, page.getByRole('combobox', { name: 'Канал', exact: true }));
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('combobox', { name: 'Канал', exact: true })).toHaveValue('fan');
  const pwm = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
  await tabTo(page, pwm);
  await page.keyboard.press('ControlOrMeta+A');
  await page.keyboard.type('55');
  expect(await writes(page)).toEqual([]);
  await activate(page, 'Применить охлаждение');
  await expect(page.getByRole('status')).toContainText('Политика принята');
  expect(await writes(page)).toEqual([{ method: 'set_cooling_policy', params: { dev: 'lcd:test', target: 'fan', policy: { mode: 'fixed', percent: 55 } } }]);
});

test('keyboard adjusts RGB slider and submits without accidental writes while editing', async ({ page }) => {
  await activate(page, 'Подсветка');
  const brightness = page.getByRole('slider', { name: /^Яркость RGB:/ });
  await tabTo(page, brightness);
  await page.keyboard.press('End');
  await page.keyboard.press('ArrowLeft');
  await expect(brightness).toHaveValue('99');
  expect(await writes(page)).toEqual([]);
  await activate(page, 'Применить подсветку');
  await expect(page.getByRole('status')).toContainText('Команда подсветки выполнена');
  expect(await writes(page)).toMatchObject([{ method: 'set_lighting', params: { kind: 'pump', cfg: { brightness_percent: 99 } } }]);
});

test('keyboard selects CPU source and preserves explicit Apply boundary', async ({ page }) => {
  await activate(page, 'Датчики');
  const source = page.getByRole('combobox', { name: 'CPU · Температура', exact: true });
  await tabTo(page, source);
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await expect(source).toHaveValue('cpu:test');
  expect(await writes(page)).toEqual([]);
  await activate(page, 'Применить источники');
  await expect(page.getByRole('status')).toContainText('Источники сохранены');
  expect(await writes(page)).toMatchObject([{ method: 'set_sensor_defaults', params: { config: { cpu: { temperature: 'cpu:test' } } } }]);
});

test('visible controls have accessible names on all five pages', async ({ page }) => {
  for (const name of ['Экран помпы', 'Датчики', 'Охлаждение', 'Подсветка', 'Настройки']) {
    await activate(page, name);
    await expect(page.locator('nav [aria-current="page"]')).toHaveCount(1);
    const controls = page.locator('main input:visible, main select:visible, main textarea:visible, main button:visible');
    for (const control of await controls.all()) await expect(control).toHaveAccessibleName(/\S/);
  }
  expect(await writes(page)).toEqual([]);
});

test('enabled text fields and selects have distinguishable boundaries on every page', async ({ page }) => {
  for (const name of ['Экран помпы', 'Датчики', 'Охлаждение', 'Подсветка', 'Настройки']) {
    await activate(page, name);
    const fields = page.locator('main input[type=text]:visible:enabled, main input[type=number]:visible:enabled, main input:not([type]):visible:enabled, main select:visible:enabled, main textarea:visible:enabled');
    for (const field of await fields.all()) {
      const ratios = await field.evaluate(el => {
        const luminance = (color: string) => {
          const components = color.match(/[\d.]+/g)!.slice(0, 3).map(Number).map(v => v / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4);
          return components[0] * .2126 + components[1] * .7152 + components[2] * .0722;
        };
        const contrast = (a: string, b: string) => {
          const x = luminance(a), y = luminance(b);
          return (Math.max(x, y) + .05) / (Math.min(x, y) + .05);
        };
        let parent = el.parentElement;
        while (parent && getComputedStyle(parent).backgroundColor === 'rgba(0, 0, 0, 0)') parent = parent.parentElement;
        if (!parent) throw new Error('No opaque parent background');
        const style = getComputedStyle(el);
        return { inside: contrast(style.borderTopColor, style.backgroundColor), outside: contrast(style.borderTopColor, getComputedStyle(parent).backgroundColor) };
      });
      expect(ratios.inside, `${name}: border against field fill`).toBeGreaterThanOrEqual(3);
      expect(ratios.outside, `${name}: border against parent background`).toBeGreaterThanOrEqual(3);
    }
  }
});

for (const width of [320, 640]) {
  test(`editors reflow at ${width} CSS pixels without horizontal scrolling`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    for (const name of ['Экран помпы', 'Датчики', 'Охлаждение', 'Подсветка', 'Настройки']) {
      await page.getByRole('button', { name, exact: true }).click();
      if (name === 'Экран помпы') {await addControls(page);await page.getByRole('button', { name: '+ Текст', exact: true }).click();}
      if (name === 'Охлаждение') await page.getByRole('combobox', { name: 'Режим', exact: true }).selectOption('curve');
      if (name === 'Подсветка') await page.getByRole('button', { name: 'Загрузить текущие оповещения', exact: true }).click();
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth), { message: `${name}: no horizontal page scrolling` }).toBeLessThanOrEqual(width);
      if (name === 'Экран помпы' || name === 'Охлаждение') await page.screenshot({ path: test.info().outputPath(`${name === 'Экран помпы' ? 'display' : 'cooling'}-${width}.png`), fullPage: true });
      const controls = page.locator('main button:visible, main input:visible, main select:visible, main textarea:visible');
      for (const control of await controls.all()) {
        const box = await control.boundingBox();
        expect(box, `${name}: visible control`).not.toBeNull();
        expect(box!.x, `${name}: control left edge`).toBeGreaterThanOrEqual(0);
        expect(box!.x + box!.width, `${name}: control right edge`).toBeLessThanOrEqual(width + 1);
      }
    }
    expect(await writes(page)).toEqual([]);
  });
}


test('text, success and validation/transport errors remain readable across all pages', async ({ page }) => {
  for (const name of ['Экран помпы', 'Датчики', 'Охлаждение', 'Подсветка', 'Настройки']) {
    await activate(page, name);
    await inspect(page);
  }
  // This fixture deliberately rejects patch_lcd_config to expose a real error UI.
  await activate(page, 'Применить яркость');
  await expect(page.getByRole('alert')).toContainText('Unexpected keyboard fixture RPC: patch_lcd_config');
  await inspect(page);

  await activate(page, 'Охлаждение');
  const pwm = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
  await page.getByRole('combobox', { name: 'Режим', exact: true }).selectOption('curve');
  await activate(page, 'Применить охлаждение');
  await expect(page.getByRole('alert')).toContainText('Выберите температурный датчик');
  await inspect(page);
  await page.getByRole('combobox', { name: 'Режим', exact: true }).selectOption('fixed');
  await pwm.fill('65');
  await activate(page, 'Применить охлаждение');
  await expect(page.getByRole('status')).toContainText('Политика принята');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await inspect(page);

  await activate(page, 'Подсветка');
  await activate(page, 'Применить подсветку');
  await expect(page.getByRole('status')).toContainText('Команда подсветки выполнена');
  await inspect(page);

  await activate(page, 'Датчики');
  await page.getByRole('combobox', { name: 'CPU · Температура', exact: true }).selectOption('cpu:test');
  await activate(page, 'Применить источники');
  await expect(page.getByRole('status')).toContainText('Источники сохранены');
  await inspect(page);

  await activate(page, 'Экран помпы');
  await activate(page, 'Применить к LCD');
  await expect(page.getByRole('alert')).toContainText('Unexpected keyboard fixture RPC: set_lcd_theme');
  await inspect(page);
});


test('cooling rejects invalid numeric drafts before RPC and recovers on valid Apply', async ({ page }) => {
  await activate(page, 'Охлаждение');
  const mode = page.getByRole('combobox', { name: 'Режим', exact: true });
  const apply = page.getByRole('button', { name: 'Применить охлаждение', exact: true });
  const pwm = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
  for (const value of ['', '19', '101', '65.5']) {
    await pwm.fill(value); await apply.click();
    await expect(pwm).toHaveAttribute('aria-invalid', 'true');
    await expect(page.getByRole('alert')).toBeVisible();
    expect(await writes(page)).toEqual([]);
  }
  await mode.selectOption('fixed_rpm');
  const rpm = page.getByRole('spinbutton', { name: 'Целевые RPM (reference mapping)', exact: true });
  for (const value of ['', '799', '3601', '2200.5']) {
    await rpm.fill(value); await apply.click();
    await expect(page.getByRole('alert')).toBeVisible();
    expect(await writes(page)).toEqual([]);
  }
  await mode.selectOption('curve');
  await page.getByRole('combobox', { name: 'Источник температуры', exact: true }).selectOption('cpu:test');
  const temp = page.getByRole('spinbutton', { name: 'Точка 1: °C', exact: true });
  const duty = page.getByRole('spinbutton', { name: 'PWM %', exact: true }).first();
  const hysteresis = page.getByRole('spinbutton', { name: 'Гистерезис, °C', exact: true });
  const fallback = page.getByRole('spinbutton', { name: 'Fallback PWM, %', exact: true });
  for (const [field, invalid, valid] of [[temp,'151','-20.5'], [duty,'40.5','40'], [hysteresis,'10.1','2.25'], [fallback,'99.5','100']] as const) {
    await field.fill(invalid); await apply.click();
    await expect(field).toHaveAttribute('aria-invalid', 'true');
    await expect(page.getByRole('alert')).toBeVisible();
    expect(await writes(page)).toEqual([]);
    await field.fill(valid);
  }
  await apply.click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await writes(page)).toMatchObject([{ params: { policy: { mode: 'curve', hysteresis_c: 2.25, points: [{ celsius: -20.5, percent: 40 }, { celsius: 70, percent: 100 }] } } }]);
  for (const nextMode of ['curve_rpm', 'fan_curve']) {
    if (nextMode === 'fan_curve') await page.getByRole('combobox', { name: 'Канал', exact: true }).selectOption('fan');
    await mode.selectOption(nextMode);
    const before = (await writes(page)).length;
    await temp.fill('30.5'); await apply.click();
    await expect(temp).toHaveAttribute('aria-invalid', 'true');
    await expect(page.getByRole('alert')).toBeVisible();
    expect(await writes(page)).toHaveLength(before);
    await temp.fill('30'); await apply.click();
    await expect(page.getByRole('alert')).toHaveCount(0);
    expect(await writes(page)).toHaveLength(before + 1);
  }

});
