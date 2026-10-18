import {openEditorSection} from './editor-controls';
import { test, expect } from '@playwright/test';
import type { ProfileEdit, ProfileLibrary } from '../src/lib/api';

interface Fixture {
  online: boolean;
  held: boolean;
  writes: string[];
  library: ProfileLibrary;
  finish: (fail: boolean) => void;
}
const actions = {
  save: 'Заменить профиль черновиком', rename: 'Переименовать профиль',
  delete: 'Удалить профиль', reset: 'Сбросить слот к базовым настройкам',
} as const;
for (const action of Object.keys(actions) as (keyof typeof actions)[]) {
  for (const outcome of ['rejected', 'rejected-disconnect', 'lost', 'late-success', 'late-error'] as const) {
    test(`${action}: ${outcome} requires readback and preserves the editor`, async ({ page }) => {
      await page.addInitScript(rejectBeforeCommit => {
        const f: Fixture = {
          online: true, held: true, writes: [], finish: () => {},
          library: { version: 1, revision: 1, profiles: [{ slot: 0, name: 'Original slot', content: {
            kind: 'screen', value: { version: 1, name: 'Original theme', background: [0, 0, 0], layers: [] },
          } }] },
        };
        Object.assign(window, { interruptedProfiles: f, __TAURI_INTERNALS__: {
          invoke: async (_: string, req: { method: string; params: { edit: ProfileEdit; expected_revision: number } }) => {
        if (_ === 'desktop_set_language') return null;
            if (!f.online) throw new Error('offline');
            switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
              case 'list_devices': return [{ id: 'A', display_name: 'LCD A', family:'galahad2-lcdv2' }];
              case 'list_sensors': return [];
              case 'get_lcd_state': return { source: 'Idle', last_error: null };
              case 'list_profiles': return structuredClone(f.library);
              case 'edit_profile': {
                if (req.params.expected_revision !== f.library.revision) throw new Error('revision conflict');
                const edit = req.params.edit;
                f.writes.push(edit.action);
                if (rejectBeforeCommit) return new Promise((_resolve, reject) => {
                  f.finish = () => reject(new Error('rejected before commit'));
                });
                if (edit.action === 'save') f.library.profiles = [structuredClone(edit.profile)];
                if (edit.action === 'rename') f.library.profiles[0].name = edit.name;
                if (edit.action === 'delete') f.library.profiles = [];
                if (edit.action === 'reset') f.library.profiles = [{ slot: 0, name: 'Screen 1', content: {
                  kind: 'screen', value: { version: 1, name: 'Reset theme', background: [0, 0, 0], layers: [] },
                } }];
                f.library.revision++;
                const result = structuredClone(f.library);
                if (f.held) return new Promise((resolve, reject) => {
                  f.finish = fail => fail ? reject(new Error('ambiguous reply')) : resolve(result);
                });
                return result;
              }
              default: f.writes.push(req.method); throw new Error(req.method);
            }
          },
        } });
      }, outcome.startsWith('rejected'));
      await page.goto('/?view=display');
      await expect(page.getByText('Daemon подключён')).toBeVisible();
      const title = page.getByLabel('Название', { exact: true });
      await title.fill('Submitted theme');
      await page.getByRole('button',{name:'Сохранённые профили',exact:true}).click();
      const refresh = page.getByRole('button', { name: 'Обновить список профилей' });
      await refresh.click();
      await page.getByLabel('Имя профиля').fill('Submitted slot');
      await page.getByRole('button', { name: actions[action] }).click();
      await expect(refresh).toBeDisabled();
      await title.fill('New unsent draft');
      if (outcome.startsWith('late-') || outcome === 'rejected-disconnect') {
        await page.evaluate(() => { (window as unknown as { interruptedProfiles: Fixture }).interruptedProfiles.online = false; });
        await expect(page.getByText('Нет связи с daemon')).toBeVisible();
        await page.evaluate(() => { (window as unknown as { interruptedProfiles: Fixture }).interruptedProfiles.online = true; });
        await expect(page.getByText('Daemon подключён')).toBeVisible();
      }
      await page.evaluate(fail => { (window as unknown as { interruptedProfiles: Fixture }).interruptedProfiles.finish(fail); }, outcome !== 'late-success');
      await expect(refresh).toBeEnabled();
      await expect(title).toHaveValue('New unsent draft');
      if (!outcome.startsWith('late-') && outcome !== 'rejected-disconnect') {
        await expect(page.getByLabel('Имя профиля')).toHaveValue('Submitted slot');
        await expect(page.getByRole('button', { name: actions[action] })).toBeDisabled();
        await expect(page.getByRole('button', { name: 'Загрузить профиль в редактор' })).toBeDisabled();
      } else await expect(page.getByLabel('Слот профиля')).toHaveCount(0);
      await expect(page.getByText('Библиотека сохранена.', { exact: false })).toHaveCount(0);
      if (!outcome.startsWith('late-') && outcome !== 'rejected-disconnect') await expect(page.getByRole('alert')).toContainText('Обновите список');
      else await expect(page.getByText('ambiguous reply', { exact: false })).toHaveCount(0);
      if (outcome === 'rejected-disconnect') await expect(page.getByRole('dialog',{name:/^(Saved profiles|Сохранённые профили)$/,exact:true}).locator('[role=alert]')).toHaveCount(0);
      await refresh.click();
      await expect(page.getByLabel('Слот профиля')).toContainText(outcome.startsWith('rejected') ? 'Original slot' : action === 'delete' ? 'Пусто' : action === 'reset' ? 'Screen 1' : 'Submitted slot');
      await expect(title).toHaveValue('New unsent draft');
      expect(await page.evaluate(() => {
        const f = (window as unknown as { interruptedProfiles: Fixture }).interruptedProfiles;
        return { writes: f.writes, revision: f.library.revision };
      })).toEqual({ writes: [action], revision: outcome.startsWith('rejected') ? 1 : 2 });
      if (outcome.startsWith('rejected') || action !== 'delete') {
        await page.getByRole('button', { name: 'Загрузить профиль в редактор' }).click();
        await expect(title).toHaveValue(outcome.startsWith('rejected') ? 'Original theme' : action === 'save' ? 'Submitted theme' : action === 'reset' ? 'Reset theme' : 'Original theme');
      }
    });
  }
}
