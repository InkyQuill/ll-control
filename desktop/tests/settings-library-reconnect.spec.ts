import {test, expect} from '@playwright/test';

interface Fixture { online:boolean; hold:string; finish:(fail?:boolean)=>void }
test.beforeEach(async({page})=>{
  await page.addInitScript(()=>{
    const fixture:Fixture={online:true,hold:'',finish:()=>{}};
    Object.assign(window,{raceFixture:fixture,__TAURI_INTERNALS__:{invoke:async(_:string,req:{method:string})=>{
      if(!fixture.online)throw new Error('offline');
      let result:unknown;
      switch(req.method){
        case 'list_devices': return [{id:'A',display_name:'LCD A'},{id:'B',display_name:'LCD B'}];
        case 'list_sensors': return [];
        case 'get_lcd_state': return {source:'Theme',last_error:null,requested:{mode:'Application',brightness_percent:60,rotation_degrees:90}};
        case 'get_lcd_theme': return {version:1,name:'Image draft',background:[0,0,0],layers:[{kind:'image',asset:'draftasset',x:0,y:0,width:480,height:480,fit:'contain',opacity:255}]};
        case 'patch_lcd_config':result={};break;
        case 'list_profiles':case 'edit_profile':result={version:1,revision:1,profiles:[]};break;
        case 'cleanup_theme_assets':result={candidates:['orphan'],deleted:[],retained_referenced:0,retained_session:0,retained_draft:1,skipped:0,failures:[]};break;
        default:throw new Error(req.method);
      }
      if(fixture.hold===req.method)return new Promise((resolve,reject)=>{fixture.finish=(fail=false)=>fail?reject(new Error('old operation failed')):resolve(result);});
      return result;
    }}});
  });
  await page.goto('/');await expect(page.getByText('Daemon подключён')).toBeVisible();
});

for(const fail of [false,true])test(`settings ignores old-device ${fail?'failure':'success'} and preserves draft`,async({page})=>{
  await page.getByRole('button',{name:'Настройки',exact:true}).click();
  await page.getByRole('combobox',{name:'Ориентация',exact:true}).selectOption('180');
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.hold='patch_lcd_config';});
  const apply=page.getByRole('button',{name:'Применить поворот'});
  await apply.click();await expect(apply).toBeDisabled();
  await page.getByLabel('Устройство',{exact:true}).selectOption('B');
  await page.evaluate(fail=>{(window as unknown as {raceFixture:Fixture}).raceFixture.finish(fail);},fail);
  await expect(apply).toBeEnabled();
  await expect(page.getByRole('combobox',{name:'Ориентация',exact:true})).toHaveValue('180');
  await expect(page.getByText('Настройка отправлена.',{exact:false})).toHaveCount(0);
  await expect(page.getByText('old operation failed',{exact:false})).toHaveCount(0);
});

test('profile edit result after disconnect requires fresh library and preserves local draft',async({page})=>{
  await page.getByText('Сохранённые профили',{exact:true}).click();
  await page.getByRole('button',{name:'Обновить список профилей'}).click();
  await page.getByLabel('Имя профиля').fill('My draft');
  await page.getByLabel('Название',{exact:true}).fill('Unsaved LCD');
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.hold='edit_profile';});
  await page.getByRole('button',{name:'Сохранить черновик в слот'}).click();
  await expect(page.getByLabel('Имя профиля')).toBeDisabled();
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.online=false;});
  await expect(page.getByText('Нет связи с daemon')).toBeVisible();
  await page.evaluate(()=>{const f=(window as unknown as {raceFixture:Fixture}).raceFixture;f.online=true;f.finish();});
  await expect(page.getByText('Daemon подключён')).toBeVisible();
  await expect(page.getByLabel('Слот профиля')).toHaveCount(0);
  await expect(page.getByText('Библиотека сохранена.',{exact:false})).toHaveCount(0);
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Unsaved LCD');
  await page.getByRole('button',{name:'Обновить список профилей'}).click();
  await expect(page.getByLabel('Имя профиля')).toHaveValue('My draft');
});

test('cleanup discards scan after draft asset set changes and requires rescan',async({page})=>{
  await page.getByRole('button',{name:'Загрузить активную тему'}).click();
  await expect(page.getByLabel('Название',{exact:true})).toHaveValue('Image draft');
  await page.getByText('Очистка неиспользуемых изображений',{exact:true}).click();
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.hold='cleanup_theme_assets';});
  const scan=page.getByRole('button',{name:'Проверить неиспользуемые изображения'});
  await scan.click();await expect(scan).toBeDisabled();
  await page.getByRole('button',{name:'Удалить слой',exact:true}).click();
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.finish();});
  await expect(scan).toBeEnabled();
  await expect(page.getByText('Можно удалить:',{exact:false})).toHaveCount(0);
  await expect(page.getByRole('button',{name:'Удалить неиспользуемые изображения',exact:true})).toHaveCount(0);
  await page.evaluate(()=>{(window as unknown as {raceFixture:Fixture}).raceFixture.hold='';});
  await scan.click();await expect(page.getByText('Можно удалить: 1',{exact:false})).toBeVisible();
});
