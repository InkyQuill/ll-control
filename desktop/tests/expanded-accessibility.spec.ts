import { loadControls, openEditorSection, selectLayer } from './editor-controls';
import { test, expect } from '@playwright/test';

import { inspect } from './accessibility-helpers';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const style = {
      x: 20,
      y: 20,
      size: 24,
      color: [255, 255, 255],
      align: 'left',
      max_width: 358,
      line_height: 40,
      line_box: true,
      shadow: { x: 2, y: 2, blur: 2, color: [0, 0, 0], opacity: 60 },
    };
    const sensor = {
      sensor_id: 'cpu',
      label: 'CPU',
      unit: 'celsius',
      temperature_unit: 'celsius',
      decimals: 1,
    };
    const theme = {
      version: 1,
      name: 'Expanded controls',
      background: [0, 0, 0],
      layers: [
        { kind: 'text', text: 'Example', style },
        { kind: 'clock', twenty_four_hour: true, show_seconds: true, style },
        { kind: 'sensor', ...sensor, style, parts: { label: style, unit: style } },
        {
          kind: 'sensor_cycle',
          interval_seconds: 3,
          sensors: [sensor, { ...sensor, label: 'CPU second' }],
          style,
          parts: { label: style, unit: style },
        },
        {
          kind: 'image',
          asset: 'fixture-image',
          x: 0,
          y: 0,
          width: 100,
          height: 100,
          fit: 'contain',
          opacity: 255,
          watermark_position: { horizontal: 50, vertical: 50 },
          shadow: style.shadow,
        },
      ],
    };
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string; params?: { delete?: boolean } }) => {
          if (_ === 'desktop_set_language') return null;
          switch (req.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return [{ id: 'A', display_name: 'LCD', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
              return [
                { id: 'cpu', label: 'CPU', unit: 'celsius', value: 42, status: 'valid', age_ms: 0 },
              ];
            case 'get_lcd_state':
              return { source: 'Theme', requested: {}, last_error: null };
            case 'get_lcd_theme':
              return theme;
            case 'get_cooling_state': {
              const channel = {
                requested: null,
                applied_percent: 80,
                motherboard: false,
                fallback_active: false,
                last_error: null,
              };
              return { pump: channel, fan: channel };
            }
            case 'list_profiles':
              return {
                version: 1,
                revision: 1,
                profiles: [{ slot: 0, name: 'Profile', content: { kind: 'screen', value: theme } }],
              };
            case 'cleanup_theme_assets':
            case 'cleanup_theme_fonts':
              if (req.params?.delete) throw new Error('Unexpected deletion');
              return {
                candidates: ['orphan'],
                deleted: [],
                retained_referenced: 1,
                retained_session: 1,
                retained_draft: 1,
                skipped: 0,
                failures: [{ key: 'bad-file', error: 'Cannot inspect asset' }],
              };
            default:
              throw new Error('Unexpected mutation ' + req.method);
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await loadControls(page);
  await page.getByRole('button', { name: 'Загрузить активную тему' }).click();
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Expanded controls');
});

test('expanded layer, profile, font and cleanup controls have names and readable text', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Библиотека', exact: true }).click();

  for (const name of [
    'Пользовательские шрифты',
    'Сохранённые профили',
    'Очистка неиспользуемых шрифтов',
    'Очистка неиспользуемых изображений',
  ]) {
    await openEditorSection(page, name);
    if (name === 'Пользовательские шрифты')
      await page.getByRole('button', { name: 'Лицензии встроенных шрифтов', exact: true }).click();
    if (name === 'Сохранённые профили')
      await page.getByRole('button', { name: 'Обновить список профилей' }).click();
    if (name.startsWith('Очистка')) {
      await page
        .getByRole('button', {
          name: name.endsWith('шрифтов')
            ? 'Проверить неиспользуемые шрифты'
            : 'Проверить неиспользуемые изображения',
        })
        .click();
      await expect(page.getByRole('alert')).toHaveCount(1);
    }
    await inspect(page);
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
  }
});

test('expanded editor reflows at 320 pixels with all five layer types', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth))
    .toBeLessThanOrEqual(320);
  for (let index = 0; index < 5; index++) {
    await selectLayer(page, index);
    await inspect(page);
  }
});

test('curve point deletion keeps keyboard focus on an editable remaining point', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Охлаждение', exact: true }).click();
  await page.getByRole('combobox', { name: 'Режим', exact: true }).selectOption('curve');
  await page.getByRole('button', { name: 'Добавить точку', exact: true }).click();
  const remove = page.getByRole('button', { name: 'Удалить точку 3', exact: true });
  await remove.focus();
  await page.keyboard.press('Enter');
  const remaining = page.getByRole('spinbutton', { name: 'Точка 2: °C', exact: true });
  await expect(remaining).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('spinbutton', { name: 'PWM %', exact: true }).nth(1)).toBeFocused();
  await page.getByRole('button', { name: 'Добавить точку', exact: true }).click();
  await page.getByRole('button', { name: 'Удалить точку 1', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('spinbutton', { name: 'Точка 1: °C', exact: true })).toBeFocused();
});
