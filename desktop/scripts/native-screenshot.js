(async()=>{
  const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
  const rpc=(method,params={})=>invoke('daemon_call',{method,params});
  const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
  const wait=async(predicate,why)=>{for(let i=0;i<300;i++){if(await predicate())return;await new Promise(r=>setTimeout(r,50));}throw new Error(why);};
  const assert=(ok,why)=>{if(!ok)throw new Error(why);};
  try {
    await wait(()=>button('Импорт с редактированием…')&&!button('Импорт с редактированием…').disabled,'editor available');
    const dev=(await rpc('list_devices'))[0].id;
    const before=(await rpc('list_named_theme_assets')).map(a=>a.key).sort().join(',');
    const lcd=(await rpc('get_lcd_state',{dev})).source;
    button('Импорт с редактированием…').click();
    await wait(()=>button('Снять экран…'),'capture action');button('Снять экран…').click();
    await wait(()=>document.body.textContent.includes('Снимок экрана.png · 64 × 32 px'),'raw native screenshot inspected');
    assert((await rpc('list_named_theme_assets')).map(a=>a.key).sort().join(',')===before,'capture did not publish');
    button('Подготовить preview изображения').click();
    await wait(()=>{const img=document.querySelector('img[alt="Результат редактирования 480 на 480"]');return img?.complete&&img.naturalWidth===480;},'native screenshot preview');
    const canvas=document.createElement('canvas');canvas.width=480;canvas.height=480;
    const ctx=canvas.getContext('2d');ctx.drawImage(document.querySelector('img[alt="Результат редактирования 480 на 480"]'),0,0);
    const left=Array.from(ctx.getImageData(80,240,1,1).data),right=Array.from(ctx.getImageData(400,240,1,1).data);
    assert(left[0]>245&&left[2]<10&&right[2]>245&&right[0]<10,'screenshot pixels preserved by native IPC and crop');
    button('Сохранить изображение в галерею').click();
    let saved;
    await wait(async()=>{saved=(await rpc('list_named_theme_assets')).find(a=>!a.is_default&&a.name==='Снимок экрана.png');return !!saved;},'screenshot published by Save');
    assert((await rpc('get_lcd_state',{dev})).source===lcd,'screenshot did not Apply');
    await wait(()=>button('Снять экран…')&&!button('Снять экран…').disabled,'capture ready after Save');
    button('Снять экран…').click();
    await wait(()=>document.body.textContent.includes('Снимок экрана отменён.'),'portal cancellation');
    assert(document.body.textContent.includes('Снимок экрана.png · 64 × 32 px'),'cancel retains source');
    button('Снять экран…').click();
    await invoke('native_media_step',{sequence:1,action:'screenshot_cancel_ready'});
    button('Закрыть редактор изображения').click();
    await wait(()=>!document.querySelector('[aria-label="Редактор изображения"]'),'editor close while capturing');
    await wait(()=>document.activeElement===button('Импорт с редактированием…'),'focus after cancelling screenshot');
    button('Импорт с редактированием…').click();
    await wait(()=>button('Снять экран…'),'reopen screenshot editor');button('Снять экран…').click();
    await invoke('native_media_step',{sequence:2,action:'screenshot_close_ready'});
    await invoke('native_smoke_result',{result:{ok:true,raw_bytes:true,preview_pixels:{left,right},saved_key:saved.key,portal_cancel:true,editor_cancel:true,close_pending:true},keepOpen:true});
  } catch(error) {
    await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack,alerts:[...document.querySelectorAll('[role="alert"]')].map(e=>e.textContent)}});
  }
})();
