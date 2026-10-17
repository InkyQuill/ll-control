import { test, expect } from '@playwright/test';
interface Config { cpu: { sensor_id: string; threshold_c: number; color: { r: number; g: number; b: number } } | null; gpu: null }
interface Write { method: string; dev: string; config?: Config; cfg?: { brightness_percent: number } }
interface Fixture { online: boolean; finish: (commit: boolean, fail: boolean) => void; writes: Write[]; reads: number }
for (const thermal of [false, true]) for (const outcome of ['lost-reply', 'rejected', 'device-success', 'device-failure', 'reconnect-success', 'reconnect-failure']) {
  test(`${thermal ? 'thermal' : 'manual'} ${outcome} preserves draft and never retries`, async ({ page }) => {
    await page.addInitScript(() => {
      const configs: Record<string, Config> = Object.fromEntries(['A', 'B'].map(dev => [dev, { cpu: { sensor_id: 'cpu', threshold_c: 80, color: { r: 255, g: 0, b: 0 } }, gpu: null }]));
      const grace: Record<string, number> = { A: 0, B: 0 };
      const state = (dev: string) => ({ requested: configs[dev], active_source: null, degraded_sources: [], grace_remaining_ms: grace[dev], blocked_reason: null, fan: { last_error: null }, pump: { last_error: null } });
      const fixture: Fixture = { online: true, finish: () => {}, writes: [], reads: 0 };
      Object.assign(window, { interruptedLighting: fixture, __TAURI_INTERNALS__: {
        invoke: async (_: string, req: { method: string; params: Omit<Write, 'method'> }) => {
        if (_ === 'desktop_set_language') return null;
          if (!fixture.online) throw new Error('offline');
          switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
            case 'list_devices': return [{ id: 'A', display_name: 'LCD A', family:'galahad2-lcdv2' }, { id: 'B', display_name: 'LCD B', family:'galahad2-lcdv2' }];
            case 'list_sensors': return [{ id: 'cpu', label: 'CPU', unit: 'celsius', value: 42, status: 'valid', age_ms: 1 }];
            case 'get_lcd_state': return { source: 'Idle', requested: {} };
            case 'get_lighting_info': return { modes: ['static_color'] };
            case 'get_thermal_state': fixture.reads++; return state(req.params.dev);
            case 'get_dynamic_rgb': return { base_mode: 'static_color', color_count: 1, speed_supported: false, requested: null, applied: null, degraded: false, suppressed_by_thermal: false, last_error: null };
            case 'set_lighting': case 'set_thermal_config': {
              const write = structuredClone({ method: req.method, ...req.params });
              fixture.writes.push(write);
              return new Promise((resolve, reject) => {
                fixture.finish = (commit, fail) => {
                  if (commit) {
                    if (write.config) configs[write.dev] = write.config;
                    else grace[write.dev] = 10000;
                  }
                  if (fail) reject(new Error('mutation response unavailable'));
                  else resolve(write.config ? state(write.dev) : {});
                };
              });
            }
            default: throw new Error(req.method);
          }
        }
      } });
    });
    await page.goto('/?view=display'); await expect(page.getByText('Daemon подключён')).toBeVisible();
    await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
    const load = page.getByRole('button', { name: 'Загрузить текущие оповещения' });
    await expect(load).toBeEnabled(); await load.click();
    const field = thermal ? page.getByLabel('Порог CPU, °C') : page.getByRole('slider', { name: 'Яркость RGB:', exact: false });
    await field.fill('91');
    const apply = page.getByRole('button', { name: thermal ? 'Применить оповещения' : 'Применить подсветку', exact: true });
    await apply.click(); await expect(apply).toBeDisabled(); await field.fill('94');
    const obsolete = outcome.startsWith('device') || outcome.startsWith('reconnect');
    if (outcome.startsWith('device')) {
      await page.getByLabel('Устройство', { exact: true }).selectOption('B');
      await expect(page.getByText('Черновик оповещений относится к другому устройству.', { exact: false })).toBeVisible();
      await page.getByLabel('Устройство', { exact: true }).selectOption('A');
    } else if (outcome.startsWith('reconnect')) {
      await page.evaluate(() => { (window as unknown as { interruptedLighting: Fixture }).interruptedLighting.online = false; });
      await expect(page.getByText('Нет связи с daemon')).toBeVisible();
      await page.evaluate(() => { (window as unknown as { interruptedLighting: Fixture }).interruptedLighting.online = true; });
      await expect(page.getByText('Daemon подключён')).toBeVisible();
    }
    const commit = outcome !== 'rejected';
    const fail = !outcome.endsWith('success');
    const reads = await page.evaluate(({ commit, fail }) => {
      const f = (window as unknown as { interruptedLighting: Fixture }).interruptedLighting;
      f.finish(commit, fail); return f.reads;
    }, { commit, fail });
    await expect(apply).toBeEnabled(); await expect(field).toHaveValue('94');
    await expect(page.getByText('mutation response unavailable', { exact: true })).toHaveCount(obsolete ? 0 : 1);
    await expect(page.getByText('Температурные оповещения сохранены.', { exact: true })).toHaveCount(0);
    await expect(page.getByText('Команда подсветки выполнена.', { exact: false })).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => (window as unknown as { interruptedLighting: Fixture }).interruptedLighting.reads)).toBeGreaterThan(reads);
    await expect(field).toHaveValue('94');
    if (thermal) {
      await load.click(); await expect(field).toHaveValue(commit ? '91' : '80');
    } else await expect(page.getByText(`Grace: ${commit ? 10 : 0} s.`, { exact: false })).toBeVisible();
    const writes = await page.evaluate(() => (window as unknown as { interruptedLighting: Fixture }).interruptedLighting.writes);
    expect(writes).toHaveLength(1); expect(writes[0].dev).toBe('A');
    expect(thermal ? writes[0].config?.cpu?.threshold_c : writes[0].cfg?.brightness_percent).toBe(91);
  });
}
