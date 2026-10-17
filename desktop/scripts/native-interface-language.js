// Real language resources and saved daemon preferences; all roots are private.
(async()=>{
 const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
 const rpc=(method,params={})=>invoke('daemon_call',{method,params});
 const wait=async(test,label)=>{for(let i=0;i<400;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
 const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
 const control=name=>button(name)??[...document.querySelectorAll('summary')].find(node=>node.getClientRects().length&&node.textContent.trim()===name);
 const click=async name=>{await wait(()=>control(name)&&!control(name).disabled,name);control(name).click();};
 const field=label=>[...document.querySelectorAll('label')].find(l=>l.textContent.trim().startsWith(label))?.querySelector('input,select');
 const canonical=value=>JSON.stringify(value&&typeof value==='object'?(Array.isArray(value)?value.map(item=>JSON.parse(canonical(item))):Object.fromEntries(Object.keys(value).sort().map(key=>[key,JSON.parse(canonical(value[key]))]))):value);
 const set=(input,value)=>{if(!input)throw new Error('Missing language fixture field');input.value=value;input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input',{bubbles:true}));};
 try{
  if(window.__nativeDesktopPhase==='cooling-errors'){
   await click('Cooling');
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'cooling language');};
   const panel=()=>document.querySelector('section[aria-label="Cooling"],section[aria-label="Охлаждение"]');
   const alerts=()=>[...panel().querySelectorAll('[role="alert"]')].map(node=>node.textContent).join('\n');
   const dev=(await rpc('list_devices'))[0].id,cases=[];
   for(const [index,lang] of ['en','ru'].entries()){
    const english=lang==='en',other=english?'ru':'en',value=61+index*2;
    await language(lang);
    const before=(await rpc('get_cooling_state',{dev})).pump.requested;
    set(field('PWM, %'),value);
    if(JSON.stringify((await rpc('get_cooling_state',{dev})).pump.requested)!==JSON.stringify(before))throw new Error('Cooling draft was applied');
    await click(english?'Apply cooling':'Применить охлаждение');
    await wait(()=>alerts().includes(english?'Cooling application was not confirmed:':'Применение охлаждения не подтверждено:')&&alerts().includes('Настройки'),'cooling lost reply message');
    await wait(async()=>(await rpc('get_cooling_state',{dev})).pump.requested?.percent===value,'real cooling commit despite reply fault');
    await wait(()=>panel().textContent.includes(`Applied: ${value}%`),'cooling readback after lost reply');
    if(Number(field('PWM, %').value)!==value)throw new Error('Lost cooling reply replaced the draft');
    await language(other);
    await wait(()=>alerts().includes(english?'Применение охлаждения не подтверждено:':'Cooling application was not confirmed:')&&alerts().includes('Настройки'),'reactive cooling failure with raw diagnostic');
    set(field('PWM, %'),value+1);
    await click(english?'Применить охлаждение':'Apply cooling');
    await wait(async()=>(await rpc('get_cooling_state',{dev})).pump.requested?.percent===value+1&&!alerts(),'explicit cooling retry');
    if(Number(field('PWM, %').value)!==value+1)throw new Error('Cooling retry lost the edited value');
    cases.push({language:lang,committed:value,retried:value+1});
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,noDevices:false,coolingErrors:true,cases,actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='library-errors'){
   await click('Pump screen');
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'library language');};
   const alerts=()=>[...document.querySelectorAll('[role="alert"]')].filter(node=>node.getClientRects().length).map(node=>node.textContent).join('\n');
   const cases=[];
   for(const lang of ['en','ru']){
    const english=lang==='en',other=english?'ru':'en';await language(lang);
    const canvas=document.createElement('canvas');canvas.width=96;canvas.height=64;const context=canvas.getContext('2d');context.fillStyle=english?'#2471c9':'#cb7624';context.fillRect(0,0,96,64);
    const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));if(!blob)throw new Error('Library seed encoding failed');
    const name=`Файл {1} <CPU> ${lang}.png`,key=await invoke('import_image',{name,bytes:Array.from(new Uint8Array(await blob.arrayBuffer()))});
    if(lang==='en'){await click('Library');await click('Image library');}else await click('Обновить изображения');
    await click(`${english?'Rename':'Переименовать'} ${name}`);set(field(english?'Image name':'Имя изображения'),'Настройки');
    await click(english?'Save image name':'Сохранить имя изображения');
    await wait(()=>alerts().includes(english?'Renaming was not confirmed':'Переименование не подтверждено')&&alerts().includes('Настройки'),'committed rename with reply fault');
    await wait(()=>!!button(`${english?'Rename':'Переименовать'} Настройки`),'fresh renamed card');
    await language(other);await wait(()=>alerts().includes(english?'Переименование не подтверждено':'Renaming was not confirmed'),'reactive rename error');
    const save=button(english?'Сохранить имя изображения':'Save image name');if(!save?.disabled)throw new Error('Stale rename form offered retry');
    if(field(english?'Имя изображения':'Image name')?.value!=='Настройки')throw new Error('Rename error lost attempted name');
    await click(`${english?'Переименовать':'Rename'} Настройки`);
    const finalName=`Final {1} <CPU> ${lang}`;set(field(english?'Имя изображения':'Image name'),finalName);await click(english?'Сохранить имя изображения':'Save image name');
    await wait(()=>!!button(`${english?'Переименовать':'Rename'} ${finalName}`),'explicit fresh revision rename');
    const asset=(await rpc('list_named_theme_assets')).find(asset=>asset.key===key);if(asset?.name!==finalName||asset.revision!==3)throw new Error('Fresh rename metadata mismatch: '+JSON.stringify(asset));
    cases.push({language:lang,key,finalName,revision:asset.revision});
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,libraryErrors:true,cases,actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='collection-video-import-errors'){
   await click('Pump screen');await click('Library');await click('Video library');
   const dev=(await rpc('list_devices'))[0].id,cases=[];
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'video import language');};
   const alerts=()=>[...document.querySelectorAll('[role="alert"]')].map(node=>node.textContent).join('\n');
   const initialLayers=document.querySelectorAll('article.layer').length;
   // Minimal owned one-pixel GIF. Exercise the real native decoder and publication.
   const bytes=Uint8Array.from(atob('R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'),c=>c.charCodeAt(0));
   for(const lang of ['en','ru'])for(const mode of ['stale','missing','null','pending','pending-detail','error','malformed']){
    await language(lang);const english=lang==='en',other=english?'ru':'en';
    const before=await rpc('list_prepared_clips'),collections=(await rpc('get_screen_collections',{dev})).collections,name=`R2 ${lang} ${mode}.gif`;
    await wait(()=>field(english?'Import GIF/MP4':'Импортировать GIF/MP4')&&!field(english?'Import GIF/MP4':'Импортировать GIF/MP4').disabled,'native video input ready');
    const input=field(english?'Import GIF/MP4':'Импортировать GIF/MP4'),files=new DataTransfer();files.items.add(new File([bytes],name,{type:'image/gif'}));input.files=files.files;input.dispatchEvent(new Event('change',{bubbles:true}));
    await wait(async()=>{const job=await invoke('media_status');return job?.phase==='complete'&&job.name===name;},'native GIF publication');
    const detail=en=>mode==='error'?'Настройки {0} <disk>':mode==='pending-detail'?(en?'Collections are awaiting persistence. Настройки {0} <disk>':'Коллекции ожидают сохранения. Настройки {0} <disk>'):mode==='pending'?(en?'Collections are awaiting persistence. Read them again.':'Коллекции ожидают сохранения. Повторите чтение.'):mode==='malformed'?(en?'Could not read collection membership':'Не удалось прочитать состав коллекций'):(en?'Adding the asset requires checking collection membership.':'Добавление требует проверки состава коллекций.');
    await wait(()=>alerts().includes(english?'The video was saved to the catalog':'Ролик сохранён в каталоге')&&alerts().includes(detail(english)),'video import recovery '+mode);
    await language(other);await wait(()=>alerts().includes(english?'Ролик сохранён в каталоге':'The video was saved to the catalog')&&alerts().includes(detail(!english)),'video import retained translation');
    const after=await rpc('list_prepared_clips'),added=after.filter(clip=>!before.some(old=>old.key===clip.key));
    if(added.length!==1||added[0].name!==name)throw new Error('GIF import duplicated or name changed');
    const state=await rpc('get_screen_collections',{dev}),expected={...collections,revision:collections.revision+1,backgrounds:[...collections.backgrounds,{kind:'video',key:added[0].key}]};
    if(state.persistence.pending||canonical(state.collections)!==canonical(expected))throw new Error('Video import changed adjacent collection state: '+JSON.stringify({state,expected}));
    if(document.querySelectorAll('article.layer').length!==initialLayers||await rpc('get_lcd_media',{dev})!==null||(await rpc('get_lcd_state',{dev})).source!=='Idle')throw new Error('Video import changed draft/output');
    cases.push({language:lang,mode,key:added[0].key,revision:state.collections.revision});
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,videoImportErrors:true,cases,actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='collection-video-errors'){
   await click('Pump screen');await click('Library');await click('Video library');
   const dev=(await rpc('list_devices'))[0].id,cases=[];
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'video replacement language');};
   const alerts=()=>[...document.querySelectorAll('[role="alert"]')].map(node=>node.textContent).join('\n');
   const panel=()=>document.querySelector('section[aria-label="Saved video editor"],section[aria-label="Редактор сохранённого ролика"]');
   const initialActive=JSON.stringify(await rpc('get_lcd_theme',{dev}));
   for(const lang of ['en','ru'])for(const mode of ['stale','missing','null','pending','pending-detail','error','malformed']){
    await language(lang);const english=lang==='en',other=english?'ru':'en';
    await click(english?'Refresh videos':'Обновить ролики');
    const before=await rpc('list_prepared_clips'),collections=(await rpc('get_screen_collections',{dev})).collections;
    const original=before.find(clip=>clip.name==='R2 source.mp4'&&collections.backgrounds.some(item=>item.kind==='video'&&item.key===clip.key));
    if(!original)throw new Error('Current video replacement source missing');
    await wait(()=>field(english?'Saved video':'Сохранённый ролик')?.querySelector(`option[value="${original.key}"]`),'source listed');
    set(field(english?'Saved video':'Сохранённый ролик'),original.key);
    await click(english?'Edit selected video':'Редактировать выбранный ролик');
    await wait(()=>panel()?.querySelector('video'),'managed video source loaded');
    await wait(()=>field(english?'Single VideoEdit range':'Один диапазон VideoEdit'),'range control');
    field(english?'Single VideoEdit range':'Один диапазон VideoEdit').click();
    await click(english?'Prepare video edits':'Подготовить изменения ролика');
    await wait(()=>panel()?.querySelector('video[aria-label="Prepared video"],video[aria-label="Готовый ролик"]'),'video review ready');
    const review=panel().querySelector('video[aria-label="Prepared video"],video[aria-label="Готовый ролик"]');
    review.scrollIntoView({block:'center'});await review.play();await wait(()=>review.readyState>=2&&review.videoWidth===480,'native review decode');review.pause();
    await click(english?'Save prepared video':'Сохранить готовый ролик');
    await wait(async()=>(await rpc('list_prepared_clips')).length===before.length+1,'one new video publication');
    const detail=en=>mode==='error'?'Настройки {0} <disk>':mode==='pending-detail'?(en?'Collections are awaiting persistence. Настройки {0} <disk>':'Коллекции ожидают сохранения. Настройки {0} <disk>'):mode==='pending'?(en?'Collections are awaiting persistence. Read them again.':'Коллекции ожидают сохранения. Повторите чтение.'):mode==='malformed'?(en?'Could not read collection membership':'Не удалось прочитать состав коллекций'):(en?'Replacement requires checking collection membership.':'Замена требует проверки состава коллекций.');
    await wait(()=>alerts().includes(english?'The new video was saved to the catalog':'Новый ролик сохранён в каталоге')&&alerts().includes(detail(english)),'video replacement diagnostic '+mode);
    await language(other);await wait(()=>alerts().includes(english?'Новый ролик сохранён в каталоге':'The new video was saved to the catalog')&&alerts().includes(detail(!english)),'video replacement retained translation');
    const after=await rpc('list_prepared_clips'),added=after.filter(clip=>!before.some(old=>old.key===clip.key));
    if(added.length!==1||added[0].name!==original.name||!after.some(clip=>clip.key===original.key))throw new Error('Video duplicated or original lost');
    const state=await rpc('get_screen_collections',{dev}),expected={...collections,revision:collections.revision+1,backgrounds:collections.backgrounds.map(item=>item.kind==='video'&&item.key===original.key?{...item,key:added[0].key}:item)};
    if(state.persistence.pending||JSON.stringify(state.collections)!==JSON.stringify(expected))throw new Error('Video replacement changed adjacent collection state');
    if(JSON.stringify(await rpc('get_lcd_theme',{dev}))!==initialActive||await rpc('get_lcd_media',{dev})!==null)throw new Error('Video replacement applied without user Apply');
    await wait(()=>!panel(),'video editor joined close');
    cases.push({language:lang,mode,key:added[0].key,oldKey:original.key,revision:state.collections.revision});
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,videoReplacementErrors:true,cases,actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='collection-replacement-errors'){
   await click('Pump screen');await click('Library');await click('Image library');
   const dev=(await rpc('list_devices'))[0].id,cases=[];
   const name='R2 original {0} <CPU>.png';
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'replacement language');};
   const alerts=()=>[...document.querySelectorAll('[role="alert"]')].map(node=>node.textContent).join('\n');
   const panel=()=>document.querySelector('section[aria-label="Image editor"],section[aria-label="Редактор изображения"]');
   const draft=()=>JSON.stringify([...document.querySelectorAll('article.layer')].map(node=>[node.textContent,[...node.querySelectorAll('input,select,textarea')].map(input=>[input.value,input.checked])]));
   const initialDraft=draft(),initialActive=JSON.stringify(await rpc('get_lcd_theme',{dev}));
   for(const lang of ['en','ru'])for(const mode of ['stale','missing','null','pending','pending-detail','error','malformed']){
    await language(lang);const english=lang==='en',other=english?'ru':'en';
    // Localized headings change with language; retain the inputs separately.
    const beforeDraft=draft(),before=await rpc('list_named_theme_assets'),collections=(await rpc('get_screen_collections',{dev})).collections;
    const original=before.find(asset=>asset.name===name&&collections.images.includes(asset.key));
    if(!original)throw new Error('Current replacement source missing');
    await click(english?`Edit ${name}…`:`Редактировать ${name}…`);
    await wait(()=>panel()?.querySelector('img')?.complete,'replacement source decode');
    const brightness=field(english?'Image brightness':'Яркость изображения');set(brightness,cases.length+1);
    await click(english?'Prepare image preview':'Подготовить preview изображения');
    await wait(()=>panel()?.querySelector('img.preview')?.naturalWidth===480,'replacement preview');
    await click(english?'Save image to gallery':'Сохранить изображение в галерею');
    await wait(async()=> (await rpc('list_named_theme_assets')).length===before.length+1,'one replacement asset published');
    const detail=en=>mode==='error'?'Настройки {0} <disk>':mode==='pending-detail'?(en?'Collections are awaiting persistence. Настройки {0} <disk>':'Коллекции ожидают сохранения. Настройки {0} <disk>'):mode==='pending'?(en?'Collections are awaiting persistence. Read them again.':'Коллекции ожидают сохранения. Повторите чтение.'):mode==='malformed'?(en?'Could not read collection membership':'Не удалось прочитать состав коллекций'):(en?'Replacement requires checking collection membership.':'Замена требует проверки состава коллекций.');
    await wait(()=>alerts().includes(english?'Replacement was not confirmed':'Замена не подтверждена')&&alerts().includes(detail(english)),'replacement recovery diagnostic '+mode);
    if(draft()!==beforeDraft)throw new Error('Unconfirmed replacement changed draft');
    await language(other);await wait(()=>alerts().includes(english?'Замена не подтверждена':'Replacement was not confirmed')&&alerts().includes(detail(!english)),'replacement translated retained error');
    const after=await rpc('list_named_theme_assets'),added=after.filter(asset=>!before.some(old=>old.key===asset.key));
    if(added.length!==1||added[0].name!==name||!after.some(asset=>asset.key===original.key))throw new Error('Replacement lost original or duplicated publication');
    const state=await rpc('get_screen_collections',{dev}),expected={...collections,revision:collections.revision+1,images:collections.images.map(key=>key===original.key?added[0].key:key)};
    if(state.persistence.pending||JSON.stringify(state.collections)!==JSON.stringify(expected))throw new Error('Real replacement or adjacent membership differs');
    if(JSON.stringify(await rpc('get_lcd_theme',{dev}))!==initialActive)throw new Error('Replacement implicitly applied');
    if(panel()){
     const save=button(english?'Сохранить изображение в галерею':'Save image to gallery');if(save&&!save.disabled)throw new Error('Unconfirmed replacement offered duplicate Save');
     await click(english?'Закрыть редактор изображения':'Close image editor');
    }
    cases.push({language:lang,mode,key:added[0].key,oldKey:original.key,revision:state.collections.revision});
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,collectionReplacementErrors:true,cases,initialDraft,actual_hardware_access:false}});return;
  }
  if(['collection-readback-errors','collection-plain-errors'].includes(window.__nativeDesktopPhase)){
   const plain=window.__nativeDesktopPhase==='collection-plain-errors';
   await click('Pump screen');
   const dev=(await rpc('list_devices'))[0].id;
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'collection readback language');};
   const panel=()=>document.querySelector('section[aria-label="Image editor"],section[aria-label="Редактор изображения"]');
   const alert=()=>[...(plain?document:panel()).querySelectorAll('[role="alert"]')].map(node=>node.textContent).join('\n');
   const assets=async()=>(await rpc('list_named_theme_assets')).filter(asset=>!asset.is_default);
   const initialLayers=document.querySelectorAll('.layer').length,cases=[];
   const modes=['stale','missing','null','pending','pending-detail','error','malformed'];
   for(const lang of ['en','ru'])for(const mode of modes){
    const english=lang==='en',other=english?'ru':'en',index=cases.length;await language(lang);
    const before=await assets(),collections=(await rpc('get_screen_collections',{dev})).collections;
    const name=`Настройки {1} <CPU> ${lang} ${mode}.png`;
    if(!plain){await click(english?'Import and edit…':'Импорт с редактированием…');await wait(()=>!!field(english?'File to edit':'Файл для редактирования'),'readback source picker');}
    const canvas=document.createElement('canvas');canvas.width=96;canvas.height=64;const context=canvas.getContext('2d');context.fillStyle=`rgb(${17+index*7},${41+index*13},${67+index*5})`;context.fillRect(0,0,96,64);
    const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));if(!blob)throw new Error('Readback source encoding failed');
    const files=new DataTransfer();files.items.add(new File([blob],name,{type:'image/png'}));const input=field(plain?(english?'+ Image':'+ Изображение'):(english?'File to edit':'Файл для редактирования'));input.files=files.files;input.dispatchEvent(new Event('change',{bubbles:true}));
    if(!plain){
    await wait(()=>panel()?.textContent.includes(name+' · 96 × 64 px'),'readback source decoded');
    await click(english?'Prepare image preview':'Подготовить preview изображения');
    await wait(()=>panel()?.querySelector('img[alt="Edited result, 480 by 480"],img[alt="Результат редактирования 480 на 480"]')?.naturalWidth===480,'readback preview decoded');
    await click(english?'Save image to gallery':'Сохранить изображение в галерею');
    }
    const detail=en=>mode==='error'?'Настройки {0} <disk>':mode==='pending-detail'?(en?'Collections are awaiting persistence. Настройки {0} <disk>':'Коллекции ожидают сохранения. Настройки {0} <disk>'):mode==='pending'?(en?'Collections are awaiting persistence. Read them again.':'Коллекции ожидают сохранения. Повторите чтение.'):mode==='malformed'?(en?'Could not read collection membership':'Не удалось прочитать состав коллекций'):(en?'Adding the asset requires checking collection membership.':'Добавление требует проверки состава коллекций.');
    await wait(()=>(plain||panel())&&alert().includes(english?'The file was saved to the catalog':'Файл сохранён в каталоге')&&alert().includes(detail(english)),`readback ${lang}/${mode} owned/raw error`);
    await language(other);await wait(()=>alert().includes(english?'Файл сохранён в каталоге':'The file was saved to the catalog')&&alert().includes(detail(!english)),`readback ${lang}/${mode} retained error after language`);
    if(!plain&&!button(english?'Сохранить изображение в галерею':'Save image to gallery')?.disabled)throw new Error(`Readback ${mode} offered duplicate Save`);
    if(!plain&&!panel().textContent.includes(name+' · 96 × 64 px'))throw new Error('Readback error changed original filename');
    if(document.querySelectorAll('.layer').length!==initialLayers)throw new Error('Readback failure modified LCD draft');
    const after=await assets(),added=after.filter(asset=>!before.some(old=>old.key===asset.key));
    if(after.length!==before.length+1||added.length!==1||added[0].name!==name)throw new Error('Readback failure did not preserve one named catalog asset');
    const state=await rpc('get_screen_collections',{dev}),expected={...collections,revision:collections.revision+1,images:[...collections.images,added[0].key]};
    if(state.persistence.pending||JSON.stringify(state.collections)!==JSON.stringify(expected))throw new Error('Fault fixture did not preserve real committed membership: '+JSON.stringify({state,expected}));
    cases.push({language:lang,mode,operation:plain?'plain':'edited',key:added[0].key,name,revision:state.collections.revision});
    if(!plain){await click(english?'Закрыть редактор изображения':'Close image editor');await wait(()=>!panel(),'readback editor joined close');}
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,collectionReadbackErrors:true,cases,initialLayers,actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='image-errors'){
   await click('Pump screen');
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'image error language');};
   const alerts=(scope=document)=>[...scope.querySelectorAll('[role="alert"]')].filter(node=>node.getClientRects().length).map(node=>node.textContent).join('\n');
   const panel=()=>document.querySelector('section[aria-label="Image editor"],section[aria-label="Редактор изображения"]');
   const rootError=()=>document.querySelector('main > .error[role="alert"]')?.textContent??'';
   const imageFile=async(color,name)=>{const canvas=document.createElement('canvas');canvas.width=96;canvas.height=64;const context=canvas.getContext('2d');context.fillStyle=color;context.fillRect(0,0,96,64);const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));if(!blob)throw new Error('Synthetic image encoding failed');return new File([blob],name,{type:'image/png'});};
   const choose=async(input,file)=>{await wait(()=>input&&!input.disabled,'image picker enabled');const files=new DataTransfer();files.items.add(file);input.files=files.files;input.dispatchEvent(new Event('change',{bubbles:true}));};
   const assets=async()=>(await rpc('list_named_theme_assets')).filter(asset=>!asset.is_default).map(asset=>asset.key).sort();
   const initialLayers=document.querySelectorAll('.layer').length,cases=[];
   for(const lang of ['en','ru']){
    const english=lang==='en',other=english?'ru':'en';await language(lang);
    await click(english?'Import and edit…':'Импорт с редактированием…');
    await choose(field(english?'File to edit':'Файл для редактирования'),new File([new Uint8Array([1,2,3,4])],'invalid.png',{type:'image/png'}));
    const sourcePrefix=english?'Could not open the image: ':'Не удалось открыть изображение: ';
    await wait(()=>panel()&&alerts(panel()).startsWith(sourcePrefix),'native decoder rejection');
    const diagnostic=alerts(panel()).slice(sourcePrefix.length);if(!diagnostic)throw new Error('Decoder diagnostic missing');
    await language(other);await wait(()=>alerts(panel()).startsWith(english?'Не удалось открыть изображение: ':'Could not open the image: ')&&alerts(panel()).endsWith(diagnostic),'raw decoder diagnostic after language switch');
    await click(english?'Закрыть редактор изображения':'Close image editor');await wait(()=>!panel(),'source editor closed');cases.push({language:lang,operation:'native_decode',diagnostic});
    await language(lang);
    const beforePlain=await assets();
    await choose(document.querySelector('.file input[type="file"]'),await imageFile(english?'#d93838':'#3859d9',`Настройки {1} <CPU> ${lang} plain.png`));
    await wait(()=>rootError().includes(english?'The file was saved to the catalog':'Файл сохранён в каталоге')&&rootError().includes('Настройки'),'plain image acknowledged before membership rejection');
    await language(other);await wait(()=>rootError().includes(english?'Файл сохранён в каталоге':'The file was saved to the catalog'),'reactive plain membership context');
    const afterPlain=await assets();if(afterPlain.length!==beforePlain.length+1)throw new Error('Plain import did not retain its acknowledged catalog asset');
    cases.push({language:lang,operation:'plain_membership',asset:afterPlain.find(key=>!beforePlain.includes(key))});await language(lang);
    await click(english?'Import and edit…':'Импорт с редактированием…');
    const name=`Настройки {1} <CPU> ${lang} editor.png`,beforeEdited=await assets();
    await choose(field(english?'File to edit':'Файл для редактирования'),await imageFile(english?'#38d993':'#b638d9',name));
    await wait(()=>panel()?.textContent.includes(name+' · 96 × 64 px'),'untranslated source filename');
    await click(english?'Prepare image preview':'Подготовить preview изображения');
    await wait(()=>panel()?.querySelector('img[alt="Edited result, 480 by 480"],img[alt="Результат редактирования 480 на 480"]')?.naturalWidth===480,'decoded native edited preview');
    await click(english?'Save image to gallery':'Сохранить изображение в галерею');
    await wait(()=>alerts(panel()).includes(english?'The file was saved to the catalog':'Файл сохранён в каталоге')&&alerts(panel()).includes('Настройки'),'editor acknowledged asset before membership rejection');
    await language(other);await wait(()=>alerts(panel()).includes(english?'Файл сохранён в каталоге':'The file was saved to the catalog'),'reactive editor membership context');
    const save=button(english?'Сохранить изображение в галерею':'Save image to gallery');if(!save?.disabled)throw new Error('Ambiguous image save offered blind retry');
    if(!panel().textContent.includes(name+' · 96 × 64 px'))throw new Error('Image error translated a filename');
    const afterEdited=await assets();if(afterEdited.length!==beforeEdited.length+1)throw new Error('Edited import did not retain its acknowledged catalog asset');
    if(document.querySelectorAll('.layer').length!==initialLayers)throw new Error('Membership rejection changed the LCD draft');
    cases.push({language:lang,operation:'editor_membership',asset:afterEdited.find(key=>!beforeEdited.includes(key))});
    await click(english?'Закрыть редактор изображения':'Close image editor');await wait(()=>!panel(),'publication editor closed');
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,imageErrors:true,cases,initialLayers,finalAssets:await assets(),actual_hardware_access:false}});return;
  }
  if(window.__nativeDesktopPhase==='lcd-errors'){
   const dev=(await rpc('list_devices'))[0]?.id;if(!dev)throw new Error('Missing mock LCDV2');
   await click('Pump screen');
   const names={en:{apply:'Apply to LCD',load:'Load active theme',localui:'Restore LocalUI',preview:'Refresh preview'},ru:{apply:'Применить к LCD',load:'Загрузить активную тему',localui:'Вернуть LocalUI',preview:'Обновить preview'}};
   const instructions={en:{apply:'Theme application was not confirmed',load:'Could not load the active theme',localui:'LocalUI restoration was not confirmed',preview:'Could not render the preview'},ru:{apply:'Применение темы не подтверждено',load:'Не удалось загрузить активную тему',localui:'Возврат LocalUI не подтверждён',preview:'Не удалось создать preview'}};
   const language=async lang=>{const settings=await rpc('get_display_settings');settings.interface_language=lang==='en'?'en-US':'ru-RU';await rpc('set_display_settings',{settings});await wait(()=>document.documentElement.lang===lang,'LCD message language');};
   const alert=()=>[...document.querySelectorAll('[role="alert"]')].filter(node=>node.getClientRects().length).map(node=>node.textContent).join('\n');
   const userName='Русское имя {1} <CPU>';
   const cases=[];
   for(const lang of ['en','ru']){
    await language(lang);set(field(lang==='en'?'Name':'Название'),userName);
    for(const operation of ['apply','load','localui','preview']){
     await click(names[lang][operation]);
     await wait(()=>alert().includes(instructions[lang][operation])&&alert().includes('Настройки'),'raw diagnostic and instruction');
     const other=lang==='en'?'ru':'en';await language(other);
     await wait(()=>alert().includes(instructions[other][operation])&&alert().includes('Настройки'),'reactive error instruction');
     if(field(other==='en'?'Name':'Название')?.value!==userName)throw new Error('LCD error changed user draft');
     await click(names[other][operation]);
     await wait(()=>button(names[other][operation])&&!button(names[other][operation]).disabled,'LCD retry completion');
     await wait(()=>!alert().includes('Настройки'),'explicit LCD retry');
     if(operation==='preview')await wait(()=>[...document.querySelectorAll('.lcd img')].some(image=>image.complete&&image.naturalWidth===480),'decoded native LCD preview');
     const expectedSource=['localui','preview'].includes(operation)?'Idle':'Theme';
     await wait(async()=>(await rpc('get_lcd_state',{dev})).source===expectedSource,operation+' source after retry');
     const state=await rpc('get_lcd_state',{dev});
     cases.push({language:lang,operation,source:state.source});await language(lang);
    }
   }
   await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,lcdErrors:true,cases,userName,actual_hardware_access:false}});return;
  }
  const imageDevice=window.__nativeDesktopPhase==='image';
  if((await rpc('list_devices')).length!==(imageDevice?1:0))throw new Error('Unexpected fixture device count');
  const recovery=window.__nativeDesktopPhase==='2';
  const initialEnglish=!recovery,targetEnglish=imageDevice||recovery;
  const initial=await rpc('get_display_settings');if(initial.interface_language!==(initialEnglish?'en-US':'ru-RU'))throw new Error('Fresh English or restored Russian language mismatch');
  await click(initialEnglish?'Pump screen':'Экран помпы');
  await wait(()=>!!field(initialEnglish?'Name':'Название'),'theme draft');
  const initialDraftName=field(initialEnglish?'Name':'Название').value;
  if(initialDraftName!==(initialEnglish?'My screen':'Мой экран'))throw new Error('New draft language mismatch: '+initialDraftName);
  const focusOption=document.querySelector('option[value="focus"]');
  if(focusOption?.textContent!==(initialEnglish?'Focus · One large reading and a clock':'Focus · Один крупный показатель и часы'))throw new Error('Layout resource language mismatch');
  set(field(initialEnglish?'Name':'Название'),'Настройки');
  await click(initialEnglish?'Settings':'Настройки');await wait(()=>!!field(initialEnglish?'Interface language':'Язык интерфейса'),'language select');
  set(field(initialEnglish?'Interface language':'Язык интерфейса'),targetEnglish?'en-US':'ru-RU');await click(initialEnglish?'Save display preferences':'Сохранить параметры отображения');
  await wait(()=>document.documentElement.lang===(targetEnglish?'en':'ru'),'reactive document language');
  await click(targetEnglish?'Pump screen':'Экран помпы');await wait(()=>field(targetEnglish?'Name':'Название')?.value==='Настройки','user draft must not be translated');
  let imageEditorPreview=false,imageMembershipReadback=null;
  if(imageDevice){
   const before=(await rpc('list_named_theme_assets')).map(asset=>asset.key).sort();
   await click('Import and edit…');await wait(()=>!!field('File to edit'),'English file input');
   const canvas=document.createElement('canvas');canvas.width=96;canvas.height=64;const context=canvas.getContext('2d');context.fillStyle='#d93838';context.fillRect(0,0,96,64);
   const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));if(!blob)throw new Error('Synthetic PNG fixture failed');
   const transfer=new DataTransfer();transfer.items.add(new File([blob],'Настройки',{type:'image/png'}));const input=field('File to edit');input.files=transfer.files;input.dispatchEvent(new Event('change',{bubbles:true}));
   const editor=()=>document.querySelector('section[aria-label="Image editor"]');
   await wait(()=>[...editor().querySelectorAll('p')].some(p=>p.textContent==='Настройки · 96 × 64 px'),'source filename must remain untranslated');
   await click('Rotate right');await click('Prepare image preview');
   await wait(()=>{const image=editor()?.querySelector('img[alt="Edited result, 480 by 480"]');return image?.complete&&image.naturalWidth===480;},'actual native image decoder/transform/preview');
   await click('Close image editor');await wait(()=>!editor(),'English editor close');
   const after=(await rpc('list_named_theme_assets')).map(asset=>asset.key).sort();if(JSON.stringify(before)!==JSON.stringify(after))throw new Error('Preview/close published an image');
   const dev=(await rpc('list_devices'))[0].id;
   await rpc('migrate_screen_collections',{dev});
   const membershipBefore=(await rpc('get_screen_collections',{dev})).collections;
   if(!membershipBefore)throw new Error('Native explicit collection migration missing');
   await click('Import and edit…');await wait(()=>!!field('File to edit'),'second English file input');
   const saveTransfer=new DataTransfer();saveTransfer.items.add(new File([blob],'Настройки',{type:'image/png'}));
   const saveInput=field('File to edit');saveInput.files=saveTransfer.files;
   if(saveInput.files.length!==1 || !saveInput.files[0].size)throw new Error('Native second file selection missing');
   saveInput.dispatchEvent(new Event('change',{bubbles:true}));
   await wait(()=>editor()?.textContent.includes('Настройки · 96 × 64 px'),'native import source');
   await click('Prepare image preview');await wait(()=>editor()?.querySelector('img[alt="Edited result, 480 by 480"]')?.naturalWidth===480,'native publication preview');
   await click('Save image to gallery');
   await wait(()=>editor()?.textContent.includes('Result saved and added to the draft'),'native acknowledged membership success');
   const published=(await rpc('list_named_theme_assets')).filter(asset=>!before.includes(asset.key));
   if(published.length!==1)throw new Error('Native Save must publish exactly one asset');
   const key=published[0].key,membershipAfter=await rpc('get_screen_collections',{dev});
   if(membershipAfter.persistence.pending || membershipAfter.collections?.revision!==membershipBefore.revision+1
      || JSON.stringify(membershipAfter.collections.images)!==JSON.stringify([...membershipBefore.images,key])
      || JSON.stringify(membershipAfter.collections.backgrounds)!==JSON.stringify(membershipBefore.backgrounds)
      || JSON.stringify(membershipAfter.collections.watermarks)!==JSON.stringify(membershipBefore.watermarks))throw new Error('Native membership readback differs from expected addition');
   if(!button('Save image to gallery')?.disabled)throw new Error('Native Save offered duplicate publication');
   const lcd=await rpc('get_lcd_state',{dev});if(lcd.source==='Theme')throw new Error('Import implicitly applied LCD theme');
   imageMembershipReadback={asset:key,revision:membershipAfter.collections.revision,pending:false,adjacentMembershipPreserved:true,explicitApply:true};
   await click('Close image editor');await wait(()=>!editor(),'saved native editor close');
   imageEditorPreview=true;
  }
  const settings=await rpc('get_display_settings');if((settings.interface_language??'ru-RU')!==(targetEnglish?'en-US':'ru-RU'))throw new Error('Saved language mismatch');
  if(settings.temperature_unit!=='celsius')throw new Error('Language change altered temperature unit');
  await invoke('native_smoke_result',{keepOpen:true,result:{ok:true,recovery,initial,settings,initialDraftName,userDraft:'Настройки',imageEditorPreview,imageMembershipReadback,documentLanguage:document.documentElement.lang,noDevices:!imageDevice}});
 }catch(e){await invoke('native_smoke_result',{result:{ok:false,error:String(e),stack:e?.stack,alerts:[...document.querySelectorAll('[role="alert"]')].map(node=>node.textContent),editor:document.querySelector('section[aria-label="Image editor"],section[aria-label="Редактор изображения"]')?.textContent}});}
})();
