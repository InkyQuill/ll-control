import { test, expect } from '@playwright/test';

test.beforeEach(async ({page}) => {
  await page.addInitScript(() => {
    const fixture={keys:['a'.repeat(32),'b'.repeat(32)],active:null as string|null,writes:[] as string[],lost:false,blocked:false,builtin:false};
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async(command:string,req:{method:string;params:{key:string}})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status'||command==='media_thumbnail_cancel')return null;
      if(command==='media_thumbnail')throw new Error('No decoder in fixture');
      switch(req.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'get_lcd_media':return fixture.active;
        case 'list_prepared_clips':return fixture.keys.map(key=>({key,name:key[0]==='a'?'Волны':'Облака',fps:24,frames:24,encoded_bytes:100,is_default:fixture.builtin}));
        case 'delete_prepared_clip':
          fixture.writes.push(req.params.key);
          if(fixture.blocked)throw new Error('clip is referenced by saved settings');
          fixture.keys=fixture.keys.filter(key=>key!==req.params.key);
          if(fixture.lost)throw new Error('reply lost');
          return null;
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('a'.repeat(32));
});

test('deletion confirms the exact choice; cancel and changing selection never delete',async({page})=>{
  await page.getByRole('button',{name:'Удалить ролик…',exact:true}).click();
  await page.getByRole('button',{name:'Отмена удаления',exact:true}).click();
  await page.getByRole('button',{name:'Удалить ролик…',exact:true}).click();
  await page.getByLabel('Сохранённый ролик').selectOption('b'.repeat(32));
  await expect(page.getByRole('button',{name:'Подтвердить удаление ролика',exact:true})).toHaveCount(0);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:string[]}}).fixture.writes)).toEqual([]);
  await page.getByRole('button',{name:'Удалить ролик…',exact:true}).click();
  await expect(page.getByText('Удалить «Облака» из библиотеки? Исходный файл останется на диске.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Подтвердить удаление ролика',exact:true}).click();
  await expect(page.getByText('Ролик удалён.',{exact:true})).toBeVisible();
  await expect(page.getByLabel('Сохранённый ролик').locator('option')).toHaveCount(1);
  await expect(page.getByLabel('Сохранённый ролик')).toBeFocused();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:string[]}}).fixture.writes)).toEqual(['b'.repeat(32)]);
});

test('lost deletion reply refreshes library and never automatically retries',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{lost:boolean}}).fixture.lost=true;});
  await page.getByRole('button',{name:'Удалить ролик…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить удаление ролика',exact:true}).click();
  await expect(page.getByText(/Удаление не подтверждено: reply lost/)).toBeVisible();
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('b'.repeat(32));
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:string[]}}).fixture.writes)).toEqual(['a'.repeat(32)]);
});

test('offline-device reference rejection preserves the clip and permits explicit recovery',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{blocked:boolean}}).fixture.blocked=true;});
  await page.getByRole('button',{name:'Удалить ролик…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить удаление ролика',exact:true}).click();
  await expect(page.getByText(/clip is referenced by saved settings/)).toBeVisible();
  await expect(page.getByLabel('Сохранённый ролик').locator('option')).toHaveCount(2);
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true})).toBeEnabled();
});

test('builtin clip allows selection and theme background but rejects delete in UI',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{builtin:boolean}}).fixture.builtin=true;});
  await page.getByRole('button',{name:'Обновить ролики',exact:true}).click();
  await expect(page.getByText('Встроенный ролик нельзя удалить или редактировать.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Удалить ролик…',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Редактировать выбранный ролик',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Использовать фоном темы',exact:true}).click();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:string[]}}).fixture.writes)).toEqual([]);
  await expect(page.getByLabel('Тип фона')).toHaveValue('video');
});
