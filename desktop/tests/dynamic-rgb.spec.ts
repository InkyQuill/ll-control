import { test, expect } from '@playwright/test';

test('dynamic RGB loads named channels, validates draft device and shows output errors', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: {method:string;params:Record<string,unknown>}[]=[];
    const config={sensor_id:'cpu:test',unit:'celsius',low_value:30,high_value:60,
      low_colors:[{b:51,r:17,g:34}],high_colors:[{g:85,b:102,r:68}],
      low_speed_percent:null,high_speed_percent:null};
    let requested: unknown=config;
    Object.assign(window,{bridgeCalls:calls,__TAURI_INTERNALS__:{invoke:async (_command:string,request:{method:string;params:Record<string,unknown>})=>{
      calls.push(request);
      const state=()=>({base_mode:'static',color_count:1,speed_supported:false,requested,
        applied:null,degraded:true,suppressed_by_thermal:false,last_error:null});
      switch(request.method){
        case 'list_devices':return [{id:'lcd:a',display_name:'LCD A'},{id:'lcd:b',display_name:'LCD B'}];
        case 'list_sensors':return [{id:'cpu:test',label:'CPU package',unit:'celsius',value:42,status:'valid',age_ms:10},{id:'power:test',label:'CPU power',unit:'watts',value:50,status:'valid',age_ms:10}];
        case 'get_lcd_state':return {source:'Idle',last_error:null,requested:{}};
        case 'get_lighting_info':return {modes:['static'],max_speed:100,max_brightness:100};
        case 'get_thermal_state':return {requested:{cpu:null,gpu:null},active_source:null,degraded_sources:[],grace_remaining_ms:0,blocked_reason:null,fan:{last_error:null},pump:{last_error:null}};
        case 'get_dynamic_rgb':return state();
        case 'set_dynamic_rgb':
          requested=request.params.config;
          return {...state(),degraded:false,suppressed_by_thermal:requested!==null,
            last_error:requested!==null?'Injected write failure':null};
        default:throw new Error('Unhandled '+request.method);
      }
    }}});
  });
  await page.goto('/');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await page.getByText('Подсветка помпы по датчику',{exact:true}).click();
  const load=page.getByRole('button',{name:'Загрузить настройки режима по датчику'});
  await expect(load).toBeEnabled();
  await load.click();
  await expect(page.getByRole('combobox',{name:'Датчик подсветки'}).getByRole('option',{name:/CPU power/})).toHaveCount(0);
  await expect(page.getByLabel('Нижний цвет 1')).toHaveValue('#112233');
  await expect(page.getByLabel('Верхний цвет 1')).toHaveValue('#445566');
  await expect(page.getByText('Датчик недоступен — используется обычная подсветка.')).toBeVisible();
  await page.getByLabel('Нижний порог (°C)').fill('45');
  // Polling must not overwrite edits.
  await expect.poll(async()=>page.evaluate(()=> (window as unknown as {bridgeCalls:{method:string}[]}).bridgeCalls.filter(c=>c.method==='get_dynamic_rgb').length)).toBeGreaterThan(1);
  await expect(page.getByLabel('Нижний порог (°C)')).toHaveValue('45');
  await page.getByRole('combobox',{name:'Устройство',exact:true}).selectOption('lcd:b');
  const apply=page.getByRole('button',{name:'Применить режим по датчику'});
  await expect(apply).toBeDisabled();
  await expect(page.getByText('Устройство или режим помпы изменились.',{exact:false})).toBeVisible();
  await expect(load).toBeEnabled();
  await load.click();
  await page.getByLabel('Нижний порог (°C)').fill('70');
  await apply.click();
  await expect(page.getByRole('alert')).toContainText('Пороги должны быть');
  expect(await page.evaluate(()=>(window as unknown as {bridgeCalls:{method:string}[]}).bridgeCalls.filter(c=>c.method==='set_dynamic_rgb').length)).toBe(0);
  await page.getByLabel('Нижний порог (°C)').fill('35');
  await apply.click();
  await expect(page.getByText('Injected write failure').first()).toBeVisible();
  await expect(page.getByText('Подсветка временно перекрыта температурным оповещением.')).toBeVisible();
  await page.getByRole('button',{name:'Отключить режим по датчику'}).click();
  await expect(page.getByText('Режим по датчику отключён.',{exact:true})).toBeVisible();
  const writes=await page.evaluate(()=>(window as unknown as {bridgeCalls:{method:string;params:Record<string,unknown>}[]}).bridgeCalls.filter(c=>c.method==='set_dynamic_rgb').map(c=>c.params));
  expect(writes).toEqual([
    {dev:'lcd:b',config:{sensor_id:'cpu:test',unit:'celsius',low_value:35,high_value:60,
      low_colors:[{r:17,g:34,b:51}],high_colors:[{r:68,g:85,b:102}],
      low_speed_percent:null,high_speed_percent:null}},
    {dev:'lcd:b',config:null}
  ]);
});

test('missing sensor status respects thermal ownership and updates after recovery', async ({ page }) => {
  await page.addInitScript(() => {
    Object.assign(window, { rgbHealth: { degraded: true, suppressed: true }, __TAURI_INTERNALS__: { invoke: async (_: string, request: { method: string }) => {
      const health = (window as unknown as { rgbHealth: { degraded: boolean; suppressed: boolean } }).rgbHealth;
      switch (request.method) {
        case 'list_devices': return [{ id: 'lcd:test', display_name: 'LCDV2' }];
        case 'list_sensors': return [];
        case 'get_lcd_state': return { source: 'Idle', requested: {}, last_error: null };
        case 'get_lighting_info': return { modes: ['static'] };
        case 'get_thermal_state': return { requested: { cpu: null, gpu: null }, active_source: health.suppressed ? 'cpu' : null, degraded_sources: [], grace_remaining_ms: 0, blocked_reason: null, pump: { last_error: null }, fan: { last_error: null } };
        case 'get_dynamic_rgb': return { requested: null, base_mode: 'static', color_count: 1, speed_supported: false, applied: null, degraded: health.degraded, suppressed_by_thermal: health.suppressed, last_error: null };
        default: throw new Error('Unexpected write ' + request.method);
      }
    } } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Подсветка', exact: true }).click();
  await page.getByText('Подсветка помпы по датчику', { exact: true }).click();
  const base = page.getByText('Датчик недоступен — используется обычная подсветка.', { exact: true });
  const waiting = page.getByText('Датчик недоступен; режим по датчику ожидает восстановления.', { exact: true });
  const thermal = page.getByText('Подсветка временно перекрыта температурным оповещением.', { exact: true });
  await expect(waiting).toBeVisible();
  await expect(thermal).toBeVisible();
  await expect(base).toHaveCount(0);
  await page.evaluate(() => Object.assign(window, { rgbHealth: { degraded: true, suppressed: false } }));
  await expect(base).toBeVisible();
  await expect(waiting).toHaveCount(0);
  await expect(thermal).toHaveCount(0);
  await page.evaluate(() => Object.assign(window, { rgbHealth: { degraded: false, suppressed: false } }));
  await expect(base).toHaveCount(0);
});
