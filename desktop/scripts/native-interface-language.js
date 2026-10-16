// Real language resources and saved daemon preferences; all roots are private.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const field=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('input,select');
 const set=(input,value)=>{if(!input)throw new Error('Missing language fixture field');input.value=value;input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input',{bubbles:true}));};
 try{
  const imageDevice=window.__nativeDesktopPhase==='image';
  if((await rpc('list_devices')).length!==(imageDevice?1:0))throw new Error('Unexpected fixture device count');
  const recovery=window.__nativeDesktopPhase==='2';
  const initialEnglish=!recovery,targetEnglish=imageDevice||recovery;
  const initial=await rpc('get_display_settings');if(initial.interface_language!==(initialEnglish?'en-US':'ru-RU'))throw new Error('Fresh English or restored Russian language mismatch');
  await wait(()=>!!field(initialEnglish?'Name':'Название'),'theme draft');
  const initialDraftName=field(initialEnglish?'Name':'Название').value;
  if(initialDraftName!==(initialEnglish?'My screen':'Мой экран'))throw new Error('New draft language mismatch: '+initialDraftName);
  const focusOption=document.querySelector('option[value="focus"]');
  if(focusOption?.textContent!==(initialEnglish?'Focus · One large reading and a clock':'Focus · Один крупный показатель и часы'))throw new Error('Layout resource language mismatch');
  set(field(initialEnglish?'Name':'Название'),'Настройки');
  await click(initialEnglish?'Settings':'Настройки');await wait(()=>!!field(initialEnglish?'Interface language':'Язык интерфейса'),'language select');
  set(field(initialEnglish?'Interface language':'Язык интерфейса'),targetEnglish?'en-US':'ru-RU');await click(initialEnglish?'Save display preferences':'Сохранить параметры отображения');
  await wait(()=>document.documentElement.lang===(targetEnglish?'en':'ru'),'reactive document language');
  await click(targetEnglish?'Pump screen':'Экран помпы');await wait(()=>field(targetEnglish?'Name':'Название')?.value==='Настройки','user draft must not be translated');
  let imageEditorPreview=false;
  if(imageDevice){
   const before=(await rpc('list_named_theme_assets')).map(asset=>asset.key).sort();
   await click('Import and edit…');await wait(()=>!!field('File to edit'),'English file input');
   const canvas=document.createElement('canvas');canvas.width=96;canvas.height=64;const context=canvas.getContext('2d');context.fillStyle='#d93838';context.fillRect(0,0,96,64);
   const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));if(!blob)throw new Error('Synthetic PNG fixture failed');
   const transfer=new DataTransfer();transfer.items.add(new File([blob],'Настройки',{type:'image/png'}));const input=field('File to edit');input.files=transfer.files;input.dispatchEvent(new Event('change',{bubbles:true}));
   const editor=()=>document.querySelector('section[aria-label="Image editor"]');
   await wait(()=>[...editor().querySelectorAll('p')].some(p=>p.textContent==='Настройки · 96 × 64 px'),'source filename must remain untranslated');
   await click('Rotate right');await click('Prepare image preview');
   await wait(()=>{const image=editor()?.querySelector('img[alt="Edited result, 480 by 480"]');return image?.complete&&image.naturalWidth===480;},'actual native image decoder/transform/preview');
   await click('Close image editor');await wait(()=>!editor(),'English editor close');
   const after=(await rpc('list_named_theme_assets')).map(asset=>asset.key).sort();if(JSON.stringify(before)!==JSON.stringify(after))throw new Error('Preview/close published an image');
   imageEditorPreview=true;
  }
  const settings=await rpc('get_display_settings');if((settings.interface_language??'ru-RU')!==(targetEnglish?'en-US':'ru-RU'))throw new Error('Saved language mismatch');
  if(settings.temperature_unit!=='celsius')throw new Error('Language change altered temperature unit');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,recovery,initial,settings,initialDraftName,userDraft:'Настройки',imageEditorPreview,documentLanguage:document.documentElement.lang,noDevices:!imageDevice}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
