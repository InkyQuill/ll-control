import { test, expect } from '@playwright/test';
test('voltage, clocks, network and storage stay typed from Sensors to LCD Apply', async ({
  page,
}) => {
  await page.addInitScript(() => {
    const calls: { method: string; params: Record<string, unknown> }[] = [];
    Object.assign(window, {
      electricalCalls: calls,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, request: { method: string; params: Record<string, unknown> }) => {
          if (_ === 'desktop_set_language') return null;
          calls.push(request);
          switch (request.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return [{ id: 'lcd:test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
              return [
                {
                  id: 'gpu:voltage',
                  label: 'GPU voltage',
                  unit: 'volts',
                  value: 0.975,
                  status: 'valid',
                  age_ms: 10,
                },
                {
                  id: 'gpu:clock',
                  label: 'GFX clock',
                  unit: 'hertz',
                  value: 1500000000,
                  status: 'valid',
                  age_ms: 10,
                },
                {
                  id: 'net:rx',
                  label: 'Network RX',
                  unit: 'bytes_per_second',
                  value: 1000,
                  status: 'valid',
                  age_ms: 10,
                },
                {
                  id: 'storage:used',
                  label: 'Root used',
                  unit: 'bytes',
                  value: 2147483648,
                  status: 'valid',
                  age_ms: 10,
                },
              ];
            case 'get_lcd_state':
              return { source: 'Idle' };
            case 'set_lcd_theme':
              return { source: 'Theme' };
            default:
              throw new Error(request.method);
          }
        },
      },
    });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button', { name: 'Датчики', exact: true }).click();
  await expect(page.getByText('0.97 V', { exact: true })).toBeVisible();
  await expect(page.getByText('1500.0 MHz', { exact: true })).toBeVisible();
  await expect(page.getByText('1000.0 B/s', { exact: true })).toBeVisible();
  await expect(page.getByText('2.0 GiB', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Экран помпы', exact: true }).click();
  await page.getByRole('button', { name: 'Готовые темы', exact: true }).click();
  await page.getByRole('button', { name: /^Quad/ }).click();
  await page.getByRole('checkbox', { name: 'GPU voltage · volts' }).check();
  await page.getByRole('checkbox', { name: 'GFX clock · hertz' }).check();
  await page.getByRole('checkbox', { name: 'Network RX · bytes_per_second' }).check();
  await page.getByRole('checkbox', { name: 'Root used · bytes' }).check();
  await page.getByRole('button', { name: 'Создать черновик из раскладки' }).click();
  await page.getByRole('button', { name: 'Применить к LCD' }).click();
  await expect(page.getByRole('status')).toContainText('Тема отправлена');
  const theme = await page.evaluate(
    () =>
      (
        window as unknown as {
          electricalCalls: {
            method: string;
            params: { theme: { layers: { kind: string; sensor_id?: string; unit?: string }[] } };
          }[];
        }
      ).electricalCalls.find((c) => c.method === 'set_lcd_theme')?.params.theme,
  );
  expect(theme?.layers.filter((l) => l.kind === 'sensor')).toMatchObject([
    { sensor_id: 'gpu:voltage', unit: 'volts' },
    { sensor_id: 'gpu:clock', unit: 'hertz' },
    { sensor_id: 'net:rx', unit: 'bytes_per_second' },
    { sensor_id: 'storage:used', unit: 'bytes' },
  ]);
});
