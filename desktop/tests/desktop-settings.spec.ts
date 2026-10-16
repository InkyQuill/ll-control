import {test,expect,type Page} from '@playwright/test';
interface Fixture {state:{revision:number;preferences:{closeToTray:boolean;startMinimized:boolean;autostart:boolean};registration:{present:boolean;enabled:boolean|null;managed:boolean;problem:string|null};integrationError:string|null;trayReady:boolean;trayError:string|null};reads:number;saves:{expectedRevision:number;preferences:Fixture['state']['preferences']}[];fail:boolean;exits:number}
async function setup(page:Page){
 await page.addInitScript(()=>{
  const state={revision:0,preferences:{closeToTray:true,startMinimized:false,autostart:false},registration:{present:false,enabled:false as boolean|null,managed:false,problem:null as string|null},integrationError:null as string|null,trayReady:false,trayError:'Tray host недоступен'};
  const f={state,reads:0,saves:[] as {expectedRevision:number;preferences:typeof state.preferences}[],fail:false,exits:0};
  Object.assign(window,{desktopFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   if(command==='desktop_settings_get'){f.reads++;return {...structuredClone(f.state),integrationError:null};}
   if(command==='desktop_settings_save'){
    const request=args as {expectedRevision:number;preferences:typeof state.preferences};f.saves.push(structuredClone(request));if(request.expectedRevision!==state.revision)throw {message:'stale desktop revision'};
    state.revision++;state.preferences=structuredClone(request.preferences);state.integrationError=f.fail?'Fixture registration failed':null;
    state.registration={present:state.preferences.autostart,enabled:f.fail?null:state.preferences.autostart,managed:!f.fail,problem:f.fail?'Fixture existing entry preserved':null};return structuredClone(state);
   }
   if(command==='desktop_exit'){f.exits++;return;}
   if(command==='group_archive_status')return null;
   switch(args.method){case 'list_devices':return [{id:'test',display_name:'LCDV2'}];case 'list_sensors':case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];case 'get_lcd_state':return {source:'Idle',requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'},last_error:null};case 'list_profiles':return {version:1,revision:0,profiles:[]};default:return null;}
  }}});
 });
 await page.goto('/');await expect(page.getByText('Daemon подключён',{exact:true})).toBeVisible({timeout:15000});await page.getByRole('button',{name:'Настройки',exact:true}).click();await page.getByText('Приложение и автозапуск',{exact:true}).click();
 await expect(page.getByRole('checkbox',{name:'Сворачивать в tray при закрытии окна',exact:true})).toBeVisible();
}
const checkbox=(page:Page)=>page.getByRole('checkbox',{name:'Запускать приложение при входе в систему',exact:true});
test('desktop drafts survive polling and save camelCase native preferences independently of daemon',async({page})=>{
 await setup(page);await checkbox(page).check();
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.reads)).toBeGreaterThanOrEqual(2);
 await expect(checkbox(page)).toBeChecked();expect(await page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.saves)).toHaveLength(0);
 await page.getByRole('button',{name:'Сохранить параметры приложения',exact:true}).click();
 await expect(page.getByText('Регистрация автозапуска: включена.',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.saves)).toEqual([{expectedRevision:0,preferences:{closeToTray:true,startMinimized:false,autostart:true}}]);
});
test('durable intent with failed registration is not reported enabled and explicit retry uses readback revision',async({page})=>{
 await setup(page);await page.evaluate(()=>{(window as unknown as {desktopFixture:Fixture}).desktopFixture.fail=true;});await checkbox(page).check();await page.getByRole('button',{name:'Сохранить параметры приложения',exact:true}).click();
 await expect(page.getByText('Регистрация автозапуска: состояние не подтверждено.',{exact:true})).toBeVisible();await expect(checkbox(page)).toBeChecked();await expect(page.getByRole('alert').filter({hasText:'Fixture registration failed'})).toBeVisible();
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.reads)).toBeGreaterThanOrEqual(2);await expect(page.getByRole('button',{name:'Сохранить и повторить регистрацию',exact:true})).toBeVisible();
 await page.evaluate(()=>{(window as unknown as {desktopFixture:Fixture}).desktopFixture.fail=false;});await page.getByRole('button',{name:'Сохранить и повторить регистрацию',exact:true}).click();await expect(page.getByText('Регистрация автозапуска: включена.',{exact:true})).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.saves.map(s=>s.expectedRevision))).toEqual([0,1]);
});
test('external settings revision disables stale save and preserves draft until explicit reload',async({page})=>{
 await setup(page);await checkbox(page).check();await page.evaluate(()=>{const f=(window as unknown as {desktopFixture:Fixture}).desktopFixture;f.state.revision++;f.state.preferences.closeToTray=false;});
 await expect(page.getByRole('alert').filter({hasText:'Настройки изменились в другом окне'})).toBeVisible();await expect(page.getByRole('button',{name:'Сохранить параметры приложения',exact:true})).toBeDisabled();await expect(checkbox(page)).toBeChecked();
 await page.getByRole('button',{name:'Обновить параметры приложения',exact:true}).click();await expect(checkbox(page)).not.toBeChecked();await expect(page.getByRole('button',{name:'Сохранить параметры приложения',exact:true})).toBeEnabled();
});
test('full Exit is explicit and locks further desktop writes while native shutdown joins workers',async({page})=>{
 await setup(page);await page.getByRole('button',{name:'Выйти из приложения полностью',exact:true}).click();await expect(page.getByText('Завершаем активные операции перед выходом…',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Сохранить параметры приложения',exact:true})).toBeDisabled();expect(await page.evaluate(()=>(window as unknown as {desktopFixture:Fixture}).desktopFixture.exits)).toBe(1);
});
