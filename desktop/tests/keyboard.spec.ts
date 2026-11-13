import { test, expect } from '@playwright/test';
test('keyboard skips navigation, identifies current page and applies separate LCD fields', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const patches: unknown[] = [];
    Object.assign(window, {
      keyboardPatches: patches,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string; params: { patch: unknown } }) => {
          if (_ === 'desktop_set_language') return null;
          switch (req.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'get_display_settings':
              return { version: 1, revision: 0, temperature_unit: 'celsius' };
            case 'list_devices':
              return [{ id: 'test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
              return [];
            case 'get_lcd_state':
              return {
                source: 'Theme',
                last_error: null,
                requested: { brightness_percent: 65, rotation_degrees: 90, mode: 'Application' },
              };
            case 'patch_lcd_config':
              patches.push(req.params.patch);
              return {};
            default:
              throw new Error(req.method);
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Экран помпы', exact: true })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.keyboard.press('Tab');
  const skip = page.getByRole('link', { name: 'Перейти к содержимому' });
  await expect(skip).toBeFocused();
  await expect(skip).toBeInViewport();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('main')).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  const settings = page.getByRole('button', { name: 'Настройки', exact: true });
  for (
    let step = 0;
    step < 4 && !(await settings.evaluate((node) => node === document.activeElement));
    step++
  )
    await page.keyboard.press('Shift+Tab');
  await expect(settings).toBeFocused();
  expect(await settings.evaluate((el) => getComputedStyle(el).outlineWidth)).toBe('2px');
  await page.keyboard.press('Enter');
  await expect(settings).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('nav [aria-current="page"]')).toHaveCount(1);
  await expect(
    page.getByRole('combobox', { name: 'Единицы температуры', exact: true }),
  ).toBeEnabled();
  const device = page.getByRole('combobox', { name: 'Устройство', exact: true });
  for (
    let step = 0;
    step < 4 && !(await device.evaluate((node) => node === document.activeElement));
    step++
  )
    await page.keyboard.press('Tab');
  await expect(device).toBeFocused();
  const units = page.getByRole('combobox', { name: 'Единицы температуры', exact: true });
  // The collapsed group archive precedes display preferences in the tab order.
  for (
    let step = 0;
    step < 8 && !(await units.evaluate((node) => node === document.activeElement));
    step++
  )
    await page.keyboard.press('Tab');
  await expect(units).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('combobox', { name: 'Язык интерфейса', exact: true })).toBeFocused();
  const loadValues = page.getByRole('button', { name: 'Загрузить текущие значения' });
  // Reach the LCD group through the visible preference controls and collapsed
  // desktop settings summary. Never set focus directly or depend on a tab count.
  for (
    let step = 0;
    step < 24 && !(await loadValues.evaluate((node) => node === document.activeElement));
    step++
  )
    await page.keyboard.press('Tab');
  await expect(loadValues).toBeFocused();
  await page.keyboard.press('Space');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('slider')).toBeFocused();
  await page.keyboard.press('ArrowLeft');
  await expect(page.getByRole('slider')).toHaveValue('64');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Применить яркость' })).toBeFocused();
  await page.keyboard.press('Space');
  await expect(page.getByText('Настройка отправлена.', { exact: false })).toBeVisible();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('combobox', { name: 'Ориентация', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Применить поворот' })).toBeFocused();
  await page.keyboard.press('Space');
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { keyboardPatches: unknown[] }).keyboardPatches),
    )
    .toEqual([{ brightness_percent: 64 }, { rotation_degrees: 180 }]);
});
