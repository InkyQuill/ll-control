// Real WebKit/Tauri commands, private file portal, production IPC, MockBackend.
(async()=>{
 const english=window.__nativeDesktopPhase==='en';
 const expectedEnglish={"Название":"Name","Настройки":"Settings","Группа настроек":"Settings group","Экспортировать группу…":"Export group…","Закрыть сведения о группе":"Close group details","Выбрать архив группы…":"Choose group archive…","Восстановить и применить группу":"Restore and apply group","Прочитать результат группы":"Read group result","Повторить применение сохранённой группы":"Reapply saved group","Экран помпы":"Pump screen","Резервная копия группы настроек":"Settings group backup","Архив группы сохранён в файл.":"Group archive saved to a file."};
 const ui=source=>english?(expectedEnglish[source]??source):source;
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<600;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===ui(name));
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const input=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(ui(label)))?.querySelector('input,select');
 const phase=async p=>(await invoke('group_archive_status'))?.phase===p;
 try {
  await wait(async()=>(await rpc('list_devices')).length===1,'mock device');const dev=(await rpc('list_devices'))[0].id;
  await click('Экран помпы');
  const group=window.__nativeGroupKind||'screen';
  async function configure(value){if(group==='screen')await rpc('patch_lcd_config',{dev,patch:{brightness_percent:value}});else await rpc('set_cooling_policy',{dev,target:'fan',policy:{mode:'fixed',percent:value}});}
  async function current(){return group==='screen'?(await rpc('get_lcd_state',{dev})).requested.brightness_percent:(await rpc('get_cooling_state',{dev})).fan.requested.percent;}
  await configure(64);
  const theme={version:1,name:'Native full group',background:[0,0,0],background_image:{asset:'builtin-midnight-v1',fit:'crop'},layers:[{kind:'clock',twenty_four_hour:true,show_seconds:true,style:{x:20,y:30,size:36,color:[255,255,255],align:'left'}}]};
  const lib=await rpc('list_profiles');await rpc('edit_profile',{expected_revision:lib.revision,edit:{action:'save',profile:{slot:7,name:'Full-group slot',content:{kind:'screen',value:theme}}}});
  await wait(()=>!!input('Название'),'editor');input('Название').value='Keep full-group draft';input('Название').dispatchEvent(new Event('input',{bubbles:true}));
  await click('Настройки');const summary=[...document.querySelectorAll('summary')].find(s=>s.textContent.trim()===ui('Резервная копия группы настроек'));summary.click();
  input('Группа настроек').value=group;input('Группа настроек').dispatchEvent(new Event('change',{bubbles:true}));
  await click('Экспортировать группу…');await wait(()=>phase('complete'),'native group export');
  await wait(()=>[...document.querySelectorAll('[role="status"]')].some(e=>e.textContent===ui('Архив группы сохранён в файл.')),'export UI result');
  await click('Закрыть сведения о группе');await configure(27);
  const before=await rpc('list_profiles');await click('Выбрать архив группы…');await wait(()=>phase('prepared'),'native inspect');
  await wait(()=>[...document.querySelectorAll('[role="status"]')].some(e=>e.textContent.includes(english?'Archive checked; waiting for Apply':'Архив проверен, ожидает применения')),'localized prepared phase');
  if((await current())!==27)throw new Error('Inspect applied settings');
  if(JSON.stringify(await rpc('list_profiles'))!==JSON.stringify(before))throw new Error('Inspect changed slots');
  await wait(()=>!!button('Восстановить и применить группу'),'preview Apply');
  await click('Восстановить и применить группу');await wait(()=>phase('complete'),'native Finish');
  const imported=await invoke('group_archive_status');if(!imported.readback?.selected_content_current)throw new Error('missing current durable receipt');
  if((await current())!==64)throw new Error('Finish did not restore LCD');
  const after=await rpc('list_profiles');const operation=imported.id;
  await click('Прочитать результат группы');await wait(()=>phase('complete'),'native readback');await wait(()=>!button('Прочитать результат группы').disabled,'joined readback');
  await click('Повторить применение сохранённой группы');await wait(()=>phase('complete'),'native reapply');await wait(()=>!button('Повторить применение сохранённой группы').disabled,'joined reapply');
  if(JSON.stringify(await rpc('list_profiles'))!==JSON.stringify(after))throw new Error('Readback/reapply created another commit');
  await click('Экран помпы');if(input('Название').value!=='Keep full-group draft')throw new Error('Full group replaced draft');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,language:english?'en-US':'ru-RU',dev,operation,library:after,receipt:imported.readback.receipt,group,explicit_apply:true,inspect_no_mutation:true,reapply_no_commit:true,preserved_editor:true}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
