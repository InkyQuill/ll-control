import { test, expect } from '@playwright/test';
interface Fixture {
  online: boolean;
  finish: (fail: boolean) => void;
}
for (const reconnect of [false, true])
  test(`dynamic RGB ignores pending mutation after ${reconnect ? 'reconnect' : 'A to B to A'} and keeps draft`, async ({
    page,
  }) => {
    await page.addInitScript(() => {
      const config = {
        sensor_id: 'cpu',
        unit: 'celsius',
        low_value: 30,
        high_value: 60,
        low_colors: [{ r: 0, g: 0, b: 255 }],
        high_colors: [{ r: 255, g: 0, b: 0 }],
        low_speed_percent: null,
        high_speed_percent: null,
      };
      const state = {
        base_mode: 'static',
        color_count: 1,
        speed_supported: false,
        requested: config,
        applied: null,
        degraded: false,
        suppressed_by_thermal: false,
        last_error: null,
      };
      const fixture: Fixture = { online: true, finish: () => {} };
      Object.assign(window, {
        dynamicFixture: fixture,
        __TAURI_INTERNALS__: {
          invoke: async (_: string, req: { method: string }) => {
            if (_ === 'desktop_set_language') return null;
            if (!fixture.online) throw new Error('offline');
            switch (req.method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'list_devices':
                return [
                  { id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' },
                  { id: 'B', display_name: 'LCD B', family: 'galahad2-lcdv2' },
                ];
              case 'list_sensors':
                return [
                  {
                    id: 'cpu',
                    label: 'CPU',
                    unit: 'celsius',
                    value: 42,
                    status: 'valid',
                    age_ms: 1,
                  },
                ];
              case 'get_lcd_state':
                return { source: 'Idle', last_error: null, requested: {} };
              case 'get_lighting_info':
                return { modes: ['static'] };
              case 'get_thermal_state':
                return {
                  requested: { cpu: null, gpu: null },
                  active_source: null,
                  degraded_sources: [],
                  grace_remaining_ms: 0,
                  blocked_reason: null,
                  fan: { last_error: null },
                  pump: { last_error: null },
                };
              case 'get_dynamic_rgb':
                return state;
              case 'set_dynamic_rgb':
                return new Promise((resolve, reject) => {
                  fixture.finish = (fail) =>
                    fail
                      ? reject(new Error('obsolete RGB error'))
                      : resolve({
                          ...state,
                          applied: { value: 99, colors: [], speed_percent: null },
                        });
                });
              default:
                throw new Error(req.method);
            }
          },
        },
      });
    });
    await page.goto('/?view=display');
    await expect(page.getByText('Daemon подключён')).toBeVisible();
    await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
    await page.getByText('Подсветка помпы по датчику', { exact: true }).click();
    const load = page.getByRole('button', { name: 'Загрузить настройки режима по датчику' });
    await expect(load).toBeEnabled();
    await load.click();
    await page.getByLabel('Нижний порог (°C)').fill('44');
    const apply = page.getByRole('button', { name: 'Применить режим по датчику' });
    await apply.click();
    await expect(apply).toBeDisabled();
    if (reconnect) {
      await page.evaluate(() => {
        (window as unknown as { dynamicFixture: Fixture }).dynamicFixture.online = false;
      });
      await expect(page.getByText('Нет связи с daemon')).toBeVisible();
      await page.evaluate(() => {
        (window as unknown as { dynamicFixture: Fixture }).dynamicFixture.online = true;
      });
      await expect(page.getByText('Daemon подключён')).toBeVisible();
    } else {
      await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('B');
      await expect(
        page.getByText('Устройство или режим помпы изменились.', { exact: false }),
      ).toBeVisible();
      await page.getByRole('combobox', { name: 'Устройство', exact: true }).selectOption('A');
    }
    await page.evaluate((fail) => {
      (window as unknown as { dynamicFixture: Fixture }).dynamicFixture.finish(fail);
    }, !reconnect);
    await expect(load).toBeEnabled();
    await expect(apply).toBeEnabled();
    await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('44');
    await expect(page.getByText('obsolete RGB error', { exact: false })).toHaveCount(0);
    await expect(page.getByText('Режим по датчику сохранён.', { exact: false })).toHaveCount(0);
    await expect(page.getByText('Применённое значение: 99')).toHaveCount(0);
  });
