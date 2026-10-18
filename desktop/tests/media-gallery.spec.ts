import { test, expect } from '@playwright/test';
test.beforeEach(async ({page})=>{
  await page.addInitScript(()=>{
    const fixture={hold:false,release:null as null|(()=>void),active:0,maxActive:0,keys:[] as string[],cancelled:[] as string[],created:[] as string[],revoked:[] as string[],writes:[] as string[],reads:0};
    const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);
    URL.createObjectURL=(blob:Blob)=>{const url=create(blob);fixture.created.push(url);return url;};
    URL.revokeObjectURL=(url:string)=>{fixture.revoked.push(url);revoke(url);};
    const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0));
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status')return null;
      if(command==='media_thumbnail_cancel'){fixture.cancelled.push(String(args.id));return null;}
      if(command==='media_thumbnail'){
        fixture.keys.push(String(args.key));fixture.active++;fixture.maxActive=Math.max(fixture.maxActive,fixture.active);
        try {
          if(fixture.hold){fixture.hold=false;await new Promise<void>(resolve=>{fixture.release=resolve;});}
          else await new Promise(resolve=>setTimeout(resolve,15));
          return png;
        } finally {fixture.active--;}
      }
      switch(args.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'get_lcd_media':return null;
        case 'list_prepared_clips':fixture.reads++;return Array.from({length:16},(_,i)=>({key:i.toString(16).padStart(32,'0'),name:`Clip ${i}`,fps:24,frames:24,encoded_bytes:100}));
        default:fixture.writes.push(String(args.method));throw new Error(String(args.method));
      }
    }}});
  });
  await page.goto('/?view=display');
});

test('gallery decodes one page sequentially, retains browsed page on refresh and selects without Apply',async({page})=>{
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await page.getByLabel('Показать карточки роликов').check();
  const gallery=page.getByRole('group',{name:'Карточки роликов'});
  await expect(gallery.getByRole('img')).toHaveCount(8);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{maxActive:number;keys:string[]}}).fixture.maxActive)).toBe(1);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{keys:string[]}}).fixture.keys.every(key=>parseInt(key,16)<8))).toBe(true);
  await gallery.getByRole('button',{name:'Следующие ролики',exact:true}).click();
  await expect(gallery.getByRole('img')).toHaveCount(8);
  await expect(gallery.getByRole('radio',{name:'Clip 15',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Обновить ролики',exact:true}).click();
  await expect(gallery.getByText('Страница 2 из 2',{exact:true})).toBeVisible();
  await gallery.getByRole('radio',{name:'Clip 15',exact:true}).check();
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('f'.padStart(32,'0'));
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:string[]}}).fixture.writes)).toEqual([]);
  await page.keyboard.press('Escape');
  await expect.poll(()=>page.evaluate(()=>{const f=(window as unknown as {fixture:{created:string[];revoked:string[]}}).fixture;return f.created.length===f.revoked.length;})).toBe(true);
});

test('page changes and close remove queued thumbnails before native dispatch',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{hold:boolean}}).fixture.hold=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{release:unknown}}).fixture.release!==null)).toBe(true);
  await page.getByLabel('Показать карточки роликов').check();
  await page.getByRole('button',{name:'Следующие ролики',exact:true}).click();
  await page.keyboard.press('Escape');
  // Popover becomes hidden before its asynchronous toggle unmounts children.
  await expect(page.getByRole('group',{name:'Карточки роликов',includeHidden:true})).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {fixture:{release:()=>void}}).fixture.release();});
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{active:number}}).fixture.active)).toBe(0);
  const state=await page.evaluate(()=>(window as unknown as {fixture:{keys:string[];created:string[];cancelled:string[]}}).fixture);
  expect(state.keys).toHaveLength(1);expect(state.created).toEqual([]);expect(state.cancelled).toHaveLength(1);
});
