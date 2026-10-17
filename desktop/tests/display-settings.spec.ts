import {test,expect,type Page} from '@playwright/test';
async function setup(page:Page){
 await page.addInitScript(()=>{
  const state={version:1,revision:0,temperature_unit:'celsius'};
  const channel={requested:{mode:'curve',sensor_id:'cpu',points:[{celsius:30,percent:40},{celsius:70,percent:100}],hysteresis_c:2,fallback_percent:100},applied_percent:60,motherboard:false,fallback_active:false,last_error:null};
  const f={state,calls:[] as {method:string;params:Record<string,unknown>}[],lost:false};
  Object.assign(window,{unitFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
   if(command==='group_archive_status')return null;
   const method=args.method as string,params=(args.params??{}) as Record<string,unknown>;f.calls.push({method,params:structuredClone(params)});
   switch(method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
    case 'get_display_settings':return structuredClone(state);
    case 'set_display_settings':{const next=params.settings as typeof state;if(next.revision!==state.revision)throw {message:'stale revision'};state.temperature_unit=next.temperature_unit;state.revision++;if(f.lost){f.lost=false;throw {message:'Lost unit reply'};}return structuredClone(state);}
    case 'list_devices':return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
    case 'list_sensors':return [{id:'cpu',label:'Fixture CPU temperature',unit:'celsius',value:40,status:'valid',age_ms:0}];
    case 'get_lcd_state':return {source:'Idle',requested:{mode:'LocalUI',brightness_percent:100,rotation_degrees:0},last_error:null};
    case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    case 'get_cooling_state':return {pump:channel,fan:channel};
    case 'set_cooling_policy':return {pump:{...channel,requested:params.policy},fan:channel};
    case 'get_sensor_defaults':return {config:{version:1,revision:0,cpu:{},gpu:null},recommended:{version:1,revision:0,cpu:{},gpu:null},values:[],candidates:[]};
    default:return null;
   }
  }}});
 });
 await page.goto('/?view=display');await page.getByRole('button',{name:'Настройки',exact:true}).click();await expect(page.getByRole('combobox',{name:'Единицы температуры',exact:true})).toBeVisible();
}
const section=(page:Page)=>page.getByRole('region',{name:'Единицы и язык'});
async function fahrenheit(page:Page){await page.getByRole('combobox',{name:'Единицы температуры',exact:true}).selectOption('fahrenheit');await section(page).getByRole('button',{name:'Сохранить параметры отображения',exact:true}).click();await expect(section(page).getByRole('status')).toContainText('Параметры отображения сохранены');}
test('global unit draft survives polling, saving changes sensor display and refresh restores persisted preference',async({page})=>{
 await setup(page);await page.getByRole('combobox',{name:'Единицы температуры',exact:true}).selectOption('fahrenheit');
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {unitFixture:{calls:{method:string}[]}}).unitFixture.calls.filter(c=>c.method==='get_display_settings').length)).toBeGreaterThan(1);
 await expect(page.getByRole('combobox',{name:'Единицы температуры',exact:true})).toHaveValue('fahrenheit');await section(page).getByRole('button',{name:'Сохранить параметры отображения',exact:true}).click();
 await page.getByRole('button',{name:'Датчики',exact:true}).click();await expect(page.getByRole('region',{name:'Показания датчиков'}).getByText('104.0 °F',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Настройки',exact:true}).click();await expect(page.getByRole('combobox',{name:'Единицы температуры',exact:true})).toHaveValue('fahrenheit');
 expect(await page.evaluate(()=>(window as unknown as {unitFixture:{calls:{method:string;params:unknown}[]}}).unitFixture.calls.filter(c=>c.method==='set_display_settings'))).toEqual([{method:'set_display_settings',params:{settings:{version:1,revision:0,temperature_unit:'fahrenheit'}}}]);
});
test('Fahrenheit curve inputs and hysteresis convert back to canonical Celsius exactly once',async({page})=>{
 await setup(page);await fahrenheit(page);await page.getByRole('button',{name:'Охлаждение',exact:true}).click();await page.getByRole('button',{name:'Загрузить текущую политику',exact:true}).click();
 const first=page.getByRole('spinbutton',{name:'Точка 1: °F',exact:true});await expect(first).toHaveValue('86');await first.fill('104');await page.getByRole('spinbutton',{name:'Точка 2: °F',exact:true}).fill('176');
 const hysteresis=page.getByRole('spinbutton',{name:'Гистерезис, °F',exact:true});await expect(hysteresis).toHaveValue('3.6');await hysteresis.fill('9');await page.getByRole('button',{name:'Применить охлаждение',exact:true}).click();
 expect(await page.evaluate(()=>(window as unknown as {unitFixture:{calls:{method:string;params:unknown}[]}}).unitFixture.calls.find(c=>c.method==='set_cooling_policy')?.params)).toEqual({dev:'test',target:'pump',policy:{mode:'curve',sensor_id:'cpu',points:[{celsius:40,percent:40},{celsius:80,percent:100}],hysteresis_c:5,fallback_percent:100}});
});
test('lost save reply retains intent, readback exposes committed revision and stale draft cannot be replayed',async({page})=>{
 await setup(page);await page.evaluate(()=>{(window as unknown as {unitFixture:{lost:boolean}}).unitFixture.lost=true;});await page.getByRole('combobox',{name:'Единицы температуры',exact:true}).selectOption('fahrenheit');await section(page).getByRole('button',{name:'Сохранить параметры отображения',exact:true}).click();
 await expect(section(page).getByRole('alert').filter({hasText:'Lost unit reply'})).toBeVisible();await expect(section(page).getByRole('alert').filter({hasText:'Параметры отображения изменились'})).toBeVisible();await expect(section(page).getByRole('button',{name:'Сохранить параметры отображения',exact:true})).toBeDisabled();
 await section(page).getByRole('button',{name:'Загрузить сохранённые параметры',exact:true}).click();await expect(page.getByRole('combobox',{name:'Единицы температуры',exact:true})).toHaveValue('fahrenheit');await expect(section(page).getByRole('button',{name:'Сохранить параметры отображения',exact:true})).toBeEnabled();
 expect(await page.evaluate(()=>(window as unknown as {unitFixture:{calls:{method:string}[]}}).unitFixture.calls.filter(c=>c.method==='set_display_settings').length)).toBe(1);
});
