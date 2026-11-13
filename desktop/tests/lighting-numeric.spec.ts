import { test, expect } from '@playwright/test';
test('numeric lighting validation exposes ranges and prevents invalid writes', async ({ page }) => {
  await page.addInitScript(() => {
    const writes: string[] = [];
    const thermal = {
      requested: { cpu: null, gpu: null },
      active_source: null,
      degraded_sources: [],
      grace_remaining_ms: 0,
      blocked_reason: null,
      fan: { last_error: null },
      pump: { last_error: null },
    };
    const dynamic = {
      base_mode: 'breathing',
      color_count: 1,
      speed_supported: true,
      requested: null,
      applied: null,
      degraded: false,
      suppressed_by_thermal: false,
      last_error: null,
    };
    Object.assign(window, {
      numericWrites: writes,
      __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string }) => {
          if (_ === 'desktop_set_language') return null;
          switch (req.method) {
            case 'preview_lcd_editor':
              return { jpeg: [], layers: [] };
            case 'list_devices':
              return [{ id: 'mock', display_name: 'LCD', family: 'galahad2-lcdv2' }];
            case 'list_sensors':
              return [
                { id: 'cpu', label: 'CPU', unit: 'celsius', value: 42, status: 'valid' },
                { id: 'pump', label: 'Pump', unit: 'rpm', value: 2200, status: 'valid' },
              ];
            case 'get_lcd_state':
              return { source: 'Idle', requested: {}, last_error: null };
            case 'get_lighting_info':
              return { modes: ['breathing'] };
            case 'get_thermal_state':
              return thermal;
            case 'get_dynamic_rgb':
              return dynamic;
            case 'set_thermal_config':
              writes.push(req.method);
              return thermal;
            case 'set_dynamic_rgb':
              writes.push(req.method);
              return dynamic;
            default:
              throw new Error(req.method);
          }
        },
      },
    });
  });
  const writes = () =>
    page.evaluate(() => (window as unknown as { numericWrites: string[] }).numericWrites);
  await page.goto('/?view=display');
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await page.getByRole('button', { name: 'Загрузить текущие оповещения' }).click();
  await page.getByLabel('Включить CPU', { exact: true }).check();
  await page.getByRole('combobox', { name: /^Датчик CPU/ }).selectOption('cpu');
  const threshold = page.getByRole('spinbutton', { name: 'Порог CPU, °C' });
  await expect(threshold).toHaveAttribute('aria-valuemin', '0');
  await expect(threshold).toHaveAttribute('aria-valuemax', '150');
  for (const bad of ['', '-1', '151']) {
    await threshold.fill(bad);
    await expect(threshold).toHaveAttribute('aria-invalid', 'true');
    await page.getByRole('button', { name: 'Применить оповещения' }).click();
    await expect(page.getByRole('alert')).toContainText('от 0 до 150');
    expect(await writes()).toEqual([]);
  }
  await threshold.fill('85.5');
  await expect(threshold).toHaveAttribute('aria-valuenow', '85.5');
  await expect(threshold).toHaveAttribute('aria-invalid', 'false');
  expect(await threshold.evaluate((input: HTMLInputElement) => input.validity.valid)).toBe(true);
  await page.getByRole('button', { name: 'Применить оповещения' }).click();
  await expect.poll(writes).toEqual(['set_thermal_config']);
  await page.getByText('Подсветка помпы по датчику', { exact: true }).click();
  await page.getByRole('button', { name: 'Загрузить настройки режима по датчику' }).click();
  await page.getByRole('combobox', { name: /^Датчик подсветки/ }).selectOption('pump');
  const low = page.getByRole('spinbutton', { name: 'Нижний порог (rpm)' });
  const high = page.getByRole('spinbutton', { name: 'Верхний порог (rpm)' });
  await expect(low).toHaveAttribute('aria-valuemax', '3600');
  await low.fill('2200');
  await high.fill('3000');
  const apply = page.getByRole('button', { name: 'Применить режим по датчику' });
  const speed = page.getByRole('spinbutton', { name: 'Нижняя скорость, %' });
  for (const bad of ['', '-1', '101', '2.5']) {
    await speed.fill(bad);
    await expect(speed).toHaveAttribute('aria-invalid', 'true');
    await apply.click();
    expect(await writes()).toEqual(['set_thermal_config']);
  }
  await speed.fill('25');
  await expect(speed).toHaveAttribute('aria-valuenow', '25');
  await expect(speed).toHaveAttribute('aria-invalid', 'false');
  await high.fill('2199');
  await expect(low).toHaveAttribute('aria-invalid', 'true');
  await expect(high).toHaveAttribute('aria-invalid', 'true');
  await apply.click();
  expect(await writes()).toEqual(['set_thermal_config']);
  await high.fill('3000');
  await apply.click();
  await expect.poll(writes).toEqual(['set_thermal_config', 'set_dynamic_rgb']);
  await page.getByRole('combobox', { name: /^Датчик подсветки/ }).selectOption('cpu');
  const celsius = page.getByRole('spinbutton', { name: 'Нижний порог (°C)' });
  await expect(celsius).toHaveAttribute('aria-valuemax', '100');
  await expect(celsius).toHaveValue('30');
  await expect(celsius).toHaveAttribute('aria-invalid', 'false');
  await page.getByRole('combobox', { name: /^Датчик подсветки/ }).selectOption('pump');
  await expect(page.getByRole('spinbutton', { name: 'Нижний порог (rpm)' })).toHaveValue('2200');
  await expect(page.getByRole('spinbutton', { name: 'Верхний порог (rpm)' })).toHaveValue('3000');
});
