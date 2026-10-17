import { test, expect } from '@playwright/test';
test('edits stay local until Apply; preview and sensor selection use daemon bridge', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { method: string; params: Record<string, unknown> }[] = [];
    Object.assign(window, { bridgeCalls: calls, __TAURI_INTERNALS__: {
      invoke: async (_command: string, args: { method: string; params: Record<string, unknown> }) => {
        if (_command === 'desktop_set_language') return null;
        if (_command === 'import_image') { calls.push({ method: 'import_image', params: {} }); return 'test-image-key'; }
        calls.push(args);
        switch (args.method) {
          case 'get_screen_collections': return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
          case 'list_devices': return [{ id: 'lcd:test', display_name: 'Test LCDV2', family:'galahad2-lcdv2' }];
          case 'list_sensors': return [{ id: 'cpu:test', label: 'CPU package', unit: 'celsius', value: 42.5, status: 'valid', age_ms: 100 }];
          case 'get_lcd_state': return { source: 'Idle', last_error: null, requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 } };
          case 'set_lcd_theme': return { source: 'Theme', last_error: null, requested: { mode: 'LocalUI', brightness_percent: 100, rotation_degrees: 0 } };
          case 'preview_lcd_theme': return Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII='), c => c.charCodeAt(0));
          default: throw { message: 'Mock has no handler for ' + args.method };
        }
      }
    } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByLabel('Название', { exact: true }).fill('Моя помпа');
  await page.getByRole('combobox',{name:'Шрифт',exact:true}).first().selectOption('serif');
  await page.getByLabel('Переносить текст по ширине').first().check();
  await page.getByLabel('Ширина текста, px').fill('300');
  await page.getByLabel('Задать межстрочный интервал').first().check();
  await page.getByLabel('Межстрочный интервал, px').fill('58');
  await page.getByLabel('Вертикальный якорь',{exact:true}).first().selectOption('center');
  await page.getByLabel('Тень текста',{exact:true}).first().check();
  await page.getByLabel('Непрозрачность текста, %',{exact:true}).first().fill('75');
  await page.getByLabel('Размытие тени',{exact:true}).fill('4');
  await page.locator('input[type=file][accept^="image/"]').setInputFiles({ name: 'image.png', mimeType: 'image/png', buffer: Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=', 'base64') });
  await expect(page.getByRole('status')).toContainText('Изображение добавлено');
  await page.getByLabel('Положение watermark как в L-Connect', {exact:true}).check();
  await page.getByLabel('Масштаб').selectOption('contain_native');
  await page.getByLabel('Watermark по горизонтали', {exact:true}).fill('100');
  await page.getByLabel('Watermark по вертикали', {exact:true}).fill('25');
  await page.getByLabel('Тень изображения', {exact:true}).check();
  await page.getByLabel('Смещение тени изображения X', {exact:true}).fill('-4');
  await page.getByLabel('Размытие тени изображения', {exact:true}).fill('6');
  await page.getByLabel('Привязать датчик').selectOption('cpu:test');
  await page.getByRole('button', { name: 'Добавить', exact: true }).click();
  await expect(page.getByLabel('Подпись')).toHaveValue('CPU package');
  await page.getByLabel('Раздельные стили подписи и единицы').check();
  await page.getByRole('group', {name:'Подпись датчика', exact:true}).getByLabel('Цвет',{exact:true}).fill('#ff0000');
  await page.getByRole('group', {name:'Единица датчика', exact:true}).getByLabel('Цвет',{exact:true}).fill('#00ff00');
  const writes = () => page.evaluate(() => (window as unknown as { bridgeCalls: { method: string }[] }).bridgeCalls.filter(c => c.method === 'set_lcd_theme').length);
  expect(await writes()).toBe(0);
  await page.getByRole('button', { name: 'Обновить preview' }).click();
  await expect(page.getByRole('status')).toContainText('На устройство ничего не отправлено');
  expect(await writes()).toBe(0);
  await page.getByRole('button', { name: 'Применить к LCD' }).click();
  await expect(page.getByRole('status')).toContainText('Тема отправлена');
  const applied = await page.evaluate(() => (window as unknown as { bridgeCalls: { method: string; params: { theme: { name: string; layers: { kind: string; sensor_id?: string; style?: {font:string;opacity:number;shadow:{blur:number}} }[] } } }[] }).bridgeCalls.find(c => c.method === 'set_lcd_theme')?.params.theme);
  expect(applied?.name).toBe('Моя помпа');
  expect(applied?.layers.find(l=>l.kind==='text')?.style).toMatchObject({font:'serif',opacity:75,shadow:{blur:4},max_width:300,line_height:58,vertical_align:'center'});
  expect(applied?.layers[0]).toMatchObject({ kind: 'image', asset: 'test-image-key', shadow:{x:-4,blur:6}, watermark_position:{horizontal:100,vertical:25},fit:'contain_native' });
  expect(applied?.layers.find(l=>l.kind==='sensor')).toMatchObject({parts:{label:{color:[255,0,0]},unit:{color:[0,255,0]}}});
  expect(applied?.layers.some(l => l.kind === 'sensor' && l.sensor_id === 'cpu:test')).toBe(true);
  await page.getByRole('button', { name: 'Датчики', exact: true }).click();
  await expect(page.getByText('42.5', { exact: false })).toBeVisible();
});
test('unavailable daemon shows error and disables applying', async ({ page }) => {
  await page.addInitScript(() => Object.assign(window, { __TAURI_INTERNALS__: { invoke: async () => { throw { message: 'Daemon unavailable' }; } } }));
  await page.goto('/?view=display');
  await expect(page.getByRole('alert')).toContainText('Daemon unavailable');
  await expect(page.getByRole('button', { name: 'Применить к LCD' })).toBeDisabled();
});
