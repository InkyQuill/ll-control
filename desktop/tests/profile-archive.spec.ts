import {test,expect,type Page} from '@playwright/test';
import type {ArchiveStatus} from '../src/lib/profile-archive';
interface Fixture {job:ArchiveStatus|null;calls:{command:string;args:unknown}[];writes:string[];failStart:boolean}
async function setup(page:Page,language='ru-RU'){
 await page.addInitScript(language=>{
  localStorage.setItem('lconnect.interface-language',language);
  const profile=(slot:number,name:string)=>({slot,name,content:{kind:'screen',value:{version:1,name:'Stored draft',background:[0,0,0],layers:[]}}});
  let library={version:1,revision:1,profiles:[profile(0,'Saved screen')]};
  const f={language,job:null as null|Record<string,unknown>,calls:[] as {command:string;args:unknown}[],writes:[] as string[],failStart:false};
  const complete=()=>{const j=f.job!;const imported=profile(j.slot as number,'Imported screen');if(j.mode==='import')library={version:1,revision:2,profiles:[...library.profiles.filter(p=>p.slot!==j.slot),imported]};j.phase='complete';j.transferred=1000;j.total=1000;j.result={state:{id:j.id,direction:j.mode,group:j.group,slot:j.slot,expected_revision:1,phase:j.mode==='import'?'imported':'ready',size:1000,received:j.mode==='import'?1000:0,profile:j.mode==='import'?imported:null,library:j.mode==='import'?structuredClone(library):null,last_error:null,committed_revision:j.mode==='import'?2:null},eof:false,bytes:[]};};
  Object.assign(window,{archiveFixture:f,completeArchive:complete,__TAURI_INTERNALS__:{invoke:async(command:string,args:{method?:string;request?:{id:string;mode:string;group:string;slot:number};id?:string;group?:string;slot?:number})=>{
        if (command === 'desktop_set_language') return null;
   if(command.startsWith('profile_archive_')){
    f.calls.push({command,args:structuredClone(args)});
    if(command==='profile_archive_status')return structuredClone(f.job);
    if(command==='profile_archive_start'){const r=args.request!;f.job={id:r.id,mode:r.mode,group:r.group,slot:r.slot,phase:r.mode==='import'?'uploading':'downloading',transferred:100,total:1000,file_name:'portable.lcp',message:null,result:null};if(f.failStart){f.failStart=false;throw {message:'Start reply lost'};}return structuredClone(f.job);}
    if(command==='profile_archive_cancel'){f.job!.phase='cancelling';return structuredClone(f.job);}
    if(command==='profile_archive_dismiss'){f.job=null;return null;}
    if(command==='profile_archive_recover'){f.job={id:args.id,mode:'import',group:args.group,slot:args.slot,phase:'recovering',transferred:0,total:0,file_name:null,message:null,result:null};complete();return structuredClone(f.job);}
   }
   if(args.method?.startsWith('set_')||args.method==='edit_profile')f.writes.push(args.method);
   switch(args.method){case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:f.language};case 'list_devices':return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'}];case 'list_sensors':return [];case 'get_lcd_state':return {source:'Idle',last_error:null,requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'}};case 'list_profiles':return structuredClone(library);case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];default:return null;}
  }}});
 },language);
 await page.goto('/?view=display');await expect(page.getByText(language==='en-US'?'Daemon connected':'Daemon подключён',{exact:true})).toBeVisible();await open(page,language);
}
async function open(page:Page,language='ru-RU'){await page.getByText(language==='en-US'?'Saved profiles':'Сохранённые профили',{exact:true}).click();await page.getByRole('button',{name:language==='en-US'?'Refresh profile list':'Обновить список профилей',exact:true}).click();await page.getByText(language==='en-US'?'Profile import and export':'Импорт и экспорт профиля',{exact:true}).click();}
const panel=(page:Page)=>page.getByText('Импорт и экспорт профиля',{exact:true}).locator('..');
const start=(page:Page)=>panel(page).getByRole('button',{name:'Импортировать архив в слот…',exact:true});
const count=(page:Page,command:string)=>page.evaluate(c=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.calls.filter(x=>x.command===c).length,command);
const complete=(page:Page)=>page.evaluate(()=>(window as unknown as {completeArchive:()=>void}).completeArchive());
test('export delegates files to native worker and reports progress without applying',async({page})=>{
 await setup(page);await panel(page).getByRole('button',{name:'Экспортировать слот в архив…',exact:true}).click();await expect(panel(page).getByRole('progressbar')).toHaveAttribute('value','100');
 const args=await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.calls.find(c=>c.command==='profile_archive_start')!.args);expect(args).toMatchObject({request:{mode:'export',group:'screen',slot:0,expectedRevision:1,suggestedName:'Saved screen.lcp'}});
 await complete(page);await expect(panel(page).getByRole('status').filter({hasText:'Архив сохранён в файл.'})).toBeVisible();expect(await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.writes)).toEqual([]);expect(await page.evaluate(()=>localStorage.getItem('lconnect.archive.pending'))).toBeNull();
});
test('import changes chosen slot only after completion and keeps editor draft without Apply',async({page})=>{
 await setup(page);await page.getByLabel('Название',{exact:true}).fill('Keep current draft');await page.getByRole('combobox',{name:'Слот профиля',exact:true}).selectOption('7');await start(page).click();await expect(panel(page).getByRole('progressbar')).toBeVisible();await expect(page.getByRole('button',{name:'Сохранить черновик в слот',exact:true})).toBeDisabled();await complete(page);await expect(page.getByRole('combobox',{name:'Слот профиля',exact:true})).toContainText('Imported screen');await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Keep current draft');expect(await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.writes)).toEqual([]);
});
test('Cancel keeps controls busy until native worker confirms termination',async({page})=>{
 await setup(page);await start(page).click();await panel(page).getByRole('button',{name:'Отменить передачу архива',exact:true}).click();await expect(panel(page).getByRole('status').filter({hasText:'Отмена передачи'})).toBeVisible();await expect(start(page)).toBeDisabled();await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.job!.phase='cancelled');await expect(start(page)).toBeEnabled();expect(await count(page,'profile_archive_cancel')).toBe(1);
});
test('lost Start reply recovers same job without a duplicate Start',async({page})=>{
 await setup(page);await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.failStart=true);await start(page).click();await expect(panel(page).getByRole('progressbar')).toBeVisible();await complete(page);await expect(panel(page).getByRole('status').filter({hasText:'Импорт подтверждён'})).toBeVisible();expect(await count(page,'profile_archive_start')).toBe(1);
});
test('pending ID survives reload and explicit readback imports no second copy',async({page})=>{
 await setup(page);await start(page).click();await expect(panel(page).getByRole('progressbar')).toBeVisible();const pending=await page.evaluate(()=>JSON.parse(localStorage.getItem('lconnect.archive.pending')!));await page.reload();await open(page);await expect(panel(page).getByRole('textbox',{name:'ID операции для восстановления'})).toHaveValue(pending.id);await panel(page).getByRole('button',{name:'Прочитать результат архива',exact:true}).click();await expect(panel(page).getByRole('status').filter({hasText:'Импорт подтверждён'})).toBeVisible();expect(await count(page,'profile_archive_start')).toBe(0);
});
test('another panel cannot overwrite the stored pending operation',async({page})=>{
 await setup(page);await page.evaluate(()=>localStorage.setItem('lconnect.archive.pending',JSON.stringify({id:'b'.repeat(32),group:'screen',slot:7})));await start(page).click();expect(await count(page,'profile_archive_start')).toBe(0);await expect(panel(page).getByRole('textbox',{name:'ID операции для восстановления'})).toHaveValue('b'.repeat(32));expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('lconnect.archive.pending')!).id)).toBe('b'.repeat(32));
});
test('commit cannot be cancelled or dismissed',async({page})=>{
 await setup(page);await start(page).click();await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.job!.phase='committing');await expect(panel(page).getByText('Завершение сохранения',{exact:false})).toBeVisible();await expect(panel(page).getByRole('button',{name:'Отменить передачу архива',exact:true})).toHaveCount(0);await expect(panel(page).getByRole('button',{name:'Закрыть сведения об архиве',exact:true})).toBeDisabled();
});

test('English archive progress and completed import notice remain reactive after switching to Russian',async({page})=>{
 await setup(page,'en-US');const english=page.getByText('Profile import and export',{exact:true}).locator('..');
 await page.getByLabel('Name',{exact:true}).fill('Настройки');
 await english.getByRole('button',{name:'Import archive into slot…',exact:true}).click();
 await expect(english.getByRole('status').filter({hasText:'Uploading archive · Screen · slot 1 · portable.lcp'})).toBeVisible();
 await complete(page);
 await expect(english.getByRole('status').filter({hasText:'Import confirmed: slot 1. Device settings have not been applied.'})).toBeVisible();
 await expect(page.getByLabel('Name',{exact:true})).toHaveValue('Настройки');
 await page.evaluate(()=>{(window as unknown as {archiveFixture:Fixture&{language:string}}).archiveFixture.language='ru-RU';});
 await expect(page.locator('html')).toHaveAttribute('lang','ru');
 await expect(panel(page).getByRole('status').filter({hasText:'Импорт подтверждён: слот 1. Настройки устройства не применены.'})).toBeVisible();
 await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Настройки');
 expect(await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.writes)).toEqual([]);
 expect(await count(page,'profile_archive_start')).toBe(1);
 const startArgs=await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.calls.find(c=>c.command==='profile_archive_start')!.args);
 expect(startArgs).toMatchObject({language:'en-US'});
});

test('typed worker errors relabel on language switch while raw filename diagnostics stay literal',async({page})=>{
 await setup(page,'en-US');const english=page.getByText('Profile import and export',{exact:true}).locator('..');
 await english.getByRole('button',{name:'Import archive into slot…',exact:true}).click();
 await page.evaluate(()=>{const j=(window as unknown as {archiveFixture:Fixture}).archiveFixture.job!;j.phase='failed';j.message='Настройки {1} <CPU>.lcp';j.message_code='destination_exists';});
 await expect(english.getByRole('alert').filter({hasText:'The file already exists. Choose a new name.'})).toBeVisible();
 await english.getByText('Technical diagnostics',{exact:true}).click();
 await expect(english.locator('pre')).toHaveText('Настройки {1} <CPU>.lcp');
 expect(await english.locator('pre cpu').count()).toBe(0);
 await page.evaluate(()=>{(window as unknown as {archiveFixture:Fixture&{language:string}}).archiveFixture.language='ru-RU';});
 await expect(panel(page).getByRole('alert').filter({hasText:'Файл уже существует: выберите новое имя'})).toBeVisible();
 await expect(panel(page).locator('pre')).toHaveText('Настройки {1} <CPU>.lcp');
 expect(await count(page,'profile_archive_start')).toBe(1);
 expect(await page.evaluate(()=>(window as unknown as {archiveFixture:Fixture}).archiveFixture.writes)).toEqual([]);
});
