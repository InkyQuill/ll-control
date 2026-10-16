// Included only in native-smoke builds; private MockBackend acceptance.
(async()=>{
  const invoke=(command,args={})=>window.__TAURI_INTERNALS__.invoke(command,args);
  const rpc=(method,params={})=>invoke('daemon_call',{method,params});
  const wait=async(test,label)=>{for(let i=0;i<300;i++){if(await test())return;await new Promise(r=>setTimeout(r,50));}throw new Error(label);};
  const button=name=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===name);
  const field=label=>[...document.querySelectorAll('label')].find(e=>e.textContent.trim().startsWith(label))?.querySelector('input,select');
  const click=async name=>{await wait(()=>button(name)&&!button(name).disabled,name);button(name).click();};
  const select=async(label,value)=>{await wait(()=>field(label)&&!field(label).matches(':disabled')&&[...field(label).options].some(o=>o.value===value),label);if(field(label).value===value)return;field(label).value=value;field(label).dispatchEvent(new Event('change',{bubbles:true}));};
  const color=async(label,value)=>{await wait(()=>field(label)&&!field(label).disabled,label);field(label).value=value;field(label).dispatchEvent(new Event('input',{bubbles:true}));};
  try {
    await wait(async()=>(await rpc('list_devices')).length===1,'mock device');
    const dev=(await rpc('list_devices'))[0].id;
    await click('Подсветка');
    for(const kind of ['pump','fan']) {
      await select('Зона',kind);
      const catalog=(await rpc('get_lighting_info',{dev,kind})).catalog;
      if(catalog.length!==(kind==='fan'?6:16))throw new Error('incomplete native catalog');
      const asHex=c=>'#'+[c.r,c.g,c.b].map(v=>v.toString(16).padStart(2,'0')).join('');
      for(const entry of catalog) {
        await select('Эффект',entry.mode);
        await wait(()=>!!field('Скорость:')===entry.speed&&!!field('Направление')===entry.direction,'mode control visibility');
        const colors=[entry.defaults.color,...(entry.defaults.additional_colors ?? [])].slice(0,entry.color_count);
        for(const [index,value] of colors.entries())await wait(()=>field(index===0?'Цвет RGB':'Цвет RGB '+(index+1))?.value===asHex(value),'mode default color');
        if(entry.speed)await wait(()=>field('Скорость:').value===String(entry.defaults.speed_percent),'mode default speed '+kind+'/'+entry.mode+' expected '+entry.defaults.speed_percent);
      }
      await select('Эффект','runway');await color('Цвет RGB','#112233');await color('Цвет RGB 2','#445566');
      await click('Применить подсветку');
      await wait(()=>!button('Применить подсветку').disabled,'inactive mode applied');
      await select('Эффект','meteor');
      for(const [index,value] of ['#010203','#040506','#070809','#0a0b0c'].entries()) {
        await color(index===0?'Цвет RGB':'Цвет RGB '+(index+1),value);
      }
      await click('Применить подсветку');
      await wait(()=>[...document.querySelectorAll('[role="status"]')].some(e=>e.textContent.includes('Команда подсветки выполнена')),'palette Apply');
      await wait(()=>!button('Применить подсветку').disabled,'Apply settled');
      await select('Эффект','runway');
      await wait(()=>field('Цвет RGB').value==='#112233'&&field('Цвет RGB 2').value==='#445566','per-mode draft restored');
      await select('Эффект','meteor');await wait(()=>field('Цвет RGB 4')?.value==='#0a0b0c','meteor draft restored');
      await click('Значения режима по умолчанию');
      const base=(await rpc('get_lighting_info',{dev,kind})).base;
      if(base.additional_colors[2].b!==12)throw new Error('reset defaults implicitly applied');

    }
    for(const kind of ['pump','fan']) {
      await select('Зона',kind);await select('Эффект','static');
      await color('Цвет RGB','#ffffff');
      await click('Загрузить настройки подсветки');
      await wait(()=>field('Эффект').value==='meteor'&&field('Цвет RGB 4')?.value==='#0a0b0c','full base loaded');
      if(field('Цвет RGB').value!=='#010203'||field('Цвет RGB 2').value!=='#040506'||field('Цвет RGB 3').value!=='#070809')throw new Error('loaded palette mismatch');
      const info=await rpc('get_lighting_info',{dev,kind});
      if(info.base.mode!=='meteor'||info.base.additional_colors.length!==3)throw new Error('owner base readback mismatch');
    }
    await select('Зона','fan');await select('Эффект','meteor');
    const sync=()=>field('Синхронизировать вентиляторы с помпой');
    const count=()=>field('Число LED в ручном режиме');
    count().value='31';count().dispatchEvent(new Event('input',{bubbles:true}));
    for(const [enabled,size] of [[true,'280'],[true,'360'],[false,'360'],[true,'280']]) {
      if(sync().checked!==enabled)sync().click();
      if(enabled)await select('Размер комплекта вентиляторов',size);
      await click('Применить подсветку');
      await wait(async()=>{const t=(await rpc('get_lighting_info',{dev,kind:'fan'})).fan_topology;return t.sync_to_pump===enabled&&t.manual_led_count===31&&(!enabled||t.size===size);},'global fan topology applied');
      if(count().value!=='31')throw new Error('sync discarded retained manual LED count');
    }
    const number=async(label,value)=>{await wait(()=>field(label)&&!field(label).matches(':disabled'),label);field(label).value=String(value);field(label).dispatchEvent(new Event('input',{bubbles:true}));};
    const summary=[...document.querySelectorAll('summary')].find(e=>e.textContent.trim()==='Подсветка помпы по датчику');summary.click();
    await click('Загрузить настройки режима по датчику');
    await select('Датчик подсветки','fixture:cpu');
    await number('Нижний порог (celsius)',45);await color('Нижний цвет 2','#112233');
    await number('Нижняя скорость',20);await number('Верхняя скорость',80);
    await number('Яркость по датчику',50);await select('Направление по датчику','reverse');
    await click('Применить режим по датчику');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.requested?.sensor_id==='fixture:cpu'&&d.requested.brightness_percent===50&&!!d.applied&&!d.degraded&&!d.last_error;},'CPU source applied');
    await select('Датчик подсветки','amdgpu:2f61:utilization');
    await number('Нижний порог (percent)',35);await number('Верхний порог (percent)',80);
    await color('Нижний цвет 3','#445566');await number('Яркость по датчику',75);
    await click('Применить режим по датчику');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.requested?.sensor_id==='amdgpu:2f61:utilization'&&!!d.applied&&!d.degraded&&!d.last_error;},'GPU source applied');
    await select('Датчик подсветки','fixture:cpu');
    if(field('Нижний порог (celsius)').value!=='45'||field('Нижний цвет 2').value!=='#112233'||field('Яркость по датчику').value!=='50'||field('Направление по датчику').value!=='reverse')throw new Error('CPU source draft lost');
    await click('Применить режим по датчику');
    await wait(async()=>(await rpc('get_dynamic_rgb',{dev})).requested?.sensor_id==='fixture:cpu','CPU selection retained');
    await select('Зона','pump');await select('Эффект','static');await click('Применить подсветку');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.base_mode==='static'&&!d.requested;},'manual mode cancels active policy only');
    await wait(()=>[...document.querySelectorAll('p')].some(e=>e.textContent.includes('Режим помпы: static')),'static metadata');
    await click('Загрузить настройки режима по датчику');await select('Датчик подсветки','hwmon:2f64:nct6775:fan1');
    await number('Нижний порог (rpm)',900);await number('Верхний порог (rpm)',1600);await color('Нижний цвет 1','#778899');
    await click('Применить режим по датчику');await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.requested?.unit==='rpm'&&!!d.applied&&!d.degraded&&!d.last_error;},'RPM profile applied');
    await select('Эффект','meteor');
    await wait(()=>field('Управление подсветкой')?.value==='true','saved dynamic mode selected');
    await click('Применить подсветку');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.base_mode==='meteor'&&d.requested?.sensor_id==='fixture:cpu'&&!!d.applied&&!d.degraded&&!d.last_error;},'Meteor source automatically restored');
    await select('Эффект','static');await click('Применить подсветку');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.base_mode==='static'&&d.requested?.unit==='rpm'&&!!d.applied&&!d.degraded&&!d.last_error;},'Static RPM mode automatically restored');
    await select('Эффект','meteor');await click('Применить подсветку');
    await wait(()=>[...document.querySelectorAll('p')].some(e=>e.textContent.includes('Режим помпы: meteor')),'meteor metadata');
    await click('Загрузить настройки режима по датчику');
    await wait(()=>field('Датчик подсветки').value==='fixture:cpu'&&field('Нижний порог (celsius)')?.value==='45','stored Meteor source restored');
    if(field('Нижний цвет 2').value!=='#112233'||field('Яркость по датчику').value!=='50')throw new Error('per-mode source profile lost');
    await click('Применить режим по датчику');await wait(async()=>!!(await rpc('get_dynamic_rgb',{dev})).requested,'restored source enabled');
    await click('Отключить режим по датчику');await wait(async()=>!(await rpc('get_dynamic_rgb',{dev})).requested,'disable retains profiles');
    await wait(()=>field('Управление подсветкой')?.value==='false','disable updates manual editor');
    await select('Эффект','static');await click('Применить подсветку');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.base_mode==='static'&&d.requested?.unit==='rpm';},'inactive dynamic flag retained');
    await select('Эффект','meteor');await click('Применить подсветку');
    await wait(async()=>{const d=await rpc('get_dynamic_rgb',{dev});return d.base_mode==='meteor'&&!d.requested;},'disabled Meteor does not resurrect');
    const dynamic_profiles=(await rpc('get_dynamic_rgb',{dev})).profiles;
    if(dynamic_profiles.modes.length!==2||dynamic_profiles.modes.find(m=>m.mode==='meteor').bindings.length!==2)throw new Error('incomplete source profile graph');
    await invoke('native_smoke_result',{result:{ok:true,dev,pump_palette:true,fan_palette:true,native_base_load:true,native_mode_catalog:true,native_mode_drafts:true,native_mode_defaults:true,applied_mode_profiles:true,native_fan_topology:true,native_dynamic_profiles:true,native_dynamic_selection:true,dynamic_profiles,selected_rgb:[[1,2,3],[4,5,6],[7,8,9],[10,11,12]]}});
  }catch(error){await invoke('native_smoke_result',{result:{ok:false,error:String(error),stack:error?.stack}});}
})();
