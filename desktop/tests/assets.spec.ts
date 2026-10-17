import { test, expect } from '@playwright/test';
test('cleanup previews before deletion and protects current draft assets', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { delete: boolean; keep: string[] }[] = [];
    Object.assign(window, { cleanupCalls: calls, __TAURI_INTERNALS__: { invoke: async (_:string, req:{method:string;params:{delete:boolean;keep:string[]}}) => {
        if (_ === 'desktop_set_language') return null;
      switch(req.method) {
        case 'list_devices': return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source:'Theme',last_error:null,requested:{mode:'Application',brightness_percent:100,rotation_degrees:0}};
        case 'get_lcd_theme': return {version:1,name:'Draft',background:[0,0,0],layers:[{kind:'image',asset:'draftasset',x:0,y:0,width:480,height:480,fit:'contain',opacity:255}]};
        case 'cleanup_theme_assets': calls.push(req.params); return {candidates:['orphan'],deleted:req.params.delete ? ['orphan'] : [],retained_referenced:1,retained_session:2,retained_draft:1,skipped:0,failures:[]};
        default: throw {message:'Unexpected '+req.method};
      }
    } } });
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:'Загрузить активную тему'}).click();
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Draft');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByText('Очистка неиспользуемых изображений',{exact:true}).click();
  await expect(page.getByRole('button',{name:'Удалить неиспользуемые изображения'})).toHaveCount(0);
  await page.getByRole('button',{name:'Проверить неиспользуемые изображения'}).click();
  await expect(page.getByRole('status')).toContainText('Можно удалить: 1');
  await page.getByRole('button',{name:'Удалить неиспользуемые изображения'}).click();
  await expect(page.getByRole('status')).toContainText('Удалено: 1');
  expect(await page.evaluate(() => (window as unknown as {cleanupCalls:unknown[]}).cleanupCalls)).toEqual([
    {delete:false,keep:['draftasset']},{delete:true,keep:['draftasset']}
  ]);
});
