// Actual GUI/Tauri/Unix IPC, private roots and MockBackend; no attached pump.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const select=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('select');
 try{
  if((await rpc('list_devices')).length)throw new Error('Unit fixture requires zero devices');
  await click('Настройки');await wait(()=>select('Единицы температуры'),'unit select');
  const recovery=window.__nativeDesktopPhase==='2';
  if(select('Единицы температуры').value!==(recovery?'fahrenheit':'celsius'))throw new Error('Initial/restored unit mismatch');
  if(!recovery){const input=select('Единицы температуры');input.value='fahrenheit';input.dispatchEvent(new Event('change',{bubbles:true}));await click('Сохранить параметры отображения');await wait(async()=>(await rpc('get_display_settings')).temperature_unit==='fahrenheit','durable Fahrenheit');}
  await click('Датчики');await wait(()=>[...document.querySelectorAll('article')].some(a=>a.querySelector('h2')?.textContent==='Fixture CPU'&&a.querySelector('strong')?.textContent.includes('°F')),'Fahrenheit sensor reading');
  const reading=(await rpc('list_sensors')).find(s=>s.id==='fixture:cpu');if(reading.unit!=='celsius')throw new Error('Canonical source unit changed');
  await click('Экран помпы');await wait(()=>select('Привязать датчик'),'LCD binding');const binding=select('Привязать датчик');binding.value='fixture:cpu';binding.dispatchEvent(new Event('change',{bubbles:true}));await click('Добавить');
  await wait(()=>select('Единица')?.value==='system','new LCD layer follows global settings');
  await click('Обновить preview');await wait(()=>document.querySelector('img[alt^="Подготовленная композиция LCD"]')?.getAttribute('src')?.startsWith('blob:'),'native LCD preview with global temperature');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,recovery,noDevices:true,rawReading:reading,settings:await rpc('get_display_settings'),nativePreview:true}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
