import {test,expect,type Page} from '@playwright/test';
async function setup(page:Page, failure = '') {
  await page.addInitScript(({failure})=>{
    const old='a'.repeat(32), fresh='b'.repeat(32), builtin='builtin-midnight-v1';
    const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII='),c=>c.charCodeAt(0));
    const fixture={online:true,release:null as null|(()=>void),calls:[] as {command:string;args:Record<string,unknown>}[],imported:false,
      collections:{version:1,revision:1,images:[old,builtin],backgrounds:[{kind:'image',key:old}],watermarks:[old]}};
    Object.assign(window,{galleryEditor:fixture,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>)=>{
        if (command === 'desktop_set_language') return null;
      fixture.calls.push({command,args});
      if(command==='image_editor') return {width:480,height:480,png};
      if(command==='import_image') {fixture.imported=true;return fresh;}
      const params=args.params as Record<string,unknown>;
      switch(args.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':if(!fixture.online)throw new Error('offline');return [{id:'lcd:A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'list_named_theme_assets':return [{key:old,name:'Исходник',revision:1,is_default:false},{key:builtin,name:'Default',revision:0,is_default:true},...(fixture.imported?[{key:fresh,name:'Результат',revision:1,is_default:false}]:[])];
        case 'get_screen_collections':return {collections:fixture.collections,persistence:{pending:false,saved:true,last_error:null}};
        case 'thumbnail_theme_asset':return png;
        case 'read_theme_asset_for_edit':if(failure==='read')throw new Error('missing source');return png;
        case 'set_screen_collections':
          if(failure.startsWith('late-'))await new Promise<void>(resolve=>fixture.release=resolve);
          if(failure==='late-error')throw new Error('Obsolete gallery failure');
          if(failure==='conflict')throw new Error('revision changed');
          if(params.expected_revision!==fixture.collections.revision)throw new Error('stale revision');
          fixture.collections=params.collections as typeof fixture.collections;
          if(failure==='reply')throw new Error('lost reply');
          return fixture.collections;
        default:return null;
      }
    }}});
  },{failure});
  await page.goto('/?view=display');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
}
for(const context of ['image','background','watermark']) test(`gallery edit replaces ${context} membership and retains other contexts/files`,async({page})=>{
  await setup(page);
  await page.getByLabel('Назначение изображения').selectOption(context);
  await page.getByRole('button',{name:'Редактировать Исходник…',exact:true}).click();
  await expect(page.getByLabel('Обрезать квадрат')).not.toBeChecked();
  await page.getByRole('button',{name:'Повернуть вправо',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect(page.getByText('Запись коллекции заменена.',{exact:false})).toBeVisible();
  await expect(page.locator('summary').filter({hasText:'Библиотека изображений'})).toBeFocused();
  const result=await page.evaluate(()=>(window as unknown as {galleryEditor:{collections:{images:string[];backgrounds:{key:string}[];watermarks:string[]};calls:{command:string;args:{method?:string}}[]}}).galleryEditor);
  expect(result.collections.images).toContain((context==='image'?'b':'a').repeat(32));
  expect(result.collections.backgrounds[0].key).toBe((context==='background'?'b':'a').repeat(32));
  expect(result.collections.watermarks[0]).toBe((context==='watermark'?'b':'a').repeat(32));
  expect(result.calls.filter(c=>c.command==='import_image')).toHaveLength(1);
  expect(result.calls.some(c=>['delete_theme_asset','set_lcd_theme'].includes(c.args.method??''))).toBe(false);
  await page.getByLabel('Показать все файлы изображений для управления').check();
  await expect(page.getByRole('button',{name:'Переименовать Исходник',exact:true})).toBeVisible();
});
for(const failure of ['conflict','reply']) test(`gallery replacement ${failure} requires readback without repeating publication`,async({page})=>{
  await setup(page,failure);
  await page.getByRole('button',{name:'Редактировать Исходник…',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect(page.getByRole('alert')).toContainText('Замена не подтверждена');
  const calls=await page.evaluate(()=>(window as unknown as {galleryEditor:{calls:{command:string;args:{method?:string}}[]}}).galleryEditor.calls);
  expect(calls.filter(c=>c.command==='import_image')).toHaveLength(1);
  expect(calls.filter(c=>c.args.method==='set_screen_collections')).toHaveLength(1);
});
test('builtins cannot open editor and failed source read never publishes',async({page})=>{
  await setup(page,'read');
  await expect(page.getByRole('button',{name:'Редактировать Default…',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Редактировать Исходник…',exact:true}).click();
  await expect(page.getByRole('region',{name:'Редактор изображения',exact:true}).getByRole('alert')).toContainText('missing source');
  await page.getByRole('button',{name:'Закрыть редактор изображения'}).click();
  await expect(page.getByRole('button',{name:'Редактировать Исходник…',exact:true})).toBeFocused();
  const calls=await page.evaluate(()=>(window as unknown as {galleryEditor:{calls:{command:string}[]}}).galleryEditor.calls);
  expect(calls.some(c=>c.command==='image_editor'||c.command==='import_image')).toBe(false);
});

test('context switch closes prepared editor for a shared asset',async({page})=>{
  await setup(page);
  await page.getByRole('button',{name:'Редактировать Исходник…',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await expect(page.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeEnabled();
  await page.getByLabel('Назначение изображения').selectOption('watermark');
  await expect(page.getByRole('region',{name:'Редактор изображения',exact:true})).toHaveCount(0);
  const calls=await page.evaluate(()=>(window as unknown as {galleryEditor:{calls:{command:string}[]}}).galleryEditor.calls);
  expect(calls.some(c=>c.command==='import_image')).toBe(false);
});

for(const outcome of ['late-error','late-success']) test(`gallery ${outcome} after reconnect cannot overwrite the new session messages`,async({page})=>{
  await setup(page,outcome);
  await page.getByRole('button',{name:'Редактировать Исходник…',exact:true}).click();await page.getByRole('button',{name:'Подготовить preview изображения'}).click();await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect.poll(()=>page.evaluate(()=>!!(window as unknown as {galleryEditor:{release:null|(()=>void)}}).galleryEditor.release)).toBe(true);
  await page.evaluate(()=>(window as unknown as {galleryEditor:{online:boolean}}).galleryEditor.online=false);await expect(page.getByText('Нет связи с daemon',{exact:true})).toBeVisible();
  await page.evaluate(()=>(window as unknown as {galleryEditor:{online:boolean}}).galleryEditor.online=true);await page.getByRole('button',{name:'Повторить подключение',exact:true}).click();await expect(page.getByText('Daemon подключён',{exact:true})).toBeVisible();
  await page.evaluate(()=>(window as unknown as {galleryEditor:{release:()=>void}}).galleryEditor.release());
  await expect(page.getByLabel('Устройство',{exact:true})).toBeEnabled();await expect(page.locator('main > .error[role=alert]')).toHaveCount(0);
  await expect(page.getByRole('status').filter({hasText:'Запись коллекции заменена'})).toHaveCount(0);
  const calls=await page.evaluate(()=>(window as unknown as {galleryEditor:{calls:{command:string;args:{method?:string}}[]}}).galleryEditor.calls);
  expect(calls.filter(c=>c.command==='import_image')).toHaveLength(1);expect(calls.filter(c=>c.args.method==='set_screen_collections')).toHaveLength(1);expect(calls.some(c=>c.args.method==='set_lcd_theme')).toBe(false);
});
