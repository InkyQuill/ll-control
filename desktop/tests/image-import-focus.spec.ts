import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    type Collections = {version:number;revision:number;images:string[];watermarks:string[];backgrounds:{kind:string;key:string}[]};
    const fixture = {collections:null as Collections|null,failMembership:false,writes:0, admitted: false, finish: (_fail: boolean) => {} };
    Object.assign(window, { imageImport: fixture, __TAURI_INTERNALS__: {
      invoke: async (command: string, args: { method: string; params:{collections:Collections;expected_revision:number} }) => {
        if (command === 'import_image') {
          fixture.admitted = true;
          return new Promise<string>((resolve, reject) => {
            fixture.finish = fail => {
              if (fail) reject({ message: 'Invalid image fixture' });
              else resolve('a'.repeat(32));
            };
          });
        }
        switch (args.method) {
          case 'get_screen_collections': return {collections:fixture.collections,persistence:{pending:false,saved:false,last_error:null}};
          case 'set_screen_collections': fixture.writes++; if(fixture.failMembership) throw new Error('Collection conflict'); fixture.collections=args.params.collections; return fixture.collections;
          case 'list_devices': return [{ id: 'lcd:test', display_name: 'LCDV2' }];
          case 'list_sensors': return [];
          case 'get_lcd_state': return { source: 'Idle', requested: {}, last_error: null };
          default: throw new Error(`Unexpected operation: ${args.method}`);
        }
      }
    } });
  });
  await page.goto('/');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
});

for (const outcome of ['success', 'error', 'move-focus', 'layer-limit', 'leave-page'] as const) {
  test(`image import ${outcome}: keyboard focus survives without stealing a new target`, async ({ page }) => {
    const input = page.locator('.file input[type=file]');
    const other = page.getByRole('button', { name: '+ Текст', exact: true });
    if (outcome === 'layer-limit') {
      while (await page.locator('.layer').count() < 31) await other.click();
    }
    await input.focus();
    await input.setInputFiles({ name: 'fixture.png', mimeType: 'image/png', buffer: Buffer.from('fixture') });
    await expect(input).toBeDisabled();
    await expect.poll(() => page.evaluate(() => (window as unknown as { imageImport: { admitted: boolean } }).imageImport.admitted)).toBe(true);
    if (outcome === 'move-focus') await other.focus();
    if (outcome === 'leave-page') await page.getByRole('button', { name: 'Настройки', exact: true }).click();
    await page.evaluate(fail => (window as unknown as { imageImport: { finish(fail: boolean): void } }).imageImport.finish(fail), outcome === 'error');
    if (outcome === 'leave-page') {
      await expect(page.getByRole('status').filter({hasText:'Изображение добавлено'})).toBeVisible();
      await expect(input).toHaveCount(0);
      await expect(page.getByRole('button', { name: 'Настройки', exact: true })).toBeFocused();
      return;
    }
    if (outcome === 'layer-limit') {
      await expect(page.locator('.layer')).toHaveCount(32);
      await expect(input).toBeDisabled();
    }
    else await expect(input).toBeEnabled();
    await expect(input).toHaveValue('');
    if (outcome === 'error') await expect(page.getByRole('alert')).toContainText('Invalid image fixture');
    else await expect(page.getByRole('status').filter({hasText:'Изображение добавлено'})).toBeVisible();
    const target = outcome === 'layer-limit' ? page.getByRole('heading', { name: '1. Изображение', exact: true }) : outcome === 'move-focus' ? other : input;
    await expect(target).toBeFocused();
    if (outcome === 'success' || outcome === 'error') {
      await page.keyboard.press('Tab');
      await expect(page.getByRole('combobox', { name: 'Привязать датчик', exact: true })).toBeFocused();
    }
  });
}

for (const fail of [false,true]) test(`import into saved image collection, membership failure=${fail}`,async({page})=>{
  await page.evaluate(fail=>{
    const f=(window as unknown as {imageImport:{collections:unknown;failMembership:boolean}}).imageImport;
    f.collections={version:1,revision:3,images:[],watermarks:[],backgrounds:[]}; f.failMembership=fail;
  },fail);
  const before=await page.locator('.layer').count();
  await page.locator('.file input[type=file]').setInputFiles({name:'fixture.png',mimeType:'image/png',buffer:Buffer.from('fixture')});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {imageImport:{admitted:boolean}}).imageImport.admitted)).toBe(true);
  await page.evaluate(()=>(window as unknown as {imageImport:{finish(fail:boolean):void}}).imageImport.finish(false));
  if(fail){
    await expect(page.getByRole('alert')).toContainText('Файл сохранён в каталоге');
    await expect(page.locator('.layer')).toHaveCount(before);
  }else{
    await expect(page.locator('.layer')).toHaveCount(before+1);
    expect(await page.evaluate(()=>(window as unknown as {imageImport:{collections:{revision:number;images:string[]}}}).imageImport.collections)).toMatchObject({revision:4,images:['a'.repeat(32)]});
  }
  expect(await page.evaluate(()=>(window as unknown as {imageImport:{writes:number}}).imageImport.writes)).toBe(1);
});
