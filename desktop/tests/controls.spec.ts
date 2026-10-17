import { test, expect } from '@playwright/test';
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { method: string; params: Record<string, unknown> }[] = [];
    const thermal = { requested: { cpu: null, gpu: { sensor_id: 'gpu:test', threshold_c: 90, color: { r: 255, g: 0, b: 0 } } }, active_source: null, degraded_sources: [], grace_remaining_ms: 0, blocked_reason: null, fan: { last_error: null }, pump: { last_error: null } };
    Object.assign(window, { bridgeCalls: calls, __TAURI_INTERNALS__: { invoke: async (_command: string, request: { method: string; params: Record<string, unknown> }) => {
        if (_command === 'desktop_set_language') return null;
      calls.push(request);
      switch (request.method) {
        case 'list_devices': return [{ id: 'lcd:test', display_name: 'LCDV2', family:'galahad2-lcdv2' }];
        case 'list_sensors': return [{ id: 'cpu:test', label: 'CPU package', unit: 'celsius', value: 42, status: 'valid', age_ms: 10 }, { id: 'gpu:test', label: 'GPU edge', unit: 'celsius', value: 45, status: 'valid', age_ms: 10 }];
        case 'get_lcd_state': case 'patch_lcd_config': return { source: 'Theme', last_error: null, requested: { mode: 'Application', brightness_percent: 65, rotation_degrees: 90 } };
        case 'get_lighting_info': return { modes: ['static_color', 'breathing'], max_speed: 100, max_brightness: 100 };
        case 'set_lighting': return {};
        case 'get_thermal_state': return thermal;
        case 'set_thermal_config': if ((window as unknown as {rejectThermal?:boolean}).rejectThermal) throw {message:'invalid argument: thermal alerts require known fan and pump lighting bases'}; return { ...thermal, requested: request.params.config };
        default: throw { message: 'Unhandled mock ' + request.method };
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
});
test('LCD settings send field patches without changing source', async ({ page }) => {
  await page.getByRole('button', { name: 'Настройки', exact: true }).click();
  await page.getByRole('button', { name: 'Загрузить текущие значения' }).click();
  await expect(page.getByRole('combobox', { name: 'Ориентация' })).toHaveValue('90');
  await page.getByRole('combobox', { name: 'Ориентация' }).selectOption('180');
  await page.getByRole('button', { name: 'Применить поворот' }).click();
  await expect(page.getByRole('region', { name: 'Настройки LCD', exact: true }).getByRole('status')).toContainText('Остальные параметры LCD сохранены');
  const patches = await page.evaluate(() => (window as unknown as { bridgeCalls: { method: string; params: { patch: object } }[] }).bridgeCalls.filter(c => c.method === 'patch_lcd_config').map(c => c.params.patch));
  expect(patches).toEqual([{ rotation_degrees: 180 }]);
});
test('lighting uses catalog; thermal editing preserves the other source rule', async ({ page }) => {
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await page.getByRole('combobox', { name: 'Эффект', exact: true }).selectOption('breathing');
  await page.getByRole('button', { name: 'Применить подсветку', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Команда подсветки выполнена');
  await expect(page.getByRole('button', { name: 'Применить оповещения' })).toBeDisabled();
  await page.getByRole('button', { name: 'Загрузить текущие оповещения' }).click();
  await page.getByLabel('Включить CPU').check();
  await page.getByRole('combobox', { name: 'Датчик CPU', exact: true }).selectOption('cpu:test');
  await page.getByRole('button', { name: 'Применить оповещения' }).click();
  await expect(page.getByRole('status')).toContainText('оповещения сохранены');
  const calls = await page.evaluate(() => (window as unknown as { bridgeCalls: { method: string; params: { cfg?: { mode: string; scope: string }; config?: { cpu: { sensor_id: string }; gpu: { sensor_id: string; threshold_c: number } } } }[] }).bridgeCalls.filter(c => c.method === 'set_lighting' || c.method === 'set_thermal_config'));
  expect(calls[0].params.cfg).toMatchObject({ mode: 'breathing', scope: 'outer' });
  expect(calls[1].params.config?.cpu.sensor_id).toBe('cpu:test');
  expect(calls[1].params.config?.gpu).toMatchObject({ sensor_id: 'gpu:test', threshold_c: 90 });
});

test('thermal setup error explains how to establish restore settings and permits retry', async ({page}) => {
  await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await page.getByRole('button',{name:'Загрузить текущие оповещения'}).click();
  await page.evaluate(()=>Object.assign(window,{rejectThermal:true}));
  await page.getByRole('button',{name:'Применить оповещения'}).click();
  await expect(page.getByRole('alert')).toContainText('Сначала примените обычную подсветку в обеих зонах');
  await page.evaluate(()=>Object.assign(window,{rejectThermal:false}));
  await page.getByRole('button',{name:'Применить оповещения'}).click();
  await expect(page.getByRole('status')).toContainText('оповещения сохранены');
  await expect(page.getByRole('alert')).toHaveCount(0);
});
