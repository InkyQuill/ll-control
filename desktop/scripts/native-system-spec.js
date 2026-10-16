// Real WebKitGTK/Tauri/Unix RPC; all inventory files belong to the private fixture.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const has=text=>[...document.querySelectorAll('span,h3,p')].some(e=>e.textContent.trim()===text);
 try{
  const devices=await invoke('daemon_call',{method:'list_devices',params:{}});if(devices.length)throw new Error('Spec fixture must not attach a pump');
  await click('Сведения о системе');await wait(()=>has('Fixture CPU'),'CPU inventory');
  await click('GPU');await wait(()=>has('Fixture GPU A'),'GPU inventory');
  const select=document.querySelector('section[aria-label="Сведения о системе"] select');
  if(select.options.length!==2)throw new Error('Two GPU inventory items required');select.value=select.options[1].value;select.dispatchEvent(new Event('change',{bubbles:true}));await wait(()=>has('Fixture GPU B'),'GPU detail selection');
  await click('Память');await wait(()=>has('32 GiB')&&has('Недоступно'),'OS memory with unknown DIMMs');
  await click('Мониторы');await wait(()=>has('Сведения в этой категории недоступны.'),'missing monitor');
  await click('Материнская плата');await wait(()=>has('Fixture Board'),'board inventory');
  await click('Накопители');await wait(()=>has('Fixture SSD'),'storage inventory');
  await click('Обновить сведения');await wait(()=>!button('Обновить сведения').disabled,'explicit refresh');
  const spec=await invoke('daemon_call',{method:'get_system_spec',params:{refresh:false}});
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,noDevices:true,categories:spec.groups.map(g=>g.kind),spec,explicitRefresh:true}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
