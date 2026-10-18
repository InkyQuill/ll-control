import {addControls,loadControls,openEditorSection} from './editor-controls';
import {test,expect,type Page} from '@playwright/test';
import type {Theme} from '../src/lib/api';
declare global {interface Window {weatherFixture:{calls:{method:string;params:Record<string,unknown>}[];fail:boolean;missing:boolean;unitChanged:boolean;preferred:string|null;defaultMode:'one'|'none'|'ambiguous';delayDefaults:boolean;failDefaults:boolean;resolveDefaults?:()=>void;active:Theme|null}}}
async function enableTemperature(page:Page) {
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['cpu/temp','celsius']));
}
test.beforeEach(async({page})=>{
  await page.addInitScript(()=>{
    const f={calls:[] as {method:string;params:Record<string,unknown>}[],fail:false,missing:false,unitChanged:false,preferred:null as string|null,defaultMode:'one' as 'one'|'none'|'ambiguous',delayDefaults:false,failDefaults:false,resolveDefaults:undefined as undefined|(()=>void),active:null as Theme|null};
    Object.assign(window,{weatherFixture:f,__TAURI_INTERNALS__:{invoke:async(command:string,request:{method:string;params:Record<string,unknown>}={method:'',params:{}})=>{
        if (command === 'desktop_set_language') return null;
      if(command!=='daemon_call')return null;
      f.calls.push(structuredClone(request));
      switch(request.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'lcd:test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
        case 'list_sensors':return f.missing?[]:[{id:'cpu/temp',label:'CPU temperature',unit:f.unitChanged?'rpm':'celsius',status:'valid',value:40,age_ms:0},{id:'cpu/load',label:'CPU load',unit:'percent',status:'valid',value:45,age_ms:0},{id:'pump/rpm',label:'Pump RPM',unit:'rpm',status:'valid',value:1000,age_ms:0}];
        case 'get_sensor_defaults':{
          if(f.failDefaults)throw new Error('Defaults read failed');
          const cpu={temperature:null,clock:null,load:f.preferred,power:null,fan:null,voltage:null};
          const reading={id:'cpu/load',label:'CPU load',unit:'percent',status:'valid',value:45,age_ms:0};
          const candidates=f.defaultMode==='none'?[]:[{role:'cpu_load',gpu_id:null,reading}];
          if(f.defaultMode==='ambiguous')candidates.push({role:'cpu_load',gpu_id:null,reading:{...reading,id:'cpu/other-load'}});
          const state={config:{version:1,revision:0,cpu,gpu:null},recommended:{version:1,revision:0,cpu:{...cpu,load:'cpu/load'},gpu:null},candidates,values:[]};
          if(f.delayDefaults)return new Promise(resolve=>{f.resolveDefaults=()=>resolve(state);});
          return state;
        }
        case 'get_lcd_state':return {source:f.active?'Theme':'Idle',last_error:null,requested:{mode:'Application',brightness_percent:50,rotation_degrees:0}};
        case 'get_lcd_theme':return structuredClone(f.active);
        case 'get_lcd_media':return null;
        case 'get_screen_collections':return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
        case 'list_prepared_clips':return [{key:'a'.repeat(32),name:'Draft background',fps:24,frames:24,encoded_bytes:100}];
        case 'list_profiles':return {version:1,revision:0,profiles:[]};
        case 'edit_profile':return {version:1,revision:1,profiles:[]};
        case 'preview_lcd_theme':return [255,216,255,217];
        case 'set_lcd_theme':if(f.fail)throw new Error('Weather Apply failed');f.active=structuredClone(request.params.theme as Theme);return {source:'Theme',last_error:null};
        default:return [];
      }
    }}});
  });
  await page.goto('/?view=display');await expect(page.getByText('Daemon подключён')).toBeVisible();
});
test('weather source and thresholds survive mode switches Apply and active-theme reload',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByRole('combobox',{name:'Интенсивность эффекта',exact:true}).selectOption('3');
  await enableTemperature(page);
  await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('30');
  await page.getByLabel('Верхний порог эффекта',{exact:true}).fill('75');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).uncheck();
  await page.getByRole('combobox',{name:'Тип фона',exact:true}).selectOption('horizontal');
  await addControls(page);await page.getByRole('button',{name:'+ Текст',exact:true}).click();await page.getByRole('button',{name:'Фон и композиция',exact:true}).click();
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  const saved=await page.evaluate(()=>window.weatherFixture.active);
  expect(saved?.layers[0]).toMatchObject({kind:'weather',effect:{kind:'rain',intensity:3,dynamic:false,sensor:{sensor_id:'cpu/temp',unit:'celsius',low:30,high:75}}});
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await expect(page.getByRole('combobox',{name:'Эффект LCD',exact:true})).toHaveValue('rain');
  await enableTemperature(page);
  await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('30');
  await expect(page.getByLabel('Верхний порог эффекта',{exact:true})).toHaveValue('75');
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('none');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(1);
});
test('weather Apply failure retains draft and never retries automatically',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await page.getByRole('combobox',{name:'Интенсивность эффекта',exact:true}).selectOption('2');
  await page.evaluate(()=>window.weatherFixture.fail=true);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect(page.getByRole('alert').filter({hasText:'Weather Apply failed'})).toContainText('Применение темы не подтверждено');
  await expect(page.getByRole('combobox',{name:'Эффект LCD',exact:true})).toHaveValue('snow');
  await expect(page.getByRole('combobox',{name:'Интенсивность эффекта',exact:true})).toHaveValue('2');
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='set_lcd_theme').length)).toBe(1);
  expect(await page.evaluate(()=>window.weatherFixture.active)).toBeNull();
});

test('missing weather source retains identity and thresholds until recovery',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await enableTemperature(page);
  await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('35');
  await page.evaluate(()=>window.weatherFixture.missing=true);
  await expect(page.getByText('Источник недоступен: используется сохранённая интенсивность.',{exact:true})).toBeVisible({timeout:10000});
  await expect(page.getByRole('combobox',{name:'Источник эффекта',exact:true})).toHaveValue(JSON.stringify(['cpu/temp','celsius']));
  await page.evaluate(()=>window.weatherFixture.missing=false);
  await expect(page.getByText('Источник недоступен: используется сохранённая интенсивность.',{exact:true})).toHaveCount(0,{timeout:10000});
  await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('35');
});
test('weather upgrades video theme to version three and removal restores version two',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.active={version:2,background_video:'a'.repeat(32),name:'Video weather',background:[0,0,0],layers:[]});
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.background_video)).toBe('a'.repeat(32));
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('none');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(2);
  expect(await page.evaluate(()=>window.weatherFixture.active?.background_video)).toBe('a'.repeat(32));
});

test('each source retains its own ranges across switching Apply and readback',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await enableTemperature(page);
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});
  const high=page.getByLabel('Верхний порог эффекта',{exact:true});
  await expect(low).toHaveValue('30');await expect(high).toHaveValue('60');
  await low.fill('32');await high.fill('71');
  await source.selectOption(JSON.stringify(['cpu/load','percent']));
  await expect(low).toHaveValue('30');await expect(high).toHaveValue('60');
  await low.fill('15');await high.fill('90');
  await source.selectOption(JSON.stringify(['pump/rpm','rpm']));
  await expect(low).toHaveValue('800');await expect(high).toHaveValue('1200');
  await low.fill('950');await high.fill('2200');
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));
  await expect(low).toHaveValue('32');await expect(high).toHaveValue('71');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  const ranges=await page.evaluate(()=>window.weatherFixture.active?.layers[0]);
  expect(ranges).toMatchObject({kind:'weather',effect:{sensor:{sensor_id:'cpu/temp',low:32,high:71},retained_sensors:[{sensor_id:'cpu/load',low:15,high:90},{sensor_id:'pump/rpm',low:950,high:2200}]}});
  await low.fill('40');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await expect(low).toHaveValue('32');
  await source.selectOption(JSON.stringify(['pump/rpm','rpm']));
  await expect(low).toHaveValue('950');await expect(high).toHaveValue('2200');
  await source.selectOption(JSON.stringify(['cpu/load','percent']));
  await expect(low).toHaveValue('15');await expect(high).toHaveValue('90');
});
test('source unit changes never reuse or discard a former range',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await enableTemperature(page);
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});
  await low.fill('35');
  await page.evaluate(()=>window.weatherFixture.unitChanged=true);
  await expect(page.getByText('Источник недоступен: используется сохранённая интенсивность.',{exact:true})).toBeVisible({timeout:10000});
  await expect(source).toHaveValue(JSON.stringify(['cpu/temp','celsius']));
  await source.selectOption(JSON.stringify(['cpu/temp','rpm']));
  await expect(low).toHaveValue('800');await low.fill('900');
  await page.evaluate(()=>window.weatherFixture.unitChanged=false);
  await expect(source.getByRole('option',{name:'CPU temperature · celsius',exact:true})).toHaveCount(1,{timeout:10000});
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));
  await expect(low).toHaveValue('35');
});

test('invalid active range cannot be hidden in retained settings by switching',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await enableTemperature(page);
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});
  await low.fill('');
  const pump=source.getByRole('option',{name:'Pump RPM · rpm',exact:true});
  await expect(pump).toHaveJSProperty('disabled',true);
  // Even an event injected by another script must leave the dropdown aligned.
  await source.evaluate(element=>{const select=element as HTMLSelectElement;select.value=JSON.stringify(['pump/rpm','rpm']);select.dispatchEvent(new Event('change',{bubbles:true}));});
  await expect(source).toHaveValue(JSON.stringify(['cpu/temp','celsius']));
  await expect(low).toHaveValue('');
  await expect(page.getByText('Задайте целые пороги; нижний не должен превышать верхний.',{exact:true})).toBeVisible();
  await low.fill('70');await expect(pump).toHaveJSProperty('disabled',true);
  await low.fill('35');await expect(pump).toHaveJSProperty('disabled',false);
  await source.selectOption(JSON.stringify(['pump/rpm','rpm']));
  await expect(low).toHaveValue('800');
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));
  await expect(low).toHaveValue('35');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
});

test('Rain and Snow keep independent intensity mode and source ranges in saved themes',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});
  const intensity=page.getByRole('combobox',{name:'Интенсивность эффекта',exact:true});
  const dynamic=page.getByLabel('Управлять эффектом от датчика',{exact:true});
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});
  await effect.selectOption('rain');await intensity.selectOption('3');await dynamic.check();await low.fill('35');
  await effect.selectOption('snow');await expect(intensity).toHaveValue('1');await expect(dynamic).not.toBeChecked();await expect(low).toHaveCount(0);
  await intensity.selectOption('2');await dynamic.check();await source.selectOption(JSON.stringify(['pump/rpm','rpm']));await low.fill('950');
  await effect.selectOption('rain');await expect(intensity).toHaveValue('3');await expect(dynamic).toBeChecked();await expect(low).toHaveValue('35');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.page_cache?.find(page=>page.page==='snow')?.theme.layers[0])).toMatchObject({kind:'weather',effect:{kind:'snow',intensity:2,dynamic:true,sensor:{sensor_id:'pump/rpm',low:950}}});
  await intensity.selectOption('1');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await expect(intensity).toHaveValue('3');
  await effect.selectOption('snow');await expect(intensity).toHaveValue('2');await expect(low).toHaveValue('950');
  await dynamic.uncheck();await effect.selectOption('rain');await expect(dynamic).toBeChecked();
  await effect.selectOption('snow');await expect(dynamic).not.toBeChecked();
});
test('weather controls expose native unit bounds and cannot hide out-of-range edits',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});
  await effect.selectOption('rain');await enableTemperature(page);
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});const high=page.getByLabel('Верхний порог эффекта',{exact:true});
  await expect(low).toHaveAttribute('min','0');await expect(high).toHaveAttribute('max','100');
  await low.fill('-1');await expect(effect.getByRole('option',{name:'Snow — снег',exact:true})).toHaveJSProperty('disabled',true);
  await effect.evaluate(element=>{const select=element as HTMLSelectElement;select.value='snow';select.dispatchEvent(new Event('change',{bubbles:true}));});
  await expect(effect).toHaveValue('rain');await expect(low).toHaveValue('-1');
  await low.fill('30');await high.fill('101');await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect(page.getByText('Проверьте пороги эффекта: 0–100 для °C/% и 0–3600 для RPM; нижний не должен превышать верхний.',{exact:true})).toBeVisible();expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='set_lcd_theme').length)).toBe(0);await expect(effect.getByRole('option',{name:'Snow — снег',exact:true})).toHaveJSProperty('disabled',true);
  await high.fill('100');await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['pump/rpm','rpm']));
  await expect(high).toHaveAttribute('max','3600');await high.fill('3601');
  await expect(effect.getByRole('option',{name:'Snow — снег',exact:true})).toHaveJSProperty('disabled',true);
  await high.fill('3600');await expect(effect.getByRole('option',{name:'Snow — снег',exact:true})).toHaveJSProperty('disabled',false);
});

test('profile Save validates the active weather range and permits explicit correction',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await enableTemperature(page);
  const high=page.getByLabel('Верхний порог эффекта',{exact:true});await high.fill('101');
  await page.getByRole('button',{name:'Сохранённые профили',exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await page.getByLabel('Имя профиля',{exact:true}).fill('Weather bounds');await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('Проверьте пороги эффекта');
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='edit_profile').length)).toBe(0);
  await high.fill('60');await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='edit_profile').length)).toBe(1);
  expect(await page.evaluate(()=>window.weatherFixture.active)).toBeNull();
});

test('initial dynamic weather selects daemon classified CPU Load regardless of catalog order',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect(page.getByRole('combobox',{name:'Источник эффекта',exact:true})).toHaveValue(JSON.stringify(['cpu/load','percent']));
  await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('30');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'weather',effect:{dynamic:true,sensor:{sensor_id:'cpu/load',unit:'percent'}}});
  expect(await page.evaluate(()=>window.weatherFixture.calls.some(call=>call.method==='set_sensor_defaults'))).toBe(false);
});
test('saved missing CPU Load retains exact identity without adopting another available source',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.preferred='cpu/missing');
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect(page.getByRole('combobox',{name:'Источник эффекта',exact:true})).toHaveValue(JSON.stringify(['cpu/missing','percent']));
  await expect(page.getByText('Источник недоступен: используется сохранённая интенсивность.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'weather',effect:{sensor:{sensor_id:'cpu/missing'}}});
});
for(const mode of ['none','ambiguous'] as const)test(`CPU Load ${mode} requires explicit selection and rejects incomplete Apply`,async({page})=>{
  await page.evaluate(mode=>window.weatherFixture.defaultMode=mode,mode);
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});await expect(source).toHaveValue('');
  await expect(page.getByText('Выберите источник эффекта: CPU Load недоступен или неоднозначен.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect(page.getByText('Выберите источник эффекта перед сохранением или Apply.',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='set_lcd_theme').length)).toBe(0);
  await source.selectOption(JSON.stringify(['pump/rpm','rpm']));
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
});
test('failed default read permits manual source selection without replacing the draft',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.failDefaults=true);
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect(page.getByText('Источник CPU Load не прочитан: Defaults read failed. Выберите датчик вручную.',{exact:true})).toBeVisible();
  await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['cpu/temp','celsius']));
  await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('30');
});
test('late CPU Load reply cannot overwrite manual selection or a newly loaded theme',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.delayDefaults=true);
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['cpu/temp','celsius']));
  await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('35');
  await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());
  await expect(page.getByRole('combobox',{name:'Источник эффекта',exact:true})).toHaveValue(JSON.stringify(['cpu/temp','celsius']));
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('snow');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect(page.getByText('Чтение источника CPU Load…',{exact:true})).toBeVisible();
  await page.evaluate(()=>window.weatherFixture.active={version:1,name:'Loaded',background:[0,0,0],layers:[]});
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());
  await expect(page.getByRole('combobox',{name:'Эффект LCD',exact:true})).toHaveValue('none');
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Loaded');
});
test('disabling Dynamic invalidates a pending default lookup and leaves a valid Static draft',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.delayDefaults=true);
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  const dynamic=page.getByLabel('Управлять эффектом от датчика',{exact:true});await dynamic.check();
  await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  await dynamic.uncheck();await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'weather',effect:{dynamic:false,sensor:null}});
});
test('unresolved CPU Load cannot enter a saved profile and manual correction remains local',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.defaultMode='none');
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await page.getByRole('button',{name:'Сохранённые профили',exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await page.getByLabel('Имя профиля',{exact:true}).fill('CPU Load draft');
  await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('Выберите источник эффекта');
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='edit_profile').length)).toBe(0);
  await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['cpu/temp','celsius']));
  await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();
  await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='edit_profile').length)).toBe(1);
  expect(await page.evaluate(()=>window.weatherFixture.active)).toBeNull();
});
for(const kind of ['rain','snow'] as const)test(`${kind} equal thresholds survive Apply readback and page switching`,async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});
  await effect.selectOption(kind);await enableTemperature(page);
  const low=page.getByLabel('Нижний порог эффекта',{exact:true});const high=page.getByLabel('Верхний порог эффекта',{exact:true});
  await low.fill('40');await high.fill('40');
  await expect(page.getByText('Задайте целые пороги; нижний не должен превышать верхний.',{exact:true})).toHaveCount(0);
  if(kind==='snow'){
    const help=page.getByText('При точном совпадении показания с порогом Snow использует минимальную скорость.',{exact:true});
    const id=await help.locator('..').getAttribute('id');
    await page.locator(`button.popover-trigger[popovertarget="${id}"]`).click();
    await expect(help).toBeVisible();await page.keyboard.press('Escape');
  }
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.version)).toBe(3);
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'weather',effect:{kind,sensor:{low:40,high:40}}});
  await low.fill('35');await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await expect(low).toHaveValue('40');
  await effect.selectOption(kind==='rain'?'snow':'rain');await effect.selectOption(kind);await expect(low).toHaveValue('40');await expect(high).toHaveValue('40');
});
test('Basic Rain and Snow retain independent names backgrounds and overlays through none Apply and reload',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});const name=page.getByLabel('Название',{exact:true});
  const background=page.getByLabel('Фон',{exact:true});const text=page.getByLabel('Текст',{exact:true}).first();
  const overlay=async(value:string,check=false)=>{const index=await effect.inputValue()==='none'?0:1;await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator(`.layer-row[data-layer-index="${index}"]`).click();if(check)await expect(text).toHaveValue(value);else await text.fill(value);await page.getByRole('button',{name:'Фон и композиция',exact:true}).click();};
  await name.fill('Basic page');await background.fill('#123456');await overlay('Basic overlay');
  await effect.selectOption('rain');await name.fill('Rain page');await background.fill('#ff0000');await overlay('Rain overlay');
  await enableTemperature(page);await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('35');
  await effect.selectOption('snow');await name.fill('Snow page');await background.fill('#0000ff');await overlay('Snow overlay');
  await effect.selectOption('rain');await expect(name).toHaveValue('Rain page');await expect(background).toHaveValue('#ff0000');await overlay('Rain overlay',true);
  await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('35');
  await effect.selectOption('none');await expect(name).toHaveValue('Basic page');await expect(background).toHaveValue('#123456');await overlay('Basic overlay',true);
  await effect.selectOption('rain');await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.name)).toBe('Rain page');
  expect(await page.evaluate(()=>window.weatherFixture.active?.page_cache?.find(page=>page.page==='snow')?.theme)).toMatchObject({name:'Snow page',background:[0,0,255]});
  await effect.selectOption('none');await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await expect(effect).toHaveValue('rain');await effect.selectOption('snow');await expect(name).toHaveValue('Snow page');await overlay('Snow overlay',true);
});

test('deleting the weather row restores Basic and preserves Rain for later Apply',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});const name=page.getByLabel('Название',{exact:true});
  await name.fill('Original Basic');await effect.selectOption('rain');await name.fill('Retained Rain');
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();await page.getByRole('button',{name:'Удалить слой',exact:true}).click();
  await expect(effect).toHaveValue('none');await expect(name).toHaveValue('Original Basic');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.name)).toBe('Original Basic');
  const saved=await page.evaluate(()=>window.weatherFixture.active);
  expect(saved?.page_cache?.map(page=>page.page)).toEqual(['rain']);
  expect(saved?.page_cache?.[0].theme.name).toBe('Retained Rain');
  await effect.selectOption('rain');await expect(name).toHaveValue('Retained Rain');
});

test('inactive Basic video cannot be deleted until its draft reference is removed',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.active={version:2,name:'Video Basic',background_video:'a'.repeat(32),background:[0,0,0],layers:[]});
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('rain');
  await page.getByRole('combobox',{name:'Тип фона',exact:true}).selectOption('solid');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await page.getByLabel('Показать все файлы видео для управления',{exact:true}).check();
  await expect(page.getByRole('combobox',{name:'Сохранённый ролик',exact:true})).toHaveValue('a'.repeat(32));
  await expect(page.getByRole('button',{name:'Удалить ролик…',exact:true})).toBeDisabled();
  await expect(page.getByText('Ролик используется в черновике страницы LCD. Замените его фон перед удалением.',{exact:true})).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'Композиция',exact:true}).click();
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('none');
  await page.getByRole('combobox',{name:'Тип фона',exact:true}).selectOption('solid');
  await page.getByRole('button',{name:'Библиотека',exact:true}).click();
  await page.getByRole('button',{name:'Библиотека роликов',exact:true}).click();
  await expect(page.getByRole('button',{name:'Удалить ролик…',exact:true})).toBeEnabled();
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='delete_prepared_clip').length)).toBe(0);
});

test('Meteor static palettes speed and full page survive other pages None Apply and reload',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('meteor');
  await expect(page.getByRole('combobox',{name:'Скорость метеоров',exact:true})).toHaveValue('2');
  await page.getByRole('combobox',{name:'Скорость метеоров',exact:true}).selectOption('3');
  await page.getByLabel('Цвет метеоров 1',{exact:true}).fill('#ff1122');await page.getByLabel('Название',{exact:true}).fill('Meteor page');
  await effect.selectOption('rain');await effect.selectOption('snow');await effect.selectOption('none');await effect.selectOption('meteor');
  await expect(page.getByRole('combobox',{name:'Скорость метеоров',exact:true})).toHaveValue('3');await expect(page.getByLabel('Цвет метеоров 1',{exact:true})).toHaveValue('#ff1122');
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Meteor page');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.name)).toBe('Meteor page');
  const theme=await page.evaluate(()=>window.weatherFixture.active);expect(theme?.version).toBe(3);expect(theme?.layers[0]).toMatchObject({kind:'meteor',effect:{speed:3,colors:[[255,17,34],[0,0,255]]}});expect(theme?.page_cache?.length).toBe(3);
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();await page.getByRole('button',{name:'Удалить слой',exact:true}).click();await expect(effect).toHaveValue('none');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await expect(effect).toHaveValue('meteor');await expect(page.getByLabel('Цвет метеоров 1',{exact:true})).toHaveValue('#ff1122');
});
test('Meteor uses CPU Load and independently retains palettes ranges and unit identities for each source',async({page})=>{
  await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('meteor');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});await expect(source).toHaveValue(JSON.stringify(['cpu/load','percent']));
  await page.getByLabel('Нижний цвет метеоров 1',{exact:true}).fill('#ff0000');await page.getByLabel('Верхний цвет метеоров 2',{exact:true}).fill('#ffff00');await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('15');
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await page.getByLabel('Нижний цвет метеоров 1',{exact:true}).fill('#ffffff');await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('40');await page.getByLabel('Верхний порог эффекта',{exact:true}).fill('40');
  await source.selectOption(JSON.stringify(['cpu/load','percent']));await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('15');await expect(page.getByLabel('Нижний цвет метеоров 1',{exact:true})).toHaveValue('#ff0000');await expect(page.getByLabel('Верхний цвет метеоров 2',{exact:true})).toHaveValue('#ffff00');
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await expect(page.getByLabel('Нижний цвет метеоров 1',{exact:true})).toHaveValue('#ffffff');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.layers[0]?.kind)).toBe('meteor');
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'meteor',effect:{sensor:{binding:{low:40,high:40},low_colors:[[255,255,255],[0,0,255]]},retained_sensors:[{binding:{sensor_id:'cpu/load',low:15},low_colors:[[255,0,0],[0,0,255]],high_colors:[[0,255,0],[255,255,0]]}]}});
});
test('Meteor missing CPU Load blocks Apply and profile Save until explicit correction',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.defaultMode='none');await page.getByRole('combobox',{name:'Эффект LCD',exact:true}).selectOption('meteor');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect(page.getByText('Выберите источник эффекта: CPU Load недоступен или неоднозначен.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect(page.getByText('Выберите источник эффекта перед сохранением или Apply.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Сохранённые профили',exact:true}).click();await page.getByRole('button',{name:'Обновить список профилей',exact:true}).click();await page.getByLabel('Имя профиля',{exact:true}).fill('Meteor draft');await page.getByRole('button',{name:'Сохранить черновик в слот',exact:true}).click();
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='edit_profile').length)).toBe(0);
  await expect(page.getByRole('combobox',{name:'Эффект LCD',exact:true}).locator('option[value="rain"]')).toHaveJSProperty('disabled',true);
  await page.getByRole('combobox',{name:'Источник эффекта',exact:true}).selectOption(JSON.stringify(['pump/rpm','rpm']));await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('800');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.layers[0]?.kind)).toBe('meteor');
});

test('late Meteor CPU Load defaults cannot overwrite manual palettes or a replacement theme',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.delayDefaults=true);
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('meteor');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await page.getByLabel('Нижний цвет метеоров 1',{exact:true}).fill('#ffffff');
  await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());await expect(source).toHaveValue(JSON.stringify(['cpu/temp','celsius']));await expect(page.getByLabel('Нижний цвет метеоров 1',{exact:true})).toHaveValue('#ffffff');
  await page.evaluate(()=>{window.weatherFixture.active={version:1,name:'Replacement',background:[0,0,0],layers:[]};window.weatherFixture.resolveDefaults=undefined;});
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await effect.selectOption('meteor');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();
  await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());
  await expect(effect).toHaveValue('none');await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Replacement');
});

test('Snow3D has independent intensity full page and no dynamic sensor controls',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('snow3d');const intensity=page.getByRole('combobox',{name:'Интенсивность Snow3D',exact:true});await expect(intensity).toHaveValue('1');
  await expect(page.getByLabel('Управлять эффектом от датчика',{exact:true})).toHaveCount(0);await intensity.selectOption('3');await page.getByLabel('Название',{exact:true}).fill('Snow3D page');await page.getByLabel('Фон',{exact:true}).fill('#112233');
  await effect.selectOption('rain');await effect.selectOption('snow');await effect.selectOption('meteor');await effect.selectOption('none');await effect.selectOption('snow3d');await expect(intensity).toHaveValue('3');await expect(page.getByLabel('Фон',{exact:true})).toHaveValue('#112233');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.name)).toBe('Snow3D page');
  const theme=await page.evaluate(()=>window.weatherFixture.active);expect(theme?.page_cache?.length).toBe(4);expect(theme?.layers[0]).toEqual({kind:'snow3d',effect:{intensity:3,seed:42}});expect(theme?.version).toBe(3);
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();await page.getByRole('button',{name:'Удалить слой',exact:true}).click();await expect(effect).toHaveValue('none');await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await expect(effect).toHaveValue('snow3d');await expect(intensity).toHaveValue('3');
  expect(await page.evaluate(()=>window.weatherFixture.calls.filter(call=>call.method==='get_sensor_defaults').length)).toBe(0);
});

test('Fireworks six static colors and intensity survive all pages Apply and reload',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('fireworks');const intensity=page.getByRole('combobox',{name:'Интенсивность Fireworks',exact:true});await expect(intensity).toHaveValue('1');
  await expect(page.getByLabel('Цвет фейерверка 6',{exact:true})).toHaveValue('#82aded');await intensity.selectOption('3');await page.getByLabel('Цвет фейерверка 6',{exact:true}).fill('#112233');await page.getByLabel('Название',{exact:true}).fill('Fireworks page');
  for(const target of ['rain','snow','meteor','snow3d','none','fireworks'])await effect.selectOption(target);
  await expect(intensity).toHaveValue('3');await expect(page.getByLabel('Цвет фейерверка 6',{exact:true})).toHaveValue('#112233');
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.name)).toBe('Fireworks page');const saved=await page.evaluate(()=>window.weatherFixture.active);expect(saved?.page_cache?.length).toBe(5);expect(saved?.layers[0]).toMatchObject({kind:'fireworks',effect:{intensity:3,colors:[[0,255,0],[0,0,255],[128,0,255],[255,0,65],[255,215,0],[17,34,51]]}});
  await page.getByRole('button',{name:/^(Слои|Layers)$/,exact:true}).click();await page.locator('.layer-row[data-layer-index="0"]').click();await page.getByRole('button',{name:'Удалить слой',exact:true}).click();await expect(effect).toHaveValue('none');await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await expect(effect).toHaveValue('fireworks');await expect(intensity).toHaveValue('3');
});
test('Fireworks per-source six palettes and equal ranges remain independent of Meteor',async({page})=>{
  const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('fireworks');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});await expect(source).toHaveValue(JSON.stringify(['cpu/load','percent']));
  await page.getByLabel('Нижний цвет фейерверка 6',{exact:true}).fill('#ffffff');await page.getByLabel('Верхний цвет фейерверка 5',{exact:true}).fill('#00ffff');await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('15');
  await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await page.getByLabel('Нижний цвет фейерверка 6',{exact:true}).fill('#ff0000');await page.getByLabel('Нижний порог эффекта',{exact:true}).fill('40');await page.getByLabel('Верхний порог эффекта',{exact:true}).fill('40');
  await source.selectOption(JSON.stringify(['cpu/load','percent']));await expect(page.getByLabel('Нижний цвет фейерверка 6',{exact:true})).toHaveValue('#ffffff');await expect(page.getByLabel('Нижний порог эффекта',{exact:true})).toHaveValue('15');
  await effect.selectOption('meteor');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();await expect(page.getByLabel('Нижний цвет метеоров 1',{exact:true})).toHaveValue('#00ff00');await expect(page.getByLabel('Нижний цвет метеоров 3',{exact:true})).toHaveCount(0);
  await effect.selectOption('fireworks');await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await expect(page.getByLabel('Нижний цвет фейерверка 6',{exact:true})).toHaveValue('#ff0000');await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();await expect.poll(()=>page.evaluate(()=>window.weatherFixture.active?.layers[0]?.kind)).toBe('fireworks');
  expect(await page.evaluate(()=>window.weatherFixture.active?.layers[0])).toMatchObject({kind:'fireworks',effect:{sensor:{binding:{low:40,high:40}},retained_sensors:[{binding:{sensor_id:'cpu/load',low:15},low_colors:[[0,255,0],[0,0,255],[128,0,255],[255,0,65],[255,215,0],[255,255,255]]}]}});
});
test('late Fireworks default reply cannot overwrite manual palettes or a replacement Meteor page',async({page})=>{
  await page.evaluate(()=>window.weatherFixture.delayDefaults=true);const effect=page.getByRole('combobox',{name:'Эффект LCD',exact:true});await effect.selectOption('fireworks');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  const source=page.getByRole('combobox',{name:'Источник эффекта',exact:true});await source.selectOption(JSON.stringify(['cpu/temp','celsius']));await page.getByLabel('Нижний цвет фейерверка 6',{exact:true}).fill('#ffffff');await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());await expect(source).toHaveValue(JSON.stringify(['cpu/temp','celsius']));await expect(page.getByLabel('Нижний цвет фейерверка 6',{exact:true})).toHaveValue('#ffffff');
  await page.evaluate(()=>{window.weatherFixture.active={version:1,name:'Replacement',background:[0,0,0],layers:[]};window.weatherFixture.resolveDefaults=undefined;});await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await effect.selectOption('fireworks');await page.getByLabel('Управлять эффектом от датчика',{exact:true}).check();await expect.poll(()=>page.evaluate(()=>typeof window.weatherFixture.resolveDefaults)).toBe('function');
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();await effect.selectOption('meteor');await page.evaluate(()=>window.weatherFixture.resolveDefaults?.());await expect(effect).toHaveValue('meteor');await expect(page.getByLabel('Цвет метеоров 2',{exact:true})).toHaveValue('#0000ff');await expect(page.getByLabel('Цвет метеоров 3',{exact:true})).toHaveCount(0);
});
