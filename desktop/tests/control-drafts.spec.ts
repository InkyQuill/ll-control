import {test,expect} from '@playwright/test';

test.beforeEach(async({page})=>{
  await page.addInitScript(()=>{
    const fixture={online:true,holdCooling:false,holdPreset:false,finishCooling:()=>{},finishPreset:()=>{}};
    const channel={requested:{mode:'fixed',percent:60},applied_percent:60,motherboard:false,fallback_active:false,last_error:null};
    const thermal={requested:{cpu:null,gpu:null},active_source:null,degraded_sources:[],grace_remaining_ms:0,blocked_reason:null,fan:{last_error:null},pump:{last_error:null}};
    Object.assign(window,{controlFixture:fixture,__TAURI_INTERNALS__:{invoke:async(cmd:string,req:{method:string;params:{policy:unknown}})=>{
        if (cmd === 'desktop_set_language') return null;
      if(!fixture.online)throw new Error('offline');
      if(cmd==='cooling_preset')return new Promise(resolve=>{fixture.finishPreset=()=>resolve({mode:'fixed',percent:45});if(!fixture.holdPreset)fixture.finishPreset();});
      switch(req.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'},{id:'B',display_name:'LCD B', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [{id:'cpu',label:'CPU',unit:'celsius',value:42,status:'valid',age_ms:1}];
        case 'get_lcd_state':return {source:'Idle',requested:{}};
        case 'get_cooling_state':return {pump:channel,fan:channel};
        case 'set_cooling_policy':return new Promise(resolve=>{fixture.finishCooling=()=>resolve({pump:{...channel,applied_percent:77,requested:req.params.policy},fan:channel});if(!fixture.holdCooling)fixture.finishCooling();});
        case 'get_lighting_info':return {modes:['static_color','breathing']};
        case 'get_thermal_state':return thermal;
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');await expect(page.getByText('Daemon подключён')).toBeVisible();
});

test('late cooling Apply response cannot appear on another device',async({page})=>{
  await page.getByRole('button',{name:'Охлаждение',exact:true}).click();
  await expect(page.getByText('Applied: 60%')).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {controlFixture:{holdCooling:boolean}}).controlFixture.holdCooling=true;});
  await page.getByLabel('PWM, %',{exact:true}).fill('77');
  await page.getByRole('button',{name:'Применить охлаждение'}).click();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await page.evaluate(()=>{(window as unknown as {controlFixture:{finishCooling:()=>void}}).controlFixture.finishCooling();});
  await expect(page.getByRole('button',{name:'Применить охлаждение'})).toBeEnabled();
  await expect(page.getByText('Applied: 60%')).toBeVisible();
  await expect(page.getByText('Политика принята daemon.',{exact:false})).toHaveCount(0);
});

test('late preset cannot replace edits made while it loads',async({page})=>{
  await page.getByRole('button',{name:'Охлаждение',exact:true}).click();
  await page.getByLabel('Источник температуры').selectOption('cpu');
  await page.evaluate(()=>{(window as unknown as {controlFixture:{holdPreset:boolean}}).controlFixture.holdPreset=true;});
  await page.getByRole('button',{name:'quiet',exact:true}).click();
  await page.getByLabel('PWM, %',{exact:true}).fill('83');
  await page.evaluate(()=>{(window as unknown as {controlFixture:{finishPreset:()=>void}}).controlFixture.finishPreset();});
  await expect(page.getByRole('status')).toContainText('Preset не заменил ваши правки');
  await expect(page.getByLabel('PWM, %',{exact:true})).toHaveValue('83');
});

test('thermal draft survives reconnect but cannot apply to a different device',async({page})=>{
  await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await page.getByRole('button',{name:'Загрузить текущие оповещения'}).click();
  await page.getByLabel('Включить CPU').check();
  await page.getByLabel('Датчик CPU',{exact:false}).selectOption('cpu');
  await page.getByLabel('Порог CPU, °C').fill('91');
  await page.getByLabel('Эффект',{exact:false}).selectOption('breathing');
  await page.evaluate(()=>{(window as unknown as {controlFixture:{online:boolean}}).controlFixture.online=false;});
  await expect(page.getByText('Нет связи с daemon')).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {controlFixture:{online:boolean}}).controlFixture.online=true;});
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await expect(page.getByLabel('Порог CPU, °C')).toHaveValue('91');
  await expect(page.getByLabel('Эффект',{exact:false})).toHaveValue('breathing');
  await expect(page.getByRole('button',{name:'Применить оповещения'})).toBeEnabled();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await expect(page.getByRole('button',{name:'Применить оповещения'})).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('Черновик оповещений относится к другому устройству');
  await page.getByLabel('Устройство',{exact:true}).selectOption('A');
  await expect(page.getByRole('button',{name:'Применить оповещения'})).toBeEnabled();
  await expect(page.getByLabel('Порог CPU, °C')).toHaveValue('91');
});
