import {test,expect} from '@playwright/test';
test('settings write failure is distinct from LCD error and recovers on status polling',async({page})=>{
  await page.addInitScript(()=>{
    const save={pending:true,saved:false,last_error:null as string|null};
    Object.assign(window,{saveFixture:save,__TAURI_INTERNALS__:{invoke:async(_cmd:string,req:{method:string})=>{
        if (_cmd === 'desktop_set_language') return null;
      switch(req.method){
        case 'list_devices':return [{id:'lcd:test',display_name:'LCD', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Theme',last_error:null,requested:{},persistence:{...save}};
        default:throw new Error('unexpected '+req.method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Настройки ожидают записи на диск.')).toBeVisible();
  await page.evaluate(()=>{(window as unknown as {saveFixture:{last_error:string|null}}).saveFixture.last_error='disk full';});
  await expect(page.getByRole('alert')).toContainText('Не удалось сохранить настройки: disk full');
  await expect(page.getByText('Источник: Тема LCD')).toBeVisible();
  await page.evaluate(()=>{Object.assign((window as unknown as {saveFixture:object}).saveFixture,{pending:false,saved:true,last_error:null});});
  await expect(page.getByText('Последняя запись настроек завершена.')).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
});
