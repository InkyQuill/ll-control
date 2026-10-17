import { test, expect } from '@playwright/test';

test('accepted Apply, disk failure and retry have distinct visible status', async ({ page }) => {
  await page.addInitScript(() => {
    const fixture = {
      writes: 0, polls: 0,
      save: { pending: false, saved: true, last_error: null as string | null },
      config: { brightness_percent: 65, rotation_degrees: 0, mode: 'LocalUI' },
    };
    Object.assign(window, {
      persistenceFixture: fixture,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string; params: { patch: { brightness_percent?: number } } }) => {
        if (_ === 'desktop_set_language') return null;
          const state = () => ({ source: 'Idle', last_error: null, requested: fixture.config, applied: fixture.config, persistence: fixture.save });
          switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
            case 'list_devices': return [{ id: 'test', display_name: 'LCDV2', family:'galahad2-lcdv2' }];
            case 'list_sensors': return [];
            case 'get_lcd_state': fixture.polls++; return structuredClone(state());
            case 'patch_lcd_config':
              fixture.writes++;
              Object.assign(fixture.config, req.params.patch);
              // saved records a previous durable write, not this new pending one.
              fixture.save = { pending: true, saved: true, last_error: null };
              return state();
            default: throw new Error(req.method);
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  const live = page.locator('#persistence-status');
  await expect(live).toHaveAttribute('aria-live', 'polite');
  await expect(live).toHaveAttribute('aria-atomic', 'true');
  await page.evaluate(() => {
    const node = document.querySelector('#persistence-status')!;
    const observed = { node, mutations: 0 };
    new MutationObserver(records => { observed.mutations += records.length; }).observe(node, { childList: true, subtree: true, characterData: true });
    Object.assign(window, { livePersistence: observed });
  });
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByRole('slider').fill('71');
  await page.getByRole('button', { name: 'Применить яркость' }).click();
  await expect(page.getByText('Настройки ожидают записи на диск.')).toBeVisible();
  await expect(page.getByText('Последняя запись настроек завершена.')).toHaveCount(0);
  await page.evaluate(() => {
    const f = (window as unknown as { persistenceFixture: { save: { last_error: string | null } } }).persistenceFixture;
    f.save.last_error = 'fixture disk unavailable';
  });
  await expect(page.getByRole('alert')).toContainText('Не удалось сохранить настройки: fixture disk unavailable');
  await expect(page.getByRole('alert')).toContainText('Daemon повторит запись.');
  await expect(live).toBeEmpty();
  await expect(live.getByRole('alert')).toHaveCount(0);
  await expect(page.getByText('Последняя запись настроек завершена.')).toHaveCount(0);
  await expect(page.getByRole('slider')).toHaveValue('71');
  await page.evaluate(() => {
    const f = (window as unknown as { persistenceFixture: { save: { pending: boolean; saved: boolean; last_error: string | null } } }).persistenceFixture;
    f.save = { pending: false, saved: true, last_error: null };
  });
  await expect(page.getByText('Последняя запись настроек завершена.')).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as { persistenceFixture: { writes: number } }).persistenceFixture.writes)).toBe(1);
  const start = await page.evaluate(() => {
    const w = window as unknown as { persistenceFixture: { polls: number }; livePersistence: { node: Element; mutations: number } };
    if (w.livePersistence.node !== document.querySelector('#persistence-status')) throw new Error('Live region was replaced');
    return { polls: w.persistenceFixture.polls, mutations: w.livePersistence.mutations };
  });
  expect(start.mutations).toBeGreaterThan(0);
  await expect.poll(() => page.evaluate(() => (window as unknown as { persistenceFixture: { polls: number } }).persistenceFixture.polls)).toBeGreaterThanOrEqual(start.polls + 2);
  expect(await page.evaluate(() => (window as unknown as { livePersistence: { mutations: number } }).livePersistence.mutations)).toBe(start.mutations);

});

