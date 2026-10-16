import {test,expect,type Page} from '@playwright/test';
import {firmwareInfo} from '../src/lib/firmware-info';

test('firmware response parsing preserves device data, accepts legacy missing date and rejects malformed values',()=>{
 const base={version:'1.6',raw_token:'Настройки {1} <CPU>'};
 expect(firmwareInfo(base)).toEqual({...base,build_datetime:null});
 expect(firmwareInfo({...base,build_datetime:'Apr 12 2024 14:33:21',future:true})).toEqual({...base,build_datetime:'Apr 12 2024 14:33:21'});
 for(const bad of [null,[],{}, {...base,version:0},{...base,version:''},{...base,raw_token:'x'.repeat(1025)},{...base,build_datetime:0},{...base,build_datetime:'bad\0date'}])expect(firmwareInfo(bad)).toBeNull();
});
interface Fixture {language:string;offline:boolean;devices:string[];hold:boolean;fail:boolean;response:unknown;calls:{method:string;params:Record<string,unknown>}[];pending:{dev:string;resolve:(value:unknown)=>void}[]}
async function setup(page:Page){
 await page.addInitScript(()=>{
  localStorage.setItem('lconnect.interface-language','en-US');
  const f={language:'en-US',offline:false,devices:['A','B'],hold:false,fail:false,response:{version:'1.6',raw_token:'Настройки {1} <CPU>',build_datetime:'Apr 12 2024 14:33:21'},calls:[] as {method:string;params:Record<string,unknown>}[],pending:[] as {dev:string;resolve:(value:unknown)=>void}[]};
  Object.assign(window,{firmwareFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   if(command!=='daemon_call')return null;
   const method=String(args.method),params=(args.params??{}) as Record<string,unknown>;f.calls.push({method,params});
   if(f.offline)throw {message:'Fixture daemon offline'};
   switch(method){
    case 'list_devices':return f.devices.map(id=>({id,display_name:id==='A'?'Настройки':'LCD B'}));
    case 'list_sensors':case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];
    case 'get_lcd_state':return {source:'Idle',requested:{mode:'LocalUI',brightness_percent:100,rotation_degrees:0},last_error:null};
    case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:f.language};
    case 'get_firmware':if(f.hold)return new Promise(resolve=>f.pending.push({dev:String(params.dev),resolve}));if(f.fail)throw {message:'Настройки {1} <CPU>'};return structuredClone(f.response);
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    default:return null;
   }
  }}});
 });
 await page.goto('/');await expect(page.getByText('Daemon connected',{exact:true})).toBeVisible();
 await page.getByLabel('Name',{exact:true}).fill('Настройки');
 await page.getByRole('button',{name:'Settings',exact:true}).click();
}
const panel=(page:Page)=>page.getByRole('region',{name:'Firmware information',exact:true});
const read=(page:Page)=>panel(page).getByRole('button',{name:'Read firmware information',exact:true});
const calls=(page:Page)=>page.evaluate(()=>(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.calls.filter(c=>c.method==='get_firmware'));
const unchanged=async(page:Page)=>expect(await page.evaluate(()=>(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.calls.filter(c=>/^(set_|patch_|clear_|update_|reset_|edit_)/.test(c.method)))).toEqual([]);

test('explicit read has no polling, stays on EN/RU without translating data and preserves the LCD draft',async({page})=>{
 await setup(page);await page.waitForTimeout(1700);expect(await calls(page)).toEqual([]);
 await read(page).click();await expect(panel(page).locator('dd').first()).toHaveText('1.6');
 await expect(panel(page).locator('dd').last()).toHaveText('Apr 12 2024 14:33:21');
 await panel(page).getByText('Device identity',{exact:true}).click();await expect(panel(page).locator('pre')).toHaveText('Настройки {1} <CPU>');
 await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.language='ru-RU';});
 const russian=page.getByRole('region',{name:'Сведения о прошивке',exact:true});await expect(russian.getByText('Версия прошивки',{exact:true})).toBeVisible();
 await expect(russian.locator('pre')).toHaveText('Настройки {1} <CPU>');await page.waitForTimeout(1700);expect(await calls(page)).toHaveLength(1);
 await page.getByRole('button',{name:'Экран помпы',exact:true}).click();await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Настройки');await unchanged(page);
});
test('missing date is unavailable; failed and malformed reads clear previous values and never retry implicitly',async({page})=>{
 await setup(page);await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.response={version:'1.6',raw_token:'legacy'};});
 await read(page).click();await expect(panel(page).locator('dd').last()).toHaveText('Unavailable');
 await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.fail=true;});await read(page).click();
 await expect(panel(page).getByRole('alert')).toHaveText('Could not read firmware information. Retry explicitly.');await expect(panel(page).locator('dd')).toHaveCount(0);
 await panel(page).getByText('Technical diagnostics',{exact:true}).click();await expect(panel(page).locator('pre')).toHaveText('Настройки {1} <CPU>');
 await page.waitForTimeout(1700);expect(await calls(page)).toHaveLength(2);
 await page.evaluate(()=>{const f=(window as unknown as {firmwareFixture:Fixture}).firmwareFixture;f.fail=false;f.response={version:99,raw_token:'bad'};});await read(page).click();
 await expect(panel(page).getByRole('alert')).toHaveText('The daemon returned unsupported firmware information.');await expect(panel(page).locator('dd')).toHaveCount(0);await unchanged(page);
});
test('A to B to A selection and late finalizers cannot relabel a newer query or clear its busy state',async({page})=>{
 await setup(page);await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.hold=true;});
 await read(page).click();await expect(read(page)).toBeFocused();await page.keyboard.press('Space');expect(await calls(page)).toHaveLength(1);
 await page.getByRole('combobox',{name:'Device',exact:true}).selectOption('B');await read(page).click();
 await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.pending[0].resolve({version:'STALE A',raw_token:'old A'});});
 await expect(read(page)).toHaveAttribute('aria-disabled','true');await expect(panel(page).locator('dd')).toHaveCount(0);
 await page.getByRole('combobox',{name:'Device',exact:true}).selectOption('A');await read(page).click();
 await page.evaluate(()=>{const f=(window as unknown as {firmwareFixture:Fixture}).firmwareFixture;f.pending[1].resolve({version:'STALE B',raw_token:'old B'});});
 await expect(read(page)).toHaveAttribute('aria-disabled','true');
 await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.pending[2].resolve({version:'CURRENT',raw_token:'current A'});});
 await expect(panel(page).locator('dd').first()).toHaveText('CURRENT');expect((await calls(page)).map(c=>c.params.dev)).toEqual(['A','B','A']);await unchanged(page);
});
for(const loss of ['device','daemon'] as const){
 test(`late firmware result is ignored after ${loss} loss and return of the same device`,async({page})=>{
  await setup(page);await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.hold=true;});await read(page).click();
  await page.evaluate(loss=>{const f=(window as unknown as {firmwareFixture:Fixture}).firmwareFixture;if(loss==='device')f.devices=[];else f.offline=true;},loss);
  await expect(panel(page).getByRole('status')).toHaveText('Connect to the daemon and select an available device.');
  await expect(read(page)).toBeDisabled();await page.evaluate(()=>{const f=(window as unknown as {firmwareFixture:Fixture}).firmwareFixture;f.devices=['A','B'];f.offline=false;});
  await expect(read(page)).toBeEnabled();await read(page).click();
  await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.pending[0].resolve({version:'STALE',raw_token:'old'});});
  await expect(read(page)).toHaveAttribute('aria-disabled','true');await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.pending[1].resolve({version:'CURRENT',raw_token:'new'});});
  await expect(panel(page).locator('dd').first()).toHaveText('CURRENT');expect(await calls(page)).toHaveLength(2);await unchanged(page);
 });
}
test('leaving Settings discards the pending result and returning does not start another firmware read',async({page})=>{
 await setup(page);await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.hold=true;});await read(page).click();
 await page.getByRole('button',{name:'Pump screen',exact:true}).click();await page.getByRole('button',{name:'Settings',exact:true}).click();
 await page.evaluate(()=>{(window as unknown as {firmwareFixture:Fixture}).firmwareFixture.pending[0].resolve({version:'STALE',raw_token:'old'});});
 await expect(panel(page).getByRole('status')).toHaveText('Firmware information has not been read yet.');await expect(panel(page).locator('dd')).toHaveCount(0);expect(await calls(page)).toHaveLength(1);await unchanged(page);
});
