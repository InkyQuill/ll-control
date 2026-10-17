<script lang="ts">
  import {t} from './i18n.svelte';
  import { untrack } from 'svelte';
  import MediaThumbnail from './MediaThumbnail.svelte';
  import type { PreparedClipInfo } from './api';
  let { clips, choice, disabled = false, onchoose }: {
    clips: PreparedClipInfo[]; choice: string; disabled?: boolean; onchoose: (key: string) => void;
  } = $props();
  const pageSize = 8;
  let page = $state(0);
  const pages = $derived(Math.max(1, Math.ceil(clips.length / pageSize)));
  const visiblePage = $derived(Math.min(page, pages - 1));
  const visible = $derived(clips.slice(visiblePage * pageSize, (visiblePage + 1) * pageSize));
  // Follow explicit external selection, but a routine metadata poll must not
  // undo the page the user is browsing.
  $effect(() => {
    const key = choice;
    untrack(() => { const index = clips.findIndex(clip => clip.key === key); if (index >= 0) page = Math.floor(index / pageSize); });
  });
</script>
<fieldset disabled={disabled}>
  <legend>{t("Карточки роликов")}</legend>
  <p>{t("Выбор карточки не запускает LCD. Примените выбранный ролик отдельной кнопкой.")}</p>
  <div class="cards">
    {#each visible as clip (clip.key)}
      <article class:selected={choice === clip.key}>
        <label><input type="radio" name="media-gallery-choice" checked={choice === clip.key} onchange={() => onchoose(clip.key)}/><span>{clip.name}</span></label>
        <MediaThumbnail clipKey={clip.key} name={clip.name} compact/>
        {#if clip.is_default}<p>{t("Встроенный ролик")}</p>{/if}
        <p>{(clip.frames / clip.fps).toFixed(1)} {t("с ·")} {clip.fps} fps</p>
      </article>
    {/each}
  </div>
  <nav aria-label="{t("Страницы роликов")}">
    <button disabled={disabled || visiblePage === 0} onclick={() => page = visiblePage - 1}>{t("Предыдущие ролики")}</button>
    <span role="status">{t("Страница")} {visiblePage + 1} {t("из")} {pages}</span>
    <button disabled={disabled || visiblePage + 1 >= pages} onclick={() => page = visiblePage + 1}>{t("Следующие ролики")}</button>
  </nav>
</fieldset>
<style>
  fieldset { min-width:0; border:0; padding:0; margin:12px 0; }
  legend { font-weight:600; }
  .cards { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,150px),1fr)); gap:12px; }
  article { min-width:0; border:1px solid var(--border-strong); border-radius:10px; padding:10px; }
  article.selected { outline:2px solid var(--success); outline-offset:1px; }
  label { display:flex; align-items:start; gap:8px; cursor:pointer; overflow-wrap:anywhere; }
  input { flex:none; }
  p { overflow-wrap:anywhere; }
  nav { display:flex; flex-wrap:wrap; align-items:center; gap:10px; margin-top:16px; }
</style>
