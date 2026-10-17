import { test, expect } from '@playwright/test';
for (const preset of ['duo','triple','cycle'] as const) {
test(preset + ' uses explicit selected sensors and stays a draft until Apply', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { method: string; params: Record<string, unknown> }[] = [];
    Object.assign(window, { bridgeCalls: calls, __TAURI_INTERNALS__: { invoke: async (_command: string, request: { method: string; params: Record<string, unknown> }) => {
        if (_command === 'desktop_set_language') return null;
      calls.push(request);
      switch(request.method) {
        case 'list_devices': return [{ id:'lcd:test',display_name:'LCDV2', family:'galahad2-lcdv2' }];
        case 'list_sensors': return ['cpu','gpu','coolant'].map(id => ({ id,label:id.toUpperCase(),unit:'celsius',value:42,status:'valid',age_ms:10 }));
        case 'get_lcd_state': case 'set_lcd_theme': return { source:'Theme',last_error:null,requested:{mode:'Application',brightness_percent:100,rotation_degrees:0} };
        default: throw { message:'Unexpected ' + request.method };
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:'Готовые темы',exact:true}).click();
  await page.getByRole('combobox', { name:'Раскладка',exact:true }).selectOption(preset);
  const create = page.getByRole('button', { name:'Создать черновик из раскладки' });
  await expect(create).toBeDisabled();
  await page.getByRole('checkbox', { name:'GPU · celsius' }).check();
  await page.getByRole('checkbox', { name:'CPU · celsius' }).check();
  if (preset !== 'duo') await page.getByRole('checkbox', {name:'COOLANT · celsius'}).check();
  await create.click();
  await expect(page.getByLabel('Название', { exact:true })).toHaveValue(preset[0].toUpperCase()+preset.slice(1));
  for (const checkbox of await page.getByRole('checkbox', { name: 'Высота блока по строкам (CSS)', exact: true }).all()) await expect(checkbox).toBeChecked();
  const applied = () => page.evaluate(() => (window as unknown as { bridgeCalls: {method:string;params:{theme?:{layers:{kind:string;sensor_id?:string;interval_seconds?:number;sensors?:{sensor_id:string}[]}[]}}}[] }).bridgeCalls.filter(c => c.method === 'set_lcd_theme'));
  expect(await applied()).toHaveLength(0);
  if (preset === 'cycle') {
    await page.getByLabel('Интервал переключения, с').fill('5');
    await page.getByRole('button',{name:'Добавить показатель в цикл'}).click();
    await page.getByRole('button',{name:'Удалить показатель 4',exact:true}).click();
  }
  await page.getByRole('button', {name:'Применить к LCD'}).click();
  await expect(page.getByRole('status')).toContainText('Тема отправлена');
  const layers=(await applied())[0].params.theme?.layers;
  if (preset === 'cycle') {
    expect(layers?.find(l=>l.kind==='sensor_cycle')).toMatchObject({interval_seconds:5,sensors:[{sensor_id:'gpu'},{sensor_id:'cpu'},{sensor_id:'coolant'}]});
  } else {
    expect(layers?.filter(l => l.kind === 'sensor').map(l => l.sensor_id)).toEqual(preset==='duo'?['gpu','cpu']:['gpu','cpu','coolant']);
  }
});

}

test('Clock preset exposes independent date, time and period colors', async ({page}) => {
  await page.addInitScript(() => {
    const calls: {method:string;params:unknown}[]=[];
    Object.assign(window,{clockCalls:calls,__TAURI_INTERNALS__:{invoke:async(_:string,req:{method:string;params:unknown})=>{
        if (_ === 'desktop_set_language') return null;
      calls.push(req);
      switch(req.method){
        case 'list_devices':return [{id:'lcd:test',display_name:'LCD', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Idle'};
        case 'set_lcd_theme':return {source:'Theme'};
        default:throw new Error(req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await page.getByRole('button',{name:'Готовые темы',exact:true}).click();
  await page.getByLabel('Раскладка',{exact:true}).selectOption('clock');
  await page.getByRole('button',{name:'Создать черновик из раскладки'}).click();
  for (const checkbox of await page.getByRole('checkbox', { name: 'Высота блока по строкам (CSS)', exact: true }).all()) await expect(checkbox).toBeChecked();
  await expect(page.getByLabel('Содержимое часов')).toHaveCount(3);
  await expect(page.getByLabel('Содержимое часов').nth(0)).toHaveValue('date');
  await expect(page.getByLabel('Содержимое часов').nth(1)).toHaveValue('time_without_period');
  await expect(page.getByLabel('Содержимое часов').nth(2)).toHaveValue('period');
  await page.getByLabel('Цвет',{exact:true}).nth(0).fill('#ff0000');
  await page.getByLabel('Цвет',{exact:true}).nth(1).fill('#00ff00');
  await page.getByLabel('Цвет',{exact:true}).nth(2).fill('#0000ff');
  await page.getByRole('button',{name:'Применить к LCD'}).click();
  await expect(page.getByRole('status')).toContainText('Тема отправлена');
  const payload=await page.evaluate(() => (window as unknown as {clockCalls:{method:string;params:{theme:{layers:{format:string;style:{color:number[]}}[]}}}[]}).clockCalls.find(c=>c.method==='set_lcd_theme')?.params.theme);
  expect(payload?.layers.map(l=>l.style.color)).toEqual([[255,0,0],[0,255,0],[0,0,255]]);
});
