import { test, expect } from '@playwright/test';

test('gradient settings survive mode changes and explicit save/load without applying drafts', async ({ page }) => {
  await page.addInitScript(() => {
    let theme: unknown = null;
    const calls: {method:string;params:Record<string,unknown>}[] = [];
    Object.assign(window, { gradientCalls:calls, __TAURI_INTERNALS__: { invoke:async (_:string, request:{method:string;params:Record<string,unknown>}) => {
        if (_ === 'desktop_set_language') return null;
      calls.push(structuredClone(request));
      switch(request.method) {
        case 'list_devices': return [{id:'lcd:test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source:'Theme',last_error:null,requested:{mode:'Application',brightness_percent:100,rotation_degrees:0}};
        case 'set_lcd_theme': theme=structuredClone(request.params.theme); return {source:'Theme',last_error:null};
        case 'get_lcd_theme': return structuredClone(theme);
        default: throw new Error(request.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  const type = page.getByRole('combobox', {name:'Тип фона', exact:true});
  await expect(type).toHaveValue('solid');
  await type.selectOption('horizontal');
  await page.getByLabel('Первый цвет фона', {exact:true}).fill('#ff0000');
  await page.getByLabel('Второй цвет фона', {exact:true}).fill('#0000ff');
  await page.getByLabel('Обратить градиент').check();
  await type.selectOption('vertical');
  await expect(page.getByLabel('Первый цвет фона', {exact:true})).toHaveValue('#34e6d1');
  await expect(page.getByLabel('Обратить градиент')).not.toBeChecked();
  await page.getByLabel('Первый цвет фона', {exact:true}).fill('#00ff00');
  await type.selectOption('solid');
  await page.getByLabel('Фон', {exact:true}).fill('#123456');
  await type.selectOption('horizontal');
  await expect(page.getByLabel('Первый цвет фона', {exact:true})).toHaveValue('#ff0000');
  await expect(page.getByLabel('Второй цвет фона', {exact:true})).toHaveValue('#0000ff');
  await expect(page.getByLabel('Обратить градиент')).toBeChecked();
  const writes = () => page.evaluate(() => (window as unknown as {gradientCalls:{method:string;params:Record<string,unknown>}[]}).gradientCalls.filter(c=>c.method==='set_lcd_theme'));
  expect(await writes()).toHaveLength(0);
  await page.getByRole('button',{name:'Применить к LCD',exact:true}).click();
  await expect(page.getByRole('status')).toContainText('Тема отправлена');
  expect((await writes())[0].params.theme).toMatchObject({background:[18,52,86],background_fill:{mode:'horizontal',horizontal:{colors:[[255,0,0],[0,0,255]],flip:true},vertical:{colors:[[0,255,0],[26,115,105]],flip:false}}});
  await type.selectOption('radial');
  await page.getByRole('button',{name:'Загрузить активную тему',exact:true}).click();
  await expect(type).toHaveValue('horizontal');
  await expect(page.getByLabel('Обратить градиент')).toBeChecked();
  expect(await writes()).toHaveLength(1);
});
