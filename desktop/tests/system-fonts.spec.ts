import { test, expect } from '@playwright/test';
import { selectLayer } from './editor-controls';
type FontCall = {
  method?: string;
  params?: {
    request?: { action: string; size: number; bytes: number[] };
    theme?: { layers: { style: { font_asset?: string } }[] };
  };
};
declare global {
  interface Window {
    systemFontCalls: FontCall[];
  }
}
for (const failure of [false, true])
  test(`system font selection copies a managed asset; failure=${failure}`, async ({ page }) => {
    await page.addInitScript(
      ({ failure }) => {
        const calls: FontCall[] = [];
        let received = 0,
          size = 0;
        Object.assign(window, {
          systemFontCalls: calls,
          __TAURI_INTERNALS__: {
            invoke: async (command: string, request: FontCall = {}) => {
              if (command === 'system_fonts')
                return [{ id: 'opaque-font-id', name: 'Installed Example · Regular' }];
              if (command === 'system_font_read') {
                if (failure) throw new Error('Font removed');
                return new Uint8Array(70000).fill(1).buffer;
              }
              if (command === 'desktop_set_language') return null;
              calls.push(request);
              switch (request.method) {
                case 'list_devices':
                  return [{ id: 'lcd:test', display_name: 'LCDV2', family: 'galahad2-lcdv2' }];
                case 'list_sensors':
                  return [];
                case 'get_lcd_state':
                  return { source: 'Idle', last_error: null };
                case 'preview_lcd_editor':
                  return { jpeg: [], layers: [] };
                case 'font_upload': {
                  const r = request.params!.request!;
                  if (r.action === 'begin') size = r.size;
                  if (r.action === 'chunk') received += r.bytes.length;
                  return {
                    id: 'upload',
                    size,
                    received,
                    asset: r.action === 'finish' ? '0123456789abcdef0123456789abcdef' : null,
                    cancelled: false,
                  };
                }
                case 'set_lcd_theme':
                  return { source: 'Theme', last_error: null };
                default:
                  throw new Error('Unexpected ' + request.method);
              }
            },
          },
        });
      },
      { failure },
    );
    await page.goto('/?view=display');
    await expect(page.getByText('Daemon подключён')).toBeVisible();
    await selectLayer(page, 0);
    const select = page.getByRole('combobox', { name: 'Шрифт', exact: true }).first();
    const original = await select.inputValue();
    await expect(select.locator('option[value="system:opaque-font-id"]')).toHaveCount(1);
    await select.selectOption('system:opaque-font-id');
    if (failure) {
      await expect(page.getByRole('alert')).toContainText('Font removed');
      await expect(select).toHaveValue(original);
    } else {
      await expect(select).toHaveValue('asset:0123456789abcdef0123456789abcdef');
      await expect(select.locator('option:checked')).toHaveText('Installed Example · Regular');
      const calls = await page.evaluate(() => window.systemFontCalls);
      expect(
        calls
          .filter((c) => c.method === 'font_upload' && c.params?.request?.action === 'chunk')
          .map((c) => c.params!.request!.bytes.length),
      ).toEqual([65536, 4464]);
      expect(calls.some((c) => c.method === 'set_lcd_theme')).toBe(false);
      await page.getByRole('button', { name: 'Применить к LCD', exact: true }).click();
      await expect
        .poll(() =>
          page.evaluate(
            () =>
              window.systemFontCalls.find((c) => c.method === 'set_lcd_theme')?.params?.theme
                ?.layers[0].style.font_asset,
          ),
        )
        .toBe('0123456789abcdef0123456789abcdef');
    }
  });
