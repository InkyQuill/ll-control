import {loadControls} from './editor-controls';
import { test, expect } from '@playwright/test';
import type { Theme } from '../src/lib/api';

type Operation = 'set_lcd_theme' | 'clear_lcd' | 'get_lcd_theme';
interface Fixture {
  online: boolean;
  hold: Operation | null;
  active: Theme | null;
  calls: string[];
  finish: (fail: boolean) => void;
}

for (const operation of ['set_lcd_theme', 'clear_lcd', 'get_lcd_theme'] as const) {
  for (const fail of [false, true]) {
    test(`${operation}: reconnect discards late ${fail ? 'error' : 'success'} without replaying the operation`, async ({ page }) => {
      await page.addInitScript(() => {
        const fixture: Fixture = {
          online: true, hold: null, calls: [], finish: () => {},
          active: { version: 1, name: 'Previously active', background: [0, 0, 0], layers: [] },
        };
        Object.assign(window, { interruptedDisplay: fixture, __TAURI_INTERNALS__: {
          invoke: async (_: string, req: { method: string; params: { theme?: Theme } }) => {
        if (_ === 'desktop_set_language') return null;
            if (!fixture.online) throw new Error('offline');
            switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
              case 'list_devices': return [{ id: 'A', display_name: 'LCD A', family:'galahad2-lcdv2' }];
              case 'list_sensors': return [];
              case 'get_lcd_state': return { source: fixture.active ? 'Theme' : 'Idle', last_error: null };
              case 'set_lcd_theme': fixture.active = structuredClone(req.params.theme!); break;
              case 'clear_lcd': fixture.active = null; break;
              case 'get_lcd_theme': break;
              default: throw new Error(req.method);
            }
            fixture.calls.push(req.method);
            const result = req.method === 'get_lcd_theme' ? structuredClone(fixture.active)
              : { source: 'Theme', last_error: 'Obsolete operation state' };
            if (fixture.hold === req.method) return new Promise((resolve, reject) => {
              fixture.finish = fail => fail ? reject(new Error('Obsolete operation error')) : resolve(result);
            });
            return result;
          },
        } });
      });
      await page.goto('/?view=display');
      await expect(page.getByText('Daemon подключён')).toBeVisible();
      const title = page.getByLabel('Название', { exact: true });
      await title.fill('Submitted draft');
      await page.evaluate(operation => {
        (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay.hold = operation;
      }, operation);
      const button = page.getByRole('button', { name: {
        set_lcd_theme: 'Применить к LCD', clear_lcd: 'Вернуть LocalUI', get_lcd_theme: 'Загрузить активную тему',
      }[operation], exact: true });
      if(operation!=='set_lcd_theme')await loadControls(page);
      await button.click();
      await expect(button).toBeDisabled();
      await title.fill('New unsent edits');
      await page.evaluate(() => { (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay.online = false; });
      await expect(page.getByText('Нет связи с daemon')).toBeVisible();
      await page.evaluate(() => { (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay.online = true; });
      await expect(page.getByText('Daemon подключён')).toBeVisible();
      await page.evaluate(fail => { (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay.finish(fail); }, fail);
      await expect(button).toBeEnabled();
      await expect(title).toHaveValue('New unsent edits');
      await expect(page.getByText('Obsolete operation', { exact: false })).toHaveCount(0);
      await expect(page.getByText('Тема отправлена.', { exact: false })).toHaveCount(0);
      await expect(page.getByText('Запрошен LocalUI.', { exact: false })).toHaveCount(0);
      await expect(page.getByText('Черновик изменился.', { exact: false })).toHaveCount(0);
      expect(await page.evaluate(() => {
        const f = (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay;
        return { calls: f.calls, active: f.active?.name ?? null };
      })).toEqual({ calls: [operation], active: operation === 'clear_lcd' ? null
        : operation === 'set_lcd_theme' ? 'Submitted draft' : 'Previously active' });
      // Explicit readback is separate from reconnect and never reapplies the new draft.
      await page.evaluate(() => { (window as unknown as { interruptedDisplay: Fixture }).interruptedDisplay.hold = null; });
      await loadControls(page);await page.getByRole('button', { name: 'Загрузить активную тему', exact: true }).click();
      if (operation === 'clear_lcd') {
        await expect(page.getByText('На устройстве нет активной темы', { exact: false })).toBeVisible();
        await expect(title).toHaveValue('New unsent edits');
      } else {
        await expect(title).toHaveValue(operation === 'set_lcd_theme' ? 'Submitted draft' : 'Previously active');
      }
    });
  }
}
