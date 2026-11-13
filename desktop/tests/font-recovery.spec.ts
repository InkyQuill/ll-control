import { selectLayer } from './editor-controls';
import { test, expect } from '@playwright/test';

type FontRequest = {
  action: string;
  id?: string;
  size?: number;
  offset?: number;
  bytes?: number[];
};
type Call = { method: string; params: { request?: FontRequest; theme?: unknown } };
type FixtureWindow = Window & {
  fontRecovery: { calls: Call[]; offline: boolean; release: (() => void) | null };
};

for (const mode of ['disconnect', 'invalid-font'] as const) {
  test(`font ${mode} preserves draft and permits explicit retry`, async ({ page }) => {
    await page.addInitScript(
      ({ mode }) => {
        const fixture = {
          calls: [] as Call[],
          offline: false,
          release: null as (() => void) | null,
        };
        let attempt = 0,
          received = 0,
          size = 0,
          asset: string | null = null;
        const state = () => ({
          id: attempt.toString(16).padStart(32, '0'),
          size,
          received,
          asset,
          cancelled: false,
        });
        Object.assign(window, {
          fontRecovery: fixture,
          __TAURI_INTERNALS__: {
            invoke: async (_: string, call: Call) => {
              if (_ === 'desktop_set_language') return null;
              fixture.calls.push(structuredClone(call));
              if (fixture.offline && call.method !== 'font_upload') throw new Error('Disconnected');
              switch (call.method) {
                case 'preview_lcd_editor':
                  return { jpeg: [], layers: [] };
                case 'list_devices':
                  return [{ id: 'lcd:test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
                case 'list_sensors':
                  return [];
                case 'get_lcd_state':
                  return {
                    source: 'Theme',
                    last_error: null,
                    requested: {
                      mode: 'Application',
                      brightness_percent: 100,
                      rotation_degrees: 0,
                    },
                  };
                case 'preview_lcd_theme':
                  return [255, 216, 255, 217];
                case 'font_upload': {
                  const request = call.params.request!;
                  if (request.action === 'begin') {
                    attempt++;
                    size = request.size!;
                    received = 0;
                    asset = null;
                  }
                  if (request.action === 'chunk') {
                    if (attempt === 1 && mode === 'disconnect')
                      await new Promise<void>((resolve) => {
                        fixture.release = resolve;
                      });
                    received += request.bytes!.length;
                  }
                  if (request.action === 'finish') {
                    if (attempt === 1 && mode === 'invalid-font')
                      throw new Error('font tables or metrics are invalid');
                    asset = state().id;
                  }
                  return state();
                }
                default:
                  throw new Error(call.method);
              }
            },
          },
        });
      },
      { mode },
    );
    await page.goto('/?view=display');
    await expect(page.getByText('Daemon подключён')).toBeVisible();
    await selectLayer(page, 0);
    await page.getByLabel('Название', { exact: true }).fill('Keep my draft');
    await page.getByRole('button', { name: /^(Библиотека|Library)$/ }).click();
    await page.getByRole('button', { name: 'Пользовательские шрифты', exact: true }).click();
    const file = page.getByLabel('Загрузить шрифт', { exact: true });
    await file.setInputFiles({
      name: 'First.ttf',
      mimeType: 'font/ttf',
      buffer: Buffer.alloc(70000, 1),
    });
    if (mode === 'disconnect') {
      await expect
        .poll(() =>
          page.evaluate(() => !!(window as unknown as FixtureWindow).fontRecovery.release),
        )
        .toBe(true);
      await page.evaluate(() => {
        (window as unknown as FixtureWindow).fontRecovery.offline = true;
      });
      await expect(page.getByText('Нет связи с daemon', { exact: true })).toBeVisible();
      await expect(file).toBeDisabled();
      await page.evaluate(() => {
        (window as unknown as FixtureWindow).fontRecovery.release!();
      });
      await expect(page.getByText('Загрузка отменена. Тема не изменена.')).toBeVisible();
      await page.evaluate(() => {
        (window as unknown as FixtureWindow).fontRecovery.offline = false;
      });
      await expect(page.getByText('Daemon подключён')).toBeVisible();
    } else {
      await expect(
        page.getByRole('alert').filter({ hasText: 'font tables or metrics are invalid' }),
      ).toBeVisible();
    }
    await expect(file).toBeEnabled();
    await expect(page.getByLabel('Название', { exact: true })).toHaveValue('Keep my draft');
    await expect(
      page.getByRole('combobox', { name: 'Шрифт', exact: true, includeHidden: true }).first(),
    ).toHaveValue('sans');
    await file.setInputFiles({
      name: 'Retry.ttf',
      mimeType: 'font/ttf',
      buffer: Buffer.alloc(70000, 1),
    });
    await expect(page.getByText('Шрифт загружен.', { exact: false })).toBeVisible();
    await page.getByRole('button', { name: 'Композиция', exact: true }).click();
    await page
      .getByRole('combobox', { name: 'Шрифт', exact: true, includeHidden: true })
      .first()
      .selectOption('asset:00000000000000000000000000000002');
    await page.getByRole('button', { name: 'Библиотека', exact: true }).click();
    const calls = await page.evaluate(
      () => (window as unknown as FixtureWindow).fontRecovery.calls,
    );
    expect(calls.filter((c) => c.method === 'set_lcd_theme')).toHaveLength(0);
    expect(calls.filter((c) => c.params?.request?.action === 'begin')).toHaveLength(2);
    expect(calls.filter((c) => c.params?.request?.action === 'cancel')).toHaveLength(2);
    expect(
      calls.filter(
        (c) =>
          c.params?.request?.action === 'chunk' &&
          c.params?.request.id === '00000000000000000000000000000001',
      ),
    ).toHaveLength(mode === 'disconnect' ? 1 : 2);
    expect(
      calls.filter(
        (c) =>
          c.params?.request?.action === 'finish' &&
          c.params?.request.id === '00000000000000000000000000000001',
      ),
    ).toHaveLength(mode === 'disconnect' ? 0 : 1);
  });
}
