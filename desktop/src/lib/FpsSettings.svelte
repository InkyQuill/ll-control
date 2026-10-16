<script lang="ts">
 import {t} from './i18n.svelte';
 import {rpc,message,type FpsSourceSettings} from './api';
 let {connected}:{connected:boolean}=$props();
 let current=$state<FpsSourceSettings|null>(null),draft=$state<FpsSourceSettings|null>(null);
 let fresh=$state(false),busy=$state(false),error=$state(''),writeError=$state(''),notice=$state(''),mustReload=$state(false);
 let generation=0,request=0;
 const conflict=$derived(!!current&&!!draft&&current.revision!==draft.revision);
 function valid(value:unknown):value is FpsSourceSettings {
  if(!value||typeof value!=='object')return false;
  const s=value as Partial<FpsSourceSettings>;
  return s.version===1&&typeof s.revision==='number'&&Number.isSafeInteger(s.revision)&&s.revision>=0&&s.revision<=9007199254740990&&typeof s.enabled==='boolean'
   &&(s.log_path===null||typeof s.log_path==='string'&&s.log_path.startsWith('/')&&new TextEncoder().encode(s.log_path).length<=384&&!/[\u0000-\u001f\u007f]/.test(s.log_path))&&(!s.enabled||s.log_path!==null);
 }
 const validDraft=$derived(!!draft&&valid(draft));
 $effect(()=>{
  const online=connected,epoch=++generation;fresh=false;
  if(!online)return;
  let cancelled=false,polling=false;
  async function poll(){
   if(polling||busy)return;polling=true;const reading=++request;
   try {const saved=await rpc<FpsSourceSettings>('get_fps_source');if(cancelled||epoch!==generation||reading!==request)return;
    if(!valid(saved))throw new Error('Неподдерживаемый ответ настроек FPS');current=saved;fresh=true;error='';if(!draft)draft={...saved};}
   catch(e){if(!cancelled&&epoch===generation&&reading===request){fresh=false;error=message(e);}}
   finally{polling=false;}
  }
  const first=setTimeout(()=>void poll(),0),timer=setInterval(()=>void poll(),1500);
  return()=>{cancelled=true;generation++;request++;clearTimeout(first);clearInterval(timer);};
 });
 async function save(){
  if(!draft||!fresh||busy||conflict||mustReload||!validDraft)return;
  const epoch=generation,settings={...draft};request++;busy=true;writeError='';notice='';
  try {const saved=await rpc<FpsSourceSettings>('set_fps_source',{settings});if(epoch!==generation)return;
   if(!valid(saved)||saved.revision!==settings.revision+1||saved.enabled!==settings.enabled||saved.log_path!==settings.log_path)throw new Error('Неподдерживаемый ответ настроек FPS');current=saved;draft={...saved};fresh=true;notice='Настройки FPS сохранены.';}
  catch(e){if(epoch===generation){fresh=false;mustReload=true;writeError=message(e);}}
  finally{busy=false;}
 }
 function reload(){if(!current||!fresh||busy)return;draft={...current};mustReload=false;writeError='';notice='';}
</script>
<section aria-label={t('Источник FPS')}>
 <h2>{t('Источник FPS')}</h2>
 <p>{t('Используйте существующий лог MangoHud с log_interval=0. Включение читает новые кадры из выбранного файла.')}</p>
 {#if draft}
  <label><input type="checkbox" bind:checked={draft.enabled} disabled={busy}/>{t('Читать FPS')}</label>
  <label>{t('Файл лога MangoHud')}<input type="text" value={draft.log_path??''} oninput={event=>{if(draft)draft.log_path=event.currentTarget.value||null;}} placeholder="/home/user/MangoHud/game.csv" disabled={busy} spellcheck={false}/></label>
  {#if !validDraft}<p role="alert">{t('Укажите абсолютный путь к логу MangoHud (до 384 байт).')}</p>{/if}
 {/if}
 {#if !connected}<p role="status">{t('Подключите daemon для настройки FPS.')}</p>{/if}
 {#if error}<p role={draft?'alert':'status'}>{t(error)}</p>{/if}
 {#if writeError}<p role="alert">{t(writeError)}</p>{/if}
 {#if mustReload}<p role="alert">{t('Сохранение FPS не подтверждено. Загрузите сохранённое состояние перед повторной записью.')}</p>{/if}
 {#if conflict}<p role="alert">{t('Настройки FPS изменились в другом окне. Загрузите сохранённое состояние.')}</p>{/if}
 <div class="actions"><button disabled={!fresh||busy||!current} onclick={reload}>{t('Загрузить настройки FPS')}</button><button disabled={!connected||!fresh||busy||!validDraft||conflict||mustReload} onclick={save}>{t('Сохранить настройки FPS')}</button></div>
 {#if notice}<p role="status">{t(notice)}</p>{/if}
</section>
<style>section{display:grid;gap:15px;max-width:760px;margin-bottom:30px;border-bottom:1px solid #35465b;padding-bottom:24px}.actions{display:flex;gap:10px;flex-wrap:wrap}label{display:grid;gap:8px}input[type=text]{max-width:620px}p{line-height:1.6;color:#b6c5d8}[role=alert]{color:#ffb8c5}[role=status]{color:#8adbc0}</style>
