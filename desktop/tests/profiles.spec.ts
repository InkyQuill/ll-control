import {openEditorSection} from './editor-controls';
import { test, expect } from '@playwright/test';
test('screen profiles save and reload drafts, rename and delete without applying', async ({ page }) => {
  await page.addInitScript(() => {
    type Profile = { slot: number; name: string; content: { kind: string; value: unknown } };
    let library: { version: number; revision: number; profiles: Profile[] } = { version: 1, revision: 0, profiles: [] };
    const writes: string[] = [];
    let groupResetCount = 0, loseGroupReply = false;
    Object.assign(window, { profileWrites: writes, getGroupResetCount: () => groupResetCount, loseNextGroupReply: () => { loseGroupReply = true; }, advanceProfileRevision: () => { library.revision++; }, __TAURI_INTERNALS__: { invoke: async (_: string, request: { method: string; params: { expected_revision: number; edit: { action: string; profile: Profile; slot: number; group: string; name: string } } }) => {
        if (_ === 'desktop_set_language') return null;
      switch (request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
        case 'list_devices': return [{ id: 'test', display_name: 'LCDV2', family:'galahad2-lcdv2' }];
        case 'list_sensors': return [];
        case 'get_lcd_state': return { source:'Idle', last_error:null, requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'} };
        case 'list_profiles': return structuredClone(library);
        case 'edit_profile': {
          if (request.params.expected_revision !== library.revision) throw { message:'profile library changed' };
          const edit = request.params.edit;
          if (edit.action === 'save') library.profiles = [structuredClone(edit.profile)];
          if (edit.action === 'rename') library.profiles[0].name = edit.name;
          if (edit.action === 'delete') library.profiles = [];
          if (edit.action === 'reset') library.profiles = [{ slot:edit.slot,name:'Screen 1',content:{kind:'screen',value:{version:1,name:'Screen 1',background:[12,18,30],layers:[]}} }];
          if (edit.action === 'reset_group') {
            groupResetCount++;
            library.profiles = Array.from({ length:8 }, (_,slot) => ({slot,name:`Screen ${slot+1}`,content:{kind:'screen',value:{version:1,name:`Screen ${slot+1}`,background:[12,18,30],layers:[]}}}));
          }
          library.revision++;
          if (edit.action === 'reset_group' && loseGroupReply) { loseGroupReply = false; throw {message:'Reply lost after commit'}; }
          return structuredClone(library);
        }
        default: writes.push(request.method); throw { message:'Unexpected ' + request.method };
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByLabel('Название', { exact:true }).fill('Saved composition');
  await page.getByText('Сохранённые профили', { exact:true }).click();
  await page.getByRole('button', { name:'Обновить список профилей' }).click();
  await page.getByLabel('Имя профиля').fill('My slot');
  await page.getByRole('button', { name:'Сохранить черновик в слот' }).click();
  await expect(page.getByRole('status')).toContainText('Библиотека сохранена');
  await page.getByLabel('Название', { exact:true }).fill('Unsaved change');
  await page.getByRole('button', { name:'Загрузить профиль в редактор' }).click();
  await expect(page.getByLabel('Название', { exact:true })).toHaveValue('Saved composition');
  await page.getByLabel('Имя профиля').fill('Renamed');
  await page.getByRole('button', { name:'Переименовать профиль' }).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Renamed');
  await page.evaluate(() => (window as unknown as { advanceProfileRevision: () => void }).advanceProfileRevision());
  await page.getByLabel('Имя профиля').fill('Stale edit');
  await page.getByRole('button', { name:'Переименовать профиль' }).click();
  await expect(page.getByRole('alert')).toContainText('profile library changed');
  await page.getByRole('button', { name:'Обновить список профилей' }).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Renamed');
  await page.getByRole('button', { name:'Удалить профиль' }).click();
  await expect(page.getByRole('button', { name:'Загрузить профиль в редактор' })).toBeDisabled();
  await expect(page.getByLabel('Название', { exact:true })).toHaveValue('Saved composition');
  await page.getByRole('button', { name:'Сбросить слот к базовым настройкам' }).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Screen 1');
  await expect(page.getByLabel('Название', { exact:true })).toHaveValue('Saved composition');
  const resetGroup = page.getByRole('button', { name:'Сбросить все слоты группы…', exact:true });
  await resetGroup.click();
  await expect(page.getByRole('group', {name:'Подтверждение сброса группы'})).toContainText('всех 8 слотов экрана');
  await page.getByRole('button', {name:'Отмена сброса группы'}).click();
  await expect(resetGroup).toBeFocused();
  expect(await page.evaluate(() => (window as unknown as {getGroupResetCount:()=>number}).getGroupResetCount())).toBe(0);
  await resetGroup.click();
  await page.evaluate(() => (window as unknown as {loseNextGroupReply:()=>void}).loseNextGroupReply());
  await page.getByRole('button', {name:'Подтвердить сброс всех слотов'}).click();
  await expect(page.getByRole('alert')).toContainText('Reply lost after commit');
  await expect(resetGroup).toBeDisabled();
  await expect(page.getByRole('button', {name:'Обновить список профилей'})).toBeFocused();
  await expect(page.getByRole('group', {name:'Подтверждение сброса группы'})).toHaveCount(0);
  await page.getByRole('button', {name:'Обновить список профилей'}).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Screen 8');
  expect(await page.evaluate(() => (window as unknown as {getGroupResetCount:()=>number}).getGroupResetCount())).toBe(1);
  await expect(page.getByLabel('Название', {exact:true})).toHaveValue('Saved composition');
  expect(await page.evaluate(() => (window as unknown as { profileWrites: string[] }).profileWrites)).toEqual([]);
});

test('pump and fan profile loads remain separate local cooling drafts', async ({ page }) => {
  await page.addInitScript(() => {
    const writes: string[] = [];
    Object.assign(window, { profileWrites: writes, __TAURI_INTERNALS__: { invoke: async (_: string, request: { method: string }) => {
        if (_ === 'desktop_set_language') return null;
      const channel = { requested:null, applied_percent:80, motherboard:false, fallback_active:false, last_error:null };
      switch (request.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
        case 'list_devices': return [{ id:'test',display_name:'LCDV2', family:'galahad2-lcdv2' }];
        case 'list_sensors': return [];
        case 'get_lcd_state': return { source:'Idle',last_error:null,requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'} };
        case 'get_cooling_state': return { fan:channel,pump:channel };
        case 'list_profiles': return { version:1,revision:1,profiles:[
          { slot:0,name:'Fan quiet',content:{kind:'fan',value:{mode:'fixed',percent:35}} },
          { slot:0,name:'Pump quiet',content:{kind:'pump',value:{mode:'fixed_rpm',rpm:2400}} }
        ] };
        default: writes.push(request.method); throw { message:'Unexpected '+request.method };
      }
    } } });
  });
  await page.goto('/?view=display');
  await page.getByRole('button', { name:'Охлаждение', exact:true }).click();
  await expect(page.getByText('Applied: 80%')).toBeVisible();
  await page.getByText('Сохранённые профили', { exact:true }).click();
  await page.getByRole('button', { name:'Обновить список профилей' }).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Pump quiet');
  await page.getByRole('button', { name:'Загрузить профиль в редактор' }).click();
  await expect(page.getByRole('combobox', { name:'Режим', exact:true })).toHaveValue('fixed_rpm');
  await page.getByRole('combobox', { name:'Канал', exact:true }).selectOption('fan');
  await page.getByText('Сохранённые профили', { exact:true }).click();
  await page.getByRole('button', { name:'Обновить список профилей' }).click();
  await expect(page.getByLabel('Слот профиля')).toContainText('Fan quiet');
  await page.getByRole('button', { name:'Загрузить профиль в редактор' }).click();
  await expect(page.getByLabel('PWM, %', { exact:true })).toHaveValue('35');
  expect(await page.evaluate(() => (window as unknown as { profileWrites: string[] }).profileWrites)).toEqual([]);
});
