import { test, expect, type Page } from '@playwright/test';
const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=';
async function setup(page: Page, fail = false, delay = false, hold = false) {
  await page.addInitScript(({png,fail,delay,hold}) => {
    const calls: {command:string;args:Record<string,unknown>}[] = [];
    const control = {offline:false,release:undefined as (()=>void)|undefined};
    Object.assign(window,{editorControl:control,editorCalls:calls,__TAURI_INTERNALS__:{invoke:async (command:string,args:Record<string,unknown>) => {
      calls.push({command,args});
      if(command==='begin_screenshot')return '11111111111111111111111111111111';
      if(command==='cancel_screenshot')return null;
      if(command==='image_editor') {
        if (delay && args.edit) await new Promise(resolve => setTimeout(resolve,600));
        return {width:960,height:480,png:Array.from(atob(png),c=>c.charCodeAt(0))};
      }
      if(command==='import_image') { if(hold) await new Promise<void>(resolve=>control.release=resolve); if(fail) throw {message:'Lost import reply'}; return 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'; }
      switch(args.method) {
        case 'list_devices': if(control.offline) throw {message:'Disconnected'}; return [{id:'lcd:test',display_name:'Test LCDV2'}];
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source:'Idle',last_error:null,requested:{mode:'LocalUI',brightness_percent:100,rotation_degrees:0}};
        case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        default: return null;
      }
    }}});
  },{png,fail,delay,hold});
  await page.goto('/');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:'Импорт с редактированием…'}).click();
  await page.getByLabel('Файл для редактирования').setInputFiles({name:'wide.png',mimeType:'image/png',buffer:Buffer.from(png,'base64')});
  await expect(page.getByLabel('Crop X',{exact:true})).toHaveValue('240');
}
test('crop and transform preview stays local, changed crop invalidates result, save publishes once',async({page})=>{
  await setup(page);
  const panel=page.getByRole('region',{name:'Редактор изображения',exact:true});
  await page.getByLabel('Crop X',{exact:true}).fill('480');
  await panel.getByRole('button',{name:'Повернуть вправо',exact:true}).click();
  await panel.getByRole('button',{name:'Отразить по горизонтали',exact:true}).click();
  await panel.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await expect(panel.getByAltText('Результат редактирования 480 на 480')).toBeVisible();
  expect(await page.evaluate(()=> (window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image').length)).toBe(0);
  const prepared=await page.evaluate(()=> (window as unknown as {editorCalls:{command:string;args:{edit:unknown}}[]}).editorCalls.filter(c=>c.command==='image_editor').at(-1)?.args.edit);
  expect(prepared).toEqual({crop:{x:480,y:0,side:480},transforms:['rotate_right','flip_horizontal'],filters:{hue:0,saturation:0,brightness:0,contrast:0,sharpen:0,blur:0}});
  await page.getByLabel('Crop X',{exact:true}).fill('481');
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeDisabled();
  await expect(panel.getByRole('alert')).toContainText('внутри изображения');
  await page.getByLabel('Crop X',{exact:true}).fill('479');
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeDisabled();
  await panel.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeEnabled();
  await panel.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect(panel.getByRole('status')).toContainText('Результат сохранён');
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeDisabled();
  const calls=await page.evaluate(()=> (window as unknown as {editorCalls:{command:string;args:{method?:string}}[]}).editorCalls);
  expect(calls.filter(c=>c.command==='import_image')).toHaveLength(1);
  expect(calls.some(c=>c.args.method==='set_lcd_theme')).toBe(false);
});
test('closing a pending preview discards its reply and restores focus without publication',async({page})=>{
  await setup(page,false,true);
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Закрыть редактор изображения'}).click();
  await expect(page.getByRole('button',{name:'Импорт с редактированием…'})).toBeFocused();
  await page.waitForTimeout(700);
  await expect(page.getByAltText('Результат редактирования 480 на 480')).toHaveCount(0);
  expect(await page.evaluate(()=> (window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image').length)).toBe(0);
});
test('lost publication reply requires catalog inspection and does not offer blind retry',async({page})=>{
  await setup(page,true);
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect(page.getByRole('alert')).toContainText('Проверьте библиотеку');
  await expect(page.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Импорт с редактированием…'})).toBeDisabled();
  expect(await page.evaluate(()=> (window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image').length)).toBe(1);
});

test('publication rechecks layer capacity after delayed import',async({page})=>{
  await setup(page,false,false,true);
  while (await page.locator('.layer').count() < 31) await page.getByRole('button',{name:'+ Текст',exact:true}).click();
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await page.getByRole('button',{name:'+ Текст',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>!!(window as unknown as {editorControl:{release?:()=>void}}).editorControl.release)).toBe(true);
  await page.evaluate(()=>(window as unknown as {editorControl:{release:()=>void}}).editorControl.release());
  await expect(page.getByRole('region',{name:'Редактор изображения',exact:true}).getByRole('status')).toContainText('лимит 32 слоёв');
  await expect(page.locator('.layer')).toHaveCount(32);
});
test('publication after connection loss reports library-only outcome',async({page})=>{
  await setup(page,false,false,true);
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect.poll(()=>page.evaluate(()=>!!(window as unknown as {editorControl:{release?:()=>void}}).editorControl.release)).toBe(true);
  await page.evaluate(()=>(window as unknown as {editorControl:{offline:boolean}}).editorControl.offline=true);
  await expect(page.getByRole('button',{name:'Применить к LCD',exact:true})).toBeDisabled();
  await expect(page.getByText('Daemon подключён',{exact:true})).toHaveCount(0);
  await page.evaluate(()=>(window as unknown as {editorControl:{release:()=>void}}).editorControl.release());
  await expect(page.getByRole('region',{name:'Редактор изображения',exact:true}).getByRole('status')).toContainText('черновик не изменён');
});

test('color controls validate Windows ranges and invalidate previously prepared output',async({page})=>{
  await setup(page);
  const panel=page.getByRole('region',{name:'Редактор изображения',exact:true});
  await panel.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeEnabled();
  await panel.getByLabel('Насыщенность',{exact:true}).fill('-26');
  await expect(panel.getByRole('button',{name:'Подготовить preview изображения'})).toBeDisabled();
  await expect(panel.getByRole('button',{name:'Сохранить изображение в галерею'})).toBeDisabled();
  await panel.getByLabel('Насыщенность',{exact:true}).fill('-25');
  await panel.getByLabel('Сдвиг тона',{exact:true}).fill('120');
  await panel.getByLabel('Яркость изображения',{exact:true}).fill('-50');
  await panel.getByLabel('Контраст изображения',{exact:true}).fill('75');
  await panel.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await expect(panel.getByAltText('Результат редактирования 480 на 480')).toBeVisible();
  const edit=await page.evaluate(()=>(window as unknown as {editorCalls:{command:string;args:{edit:unknown}}[]}).editorCalls.filter(c=>c.command==='image_editor').at(-1)?.args.edit);
  expect(edit).toMatchObject({filters:{hue:120,saturation:-25,brightness:-50,contrast:75}});
  await panel.getByLabel('Сдвиг тона',{exact:true}).fill('');
  await expect(panel.getByRole('button',{name:'Подготовить preview изображения'})).toBeDisabled();
  await panel.getByRole('button',{name:'Сбросить правки'}).click();
  await expect(panel.getByLabel('Сдвиг тона',{exact:true})).toHaveValue('0');
});

test('spatial filters validate bounds, invalidate preview and reset',async({page})=>{
  await setup(page);
  const panel=page.getByRole('region',{name:'Редактор изображения',exact:true});
  const prepare=panel.getByRole('button',{name:'Подготовить preview изображения'});
  const save=panel.getByRole('button',{name:'Сохранить изображение в галерею'});
  await prepare.click();
  await expect(save).toBeEnabled();
  await panel.getByLabel('Резкость',{exact:true}).fill('5');
  await expect(save).toBeDisabled();
  await panel.getByLabel('Размытие',{exact:true}).fill('11');
  await expect(prepare).toBeDisabled();
  await panel.getByLabel('Размытие',{exact:true}).fill('2.5');
  await prepare.click();
  await expect(save).toBeEnabled();
  const edit=await page.evaluate(()=>(window as unknown as {editorCalls:{command:string;args:{edit:unknown}}[]}).editorCalls.filter(c=>c.command==='image_editor').at(-1)?.args.edit);
  expect(edit).toMatchObject({filters:{sharpen:5,blur:2.5}});
  await panel.getByLabel('Резкость',{exact:true}).fill('');
  await expect(prepare).toBeDisabled();
  await panel.getByRole('button',{name:'Сбросить правки'}).click();
  await expect(panel.getByLabel('Резкость',{exact:true})).toHaveValue('0');
  await expect(panel.getByLabel('Размытие',{exact:true})).toHaveValue('0');
});

test('portal screenshot stays in editor until explicit preview and Save',async({page})=>{
  await setup(page);
  await page.evaluate(png=>{
    const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}};
    const original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>command==='capture_screenshot'
      ? Uint8Array.from(atob(png),c=>c.charCodeAt(0)).buffer : original(command,args);
  },png);
  await page.getByRole('button',{name:'Снять экран…',exact:true}).click();
  await expect(page.getByText('Снимок экрана.png · 960 × 480 px')).toBeVisible();
  expect(await page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image'))).toHaveLength(0);
  await page.getByRole('button',{name:'Подготовить preview изображения'}).click();
  await page.getByRole('button',{name:'Сохранить изображение в галерею'}).click();
  await expect(page.getByText('Результат сохранён', {exact:false})).toBeVisible();
  const imports=await page.evaluate(()=>(window as unknown as {editorCalls:{command:string;args:{name?:string}}[]}).editorCalls.filter(c=>c.command==='import_image'));
  expect(imports).toHaveLength(1);expect(imports[0].args.name).toBe('Снимок экрана.png');
});

test('portal cancel preserves existing source and pending capture blocks repeated capture',async({page})=>{
  await setup(page);
  await page.evaluate(()=>{
    const w=window as unknown as {captureRelease?:()=>void;__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}};
    const original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>command==='capture_screenshot'
      ? new Promise<ArrayBuffer>(resolve=>w.captureRelease=()=>resolve(new ArrayBuffer(0))) : original(command,args);
  });
  await page.getByRole('button',{name:'Снять экран…',exact:true}).click();
  await expect(page.getByRole('button',{name:'Снять экран…',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Закрыть редактор изображения'})).toBeEnabled();
  await page.evaluate(()=>(window as unknown as {captureRelease:()=>void}).captureRelease());
  await expect(page.getByText('Снимок экрана отменён.')).toBeVisible();
  await expect(page.getByText('wide.png · 960 × 480 px')).toBeVisible();
  await expect(page.getByRole('button',{name:'Закрыть редактор изображения'})).toBeEnabled();
});

test('portal failure reports an error without erasing or publishing the current draft',async({page})=>{
  await setup(page);
  await page.evaluate(()=>{
    const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}};
    const original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='capture_screenshot')throw {message:'Screenshot portal unavailable'};
      return original(command,args);
    };
  });
  await page.getByRole('button',{name:'Снять экран…',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('Screenshot portal unavailable');
  await expect(page.getByText('wide.png · 960 × 480 px')).toBeVisible();
  await expect(page.getByRole('button',{name:'Снять экран…',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image'))).toHaveLength(0);
});

test('closing before screenshot reservation reply cancels it without starting capture',async({page})=>{
  await setup(page);
  await page.evaluate(()=>{
    const w=window as unknown as {releaseReservation:()=>void;__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}};
    const original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='begin_screenshot')return new Promise<string>(resolve=>w.releaseReservation=()=>resolve('delayed-reservation'));
      return original(command,args);
    };
  });
  await page.getByRole('button',{name:'Снять экран…',exact:true}).click();
  await page.getByRole('button',{name:'Закрыть редактор изображения'}).click();
  await page.evaluate(()=>(window as unknown as {releaseReservation:()=>void}).releaseReservation());
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {editorCalls:{command:string;args:{id?:string}}[]}).editorCalls.some(c=>c.command==='cancel_screenshot'&&c.args.id==='delayed-reservation'))).toBe(true);
  await expect(page.getByRole('region',{name:'Редактор изображения',exact:true})).toHaveCount(0);
  expect(await page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='capture_screenshot'||c.command==='import_image'))).toHaveLength(0);
});

test('closing a pending screenshot sends cancellation and ignores late pixels',async({page})=>{
  await setup(page);
  await page.evaluate(png=>{
    const w=window as unknown as {releaseScreenshot:()=>void;__TAURI_INTERNALS__:{invoke:(command:string,args:unknown)=>Promise<unknown>}};
    const original=w.__TAURI_INTERNALS__.invoke;
    w.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='capture_screenshot')return new Promise<ArrayBuffer>(resolve=>w.releaseScreenshot=()=>resolve(Uint8Array.from(atob(png),c=>c.charCodeAt(0)).buffer));
      return original(command,args);
    };
  },png);
  await page.getByRole('button',{name:'Снять экран…',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>typeof (window as unknown as {releaseScreenshot?:()=>void}).releaseScreenshot)).toBe('function');
  await page.getByRole('button',{name:'Закрыть редактор изображения'}).click();
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.some(c=>c.command==='cancel_screenshot'))).toBe(true);
  await page.evaluate(()=>(window as unknown as {releaseScreenshot:()=>void}).releaseScreenshot());
  await expect(page.getByRole('button',{name:'Импорт с редактированием…'})).toBeFocused();
  expect(await page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='image_editor'))).toHaveLength(1);
  expect(await page.evaluate(()=>(window as unknown as {editorCalls:{command:string}[]}).editorCalls.filter(c=>c.command==='import_image'))).toHaveLength(0);
});
