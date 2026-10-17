// Private native FileChooser -> Screen bindings -> explicit group Apply/recovery.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args),rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<600;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const assert=(ok,why)=>{if(!ok)throw new Error(why);};
 const text=(en,ru)=>window.__nativeDesktopLanguage==='en-US'?en:ru;
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name&&!b.closest('[hidden]'));
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const field=(label,root=document)=>[...root.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label)&&!l.closest('[hidden]'))?.querySelector('input,select');
 const set=(input,value)=>{assert(input,'native field');input.value=value;input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input',{bubbles:true}));};
 const status=()=>invoke('group_archive_status');
 const phase=async name=>(await status())?.phase===name;
 const open=async()=>{await click(text('Settings','Настройки'));const s=[...document.querySelectorAll('summary')].find(s=>s.textContent.trim()===text('Settings group backup','Резервная копия группы настроек'));if(!s.parentElement.open)s.click();};
 try{
  await wait(async()=>(await rpc('list_devices')).length===1,'mock device');const dev=(await rpc('list_devices'))[0].id;
  if(window.__nativeWindowsRecover){
   const pending=JSON.parse(localStorage.getItem('lconnect.group.pending'));assert(pending?.mode==='windows_screen','Screen operation persisted');assert(await status()===null,'no reconstructed source grant');
   const library=await rpc('list_profiles');await open();await click(text('Read group result','Прочитать результат группы'));await wait(()=>phase('complete'),'cold Screen receipt');
   const saved=await status(),editor=await rpc('get_imported_lcd_editor',{dev}),active=await rpc('get_lcd_theme',{dev});
   assert(saved.readback.operation_id===pending.id,'original receipt');assert(editor?.basic_editor&&active&&!active.basic_editor,'independent editor and runtime after restart');assert(JSON.stringify(library)===JSON.stringify(await rpc('list_profiles')),'readback does not import');
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,dev,operation:pending.id,library,editor,active,cold_readback:true}});return;
  }
  const initial=await rpc('list_profiles');await rpc('edit_profile',{expected_revision:initial.revision,edit:{action:'save',profile:{slot:3,name:'Keep native Screen slot',content:{kind:'screen',value:{version:1,name:'Kept native',background:[4,5,6],layers:[]}}}}});
  const library=await rpc('list_profiles'),oldActive=await rpc('get_lcd_theme',{dev}),oldClips=await rpc('list_prepared_clips'),oldAssets=await rpc('list_named_theme_assets');
  await click(text('Pump screen','Экран помпы'));set(field(text('Name','Название')),'Keep Screen local draft');
  await open();const choose=text('Choose Windows Screen ZIP…','Выбрать Windows ZIP экрана…');
  await click(choose);await wait(()=>phase('bindings'),'Screen source selected');await click(text('Cancel group transfer','Отменить передачу группы'));await wait(()=>phase('cancelled'),'source cancellation');await click(text('Close group details','Закрыть сведения о группе'));
  await click(choose);await wait(()=>phase('bindings'),'fresh Screen source');
  await wait(()=>!!document.querySelector('.screen-bindings'),'Screen binding form rendered');const area=document.querySelector('.screen-bindings');set(field(text('Windows source controller','Исходный контроллер Windows'),area),'0');await click(text('Read screen requirements','Прочитать требования экрана'));
  await wait(async()=>!!(await status())?.windows_screen_requirements,'requirements metadata');const requirements=(await status()).windows_screen_requirements;
  const videoPayload=requirements.payloads.find(p=>p.kind==='video'),videoSource=!!videoPayload;
  assert(requirements.bindings.sensor_roles.length===6&&requirements.deferred.length===0,'complete supported source requirements');assert(!JSON.stringify(await status()).includes('ScreenProfiles'),'private Windows history is not exposed');
  await wait(()=>area.querySelectorAll('.fields input').length===6,'requirements form rendered');
  set(field(text('Imported theme name','Название импортированной темы'),area),'Native imported Screen');
  const inputs=[...area.querySelectorAll('.fields input')];assert(inputs.length===6,'six source roles');inputs.forEach((input,i)=>set(input,requirements.bindings.sensor_roles[i]==='CPUTemperature'?'fixture:cpu':`explicit:${requirements.bindings.sensor_roles[i]}`));
  for(const label of area.querySelectorAll('label'))if(label.textContent.trim().startsWith(text('Font for ','Шрифт для ')))set(label.querySelector('select'),'sans');
  for(const select of area.querySelectorAll('select'))if([...select.options].some(o=>o.value==='archive:wmsq/art.png'))set(select,'archive:wmsq/art.png');
  if(videoPayload)for(const select of area.querySelectorAll('select'))if([...select.options].some(o=>o.value===`archive:${videoPayload.entry}`))set(select,`archive:${videoPayload.entry}`);
  await click(text('Prepare screen import','Подготовить импорт экрана'));
  if(videoPayload?.entry==='wmsq/corrupt.mp4'){
   await wait(()=>phase('failed'),'corrupt source rejected');
   await wait(()=>[...document.querySelectorAll('[role=alert]')].some(a=>!a.closest('[hidden]')&&a.textContent.trim()),'visible source rejection');
   assert(!button(text('Import screen and apply','Импортировать экран и применить')),'corrupt source has no Apply');
   assert(JSON.stringify(library)===JSON.stringify(await rpc('list_profiles')),'rejection preserves library');
   assert(JSON.stringify(oldActive)===JSON.stringify(await rpc('get_lcd_theme',{dev})),'rejection preserves active LCD');
   assert(JSON.stringify(oldClips)===JSON.stringify(await rpc('list_prepared_clips')),'rejection publishes no media');
   assert(JSON.stringify(oldAssets)===JSON.stringify(await rpc('list_named_theme_assets')),'rejection publishes no images');
   const rejected=await status();
   await click(text('Close group details','Закрыть сведения о группе'));await click(text('Pump screen','Экран помпы'));
   assert(field(text('Name','Название')).value==='Keep Screen local draft','rejection preserves draft');
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,dev,operation:rejected.id,source_rejected:true,library,active:oldActive,draft_preserved:true,no_publication:true,message_code:rejected.message_code}});return;
  }
  await wait(()=>phase('prepared'),'owned Screen preflight');
  const prepared=await status();assert(prepared.windows_screen_options&&prepared.summary.theme_name==='Native imported Screen','prepared immutable options visible');assert(JSON.stringify(library)===JSON.stringify(await rpc('list_profiles')),'no profile commit before Apply');assert(JSON.stringify(oldActive)===JSON.stringify(await rpc('get_lcd_theme',{dev})),'preview preserves active LCD');
  for(const source of ['scene','editor']){
   await wait(()=>!!document.querySelector('.group-preview select'),'prepared preview form');
   set(document.querySelector('.group-preview select'),source);
   await wait(()=>!document.querySelector('.group-preview img'),'previous private preview cleared');
   await click(text('Show prepared frame','Показать подготовленный кадр'));
   await wait(()=>{const image=document.querySelector('.group-preview img');return image?.complete&&image.naturalWidth===480&&image.naturalHeight===480;},`decoded private ${source} preview`);
   assert(document.querySelector('.group-preview .lcd').dataset.lcdShape==='square','private square preview');
   assert(JSON.stringify(library)===JSON.stringify(await rpc('list_profiles')),'private preview preserves library');
   assert(JSON.stringify(oldClips)===JSON.stringify(await rpc('list_prepared_clips')),'private preview does not publish media');
   assert(JSON.stringify(oldActive)===JSON.stringify(await rpc('get_lcd_theme',{dev})),'private preview preserves active LCD');
  }
  await click(text('Import screen and apply','Импортировать экран и применить'));await wait(()=>phase('complete'),'explicit native Screen Apply');const committed=await status();assert(committed.readback?.selected_content_current&&!committed.readback.runtime.last_error,'Screen commit and runtime');
  const after=await rpc('list_profiles');assert(after.revision===library.revision+1&&JSON.stringify(after.profiles)===JSON.stringify(library.profiles),'one transaction preserves native library');
  const active=await rpc('get_lcd_theme',{dev}),editor=await rpc('get_imported_lcd_editor',{dev});assert((videoSource?active?.background_video:active?.background_image)&&!active.basic_editor&&editor?.basic_editor,'runtime/editor independent');
  if(videoSource)assert((await rpc('list_prepared_clips')).some(clip=>clip.key===active.background_video),'video source published after explicit Apply');
  if(videoSource)assert(active.layers.some(layer=>layer.kind==='sensor_cycle'&&layer.sensors.some(sensor=>sensor.sensor_id==='fixture:cpu')),'imported video retains live CPU sensor overlay');
  await click(text('Pump screen','Экран помпы'));assert(field(text('Name','Название')).value==='Keep Screen local draft','import preserved local draft');
  await click(text('Load imported editor','Загрузить импортированный редактор'));await wait(()=>field(text('Name','Название')).value==='Native imported Screen','explicit editor draft load');
  await click(text('Refresh preview','Обновить preview'));await wait(()=>{const image=document.querySelector('.lcd img');return image?.complete&&image.naturalWidth===480&&image.naturalHeight===480;},'decoded 480x480 editor sensor preview');assert(document.querySelector('.lcd').dataset.lcdShape==='square','square LCDV2 preview');assert(JSON.stringify(active)===JSON.stringify(await rpc('get_lcd_theme',{dev})),'editor load and preview do not Apply');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,dev,operation:committed.id,library:after,active,editor,source_cancel:true,requirements:true,prepared_no_apply:true,private_scene_preview:true,private_editor_preview:true,video_source:videoSource,explicit_import:true,editor_load_no_apply:true,square_preview:true}});
 }catch(error){await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack,preview:document.querySelector('.group-preview')?.textContent}});}
})();
