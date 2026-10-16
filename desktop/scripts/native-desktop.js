// Real Tauri settings, GTK tray/menu and private XDG paths. MockBackend only.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const wait=async(test,label)=>{for(let i=0;i<600;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const checkbox=text=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim()===text)?.querySelector('input');
 const set=(label,value)=>{const input=checkbox(label);if(!input)throw new Error(label);if(input.checked!==value)input.click();};
 try {
  const phase=window.__nativeDesktopPhase;
  await click('Настройки');[...document.querySelectorAll('summary')].find(s=>s.textContent.trim()==='Приложение и автозапуск').click();
  await wait(()=>!!checkbox('Сворачивать в tray при закрытии окна'),'desktop form');
  if(phase!=='3')await wait(async()=>(await invoke('desktop_settings_get')).trayReady,'registered own tray item');
  if(phase==='1'){
   set('Сворачивать в tray при закрытии окна',true);set('Запускать приложение свёрнутым',true);set('Запускать приложение при входе в систему',true);
   await click('Сохранить параметры приложения');await wait(async()=>(await invoke('desktop_settings_get')).registration.enabled===true,'actual autostart registration');
  }
  await invoke('native_desktop_step',{sequence:Number(phase)});
  const state=await invoke('desktop_settings_get');
  if(phase==='2'){
   if(!state.preferences.startMinimized||!state.trayReady)throw new Error('Minimized preference/tray lost on restart');
   set('Запускать приложение при входе в систему',false);await click('Сохранить параметры приложения');
   await wait(async()=>(await invoke('desktop_settings_get')).registration.enabled===false,'autostart disable');
  }
  if(phase==='3'){
   if(state.trayReady)throw new Error('No-host fixture unexpectedly ready');
   set('Сворачивать в tray при закрытии окна',false);set('Запускать приложение свёрнутым',false);set('Запускать приложение при входе в систему',false);
   await click('Сохранить параметры приложения');
   await wait(async()=>!(await invoke('desktop_settings_get')).preferences.closeToTray,'reset isolated preferences');
  }
  await invoke('native_smoke_result',{keepOpen:phase!=='3',result:{ok:true,phase,state:await invoke('desktop_settings_get')}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
