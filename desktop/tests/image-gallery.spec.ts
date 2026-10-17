import { test, expect } from '@playwright/test';
test.beforeEach(async ({page}) => {
  await page.addInitScript(() => {
    const fixture={scoped:false,collectionError:false,names:{} as Record<string,{name:string;revision:number}>, loseRename:false, calls:[] as {method:string;params:Record<string,unknown>}[], created:[] as string[],revoked:[] as string[],hold:false,release:null as null|(()=>void)};
    const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);
    URL.createObjectURL=(blob:Blob)=>{const url=create(blob);if(blob.type==='image/png')fixture.created.push(url);return url;};
    URL.revokeObjectURL=(url:string)=>{fixture.revoked.push(url);revoke(url);};
    const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0));
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async(command:string,args:{method:string;params:Record<string,unknown>})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status')return null;
      fixture.calls.push(args);
      switch(args.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_screen_collections': if(fixture.collectionError) throw new Error('collection read failed'); return {collections:fixture.scoped ? {version:1,revision:1,images:['0'.repeat(32)],backgrounds:[{kind:'image',key:'2'.repeat(32)}],watermarks:['3'.repeat(32)]}:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'list_named_theme_assets':return Array.from({length:10},(_,i)=>{const key=i.toString(16).repeat(32);return {key,name:fixture.names[key]?.name ?? null,revision:fixture.names[key]?.revision ?? 0};});
        case 'rename_theme_asset': { const key=String(args.params.key);fixture.names[key]={name:String(args.params.name),revision:1};if(fixture.loseRename)throw new Error('reply lost');return {key,...fixture.names[key]}; }
        case 'thumbnail_theme_asset':
          if(fixture.hold){fixture.hold=false;await new Promise<void>(resolve=>{fixture.release=resolve;});}
          if(args.params.key==='1'.repeat(32))throw new Error('corrupt image');
          return png;
        case 'set_lcd_theme':return {source:'Theme'};
        case 'preview_lcd_theme':return png;
        default:throw new Error(args.method);
      }
    }}});
  });
  await page.goto('/?view=display');
});
test('saved images page, isolate corrupt previews and add to draft until Apply',async({page})=>{
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  const library=page.locator('details').filter({has:page.locator('summary',{hasText:'Библиотека изображений'})});
  await expect(library.getByRole('img')).toHaveCount(7);
  await expect(library.getByRole('button',{name:'Добавить в тему 11111111'})).toBeDisabled();
  await library.getByRole('button',{name:'Следующие изображения'}).click();
  await expect(library.getByRole('img')).toHaveCount(2);
  await library.getByRole('button',{name:'Добавить в тему 99999999'}).click();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{method:string}[]}}).fixture.calls.some(c=>c.method==='set_lcd_theme'))).toBe(false);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{calls:{method:string;params:{theme?:{layers:{kind:string;asset?:string}[]}}}[]}}).fixture.calls.find(c=>c.method==='set_lcd_theme')?.params.theme?.layers.some(l=>l.kind==='image'&&l.asset==='9'.repeat(32)))).toBe(true);
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>{const f=(window as unknown as {fixture:{created:string[];revoked:string[]}}).fixture;return f.created.every(url=>f.revoked.includes(url));})).toBe(true);
});
test('close discards late image and stops loading the remaining page',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{hold:boolean}}).fixture.hold=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{release:unknown}}).fixture.release!==null)).toBe(true);
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Добавить в тему 00000000',includeHidden:true})).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {fixture:{release:()=>void}}).fixture.release();});
  const state=await page.evaluate(()=>(window as unknown as {fixture:{calls:{method:string}[];created:string[]}}).fixture);
  expect(state.created).toEqual([]);
  expect(state.calls.filter(c=>c.method==='thumbnail_theme_asset')).toHaveLength(1);
});

for (const lost of [false,true]) test(`image rename ${lost ? 'lost reply reads back without retry' : 'updates card without applying theme'}`,async({page})=>{
  await page.evaluate(lost=>{(window as unknown as {fixture:{loseRename:boolean}}).fixture.loseRename=lost;},lost);
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  await page.getByRole('button',{name:'Переименовать Изображение 00000000',exact:true}).click();
  await page.getByLabel('Имя изображения',{exact:true}).fill('Сад');
  await page.getByRole('button',{name:'Сохранить имя изображения',exact:true}).click();
  await expect(page.getByRole('img',{name:'Сад',exact:true})).toBeVisible();
  if(lost) { await expect(page.getByRole('alert')).toContainText('reply lost');await expect(page.getByLabel('Имя изображения',{exact:true})).toHaveValue('Сад');await expect(page.getByRole('button',{name:'Сохранить имя изображения',exact:true})).toBeDisabled(); }
  const calls=await page.evaluate(()=>(window as unknown as {fixture:{calls:{method:string;params:Record<string,unknown>}[]}}).fixture.calls);
  expect(calls.filter(c=>c.method==='rename_theme_asset')).toHaveLength(1);
  expect(calls.find(c=>c.method==='rename_theme_asset')?.params).toEqual({key:'0'.repeat(32),name:'Сад',expected_revision:0});
  expect(calls.some(c=>c.method==='set_lcd_theme')).toBe(false);
});

test('image selection follows each collection and failed readback hides stale entries', async ({page}) => {
  await page.evaluate(() => { (window as unknown as {fixture:{scoped:boolean}}).fixture.scoped=true; });
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека изображений',{exact:true}).click();
  const gallery=page.locator('details').filter({has:page.locator('summary',{hasText:'Библиотека изображений'})});
  await expect(gallery.locator('article')).toHaveCount(1);
  await expect(gallery.getByRole('button',{name:'Добавить в тему 00000000'})).toBeEnabled();
  await gallery.getByLabel('Назначение изображения').selectOption('background');
  await expect(gallery.getByRole('button',{name:'Сделать фоном Изображение 22222222'})).toBeEnabled();
  await gallery.getByLabel('Назначение изображения').selectOption('watermark');
  await expect(gallery.getByRole('button',{name:'Добавить водяной знак Изображение 33333333'})).toBeEnabled();
  await gallery.getByLabel('Показать все файлы изображений для управления').check();
  await expect(gallery.locator('article')).toHaveCount(8);
  await expect(gallery.getByRole('button',{name:'Добавить водяной знак Изображение 00000000'})).toBeDisabled();
  await expect(gallery.getByRole('button',{name:'Удалить Изображение 00000000…',exact:true})).toBeEnabled();
  await page.evaluate(() => { (window as unknown as {fixture:{collectionError:boolean}}).fixture.collectionError=true; });
  await gallery.getByRole('button',{name:'Обновить изображения'}).click();
  await expect(gallery.getByRole('alert')).toContainText('collection read failed');
  await expect(gallery.locator('article')).toHaveCount(0);
});

test('unnamed image fallback labels use English without changing asset metadata',async({page})=>{
 await page.evaluate(()=>localStorage.setItem('lconnect.interface-language','en-US'));await page.reload();
 await page.getByRole('button',{name:'Library',exact:true}).click();await page.getByText('Image library',{exact:true}).click();
 await expect(page.getByRole('button',{name:'Rename Image 00000000',exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {fixture:{calls:{method:string}[]}}).fixture.calls.filter(c=>c.method==='rename_theme_asset'||c.method==='set_lcd_theme'))).toEqual([]);
});
