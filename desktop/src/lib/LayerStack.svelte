<script lang="ts">
  import {t} from './i18n.svelte';
  import type {Layer} from './api';
  import {isProcedural} from './lcd-pages';
  let {layers,labels,selected,onselect,onmove,onduplicate,onremove,onvisibility,onlock}: {
    layers:Layer[];labels:Record<Layer['kind'],string>;selected:number;
    onselect:(index:number)=>void;onmove:(index:number,target:number)=>void;
    onduplicate:(index:number)=>void;onremove:(index:number)=>void;onvisibility:(index:number)=>void;onlock:(index:number)=>void;
  }=$props();
  let dragging=$state<number|null>(null);
  function title(layer:Layer):string {
    return layer.editor?.name || (layer.kind==='text'?layer.text:layer.kind==='sensor'?layer.label:labels[layer.kind]);
  }
</script>
<section class="stack" aria-label={t('Слои')}>
  <header><h2>{t('Слои')}</h2><span>{layers.length} / 32</span></header>
  <div class="rows">
    {#each layers.map((layer,index)=>({layer,index})).reverse() as item (item.layer)}
      <div class="layer-entry"><button class="layer-row" data-layer-index={item.index} class:selected={selected===item.index}
        aria-pressed={selected===item.index} title={title(item.layer)}
        draggable={!isProcedural(item.layer)}
        ondragstart={event=>{dragging=item.index;event.dataTransfer?.setData('text/plain',String(item.index));}}
        ondragend={()=>dragging=null}
        ondragover={event=>{if(dragging!==null&&!isProcedural(item.layer))event.preventDefault();}}
        ondrop={event=>{event.preventDefault();if(dragging!==null)onmove(dragging,item.index);dragging=null;}}
        onclick={()=>onselect(item.index)}
        onkeydown={event=>{if(event.altKey && (event.key==='ArrowUp'||event.key==='ArrowDown')){event.preventDefault();onmove(item.index,item.index+(event.key==='ArrowUp'?1:-1));}}}>
        <span class="layer-kind">{labels[item.layer.kind]}</span><span class="name">{title(item.layer)}</span>
      </button>
      <button class="toggle" aria-label={t('Видимость слоя {0}',item.index+1)} aria-pressed={!item.layer.editor?.hidden} onclick={()=>onvisibility(item.index)} title={t('Видимость')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>{#if item.layer.editor?.hidden}<path d="m3 3 18 18"/>{/if}</svg>
      </button>
      <button class="toggle" aria-label={t('Блокировка слоя {0}',item.index+1)} aria-pressed={!!item.layer.editor?.locked} onclick={()=>onlock(item.index)} title={t('Блокировка')}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="2"/><path d={item.layer.editor?.locked?'M8 10V6a4 4 0 0 1 8 0v4':'M8 10V6a4 4 0 0 1 8 0'}/></svg>
      </button></div>
    {/each}
  </div>
  <button class="layer-row background-row" class:selected={selected===-1} aria-pressed={selected===-1} onclick={()=>onselect(-1)}>{t('Фон и композиция')}</button>
  <footer>
    <button disabled={selected<0||selected>=layers.length-1||isProcedural(layers[selected])} onclick={()=>onmove(selected,selected+1)}>{t('На передний план')}</button>
    <button disabled={selected<0||selected<=(isProcedural(layers[0])?1:0)} onclick={()=>onmove(selected,selected-1)}>{t('На задний план')}</button>
    <button disabled={selected<0||selected>=layers.length||layers.length>=32||isProcedural(layers[selected])} onclick={()=>onduplicate(selected)}>{t('Дублировать')}</button>
    <button disabled={selected<0} onclick={()=>onremove(selected)}>{t('Удалить слой')}</button>
  </footer>
</section>
<style>
  .stack{border-block:1px solid var(--border);padding-block:12px;margin-block:16px}
  header{display:flex;align-items:center;justify-content:space-between;margin-bottom:8px}h2{margin:0;font-size:15px}header>span{color:var(--text-muted);font-size:12px;font-variant-numeric:tabular-nums}
  .rows{display:grid;gap:2px;max-height:280px;overflow:auto}
  .layer-entry{display:flex;gap:2px}.layer-entry>.layer-row{flex:1;min-width:0}.toggle{padding:6px;min-height:30px;border-color:transparent;background:transparent}.toggle[aria-pressed=true]{color:var(--accent-text)}svg{width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:1.5}
  .layer-row{display:flex;align-items:center;gap:12px;text-align:left;padding:7px 10px;min-height:34px;border-color:transparent;border-radius:6px;background:transparent}
  .background-row{width:100%;margin-top:4px}
  .layer-row.selected{background:var(--accent-surface);border-color:var(--accent);color:var(--accent-text)}
  .layer-kind{font-size:11px;color:var(--text-muted);min-width:68px}.name{overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
  footer{display:flex;flex-wrap:wrap;gap:6px;padding-top:10px}footer button{padding:5px 8px;font-size:12px;min-height:32px}
</style>
