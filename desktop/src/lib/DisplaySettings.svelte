<script lang="ts">
  import {t} from './i18n.svelte';
 import {rpc,message,type DisplaySettings} from './api';
 import {presentation,publishDisplaySettings} from './presentation.svelte';
 let {connected}:{connected:boolean}=$props();
 let draft=$state<DisplaySettings|null>(null),fresh=$state(false),busy=$state(false),error=$state(''),writeError=$state(''),notice=$state('');
 let generation=0,request=0;
 const conflict=$derived(!!draft&&!!presentation.settings&&draft.revision!==presentation.settings.revision);
 function valid(value:DisplaySettings|null):value is DisplaySettings{return !!value&&value.version===1&&Number.isSafeInteger(value.revision)&&value.revision>=0&&['celsius','fahrenheit'].includes(value.temperature_unit)&&(!value.interface_language||['ru-RU','en-US'].includes(value.interface_language));}
 $effect(()=>{
  const online=connected,epoch=++generation;fresh=false;
  if(!online)return;
  let cancelled=false,polling=false;
  async function poll(){
   if(busy||polling)return;polling=true;const reading=++request;
   try{const next=await rpc<DisplaySettings>('get_display_settings');if(cancelled||epoch!==generation||reading!==request)return;if(!valid(next))throw new Error('Daemon не вернул поддерживаемые настройки единиц');publishDisplaySettings(next);fresh=true;error='';if(!draft)draft={...next};}
   catch(e){if(!cancelled&&epoch===generation&&reading===request){fresh=false;error=message(e);}}
   finally{polling=false;}
  }
  const first=setTimeout(()=>void poll(),0),timer=setInterval(()=>void poll(),1500);
  return()=>{cancelled=true;generation++;request++;clearTimeout(first);clearInterval(timer);};
 });
 async function save(){
  if(!draft||busy||!fresh||conflict)return;const epoch=generation;const settings={...draft};request++;busy=true;writeError='';notice='';
  try{const saved=await rpc<DisplaySettings>('set_display_settings',{settings});if(epoch!==generation)return;if(!valid(saved))throw new Error('Неподдерживаемый ответ сохранения');publishDisplaySettings(saved);draft={...saved};fresh=true;notice='Параметры отображения сохранены. Исходные показания и пороги управления остаются в °C.';}
  catch(e){if(epoch===generation){fresh=false;writeError=`${message(e)}. Проверьте сохранённое состояние перед повторной записью.`;}}
  finally{busy=false;}
 }
</script>
<section aria-label="{t("Единицы и язык")}">
 <h2>{t("Единицы и язык")}</h2>
 <p>{t("Для показаний, температурных полей и LCD-слоёв с выбором «Общие настройки». Явные °C/°F в сохранённых LCD-темах сохраняются.")}</p>
 {#if draft}<label>{t("Единицы температуры")}<select bind:value={draft.temperature_unit} disabled={!connected||busy}><option value="celsius">{t("Цельсий (°C)")}</option><option value="fahrenheit">{t("Фаренгейт (°F)")}</option></select></label>{/if}
 {#if draft}<label>{t("Язык интерфейса")}<select value={draft.interface_language??'ru-RU'} onchange={event=>{if(draft)draft.interface_language=event.currentTarget.value as 'ru-RU'|'en-US';}} disabled={!connected||busy}><option value="ru-RU">{t("Русский")}</option><option value="en-US">English (partial)</option></select></label>{/if}
 {#if !connected}<p role="status">{t("Подключите daemon для загрузки и сохранения единиц.")}</p>{/if}
 {#if error}<p role="alert">{t(error)}</p>{/if}
 {#if writeError}<p role="alert">{t(writeError)}</p>{/if}
 {#if conflict}<p role="alert">{t("Параметры отображения изменились в другом окне. Загрузите сохранённое состояние.")}</p>{/if}
 <div class="actions"><button disabled={!fresh||busy||!presentation.settings} onclick={()=>{if(presentation.settings){draft={...presentation.settings};notice='';writeError='';}}}>{t("Загрузить сохранённые параметры")}</button><button disabled={!connected||!fresh||busy||!draft||conflict} onclick={save}>{t("Сохранить параметры отображения")}</button></div>
 {#if notice}<p role="status">{t(notice)}</p>{/if}
</section>
<style>section{max-width:760px;display:grid;gap:15px;margin-bottom:30px;border-bottom:1px solid #35465b;padding-bottom:24px}.actions{display:flex;flex-wrap:wrap;gap:10px}p{line-height:1.6;color:#b6c5d8}[role=alert]{color:#ffb8c5}[role=status]{color:#8adbc0}label{max-width:400px}</style>
