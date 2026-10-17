import { test, expect } from '@playwright/test';
declare global { interface Window { imageDelete: { keys:string[]; writes:unknown[]; lose:boolean; hold:boolean; release:(()=>void)|null } } }
test.beforeEach(async({page})=>{
  await page.addInitScript(()=>{
    const f={keys:['a'.repeat(32),'b'.repeat(32)],writes:[] as unknown[],lose:false,hold:false,release:null as (()=>void)|null};
    const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0));
    Object.assign(window,{imageDelete:f,__TAURI_INTERNALS__:{invoke:async(command:string,r:{method:string;params:Record<string,unknown>})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status')return null;
      switch(r.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'list_named_theme_assets':return f.keys.map(key=>({key,name:key[0]==='a'?'Garden':'Logo',revision:1,is_default:key.startsWith("builtin-")}));
        case 'thumbnail_theme_asset':return png;
        case 'delete_theme_asset':
          f.writes.push(structuredClone(r.params));
          if(f.hold)await new Promise<void>(resolve=>{f.release=resolve;});
          f.keys=f.keys.filter(key=>key!==r.params.key);
          if(f.lose)throw new Error('reply lost');
          return null;
        default:throw new Error(r.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
});
test('confirmation deletes only selected image, restores focus and protects draft images',async({page})=>{
  await page.getByRole('button',{name:'Добавить в тему bbbbbbbb',exact:true}).click();
  await expect(page.getByRole('button',{name:'Удалить Logo…',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Удалить Garden…',exact:true}).click();
  await page.getByRole('button',{name:'Отмена удаления изображения',exact:true}).click();
  expect(await page.evaluate(()=>window.imageDelete.writes)).toEqual([]);
  await page.getByRole('button',{name:'Удалить Garden…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить удаление изображения',exact:true}).click();
  await expect(page.getByRole('button',{name:'Удалить Garden…',exact:true})).toHaveCount(0);
  await expect(page.locator('summary').filter({hasText:'Библиотека изображений'})).toBeFocused();
  expect(await page.evaluate(()=>window.imageDelete.writes)).toEqual([{key:'a'.repeat(32),expected_revision:1,keep:['b'.repeat(32)]}]);
});
test('lost reply reads back actual deletion without retry',async({page})=>{
  await page.evaluate(()=>{window.imageDelete.lose=true;});
  await page.getByRole('button',{name:'Удалить Garden…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить удаление изображения',exact:true}).click();
  await expect(page.getByText(/Удаление не подтверждено: reply lost/)).toBeVisible();
  await expect(page.getByRole('button',{name:'Удалить Garden…',exact:true})).toHaveCount(0);
  expect(await page.evaluate(()=>window.imageDelete.writes)).toHaveLength(1);
});
test('close and reopen during pending deletion still refreshes after settlement',async({page})=>{
  await page.evaluate(()=>{window.imageDelete.hold=true;});
  await page.getByRole('button',{name:'Удалить Garden…',exact:true}).click();
  await page.getByRole('button',{name:'Подтвердить удаление изображения',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>!!window.imageDelete.release)).toBe(true);
  const summary=page.getByText('Библиотека изображений',{exact:true});
  await summary.click();await summary.click();
  await expect(page.getByRole('button',{name:'Удалить Garden…',exact:true})).toBeDisabled();
  await page.evaluate(()=>window.imageDelete.release!());
  await expect(page.getByRole('button',{name:'Удалить Garden…',exact:true})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Удалить Logo…',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>window.imageDelete.writes)).toHaveLength(1);
});

test('builtin image remains selectable but cannot be renamed or deleted',async({page})=>{
  await page.evaluate(()=>{window.imageDelete.keys=['builtin-orbit-mark-v1'];});
  await page.getByRole('button',{name:'Обновить изображения',exact:true}).click();
  await expect(page.getByText('Встроенное изображение',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Удалить Logo…',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Переименовать Logo',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Добавить в тему Logo',exact:true}).click();
  await expect(page.getByText('Изображение используется в черновике темы.',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=>window.imageDelete.writes)).toEqual([]);
});
