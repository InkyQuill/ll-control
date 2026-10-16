(async()=>{
  const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
  const rpc=(method,params={})=>invoke('daemon_call',{method,params});
  const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
  const wait=async(predicate,why)=>{for(let i=0;i<400;i++){if(await predicate())return;await new Promise(r=>setTimeout(r,50));}throw new Error(why);};
  const assert=(ok,why)=>{if(!ok)throw new Error(why);};
  const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
  const fill=async(label,value)=>{
    const input=()=>[...document.querySelectorAll('label')].find(e=>e.textContent.trim()===label)?.querySelector('input');
    await wait(()=>input()&&!input().disabled,label);input().value=String(value);input().dispatchEvent(new Event('input',{bubbles:true}));
  };
  try {
    await wait(async()=>(await rpc('list_devices')).length>0,'mock device');
    const dev=(await rpc('list_devices'))[0].id;
    await rpc('migrate_screen_collections',{dev});
    await wait(()=>[...document.querySelectorAll('summary')].find(e=>e.textContent==='Библиотека роликов'),'video library');
    [...document.querySelectorAll('summary')].find(e=>e.textContent==='Библиотека роликов').click();
    await click('Записать экран…');await click('Выбрать экран или окно');
    await wait(async()=>(await invoke('screen_record_status'))?.phase==='setup','initial setup');
    await wait(()=>document.querySelector('button[aria-label="Выбор области захвата на кадре"]'),'initial source preview');
    await fill('Захват X, %',90);await fill('Захват Y, %',20);await fill('Ширина захвата, %',6);await fill('Высота захвата, %',5);
    await click('Применить область захвата');
    await wait(async()=>(await invoke('screen_record_status')).region?.x===.9,'initial region readback');
    await new Promise(resolve=>setTimeout(resolve,500));
    const setupState=await invoke('screen_record_status');
    if(setupState.frames!==0 || setupState.recordedMillis!==0)throw new Error('Setup recorded frames or time');
    await click('Начать запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.frames>=30,'synthetic recording frames');
    await click('Приостановить запись');await wait(async()=>(await invoke('screen_record_status'))?.phase==='paused','pause');
    await wait(()=>document.querySelector('button[aria-label="Выбор области захвата на кадре"]'),'source preview control');
    await fill('Захват X, %',75);await fill('Захват Y, %',20);await fill('Ширина захвата, %',6);await fill('Высота захвата, %',5);
    const selector=document.querySelector('button[aria-label="Выбор области захвата на кадре"]');
    selector.focus();selector.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
    await wait(()=>[...document.querySelectorAll('label')].find(e=>e.textContent.trim()==='Захват X, %').querySelector('input').valueAsNumber===76,'keyboard region move');
    selector.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true}));
    await wait(()=>{
      const canvas=document.querySelector('canvas[aria-label="Предпросмотр области захвата"]');
      if(!canvas)return false;const pixel=canvas.getContext('2d').getImageData(canvas.width/2,canvas.height/2,1,1).data;
      return pixel[0]>200&&pixel[1]<35&&pixel[2]<35;
    },'red region visual preview');
    await click('Применить область захвата');
    await wait(async()=>(await invoke('screen_record_status')).region?.x===.75,'red region readback');
    const pausedFrames=(await invoke('screen_record_status')).frames;
    await click('Продолжить запись');await wait(async()=>(await invoke('screen_record_status')).frames>=pausedFrames+6,'frames after resume');
    await click('Приостановить запись');await wait(async()=>(await invoke('screen_record_status')).phase==='paused','second pause');
    const snapshotStatus=await invoke('screen_record_status');
    const snapshotBytes=await invoke('screen_record_snapshot',{id:snapshotStatus.id});
    const snapshotUrl=URL.createObjectURL(new Blob([new Uint8Array(snapshotBytes)],{type:'image/jpeg'}));
    try {
      const picture=new Image();picture.src=snapshotUrl;await picture.decode();
      assert(picture.naturalWidth===320&&picture.naturalHeight===240,'full-source snapshot dimensions');
      const canvas=document.createElement('canvas');canvas.width=320;canvas.height=240;
      const context=canvas.getContext('2d');context.drawImage(picture,0,0);
      const blue=context.getImageData(288,48,1,1).data;
      assert(blue[2]>200&&blue[0]<35&&blue[1]<35,'snapshot retains blue area outside active red crop');
      assert((await invoke('screen_record_status')).frames===snapshotStatus.frames,'snapshot does not resume capture');
    }finally{URL.revokeObjectURL(snapshotUrl);}
    await fill('Захват X, %',90);await click('Применить область захвата');
    await wait(async()=>(await invoke('screen_record_status')).region?.x===.9,'blue region readback');
    const secondPause=(await invoke('screen_record_status')).frames;
    await click('Продолжить запись');await wait(async()=>(await invoke('screen_record_status')).frames>=secondPause+6,'frames after second region');
    await click('Завершить и просмотреть запись');await click('Редактировать запись');
    await wait(()=>document.querySelector('video[aria-label="Исходное видео"]')?.readyState>=2,'native source decode');
    const source=document.querySelector('video[aria-label="Исходное видео"]');
    const dimensions=[source.videoWidth,source.videoHeight];source.currentTime=.5;
    await wait(()=>!source.seeking&&Math.abs(source.currentTime-.5)<.05,'source seek');
    const before=(await rpc('list_prepared_clips')).map(c=>c.key);
    // One-shot transport failures exercise production lease cleanup and UI recovery.
    const originalFetch=window.fetch;
    try {
      for(const command of ['screen_record_admit','screen_record_prepare']) {
        let rejected=false;
        window.fetch=(input,options)=>{
          const url=new URL(input instanceof Request?input.url:String(input),location.href);
          if(url.pathname.endsWith('/'+command)&&!rejected){
            rejected=true;
            return Promise.resolve(new Response(JSON.stringify('injected '+command+' rejection'),{headers:{'Content-Type':'application/json','Tauri-Response':'error'}}));
          }
          return originalFetch(input,options);
        };
        await click('Подготовить запись для LCD');
        await wait(()=>[...document.querySelectorAll('[role="alert"]')].some(e=>e.textContent.includes('injected '+command)),command+' surfaced');
        await click('Вернуться к редактированию записи');
        await wait(()=>button('Подготовить запись для LCD')&&!button('Подготовить запись для LCD').disabled,'edit restored');
        assert((await invoke('screen_record_status')).phase==='ready','failed review retains capture');
        assert((await rpc('list_prepared_clips')).every(c=>before.includes(c.key)),'failed review never published');
        if(command==='screen_record_prepare')assert((await invoke('media_status')).phase==='cancelled','admitted review cancelled before editing');
      }
    }finally{window.fetch=originalFetch;}
    await click('Подготовить запись для LCD');
    await wait(()=>document.querySelector('video[aria-label="Готовый ролик"]')?.readyState>=2,'native prepared decode');
    assert((await rpc('list_prepared_clips')).every(c=>before.includes(c.key)),'no publication before Save');
    const firstReview=(await invoke('media_status')).id;
    await click('Вернуться к редактированию записи');
    await wait(()=>document.querySelector('video[aria-label="Исходное видео"]')?.readyState>=2,'source retained after review cancellation');
    assert((await invoke('screen_record_status')).phase==='ready','capture retained');
    await click('Подготовить запись для LCD');
    await wait(()=>document.querySelector('video[aria-label="Готовый ролик"]')?.readyState>=2,'second review decode');
    assert((await invoke('media_status')).id!==firstReview,'new review ID');
    assert((await rpc('list_prepared_clips')).every(c=>before.includes(c.key)),'cancelled review never published');
    await click('Сохранить запись в библиотеку');
    await wait(async()=>(await invoke('media_status'))?.phase==='complete','native publication');
    const saved=(await invoke('media_status')).key;
    await wait(async()=>{const s=await rpc('get_screen_collections',{dev});return !!s.collections&&s.collections.backgrounds.some(a=>a.kind==='video'&&a.key===saved);},'collection membership');
    await click('Закрыть запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.phase==='cancelled','capture lease cleanup');
    await click('Записать экран…');await click('Выбрать экран или окно');await click('Начать запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.frames>=12,'whole import recording');
    const beforeWhole=(await rpc('list_prepared_clips')).map(c=>c.key);
    await click('Завершить и просмотреть запись');await click('Подготовить запись целиком');
    await wait(()=>document.querySelector('video[aria-label="Готовый ролик"]')?.readyState>=2,'whole recording prepared decode');
    assert((await rpc('list_prepared_clips')).every(c=>beforeWhole.includes(c.key)),'whole import awaits explicit Save');
    await click('Закрыть запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.phase==='cancelled','whole import cleanup');
    await click('Записать экран…');await click('Выбрать экран или окно');await click('Начать запись экрана');
    await wait(async()=>(await invoke('screen_record_status'))?.phase==='recording','third capture for window close');
    await invoke('native_smoke_result',{result:{ok:true,source_dimensions:dimensions,source_seek:true,prepared_decode:true,explicit_save:true,review_reedit:true,admission_failure_recovery:true,prepare_failure_recovery:true,operation_choice:true,capture_regions:true,source_snapshot:true,visual_region_preview:true,initial_region_setup:true,resumed_frames:true,collection_membership:true,saved_key:saved,close_while_recording:true},keepOpen:true});
  }catch(error){await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack,alerts:[...document.querySelectorAll('[role="alert"]')].map(e=>e.textContent)}});}
})();
