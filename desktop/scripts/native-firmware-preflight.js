// Private native-smoke build only. Actual Tauri command/IPC/MockBackend query.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await sleep(50);}throw new Error(label);};
 const russian=window.__nativeDesktopPhase.startsWith('ru');
 const hasDate=window.__nativeDesktopPhase.endsWith('with-date');
 const title=russian?'Сведения о прошивке':'Firmware information';
 const readLabel=russian?'Прочитать сведения о прошивке':'Read firmware information';
 const button=label=>[...document.querySelectorAll('button')].find(node=>node.textContent.trim()===label);
 const panel=()=>document.querySelector(`section[aria-label="${title}"]`);
 try {
  const devices=await rpc('list_devices');if(devices.length!==1)throw new Error('Expected one mock device');const dev=devices[0].id;
  await wait(()=>button(russian?'Настройки':'Settings'),'localized Settings');button(russian?'Настройки':'Settings').click();
  await wait(()=>panel()&&button(readLabel),'firmware card');
  if(panel().querySelectorAll('dd').length)throw new Error('Initial firmware value fabricated');
  const before=await rpc('get_lcd_state',{dev}),cooling=await rpc('get_cooling_state',{dev});
  await sleep(1700);
  if(panel().querySelectorAll('dd').length)throw new Error('Firmware was read without an explicit click');
  button(readLabel).focus();button(readLabel).click();
  await wait(()=>panel().querySelector('dd')?.textContent==='1.6','native firmware query');
  const date=panel().querySelectorAll('dd')[1].textContent;
  if(date!==(hasDate?'Apr 12 2024 14:33:21':russian?'Недоступна':'Unavailable'))throw new Error('Date presence/locale mismatch: '+date);
  const identity=[...panel().querySelectorAll('summary')].find(node=>node.textContent===(russian?'Идентификация устройства':'Device identity'));identity.click();
  const token=panel().querySelector('pre').textContent;
  if(!token.startsWith('N9,01,HS,SQ,CA_II-Vision,V2.01.041,1.6|'))throw new Error('Raw firmware identity changed');
  const after=await rpc('get_lcd_state',{dev}),afterCooling=await rpc('get_cooling_state',{dev});
  if(JSON.stringify(before)!==JSON.stringify(after)||JSON.stringify(cooling)!==JSON.stringify(afterCooling))throw new Error('Information query changed device settings');
  await sleep(1700);
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,language:russian?'ru-RU':'en-US',hasDate,date,token,explicit_read:true,no_implicit_apply:true}});
 }catch(error){await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack}});}
})();
