import { openEditorSection } from './editor-controls';
import { test, expect } from '@playwright/test';
import { inspect } from './accessibility-helpers';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const fixture = {
      devices: [
        {
          id: 'lcd:studio',
          family: 'galahad2-lcdv2',
          display_name: 'Galahad II LCDV2',
          vid: 0x0416,
          pid: 0x7395,
          serial: 'LCD-IDENTITY-TEST',
        },
        { id: 'future:studio', family: 'future-controller', display_name: 'Future controller' },
      ],
      calls: [] as { method: string; params?: Record<string, unknown> }[],
    };
    Object.assign(window, {
      studioFixture: fixture,
      __TAURI_INTERNALS__: {
        invoke: async (
          _: string,
          request: { method: string; params?: Record<string, unknown> },
        ) => {
          if (_ === 'desktop_set_language') return null;
          fixture.calls.push(request);
          switch (request.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return fixture.devices;
            case 'list_sensors':
              return [];
            case 'get_lcd_state':
              return {
                source: 'Idle',
                requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 },
                last_error: null,
              };
            case 'get_screen_collections':
              return {
                collections: null,
                persistence: { pending: false, saved: false, last_error: null },
              };
            case 'list_profiles':
              return { version: 1, revision: 0, profiles: [] };
            default:
              throw new Error(`Unavailable fixture method: ${request.method}`);
          }
        },
      },
    });
  });
});

test('overview reflects discovered devices, unknown families never inherit LCD calls or controls', async ({
  page,
}) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Ваши устройства', exact: true })).toBeVisible();
  const cards = page.locator('.device-card');
  await expect(cards).toHaveCount(2);
  await expect(cards.locator('[data-status="experimental"]')).toHaveCount(2);
  await cards
    .first()
    .getByRole('button', { name: 'Ограничения экспериментальной функции', exact: true })
    .click();
  await expect(
    page.getByRole('dialog', { name: 'Ограничения экспериментальной функции', exact: true }),
  ).toContainText('Используйте элементы управления на свой риск.');
  await page.keyboard.press('Escape');
  await expect(cards.first()).toContainText('0416:7395');
  await expect(cards.first()).toContainText('LCD-IDENTITY-TEST');
  await expect(cards.first()).toContainText(
    'Отдельные контроллеры UNI FAN TL пока не поддерживаются.',
  );
  await expect(
    cards.filter({ hasText: 'Galahad II' }).locator('[data-device-screen="square"]'),
  ).toHaveCount(1);
  await page.screenshot({ path: '../.impeccable/review/production-overview.png', fullPage: true });
  await expect(
    cards.filter({ hasText: 'Future controller' }).locator('.device-actions').getByRole('button'),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Future controller', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Экран помпы', exact: true })).toHaveCount(0);
  await page.waitForTimeout(1600); // Exercise the next real refresh after selecting the unsupported family.
  const reads = await page.evaluate(() =>
    (
      window as unknown as {
        studioFixture: { calls: { method: string; params?: { dev: string } }[] };
      }
    ).studioFixture.calls.filter(
      (c) => c.method === 'get_lcd_state' && c.params?.dev === 'future:studio',
    ),
  );
  expect(reads).toEqual([]);
});

test('composition survives library navigation and overview; disconnected selection stays explicit', async ({
  page,
}) => {
  await page.goto('/');
  await page
    .locator('.device-card')
    .filter({ hasText: 'Galahad II' })
    .getByRole('button', { name: 'Экран помпы', exact: true })
    .click();
  await expect(page.locator('.lcd')).toHaveAttribute('data-lcd-shape', 'square');
  await expect(page.locator('.lcd')).toHaveCSS('border-top-left-radius', '0px');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: '../.impeccable/review/production-lcd.png', fullPage: true });
  await page.getByRole('button', { name: 'Готовые темы', exact: true }).click();
  await expect(page.getByLabel('Раскладка', { exact: true })).toBeVisible();
  await page.screenshot({ path: '../.impeccable/review/production-presets.png', fullPage: true });
  await page.getByRole('button', { name: 'Композиция', exact: true }).click();
  await page.getByLabel('Название', { exact: true }).fill('Мой несохранённый экран');
  await page.getByRole('button', { name: 'Библиотека', exact: true }).click();
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Мой несохранённый экран');
  await page.getByRole('button', { name: 'Сохранённые профили', exact: true }).click();
  await page.getByRole('button', { name: 'Обновить список профилей', exact: true }).click();
  await page.getByLabel('Имя профиля').fill('Сохранить позже');
  await page.getByRole('button', { name: 'Композиция', exact: true }).click();
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Мой несохранённый экран');
  await page.getByRole('button', { name: 'Библиотека', exact: true }).click();
  await expect(page.getByLabel('Имя профиля')).toHaveValue('Сохранить позже');
  await page.getByRole('button', { name: 'Обзор', exact: true }).click();
  await page.locator('nav').getByRole('button', { name: 'Экран помпы', exact: true }).click();
  await page.getByRole('button', { name: 'Композиция', exact: true }).click();
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Мой несохранённый экран');
  await page.evaluate(() => {
    (window as unknown as { studioFixture: { devices: unknown[] } }).studioFixture.devices = [];
  });
  await expect(
    page.getByText(
      'Выбранное устройство недоступно. Черновик сохранён; выберите устройство явно или дождитесь подключения.',
    ),
  ).toBeVisible();
  await expect(page.getByRole('button', { name: 'Применить к LCD', exact: true })).toBeDisabled();
  await expect(page.locator('.lcd')).toHaveAttribute('data-lcd-shape', 'square');
  await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Мой несохранённый экран');
});

test('overview and editor retain contrast and reflow in compact desktop windows', async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 900 });
  await page.goto('/');
  await expect(page.locator('.device-card')).toHaveCount(2);
  await inspect(page);
  await page
    .locator('.device-card')
    .first()
    .getByRole('button', { name: 'Экран помпы', exact: true })
    .click();
  await inspect(page);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '../.impeccable/review/production-compact.png', fullPage: true });
});
