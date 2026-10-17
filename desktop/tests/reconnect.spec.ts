import {loadControls} from './editor-controls';
import { test, expect } from '@playwright/test';

test('disconnect and missing device preserve the draft without redirecting Apply', async ({ page }) => {
  await page.addInitScript(() => {
    const fixture = { online: true, ids: ['A', 'B'], writes: [] as string[], polls: 0 };
    Object.assign(window, { fixture, __TAURI_INTERNALS__: { invoke: async (_: string, req: {method: string; params: {dev: string}}) => {
        if (_ === 'desktop_set_language') return null;
      if (!fixture.online) throw new Error('fixture disconnected');
      switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices': fixture.polls++; return fixture.ids.map(id => ({id, display_name: `LCD ${id}`, family:'galahad2-lcdv2'}));
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source: 'Idle'};
        case 'set_lcd_theme': fixture.writes.push(req.params.dev); return {source: 'Theme'};
        default: throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByLabel('Устройство', {exact: true})).toHaveValue('A');
  const connection = page.locator('#connection-status');
  await expect(connection).toHaveAttribute('aria-live', 'polite');
  await expect(connection).toHaveAttribute('aria-atomic', 'true');
  await page.evaluate(() => {
    const node = document.querySelector('#connection-status')!;
    const observation = { node, mutations: 0 };
    new MutationObserver(records => { observation.mutations += records.length; }).observe(node, { childList: true, subtree: true, characterData: true });
    Object.assign(window, { connectionNode: node, connectionObservation: observation });
  });
  await loadControls(page);
  await page.getByLabel('Название', {exact: true}).fill('My unsent draft');
  await page.evaluate(() => { (window as unknown as {fixture: {online: boolean}}).fixture.online = false; });
  await expect(page.getByText('Нет связи с daemon')).toBeVisible();
  await expect(page.getByRole('button', {name: 'Применить к LCD'})).toBeDisabled();
  await expect(page.getByRole('button', {name: 'Вернуть LocalUI'})).toBeDisabled();
  await page.evaluate(() => { Object.assign((window as unknown as {fixture: object}).fixture, {online: true, ids: ['B']}); });
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByLabel('Устройство', {exact: true})).toHaveValue('A');
  await expect(page.getByRole('button', {name: 'Применить к LCD'})).toBeDisabled();
  await expect(page.getByLabel('Название', {exact: true})).toHaveValue('My unsent draft');
  await page.evaluate(() => { (window as unknown as {fixture: {ids: string[]}}).fixture.ids = ['B', 'A']; });
  await expect(page.getByRole('button', {name: 'Применить к LCD'})).toBeEnabled();
  await page.getByRole('button', {name: 'Применить к LCD'}).click();
  expect(await page.evaluate(() => (window as unknown as {fixture: {writes: string[]}}).fixture.writes)).toEqual(['A']);
  expect(await page.evaluate(() => (window as unknown as { connectionNode: Element }).connectionNode === document.querySelector('#connection-status'))).toBe(true);
  const before = await page.evaluate(() => {
    const w = window as unknown as { fixture: { polls: number }; connectionObservation: { mutations: number } };
    return { polls: w.fixture.polls, mutations: w.connectionObservation.mutations };
  });
  expect(before.mutations).toBeGreaterThan(0);
  await expect.poll(() => page.evaluate(() => (window as unknown as { fixture: { polls: number } }).fixture.polls)).toBeGreaterThanOrEqual(before.polls + 2);
  expect(await page.evaluate(() => (window as unknown as { connectionObservation: { mutations: number } }).connectionObservation.mutations)).toBe(before.mutations);

});

test('late active-theme load cannot replace edits made while waiting', async ({page}) => {
  await page.addInitScript(() => {
    Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (_: string, req: {method: string}) => {
        if (_ === 'desktop_set_language') return null;
      switch(req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices': return [{id: 'A', display_name: 'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source: 'Theme'};
        case 'get_lcd_theme': return new Promise(resolve => Object.assign(window, {finishLoad: () => resolve({version:1,name:'Old daemon theme',background:[0,0,0],layers:[]})}));
        default: throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await loadControls(page);await page.getByRole('button', {name: 'Загрузить активную тему'}).click();
  await page.getByLabel('Название', {exact: true}).fill('New local edits');
  await page.evaluate(() => (window as unknown as {finishLoad: () => void}).finishLoad());
  await expect(page.getByText('Черновик изменился.', {exact: false})).toBeVisible();
  await expect(page.getByLabel('Название', {exact: true})).toHaveValue('New local edits');
});

test('late LCD status from the previous device is discarded', async ({page}) => {
  await page.addInitScript(() => {
    Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (_: string, req: {method: string; params: {dev: string}}) => {
        if (_ === 'desktop_set_language') return null;
      switch(req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices': return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'},{id:'B',display_name:'LCD B', family:'galahad2-lcdv2'}];
        case 'list_sensors': return [];
        case 'get_lcd_state':
          if(req.params.dev === 'A') return new Promise(resolve => Object.assign(window, {finishState: () => resolve({source:'Theme',last_error:'A error'})}));
          return {source:'Idle',last_error:null};
        default: throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByLabel('Устройство', {exact: true})).toHaveValue('A');
  await page.getByLabel('Устройство', {exact: true}).selectOption('B');
  await page.evaluate(() => (window as unknown as {finishState: () => void}).finishState());
  await expect(page.getByText('Источник: Нет программного потока')).toBeVisible();
  await expect(page.getByText('A error')).toHaveCount(0);
});
