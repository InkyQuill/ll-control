import { test, expect, type Page } from '@playwright/test';
type Language='en-US'|'ru-RU';
type Operation='image-list'|'video-list'|'rename'|'image-delete'|'video-delete'|'video-edit';
interface State {settings:{version:number;revision:number;temperature_unit:string;interface_language:Language};online:boolean;holdCollection:boolean;release:null|((fail:boolean)=>void);fail:boolean;readbackArmed:boolean;readbackFailed:boolean;writes:{method:string;params:Record<string,unknown>}[]}
declare global {interface Window {libraryMessages:State}}
const name='Имя {1} <CPU>';
async function setup(page:Page,language:Language,operation:Operation){
 await page.addInitScript(({language,operation,name})=>{
  const png=Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII='),c=>c.charCodeAt(0));
  const f:State={settings:{version:1,revision:0,temperature_unit:'celsius',interface_language:language},online:true,holdCollection:false,release:null,fail:true,readbackArmed:false,readbackFailed:false,writes:[]};
  let images=[{key:'a'.repeat(32),name,revision:1,is_default:false}],videos=[{key:'b'.repeat(32),name,fps:24,frames:24,encoded_bytes:100}];
  localStorage.setItem('lconnect.interface-language',language);window.libraryMessages=f;
  Object.assign(window,{__TAURI_INTERNALS__:{invoke:async(command:string,args:{method?:string;params?:Record<string,unknown>}={})=>{
        if (command === 'desktop_set_language') return null;
   if(command==='media_thumbnail')return png;
   const params=args.params??{};
   switch(args.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
    case 'get_display_settings':return structuredClone(f.settings);
    case 'list_devices':if(!f.online)throw new Error('offline');return [{id:'A',display_name:'LCD A',family:'galahad2-lcdv2'}];
    case 'list_sensors':return [];
    case 'get_lcd_state':return {source:'Idle',last_error:null,requested:{}};
    case 'get_lcd_media':return null;
    case 'get_screen_collections':
     if(f.holdCollection){f.holdCollection=false;await new Promise<void>((resolve,reject)=>f.release=fail=>fail?reject(new Error('Obsolete video editor failure')):resolve());}
     return {collections:operation==='video-edit'?{version:1,revision:1,images:[],watermarks:[],backgrounds:[{kind:'video',key:'b'.repeat(32)}]}:null,persistence:{pending:false,saved:true,last_error:null}};
    case 'list_named_theme_assets':if(f.readbackFailed||(operation==='image-list'&&f.fail))throw new Error('Настройки');return structuredClone(images);
    case 'list_prepared_clips':if(operation==='video-list'&&f.fail)throw new Error('Настройки');return structuredClone(videos);
    case 'thumbnail_theme_asset':return png;
    case 'rename_theme_asset':
     f.writes.push({method:args.method,params:structuredClone(params)});
     if(params.expected_revision!==images[0].revision)throw new Error('stale revision');
     images[0]={...images[0],name:String(params.name),revision:images[0].revision+1};if(f.readbackArmed)f.readbackFailed=true;if(f.fail)throw new Error('Настройки');return null;
    case 'delete_theme_asset':case 'delete_prepared_clip':
     f.writes.push({method:args.method,params:structuredClone(params)});
     if(args.method==='delete_theme_asset')images=[];else videos=[];
     if(f.fail)throw new Error('Настройки');return null;
    default:return null;
   }
  }}});
 },{language,operation,name});
 await page.goto('/?view=display');await expect(page.getByText(language==='en-US'?'Daemon connected':'Daemon подключён',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:language==='en-US'?'Library':'Библиотека',exact:true}).click();
 await page.getByText(operation.startsWith('video')?(language==='en-US'?'Video library':'Библиотека роликов'):(language==='en-US'?'Image library':'Библиотека изображений'),{exact:true}).click();
}
async function switchLanguage(page:Page,language:Language){
 await page.evaluate(language=>{window.libraryMessages.settings.interface_language=language;window.libraryMessages.settings.revision++;},language);
 await expect(page.locator('html')).toHaveAttribute('lang',language==='en-US'?'en':'ru');
}
for(const language of ['en-US','ru-RU'] as const){
 for(const operation of ['image-list','video-list','rename','image-delete','video-delete'] as const){
  test(`${operation} preserves diagnostics and mutation ownership across language switches (${language})`,async({page})=>{
   const english=language==='en-US',other:Language=english?'ru-RU':'en-US';await setup(page,language,operation);
   if(operation==='rename'){
    await page.getByRole('button',{name:`${english?'Rename':'Переименовать'} ${name}`,exact:true}).click();
    await page.getByLabel(english?'Image name':'Имя изображения',{exact:true}).fill('Черновик {1} <CPU>');
    await page.getByRole('button',{name:english?'Save image name':'Сохранить имя изображения',exact:true}).click();
   }else if(operation.endsWith('delete')){
    await page.getByRole('button',{name:operation==='image-delete'?`${english?'Delete':'Удалить'} ${name}…`:(english?'Delete video…':'Удалить ролик…'),exact:true}).click();
    await page.getByRole('button',{name:operation==='image-delete'?(english?'Confirm image deletion':'Подтвердить удаление изображения'):(english?'Confirm video deletion':'Подтвердить удаление ролика'),exact:true}).click();
   }
   const diagnostic=page.getByRole(operation.endsWith('delete')?'status':'alert').filter({hasText:'Настройки'});await expect(diagnostic).toBeVisible();
   await switchLanguage(page,other);await expect(diagnostic).toBeVisible();
   if(operation==='rename'){
    await expect(diagnostic).toContainText(english?'Переименование не подтверждено':'Renaming was not confirmed');
    const input=page.getByLabel(english?'Имя изображения':'Image name',{exact:true});await expect(input).toHaveValue('Черновик {1} <CPU>');
    await expect(page.getByRole('button',{name:english?'Сохранить имя изображения':'Save image name',exact:true})).toBeDisabled();
    await input.press('Enter');expect(await page.evaluate(()=>window.libraryMessages.writes.length)).toBe(1);
    await page.evaluate(()=>window.libraryMessages.fail=false);
    await page.getByRole('button',{name:`${english?'Переименовать':'Rename'} Черновик {1} <CPU>`,exact:true}).click();
    await expect(page.getByRole('button',{name:english?'Сохранить имя изображения':'Save image name',exact:true})).toBeEnabled();
    await input.fill('Next');await page.getByRole('button',{name:english?'Сохранить имя изображения':'Save image name',exact:true}).click();
    await expect(page.getByRole('button',{name:`${english?'Переименовать':'Rename'} Next`,exact:true})).toBeVisible();
    expect(await page.evaluate(()=>window.libraryMessages.writes.map(w=>w.params.expected_revision))).toEqual([1,2]);
   }else if(operation.endsWith('delete')){
    await expect(diagnostic).toContainText(english?'Удаление не подтверждено':'Deletion was not confirmed');
    expect(await page.evaluate(()=>window.libraryMessages.writes.length)).toBe(1);
    if(operation==='image-delete')await expect(page.getByRole('button',{name:new RegExp(name.replace(/[{}]/g,'\\$&'))})).toHaveCount(0);
    else await expect(page.getByRole('button',{name:english?'Удалить ролик…':'Delete video…',exact:true})).toHaveCount(0);
   }else{
    await page.evaluate(()=>window.libraryMessages.fail=false);
    await page.getByRole('button',{name:operation==='image-list'?(english?'Обновить изображения':'Refresh images'):(english?'Обновить ролики':'Refresh videos'),exact:true}).click();
    await expect(diagnostic).toHaveCount(0);expect(await page.evaluate(()=>window.libraryMessages.writes)).toEqual([]);
   }
  });
 }
}

test('rename readback failure keeps the old form blocked until explicit fresh selection',async({page})=>{
 await setup(page,'en-US','rename');await page.evaluate(()=>window.libraryMessages.readbackArmed=true);
 await page.getByRole('button',{name:`Rename ${name}`,exact:true}).click();await page.getByLabel('Image name',{exact:true}).fill('Pending');await page.getByRole('button',{name:'Save image name',exact:true}).click();
 await expect(page.getByRole('alert').filter({hasText:'Could not load images'})).toBeVisible();
 await expect(page.getByRole('button',{name:'Save image name',exact:true})).toBeDisabled();await expect(page.getByRole('button',{name:'Rename Pending',exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>window.libraryMessages.writes.length)).toBe(1);
 await page.evaluate(()=>{window.libraryMessages.readbackArmed=false;window.libraryMessages.readbackFailed=false;window.libraryMessages.fail=false;});
 await page.getByRole('button',{name:'Refresh images',exact:true}).click();await expect(page.getByRole('button',{name:'Rename Pending',exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Save image name',exact:true})).toBeDisabled();
 await page.getByRole('button',{name:'Rename Pending',exact:true}).click();await expect(page.getByRole('button',{name:'Save image name',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>window.libraryMessages.writes.length)).toBe(1);
});
test('successful video deletion has an English completion notice',async({page})=>{
 await setup(page,'en-US','video-delete');await page.evaluate(()=>window.libraryMessages.fail=false);
 await page.getByRole('button',{name:'Delete video…',exact:true}).click();await page.getByRole('button',{name:'Confirm video deletion',exact:true}).click();
 await expect(page.getByRole('status').filter({hasText:'Video deleted.'})).toBeVisible();expect(await page.evaluate(()=>window.libraryMessages.writes.length)).toBe(1);
});

for(const fail of [false,true])test(`late video editor snapshot after reconnect is discarded, failed=${fail}`,async({page})=>{
 await setup(page,'en-US','video-edit');const edit=page.getByRole('button',{name:'Edit selected video',exact:true});await expect(edit).toBeEnabled();
 await page.evaluate(()=>window.libraryMessages.holdCollection=true);await edit.click();await expect.poll(()=>page.evaluate(()=>!!window.libraryMessages.release)).toBe(true);
 await page.evaluate(()=>window.libraryMessages.online=false);await expect(page.getByText('Daemon disconnected',{exact:true})).toBeVisible();
 await page.evaluate(()=>window.libraryMessages.online=true);await page.getByRole('button',{name:'Reconnect',exact:true}).click();await expect(page.getByText('Daemon connected',{exact:true})).toBeVisible();
 await page.evaluate(fail=>window.libraryMessages.release!(fail),fail);await expect(edit).toBeEnabled();
 await expect(page.getByRole('region',{name:'Saved video editor',exact:true})).toHaveCount(0);await expect(page.getByRole('alert').filter({hasText:'Obsolete video editor failure'})).toHaveCount(0);
 expect(await page.evaluate(()=>window.libraryMessages.writes)).toEqual([]);
});
