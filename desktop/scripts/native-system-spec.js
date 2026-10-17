// Real WebKitGTK/Tauri/Unix RPC; all inventory files belong to the private fixture.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const labels={
  "section[aria-label=\"Сведения о системе\"] select": "section[aria-label=\"System information\"] select",
  "Материнская плата": "Motherboard",
  "Мониторы": "Monitors",
  "Накопители": "Storage",
  "Недоступно": "Unavailable",
  "Обновить сведения": "Refresh information",
  "Память": "Memory",
  "Сведения в этой категории недоступны.": "Information in this category is unavailable.",
  "Сведения о системе": "System information"
};
 let language='ru-RU';
 const text=value=>language==='en-US'?labels[value]:value;
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const has=text=>[...document.querySelectorAll('span,h3,p')].some(e=>e.textContent.trim()===text);
 try{
  language=(await invoke('daemon_call',{method:'get_display_settings',params:{}})).interface_language;
  const devices=await invoke('daemon_call',{method:'list_devices',params:{}});if(devices.length)throw new Error('Spec fixture must not attach a pump');
  await click(text("Сведения о системе"));await wait(()=>has('Fixture CPU'),'CPU inventory');
  await click('GPU');await wait(()=>has('Fixture GPU A'),'GPU inventory');
  const select=document.querySelector(text("section[aria-label=\"Сведения о системе\"] select"));
  if(select.options.length!==2)throw new Error('Two GPU inventory items required');select.value=select.options[1].value;select.dispatchEvent(new Event('change',{bubbles:true}));await wait(()=>has('Fixture GPU B'),'GPU detail selection');
  await click(text("Память"));await wait(()=>has((1.5).toLocaleString(language,{maximumFractionDigits:2})+' GiB')&&has(text("Недоступно")),'OS memory with unknown DIMMs');
  await click(text("Мониторы"));await wait(()=>has(text("Сведения в этой категории недоступны.")),'missing monitor');
  await click(text("Материнская плата"));await wait(()=>has('Fixture Board'),'board inventory');
  await click(text("Накопители"));await wait(()=>has('Fixture SSD'),'storage inventory');
  await click(text("Обновить сведения"));await wait(()=>!button(text("Обновить сведения")).disabled,'explicit refresh');
  const spec=await invoke('daemon_call',{method:'get_system_spec',params:{refresh:false}});
  const stamp=document.querySelector('.updated').textContent;
  if(!stamp.includes(new Date(spec.collected_at_unix_ms).toLocaleString(language)))throw new Error('collection date locale');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,language,fractional_bytes:true,collection_date_locale:true,noDevices:true,categories:spec.groups.map(g=>g.kind),spec,explicitRefresh:true}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
