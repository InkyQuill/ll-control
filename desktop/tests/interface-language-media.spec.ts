import {test,expect,type Page} from '@playwright/test';
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1sAAAAASUVORK5CYII=';
async function setup(page:Page){
 await page.addInitScript(({png})=>{
  localStorage.setItem('lconnect.interface-language','en-US');
  const bytes=Array.from(atob(png),c=>c.charCodeAt(0));
  const assets=[{key:'a'.repeat(32),name:'Фон',revision:1,is_default:false}];
  const f={calls:[] as {command:string;args:Record<string,unknown>}[],assets,job:null as null|{id:string;name:string;total:number;received:number;phase:string;key:null;message:null}};
  Object.assign(window,{englishMedia:f,__TAURI_INTERNALS__:{invoke:async(command:string,args:Record<string,unknown>={})=>{
   f.calls.push({command,args:structuredClone(args)});
   if(command==='group_archive_status')return null;
   if(command==='begin_screenshot')return '1'.repeat(32);
   if(command==='capture_screenshot')return Uint8Array.from(bytes).buffer;
   if(command==='cancel_screenshot')return null;
   if(command==='image_editor')return {width:960,height:480,png:bytes};
   if(command==='import_image'){assets.push({key:'b'.repeat(32),name:args.name as string,revision:1,is_default:false});return 'b'.repeat(32);}
   if(command==='media_begin'){f.job={id:args.id as string,name:args.name as string,total:args.total as number,received:0,phase:'uploading',key:null,message:null};return {...f.job};}
   if(command==='media_append'){if(f.job)f.job.received+=(args.bytes as number[]).length;return {...f.job};}
   if(command==='media_start'){if(f.job)f.job.phase='review_ready';return null;}
   if(command==='media_status')return f.job?{...f.job}:null;
   if(command==='media_playback_url')return 'data:video/mp4;base64,AA==';
   if(command==='media_cancel'){if(f.job)f.job.phase='cancelled';return f.job?{...f.job}:null;}
   switch(args.method){
    case 'get_display_settings':return {version:1,revision:0,temperature_unit:'celsius',interface_language:'en-US'};
    case 'list_devices':return [{id:'test',display_name:'LCDV2'}];case 'list_sensors':case 'list_prepared_clips':case 'list_theme_assets':return [];
    case 'get_lcd_state':return {source:'Idle',requested:{mode:'LocalUI',brightness_percent:100,rotation_degrees:0},last_error:null};
    case 'get_screen_collections':return {collections:null,persistence:{pending:false,saved:true,last_error:null}};
    case 'list_profiles':return {version:1,revision:0,profiles:[]};
    case 'list_named_theme_assets':return structuredClone(assets);
    case 'thumbnail_theme_asset':return bytes;
    case 'rename_theme_asset':{const p=args.params as {key:string;name:string;expected_revision:number};const asset=assets.find(a=>a.key===p.key)!;if(asset.revision!==p.expected_revision)throw {message:'stale revision'};asset.name=p.name;asset.revision++;return null;}
    default:return null;
   }
  }}});
 },{png});
 await page.goto('/');await expect(page.getByText('Daemon connected',{exact:true})).toBeVisible({timeout:15000});
}
test('English image import previews crop/filters, saves the original filename once and leaves LCD Apply explicit',async({page})=>{
 await setup(page);await page.getByRole('button',{name:'Import and edit…',exact:true}).click();
 await page.getByLabel('File to edit',{exact:true}).setInputFiles({name:'Настройки',mimeType:'image/png',buffer:Buffer.from(png,'base64')});
 const editor=page.getByRole('region',{name:'Image editor',exact:true});await expect(editor.getByText('Настройки · 960 × 480 px',{exact:true})).toBeVisible();
 await page.getByLabel('Crop X',{exact:true}).fill('480');await editor.getByRole('spinbutton',{name:'Image contrast',exact:true}).fill('12');await editor.getByRole('button',{name:'Rotate right',exact:true}).click();
 await editor.getByRole('button',{name:'Prepare image preview',exact:true}).click();await expect(editor.getByAltText('Edited result, 480 by 480',{exact:true})).toBeVisible();
 await editor.getByRole('button',{name:'Save image to gallery',exact:true}).click();await expect(editor.getByRole('status')).toContainText('Result saved and added to the draft');
 const calls=await page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string;args:Record<string,unknown>}[]}}).englishMedia.calls);
 expect(calls.filter(c=>c.command==='import_image')).toHaveLength(1);expect(calls.find(c=>c.command==='import_image')?.args.name).toBe('Настройки');
 expect(calls.filter(c=>c.command==='image_editor').at(-1)?.args.edit).toEqual({crop:{x:480,y:0,side:480},transforms:['rotate_right'],filters:{hue:0,saturation:0,brightness:0,contrast:12,sharpen:0,blur:0}});
 expect(calls.some(c=>c.args.method==='set_lcd_theme')).toBe(false);
});
test('English gallery rename preserves resource-like user names and cancelling deletion never deletes the file',async({page})=>{
 await setup(page);await page.getByText('Image library',{exact:true}).click();await page.getByRole('button',{name:'Rename Фон',exact:true}).click();await page.getByLabel('Image name',{exact:true}).fill('Редактор изображения');await page.getByRole('button',{name:'Save image name',exact:true}).click();
 await expect(page.getByRole('button',{name:'Rename Редактор изображения',exact:true})).toBeVisible();await page.getByRole('button',{name:'Delete Редактор изображения…',exact:true}).click();await page.getByRole('button',{name:'Cancel image deletion',exact:true}).click();await expect(page.getByRole('button',{name:'Confirm image deletion',exact:true})).toHaveCount(0);
 const calls=await page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string;args:Record<string,unknown>}[]}}).englishMedia.calls);
 expect(calls.find(c=>c.args.method==='rename_theme_asset')?.args.params).toEqual({key:'a'.repeat(32),name:'Редактор изображения',expected_revision:1});expect(calls.some(c=>c.args.method==='delete_theme_asset')).toBe(false);
});
test('English video review and cancellation keep source filename and fit settings and never confirm publication',async({page})=>{
 await setup(page);await page.getByText('Video library',{exact:true}).click();await page.getByRole('checkbox',{name:'Video color correction',exact:true}).check();
 await page.getByLabel('Import GIF/MP4',{exact:true}).setInputFiles({name:'Импорт ролика.mp4',mimeType:'video/mp4',buffer:Buffer.from([1,2,3])});
 await page.getByRole('button',{name:'Prepare video',exact:true}).click();
 await expect(page.getByRole('status').filter({hasText:'Video ready for review: Импорт ролика.mp4'})).toBeVisible();await expect(page.getByRole('button',{name:'Save prepared video',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Cancel import',exact:true}).click();await expect(page.getByRole('status').filter({hasText:'Import cancelled: Импорт ролика.mp4'})).toBeVisible();
 const calls=await page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string;args:Record<string,unknown>}[]}}).englishMedia.calls);
 expect(calls.find(c=>c.command==='media_begin')?.args).toMatchObject({name:'Импорт ролика.mp4',fit:'contain',adjustments:{contrast:0,brightness:0}});expect(calls.find(c=>c.command==='media_start')?.args.review).toBe(true);expect(calls.some(c=>c.command==='media_confirm')).toBe(false);
});

test('English screenshot gets a localized filename and publishes it only on explicit Save',async({page})=>{
 await setup(page);await page.getByRole('button',{name:'Import and edit…',exact:true}).click();
 const panel=page.getByRole('region',{name:'Image editor',exact:true});
 await panel.getByRole('button',{name:'Take screenshot…',exact:true}).click();
 await expect(panel.getByText('Screenshot.png · 960 × 480 px',{exact:true})).toBeVisible();
 await panel.getByRole('button',{name:'Prepare image preview',exact:true}).click();
 await expect(panel.getByAltText('Edited result, 480 by 480')).toBeVisible();
 expect(await page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string}[]}}).englishMedia.calls.some(c=>c.command==='import_image'))).toBe(false);
 await panel.getByRole('button',{name:'Save image to gallery',exact:true}).click();
 await expect.poll(()=>page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string}[]}}).englishMedia.calls.filter(c=>c.command==='import_image').length)).toBe(1);
 const calls=await page.evaluate(()=>(window as unknown as {englishMedia:{calls:{command:string;args:Record<string,unknown>}[]}}).englishMedia.calls);
 expect(calls.filter(c=>c.command==='import_image').map(c=>c.args.name)).toEqual(['Screenshot.png']);
 expect(calls.some(c=>c.args.method==='set_lcd_theme')).toBe(false);
});
