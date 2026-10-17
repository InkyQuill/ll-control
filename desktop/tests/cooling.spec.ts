import { test, expect } from '@playwright/test';
test('cooling edits and presets stay local; Apply sends selected channel and policy', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { method: string; params: Record<string, unknown> }[] = [];
    const channel = { requested: null, applied_percent: 80, motherboard: false, fallback_active: false, last_error: null };
    Object.assign(window, { bridgeCalls: calls, __TAURI_INTERNALS__: {
      invoke: async (command: string, args: Record<string, unknown>) => {
        if (command === 'desktop_set_language') return null;
        if (command === 'cooling_preset') return { mode: 'curve_rpm', sensor_id: args.sensorId, points: [{ celsius: 30, rpm: 2000 }, { celsius: 85, rpm: 3000 }], hysteresis_c: 2, fallback_percent: 100 };
        const request = args as { method: string; params: Record<string, unknown> }; calls.push(request);
        switch (request.method) {
          case 'list_devices': return [{ id: 'lcd:test', display_name: 'LCDV2', family:'galahad2-lcdv2' }];
          case 'list_sensors': return [{ id: 'cpu:test', label: 'CPU package', unit: 'celsius', value: 42, status: 'valid', age_ms: 10 }];
          case 'get_lcd_state': return { source: 'Idle', last_error: null, requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 } };
          case 'get_cooling_state': return { pump: channel, fan: channel };
          case 'set_cooling_policy': return { pump: { ...channel, requested: request.params.policy, applied_percent: 65 }, fan: channel };
          default: throw { message: 'Unhandled mock: ' + request.method };
        }
      }
    } });
  });
  await page.goto('/?view=display');
  await page.getByRole('button', { name: 'Охлаждение', exact: true }).click();
  await expect(page.getByText('Applied: 80%')).toBeVisible();
  const numeric = page.getByRole('spinbutton', { name: 'PWM, %', exact: true });
  await expect(numeric).toHaveAttribute('aria-valuenow', '80');
  await expect(numeric).toHaveAttribute('aria-valuemin', '20');
  await expect(numeric).toHaveAttribute('aria-valuemax', '100');
  await numeric.fill('');
  await expect(numeric).not.toHaveAttribute('aria-valuenow');
  await expect(numeric).toHaveAttribute('aria-invalid', 'true');
  await numeric.fill('19');
  await expect(numeric).toHaveAttribute('aria-invalid', 'true');
  await numeric.fill('65');
  await expect(numeric).toHaveAttribute('aria-valuenow', '65');
  await expect(numeric).toHaveAttribute('aria-invalid', 'false');
  await page.getByRole('combobox', { name: 'Режим', exact: true }).selectOption('fixed_rpm');
  const rpm = page.getByRole('spinbutton', { name: 'Целевые RPM (reference mapping)', exact: true });
  await expect(rpm).toHaveAttribute('aria-valuemin', '800');
  await expect(rpm).toHaveAttribute('aria-valuemax', '3600');
  await expect(rpm).toHaveAttribute('aria-valuenow', '2200');
  await rpm.fill('2500');
  await expect(rpm).toHaveAttribute('aria-valuenow', '2500');
  await page.getByRole('combobox', { name: 'Канал', exact: true }).selectOption('fan');
  await expect(numeric).toHaveAttribute('aria-valuemin', '10');
  await expect(numeric).toHaveAttribute('aria-valuenow', '65');
  await page.getByRole('combobox', { name: 'Канал', exact: true }).selectOption('pump');

  await page.getByLabel('Источник температуры').selectOption('cpu:test');
  await page.getByRole('button', { name: 'quiet', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Preset загружен в редактор');
  await expect(page.getByRole('combobox', { name: 'Режим', exact: true })).toHaveValue('curve_rpm');
  const writes = () => page.evaluate(() => (window as unknown as { bridgeCalls: { method: string }[] }).bridgeCalls.filter(c => c.method === 'set_cooling_policy'));
  expect(await writes()).toHaveLength(0);
  const temperature = page.getByRole('spinbutton', { name: 'Точка 1: °C', exact: true });
  await expect(temperature).toHaveAttribute('aria-valuemin', '-50');
  await expect(temperature).toHaveAttribute('aria-valuemax', '150');
  await temperature.fill('');
  await expect(temperature).not.toHaveAttribute('aria-valuenow');
  await expect(temperature).toHaveAttribute('aria-invalid', 'true');
  await temperature.fill('35');
  await expect(temperature).toHaveAttribute('aria-valuenow', '35');
  const curveRpm = page.getByRole('spinbutton', { name: 'RPM', exact: true }).first();
  await expect(curveRpm).toHaveAttribute('aria-valuemin', '800');
  await expect(curveRpm).toHaveAttribute('aria-valuemax', '3600');
  await curveRpm.fill('799');
  await expect(curveRpm).toHaveAttribute('aria-invalid', 'true');
  await curveRpm.fill('2200');
  await expect(curveRpm).toHaveAttribute('aria-invalid', 'false');
  for (const [name, min, max, invalid, valid] of [
    ['Гистерезис, °C', '0', '10', '10.1', '2.5'],
    ['Fallback PWM, %', '20', '100', '19', '95'],
  ]) {
    const input = page.getByRole('spinbutton', { name, exact: true });
    await expect(input).toHaveAttribute('aria-valuemin', min);
    await expect(input).toHaveAttribute('aria-valuemax', max);
    await input.fill(invalid);
    await expect(input).toHaveAttribute('aria-invalid', 'true');
    await input.fill(valid);
    await expect(input).toHaveAttribute('aria-invalid', 'false');
    await expect(input).toHaveAttribute('aria-valuenow', valid);
  }

  await page.getByLabel('Точка 1: °C').fill('35');
  await page.getByRole('button', { name: 'Применить охлаждение' }).click();
  await expect(page.getByRole('status')).toContainText('Политика принята');
  const applied = await page.evaluate(() => (window as unknown as { bridgeCalls: { method: string; params: { target: string; policy: { sensor_id: string; points: { celsius: number }[] } } }[] }).bridgeCalls.find(c => c.method === 'set_cooling_policy')?.params);
  expect(applied?.target).toBe('pump'); expect(applied?.policy.sensor_id).toBe('cpu:test'); expect(applied?.policy.points[0].celsius).toBe(35);
  await page.evaluate(() => { (document.activeElement as HTMLElement)?.blur(); window.scrollTo(0, 0); });
  await page.screenshot({path:'../.impeccable/review/production-cooling.png',fullPage:true});
  await page.getByLabel('Точка 2: °C').fill('20');
  await expect(page.getByLabel('Точка 2: °C')).toHaveAttribute('aria-invalid', 'true');
  await page.getByRole('button', { name: 'Применить охлаждение' }).click();
  await expect(page.getByRole('alert')).toContainText('Температуры должны возрастать');
  expect(await writes()).toHaveLength(1);
});
