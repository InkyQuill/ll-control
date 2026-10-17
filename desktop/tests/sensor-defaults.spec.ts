import { test, expect } from '@playwright/test';

test('global source draft survives polling/navigation/reconnect and saves only on Apply', async ({ page }) => {
  await page.addInitScript(() => {
    const empty = () => ({ temperature: null, clock: null, load: null, power: null, fan: null, voltage: null });
    let config = { version: 1, revision: 0, cpu: empty(), gpu: null } as {
      version: number; revision: number; cpu: Record<string, string | null>;
      gpu: (Record<string, string | null> & { device_id: string }) | null
    };
    const candidates = [...['A', 'B'].map(gpu => ({ role: 'gpu_load', gpu_id: `gpu:${gpu}`, reading: { id: `${gpu}:load`, label: `GPU ${gpu} load`, unit: 'percent', value: 25, status: 'valid', age_ms: 0 } })), { role: 'cpu_fan', gpu_id: null, reading: { id: 'hwmon:2f64:nct6775:fan1', label: 'Motherboard header', unit: 'rpm', value: 0, status: 'valid', age_ms: 0 } }];
    const writes: unknown[] = [];
    Object.assign(window, { sourceWrites: writes, sourceOffline: false, externalSourceEdit: () => { config.revision++; } });
    Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (_: string, request: { method: string; params: { config: typeof config } }) => {
        if (_ === 'desktop_set_language') return null;
      if ((window as unknown as { sourceOffline: boolean }).sourceOffline) throw new Error('offline');
      switch (request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices': return [];
        case 'list_sensors': return candidates.map(c => c.reading);
        case 'get_sensor_defaults': return { config: structuredClone(config), recommended: { version: 1, revision: config.revision, cpu: empty(), gpu: { ...empty(), memory_used: null, device_id: 'gpu:A', load: 'A:load' } }, candidates, values: [{ role: 'gpu_load', sensor_id: config.gpu?.load ?? null, reading: candidates.find(c => c.reading.id === config.gpu?.load)?.reading ?? null }] };
        case 'set_sensor_defaults':
          if (request.params.config.revision !== config.revision) throw new Error('revision conflict');
          writes.push(structuredClone(request.params.config));
          config = { ...structuredClone(request.params.config), revision: config.revision + 1 };
          return structuredClone(config);
        default: throw new Error(request.method);
      }
    } } });
  });
  await page.goto('/?view=display');
  await page.getByRole('button', { name: 'Датчики', exact: true }).click();
  const editor = page.getByRole('region', { name: 'Источники системных датчиков' });
  const gpu = editor.getByLabel('Системная видеокарта');
  await editor.getByRole('button', { name: 'Восстановить рекомендуемый выбор' }).click();
  await expect(gpu).toHaveValue('gpu:A');
  const fan = editor.getByRole('combobox', { name: 'CPU · Вентилятор', exact: true });
  await expect(fan).toHaveValue('');
  await expect(fan).toHaveAttribute('aria-describedby', 'cpu-fan-help');
  await fan.selectOption('hwmon:2f64:nct6775:fan1');
  expect(await page.evaluate(() => (window as unknown as { sourceWrites: unknown[] }).sourceWrites.length)).toBe(0);
  await gpu.selectOption('gpu:B');
  await editor.getByRole('combobox', { name: 'GPU · Загрузка', exact: true }).selectOption('B:load');
  await expect(editor.getByRole('combobox', { name: 'GPU · Загрузка', exact: true }).locator('option')).not.toContainText(['GPU A load']);
  await expect(editor.getByText('Есть несохранённые изменения.')).toBeVisible();
  await page.getByRole('button', { name: 'Экран помпы', exact: true }).click();
  await page.getByRole('button', { name: 'Датчики', exact: true }).click();
  await expect(gpu).toHaveValue('gpu:B');
  await page.evaluate(() => Object.assign(window, { sourceOffline: true }));
  await expect(page.getByText('Нет связи с daemon', { exact: true })).toBeVisible();
  await expect(editor.getByRole('button', { name: 'Применить источники' })).toBeDisabled();
  await page.evaluate(() => Object.assign(window, { sourceOffline: false }));
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await expect(fan).toHaveValue('hwmon:2f64:nct6775:fan1');
  await expect(editor.getByRole('combobox', { name: 'GPU · Загрузка', exact: true })).toHaveValue('B:load');
  expect(await page.evaluate(() => (window as unknown as { sourceWrites: unknown[] }).sourceWrites.length)).toBe(0);
  await editor.getByRole('button', { name: 'Применить источники' }).click();
  await expect(editor.getByRole('status')).toContainText('Источники сохранены');
  expect(await page.evaluate(() => (window as unknown as { sourceWrites: { cpu: { fan: string } }[] }).sourceWrites[0].cpu.fan)).toBe('hwmon:2f64:nct6775:fan1');
  await expect(editor.getByText('25.0%', { exact: true })).toBeVisible();
  await editor.getByRole('button', { name: 'Очистить выбор в черновике' }).click();
  await expect(gpu).toHaveValue('');
  await expect(fan).toHaveValue('');
  expect(await page.evaluate(() => (window as unknown as { sourceWrites: unknown[] }).sourceWrites.length)).toBe(1);
  await page.evaluate(() => (window as unknown as { externalSourceEdit: () => void }).externalSourceEdit());
  await expect(editor.getByRole('alert')).toContainText('Настройки изменились извне');
  await expect(editor.getByRole('button', { name: 'Применить источники' })).toBeDisabled();
  await editor.getByRole('button', { name: 'Загрузить сохранённый выбор' }).click();
  await expect(gpu).toHaveValue('gpu:B');
  await expect(editor.getByRole('button', { name: 'Применить источники' })).toBeEnabled();
  await expect(fan).toHaveValue('hwmon:2f64:nct6775:fan1');
});
