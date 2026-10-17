// Actual GUI/Tauri/Unix IPC, private roots and MockBackend; no attached pump.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
 const select=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('select');
 try{
  const devicePhase=window.__nativeDesktopPhase==='device';
  const devices=await rpc('list_devices');if(devices.length!==(devicePhase?1:0))throw new Error('Unit fixture device count mismatch');
  await click('Настройки');await wait(()=>select('Единицы температуры'),'unit select');
  const recovery=window.__nativeDesktopPhase==='2'||devicePhase;
  if(select('Единицы температуры').value!==(recovery?'fahrenheit':'celsius'))throw new Error('Initial/restored unit mismatch');
  if(!recovery){const input=select('Единицы температуры');input.value='fahrenheit';input.dispatchEvent(new Event('change',{bubbles:true}));await click('Сохранить параметры отображения');await wait(async()=>(await rpc('get_display_settings')).temperature_unit==='fahrenheit','durable Fahrenheit');}
  await click('Датчики');await wait(()=>[...document.querySelectorAll('article')].some(a=>a.querySelector('h2')?.textContent==='Fixture CPU'&&a.querySelector('strong')?.textContent.includes('°F')),'Fahrenheit sensor reading');
  const reading=(await rpc('list_sensors')).find(s=>s.id==='fixture:cpu');if(reading.unit!=='celsius')throw new Error('Canonical source unit changed');
  if(!devicePhase&&(button('Подсветка')||button('Охлаждение')))throw new Error('Hardware controls exposed without a device');
  let rgbConfig=null;
  if(devicePhase){
  const field=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('input,select');
  const set=(input,value)=>{if(!input)throw new Error('Missing temperature field');input.value=value;input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input',{bubbles:true}));};
  const dev=devices[0].id;await click('Подсветка');await wait(()=>select('Эффект')&&!select('Эффект').disabled&&select('Эффект').options.length>0,'lighting catalog ready');
  const effect=select('Эффект'),selectedMode=[...effect.options].some(o=>o.value==='static')?'static':'static_color';set(effect,selectedMode);
  if(effect.value!==selectedMode)throw new Error('Static mode option missing');await click('Применить подсветку');
  await wait(async()=>['static','static_color'].includes((await rpc('get_dynamic_rgb',{dev})).base_mode),'confirmed Static pump base');
  [...document.querySelectorAll('summary')].find(n=>n.textContent.trim()==='Подсветка помпы по датчику').click();await click('Загрузить настройки режима по датчику');
  set(select('Датчик подсветки'),'fixture:cpu');await wait(()=>field('Нижний порог (°F)'),'Fahrenheit RGB fields');
  const low=field('Нижний порог (°F)'),high=field('Верхний порог (°F)');set(low,'100');
  await wait(()=>low.value==='100'&&low.getAttribute('aria-valuenow')==='100'&&low.getAttribute('aria-invalid')==='true','consistent native accessible 100F');
  if(low.getAttribute('aria-describedby')!=='dynamic-rgb-temperature-grid'||!document.getElementById('dynamic-rgb-temperature-grid'))throw new Error('RGB grid description missing');
  await click('Применить режим по датчику');await wait(()=>[...document.querySelectorAll('[role=alert]')].some(n=>n.textContent.includes('целым °C')),'canonical grid validation');
  if((await rpc('get_dynamic_rgb',{dev})).requested!==null)throw new Error('Fractional Celsius unexpectedly installed');
  set(low,'87.8');set(high,'143.6');await wait(()=>low.getAttribute('aria-invalid')==='false'&&high.getAttribute('aria-invalid')==='false','whole Celsius input');await click('Применить режим по датчику');
  await wait(async()=>{const c=(await rpc('get_dynamic_rgb',{dev})).requested;return c?.low_value===31&&c.high_value===62&&c.unit==='celsius';},'actual IPC canonical integer RGB values');
  rgbConfig=(await rpc('get_dynamic_rgb',{dev})).requested;
  }
  await click('Экран помпы');await wait(()=>select('Привязать датчик'),'LCD binding');const binding=select('Привязать датчик');binding.value='fixture:cpu';binding.dispatchEvent(new Event('change',{bubbles:true}));await click('Добавить');
  await wait(()=>select('Единица')?.value==='system','new LCD layer follows global settings');
  if(!devicePhase&&!button('Применить к LCD')?.disabled)throw new Error('Offline draft allowed Apply');
  await click('Обновить preview');await wait(()=>document.querySelector('img[alt^="Подготовленная композиция LCD"]')?.getAttribute('src')?.startsWith('blob:'),'native LCD preview with global temperature');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,recovery,noDevices:!devicePhase,offlineApplyBlocked:!devicePhase,rawReading:reading,settings:await rpc('get_display_settings'),nativePreview:true,integerRgb:devicePhase,rgbBaseMode:devicePhase?(await rpc('get_dynamic_rgb',{dev:devices[0].id})).base_mode:null,rgbConfig}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack}});}
})();
