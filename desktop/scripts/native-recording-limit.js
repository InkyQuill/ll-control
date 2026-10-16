// Embedded only in the isolated native-smoke build; uses the production budget.
(async()=>{
  const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
  const rpc=(method,params={})=>invoke('daemon_call',{method,params});
  const assert=(ok,why)=>{if(!ok)throw new Error(why);};
  const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
  const wait=async(predicate,why,timeout=20000)=>{
    const deadline=performance.now()+timeout;
    while(performance.now()<deadline){if(await predicate())return;await delay(100);}
    throw new Error(why);
  };
  const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
  try{
    await wait(async()=>(await rpc('list_devices')).length>0,'mock device');
    const dev=(await rpc('list_devices'))[0].id;
    await rpc('migrate_screen_collections',{dev});
    const before=(await rpc('list_prepared_clips')).map(c=>c.key);
    await wait(()=>[...document.querySelectorAll('summary')].find(e=>e.textContent==='Библиотека роликов'),'library');
    [...document.querySelectorAll('summary')].find(e=>e.textContent==='Библиотека роликов').click();
    await click('Записать экран…');await click('Выбрать экран или окно');
    await wait(()=>document.querySelector('button[aria-label="Выбор области захвата на кадре"]'),'setup preview');
    const started=performance.now();
    await click('Начать запись экрана');
    await wait(async()=>{
      const status=await invoke('screen_record_status');
      assert(!['failed','cancelled'].includes(status?.phase),'recording failed before limit');
      return status?.phase==='limit_reached';
    },'production recording limit',200000);
    const limited=await invoke('screen_record_status');
    assert(performance.now()-started>=179000,'limit arrived too early');
    assert(limited.recordedMillis===180000&&limited.frames===4320,'production limit counters');
    await wait(()=>document.querySelector('dialog[open] h3')?.textContent==='Достигнут лимит записи','automatic limit dialog');
    await click('Вернуться к записи');
    await wait(()=>!document.querySelector('dialog[open]'),'cancel limit dialog');
    assert(document.activeElement===button('Завершить и просмотреть запись'),'cancel restores Finish focus');
    await delay(1000);
    const retained=await invoke('screen_record_status');
    assert(retained.phase==='limit_reached'&&retained.frames===limited.frames&&retained.recordedMillis===limited.recordedMillis,'cancel retains stable limit');
    assert(!document.querySelector('dialog[open]'),'poll must not reopen cancelled limit dialog');
    let rejected=false;
    try{await invoke('screen_record_control',{id:limited.id,action:'resume'});}catch{rejected=true;}
    assert(rejected,'resume at limit must reject');
    assert(!button('Продолжить запись'),'UI must not offer resume at limit');
    await click('Завершить и просмотреть запись');await click('Подготовить запись целиком');
    await wait(()=>document.querySelector('video[aria-label="Готовый ролик"]')?.readyState>=2,'full recording review decode',60000);
    const video=document.querySelector('video[aria-label="Готовый ролик"]');
    const reviewDuration=video.duration;
    assert(Math.abs(reviewDuration-180)<.05,'review duration');
    video.currentTime=179.5;
    await wait(()=>!video.seeking&&Math.abs(video.currentTime-179.5)<.05&&video.readyState>=2,'seek near final frame');
    assert((await rpc('list_prepared_clips')).every(c=>before.includes(c.key)),'no publication before Save');
    await click('Сохранить запись в библиотеку');
    await wait(async()=>(await invoke('media_status'))?.phase==='complete','save full recording',30000);
    const saved=(await invoke('media_status')).key;
    await wait(async()=>{const s=await rpc('get_screen_collections',{dev});return s.collections?.backgrounds.some(a=>a.kind==='video'&&a.key===saved);},'membership');
    await click('Закрыть запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.phase==='cancelled','release capture lease');
    await invoke('native_smoke_result',{result:{ok:true,full_limit:true,frames:limited.frames,recorded_millis:limited.recordedMillis,automatic_dialog:true,cancel_retains_limit:true,resume_rejected:true,review_duration:reviewDuration,final_seek:true,explicit_save:true,collection_membership:true,saved_key:saved},keepOpen:true});
  }catch(error){await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack,alerts:[...document.querySelectorAll('[role="alert"]')].map(e=>e.textContent)}});}
})();
