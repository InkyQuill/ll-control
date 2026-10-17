import {test,expect,type Page} from '@playwright/test';
type Kind='display'|'fps'|'sources';
interface Fixture {language:string;fail:string;invalid:string;badLanguage:unknown;badLanguageEnabled:boolean;writes:string[];display:{version:number;revision:number;temperature_unit:string;interface_language?:string};fps:{version:number;revision:number;enabled:boolean;log_path:string|null};sources:{version:number;revision:number;cpu:Record<string,string|null>;gpu:null}}
async function setup(page:Page,language:string,kind:Kind){
 await page.addInitScript(language=>{
  localStorage.setItem('lconnect.interface-language',language);
  const f:Fixture={language,fail:'',invalid:'',badLanguage:null,badLanguageEnabled:false,writes:[],display:{version:1,revision:0,temperature_unit:'celsius',interface_language:language},fps:{version:1,revision:0,enabled:false,log_path:null},sources:{version:1,revision:0,cpu:{temperature:null,clock:null,load:null,power:null,fan:null,voltage:null},gpu:null}};
  Object.assign(window,{settingsMessages:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   if(command!=='daemon_call')return null;
   const method=String(args.method),params=(args.params??{}) as Record<string,unknown>;
   if(method.startsWith('set_'))f.writes.push(method);
   if(f.fail===method)throw {message:'Настройки'};
   if(f.invalid===method)return {version:999};
   switch(method){
    case 'get_display_settings':return {...f.display,interface_language:f.badLanguageEnabled?f.badLanguage:f.language};
    case 'set_display_settings':f.display={...(params.settings as Fixture['display']),revision:f.display.revision+1};f.language=f.display.interface_language??'ru-RU';return structuredClone(f.display);
    case 'get_fps_source':return structuredClone(f.fps);
    case 'set_fps_source':f.fps={...(params.settings as Fixture['fps']),revision:f.fps.revision+1};return structuredClone(f.fps);
    case 'get_sensor_defaults':return {config:structuredClone(f.sources),recommended:structuredClone(f.sources),values:[],candidates:[]};
    case 'set_sensor_defaults':f.sources={...(params.config as Fixture['sources']),revision:f.sources.revision+1};return structuredClone(f.sources);
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    case 'list_devices':case 'list_sensors':case 'list_theme_assets':case 'list_named_theme_assets':case 'list_prepared_clips':return [];
    default:return null;
   }
  }}});
 },language);
 await page.goto('/');await page.getByRole('button',{name:kind==='sources'?(language==='en-US'?'Sensors':'Датчики'):(language==='en-US'?'Settings':'Настройки'),exact:true}).click();await expect(panel(page,kind)).toBeVisible();
}
const names={display:/^(Units and language|Единицы и язык)$/,fps:/^(FPS source|Источник FPS)$/,sources:/^(System sensor sources|Источники системных датчиков)$/};
const panel=(page:Page,kind:Kind)=>page.getByRole('region',{name:names[kind]});
const setters={display:'set_display_settings',fps:'set_fps_source',sources:'set_sensor_defaults'},getters={display:'get_display_settings',fps:'get_fps_source',sources:'get_sensor_defaults'};
const saveNames={display:/^(Save display preferences|Сохранить параметры отображения)$/,fps:/^(Save FPS settings|Сохранить настройки FPS)$/,sources:/^(Apply sources|Применить источники)$/};
const reloadNames={display:/^(Reload saved preferences|Загрузить сохранённые параметры)$/,fps:/^(Load FPS settings|Загрузить настройки FPS)$/,sources:/^(Reload saved selection|Загрузить сохранённый выбор)$/};
async function switchLanguage(page:Page,language:string){await page.evaluate(language=>{(window as unknown as {settingsMessages:Fixture}).settingsMessages.language=language;},language);await expect(page.getByRole('button',{name:language==='en-US'?'Settings':'Настройки',exact:true})).toBeVisible();}
for(const language of ['en-US','ru-RU'])for(const kind of ['display','fps','sources'] as const){
 test(`${kind} raw save error and translated recovery require explicit reload (${language})`,async({page})=>{
  await setup(page,language,kind);const p=panel(page,kind),save=p.getByRole('button',{name:saveNames[kind]});await expect(save).toBeEnabled();
  await page.evaluate(method=>{(window as unknown as {settingsMessages:Fixture}).settingsMessages.fail=method;},setters[kind]);await save.click();await expect(p.getByText('Настройки',{exact:true})).toBeVisible();
  const guidance=(en:boolean)=>kind==='display'?(en?'Read back the saved settings before writing again.':'Проверьте сохранённое состояние перед повторной записью.'):kind==='fps'?(en?'FPS save is unconfirmed. Load the saved state before writing again.':'Сохранение FPS не подтверждено. Загрузите сохранённое состояние перед повторной записью.'):(en?'Check the saved selection before trying again.':'Проверьте сохранённый выбор перед повторной попыткой.');
  await expect(p.getByText(guidance(language==='en-US'),{exact:true})).toBeVisible();await switchLanguage(page,language==='en-US'?'ru-RU':'en-US');await expect(p.getByText(guidance(language!=='en-US'),{exact:true})).toBeVisible();await expect(p.getByText('Настройки',{exact:true})).toBeVisible();
  await expect(save).toBeDisabled();const reload=p.getByRole('button',{name:reloadNames[kind]});await expect(reload).toBeEnabled();expect(await page.evaluate(()=>(window as unknown as {settingsMessages:Fixture}).settingsMessages.writes)).toEqual([setters[kind]]);
  await page.evaluate(()=>{(window as unknown as {settingsMessages:Fixture}).settingsMessages.fail='';});await reload.click();await expect(p.getByText('Настройки',{exact:true})).toHaveCount(0);await expect(save).toBeEnabled();
 });
 test(`${kind} read resource-key diagnostic remains raw (${language})`,async({page})=>{
  await setup(page,language,kind);const p=panel(page,kind);await expect(p.getByRole('button',{name:saveNames[kind]})).toBeEnabled();await page.evaluate(method=>{(window as unknown as {settingsMessages:Fixture}).settingsMessages.fail=method;},getters[kind]);await expect(p.getByText('Настройки',{exact:true})).toBeVisible();await expect(p.getByRole('button',{name:saveNames[kind]})).toBeDisabled();expect(await page.evaluate(()=>(window as unknown as {settingsMessages:Fixture}).settingsMessages.writes)).toEqual([]);
 });
}
for(const language of ['en-US','ru-RU'])for(const kind of ['display','fps'] as const)test(`${kind} owned validation follows language (${language})`,async({page})=>{
 await setup(page,language,kind);const p=panel(page,kind);await expect(p.getByRole('button',{name:saveNames[kind]})).toBeEnabled();await page.evaluate(method=>{(window as unknown as {settingsMessages:Fixture}).settingsMessages.invalid=method;},setters[kind]);await p.getByRole('button',{name:saveNames[kind]}).click();
 const text=(en:boolean)=>kind==='display'?(en?'Unsupported save response':'Неподдерживаемый ответ сохранения'):(en?'Unsupported FPS settings response':'Неподдерживаемый ответ настроек FPS');await expect(p.getByText(text(language==='en-US'),{exact:true})).toBeVisible();await switchLanguage(page,language==='en-US'?'ru-RU':'en-US');await expect(p.getByText(text(language!=='en-US'),{exact:true})).toBeVisible();
});
for(const value of ['',null,false,0])test(`display rejects malformed language ${JSON.stringify(value)}`,async({page})=>{
 await setup(page,'en-US','display');const p=panel(page,'display');await expect(p.getByRole('button',{name:saveNames.display})).toBeEnabled();await page.evaluate(value=>{const f=(window as unknown as {settingsMessages:Fixture}).settingsMessages;f.badLanguageEnabled=true;f.badLanguage=value;},value);await expect(p.getByText('The daemon did not return supported display preferences',{exact:true})).toBeVisible();await expect(page.getByRole('button',{name:'Settings',exact:true})).toBeVisible();await expect(p.getByRole('button',{name:saveNames.display})).toBeDisabled();
});
