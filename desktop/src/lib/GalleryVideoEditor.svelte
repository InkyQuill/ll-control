<script lang="ts">
  import {t} from './i18n.svelte';
  import {onMount} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import MediaImport from './MediaImport.svelte';
  import {message} from './api';
  import type {SourceMetadata} from './video-range';
  let {clipKey,name,device,saving=false,onclose,onsave}:{saving?:boolean;clipKey:string;name:string;device:string;onclose:()=>void;onsave:(key:string)=>void}=$props();
  type Source={id:string;key:string;phase:string;message:string|null;metadata?:SourceMetadata|null};
  let source=$state<Source|null>(null),error=$state(''),busy=$state(false),sourceUrl=$state('');
  let heading=$state<HTMLHeadingElement>();
  let mounted=false;
  const id=crypto.randomUUID().replaceAll('-','');
  onMount(()=>{
    mounted=true;heading?.focus();let timer:ReturnType<typeof setTimeout>|undefined;
    async function poll() {
      try {
        const next=await invoke<Source|null>('media_source_status');
        if(mounted && next?.id===id){source=next;error='';}
      }catch(e){if(mounted)error=message(e);}
      if(mounted)timer=setTimeout(poll,400);
    }
    void invoke<string>('media_playback_url',{id,kind:'source'}).then(async url=>{
      if(!mounted)return;
      sourceUrl=url;
      const next=await invoke<Source>('media_source_start',{id,key:clipKey});
      if(mounted)source=next;
    }).catch(e=>{if(mounted)error=message(e);}).finally(()=>{
      if(mounted)void poll();else void invoke('media_source_cancel',{id}).catch(()=>{});
    });
    return ()=>{mounted=false;clearTimeout(timer);void invoke('media_source_cancel',{id}).catch(()=>{});};
  });
</script>
<section aria-label="{t("Редактор сохранённого ролика")}">
  <h3 bind:this={heading} tabindex="-1">{t("Редактирование «")}{name}»</h3>
  {#if source?.phase==='ready'}
    <fieldset disabled={saving} aria-label="{t("Изменения ролика")}"><MediaImport {device} existing={{id,name,url:sourceUrl,metadata:source.metadata}} onbusy={value=>busy=value} onimport={(key,target)=>{if(key && target===device)onsave(key);}}/></fieldset>
  {:else}<p role="status">{source?.phase==='failed' ? "" + t("Исходник не загружен.") + "" : source?.phase==='cancelled' ? "" + t("Загрузка отменена.") + "" : "" + t("Подготовка исходника для редактирования…") + ""}</p>{/if}
  {#if error || source?.message}<p role="alert">{error || source?.message}</p>{/if}
  <button disabled={busy || saving} onclick={onclose}>{t("Закрыть редактор ролика")}</button>
</section>
