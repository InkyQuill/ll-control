import {test,expect,type Page} from '@playwright/test';
import type {GroupStatus} from '../src/lib/group-archive';
interface Fixture {job:GroupStatus|null;calls:{command:string;args:Record<string,unknown>}[];lost:boolean;lostFinish?:boolean}
const panel=(p:Page)=>p.getByText('Резервная копия группы настроек',{exact:true}).locator('..');
async function setup(page:Page,language='ru-RU'){
 await page.addInitScript(language=>{
  localStorage.setItem('lconnect.interface-language',language);
  const summary={version:1,slots:[{group:'screen',slot:7,name:'Ночной экран'}],lcd:{mode:'Application',brightness_percent:42,rotation_degrees:90},theme_name:'Часы и температура',media_key:null,fan_manual_percent:0,pump_manual:{percent:55,source:'mcu'},fan_led_mode:'static',pump_led_mode:'rainbow',cooling:{fan:{mode:'fixed',percent:0},pump:{mode:'motherboard',percent:55}},dynamic_rgb:null};
  const f={language,job:null as null|Record<string,unknown>,calls:[] as {command:string;args:Record<string,unknown>}[],lost:false,lostFinish:false};
  function committed(){const j=f.job!;j.phase='complete';j.readback={dev:j.dev,group:j.group,operation_id:j.id,receipt:'opaque durable receipt',selected_content_current:true,summary:j.summary??summary,runtime:{revision:'opaque runtime token',last_error:null,recovery_required:false}};}
  Object.assign(window,{groupFixture:f,commitGroup:committed,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   if(command.startsWith('group_archive_')){
    f.calls.push({command,args:structuredClone(args)});
    if(command==='group_archive_status')return structuredClone(f.job);
    if(command==='group_archive_start'){const r=args.request as Record<string,unknown>;f.job={...r,phase:r.mode==='export'?'downloading':'prepared',transferred:1024,total:1024,file_name:r.mode==='reset'?null:'backup.lcg',message:null,summary,readback:null,snapshot:{expected:'opaque original token'}};if(f.lost){f.lost=false;throw {message:'Start reply lost'};}return structuredClone(f.job);}
    if(command==='group_archive_action'){
     const r=args.request as Record<string,unknown>;
     if(args.action==='dismiss'){f.job=null;return null;}
     if(args.action==='recover'){f.job={...r,transferred:1024,total:1024,file_name:null,message:null,summary,readback:null};committed();}
     if(args.action==='finish'||args.action==='reapply'){committed();if(f.lostFinish){f.lostFinish=false;f.job!.phase='uncertain';f.job!.readback=null;throw {message:'Finish reply lost'};}}
     if(args.action==='cancel')f.job!.phase='cancelling';
     return structuredClone(f.job);
    }
   }
   switch(args.method){
    case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:f.language};
    case 'list_devices':return [{id:'test',display_name:'LCDV2'},{id:'other',display_name:'Other mock device'}];
    case 'list_sensors':return [{id:'cpu:test',label:'Test CPU',unit:'celsius',value:40,status:'valid',age_ms:0}];
    case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];
    case 'get_lcd_state':return {source:'Idle',requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'},last_error:null};
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    default:return null;
   }
  }}});
 },language);
 await page.goto('/');await expect(page.getByText(language==='en-US'?'Daemon connected':'Daemon подключён',{exact:true})).toBeVisible({timeout:15000});
 await page.getByRole('button',{name:language==='en-US'?'Settings':'Настройки',exact:true}).click();await page.getByText(language==='en-US'?'Settings group backup':'Резервная копия группы настроек',{exact:true}).click();
}
async function calls(p:Page,action:string){return p.evaluate(a=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.filter(c=>c.args.action===a),action);}
const choose=(p:Page)=>panel(p).getByRole('button',{name:'Выбрать архив группы…',exact:true});
test('inspection displays actual LCD values and zero PWM before explicit target-bound Apply; preserves editor',async({page})=>{
 await setup(page);await page.getByRole('button',{name:'Экран помпы',exact:true}).click();await page.getByLabel('Название',{exact:true}).fill('Мой черновик');await page.getByRole('button',{name:'Настройки',exact:true}).click();
 await choose(page).click();await expect(panel(page).getByText('LCD: 42% · 90° · Application',{exact:true})).toBeVisible();await expect(panel(page).getByText('Вентиляторы: 0%',{exact:true})).toBeVisible();expect(await calls(page,'finish')).toHaveLength(0);
 await panel(page).getByRole('button',{name:'Восстановить и применить группу',exact:true}).click();await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();expect((await calls(page,'finish'))[0].args.request).toMatchObject({dev:'test',group:'screen',mode:'import'});
 await page.getByRole('button',{name:'Экран помпы',exact:true}).click();await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Мой черновик');
});
test('export stays in Rust, has explicit fan+lighting target and reports completion',async({page})=>{
 await setup(page);await panel(page).getByLabel('Группа настроек',{exact:true}).selectOption('fan_and_lighting');await panel(page).getByRole('button',{name:'Экспортировать группу…',exact:true}).click();await expect(panel(page).getByRole('progressbar')).toBeVisible();
 const start=await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.find(c=>c.command==='group_archive_start')!.args);
 expect(Object.keys(start).sort()).toEqual(['language','request']);expect(start.language).toBe('ru-RU');expect(start.request).toMatchObject({dev:'test',group:'fan_and_lighting',mode:'export'});expect(JSON.stringify(start)).not.toContain('bytes');
 await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.job!.phase='complete');await expect(panel(page).getByText('Архив группы сохранён в файл.',{exact:true})).toBeVisible();
});
test('prepared import cannot be dismissed, cancelled worker remains busy until acknowledgement',async({page})=>{
 await setup(page);await choose(page).click();await expect(panel(page).getByRole('button',{name:'Закрыть сведения о группе',exact:true})).toBeDisabled();await panel(page).getByRole('button',{name:'Отменить передачу группы',exact:true}).click();await expect(panel(page).getByRole('progressbar')).toBeVisible();await expect(choose(page)).toBeDisabled();await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.job!.phase='cancelled');await expect(panel(page).getByRole('button',{name:'Закрыть сведения о группе',exact:true})).toBeEnabled();await panel(page).getByRole('button',{name:'Закрыть сведения о группе',exact:true}).click();await expect(choose(page)).toBeEnabled();
});
test('lost Start response recovers preview by polling without automatic Finish or duplicate upload',async({page})=>{
 await setup(page);await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.lost=true);await choose(page).click();await expect(panel(page).getByRole('button',{name:'Восстановить и применить группу',exact:true})).toBeVisible();expect(await calls(page,'finish')).toHaveLength(0);expect(await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.filter(c=>c.command==='group_archive_start').length)).toBe(1);
});
test('stored ID/device/group survives reload and readback never repeats Finish',async({page})=>{
 await setup(page);await choose(page).click();const pending=await page.evaluate(()=>JSON.parse(localStorage.getItem('lconnect.group.pending')!));await page.reload();await page.getByRole('button',{name:'Настройки',exact:true}).click();await page.getByText('Резервная копия группы настроек',{exact:true}).click();await expect(panel(page).getByLabel('ID операции группы',{exact:true})).toHaveValue(pending.id);
 await panel(page).getByRole('button',{name:'Прочитать результат группы',exact:true}).click();await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();expect(await calls(page,'finish')).toHaveLength(0);expect((await calls(page,'recover'))[0].args.request).toEqual(pending);
});
test('reapply echoes opaque revision and a later content change disables another Apply',async({page})=>{
 await setup(page);await choose(page).click();await panel(page).getByRole('button',{name:'Восстановить и применить группу',exact:true}).click();await panel(page).getByRole('button',{name:'Повторить применение сохранённой группы',exact:true}).click();expect((await calls(page,'reapply'))[0].args.expected).toBe('opaque runtime token');
 await page.evaluate(()=>{const j=(window as unknown as {groupFixture:Fixture}).groupFixture.job!;j.readback!.selected_content_current=false;});await expect(panel(page).getByRole('button',{name:'Повторить применение сохранённой группы',exact:true})).toBeDisabled();
});
test('receipt with Apply error distinguishes committed settings from runtime recovery',async({page})=>{
 await setup(page);await choose(page).click();await panel(page).getByRole('button',{name:'Восстановить и применить группу',exact:true}).click();await page.evaluate(()=>{const j=(window as unknown as {groupFixture:Fixture}).groupFixture.job!;j.readback!.runtime.last_error='Mock LCD frame failure';j.readback!.runtime.recovery_required=true;});await expect(panel(page).getByRole('alert').filter({hasText:'Mock LCD frame failure'})).toBeVisible();await expect(panel(page).getByRole('button',{name:'Повторить применение сохранённой группы',exact:true})).toBeDisabled();
});
test('another persisted operation cannot be replaced by a new Start',async({page})=>{
 await setup(page);await page.evaluate(()=>localStorage.setItem('lconnect.group.pending',JSON.stringify({id:'b'.repeat(32),dev:'other-device',group:'screen',mode:'import'})));await choose(page).click();await expect(panel(page).getByRole('alert')).toContainText('Сначала прочитайте');expect(await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.filter(c=>c.command==='group_archive_start').length)).toBe(0);
});

test('changing selected device after Inspect cannot redirect Apply to another device',async({page})=>{
 await setup(page);await choose(page).click();await page.getByRole('combobox',{name:'Устройство',exact:true}).selectOption('other');await expect(panel(page).getByRole('button',{name:'Восстановить и применить группу',exact:true})).toBeDisabled();expect(await calls(page,'finish')).toHaveLength(0);await expect(panel(page).getByText('test',{exact:true})).toBeVisible();
});


test('full reset requires temperature selection and separate confirmation; preview cancel never applies',async({page})=>{
 await setup(page);await panel(page).getByRole('combobox',{name:'Группа настроек',exact:true}).selectOption('fan_and_lighting');
 const start=panel(page).getByRole('button',{name:'Подготовить полный сброс охлаждения и подсветки…',exact:true});
 await expect(start).toBeDisabled();
 await panel(page).getByRole('combobox',{name:'Температурный датчик CPU для сброса',exact:true}).selectOption('cpu:test');
 await start.click();await expect(panel(page).getByText('Сброс подготовлен, ожидает подтверждения',{exact:true})).toBeVisible();
 const admitted=await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.filter(c=>c.command==='group_archive_start'));
 expect(admitted).toHaveLength(1);expect(admitted[0].args.request).toMatchObject({mode:'reset',group:'fan_and_lighting',sensor_id:'cpu:test'});
 expect(await calls(page,'finish')).toHaveLength(0);
 await panel(page).getByRole('button',{name:'Отменить подготовку сброса',exact:true}).click();
 expect(await calls(page,'finish')).toHaveLength(0);
});

test('full reset readback survives reload without repeating reset; newer content disables reapply',async({page})=>{
 await setup(page);await panel(page).getByRole('combobox',{name:'Группа настроек',exact:true}).selectOption('fan_and_lighting');
 await panel(page).getByRole('combobox',{name:'Температурный датчик CPU для сброса',exact:true}).selectOption('cpu:test');
 await panel(page).getByRole('button',{name:'Подготовить полный сброс охлаждения и подсветки…',exact:true}).click();
 await panel(page).getByRole('button',{name:'Подтвердить полный сброс и применить',exact:true}).click();
 await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
 const pending=await page.evaluate(()=>JSON.parse(localStorage.getItem('lconnect.group.pending')!));expect(pending.mode).toBe('reset');
 await page.reload();await page.getByRole('button',{name:'Настройки',exact:true}).click();await page.getByText('Резервная копия группы настроек',{exact:true}).click();
 await panel(page).getByRole('button',{name:'Прочитать результат группы',exact:true}).click();
 await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();expect(await calls(page,'finish')).toHaveLength(0);
 await page.evaluate(()=>{(window as unknown as {groupFixture:Fixture}).groupFixture.job!.readback!.selected_content_current=false;});
 await expect(panel(page).getByRole('button',{name:'Повторить применение сохранённой группы',exact:true})).toBeDisabled();
});


test('lost reset reply offers only explicit retry with the original operation identity',async({page})=>{
 await setup(page);await panel(page).getByRole('combobox',{name:'Группа настроек',exact:true}).selectOption('fan_and_lighting');
 await panel(page).getByRole('combobox',{name:'Температурный датчик CPU для сброса',exact:true}).selectOption('cpu:test');
 await panel(page).getByRole('button',{name:'Подготовить полный сброс охлаждения и подсветки…',exact:true}).click();
 await page.evaluate(()=>{(window as unknown as {groupFixture:Fixture}).groupFixture.lostFinish=true;});
 await panel(page).getByRole('button',{name:'Подтвердить полный сброс и применить',exact:true}).click();
 const retry=panel(page).getByRole('button',{name:'Повторить запрос сброса с тем же ID',exact:true});
 await expect(retry).toBeVisible();expect(await calls(page,'finish')).toHaveLength(1);
 await expect(panel(page).getByRole('button',{name:'Отменить подготовку сброса',exact:true})).toHaveCount(0);
 await retry.click();await expect(panel(page).getByText(/Сохранение группы подтверждено/)).toBeVisible();
 const sent=await calls(page,'finish');expect(sent).toHaveLength(2);expect(sent[0].args.request).toEqual(sent[1].args.request);
});

test('English group phases and parameterized curves switch to Russian without translating stored names or applying',async({page})=>{
 await setup(page,'en-US');
 const english=page.getByText('Settings group backup',{exact:true}).locator('..');
 await english.getByRole('button',{name:'Choose group archive…',exact:true}).click();
 await expect(english.getByRole('status').filter({hasText:'Archive checked; waiting for Apply'})).toBeVisible();
 await page.evaluate(()=>{const f=(window as unknown as {groupFixture:Fixture}).groupFixture;const summary=f.job!.summary!;summary.slots[0].name='Настройки';summary.theme_name='Экран';summary.cooling={fan:{mode:'curve',sensor_id:'Настройки {1} <CPU>',points:[{celsius:30,percent:40},{celsius:70,percent:100}],hysteresis_c:2,fallback_percent:100},pump:null};});
 await expect(english.getByText('Fan policy: PWM curve · Настройки {1} <CPU> · points: 2',{exact:true})).toBeVisible();
 await expect(english.getByText('Pump policy: not configured',{exact:true})).toBeVisible();
 await expect(english.getByText('screen · 8: Настройки',{exact:true})).toBeVisible();
 await expect(english.getByText('Theme: Экран',{exact:true})).toBeVisible();
 await page.evaluate(()=>{const f=(window as unknown as {groupFixture:Fixture&{language:string}}).groupFixture;f.job!.phase='downloading';f.language='ru-RU';});
 await expect(page.locator('html')).toHaveAttribute('lang','ru');
 await expect(panel(page).getByRole('status').filter({hasText:'Сохранение файла'})).toBeVisible();
 await expect(panel(page).getByRole('progressbar',{name:'Передача группы',exact:true})).toBeVisible();
 await expect(panel(page).getByText('Политика вентиляторов: Кривая PWM · Настройки {1} <CPU> · точек: 2',{exact:true})).toBeVisible();
 expect(await calls(page,'finish')).toHaveLength(0);expect(await calls(page,'reapply')).toHaveLength(0);
 const startArgs=await page.evaluate(()=>(window as unknown as {groupFixture:Fixture}).groupFixture.calls.find(c=>c.command==='group_archive_start')!.args);
 expect(startArgs).toMatchObject({language:'en-US'});
});

test('typed group action rejection preserves raw diagnostic and never infers notice codes from text',async({page})=>{
 await setup(page,'en-US');const english=page.getByText('Settings group backup',{exact:true}).locator('..');
 await english.getByRole('button',{name:'Choose group archive…',exact:true}).click();
 await page.evaluate(()=>{
  const w=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args?:Record<string,unknown>)=>Promise<unknown>}};
  const original=w.__TAURI_INTERNALS__.invoke;
  w.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command==='group_archive_action')throw {message:'Настройки {1} <CPU>',message_code:'group_export_unconfirmed'};return original(command,args);};
 });
 await english.getByRole('button',{name:'Restore and apply group',exact:true}).click();
 await expect(english.getByRole('alert').filter({hasText:'The local file is unconfirmed. Cancel the transfer and export again.'})).toBeVisible();
 await english.getByText('Technical diagnostics',{exact:true}).click();await expect(english.locator('pre')).toHaveText('Настройки {1} <CPU>');
 await page.evaluate(()=>{const j=(window as unknown as {groupFixture:Fixture}).groupFixture.job!;j.phase='failed';j.message='Настройки';j.message_code='future_code';});
 await expect(english.getByRole('alert').filter({hasText:'Настройки'})).toBeVisible();
 expect(await calls(page,'finish')).toHaveLength(0);
});
