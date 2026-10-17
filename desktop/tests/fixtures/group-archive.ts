import {expect,type Page} from '@playwright/test';
import type {GroupStatus} from '../../src/lib/group-archive';
export interface Fixture {job:GroupStatus|null;calls:{command:string;args:Record<string,unknown>}[];lost:boolean;lostFinish?:boolean;rejectWindows?:boolean;commits?:number;holdPreview?:boolean;rejectPreview?:boolean;releasePreview?:(()=>void)|null}
export const panel=(p:Page)=>p.getByText('Резервная копия группы настроек',{exact:true}).locator('..');
export async function setup(page:Page,language='ru-RU'){
 await page.addInitScript(language=>{
  localStorage.setItem('lconnect.interface-language',language);
  const summary={version:1,slots:[{group:'screen',slot:7,name:'Ночной экран'}],lcd:{mode:'Application',brightness_percent:42,rotation_degrees:90},theme_name:'Часы и температура',media_key:null,fan_manual_percent:0,pump_manual:{percent:55,source:'mcu'},fan_led_mode:'static',pump_led_mode:'rainbow',cooling:{fan:{mode:'fixed',percent:0},pump:{mode:'motherboard',percent:55}},dynamic_rgb:null};
  const f={language,job:null as null|Record<string,unknown>,calls:[] as {command:string;args:Record<string,unknown>}[],lost:false,lostFinish:false,rejectWindows:false,holdPreview:false,rejectPreview:false,releasePreview:null as (()=>void)|null,commits:Number(localStorage.getItem('test.windows.commits')??0)};
  function committed(){if(f.job?.mode==='windows'&&localStorage.getItem('test.windows.receipt')!==f.job.id){f.commits++;localStorage.setItem('test.windows.commits',String(f.commits));localStorage.setItem('test.windows.receipt',String(f.job.id));}const j=f.job!;j.phase='complete';j.readback={dev:j.dev,group:j.group,operation_id:j.id,receipt:'opaque durable receipt',selected_content_current:true,summary:j.summary??summary,runtime:{revision:'opaque runtime token',last_error:null,recovery_required:false}};}
  Object.assign(window,{groupFixture:f,commitGroup:committed,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
        if (command === 'desktop_set_language') return null;
   if(command==='group_windows_prepare') {
    f.calls.push({command,args:structuredClone(args)});if(f.rejectWindows)throw {message:'owned binding rejection'};
    if((args.request as Record<string,unknown>).mode==='windows_screen'){
     const options=args.options as {record:number;inspect_only?:boolean;bindings_json?:string;resources_json?:string};
     if(options.inspect_only){f.job!.phase='bindings';f.job!.windows_screen_requirements={record_index:options.record,backup_sha256:'a'.repeat(64),bindings:{sensor_roles:['CPUTemperature','PumpRPM'],fonts:['HyperspaceRace_BoldItalic'],resources:{'file:///C:/owned/background.mp4':false,'file:///C:/owned/watermark.png':true}},payloads:[{entry:'wmsq/art.png',bytes:100,kind:'image'},{entry:'wmsq/clip.mp4',bytes:1000,kind:'video'}],deferred:[],payloads_decoded:false,apply_supported:false};}
     else{f.job!.phase='prepared';f.job!.windows_screen_options=options;f.job!.snapshot={expected:'screen target'};f.job!.summary={...summary,theme_name:JSON.parse(options.bindings_json!).name,fan_manual_percent:null,pump_manual:null,cooling:null,fan_led_mode:null,pump_led_mode:null};}
     return structuredClone(f.job);
    }
    const options=args.options as {record:number;bindings:Record<string,{sensor_id:string;unit:string}>};
    const names=['rainbow','rainbow_morph','static','breathing','runway','meteor','ticker_tape','fluctuation','transmit','colorful_starry_night','static_starry_night','voice','big_bang','burst','colors_morph','bounce'];
    const modes=names.map(mode=>({mode,color:{r:255,g:0,b:0},brightness_percent:75,speed_percent:50,direction:'forward',dynamic_mode:mode==='big_bang'}));
    f.job!.summary={...summary,lcd:null,theme_name:null,slots:[{group:'fan',slot:2,name:'Keep Linux profile'}],fan_manual_percent:null,pump_manual:null,cooling:{fan:{mode:'fixed',percent:45},pump:{mode:'fixed_rpm',rpm:2200},fan_mode:'StandardSpeed',pump_mode:'PWMQuiet'}};f.job!.phase='prepared';f.job!.windows_options=options;f.job!.snapshot={expected:'original:window:target'};
    f.job!.windows_lighting={fan_topology:{sync_to_pump:false,size:'360',manual_led_count:24},fan_modes:modes.slice(0,6),pump_modes:modes,dynamic_rgb_profiles:{modes:names.map(mode=>({mode,enabled:mode==='big_bang',bindings:Object.values(options.bindings).map(b=>({...b,low_value:30,high_value:60,low_colors:[{r:255,g:0,b:0}],high_colors:[{r:0,g:255,b:0}],low_speed_percent:25,high_speed_percent:75,brightness_percent:90,direction:'forward'}))}))}};
    return structuredClone(f.job);
   }
   if(command.startsWith('group_archive_')){
    f.calls.push({command,args:structuredClone(args)});
    if(command==='group_archive_status')return structuredClone(f.job);
    if(command==='group_archive_start'){const r=args.request as Record<string,unknown>;f.job={...r,phase:r.mode==='export'?'downloading':'prepared',transferred:1024,total:1024,file_name:r.mode==='reset'?null:'backup.lcg',message:null,summary,readback:null,snapshot:{expected:'opaque original token'}};if(r.mode==='windows'||r.mode==='windows_screen'){f.job.phase='bindings';f.job.summary=null;f.job.snapshot=null;f.job.windows_source={source_version:'2.1.28.0',backup_sha256:'a'.repeat(64),controllers:[{record_index:1,profile_sha256:'b'.repeat(64)},{record_index:3,profile_sha256:'c'.repeat(64)}]};}if(f.lost){f.lost=false;throw {message:'Start reply lost'};}return structuredClone(f.job);}
    if(command==='group_archive_action'){
     const r=args.request as Record<string,unknown>;
     if(args.action==='dismiss'){f.job=null;return null;}
     if(args.action==='recover'){f.job={...r,transferred:1024,total:1024,file_name:null,message:null,summary,readback:null};committed();}
     if(args.action==='finish'||args.action==='reapply'){committed();if(f.lostFinish){f.lostFinish=false;f.job!.phase='uncertain';f.job!.readback=null;if(f.job!.mode==='windows'){f.job!.message='owned post-commit reply loss';f.job!.message_code='windows_import_unconfirmed';return structuredClone(f.job);}throw {message:'Finish reply lost'};}}
     if(args.action==='cancel')f.job!.phase='cancelling';
     return structuredClone(f.job);
    }
   }
   switch(args.method){
    case 'preview_windows_screen_upload':{
     f.calls.push({command,args:structuredClone(args)});
     if(f.holdPreview)await new Promise<void>(resolve=>{f.releasePreview=resolve;});
     if(f.rejectPreview)throw {message:'owned preview rejection'};
     const canvas=document.createElement('canvas');canvas.width=480;canvas.height=480;
     const context=canvas.getContext('2d')!;
     context.fillStyle=(args.params as {source:string}).source==='scene'?'#2468ac':'#ac6824';context.fillRect(0,0,480,480);
     return Array.from(Uint8Array.from(atob(canvas.toDataURL('image/jpeg').split(',')[1]),c=>c.charCodeAt(0)));
    }
    case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:f.language};
    case 'list_devices':return [{id:'test',display_name:'LCDV2', family:'galahad2-lcdv2'},{id:'other',display_name:'Other mock device', family:'galahad2-lcdv2'}];
    case 'list_sensors':return [{id:'cpu:test',label:'Test CPU',unit:'celsius',value:40,status:'valid',age_ms:0}];
    case 'list_prepared_clips':case 'list_theme_assets':case 'list_named_theme_assets':return [];
    case 'get_lcd_state':return {source:'Idle',requested:{brightness_percent:100,rotation_degrees:0,mode:'LocalUI'},last_error:null};
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    default:return null;
   }
  }}});
 },language);
 await page.goto('/?view=display');await expect(page.getByText(language==='en-US'?'Daemon connected':'Daemon подключён',{exact:true})).toBeVisible({timeout:15000});
 await page.getByRole('button',{name:language==='en-US'?'Settings':'Настройки',exact:true}).click();await page.getByText(language==='en-US'?'Settings group backup':'Резервная копия группы настроек',{exact:true}).click();
}
