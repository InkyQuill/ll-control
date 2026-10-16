// Private native-smoke only: actual WebKit/Tauri/Unix IPC/global settings.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let n=0;n<400;n++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const recovery=window.__nativeDesktopPhase==='2';
 try {
  if((await rpc('list_devices')).length!==0)throw new Error('FPS settings require no device in this fixture');
  await click(recovery?'Настройки':'Settings');
  let panel;await wait(()=>panel=document.querySelector(`section[aria-label="${recovery?'Источник FPS':'FPS source'}"]`),'FPS panel');
  await wait(()=>panel.querySelector('input[type=text]'),'loaded FPS draft');
  const input=panel.querySelector('input[type=text]'),enabled=panel.querySelector('input[type=checkbox]');
  if(enabled.checked)throw new Error('Initial/restored source should be off');
  const initial=await rpc('get_fps_source');
  if(recovery) {
   if(input.value!==window.__nativeFpsLog||initial.revision!==2||initial.log_path!==window.__nativeFpsLog)throw new Error('Saved source did not survive daemon/GUI restart');
  } else {
   if(input.value!=='')throw new Error('Fresh source must be unselected');
   input.value=window.__nativeFpsLog;input.dispatchEvent(new Event('input',{bubbles:true}));
   enabled.checked=true;enabled.dispatchEvent(new Event('change',{bubbles:true}));
   await click('Save FPS settings');await wait(async()=>{const s=await rpc('get_fps_source');return s.enabled&&s.revision===1&&s.log_path===window.__nativeFpsLog;},'enabled source saved');
   await wait(()=>button('Save FPS settings')&&!button('Save FPS settings').disabled&&!enabled.disabled,'native enabled readback');
   if(!enabled.checked||input.value!==window.__nativeFpsLog)throw new Error('Enabled UI readback mismatch');
   const display=await rpc('get_display_settings');await rpc('set_display_settings',{settings:{...display,interface_language:'ru-RU'}});
   await wait(()=>panel.getAttribute('aria-label')==='Источник FPS','Russian resource');
   if(input.value!==window.__nativeFpsLog)throw new Error('Language switch rewrote source data');
   enabled.checked=false;enabled.dispatchEvent(new Event('change',{bubbles:true}));
   await click('Сохранить настройки FPS');await wait(async()=>{const s=await rpc('get_fps_source');return !s.enabled&&s.revision===2&&s.log_path===window.__nativeFpsLog;},'disabled source preserves log');
   await wait(()=>button('Сохранить настройки FPS')&&!button('Сохранить настройки FPS').disabled&&!enabled.disabled,'native disabled readback');
   if(enabled.checked)throw new Error('Disabled UI readback mismatch');
  }
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,recovery,noDevices:true,settings:await rpc('get_fps_source'),language:await rpc('get_display_settings')}});
 } catch(e) {await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
