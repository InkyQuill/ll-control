<script lang="ts">
  import {t} from './i18n.svelte';
  import { requestThumbnail } from './media-thumbnails';
  import { message } from './api';
  let { clipKey, name, compact = false }: { clipKey: string; name: string; compact?: boolean } = $props();
  let source = $state(''), error = $state(''), loading = $state(false), retry = $state(0);
  $effect(() => {
    const key = clipKey; void retry;
    const controller = new AbortController();
    let active = true, objectUrl = '';
    source = ''; error = ''; loading = true;
    async function load() {
      try {
        const bytes = await requestThumbnail(key, controller.signal);
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)], {type:'image/png'}));
        source = objectUrl;
      } catch (e) { if (active) error = message(e); }
      finally { if (active) loading = false; }
    }
    void load();
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      controller.abort();
    };
  });
</script>

<figure class:compact>
  {#if source}<img src={source} alt={`Первый кадр: ${name}`} width="144" height="144" />{/if}
  {#if loading}<p role="status">{t("Подготовка миниатюры…")}</p>{/if}
  {#if error}<p role="status">{t("Миниатюра недоступна:")} {t(error)}</p>{/if}
  {#if !compact || error}<button disabled={loading} onclick={() => retry++}>{compact ? t("Повторить миниатюру: {0}", name) : "" + t("Обновить миниатюру") + ""}</button>{/if}
  <figcaption>{t("Первый кадр ролика. Предпросмотр не запускает LCD.")}</figcaption>
</figure>
<style>
  figure { margin:12px 0; }
  .compact { margin:8px 0; }
  .compact figcaption { font-size:12px; }
  img { display:block; max-width:100%; height:auto; border-radius:8px; }
  figcaption, p { overflow-wrap:anywhere; margin:8px 0; }
</style>
