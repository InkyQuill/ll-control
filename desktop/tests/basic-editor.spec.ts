import {selectLayer,openEditorSection} from './editor-controls';
import { test, expect } from '@playwright/test';
import raw from './fixtures/basic-editor-theme.json' with {type:'json'};
import type { Theme, ProfileLibrary, ProfileEdit } from '../src/lib/api';
declare global {interface Window {basicUi:{active:Theme|null;library:ProfileLibrary;writes:string[];pending:boolean;release?:()=>void}}}
const fixture=():Theme=>JSON.parse(JSON.stringify(raw));
for(const language of ['en-US','ru-RU'] as const)for(const width of [1280,1024])test(`${language} ${width}: independent basic banks preserve drafts with explicit Save and Apply`,async({page})=>{
 await page.setViewportSize({width,height:1000});
 await page.addInitScript(({theme,language})=>{
  localStorage.setItem('lconnect.interface-language',language);
  const state:Window['basicUi']={active:null,library:{version:1,revision:1,profiles:[{slot:0,name:'Owned banks',content:{kind:'screen',value:theme}}]},writes:[],pending:false};
  Object.assign(window,{basicUi:state,__TAURI_INTERNALS__:{invoke:async(command:string,request:{method:string;params:Record<string,unknown>}={method:'',params:{}})=>{
   if(command!=='daemon_call')return null;
   switch(request.method){
        case 'preview_lcd_editor': return {jpeg:[],layers:[]};
    case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:language};
    case 'list_devices':return [{id:'lcd:fixture',display_name:'Owned LCDV2',family:'galahad2-lcdv2'}];
    case 'list_sensors':return [{id:'fixture:cpu',label:'Owned CPU',unit:'celsius',status:'valid',value:42,age_ms:0}];
    case 'get_lcd_state':return {source:state.active?'Theme':'Idle',last_error:null,requested:{mode:'LocalUI',brightness_percent:50,rotation_degrees:0}};
    case 'get_lcd_theme':return structuredClone(state.active);
    case 'get_lcd_media':return null;
    case 'get_screen_collections':return {collections:null,persistence:{pending:false,saved:false,last_error:null}};
    case 'list_profiles':return structuredClone(state.library);
    case 'edit_profile':{state.writes.push(request.method);const edit=request.params.edit as ProfileEdit;if(edit.action!=='save')throw new Error('Unexpected operation');state.library={...state.library,revision:state.library.revision+1,profiles:[structuredClone(edit.profile)]};return structuredClone(state.library);}
    case 'set_lcd_theme':{state.writes.push(request.method);if(state.pending)await new Promise<void>(resolve=>state.release=resolve);state.active=structuredClone(request.params.theme as Theme);return {source:'Theme',last_error:null};}
    default:return [];
   }
  }}});
 },{theme:fixture(),language});
 const label=(en:string,ru:string)=>language==='en-US'?en:ru;
 await page.goto('/?view=display');
 const background=page.getByRole('combobox',{name:label('Background preset','Вариант фона'),exact:true}),text=page.getByRole('combobox',{name:label('Text preset','Вариант текста'),exact:true});
 await expect(background).toHaveCount(0);
 const backgroundRow=()=>page.getByRole('button',{name:label('Background and composition','Фон и композиция'),exact:true}).click();
 const editText=async(value?:string)=>{await selectLayer(page,0);const field=page.locator('.inspector textarea');if(value!==undefined)await field.fill(value);return field;};
 await openEditorSection(page,'Saved profiles|Сохранённые профили');
 await page.getByRole('button',{name:label('Refresh profile list','Обновить список профилей'),exact:true}).click();
 await openEditorSection(page,'Saved profiles|Сохранённые профили');await page.getByRole('button',{name:label('Load profile into editor','Загрузить профиль в редактор'),exact:true}).click();await page.keyboard.press('Escape');
 await expect(background).toHaveValue('image_video');await expect(text).toHaveValue('custom');
 await page.getByLabel(label('Name','Название'),{exact:true}).fill('Authored title');await editText('Image authored');await backgroundRow();
 await text.selectOption('time');await text.selectOption('custom');await expect(await editText()).toHaveValue('Image authored');await backgroundRow();
 await background.selectOption('background_color');await expect(await editText()).toHaveValue('Color custom');await editText('Color authored');await backgroundRow();
 await page.getByLabel(label('First background color','Первый цвет фона'),{exact:true}).fill('#a1b2c3');
 for(const [kind,count] of [['none',0],['slider_sensor',1],['duo_sensor',2],['three_sensor',5],['four_sensor',7],['time',1],['custom',1]] as const){
  await text.focus();await text.selectOption(kind);await expect(text).toBeFocused();await expect(page.locator('.layer-row[data-layer-index]')).toHaveCount(count);await expect(page.getByLabel(label('First background color','Первый цвет фона'),{exact:true})).toHaveValue('#a1b2c3');
 }
 await expect(await editText()).toHaveValue('Color authored');await backgroundRow();await background.selectOption('image_video');
 for(const kind of ['none','slider_sensor','duo_sensor','three_sensor','four_sensor','time','custom'])await text.selectOption(kind);
 await expect(await editText()).toHaveValue('Image authored');await backgroundRow();
 expect(await page.evaluate(()=>window.basicUi.writes)).toEqual([]);
 await text.selectOption('four_sensor');await selectLayer(page,4);
 const divider=page.locator('.divider-controls');await divider.getByLabel('X',{exact:true}).fill('239.5');await divider.getByLabel(label('Thickness, px','Толщина, px'),{exact:true}).fill('1.5');
 await backgroundRow();await text.selectOption('custom');await text.selectOption('four_sensor');await selectLayer(page,4);await expect(divider.getByLabel('X',{exact:true})).toHaveValue('239.5');await backgroundRow();
 await background.selectOption('background_color');await text.selectOption('four_sensor');await selectLayer(page,4);await expect(divider.getByLabel('X',{exact:true})).toHaveValue('239');await backgroundRow();await text.selectOption('custom');
 await background.selectOption('image_video');await text.selectOption('four_sensor');await selectLayer(page,4);await expect(divider.getByLabel('X',{exact:true})).toHaveValue('239.5');await backgroundRow();await text.selectOption('custom');
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await openEditorSection(page,'Saved profiles|Сохранённые профили');await page.getByRole('button',{name:label('Replace profile with draft','Заменить профиль черновиком'),exact:true}).click();await page.keyboard.press('Escape');await expect.poll(()=>page.evaluate(()=>window.basicUi.library.revision)).toBe(2);expect(await page.evaluate(()=>window.basicUi.active)).toBeNull();
 await page.evaluate(()=>window.basicUi.pending=true);await page.getByRole('button',{name:label('Apply to LCD','Применить к LCD'),exact:true}).click();await expect(background).toBeDisabled();await expect(text).toBeDisabled();await expect.poll(()=>page.evaluate(()=>typeof window.basicUi.release)).toBe('function');await page.evaluate(()=>window.basicUi.release?.());await expect(background).toBeEnabled();
 const applied=await page.evaluate(()=>window.basicUi.active);expect(applied?.basic_editor?.banks.find(bank=>bank.background==='image_video')?.overlays.find(item=>item.overlay==='four_sensor')?.theme.layers.filter(layer=>layer.kind==='divider')[0]).toMatchObject({x:239.5,thickness:1.5});expect(applied?.name).toBe('Authored title');expect(applied?.basic_editor?.banks.find(bank=>bank.background==='background_color')?.overlays.every(item=>JSON.stringify(item.theme.background_fill?.horizontal.colors[0])==='[161,178,195]')).toBe(true);
 await editText('Unsaved extra');await openEditorSection(page,'Saved profiles|Сохранённые профили');await page.getByRole('button',{name:label('Load profile into editor','Загрузить профиль в редактор'),exact:true}).click();await page.keyboard.press('Escape');await expect(await editText()).toHaveValue('Image authored');await backgroundRow();await background.selectOption('background_color');await expect(await editText()).toHaveValue('Color authored');expect(await page.evaluate(()=>window.basicUi.writes)).toEqual(['edit_profile','set_lcd_theme']);

});
test('missing targets and malformed banks do not mutate caller data',async({page})=>{
 await page.goto('/');const result=await page.evaluate(async({modulePath,theme})=>{
  const {selectBasicEditor}=await import(/* @vite-ignore */modulePath);theme.basic_editor!.banks=theme.basic_editor!.banks.filter(bank=>bank.background==='image_video');const before=JSON.stringify(theme);let absent=false,invalid=false;
  try{selectBasicEditor(theme,'background_color','custom');}catch{absent=true;}const unchanged=JSON.stringify(theme)===before;theme.basic_editor!.banks[0].overlays.pop();const malformed=JSON.stringify(theme);try{selectBasicEditor(theme,'image_video','custom');}catch{invalid=true;}
  return {absent,invalid,unchanged,invalidUnchanged:malformed===JSON.stringify(theme)};
 },{modulePath:'/src/lib/basic-editor.ts',theme:fixture()});expect(result).toEqual({absent:true,invalid:true,unchanged:true,invalidUnchanged:true});
});
