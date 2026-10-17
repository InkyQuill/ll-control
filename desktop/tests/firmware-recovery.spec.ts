import {test,expect,type Page} from '@playwright/test';
const guard={settings_key:'0416_7395_serial_5245434f56455259',operation_id:'a'.repeat(32),image_sha256:'b'.repeat(64)};
const saved=(stage='unknown')=>({...guard,stage,vid:0x0416,pid:0x7395,serial:'raw <RECOVERY> 日本語',previous_hid_path:'/mock/previous-path',progress:['armed','effects_possible'].includes(stage)?null:stage==='not_started'?{total_chunks:0,chunk_writes:0,chunk_echoes:0,pump_hold_written:false,fan_hold_written:false,activation_written:false}:{total_chunks:619,chunk_writes:stage==='awaiting_reconnect'?619:100,chunk_echoes:stage==='awaiting_reconnect'?619:99,pump_hold_written:true,fan_hold_written:false,activation_written:stage==='awaiting_reconnect'},diagnostic:'Raw <b>device</b> detail\nline2'});
interface Fixture {response:unknown;fail:boolean;hold:boolean;pending:(()=>void)[];sealed:boolean;calls:{method:string;params:unknown}[]}
async function setup(page:Page,response:unknown,language='en-US',sealed=false){
 await page.addInitScript(({response,language,sealed})=>{
  localStorage.setItem('lconnect.interface-language',language);
  const f:Fixture={response,fail:false,hold:false,pending:[],sealed,calls:[]};
  Object.assign(window,{recoveryFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
   if(command!=='daemon_call')return null;
   const method=String(args.method);f.calls.push({method,params:structuredClone(args.params)});
   if(method==='get_firmware_recovery'){
    const reply=structuredClone(f.response);if(f.hold)await new Promise<void>(resolve=>f.pending.push(resolve));if(f.fail)throw {message:'private read unavailable'};return reply;
   }
   if(f.sealed)throw {message:'ordinary work admission closed for firmware maintenance'};
   switch(method){
    case 'get_display_settings':return {version:1,revision:0,interface_language:language,temperature_unit:'celsius'};
    case 'get_fps_source':return {version:1,revision:0,enabled:false,log_path:null};
    case 'list_devices':case 'list_sensors':case 'list_prepared_clips':return [];
    default:return null;
   }
  }}});
 },{response,language,sealed});
 await page.goto('/?view=display');await page.getByRole('button',{name:language==='en-US'?'Settings':'Настройки',exact:true}).click();
}
const card=(page:Page,language='en-US')=>page.getByRole('region',{name:language==='en-US'?'Saved firmware state':'Сохранённое состояние прошивки',exact:true});
const read=(page:Page,language='en-US')=>card(page,language).getByRole('button',{name:language==='en-US'?'Read saved firmware state':'Прочитать сохранённое состояние прошивки',exact:true});
const stateCases=[
 ['no_saved_operation',null,[],'No saved firmware journal or guards were found.'],
 ['journal_only',saved('armed'),[],'An unresolved firmware journal exists.'],
 ['guarded',saved(),[guard],'A saved firmware operation has quarantined settings replay.'],
 ['guard_only',null,[guard],'A settings guard exists without a firmware journal.'],
 ['inconsistent',saved(),[{...guard,operation_id:'c'.repeat(32)}],'The firmware journal and settings guards disagree.'],
 ['unreadable',null,[],'Saved firmware state could not be read.']
] as const;
for(const [state,operation,guards,label] of stateCases)test(`saved ${state} is inspectable with no device and sealed admission`,async({page})=>{
 await setup(page,{state,operation,guards,diagnostic:'raw diagnostic'},'en-US',true);
 await expect(read(page)).toBeEnabled();
 expect(await page.evaluate(()=>(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.calls.filter(call=>call.method==='get_firmware_recovery'))).toHaveLength(0);
 await read(page).click();await expect(card(page).getByRole(state==='no_saved_operation'?'status':'alert')).toContainText(label);
 if(operation){await expect(card(page)).toContainText(operation.serial);await expect(card(page)).toContainText('0416:7395');if(operation.progress)await expect(card(page)).toContainText('Saved transfer markers');}
 await card(page).getByText('Saved firmware diagnostics',{exact:true}).click();
 const raw=JSON.parse(await card(page).getByRole('textbox',{name:'Saved firmware diagnostics',exact:true}).inputValue());expect(raw).toMatchObject({state,operation,guards});
 const calls=await page.evaluate(()=>(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.calls);
 expect(calls.filter(call=>call.method==='get_firmware_recovery')).toEqual([{method:'get_firmware_recovery',params:{}}]);
 expect(calls.some(call=>/^(set_|clear_|firmware_|get_firmware$)/.test(call.method))).toBe(false);
});
for(const stage of ['armed','effects_possible','not_started','unknown','awaiting_reconnect'] as const)test(`saved ${stage} stays a historical marker in Russian`,async({page})=>{
 await setup(page,{state:'guarded',operation:saved(stage),guards:[guard],diagnostic:null},'ru-RU',true);
 await read(page,'ru-RU').click();await expect(card(page,'ru-RU').getByRole('alert')).toContainText('Сохранённая операция обновления прошивки');
 const stageLabel={armed:'операция подготовлена',effects_possible:'команды могли изменить устройство',not_started:'передача не начата',unknown:'исход неизвестен',awaiting_reconnect:'ожидание переподключения'}[stage]!;
 await expect(card(page,'ru-RU')).toContainText('Сохранено: '+stageLabel);
 if(saved(stage).progress){await expect(card(page,'ru-RU')).toContainText('Сохранённые отметки передачи');await expect(card(page,'ru-RU')).toContainText('Отметки могут отставать');}else await expect(card(page,'ru-RU').getByRole('heading',{name:'Сохранённые отметки передачи'})).toHaveCount(0);
 await expect(card(page,'ru-RU')).toContainText('raw <RECOVERY> 日本語');
 await expect(card(page,'ru-RU').locator('b')).toHaveCount(0);
});
test('read failure and unsupported response erase old snapshot; retry is explicit',async({page})=>{
 await setup(page,{state:'guarded',operation:saved(),guards:[guard],diagnostic:null});await read(page).click();await expect(card(page)).toContainText(guard.operation_id);
 await page.evaluate(()=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.fail=true;});await read(page).click();
 await expect(card(page).getByRole('alert')).toContainText('Could not read');await expect(card(page)).not.toContainText(guard.operation_id);
 await page.evaluate(()=>{const f=(window as unknown as {recoveryFixture:Fixture}).recoveryFixture;f.fail=false;f.response={state:'no_saved_operation',operation:{},guards:[],diagnostic:null};});await read(page).click();
 await expect(card(page).getByRole('alert')).toContainText('unsupported saved firmware state');
 await page.evaluate(()=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.response={state:'no_saved_operation',operation:null,guards:[],diagnostic:null};});
 await read(page).click();await expect(card(page).getByRole('status')).toContainText('No saved firmware journal');
});
test('leaving Settings discards a late reply and repeated activation does not queue another read',async({page})=>{
 await setup(page,{state:'guard_only',operation:null,guards:[guard],diagnostic:null});
 await page.evaluate(()=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.hold=true;});await read(page).click();await read(page).evaluate(button=>(button as HTMLButtonElement).click());
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.pending.length)).toBe(1);
 await page.getByRole('button',{name:'Sensors',exact:true}).click();await page.getByRole('button',{name:'Settings',exact:true}).click();
 await page.evaluate(()=>{const f=(window as unknown as {recoveryFixture:Fixture}).recoveryFixture;f.hold=false;f.pending.splice(0).forEach(release=>release());});
 await expect(card(page).getByRole('status')).toContainText('has not been read');await expect(card(page)).not.toContainText(guard.operation_id);
 await read(page).click();await expect(card(page).getByRole('alert')).toContainText('without a firmware journal');
});
test('unknown enums, inconsistent guards and non-integer counters never show a trusted snapshot',async({page})=>{
 await setup(page,{state:'new_daemon_state',operation:null,guards:[],diagnostic:null});
 for(const response of [
  {state:'new_daemon_state',operation:null,guards:[],diagnostic:null},
  {state:'guarded',operation:saved('new_stage'),guards:[guard],diagnostic:null},
  {state:'guarded',operation:saved(),guards:[{...guard,operation_id:'c'.repeat(32)}],diagnostic:null},
  {state:'guarded',operation:{...saved(),progress:{...saved().progress,chunk_echoes:1.5}},guards:[guard],diagnostic:null},
  {state:'guard_only',operation:null,guards:Array.from({length:257},()=>guard),diagnostic:null}
 ]){
  await page.evaluate(response=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.response=response;},response);
  await read(page).click();await expect(card(page).getByRole('alert')).toContainText('unsupported saved firmware state');await expect(card(page).locator('dl')).toHaveCount(0);
 }
});


test('ordinary connection loss discards a pending snapshot but sealed inspection remains available',async({page})=>{
 await setup(page,{state:'guarded',operation:saved(),guards:[guard],diagnostic:null});
 await expect(page.getByText('Daemon connected',{exact:true})).toBeVisible();
 await page.evaluate(()=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.hold=true;});await read(page).click();
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.pending.length)).toBe(1);
 await page.evaluate(()=>{(window as unknown as {recoveryFixture:Fixture}).recoveryFixture.sealed=true;});
 await expect(card(page).getByRole('status')).toContainText('has not been read yet');
 await page.evaluate(()=>{const f=(window as unknown as {recoveryFixture:Fixture}).recoveryFixture;f.hold=false;f.pending.splice(0).forEach(release=>release());f.response={state:'guard_only',operation:null,guards:[{settings_key:'guard-key',operation_id:'c'.repeat(32),image_sha256:'d'.repeat(64)}],diagnostic:null};});
 await expect(card(page).locator('dl')).toHaveCount(0);await read(page).click();
 await expect(card(page).getByRole('alert')).toContainText('without a firmware journal');await expect(card(page)).toContainText('guard-key');
});
