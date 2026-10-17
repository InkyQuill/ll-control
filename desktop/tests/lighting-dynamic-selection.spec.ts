import {test,expect,type Page} from '@playwright/test';
const mode=(p:Page)=>p.getByRole('combobox',{name:'Эффект',exact:true});
const selection=(p:Page)=>p.getByRole('combobox',{name:'Управление подсветкой',exact:true});
const apply=(p:Page)=>p.getByRole('button',{name:'Применить подсветку',exact:true});
async function setup(page:Page,legacy=false) {
  await page.addInitScript(({legacy})=>{
    type C={dynamic_mode?:boolean;mode:string;color:{r:number;g:number;b:number};additional_colors:never[];brightness_percent:number;speed_percent:number;direction:string;scope:string|null;disabled:boolean};
    const defaults=(mode:string):C=>({mode,color:{r:255,g:0,b:0},additional_colors:[],brightness_percent:100,speed_percent:mode==='static'?0:75,direction:'forward',scope:'inner',disabled:false});
    const fixture=JSON.parse(localStorage.getItem('dynamic-selection')??'null')??{mode:'static',enabled:{static:true,breathing:false},writes:[],fail:false};
    Object.assign(window,{dynamicSelection:fixture,__TAURI_INTERNALS__:{invoke:async(_:string,{method,params}:{method:string;params:{dev:string;kind:string;cfg:C;config:unknown}})=>{
        if (_ === 'desktop_set_language') return null;
      const state=()=>({base_mode:fixture.mode,color_count:1,speed_supported:fixture.mode!=='static',requested:fixture.enabled[fixture.mode]?{sensor_id:'cpu',unit:'celsius',low_value:30,high_value:60,low_colors:[{r:0,g:0,b:255}],high_colors:[{r:255,g:0,b:0}],low_speed_percent:null,high_speed_percent:null}:null,applied:null,last_error:null,degraded:false,suppressed_by_thermal:false});
      switch(method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [{id:'cpu',unit:'celsius',label:'CPU',value:50,status:'valid',age_ms:0}];
        case 'get_lighting_info':return {dynamic_mode_supported:!legacy&&params.kind==='pump',modes:['static','breathing'],base:{...defaults(fixture.mode),...(!legacy?{dynamic_mode:fixture.enabled[fixture.mode]}:{})},profiles:['static','breathing'].map(m=>({...defaults(m),...(!legacy?{dynamic_mode:fixture.enabled[m]}:{})})),catalog:['static','breathing'].map(m=>({mode:m,color_count:1,speed:m!=='static',brightness:true,direction:false,defaults:defaults(m)}))};
        case 'get_thermal_state':return {requested:{cpu:null,gpu:null},degraded_sources:[],fan:{last_error:null},pump:{last_error:null},grace_remaining_ms:0};
        case 'set_lighting':if(fixture.fail)throw new Error('injected write failure');fixture.writes.push(structuredClone(params.cfg));fixture.mode=params.cfg.mode;fixture.enabled[fixture.mode]=params.cfg.dynamic_mode??false;localStorage.setItem('dynamic-selection',JSON.stringify(fixture));return null;
        case 'get_dynamic_rgb':return state();
        case 'set_dynamic_rgb':fixture.enabled[fixture.mode]=!!params.config;localStorage.setItem('dynamic-selection',JSON.stringify(fixture));return state();
        default:return null;
      }
    }}});
  },{legacy});
  await page.goto('/?view=display');await page.getByRole('button',{name:'Подсветка',exact:true}).click();await expect(mode(page)).toHaveValue('static');
}
test('per-mode Static/Dynamic choice follows drafts and reload without implicit writes',async({page})=>{
  await setup(page);await expect(selection(page)).toHaveValue('true');
  await mode(page).selectOption('breathing');await expect(selection(page)).toHaveValue('false');await apply(page).click();
  await mode(page).selectOption('static');await expect(selection(page)).toHaveValue('true');await apply(page).click();
  expect(await page.evaluate(()=>(window as unknown as {dynamicSelection:{writes:unknown[]}}).dynamicSelection.writes)).toMatchObject([{mode:'breathing',dynamic_mode:false},{mode:'static',dynamic_mode:true}]);
  await page.reload();await page.getByRole('button',{name:'Подсветка',exact:true}).click();await expect(selection(page)).toHaveValue('true');
  await selection(page).selectOption('false');await apply(page).click();
  await page.reload();await page.getByRole('button',{name:'Подсветка',exact:true}).click();await expect(selection(page)).toHaveValue('false');
});
test('explicit dynamic disable updates the matching mode draft and survives mode switches',async({page})=>{
  await setup(page);await page.getByText('Подсветка помпы по датчику',{exact:true}).click();
  await page.getByRole('button',{name:'Отключить режим по датчику',exact:true}).click();
  await expect(selection(page)).toHaveValue('false');
  await mode(page).selectOption('breathing');await mode(page).selectOption('static');await expect(selection(page)).toHaveValue('false');await apply(page).click();
  expect(await page.evaluate(()=>(window as unknown as {dynamicSelection:{writes:unknown[]}}).dynamicSelection.writes)).toMatchObject([{mode:'static',dynamic_mode:false}]);
});
test('failed apply retains selected intent and leaves saved mode unchanged',async({page})=>{
  await setup(page);await mode(page).selectOption('breathing');await selection(page).selectOption('true');
  await page.evaluate(()=>(window as unknown as {dynamicSelection:{fail:boolean}}).dynamicSelection.fail=true);
  await apply(page).click();await expect(page.getByRole('alert').filter({hasText:'injected write failure'})).toBeVisible();await expect(selection(page)).toHaveValue('true');
  expect(await page.evaluate(()=>(window as unknown as {dynamicSelection:{mode:string}}).dynamicSelection.mode)).toBe('static');
});
test('legacy daemon hides restore and preserves the old request contract',async({page})=>{
  await setup(page,true);await expect(selection(page)).toHaveCount(0);await apply(page).click();
  expect(await page.evaluate(()=>(window as unknown as {dynamicSelection:{writes:Record<string,unknown>[]}}).dynamicSelection.writes[0])).not.toHaveProperty('dynamic_mode');
});
