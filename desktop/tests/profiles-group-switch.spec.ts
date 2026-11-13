import { openEditorSection } from './editor-controls';
import { test, expect } from '@playwright/test';
import type { ProfileEdit, ProfileLibrary } from '../src/lib/api';
interface Fixture {
  library: ProfileLibrary;
  calls: string[];
  finish: (fail: boolean) => void;
}
for (const fail of [false, true]) {
  test(`pump reply after switching to fan: late ${fail ? 'error' : 'success'} cannot replace fan state`, async ({
    page,
  }) => {
    await page.addInitScript(() => {
      const f: Fixture = {
        library: { version: 1, revision: 0, profiles: [] },
        calls: [],
        finish: () => {},
      };
      Object.assign(window, {
        switchedProfiles: f,
        __TAURI_INTERNALS__: {
          invoke: async (
            _: string,
            req: { method: string; params: { edit: ProfileEdit; expected_revision: number } },
          ) => {
            if (_ === 'desktop_set_language') return null;
            switch (req.method) {
              case 'preview_lcd_editor':
                return { jpeg: [], layers: [] };
              case 'get_display_settings':
                return { version: 1, revision: 0, temperature_unit: 'celsius' };
              case 'list_devices':
                return [{ id: 'A', display_name: 'LCD A', family: 'galahad2-lcdv2' }];
              case 'list_sensors':
                return [];
              case 'get_lcd_state':
                return { source: 'Idle', last_error: null };
              case 'get_cooling_state': {
                const channel = {
                  requested: null,
                  applied_percent: 80,
                  motherboard: false,
                  fallback_active: false,
                  last_error: null,
                };
                return { fan: channel, pump: channel };
              }
              case 'list_profiles':
                return structuredClone(f.library);
              case 'edit_profile': {
                const edit = req.params.edit;
                if (edit.action !== 'save') throw new Error('unexpected edit');
                if (req.params.expected_revision !== f.library.revision)
                  throw new Error('revision conflict');
                f.calls.push(edit.profile.content.kind);
                f.library.profiles.push(structuredClone(edit.profile));
                f.library.revision++;
                const result = structuredClone(f.library);
                if (edit.profile.content.kind === 'pump')
                  return new Promise((resolve, reject) => {
                    f.finish = (fail) =>
                      fail ? reject(new Error('obsolete pump failure')) : resolve(result);
                  });
                return result;
              }
              default:
                f.calls.push(req.method);
                throw new Error(req.method);
            }
          },
        },
      });
    });
    await page.goto('/?view=display');
    await page.getByRole('button', { name: 'Охлаждение', exact: true }).click();
    await expect(page.getByText('Применённый PWM: 80%')).toBeVisible();
    await page.getByLabel('PWM, %', { exact: true }).fill('70');
    const open = async () => {
      await page.getByRole('button', { name: 'Сохранённые профили', exact: true }).click();
      await page.getByRole('button', { name: 'Обновить список профилей' }).click();
    };
    await open();
    await page.getByLabel('Имя профиля').fill('Submitted pump');
    await page.getByRole('button', { name: 'Сохранить черновик в слот' }).click();
    await expect(page.getByRole('button', { name: 'Обновить список профилей' })).toBeDisabled();
    await page.getByRole('combobox', { name: 'Канал', exact: true }).selectOption('fan');
    await page.getByLabel('PWM, %', { exact: true }).fill('35');
    await open();
    await page.getByLabel('Имя профиля').fill('New fan');
    await page.getByRole('button', { name: 'Сохранить черновик в слот' }).click();
    await expect(page.getByLabel('Слот профиля')).toContainText('New fan');
    await page.getByLabel('PWM, %', { exact: true }).fill('36');
    await page.evaluate((fail) => {
      (window as unknown as { switchedProfiles: Fixture }).switchedProfiles.finish(fail);
    }, fail);
    await expect(page.getByLabel('Слот профиля')).toContainText('New fan');
    await expect(page.getByLabel('Имя профиля')).toHaveValue('New fan');
    await expect(page.getByLabel('PWM, %', { exact: true })).toHaveValue('36');
    await expect(page.getByText('obsolete pump failure')).toHaveCount(0);
    await page.getByRole('button', { name: 'Загрузить профиль в редактор' }).click();
    await expect(page.getByLabel('PWM, %', { exact: true })).toHaveValue('35');
    await page.getByRole('combobox', { name: 'Канал', exact: true }).selectOption('pump');
    await open();
    await page.getByRole('button', { name: 'Загрузить профиль в редактор' }).click();
    await expect(page.getByLabel('PWM, %', { exact: true })).toHaveValue('70');
    expect(
      await page.evaluate(() => {
        const f = (window as unknown as { switchedProfiles: Fixture }).switchedProfiles;
        return { calls: f.calls, revision: f.library.revision };
      }),
    ).toEqual({ calls: ['pump', 'fan'], revision: 2 });
  });
}
