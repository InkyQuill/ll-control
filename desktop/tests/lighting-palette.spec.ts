import {test,expect} from '@playwright/test';
import {lightingColorCount} from '../src/lib/lighting-palette';
test('manual palettes use mode-specific slots and send all selected colors',async({page})=>{
  await page.addInitScript(()=>{
    const writes:unknown[]=[];
    Object.assign(window,{paletteWrites:writes,__TAURI_INTERNALS__:{invoke:async (_:string,req:{method:string;params:unknown})=>{
        if (_ === 'desktop_set_language') return null;
      switch(req.method){
        case 'list_devices':return [{id:'A',display_name:'LCD A', family:'galahad2-lcdv2'}];
        case 'list_sensors':return [];
        case 'get_lighting_info':return {modes:['static','runway','meteor','rainbow']};
        case 'get_thermal_state':return {requested:{cpu:null,gpu:null},degraded_sources:[],fan:{last_error:null},pump:{last_error:null},grace_remaining_ms:0};
        case 'set_lighting':writes.push(req.params);return null;
        default:return null;
      }
    }}});
  });
  await page.goto('/?view=display');await page.getByRole('button',{name:'Подсветка',exact:true}).click();
  await expect(page.getByRole('combobox',{name:'Эффект',exact:true})).toHaveValue('static');
  await expect(page.getByLabel('Цвет RGB',{exact:true})).toBeVisible();
  await expect(page.getByLabel('Цвет RGB 2',{exact:true})).toHaveCount(0);
  await page.getByRole('combobox',{name:'Эффект',exact:true}).selectOption('meteor');
  for(const [index,color] of ['#010203','#040506','#070809','#0a0b0c'].entries()){
    await page.getByLabel(index===0?'Цвет RGB':`Цвет RGB ${index+1}`,{exact:true}).fill(color);
  }
  await page.getByRole('button',{name:'Применить подсветку',exact:true}).click();
  const sent=await page.evaluate(()=>(window as unknown as {paletteWrites:unknown[]}).paletteWrites);
  expect(sent).toMatchObject([{kind:'pump',cfg:{mode:'meteor',color:{r:1,g:2,b:3},additional_colors:[{r:4,g:5,b:6},{r:7,g:8,b:9},{r:10,g:11,b:12}]}}]);
  await page.getByRole('combobox',{name:'Эффект',exact:true}).selectOption('runway');
  await expect(page.getByLabel('Цвет RGB 2',{exact:true})).toHaveValue('#040506');
  await expect(page.getByLabel('Цвет RGB 3',{exact:true})).toHaveCount(0);
  await page.getByRole('combobox',{name:'Эффект',exact:true}).selectOption('rainbow');
  await expect(page.getByLabel('Цвет RGB',{exact:true})).toHaveCount(0);
  await page.getByRole('combobox',{name:'Зона',exact:true}).selectOption('fan');
  await page.getByRole('combobox',{name:'Эффект',exact:true}).selectOption('meteor');
  await expect(page.getByLabel('Цвет RGB 4',{exact:true})).toBeVisible();
  await page.getByLabel('Цвет RGB',{exact:true}).fill('#ad85ff');
  await page.getByRole('button',{name:'Обзор',exact:true}).click();
  await page.locator('nav').getByRole('button',{name:'Подсветка',exact:true}).click();
  await expect(page.getByLabel('Цвет RGB',{exact:true})).toHaveValue('#ad85ff');
  await page.evaluate(() => { (document.activeElement as HTMLElement)?.blur(); window.scrollTo(0, 0); });
  await page.screenshot({path:'../.impeccable/review/production-lighting.png',fullPage:true});
});
test('palette inventory covers the Windows 2.1.28 fan/pump modes',()=>{
  for(const mode of ['rainbow','rainbow_morph','colorful_starry_night','colors_morph'])expect(lightingColorCount('pump',mode)).toBe(0);
  for(const mode of ['static','static_color','breathing','static_starry_night','voice','burst'])expect(lightingColorCount('pump',mode)).toBe(1);
  for(const mode of ['runway','transmit'])expect(lightingColorCount('pump',mode)).toBe(2);
  for(const mode of ['meteor','ticker_tape','fluctuation','big_bang','bounce'])expect(lightingColorCount('pump',mode)).toBe(4);
  expect(lightingColorCount('fan','transmit')).toBeNull();
});
