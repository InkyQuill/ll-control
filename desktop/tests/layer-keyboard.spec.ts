import {loadControls,selectLayer,addControls} from './editor-controls';
import {test,expect} from '@playwright/test';
test('keyboard reorder and deletion keep focus on the edited layer or add action',async({page})=>{
  await page.addInitScript(()=>{
    Object.assign(window,{__TAURI_INTERNALS__:{invoke:async(_:string,req:{method:string})=>{
        if (_ === 'desktop_set_language') return null;
      const style={x:0,y:0,size:24,color:[255,255,255],align:'left'};
      switch(req.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'test',display_name:'LCD', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Theme',requested:{},last_error:null};
        case 'get_lcd_theme':return {version:1,name:'Keyboard layers',background:[0,0,0],layers:[{kind:'text',text:'A',style},{kind:'text',text:'B',style}]};
        default:throw new Error('unexpected device write '+req.method);
      }
    }}});
  });
  await page.goto('/?view=display');await expect(page.getByText('Daemon подключён')).toBeVisible();
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему'}).focus();await page.keyboard.press('Enter');
  await selectLayer(page,0);
  const editor=page.getByRole('region',{name:'Редактор темы'});
  await expect(editor.getByLabel('Текст',{exact:true})).toHaveValue('A');
  await page.locator('.layer-row[data-layer-index="0"]').focus();await page.keyboard.press('Alt+ArrowUp');
  await expect(page.locator('.layer-row[data-layer-index="1"]')).toHaveAttribute('aria-pressed','true');
  await expect(editor.getByLabel('Текст',{exact:true})).toHaveValue('A');
  await editor.getByRole('button',{name:'Удалить слой',exact:true}).focus();await page.keyboard.press('Enter');
  await expect(editor.getByRole('heading',{name:'Текст',exact:true})).toBeFocused();
  await expect(editor.getByLabel('Текст',{exact:true})).toHaveValue('B');
  await editor.getByRole('button',{name:'Удалить слой',exact:true}).focus();await page.keyboard.press('Enter');
  await expect(page.locator('.add-layer summary')).toBeFocused();
  await addControls(page);await editor.getByRole('button',{name:'+ Текст',exact:true}).focus();await page.keyboard.press('Enter');
  await expect(editor.getByLabel('Текст',{exact:true})).toHaveValue('Новый текст');

});

test('deleting cycle readings keeps keyboard focus on the remaining source', async ({ page }) => {
  await page.addInitScript(() => {
    Object.assign(window, { __TAURI_INTERNALS__: { invoke: async (_: string, req: { method: string }) => {
        if (_ === 'desktop_set_language') return null;
      const style = { x: 240, y: 200, size: 32, color: [255, 255, 255], align: 'center' };
      const sensor = (id: string) => ({ sensor_id: id, label: id, unit: 'celsius', temperature_unit: 'celsius', decimals: 1 });
      switch (req.method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices': return [{ id: 'test', display_name: 'LCD', family:'galahad2-lcdv2' }];
        case 'list_sensors': return ['CPU', 'GPU', 'Water'].map(id => ({ id, label: id, unit: 'celsius', value: 42, status: 'valid' }));
        case 'get_lcd_state': return { source: 'Theme', requested: {}, last_error: null };
        case 'get_lcd_theme': return { version: 1, name: 'Cycle keyboard', background: [0, 0, 0], layers: [
          { kind: 'sensor_cycle', interval_seconds: 3, style, sensors: ['CPU', 'GPU', 'Water'].map(sensor) },
        ] };
        default: throw new Error('unexpected write ' + req.method);
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await loadControls(page);await page.getByRole('button', { name: 'Загрузить активную тему', exact: true }).click();
  await selectLayer(page,0);
  const removeLast = page.getByRole('button', { name: 'Удалить показатель 3', exact: true });
  await expect(removeLast).toBeVisible();
  await removeLast.focus(); await page.keyboard.press('Enter');
  const source = page.getByRole('combobox', { name: 'Источник 2', exact: true });
  await expect(source).toBeFocused();
  await expect(source).toHaveValue('GPU');
  await page.keyboard.press('Tab');
  await expect(page.getByLabel('Подпись 2', { exact: true })).toBeFocused();
  const removeFirst = page.getByRole('button', { name: 'Удалить показатель 1', exact: true });
  await removeFirst.focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('combobox', { name: 'Источник 1', exact: true })).toBeFocused();
  await expect(page.getByRole('combobox', { name: 'Источник 1', exact: true })).toHaveValue('GPU');
  await expect(removeFirst).toBeDisabled();
});
