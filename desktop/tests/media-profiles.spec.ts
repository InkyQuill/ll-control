import { test, expect } from '@playwright/test';
test.beforeEach(async ({page})=>{
  await page.addInitScript(()=>{
    type Profile={slot:number;name:string;content:{kind:string;value:unknown}};
    const fixture={library:{version:1,revision:0,profiles:[] as Profile[]},writes:[] as {method:string;params:Record<string,unknown>}[]};
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async(command:string,req:{method:string;params:Record<string,unknown>})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status'||command==='media_thumbnail_cancel')return null;
      if(command==='media_thumbnail')throw new Error('No decoder in fixture');
      switch(req.method){
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'get_lcd_media':return null;
        case 'list_prepared_clips':return [{key:'a'.repeat(32),name:'Cinema',fps:24,frames:24,encoded_bytes:100}];
        case 'list_profiles':return structuredClone(fixture.library);
        case 'edit_profile': {
          fixture.writes.push({method:req.method,params:req.params});
          if(req.params.expected_revision!==fixture.library.revision)throw new Error('revision conflict');
          const edit=req.params.edit as {action:string;profile:Profile;slot:number;name:string};
          if(edit.action==='save')fixture.library.profiles=[edit.profile];
          if(edit.action==='rename')fixture.library.profiles[0].name=edit.name;
          if(edit.action==='delete')fixture.library.profiles=[];
          fixture.library.revision++;return structuredClone(fixture.library);
        }
        case 'set_lcd_media':fixture.writes.push({method:req.method,params:req.params});return {source:'DiagnosticClip'};
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
});
test('save and load media slot preserve theme draft and require explicit Apply',async({page})=>{
  await page.getByLabel('Название',{exact:true}).fill('Мой черновик');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('a'.repeat(32));
  await page.getByText('Сохранённые профили',{exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await page.getByLabel('Слот профиля').selectOption('7');
  await page.getByLabel('Имя профиля').fill('Cinema slot');
  await page.getByRole('button',{name:'Сохранить выбранный ролик в слот',exact:true}).click();
  await page.getByRole('button',{name:'Загрузить профиль в редактор',exact:true}).click();
  await expect(page.getByRole('button',{name:'Применить ролик из профиля',exact:true})).toBeVisible();
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Мой черновик');
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:unknown[]}}).fixture.writes)).toEqual([{method:'edit_profile',params:{expected_revision:0,edit:{action:'save',profile:{slot:7,name:'Cinema slot',content:{kind:'screen_media',value:'a'.repeat(32)}}}}}]);
  await page.getByRole('button',{name:'Применить ролик из профиля',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{writes:unknown[]}}).fixture.writes.length)).toBe(2);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:unknown[]}}).fixture.writes[1])).toEqual({method:'set_lcd_media',params:{dev:'A',key:'a'.repeat(32)}});
});
test('restored media slot uses screen rename and delete controls',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{library:unknown}}).fixture.library={version:1,revision:4,profiles:[{slot:0,name:'Restored video',content:{kind:'screen_media',value:'a'.repeat(32)}}]};});
  await page.getByText('Сохранённые профили',{exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await expect(page.getByLabel('Слот профиля').locator('option:checked')).toHaveText('1 · Restored video');
  await page.getByLabel('Имя профиля').fill('Renamed video');
  await page.getByRole('button',{name:'Переименовать профиль',exact:true}).click();
  await expect(page.getByLabel('Слот профиля').locator('option:checked')).toHaveText('1 · Renamed video');
  await page.getByRole('button',{name:'Удалить профиль',exact:true}).click();
  await expect(page.getByRole('button',{name:'Загрузить профиль в редактор',exact:true})).toBeDisabled();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:{params:{edit:{group:string}}}[]}}).fixture.writes.map(w=>w.params.edit.group))).toEqual(['screen','screen']);
});
