import { test, expect, type Page } from '@playwright/test';
interface Fixture {
  holdNext: boolean;
  release: null | (() => void);
  missing: boolean;
  legacy: boolean;
  failNext: boolean;
  writes: number;
  reads: number;
}
async function setup(page: Page) {
  await page.addInitScript(() => {
    const f = {
      holdNext: false,
      release: null as null | (() => void),
      missing: false,
      legacy: false,
      failNext: false,
      writes: 0,
      reads: 0,
    };
    const modes = ['static', 'runway', 'meteor'];
    Object.assign(window, {
      lightingLoad: f,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string; params: { kind: string } }) => {
          if (_ === 'desktop_set_language') return null;
          switch (req.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return [
                { id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' },
                { id: 'B', display_name: 'LCD B', family: 'galahad2-lcdv2' },
              ];
            case 'list_sensors':
              return [];
            case 'get_lcd_state':
              return { source: 'Idle', requested: {} };
            case 'get_lighting_info': {
              f.reads++;
              if (f.failNext) {
                f.failNext = false;
                throw new Error('load rejected');
              }
              const result = {
                modes,
                base: f.missing
                  ? null
                  : {
                      mode: 'meteor',
                      color: { r: 1, g: 2, b: 3 },
                      additional_colors: f.legacy
                        ? undefined
                        : [
                            { r: 4, g: 5, b: 6 },
                            { r: 7, g: 8, b: 9 },
                            { r: 10, g: 11, b: 12 },
                          ],
                      brightness_percent: 77,
                      speed_percent: 25,
                      direction: 'reverse',
                      scope: req.params.kind === 'pump' ? 'inner' : null,
                      disabled: true,
                    },
              };
              if (f.holdNext) {
                f.holdNext = false;
                await new Promise<void>((resolve) => (f.release = resolve));
              }
              return result;
            }
            case 'get_thermal_state':
              return {
                requested: { cpu: null, gpu: null },
                degraded_sources: [],
                fan: { last_error: null },
                pump: { last_error: null },
                grace_remaining_ms: 0,
              };
            case 'set_lighting':
              f.writes++;
              return null;
            default:
              return null;
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Эффект', exact: true })).toHaveValue('static');
}
const load = (page: Page) =>
  page.getByRole('button', { name: 'Загрузить настройки подсветки', exact: true });
test('explicit load restores full mode settings without applying them', async ({ page }) => {
  await setup(page);
  await page.getByLabel('Цвет RGB', { exact: true }).fill('#ffffff');
  await load(page).click();
  await expect(page.getByRole('combobox', { name: 'Эффект', exact: true })).toHaveValue('meteor');
  for (const [index, color] of ['#010203', '#040506', '#070809', '#0a0b0c'].entries())
    await expect(
      page.getByLabel(index === 0 ? 'Цвет RGB' : `Цвет RGB ${index + 1}`, { exact: true }),
    ).toHaveValue(color);
  await expect(page.getByRole('slider', { name: /Яркость RGB:/ })).toHaveValue('77');
  await expect(page.getByRole('slider', { name: /Скорость:/ })).toHaveValue('25');
  await expect(page.getByRole('combobox', { name: 'Направление', exact: true })).toHaveValue(
    'reverse',
  );
  await expect(page.getByRole('combobox', { name: 'Контур', exact: true })).toHaveValue('inner');
  await expect(
    page.getByRole('checkbox', { name: 'Выключить подсветку', exact: true }),
  ).toBeChecked();
  expect(
    await page.evaluate(() => (window as unknown as { lightingLoad: Fixture }).lightingLoad.writes),
  ).toBe(0);
});
test('absent mode base preserves manual draft', async ({ page }) => {
  await setup(page);
  await page.getByLabel('Цвет RGB', { exact: true }).fill('#ffffff');
  await page.evaluate(
    () => ((window as unknown as { lightingLoad: Fixture }).lightingLoad.missing = true),
  );
  await load(page).click();
  await expect(
    page.getByText('Настройка режима для загрузки отсутствует. Черновик сохранён.', {
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.getByLabel('Цвет RGB', { exact: true })).toHaveValue('#ffffff');
});
for (const change of ['edit', 'zone', 'device-roundtrip'] as const)
  test(`late base load cannot replace draft after ${change}`, async ({ page }) => {
    await setup(page);
    await page.evaluate(
      () => ((window as unknown as { lightingLoad: Fixture }).lightingLoad.holdNext = true),
    );
    await load(page).click();
    await expect
      .poll(() =>
        page.evaluate(
          () => !!(window as unknown as { lightingLoad: Fixture }).lightingLoad.release,
        ),
      )
      .toBe(true);
    if (change === 'edit') await page.getByLabel('Цвет RGB', { exact: true }).fill('#ffffff');
    if (change === 'zone')
      await page.getByRole('combobox', { name: 'Зона', exact: true }).selectOption('fan');
    if (change === 'device-roundtrip') {
      await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('B');
      await expect
        .poll(() =>
          page.evaluate(() => (window as unknown as { lightingLoad: Fixture }).lightingLoad.reads),
        )
        .toBeGreaterThan(2);
      await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('A');
    }
    await page.evaluate(() =>
      (window as unknown as { lightingLoad: Fixture }).lightingLoad.release!(),
    );
    await expect(load(page)).toBeEnabled();
    await expect(page.getByRole('combobox', { name: 'Эффект', exact: true })).toHaveValue('static');
    await expect(page.getByLabel('Цвет RGB', { exact: true })).toHaveValue(
      change === 'edit' ? '#ffffff' : '#7cdcca',
    );
    expect(
      await page.evaluate(
        () => (window as unknown as { lightingLoad: Fixture }).lightingLoad.writes,
      ),
    ).toBe(0);
  });

test('legacy single-color mode loads remaining firmware slots as black', async ({ page }) => {
  await setup(page);
  await page.evaluate(
    () => ((window as unknown as { lightingLoad: Fixture }).lightingLoad.legacy = true),
  );
  await load(page).click();
  await expect(page.getByLabel('Цвет RGB', { exact: true })).toHaveValue('#010203');
  for (const index of [2, 3, 4])
    await expect(page.getByLabel(`Цвет RGB ${index}`, { exact: true })).toHaveValue('#000000');
});
test('failed load retains the draft and can be explicitly retried', async ({ page }) => {
  await setup(page);
  await page.getByLabel('Цвет RGB', { exact: true }).fill('#ffffff');
  await page.evaluate(
    () => ((window as unknown as { lightingLoad: Fixture }).lightingLoad.failNext = true),
  );
  await load(page).click();
  await expect(page.getByRole('alert').filter({ hasText: 'load rejected' })).toBeVisible();
  await expect(page.getByLabel('Цвет RGB', { exact: true })).toHaveValue('#ffffff');
  await load(page).click();
  await expect(page.getByLabel('Цвет RGB 4', { exact: true })).toHaveValue('#0a0b0c');
  expect(
    await page.evaluate(() => (window as unknown as { lightingLoad: Fixture }).lightingLoad.writes),
  ).toBe(0);
});
