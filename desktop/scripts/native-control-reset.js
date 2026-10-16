// Production WebKitGTK/Tauri/Unix IPC path. Only private MockBackend fixture.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<600;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const input=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('input,select');
 const phase=async p=>(await invoke('group_archive_status'))?.phase===p;
 const open=async()=>{await click('Настройки');[...document.querySelectorAll('summary')].find(s=>s.textContent.trim()==='Резервная копия группы настроек').click();};
 try {
  await wait(async()=>(await rpc('list_devices')).length===1,'mock device');const dev=(await rpc('list_devices'))[0].id;
  if(window.__nativeResetRecover){
   const pending=JSON.parse(localStorage.getItem('lconnect.group.pending'));
   if(pending?.mode!=='reset'||pending.dev!==dev)throw new Error('Reset ID was not preserved across desktop restart');
   const before=await rpc('list_profiles');
   if(await invoke('group_archive_status'))throw new Error('Unexpected implicit reset worker after restart');
   await open();await click('Прочитать результат группы');await wait(()=>phase('complete'),'read reset after desktop restart');
   const status=await invoke('group_archive_status');
   if(status.readback.selected_content_current)throw new Error('Newer manual intent unexpectedly matched old reset');
   if(JSON.stringify(before)!==JSON.stringify(await rpc('list_profiles')))throw new Error('Recovery edited slots');
   const fan=(await rpc('get_cooling_state',{dev})).fan.requested;
   if(fan.mode!=='fixed'||fan.percent!==37)throw new Error('Recovery replayed reset over newer fan setting');
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,dev,operation:pending.id,library:before,desktop_restart_readback:true,newer_intent_preserved:true}});return;
  }
  const theme={version:1,name:'Keep reset screen',background:[0,0,0],background_image:{asset:'builtin-midnight-v1',fit:'crop'},layers:[]};
  await rpc('set_lcd_theme',{dev,theme});await rpc('patch_lcd_config',{dev,patch:{brightness_percent:64,rotation_degrees:90}});
  let library=await rpc('list_profiles');await rpc('edit_profile',{expected_revision:library.revision,edit:{action:'save',profile:{slot:7,name:'Keep reset slot',content:{kind:'screen',value:theme}}}});
  await rpc('set_cooling_policy',{dev,target:'fan',policy:{mode:'fixed',percent:37}});
  await wait(()=>!!input('Название'),'editor');input('Название').value='Keep reset draft';input('Название').dispatchEvent(new Event('input',{bubbles:true}));
  const before=await rpc('list_profiles'),screen=await rpc('get_lcd_theme',{dev}),lcd=(await rpc('get_lcd_state',{dev})).requested;
  await open();input('Группа настроек').value='fan_and_lighting';input('Группа настроек').dispatchEvent(new Event('change',{bubbles:true}));
  await wait(()=>!!input('Температурный датчик CPU для сброса')?.querySelector('option[value="fixture:cpu"]'),'synthetic CPU option');
  input('Температурный датчик CPU для сброса').value='fixture:cpu';input('Температурный датчик CPU для сброса').dispatchEvent(new Event('change',{bubbles:true}));
  await click('Подготовить полный сброс охлаждения и подсветки…');await wait(()=>phase('prepared'),'read-only reset preview');
  if(JSON.stringify(before)!==JSON.stringify(await rpc('list_profiles')))throw new Error('Preview edited slots');
  if((await rpc('get_cooling_state',{dev})).fan.requested.percent!==37)throw new Error('Preview applied defaults');
  await click('Отменить подготовку сброса');await wait(()=>phase('cancelled'),'cancel reset preview');await click('Закрыть сведения о группе');
  await click('Подготовить полный сброс охлаждения и подсветки…');await wait(()=>phase('prepared'),'second reset preview');
  await click('Подтвердить полный сброс и применить');await wait(()=>phase('complete'),'confirmed reset');
  const saved=await invoke('group_archive_status'),operation=saved.id,after=await rpc('list_profiles');
  if(after.revision!==before.revision+1||saved.summary.slots.length!==8)throw new Error('Reset did not commit all control slots exactly once');
  if(JSON.stringify(await rpc('get_lcd_theme',{dev}))!==JSON.stringify(screen)||JSON.stringify((await rpc('get_lcd_state',{dev})).requested)!==JSON.stringify(lcd))throw new Error('Reset changed Screen');
  await wait(()=>button('Повторить применение сохранённой группы')&&!button('Повторить применение сохранённой группы').disabled,'reset receipt');
  await click('Повторить применение сохранённой группы');await wait(()=>phase('complete'),'reset explicit reapply');
  if(JSON.stringify(after)!==JSON.stringify(await rpc('list_profiles')))throw new Error('Reapply made another commit');
  await rpc('set_cooling_policy',{dev,target:'fan',policy:{mode:'fixed',percent:37}});
  await click('Прочитать результат группы');await wait(()=>phase('complete'),'newer intent readback');
  await wait(()=>button('Повторить применение сохранённой группы').disabled,'block stale reset reapply');
  await click('Экран помпы');if(input('Название').value!=='Keep reset draft')throw new Error('Reset overwrote editor draft');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,dev,operation,library:after,screen,lcd,explicit_reset:true,preview_no_mutation:true,preview_cancelled:true,reapply_no_commit:true,preserved_editor:true,newer_intent_preserved:true}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
