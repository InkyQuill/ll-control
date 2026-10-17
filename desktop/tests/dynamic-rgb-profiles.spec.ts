import {test,expect,type Page} from '@playwright/test';
interface Fixture {mode:string;empty:boolean;holdCpu:boolean;releaseCpu:null|(()=>void);writes:unknown[]}
async function setup(page:Page){
  await page.addInitScript(()=>{
    interface C {sensor_id:string;unit:string;low_value:number;high_value:number;low_colors:{r:number;g:number;b:number}[];high_colors:{r:number;g:number;b:number}[];low_speed_percent:number|null;high_speed_percent:number|null;brightness_percent?:number;direction?:string}
    const colors=(seed:number,n:number)=>Array.from({length:n},(_,i)=>({r:seed+i,g:seed+i+1,b:seed+i+2}));
    const binding=(id:string,unit:string,low:number,high:number,seed:number,n=4):C=>({sensor_id:id,unit,low_value:low,high_value:high,low_colors:colors(seed,n),high_colors:colors(seed+30,n),low_speed_percent:n===4?20:null,high_speed_percent:n===4?80:null,brightness_percent:seed===1?50:75,direction:n===4&&seed===1?'reverse':'forward'});
    const profiles={modes:[{mode:'meteor',bindings:[binding('cpu:temp','celsius',33,66,1),binding('gpu:load','percent',40,90,11)]},{mode:'static',bindings:[{...binding('pump:rpm','rpm',800,1200,21,1),brightness_percent:undefined,direction:undefined}]}]};
    const f={mode:'meteor',empty:false,holdCpu:false,releaseCpu:null as null|(()=>void),writes:[] as unknown[]};let active:C|null=null;
    const state=()=>({base_mode:f.mode,color_count:f.mode==='meteor'?4:1,speed_supported:f.mode==='meteor',direction_supported:f.mode==='meteor',requested:active,applied:null,degraded:false,suppressed_by_thermal:false,last_error:null,profiles:f.empty?{modes:[]}:profiles,mode_defaults:{color:{r:255,g:0,b:0},additional_colors:f.mode==='meteor'?[{r:0,g:0,b:255},{r:0,g:255,b:0},{r:255,g:255,b:0}]:[],speed_percent:f.mode==='meteor'?75:0,brightness_percent:100,direction:'forward'}});
    Object.assign(window,{dynamicProfiles:f,__TAURI_INTERNALS__:{invoke:async(_:string,req:{method:string;params?:{config:C|null}})=>{
        if (_ === 'desktop_set_language') return null;
      switch(req.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [['cpu:temp','celsius'],['gpu:load','percent'],['pump:rpm','rpm'],['cpu:load','percent']].map(([id,unit])=>({id,unit,label:id,status:'valid',value:45,age_ms:0}));
        case 'get_lighting_info':return {modes:['static','meteor']};
        case 'get_thermal_state':return {requested:{cpu:null,gpu:null},degraded_sources:[],fan:{last_error:null},pump:{last_error:null},grace_remaining_ms:0};
        case 'get_dynamic_rgb':return state();
        case 'get_sensor_defaults':if(f.holdCpu)await new Promise<void>(resolve=>f.releaseCpu=resolve);return {config:{cpu:{load:'cpu:load'}},candidates:[]};
        case 'set_dynamic_rgb':active=req.params!.config;f.writes.push(structuredClone(active));return state();
        default:return null;
      }
    }}});
  });
  await page.goto('/?view=display');await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await page.getByText('Подсветка помпы по датчику',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Загрузить настройки режима по датчику',exact:true})).toBeEnabled();
}
const load=(page:Page)=>page.getByRole('button',{name:'Загрузить настройки режима по датчику',exact:true});
const source=(page:Page)=>page.getByRole('combobox',{name:'Датчик подсветки',exact:true});
test('per-source ranges palettes brightness and direction restore without losing unapplied changes',async({page})=>{
  await setup(page);await load(page).click();await expect(source(page)).toHaveValue('gpu:load');
  await source(page).selectOption('cpu:temp');await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('33');
  await expect(page.getByRole('spinbutton',{name:'Яркость по датчику, %',exact:true})).toHaveValue('50');
  await expect(page.getByRole('combobox',{name:'Направление по датчику',exact:true})).toHaveValue('reverse');
  await page.getByLabel('Нижний порог (°C)').fill('45');await page.getByLabel('Нижний цвет 2',{exact:true}).fill('#112233');
  await source(page).selectOption('gpu:load');await expect(page.getByLabel('Нижний порог (percent)')).toHaveValue('40');
  await source(page).selectOption('cpu:temp');await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('45');await expect(page.getByLabel('Нижний цвет 2',{exact:true})).toHaveValue('#112233');
  await page.getByRole('button',{name:'Применить режим по датчику',exact:true}).click();
  expect(await page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.writes)).toMatchObject([{sensor_id:'cpu:temp',brightness_percent:50,direction:'reverse',low_value:45}]);
});
test('different mode and RPM source restore their own settings and legacy inheritance',async({page})=>{
  await setup(page);await load(page).click();
  await page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.mode='static');
  await expect(page.getByText('Режим помпы: static',{exact:false})).toBeVisible();await load(page).click();
  await expect(source(page)).toHaveValue('pump:rpm');await expect(page.getByLabel('Нижний порог (rpm)')).toHaveValue('800');
  await expect(page.getByLabel('Нижний цвет 2',{exact:true})).toHaveCount(0);
  await expect(page.getByRole('checkbox',{name:'Яркость обычного режима',exact:true})).toBeChecked();
  await source(page).selectOption('cpu:temp');await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('30');
  await expect(page.getByLabel('Нижний цвет 1',{exact:true})).toHaveValue('#ff0000');
});
test('new mode uses semantic CPU Load and native mode defaults',async({page})=>{
  await setup(page);await page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.empty=true);
  await expect.poll(()=>page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.empty)).toBe(true);
  // Wait for the next snapshot rather than relying on the previous profile list.
  await expect.poll(async()=>{await load(page).click();return source(page).inputValue();}).toBe('cpu:load');
  await expect(page.getByLabel('Нижний порог (percent)')).toHaveValue('30');await expect(page.getByLabel('Нижняя скорость, %')).toHaveValue('75');
  await expect(page.getByRole('spinbutton',{name:'Яркость по датчику, %',exact:true})).toHaveValue('100');
});
for(const change of ['source','mode'] as const)test(`late CPU default does not override ${change}`,async({page})=>{
  await setup(page);await page.evaluate(()=>{const f=(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles;f.empty=true;f.holdCpu=true;});
  await expect.poll(async()=>{await load(page).click();return page.evaluate(()=>!!(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.releaseCpu);}).toBe(true);
  if(change==='source'){await source(page).selectOption('gpu:load');await page.getByLabel('Нижний порог (percent)').fill('45');}
  else {await page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.mode='static');await expect(page.getByText('Режим помпы: static',{exact:false})).toBeVisible();}
  await page.evaluate(()=>(window as unknown as {dynamicProfiles:Fixture}).dynamicProfiles.releaseCpu!());
  await expect(page.getByText('Чтение источника CPU Load…',{exact:true})).toHaveCount(0);
  await expect(source(page)).toHaveValue(change==='source'?'gpu:load':'');
});
