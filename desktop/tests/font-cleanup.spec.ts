import {loadControls,selectLayer} from './editor-controls';
import { test, expect } from '@playwright/test';
test('font cleanup protects loaded draft keys and requires fresh scan after selection changes', async ({page}) => {
  await page.addInitScript(() => {
    const key='0123456789abcdef0123456789abcdef';
    Object.assign(window,{__TAURI_INTERNALS__:{invoke:async(_:string,{method,params}:{method:string;params:{delete?:boolean;keep?:string[]}})=>{
        if (_ === 'desktop_set_language') return null;
      switch(method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'lcd:test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'Theme',last_error:null,requested:{brightness_percent:100,rotation_degrees:0,mode:'Application'}};
        case 'get_lcd_theme':return {version:1,name:'Font draft',background:[0,0,0],layers:[{kind:'text',text:'A',style:{font_asset:key,x:1,y:1,size:20,color:[255,255,255],align:'left'}}]};
        case 'preview_lcd_theme':return [255,216,255,217];
        case 'cleanup_theme_fonts':return {candidates:params.keep?.includes(key)?[]:[key],deleted:params.delete?[key]:[],retained_referenced:0,retained_session:0,retained_draft:params.keep?.length??0,skipped:0,failures:[]};
        default:throw new Error('Unexpected '+method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await loadControls(page);await page.getByRole('button',{name:'Загрузить активную тему'}).click();
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Font draft');
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Очистка неиспользуемых шрифтов',exact:true}).click();
  await page.getByRole('button',{name:'Проверить неиспользуемые шрифты',exact:true}).click();
  await expect(page.getByText('Можно удалить: 0',{exact:false})).toBeVisible();
  await expect(page.getByText('В черновике: 1',{exact:false})).toBeVisible();
  await page.getByRole('button',{name:'Композиция',exact:true}).click();await selectLayer(page,0);
  await page.getByRole('combobox',{name:'Шрифт',exact:true,includeHidden:true}).selectOption('sans');
  await page.getByRole('button',{name:'Библиотека',exact:true}).click();
  await page.getByRole('button',{name:'Очистка неиспользуемых шрифтов',exact:true}).click();
  await expect(page.getByText('Можно удалить:',{exact:false})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Удалить неиспользуемые шрифты',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Проверить неиспользуемые шрифты',exact:true}).click();
  await expect(page.getByText('Можно удалить: 1',{exact:false})).toBeVisible();
  await page.getByRole('button',{name:'Удалить неиспользуемые шрифты',exact:true}).click();
  await expect(page.getByText('Удалено: 1',{exact:false})).toBeVisible();
});

test('partial font cleanup shows failure and supports explicit rescan and retry', async ({page}) => {
  await page.addInitScript(() => {
    const failed='11111111111111111111111111111111', removed='22222222222222222222222222222222';
    let deletes=0;
    Object.assign(window,{__TAURI_INTERNALS__:{invoke:async(_:string,{method,params}:{method:string;params:{delete?:boolean}})=>{
        if (_ === 'desktop_set_language') return null;
      switch(method) {
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
        case 'list_devices':return [{id:'lcd:test',display_name:'LCDV2', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lcd_state':return {source:'None',last_error:null,requested:{brightness_percent:100,rotation_degrees:0,mode:'Application'}};
        case 'cleanup_theme_fonts': {
          if(params.delete)deletes++;
          return {candidates:deletes===0?[failed,removed]:[failed],deleted:params.delete?(deletes===1?[removed]:[failed]):[],
            failures:params.delete&&deletes===1?[{key:failed,error:'permission denied'}]:[],
            retained_referenced:0,retained_session:0,retained_draft:0,skipped:0};
        }
        default:throw new Error('Unexpected '+method);
      }
    }}});
  });
  await page.goto('/?view=display');
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await page.getByRole('button',{name:/^(Библиотека|Library)$/}).click();
  await page.getByRole('button',{name:'Очистка неиспользуемых шрифтов',exact:true}).click();
  const scan=page.getByRole('button',{name:'Проверить неиспользуемые шрифты',exact:true});
  const remove=page.getByRole('button',{name:'Удалить неиспользуемые шрифты',exact:true});
  await scan.click();
  await expect(page.getByRole('status').filter({hasText:'Можно удалить: 2'})).toBeVisible();
  await remove.click();
  await expect(page.getByRole('alert').filter({hasText:'11111111111111111111111111111111: permission denied'})).toBeVisible();
  await expect(page.getByRole('status').filter({hasText:'Удалено: 1'})).toBeVisible();
  await expect(remove).toHaveCount(0);
  await scan.click();
  await expect(page.getByRole('status').filter({hasText:'Можно удалить: 1'})).toBeVisible();
  await remove.click();
  await expect(page.getByRole('status').filter({hasText:'Удалено: 1'})).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
});
