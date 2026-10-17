import { test, expect } from '@playwright/test';

test.beforeEach(async ({page})=>{
  await page.addInitScript(()=>{
    const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='),c=>c.charCodeAt(0));
    const fixture={corrupt:false,delay:false,release:null as null|(()=>void),busy:0,fail:false,requests:0,cancelled:[] as string[],created:[] as string[],revoked:[] as string[],writes:[] as string[]};
    const create=URL.createObjectURL.bind(URL),revoke=URL.revokeObjectURL.bind(URL);
    URL.createObjectURL=(blob:Blob)=>{const url=create(blob);fixture.created.push(url);return url;};
    URL.revokeObjectURL=(url:string)=>{fixture.revoked.push(url);revoke(url);};
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
      if(command==='media_status')return null;
      if(command==='media_thumbnail_cancel'){fixture.cancelled.push(args.id as string);return null;}
      if(command==='media_thumbnail'){
        fixture.requests++;
        if(fixture.busy-->0)throw {message:'Thumbnail worker busy'};
        if(fixture.fail)throw {message:'Decoder unavailable'};
        if(fixture.delay&&args.key==='a'.repeat(32)){fixture.delay=false;await new Promise<void>(resolve=>{fixture.release=resolve;});}
        return fixture.corrupt?[1,2,3,4]:png;
      }
      switch(args.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius'};
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'get_lcd_media':return null;
        case 'list_prepared_clips':return ['a','b'].map((v,i)=>({key:v.repeat(32),name:['Волны','Облака'][i],fps:24,frames:24,encoded_bytes:100}));
        default:fixture.writes.push(String(args.method));throw new Error(String(args.method));
      }
    }}});
  });
  await page.goto('/?view=display');
});

test('late old thumbnail cannot replace selected clip; unmount releases image URL',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{delay:boolean}}).fixture.delay=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{release:unknown}}).fixture.release!==null)).toBe(true);
  await page.getByLabel('Сохранённый ролик').selectOption('b'.repeat(32));
  const image=page.getByRole('img',{name:'Первый кадр: Облака'});
  // The next decoder waits for acknowledgement from the cancelled old one.
  await expect(image).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {fixture:{release:()=>void}}).fixture.release();});
  await expect(image).toBeVisible(); const source=await image.getAttribute('src');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  // Native details toggle is delivered asynchronously; wait for Svelte cleanup.
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {fixture:{revoked:string[]}}).fixture.revoked)).toEqual([source]);
  const state=await page.evaluate(()=>(window as unknown as {fixture:{created:string[];revoked:string[];cancelled:string[];writes:string[]}}).fixture);
  expect(state.created).toEqual([source]);expect(state.revoked).toEqual([source]);
  expect(state.cancelled.length).toBeGreaterThanOrEqual(1);expect(state.writes).toEqual([]);
});

test('busy decoder retries within bound without applying playback',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{busy:number}}).fixture.busy=3;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByRole('img',{name:'Первый кадр: Волны'})).toBeVisible();
  expect(await page.evaluate(()=>(window as unknown as {fixture:{requests:number}}).fixture.requests)).toBe(4);
});

test('thumbnail failure does not block selecting media and allows explicit retry',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{fail:boolean}}).fixture.fail=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByText('Миниатюра недоступна: Decoder unavailable',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true})).toBeEnabled();
  await page.evaluate(()=>{(window as unknown as {fixture:{fail:boolean}}).fixture.fail=false;});
  await page.getByRole('button',{name:'Обновить миниатюру',exact:true}).click();
  await expect(page.getByRole('img',{name:'Первый кадр: Волны'})).toBeVisible();
});

test('browser decode failure exposes explicit retry and releases both thumbnail URLs',async({page})=>{
 await page.evaluate(()=>{(window as unknown as {fixture:{corrupt:boolean}}).fixture.corrupt=true;});
 await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();await page.getByText('Библиотека роликов',{exact:true}).click();
 await expect(page.getByRole('status').filter({hasText:'Не удалось показать миниатюру. Повторите запрос.'})).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {fixture:{requests:number}}).fixture.requests)).toBe(1);
 await page.evaluate(()=>{(window as unknown as {fixture:{corrupt:boolean}}).fixture.corrupt=false;});await page.getByRole('button',{name:'Обновить миниатюру',exact:true}).click();
 const image=page.getByRole('img',{name:'Первый кадр: Волны',exact:true});await image.evaluate((node:HTMLImageElement)=>node.decode());await expect(image).toBeVisible();
 await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();await page.getByText('Библиотека роликов',{exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>{const f=(window as unknown as {fixture:{created:string[];revoked:string[]}}).fixture;return f.revoked.length;})).toBe(2);
 const state=await page.evaluate(()=>(window as unknown as {fixture:{created:string[];revoked:string[];requests:number;writes:string[]}}).fixture);expect(state.revoked).toEqual(state.created);expect(state.requests).toBe(2);expect(state.writes).toEqual([]);
});
