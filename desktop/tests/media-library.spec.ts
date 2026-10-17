import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const keys = ['a'.repeat(32), 'b'.repeat(32)];
    const fixture = {settings:{version:1,revision:0,temperature_unit:'celsius',interface_language:'ru-RU'},selectionFailure:false,scoped:false,online:true, ids:['A','B'], active:null as string|null, fail:false, writes:[] as {method:string;dev:string;key?:string}[], delay:false, resolve:null as null|(()=>void)};
    Object.assign(window,{fixture,__TAURI_INTERNALS__:{invoke:async (command:string,req:{method:string;params:{dev:string;key:string}})=>{
        if (command === 'desktop_set_language') return null;
      if(command === 'media_thumbnail_cancel') return null;
      if(command === 'media_thumbnail') return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=='), c=>c.charCodeAt(0));
      if(command === 'media_status') return null;
      if(!fixture.online) throw new Error('offline');
      switch(req.method){
        case 'get_display_settings': return structuredClone(fixture.settings);
        case 'get_screen_collections': return {collections:fixture.scoped ? {version:1,revision:1,images:[],watermarks:[],backgrounds:[{kind:'video',key:keys[req.params.dev==='A'?0:1]}]}:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_devices': return fixture.ids.map(id=>({id,display_name:`LCD ${id}`, family:'galahad2-lcdv2'}));
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source:fixture.active?'DiagnosticClip':'Idle'};
        case 'list_prepared_clips':
          if(fixture.fail) throw new Error('Damaged clip');
          return keys.map((key,i)=>({key,name:['Волны','Облака'][i],fps:24,frames:48,encoded_bytes:1000}));
        case 'get_lcd_media':
          if(fixture.delay && req.params.dev==='A') {fixture.delay=false; await new Promise<void>(resolve=>{fixture.resolve=resolve;}); return keys[0];}
          return fixture.active;
        case 'set_lcd_media': fixture.writes.push({method:req.method,...req.params}); fixture.active=req.params.key;if(fixture.selectionFailure){fixture.selectionFailure=false;throw new Error('Настройки');}return {source:'DiagnosticClip'};
        case 'clear_lcd': fixture.writes.push({method:req.method,...req.params});fixture.active=null;return null;
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByLabel('Устройство',{exact:true})).toHaveValue('A');
});

test('select and stop saved media target selected device and retain theme draft',async({page})=>{
  await page.getByLabel('Название',{exact:true}).fill('Несохранённая тема');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await page.getByLabel('Сохранённый ролик').selectOption('b'.repeat(32));
  await page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true}).click();
  await expect(page.getByText('Сохранённый выбор: Облака',{exact:true})).toBeVisible();
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Несохранённая тема');
  await page.getByRole('button',{name:'Остановить ролик и вернуть LocalUI',exact:true}).click();
  await expect(page.getByText('Ролик для автовосстановления не выбран.',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=> (window as unknown as {fixture:{writes:unknown[]}}).fixture.writes)).toEqual([
    {method:'set_lcd_media',dev:'A',key:'b'.repeat(32)},{method:'clear_lcd',dev:'A'}]);
});

test('library failure retries and lost device never redirects playback',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{fail:boolean}}).fixture.fail=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('Damaged clip');
  await expect(page.getByText('Ролик для автовосстановления не выбран.',{exact:true})).toHaveCount(0);
  await page.evaluate(()=>{Object.assign((window as unknown as {fixture:object}).fixture,{fail:false,ids:['B']});});
  await expect(page.getByLabel('Устройство',{exact:true})).toHaveValue('A');
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true})).toBeDisabled();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true})).toBeEnabled();
  await page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true}).click();
  expect(await page.evaluate(()=> (window as unknown as {fixture:{writes:{dev:string}[]}}).fixture.writes[0].dev)).toBe('B');
});

test('late selection response from old device is discarded',async({page})=>{
  await page.evaluate(()=>{(window as unknown as {fixture:{delay:boolean}}).fixture.delay=true;});
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByText('Загрузка роликов…',{exact:true})).toBeVisible();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await expect(page.getByText('Ролик для автовосстановления не выбран.',{exact:true})).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {fixture:{resolve:null|(()=>void)}}).fixture.resolve?.();});
  await expect(page.getByText('Сохранённый выбор: Волны',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Выбрать ролик для LCD',exact:true}).click();
  expect(await page.evaluate(()=> (window as unknown as {fixture:{writes:{dev:string}[]}}).fixture.writes[0].dev)).toBe('B');
});

test('video selection uses the selected device background collection',async({page})=>{
  await page.evaluate(() => { (window as unknown as {fixture:{scoped:boolean}}).fixture.scoped=true; });
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await expect(page.getByLabel('Сохранённый ролик').locator('option')).toHaveCount(1);
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('a'.repeat(32));
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await expect(page.getByLabel('Сохранённый ролик')).toHaveValue('b'.repeat(32));
  await expect(page.getByLabel('Сохранённый ролик').locator('option')).toHaveCount(1);
});

test('full video catalog permits file management but not playback outside collection',async({page})=>{
  await page.evaluate(() => { (window as unknown as {fixture:{scoped:boolean}}).fixture.scoped=true; });
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Библиотека роликов',{exact:true}).click();
  await page.getByLabel('Показать все файлы видео для управления').check();
  await page.getByLabel('Сохранённый ролик').selectOption('b'.repeat(32));
  await expect(page.getByRole('button',{name:'Выбрать ролик для LCD'})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Использовать фоном темы'})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Удалить ролик…',exact:true})).toBeEnabled();
});

for(const language of ['en-US','ru-RU'] as const) test(`media selection preserves raw errors and switches language without replay (${language})`,async({page})=>{
  await page.evaluate(language=>{const f=(window as unknown as {fixture:{settings:{interface_language:string;revision:number};selectionFailure:boolean}}).fixture;f.settings.interface_language=language;f.settings.revision++;f.selectionFailure=true;},language);
  const english=language==='en-US';await expect(page.locator('html')).toHaveAttribute('lang',english?'en':'ru');
  await page.getByLabel(english?'Name':'Название',{exact:true}).fill('Имя {1} <CPU>');
  await page.getByRole('button',{name:english?'Library':'Библиотека',exact:true}).click();
  await page.getByText(english?'Video library':'Библиотека роликов',{exact:true}).click();
  await page.getByLabel(english?'Saved video':'Сохранённый ролик').selectOption('b'.repeat(32));
  await page.getByRole('button',{name:english?'Select video for LCD':'Выбрать ролик для LCD',exact:true}).click();
  const alert=page.getByRole('alert').filter({hasText:'Настройки'});await expect(alert).toBeVisible();
  await expect(alert).toContainText(english?'Video selection was not confirmed':'Выбор ролика не подтверждён');
  await page.evaluate(language=>{const f=(window as unknown as {fixture:{settings:{interface_language:string;revision:number}}}).fixture;f.settings.interface_language=language;f.settings.revision++;},english?'ru-RU':'en-US');
  await expect(page.locator('html')).toHaveAttribute('lang',english?'ru':'en');
  await expect(alert).toContainText(english?'Выбор ролика не подтверждён':'Video selection was not confirmed');
  await expect(page.getByLabel(english?'Название':'Name',{exact:true})).toHaveValue('Имя {1} <CPU>');
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:unknown[]}}).fixture.writes.length)).toBe(1);
  await page.getByRole('button',{name:english?'Выбрать ролик для LCD':'Select video for LCD',exact:true}).click();
  await expect(alert).toHaveCount(0);
  expect(await page.evaluate(()=>(window as unknown as {fixture:{writes:{method:string;key:string}[]}}).fixture.writes.map(w=>[w.method,w.key]))).toEqual([['set_lcd_media','b'.repeat(32)],['set_lcd_media','b'.repeat(32)]]);
});
