<script lang="ts">
  import {t} from './i18n.svelte';
  import { onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { message, rpc } from './api';
  type Transform = 'rotate_left' | 'rotate_right' | 'flip_horizontal' | 'flip_vertical';
  interface Prepared { width: number; height: number; png: number[] }
  let { disabled = false, existing, onpublish }: { existing?: {key:string; name:string}; disabled?: boolean; onpublish: (png: number[], name: string) => Promise<string> } = $props();
  let open = $state(false), loading = $state(false), saving = $state(false), attempted = $state(false);
  let error = $state(''), notice = $state(''), name = $state('');
  let source = $state.raw<number[]>([]), prepared = $state.raw<number[]>([]);
  let width = $state(0), height = $state(0), x = $state<number | undefined>(0), y = $state<number | undefined>(0), side = $state<number | undefined>(1);
  let crop = $state(true), transforms = $state<Transform[]>([]);
  type ColorKey = 'hue' | 'saturation' | 'brightness' | 'contrast' | 'sharpen' | 'blur';
  const colorControls: {key:ColorKey;label:string;min:number;max:number}[] = [
    {key:'hue',label:'Сдвиг тона',min:0,max:360},
    {key:'saturation',label:'Насыщенность',min:-25,max:75},
    {key:'brightness',label:'Яркость изображения',min:-100,max:100},
    {key:'contrast',label:'Контраст изображения',min:-100,max:100},
    {key:'sharpen',label:'Резкость',min:0,max:10},
    {key:'blur',label:'Размытие',min:0,max:10},
  ];
  let filters = $state<Record<ColorKey,number|undefined>>({hue:0,saturation:0,brightness:0,contrast:0,sharpen:0,blur:0});
  const filtersValid = $derived(colorControls.every(c => typeof filters[c.key] === 'number' && Number.isFinite(filters[c.key]) && filters[c.key]! >= c.min && filters[c.key]! <= c.max));
  let sourceUrl = $state(''), previewUrl = $state(''), preparedSignature = $state('');
  let trigger: HTMLButtonElement | undefined = $state();
  let generation = 0, dragging = false;
  let capturing = $state(false);
  let captureId: string | null = null;
  const valid = $derived(!crop || ([x,y,side].every(n => Number.isInteger(n)) && x! >= 0 && y! >= 0 && side! > 0 && x! + side! <= width && y! + side! <= height));
  const signature = $derived(JSON.stringify({crop: crop ? {x,y,side} : null, transforms, filters}));
  const current = $derived(prepared.length > 0 && signature === preparedSignature);
  function release() { if (sourceUrl) URL.revokeObjectURL(sourceUrl); if (previewUrl) URL.revokeObjectURL(previewUrl); sourceUrl = ''; previewUrl = ''; }
  onDestroy(() => {
    generation++; release();
    if(captureId)void invoke('cancel_screenshot',{id:captureId}).catch(()=>{});
  });
  async function cancelCapture() {
    const cancelledGeneration=++generation;
    const id=captureId;captureId=null;capturing=false;loading=false;
    notice='Снимок экрана отменён.';
    if(id)try {await invoke('cancel_screenshot',{id});}catch(e){if(generation===cancelledGeneration)error=message(e);}
  }
  async function close() {
    if (saving) return;
    const pending=capturing ? cancelCapture() : Promise.resolve();
    generation++; open = false; source = []; prepared = []; release();
    await pending; await tick(); if(!open)trigger?.focus();
  }
  function reset() { x = Math.floor((width - Math.min(width,height))/2); y = Math.floor((height - Math.min(width,height))/2); side = Math.min(width,height); crop = true; transforms = []; filters={hue:0,saturation:0,brightness:0,contrast:0,sharpen:0,blur:0}; }
  async function start() {
    if (disabled || open || saving) return;
    open = true; error = ''; notice = ''; attempted = false; loading = false;
    if (!existing) return;
    const selected = {...existing}, started = ++generation;
    loading = true;
    try {
      const bytes = await rpc<number[]>('read_theme_asset_for_edit', {key:selected.key});
      if (started !== generation) return;
      const info = await invoke<Prepared>('image_editor', {bytes,edit:null});
      if (started !== generation) return;
      source=bytes; width=info.width; height=info.height; name=selected.name; reset(); crop=false;
      sourceUrl=URL.createObjectURL(new Blob([new Uint8Array(info.png)],{type:'image/png'}));
    } catch(e) { if(started===generation) error=message(e); }
    finally { if(started===generation) loading=false; }
  }
  async function screenshot() {
    if (loading || saving || disabled) return;
    const started = ++generation;
    loading = true; capturing = true; error = ''; notice = '';
    let id: string | null = null;
    try {
      id = await invoke<string>('begin_screenshot');
      if (started !== generation) return;
      captureId=id;
      const result = await invoke<ArrayBuffer>('capture_screenshot',{id});
      if (started !== generation) return;
      if (!result.byteLength) { notice = 'Снимок экрана отменён.'; return; }
      const bytes = Array.from(new Uint8Array(result));
      const info = await invoke<Prepared>('image_editor', {bytes, edit:null});
      if (started !== generation) return;
      release(); source = bytes; prepared = []; attempted = false;
      width = info.width; height = info.height; name = t('Снимок экрана.png'); reset();
      sourceUrl = URL.createObjectURL(new Blob([new Uint8Array(info.png)], {type:'image/png'}));
    } catch (e) { if (started === generation) error = message(e); }
    finally {
      if(id)void invoke('cancel_screenshot',{id}).catch(()=>{});
      if (started === generation) {captureId=null;loading = false; capturing = false;}
    }
  }
  async function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = '';
    if (!file || loading || saving) return;
    const started = ++generation;
    release(); source = []; prepared = []; error = ''; notice = ''; attempted = false; loading = true;
    try {
      if (!file.size || file.size > 16*1024*1024) throw new Error('Выберите изображение до 16 MiB.');
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      if (started !== generation) return;
      const info = await invoke<Prepared>('image_editor', {bytes, edit:null});
      if (started !== generation) return;
      source = bytes; width = info.width; height = info.height; name = file.name; reset();
      sourceUrl = URL.createObjectURL(new Blob([new Uint8Array(info.png)],{type:'image/png'}));
    } catch (e) { if (started === generation) error = message(e); }
    finally { if (started === generation) loading = false; }
  }
  async function preview() {
    if (!valid || !filtersValid || !source.length || loading || saving) return;
    const started = generation, requestSignature = signature;
    loading = true; error = ''; prepared = [];
    try {
      const result = await invoke<Prepared>('image_editor', {bytes:source, edit:JSON.parse(requestSignature)});
      if (started !== generation) return;
      prepared = result.png; preparedSignature = requestSignature;
      if (previewUrl) URL.revokeObjectURL(previewUrl);
      previewUrl = URL.createObjectURL(new Blob([new Uint8Array(result.png)],{type:'image/png'}));
    } catch (e) { if (started === generation) error = message(e); }
    finally { if (started === generation) loading = false; }
  }
  async function save() {
    if (!current || !filtersValid || disabled || loading || saving || attempted) return;
    saving = true; attempted = true; error = '';
    try { notice = await onpublish(prepared,name); }
    catch (e) { error = `${message(e)}. Проверьте библиотеку и коллекции: автоматического повтора сохранения не будет.`; }
    finally { saving = false; }
  }
  function move(event: PointerEvent) {
    if (!dragging || !crop || !valid || loading || saving || side === undefined) return;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    x = Math.max(0,Math.min(width-side,Math.round((event.clientX-rect.left)/rect.width*width-side/2)));
    y = Math.max(0,Math.min(height-side,Math.round((event.clientY-rect.top)/rect.height*height-side/2)));
  }
  function transform(value: Transform) { if (transforms.length < 64) transforms.push(value); }
</script>
<button bind:this={trigger} disabled={disabled || saving || open} onclick={start}>{existing ? t("Редактировать {0}…", existing.name) : "" + t("Импорт с редактированием…") + ""}</button>
{#if open}
  <section aria-label="{t("Редактор изображения")}">
    <h3>{t("Редактор изображения")}</h3>
    <p>{t("Выберите квадратную область и преобразования. Исходный файл сохранится. На LCD ничего не отправляется до применения темы.")}</p>
    {#if existing}<p>{t("Результат заменит запись выбранной коллекции. Прежний файл останется доступен сохранённым темам.")}</p>{:else}<label>{t("Файл для редактирования")}<input type="file" accept=".png,.jpg,.jpeg,.webp,.bmp" disabled={loading || saving || disabled} onchange={choose}/></label>{/if}
    {#if !existing}
      <button disabled={loading || saving || disabled} onclick={screenshot}>{t("Снять экран…")}</button>
      {#if capturing}<p role="status">{t("Выберите область или экран в системном окне.")}</p><button onclick={cancelCapture}>{t("Отменить снимок")}</button>{/if}
    {/if}
    {#if source.length}
      <p>{name} · {width} × {height} px</p>
      <fieldset disabled={loading || saving || attempted}>
        <legend>{t("Область и преобразования")}</legend>
        <label><input type="checkbox" bind:checked={crop}/>{t("Обрезать квадрат")}</label>
        {#if crop}
          <label>Crop X<input type="number" min="0" max={width-1} step="1" bind:value={x}/></label>
          <label>Crop Y<input type="number" min="0" max={height-1} step="1" bind:value={y}/></label>
          <label>{t("Сторона crop")}<input type="number" min="1" max={Math.min(width,height)} step="1" bind:value={side}/></label>
        {:else}<p>{t("Всё изображение будет растянуто до 480 × 480.")}</p>{/if}
        <button class="source" aria-label="{t("Переместить область crop указателем; точные координаты доступны в полях выше")}" onpointerdown={e => {dragging=true; e.currentTarget.setPointerCapture(e.pointerId); move(e);}} onpointermove={move} onpointerup={() => dragging=false} onpointercancel={() => dragging=false}>
          <img src={sourceUrl} alt="{t("Исходное изображение для crop")}"/>
          {#if crop && valid}<span class="selection" style:left={`${x!/width*100}%`} style:top={`${y!/height*100}%`} style:width={`${side!/width*100}%`} style:height={`${side!/height*100}%`}></span>{/if}
        </button>
        <div class="actions">
          <button disabled={transforms.length >= 64} onclick={() => transform('rotate_left')}>{t("Повернуть влево")}</button>
          <button disabled={transforms.length >= 64} onclick={() => transform('rotate_right')}>{t("Повернуть вправо")}</button>
          <button disabled={transforms.length >= 64} onclick={() => transform('flip_horizontal')}>{t("Отразить по горизонтали")}</button>
          <button disabled={transforms.length >= 64} onclick={() => transform('flip_vertical')}>{t("Отразить по вертикали")}</button>
          <button disabled={!transforms.length} onclick={() => transforms = transforms.slice(0,-1)}>{t("Отменить преобразование")}</button>
          <button onclick={reset}>{t("Сбросить правки")}</button>
        </div>
        <p>{t("Преобразований:")} {transforms.length}</p>
        <fieldset><legend>{t("Фильтры изображения")}</legend>
          {#each colorControls as control (control.key)}
            <label>{t(control.label)}<input type="number" min={control.min} max={control.max} step="any" bind:value={filters[control.key]}/></label>
            <label>{t("Ползунок:")} {t(control.label)}<input type="range" min={control.min} max={control.max} step="1" bind:value={filters[control.key]}/></label>
          {/each}
        </fieldset>
      </fieldset>
      {#if !valid}<p role="alert">{t("Область должна быть целой, непустой и находиться внутри изображения.")}</p>{/if}
      {#if !filtersValid}<p role="alert">{t("Заполните параметры фильтров в указанных диапазонах.")}</p>{/if}
      <button disabled={!valid || !filtersValid || loading || saving || attempted} onclick={preview}>{t("Подготовить preview изображения")}</button>
      {#if current}<img class="preview" src={previewUrl} alt="{t("Результат редактирования 480 на 480")}" width="240" height="240"/>{/if}
      <button disabled={!current || !filtersValid || disabled || loading || saving || attempted} onclick={save}>{t("Сохранить изображение в галерею")}</button>
    {/if}
    {#if loading}<p role="status">{t("Подготовка изображения…")}</p>{/if}
    {#if saving}<p role="status">{t("Сохранение изображения…")}</p>{/if}
    {#if notice}<p role="status">{t(notice)}</p>{/if}
    {#if error}<p role="alert">{t(error)}</p>{/if}
    <button disabled={saving} onclick={close}>{t("Закрыть редактор изображения")}</button>
  </section>
{/if}
<style>
  section { margin:16px 0; padding:16px; border:1px solid #64748b; border-radius:12px; }
  fieldset { min-width:0; }
  .source { display:block; position:relative; width:min(100%,480px); padding:0; border:0; touch-action:none; margin:12px 0; }
  .source img { display:block; width:100%; height:auto; }
  .selection { position:absolute; border:2px solid white; outline:2px solid black; box-sizing:border-box; pointer-events:none; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
  .preview { display:block; max-width:100%; height:auto; margin:12px 0; }
  p { overflow-wrap:anywhere; }
</style>
