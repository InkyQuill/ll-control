import { loadControls } from './editor-controls';
import { test, expect } from '@playwright/test';

interface ExternalFixture {
  changed: boolean;
  modeChanged: boolean;
  cleared: boolean;
  writes: string[];
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const fixture: ExternalFixture = {
      changed: false,
      modeChanged: false,
      cleared: false,
      writes: [],
    };
    Object.assign(window, {
      externalFixture: fixture,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string }) => {
          if (_ === 'desktop_set_language') return null;
          if (
            req.method.startsWith('set_') ||
            req.method === 'patch_lcd_config' ||
            req.method === 'clear_lcd'
          ) {
            fixture.writes.push(req.method);
            throw new Error('Unexpected mutation');
          }
          const channel = {
            requested: { mode: 'fixed', percent: fixture.changed ? 72 : 60 },
            applied_percent: fixture.changed ? 72 : 60,
            motherboard: false,
            fallback_active: false,
            last_error: null,
          };
          switch (req.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return [{ id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
              return [
                { id: 'cpu', label: 'CPU', unit: 'celsius', value: 42, status: 'valid', age_ms: 1 },
              ];
            case 'get_lcd_state':
              return {
                source: fixture.cleared ? 'Idle' : 'Theme',
                persistence: {
                  pending: fixture.changed,
                  saved: !fixture.changed,
                  last_error: null,
                },
                last_error: null,
                requested: {
                  mode: 'Application',
                  brightness_percent: fixture.changed ? 72 : 60,
                  rotation_degrees: fixture.changed ? 270 : 90,
                },
              };
            case 'get_lcd_theme':
              return fixture.cleared
                ? null
                : {
                    version: 1,
                    name: fixture.changed ? 'External replacement' : 'Original theme',
                    background: [0, 0, 0],
                    layers: [],
                  };
            case 'list_profiles':
              return { revision: 0, profiles: [] };
            case 'get_cooling_state':
              return { pump: channel, fan: channel };
            case 'get_lighting_info':
              return { modes: ['static', 'breathing'] };
            case 'get_thermal_state':
              return {
                requested: {
                  cpu: {
                    sensor_id: 'cpu',
                    threshold_c: fixture.changed ? 95 : 80,
                    color: { r: 255, g: 0, b: 0 },
                  },
                  gpu: null,
                },
                active_source: fixture.changed ? 'cpu' : null,
                degraded_sources: [],
                grace_remaining_ms: 0,
                blocked_reason: null,
                fan: { last_error: null },
                pump: { last_error: null },
              };
            case 'get_dynamic_rgb':
              return {
                base_mode: fixture.modeChanged ? 'breathing' : 'static',
                color_count: 1,
                speed_supported: fixture.modeChanged,
                requested: {
                  sensor_id: 'cpu',
                  unit: 'celsius',
                  low_value: fixture.changed ? 25 : 30,
                  high_value: fixture.changed ? 75 : 60,
                  low_colors: [{ r: 0, g: 0, b: 255 }],
                  high_colors: [{ r: 255, g: 0, b: 0 }],
                  low_speed_percent: null,
                  high_speed_percent: null,
                },
                applied: { value: fixture.changed ? 55 : 42, colors: [], speed_percent: null },
                degraded: false,
                suppressed_by_thermal: false,
                last_error: null,
              };
            default:
              throw new Error(req.method);
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
});

test('external cooling policy updates applied state without replacing PWM draft', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Охлаждение', exact: true }).click();
  await expect(page.getByText('Применённый PWM: 60%')).toBeVisible();
  const pwm = page.getByLabel('PWM, %', { exact: true });
  await pwm.fill('83');
  await page.evaluate(() => {
    (window as unknown as { externalFixture: ExternalFixture }).externalFixture.changed = true;
  });
  await expect(page.getByText('Применённый PWM: 72%')).toBeVisible();
  await expect(pwm).toHaveValue('83');
  await page.getByRole('button', { name: 'Загрузить текущую политику' }).click();
  await expect(pwm).toHaveValue('72');
});

test('external thermal rule updates status without replacing edited threshold', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  const load = page.getByRole('button', { name: 'Загрузить текущие оповещения' });
  await expect(load).toBeEnabled();
  await load.click();
  const threshold = page.getByLabel('Порог CPU, °C');
  await expect(threshold).toHaveValue('80');
  await threshold.fill('91');
  await page.evaluate(() => {
    (window as unknown as { externalFixture: ExternalFixture }).externalFixture.changed = true;
  });
  await expect(page.getByText('Активный источник: cpu', { exact: false })).toBeVisible();
  await expect(threshold).toHaveValue('91');
  await load.click();
  await expect(threshold).toHaveValue('95');
});

for (const modeChanged of [false, true]) {
  test(`external dynamic RGB ${modeChanged ? 'mode' : 'configuration'} preserves draft until explicit reload`, async ({
    page,
  }) => {
    await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
    await page.getByText('Подсветка помпы по датчику', { exact: true }).click();
    const load = page.getByRole('button', { name: 'Загрузить настройки режима по датчику' });
    await expect(load).toBeEnabled();
    await load.click();
    const low = page.getByLabel('Нижний порог (°C)');
    await low.fill('44');
    await page.evaluate((modeChanged) => {
      const fixture = (window as unknown as { externalFixture: ExternalFixture }).externalFixture;
      fixture.changed = true;
      fixture.modeChanged = modeChanged;
    }, modeChanged);
    await expect(page.getByText('Применённое значение: 55')).toBeVisible();
    await expect(low).toHaveValue('44');
    const apply = page.getByRole('button', { name: 'Применить режим по датчику' });
    if (modeChanged) {
      await expect(apply).toBeDisabled();
      await expect(
        page.getByText('Устройство или режим помпы изменились.', { exact: false }),
      ).toBeVisible();
    } else await expect(apply).toBeEnabled();
    await load.click();
    await expect(low).toHaveValue('25');
    await expect(page.getByLabel('Верхний порог (°C)')).toHaveValue('75');
    await expect(apply).toBeEnabled();
  });
}

test('external LCD settings update requested state while both edited fields survive', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  const load = page.getByRole('button', { name: 'Загрузить текущие значения' });
  await load.click();
  const brightness = page.getByRole('slider');
  const rotation = page.getByRole('combobox', { name: 'Ориентация', exact: true });
  await expect(brightness).toHaveValue('60');
  await expect(rotation).toHaveValue('90');
  await brightness.fill('83');
  await rotation.selectOption('180');
  await page.evaluate(() => {
    (window as unknown as { externalFixture: ExternalFixture }).externalFixture.changed = true;
  });
  await expect(
    page.getByText('Запрошенные настройки: 72% · 270°.', { exact: false }),
  ).toBeVisible();
  await expect(brightness).toHaveValue('83');
  await expect(rotation).toHaveValue('180');
  await load.click();
  await expect(brightness).toHaveValue('72');
  await expect(rotation).toHaveValue('270');
});

for (const cleared of [false, true]) {
  test(`external display ${cleared ? 'clear' : 'replacement'} preserves theme edits until explicit load`, async ({
    page,
  }) => {
    await loadControls(page);
    const load = page.getByRole('button', { name: 'Загрузить активную тему' });
    const name = page.getByLabel('Название', { exact: true });
    await load.click();
    await expect(name).toHaveValue('Original theme');
    await name.fill('Unsaved local design');
    await page.evaluate((cleared) => {
      const fixture = (window as unknown as { externalFixture: ExternalFixture }).externalFixture;
      fixture.changed = true;
      fixture.cleared = cleared;
    }, cleared);
    await expect(
      page.getByText('Настройки ожидают записи на диск.', { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText(`Источник: ${cleared ? 'Нет программного потока' : 'Тема LCD'}`, {
        exact: true,
      }),
    ).toBeVisible();
    await expect(name).toHaveValue('Unsaved local design');
    await load.click();
    if (cleared) {
      await expect(
        page.getByText('На устройстве нет активной темы', { exact: true }),
      ).toBeVisible();
      await expect(name).toHaveValue('Unsaved local design');
    } else await expect(name).toHaveValue('External replacement');
  });
}

test.afterEach(async ({ page }) => {
  expect(
    await page.evaluate(
      () => (window as unknown as { externalFixture: ExternalFixture }).externalFixture.writes,
    ),
  ).toEqual([]);
});
