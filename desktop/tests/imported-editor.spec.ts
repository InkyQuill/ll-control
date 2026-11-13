import { loadControls } from './editor-controls';
import { test, expect } from '@playwright/test';
import raw from './fixtures/basic-editor-theme.json' with { type: 'json' };
import type { Theme } from '../src/lib/api';
declare global {
  interface Window {
    importedLoad: {
      result: Theme | null;
      pending: boolean;
      release?: () => void;
      fail: boolean;
      writes: string[];
    };
  }
}
for (const language of ['en-US', 'ru-RU'] as const)
  test(`${language}: imported editor loads into draft, preserves newer edits and requires explicit Apply`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1024, height: 1000 });
    await page.addInitScript(
      ({ theme, language }) => {
        localStorage.setItem('lconnect.interface-language', language);
        const state: Window['importedLoad'] = {
          result: theme,
          pending: false,
          fail: false,
          writes: [],
        };
        Object.assign(window, {
          importedLoad: state,
          __TAURI_INTERNALS__: {
            invoke: async (
              command: string,
              request: { method: string; params: Record<string, unknown> } = {
                method: '',
                params: {},
              },
            ) => {
              if (command !== 'daemon_call') return null;
              switch (request.method) {
                case 'preview_lcd_editor':
                  return { jpeg: [], layers: [] };
                case 'get_display_settings':
                  return {
                    version: 1,
                    revision: 0,
                    temperature_unit: 'celsius',
                    interface_language: language,
                  };
                case 'list_devices':
                  return [
                    { id: 'lcd:fixture', display_name: 'Owned LCDV2', family: 'galahad2-lcdv2' },
                  ];
                case 'list_sensors':
                  return [];
                case 'get_lcd_state':
                  return {
                    source: 'Idle',
                    last_error: null,
                    requested: { mode: 'LocalUI', brightness_percent: 50, rotation_degrees: 0 },
                  };
                case 'get_lcd_media':
                  return null;
                case 'get_screen_collections':
                  return {
                    collections: null,
                    persistence: { pending: false, saved: false, last_error: null },
                  };
                case 'list_profiles':
                  return { version: 1, revision: 0, profiles: [] };
                case 'get_imported_lcd_editor':
                  if (state.pending)
                    await new Promise<void>((resolve) => (state.release = resolve));
                  if (state.fail) throw new Error('owned failure');
                  return structuredClone(state.result);
                case 'set_lcd_theme':
                  state.writes.push(request.method);
                  return { source: 'Theme', last_error: null };
                default:
                  return [];
              }
            },
          },
        });
      },
      { theme: raw as Theme, language },
    );
    const text = (en: string, ru: string) => (language === 'en-US' ? en : ru);
    await page.goto('/?view=display');
    await loadControls(page);
    const load = page.getByRole('button', {
      name: text('Load imported editor', 'Загрузить импортированный редактор'),
      exact: true,
    });
    const name = page.getByLabel(text('Name', 'Название'), { exact: true });
    await load.click();
    await expect(name).toHaveValue(raw.name);
    await expect(
      page.getByRole('combobox', { name: text('Background preset', 'Вариант фона'), exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('status').filter({
        hasText: text(
          'Imported editor loaded into the draft. Applying is a separate action.',
          'Импортированный редактор загружен в черновик. Применение — отдельное действие.',
        ),
      }),
    ).toBeVisible();
    await page.screenshot({ path: `../.impeccable/review/imported-editor-${language}.png` });
    expect(await page.evaluate(() => window.importedLoad.writes)).toEqual([]);
    await page.evaluate(() => {
      window.importedLoad.pending = true;
    });
    await load.click();
    await expect(load).toBeDisabled();
    await name.fill('Newer local draft');
    await page.evaluate(() => {
      window.importedLoad.pending = false;
      window.importedLoad.release?.();
    });
    await expect(load).toBeEnabled();
    await expect(name).toHaveValue('Newer local draft');
    await page.evaluate(() => {
      window.importedLoad.result = null;
    });
    await load.click();
    await expect(page.getByRole('alert')).toContainText(
      text(
        'No imported basic editor is available to load.',
        'Нет импортированного basic-редактора для загрузки.',
      ),
    );
    await expect(name).toHaveValue('Newer local draft');
    await page.evaluate(() => {
      window.importedLoad.fail = true;
    });
    await load.click();
    await expect(page.getByRole('alert')).toContainText(
      text('Could not load the imported editor:', 'Не удалось загрузить импортированный редактор:'),
    );
    await expect(name).toHaveValue('Newer local draft');
    expect(await page.evaluate(() => window.importedLoad.writes)).toEqual([]);
    await page
      .getByRole('button', { name: text('Apply to LCD', 'Применить к LCD'), exact: true })
      .click();
    await expect
      .poll(() => page.evaluate(() => window.importedLoad.writes))
      .toEqual(['set_lcd_theme']);
  });
