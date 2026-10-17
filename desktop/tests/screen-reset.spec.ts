import { test, expect, type Page } from '@playwright/test';

async function setup(page: Page) {
  await page.addInitScript(() => {
    let generation = 0, revision = 0, collection = 0;
    let receipt: null | {operation_id:string;profile_revision:number;collection_revision:number} = null;
    let lcdError: string | null = null, retry = false, readError = false;
    let lose = false, reject = false, hold = false, late = false;
    let release: (() => void) | undefined;
    const writes: {method:string;params:Record<string,unknown>}[] = [];
    const lcd = () => ({generation,source:receipt ? 'Theme':'Idle',last_error:lcdError,requested:{brightness_percent:50,rotation_degrees:0,mode:'Application'}});
    Object.assign(window, {
      resetWrites: writes,
      resetFault: (kind: string) => { if(kind==='lost')lose=true; if(kind==='reject')reject=true; if(kind==='hold')hold=true; if(kind==='late')late=true; if(kind==='lcd'){lcdError='USB failed';retry=true;} if(kind==='read')readError=true; if(kind==='recover')readError=false; },
      releaseReset: () => release?.(),
      __TAURI_INTERNALS__: {invoke: async (_:string, r:{method:string;params:Record<string,unknown>}) => {
        if (_ === 'desktop_set_language') return null;
        switch(r.method) {
          case 'list_devices': return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'},{id:'other',display_name:'Other LCD', family:'galahad2-lcdv2'}];
          case 'list_sensors': return [];
          case 'get_lcd_state': return lcd();
          case 'list_profiles': return {version:1,revision,profiles:[]};
          case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:true,last_error:null}};
          case 'get_screen_reset': if(readError)throw {message:'offline'}; return {receipt,persistence:{pending:false,saved:true,last_error:null},reapply_available:retry,lcd:lcd()};
          case 'reset_screen':
            writes.push(structuredClone(r));
            if(late){late=false;release=()=>{revision++;collection++;generation++;receipt={operation_id:String(r.params.operation_id),profile_revision:revision,collection_revision:collection};};throw {message:'request still pending'};}
            if(reject){reject=false;throw {message:'revision changed',code:-32002};}
            if(hold){hold=false;await new Promise<void>(resolve=>release=resolve);}
            if(receipt?.operation_id !== r.params.operation_id){revision++;collection++;generation++;receipt={operation_id:String(r.params.operation_id),profile_revision:revision,collection_revision:collection};}
            if(lose){lose=false;readError=true;throw {message:'reply lost'};}
            return {receipt,repeated:false,lcd:lcd()};
          case 'reapply_screen_reset': writes.push(structuredClone(r));generation++;lcdError=null;retry=false;return lcd();
          case 'list_named_theme_assets': case 'list_prepared_clips': case 'list_theme_fonts': return [];
          case 'get_lcd_media': return null;
          default: return null;
        }
      }}
    });
  });
  await page.goto('/?view=display');
  await page.getByText('Сброс экрана LCD',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Сбросить экран…',exact:true})).toBeEnabled();
}
async function confirm(page:Page) {
  await page.getByRole('button',{name:'Сбросить экран…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить сброс экрана',exact:true}).click();
}
async function fault(page:Page, kind:string) {
  await page.evaluate(kind => (window as unknown as {resetFault:(kind:string)=>void}).resetFault(kind),kind);
}
async function writes(page:Page) {
  return page.evaluate(() => (window as unknown as {resetWrites:{method:string;params:Record<string,unknown>}[]}).resetWrites);
}

test('reset scope, cancel, durable receipt and explicit LCD reapply', async ({page}) => {
  await setup(page);
  await expect(page.getByText(/все восемь общих Screen-слотов/)).toBeVisible();
  await page.getByRole('button',{name:'Сбросить экран…',exact:true}).click();
  await page.getByRole('button',{name:'Отмена сброса'}).click();
  expect(await writes(page)).toEqual([]);
  await fault(page,'lcd');
  await confirm(page);
  await expect(page.getByText('Сброс сохранён. Передача на LCD завершилась ошибкой.')).toBeVisible();
  await page.getByRole('button',{name:'Повторно применить к LCD'}).click();
  await expect(page.getByRole('button',{name:'Повторно применить к LCD'})).toHaveCount(0);
  expect((await writes(page)).map(x=>x.method)).toEqual(['reset_screen','reapply_screen_reset']);
  await expect(page.getByRole('button',{name:'Проверить результат сброса'})).toBeFocused();
});

test('lost reply blocks new reset until receipt readback, without automatic retry', async ({page}) => {
  await setup(page);await fault(page,'lost');await confirm(page);
  await expect(page.getByText(/Не удалось проверить сброс/)).toBeVisible();
  await expect(page.getByRole('button',{name:'Повторить запрос сброса'})).toBeDisabled();
  expect((await writes(page)).length).toBe(1);
  await fault(page,'recover');
  await page.getByRole('button',{name:'Проверить результат сброса'}).click();
  await expect(page.getByText('Сброс сохранён, базовая тема отправлена на LCD.')).toBeVisible();
  expect((await writes(page)).length).toBe(1);
});

test('pending reset survives tab changes and locks Apply and device selection', async ({page}) => {
  await setup(page);await fault(page,'hold');await confirm(page);
  await expect(page.getByRole('button',{name:'Применить к LCD',exact:true})).toBeDisabled();
  await expect(page.getByLabel('Устройство',{exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Датчики',exact:true}).click();
  await page.getByRole('button',{name:'Экран помпы',exact:true}).click();
  await expect(page.getByRole('button',{name:'Повторить запрос сброса'})).toBeDisabled();
  await page.evaluate(() => (window as unknown as {releaseReset:()=>void}).releaseReset());
  await expect(page.getByRole('button',{name:'Сбросить экран…',exact:true})).toBeEnabled();
  expect((await writes(page)).length).toBe(1);
});

test('definitive revision rejection requires a fresh confirmation', async ({page}) => {
  await setup(page);await fault(page,'reject');await confirm(page);
  await expect(page.getByText(/Сброс отклонён: revision changed/)).toBeVisible();
  await expect(page.getByRole('button',{name:'Повторить запрос сброса'})).toHaveCount(0);
  await confirm(page);
  const calls = await writes(page);
  expect(calls).toHaveLength(2);
  expect(calls[0].params.operation_id).not.toBe(calls[1].params.operation_id);
});


test('late receipt confirmation invalidates a profile list reloaded before commit', async ({page}) => {
  await setup(page);await fault(page,'late');await confirm(page);
  await expect(page.getByRole('button',{name:'Повторить запрос сброса'})).toBeEnabled();
  await page.getByText('Сохранённые профили',{exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей'}).click();
  await expect(page.getByRole('combobox',{name:'Слот профиля',exact:true})).toBeVisible();
  await page.evaluate(() => (window as unknown as {releaseReset:()=>void}).releaseReset());
  await page.getByRole('button',{name:'Проверить результат сброса'}).click();
  await expect(page.getByText('Сброс сохранён, базовая тема отправлена на LCD.')).toBeVisible();
  await expect(page.getByText(/Ответ не подтверждён:/)).toHaveCount(0);
  await expect(page.getByRole('combobox',{name:'Слот профиля',exact:true})).toHaveCount(0);
  expect((await writes(page)).length).toBe(1);
});

test('explicit retry after an ambiguous response reuses the original operation', async ({page}) => {
  await setup(page);await fault(page,'late');await confirm(page);
  await page.getByRole('button',{name:'Повторить запрос сброса'}).click();
  await expect(page.getByText('Сброс сохранён, базовая тема отправлена на LCD.')).toBeVisible();
  const calls=await writes(page);
  expect(calls).toHaveLength(2);
  expect(calls[1].params).toEqual(calls[0].params);
});
