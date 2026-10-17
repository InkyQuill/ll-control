// Native-smoke-only inspection of a private MockBackend document. No device Apply.
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  const wait = async check => { for (let i=0;i<600;i++) { if(check())return;await delay(50); }throw new Error('editor view timeout'); };
  const en = window.__nativeDesktopLanguage === 'en-US';
  const click = async (english,russian) => {
    const name=en?english:russian;
    await wait(()=>[...document.querySelectorAll('button')].some(n=>n.textContent.trim()===name&&!n.disabled));
    [...document.querySelectorAll('button')].find(n=>n.textContent.trim()===name&&!n.disabled).click();
  };
  try {
    await click('Pump screen','Экран помпы');await click('Composition','Композиция');
    await wait(()=>document.querySelector('.visual details'));
    const load=[...document.querySelectorAll('summary')].find(n=>n.textContent.trim()===(en?'Load or stop':'Загрузить или остановить'));
    load.parentElement.open=true;
    await click('Load active theme','Загрузить активную тему');
    await wait(()=>document.querySelectorAll('.layer-row[data-layer-index]').length>=7);
    document.querySelector('.layer-row[data-layer-index="1"]').click();
    document.querySelectorAll('.visual details').forEach(n=>n.open=false);
    await wait(()=>document.querySelector('.canvas img')&&!document.querySelector('.canvas .pending'));
    document.activeElement?.blur();window.scrollTo(0,0);
    const frames=[];const started=performance.now();
    const observer=new MutationObserver(changes=>{if(changes.some(c=>c.attributeName==='src'))frames.push(performance.now());});
    observer.observe(document.querySelector('.canvas'),{subtree:true,attributes:true,attributeFilter:['src']});
    await delay(10000);observer.disconnect();
    const apply=[...document.querySelectorAll('button')].find(n=>n.textContent.trim()===(en?'Apply to LCD':'Применить к LCD'));
    const rect=n=>{const r=n.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom};};
    const result={ok:true,viewport:{width:innerWidth,height:innerHeight,scale:devicePixelRatio},frames:frames.length,fps:frames.length/((performance.now()-started)/1000),layers:document.querySelectorAll('.layer-row[data-layer-index]').length,apply:rect(apply),inspector:rect(document.querySelector('.inspector')),canvas:rect(document.querySelector('.canvas')),horizontalOverflow:document.documentElement.scrollWidth>innerWidth};
    await invoke('native_smoke_result',{keepOpen:true,result});
    await delay(3000);await click('Layers','Слои');await delay(300);
    await invoke('native_smoke_result',{keepOpen:true,result:{...result,pane:'layers'}});
  } catch(error) { await invoke('native_smoke_result',{result:{ok:false,error:String(error)}}); }
})();
