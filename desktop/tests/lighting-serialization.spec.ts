import {test,expect} from '@playwright/test';
interface Fixture {finishDynamic:(fail:boolean)=>void;finishManual:()=>void;writes:string[]}
for(const fail of [false,true])test(`lighting serializes manual and dynamic writes after ${fail?'failure':'success'}`,async({page})=>{
  await page.addInitScript(()=>{
    const config={sensor_id:'cpu',unit:'celsius',low_value:30,high_value:60,low_colors:[{r:0,g:0,b:255}],high_colors:[{r:255,g:0,b:0}],low_speed_percent:null,high_speed_percent:null};
    let enabled=true;
    const state=()=>({base_mode:'static',color_count:1,speed_supported:false,requested:enabled?config:null,applied:null,degraded:false,suppressed_by_thermal:false,last_error:null});
    const fixture:Fixture={finishDynamic:()=>{},finishManual:()=>{},writes:[]};
    Object.assign(window,{lightingFixture:fixture,__TAURI_INTERNALS__:{invoke:async(_:string,req:{method:string})=>{
        if (_ === 'desktop_set_language') return null;
      switch(req.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [{id:'cpu',label:'CPU',unit:'celsius',value:42,status:'valid',age_ms:1}];
        case 'get_lcd_state':return {source:'Idle',last_error:null,requested:{}};
        case 'get_lighting_info':return {modes:['static']};
        case 'get_thermal_state':return {requested:{cpu:null,gpu:null},active_source:null,degraded_sources:[],grace_remaining_ms:0,blocked_reason:null,fan:{last_error:null},pump:{last_error:null}};
        case 'get_dynamic_rgb':return state();
        case 'set_dynamic_rgb':fixture.writes.push(req.method);return new Promise((resolve,reject)=>{fixture.finishDynamic=fail=>fail?reject(new Error('dynamic write failed')):resolve(state());});
        case 'set_lighting':fixture.writes.push(req.method);return new Promise(resolve=>{fixture.finishManual=()=>{enabled=false;resolve({});};});
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await page.getByText('Подсветка помпы по датчику',{exact:true}).click();
  const load=page.getByRole('button',{name:'Загрузить настройки режима по датчику'});
  await expect(load).toBeEnabled();await load.click();
  await page.getByLabel('Нижний порог (°C)').fill('43');
  await page.getByRole('button',{name:'Загрузить текущие оповещения'}).click();
  const dynamic=page.getByRole('button',{name:'Применить режим по датчику'});
  const manual=page.getByRole('button',{name:'Применить подсветку',exact:true});
  const thermal=page.getByRole('button',{name:'Применить оповещения'});
  await dynamic.click();
  await expect(manual).toBeDisabled();await expect(thermal).toBeDisabled();
  await page.evaluate(fail=>{(window as unknown as {lightingFixture:Fixture}).lightingFixture.finishDynamic(fail);},fail);
  await expect(manual).toBeEnabled();
  if(fail)await expect(page.getByText('dynamic write failed',{exact:true})).toBeVisible();
  await manual.click();
  await expect(dynamic).toBeDisabled();await expect(load).toBeDisabled();
  await expect(page.getByText('dynamic write failed',{exact:true})).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {lightingFixture:Fixture}).lightingFixture.finishManual();});
  await expect(load).toBeEnabled();
  await expect(page.getByText('Режим помпы: static · По датчику: выключен')).toBeVisible();
  await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('43');
  expect(await page.evaluate(()=>(window as unknown as {lightingFixture:Fixture}).lightingFixture.writes)).toEqual(['set_dynamic_rgb','set_lighting']);
});
