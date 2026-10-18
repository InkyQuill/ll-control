<script lang="ts">
  import StudioIcon from './StudioIcon.svelte';
  import StudioPopover from "./StudioPopover.svelte";
  import { ValidationError, type UiDiagnostic } from './validation-error';
  import {t, diagnosticText} from './i18n.svelte';
  import { onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { message, rpc } from './api';
  import { ImportedImageMembershipError } from './collection-membership';
  type Transform = 'rotate_left' | 'rotate_right' | 'flip_horizontal' | 'flip_vertical';
  interface Prepared { width: number; height: number; png: number[] }
  let { disabled = false, existing, onpublish }: { existing?: {key:string; name:string}; disabled?: boolean; onpublish: (png: number[], name: string) => Promise<string> } = $props();
  let open = $state(false), loading = $state(false), saving = $state(false), attempted = $state(false);
  type Failure = {context:'source'|'capture'|'cancel-capture'|'preview'|'save'|'membership';detail:UiDiagnostic};
  let error = $state<string|Failure>(''), notice = $state(''), name = $state('');
  function errorText(value:string|Failure):string {
    if(typeof value==='string')return t(value);
    switch(value.context){
      case 'source':return t('Не удалось открыть изображение: {0}',diagnosticText(value.detail));
      case 'capture':return t('Не удалось получить снимок экрана: {0}',diagnosticText(value.detail));
      case 'cancel-capture':return t('Отмена снимка экрана не подтверждена: {0}',diagnosticText(value.detail));
      case 'preview':return t('Не удалось подготовить preview изображения: {0}',diagnosticText(value.detail));
      case 'save':return t('Сохранение изображения не подтверждено: {0}. Проверьте библиотеку и коллекции: автоматического повтора сохранения не будет.',diagnosticText(value.detail));
      case 'membership':return t('Файл сохранён в каталоге, но добавление в коллекцию не подтверждено: {0}. Проверьте библиотеку и коллекции: автоматического повтора сохранения не будет.',diagnosticText(value.detail));
    }
  }
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
  let panel: HTMLElement | undefined = $state();
  $effect(()=>{
    const node=panel;if(!node)return;
    const ancestors:HTMLElement[]=[];
    for(let parent=node.parentElement;parent;parent=parent.parentElement)ancestors.push(parent);
    const sync=()=>{if(ancestors.some(parent=>parent.hidden))node.hidePopover();else node.showPopover();};
    sync();
    const observer=new MutationObserver(sync);
    for(const parent of ancestors)observer.observe(parent,{attributes:true,attributeFilter:['hidden']});
    return ()=>observer.disconnect();
  });
  let generation = 0, dragging = false;
  let capturing = $state(false), cancelling = $state(false), cancelRequested = $state(false), captureOwnerGone = $state(false);
  let captureId: string | null = null;
  let reservation: Promise<string> | null = null;
  let cancelFlight: Promise<boolean> | null = null;
  const valid = $derived(!crop || ([x,y,side].every(n => Number.isInteger(n)) && x! >= 0 && y! >= 0 && side! > 0 && x! + side! <= width && y! + side! <= height));
  const signature = $derived(JSON.stringify({crop: crop ? {x,y,side} : null, transforms, filters}));
  const current = $derived(prepared.length > 0 && signature === preparedSignature);
  function release() { if (sourceUrl) URL.revokeObjectURL(sourceUrl); if (previewUrl) URL.revokeObjectURL(previewUrl); sourceUrl = ''; previewUrl = ''; }
  onDestroy(() => {
    generation++; cancelRequested=true; release();
    if(captureId || reservation)void releaseCapture(captureId);
  });
  function releaseCapture(id: string|null):Promise<boolean> {
    if(cancelFlight)return cancelFlight;
    const pending=reservation;
    cancelling=true;
    cancelFlight=Promise.resolve().then(async()=>{
      const owned=id ?? (pending ? await pending.catch(()=>null) : null);
      if(owned)captureId=owned;
      try {
        const outcome=owned ? await invoke<unknown>('cancel_screenshot',{id:owned}) : 'closed';
        if(outcome!=='closed' && outcome!=='owner_gone')throw new Error('Unsupported screenshot cancellation receipt');
        captureOwnerGone=outcome==='owner_gone';
        if(!owned || captureId===owned){captureId=null;capturing=false;loading=false;}
        return true;
      } catch(e) {
        capturing=true;loading=true;cancelRequested=true;
        error={context:'cancel-capture',detail:e instanceof ValidationError ? e : message(e)};
        return false;
      }
    }).finally(()=>{cancelling=false;cancelFlight=null;});
    return cancelFlight;
  }
  async function cancelCapture() {
    generation++;cancelRequested=true;notice='';error='';
    const released=await releaseCapture(captureId);
    if(released)notice=captureOwnerGone?'Портал отключился. Предыдущий запрос снимка завершён.':'Снимок экрана отменён.';
    return released;
  }
  async function close() {
    if (saving || cancelling) return;
    if((capturing || captureId || reservation) && !await cancelCapture())return;
    generation++; open = false; source = []; prepared = []; release();
    await tick(); if(!open)trigger?.focus();
  }
  function reset() { x = Math.floor((width - Math.min(width,height))/2); y = Math.floor((height - Math.min(width,height))/2); side = Math.min(width,height); crop = true; transforms = []; filters={hue:0,saturation:0,brightness:0,contrast:0,sharpen:0,blur:0}; }
  async function start() {
    if (disabled || saving) return;
    if(open){panel?.showPopover();return;}
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
    } catch(e) { if(started===generation) error={context:'source',detail:e instanceof ValidationError ? e : message(e)}; }
    finally { if(started===generation) loading=false; }
  }
  async function screenshot() {
    if (loading || saving || disabled || capturing || cancelling || captureId || reservation) return;
    const started = ++generation;
    loading = true; capturing = true; cancelRequested=false; captureOwnerGone=false; error = ''; notice = '';
    let id: string | null = null;
    const pending=invoke<string>('begin_screenshot');reservation=pending;
    try {
      id = await pending;
      if(reservation===pending)reservation=null;
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
    } catch (e) {
      if (started === generation) {
        error = {context:'capture',detail:e instanceof ValidationError ? e : message(e)};
        if(!id){
          // Reclaim only quarantined cleanup, never another editor's active capture.
          const cleanup=await invoke<string|null>('pending_screenshot_cleanup').catch(()=>null);
          if(started===generation && typeof cleanup==='string' && cleanup){
            captureId=cleanup;cancelRequested=true;
            error={context:'cancel-capture',detail:e instanceof ValidationError ? e : message(e)};
          }
        }
      }
    }
    finally {
      if(reservation===pending)reservation=null;
      if(id && started===generation && !cancelRequested)await releaseCapture(id);
      if(started===generation && !captureId){loading=false;capturing=false;}
    }
  }
  async function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement, file = input.files?.[0]; input.value = '';
    if (!file || loading || saving) return;
    const started = ++generation;
    release(); source = []; prepared = []; error = ''; notice = ''; attempted = false; loading = true;
    try {
      if (!file.size || file.size > 16*1024*1024) { error='Выберите изображение до 16 MiB.'; return; }
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      if (started !== generation) return;
      const info = await invoke<Prepared>('image_editor', {bytes, edit:null});
      if (started !== generation) return;
      source = bytes; width = info.width; height = info.height; name = file.name; reset();
      sourceUrl = URL.createObjectURL(new Blob([new Uint8Array(info.png)],{type:'image/png'}));
    } catch (e) { if (started === generation) error = {context:'source',detail:e instanceof ValidationError ? e : message(e)}; }
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
    } catch (e) { if (started === generation) error = {context:'preview',detail:e instanceof ValidationError ? e : message(e)}; }
    finally { if (started === generation) loading = false; }
  }
  async function save() {
    if (!current || !filtersValid || disabled || loading || saving || attempted) return;
    saving = true; attempted = true; error = '';
    try { notice = await onpublish(prepared,name); }
    catch (e) { error = {context:e instanceof ImportedImageMembershipError?'membership':'save',detail:e instanceof ImportedImageMembershipError ? e.detail : e instanceof ValidationError ? e : message(e)}; }
    finally { saving = false; }
  }
  function move(event: PointerEvent) {
    // A native backend can omit pointerup when release happens outside the area.
    // Never continue a crop drag once the primary button is no longer held.
    const area = event.currentTarget;
    if (!(area instanceof HTMLElement)) return;
    const captured = area.hasPointerCapture(event.pointerId);
    if ((event.buttons & 1) === 0 || !captured) {
      dragging = false;
      if (captured) area.releasePointerCapture(event.pointerId);
      return;
    }
    if (!dragging || !crop || !valid || loading || saving || side === undefined) return;
    const rect = area.getBoundingClientRect();
    x = Math.max(0,Math.min(width-side,Math.round((event.clientX-rect.left)/rect.width*width-side/2)));
    y = Math.max(0,Math.min(height-side,Math.round((event.clientY-rect.top)/rect.height*height-side/2)));
  }
  function transform(value: Transform) { if (transforms.length < 64) transforms.push(value); }
</script>
<button bind:this={trigger} disabled={disabled || saving} onclick={start}>{existing ? t("Редактировать {0}…", existing.name) : "" + t("Импорт с редактированием…") + ""}</button>
{#if open}
  <section bind:this={panel} popover="auto" aria-label="{t("Редактор изображения")}">
    <header><h3>{t("Редактор изображения")}</h3><button class="icon-action" aria-label={t('Закрыть редактор изображения')} title={t('Закрыть редактор изображения')} disabled={saving || cancelling} onclick={close}><StudioIcon name="close"/></button></header>
    <StudioPopover label={t("Справка")} icon="help"><p>{t("Выберите квадратную область и преобразования. Исходный файл сохранится. На LCD ничего не отправляется до применения темы.")}</p></StudioPopover>
    {#if existing}<StudioPopover label={t("Справка")} icon="help"><p>{t("Результат заменит запись выбранной коллекции. Прежний файл останется доступен сохранённым темам.")}</p></StudioPopover>{:else}<label>{t("Файл для редактирования")}<input type="file" accept=".png,.jpg,.jpeg,.webp,.bmp" disabled={loading || saving || disabled} onchange={choose}/></label>{/if}
    {#if !existing}
      <button disabled={loading || saving || disabled} onclick={screenshot}>{t("Снять экран…")}</button>
      {#if capturing}<p role="status">{cancelRequested?t('Отмена снимка не подтверждена. Повторите отмену перед новым снимком.'):t("Выберите область или экран в системном окне.")}</p><button disabled={cancelling} onclick={cancelCapture}>{t("Отменить снимок")}</button>{/if}
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
        {:else}<StudioPopover label={t("Справка")} icon="help"><p>{t("Всё изображение будет растянуто до 480 × 480.")}</p></StudioPopover>{/if}
        <button class="source" aria-label="{t("Переместить область crop указателем; точные координаты доступны в полях выше")}" onpointerdown={e => {if(e.button!==0)return; dragging=true; e.currentTarget.setPointerCapture(e.pointerId); move(e);}} onpointermove={move} onpointerup={() => dragging=false} onpointercancel={() => dragging=false} onlostpointercapture={() => dragging=false}>
          <img src={sourceUrl} draggable={false} alt="{t("Исходное изображение для crop")}"/>
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
    {#if error}<p role="alert">{errorText(error)}</p>{/if}

  </section>
{/if}
<style>
  section { position:fixed;inset:24px 16px auto auto;margin:0;width:min(620px,calc(100vw - 32px));max-height:calc(100vh - 48px);overflow:auto;padding:16px;border:0;border-radius:12px;background:var(--surface-raised);color:var(--text);box-shadow:0 12px 32px #0008; }
  header{display:flex;align-items:center;justify-content:space-between;gap:16px}h3{margin:0}
  fieldset { min-width:0; }
  .source { display:block; position:relative; width:min(100%,480px); padding:0; border:0; touch-action:none; margin:12px 0; }
  .source img { display:block; width:100%; height:auto; }
  .selection { position:absolute; border:2px solid white; outline:2px solid black; box-sizing:border-box; pointer-events:none; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
  .preview { display:block; max-width:100%; height:auto; margin:12px 0; }
  p { overflow-wrap:anywhere; }
</style>
