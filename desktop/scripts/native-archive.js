// Private Xvfb/D-Bus/MockBackend workflow. File chooser is a private portal fixture.
(async()=>{
 const english=window.__nativeDesktopPhase==='en';
 const expectedEnglish={"Экран помпы":"Pump screen","Название":"Name","Обновить список профилей":"Refresh profile list","Экспортировать слот в архив…":"Export slot to archive…","Слот профиля":"Profile slot","Импортировать архив в слот…":"Import archive into slot…","Прочитать результат архива":"Read archive result","Сохранённые профили":"Saved profiles","Импорт и экспорт профиля":"Profile import and export","Архив сохранён в файл.":"Archive saved to file."};
 const ui=source=>english?(expectedEnglish[source]??source):source;
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<600;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===ui(name));
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const field=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(ui(label)))?.querySelector('input,select');
 const select=async(label,value)=>{await wait(()=>field(label)&&!field(label).matches(':disabled'),label);field(label).value=value;field(label).dispatchEvent(new Event('change',{bubbles:true}));};
 try {
  await wait(async()=>(await rpc('list_devices')).length===1,'mock device');const dev=(await rpc('list_devices'))[0].id;
  await click('Экран помпы');
  const before=await rpc('get_lcd_state',{dev});
  const bytes=window.__nativeFontBytes;const upload=await rpc('font_upload',{request:{action:'begin',size:bytes.length}});
  for(let offset=0;offset<bytes.length;offset+=65536)await rpc('font_upload',{request:{action:'chunk',id:upload.id,offset,bytes:bytes.slice(offset,offset+65536)}});
  const font=(await rpc('font_upload',{request:{action:'finish',id:upload.id}})).asset;
  const theme={version:1,name:'Archive source theme',background:[0,0,0],background_image:{asset:'builtin-midnight-v1',fit:'crop'},layers:[{kind:'text',text:'Native archive text',style:{font_asset:font,x:30,y:50,size:32,color:[255,255,255],align:'left'}}],page_cache:[{page:'rain',theme:{version:3,name:'Cached Rain',background:[0,0,0],background_video:'bcf5a102bf2a426286d34310cad4a001',layers:[{kind:'weather',effect:{kind:'rain',intensity:1,dynamic:false,sensor:null,seed:42}}]}}]};
  const initial=await rpc('list_profiles');await rpc('edit_profile',{expected_revision:initial.revision,edit:{action:'save',profile:{slot:0,name:'Archive source',content:{kind:'screen',value:theme}}}});
  await wait(()=>!!field('Название'),'theme editor');field('Название').value='Keep current editor';field('Название').dispatchEvent(new Event('input',{bubbles:true}));
  const summary=[...document.querySelectorAll('summary')].find(e=>e.textContent.trim()===ui('Сохранённые профили'));if(!summary.parentElement.open)summary.click();await click('Обновить список профилей');
  await wait(()=>[...document.querySelectorAll('summary')].some(e=>e.textContent.trim()===ui('Импорт и экспорт профиля')),'archive panel');[...document.querySelectorAll('summary')].find(e=>e.textContent.trim()===ui('Импорт и экспорт профиля')).click();
  await click('Экспортировать слот в архив…');await wait(async()=>{const s=await invoke('profile_archive_status');return s?.mode==='export'&&s.phase==='complete';},'native file export');
  await wait(()=>[...document.querySelectorAll('[role="status"]')].some(e=>e.textContent.includes(ui('Архив сохранён в файл.'))),'export UI result');
  await click('Экспортировать слот в архив…');
  await wait(async()=>{const s=await invoke('profile_archive_status');return s?.mode==='export'&&s.phase==='failed';},'existing destination is rejected');
  const rejected=await invoke('profile_archive_status');
  if(rejected.message_code!=='destination_exists'||rejected.message!=='The file already exists. Choose a new name.')throw new Error('typed destination error was lost');
  await wait(()=>[...document.querySelectorAll('[role="alert"]')].some(e=>e.textContent===(english?'The file already exists. Choose a new name.':'Файл уже существует: выберите новое имя')),'localized actual worker error');
  await select('Слот профиля','7');await click('Импортировать архив в слот…');
  await wait(async()=>{const s=await invoke('profile_archive_status');return s?.mode==='import'&&s.slot===7&&s.phase==='complete';},'native file import');
  await wait(()=>field('Слот профиля')?.querySelector('option[value="7"]')?.textContent.includes('Archive source'),'imported slot readback');
  if(field('Название').value!=='Keep current editor')throw new Error('archive import replaced current editor draft');
  await wait(()=>[...document.querySelectorAll('[role="status"]')].some(e=>e.textContent===(english?'Import confirmed: slot 8. Device settings have not been applied.':'Импорт подтверждён: слот 8. Настройки устройства не применены.')),'localized import receipt');
  const saved=await rpc('list_profiles');if(saved.revision!==2||saved.profiles.length!==2||saved.archive_imports?.length!==1)throw new Error('unexpected archive library commits');
  const imported=saved.profiles.find(p=>p.slot===7).content.value;
  if(imported.background_image.asset==='builtin-midnight-v1'||imported.layers[0].style.font_asset===font||imported.page_cache[0].theme.background_video==='bcf5a102bf2a426286d34310cad4a001')throw new Error('dependencies were not remapped');
  const completed=await invoke('profile_archive_status');if(completed.message_code)throw new Error('new successful job retained old notice code');
  const operation=completed.id;
  await click('Прочитать результат архива');await wait(()=>!button('Прочитать результат архива').disabled,'readback joined');
  await wait(async()=>{const s=await invoke('profile_archive_status');return s?.id===operation&&s.phase==='complete';},'durable result readback');
  const after=await rpc('get_lcd_state',{dev});if(after.source!==before.source||JSON.stringify(after.requested)!==JSON.stringify(before.requested))throw new Error('archive implicitly applied LCD state');
  if((await rpc('list_profiles')).revision!==2)throw new Error('readback created another import');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,language:english?'en-US':'ru-RU',dev,native_file_chooser:true,native_archive_export:true,typed_destination_error:rejected.message_code,native_error_language:english?'en-US':'ru-RU',cleared_notice_on_new_job:true,native_archive_import:true,native_archive_readback:true,no_implicit_apply:true,preserved_editor:true,operation,library:saved,before,after}});
 } catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
