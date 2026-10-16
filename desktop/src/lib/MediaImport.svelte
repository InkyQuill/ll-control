<script lang="ts">
  import {t} from './i18n.svelte';
  import { onMount, tick, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import VideoSourcePreview from './VideoSourcePreview.svelte';
  import PreparedVideoPreview from './PreparedVideoPreview.svelte';
  import { normalizedCrop, type CropFields, type VideoCrop } from './video-crop';
  import { message } from './api';
  import { validVideoAmount } from './video-adjustments';
  import {videoEditBounds,validVideoRange,type SourceMetadata} from './video-range';
  let { device = null, existing, onbusy = () => {}, onimport }: { existing?:{id:string;name:string;url:string;metadata?:SourceMetadata|null}; onbusy?:(busy:boolean)=>void; device?: string | null; onimport: (key: string | null, device: string | null) => void } = $props();
  type Phase = 'uploading' | 'inspecting' | 'source_ready' | 'encoding' | 'validating' | 'remuxing' | 'review_ready' | 'cancelling' | 'transferring' | 'publishing' | 'complete' | 'cancelled' | 'failed';
  interface Job { id: string; name: string; total: number; received: number; phase: Phase; key: string | null; message: string | null; metadata?:SourceMetadata|null }
  const terminal = (phase: Phase) => ['complete', 'cancelled', 'failed'].includes(phase);
  const labels: Record<Phase, string> = { uploading: 'Загрузка файла', inspecting: 'Проверка файла', source_ready: 'Исходник проверен', encoding: 'Конвертация для LCD', validating: 'Проверка результата', remuxing: 'Подготовка просмотра', review_ready: 'Ролик готов к просмотру', cancelling: 'Отмена импорта', transferring: 'Передача ролика в библиотеку', publishing: 'Сохранение в библиотеке', complete: 'Ролик сохранён в библиотеке', cancelled: 'Импорт отменён', failed: 'Импорт не завершён' };
  let job = $state<Job | null>(null), error = $state(''), fit = $state<'contain' | 'cover' | 'stretch'>('contain');
  let mounted = false, announced: string | null = null;
  let ownedId=$state<string|null>(null);
  const sourceReady=$derived(job?.phase==='source_ready' && job.id===ownedId);
  let sourcePreview=$state<{currentPosition:()=>number|null;currentExactPosition:()=>number|null}>();
  let cutEnabled = $state(false), effectsEnabled=$state(false), rangeEnabled=$state(false);
  let contrast=$state<number|undefined>(0), brightness=$state<number|undefined>(0);
  const effectsValid=$derived(!effectsEnabled || (validVideoAmount(contrast) && validVideoAmount(brightness)));
  const adjustments=$derived(effectsEnabled && validVideoAmount(contrast) && validVideoAmount(brightness) ? {contrast,brightness} : null);
  const editing=$derived(cutEnabled || effectsEnabled || rangeEnabled);
  let rangeStart=$state<number|undefined>(0),rangeEnd=$state<number|undefined>();
  let initializedRangeId:string|null=null;
  const rangeMetadata=$derived(existing?.metadata ?? (sourceReady ? job?.metadata : null));
  const rangeBounds=$derived(videoEditBounds(rangeMetadata));
  const rangeValid=$derived(!rangeEnabled || validVideoRange(rangeStart,rangeEnd,rangeBounds));
  const range=$derived(rangeEnabled && rangeValid ? {startSeconds:rangeStart!,endSeconds:rangeEnd!}:null);
  function resetRange(metadata:SourceMetadata|null|undefined=rangeMetadata) {
    rangeStart=0;rangeEnd=videoEditBounds(metadata)?.maximumSeconds;
  }
  function toggleRange() {
    if(rangeEnabled) {cutEnabled=false;recordMode=false;cropSegmentId=null;resetRange();if(!existing)clearSource();}
    else if(!cutEnabled && !effectsEnabled && !rangeEnabled)clearSource();
  }
  function markRange(edge:'start'|'end') {
    const position=sourcePreview?.currentExactPosition();
    if(position===null || position===undefined || active || !rangeBounds)return;
    const value=Math.min(position,rangeBounds.maximumSeconds);
    if(edge==='start')rangeStart=value;else rangeEnd=value;
  }
  let selectedFile=$state<File|null>(null), sourceDuration=$state<number|null>(null), sourcePosition=$state<number|null>(null);
  function markSegment(segment:{start:number|undefined;end:number|undefined},edge:'start'|'end') {
    const position=sourcePreview?.currentPosition();
    if(position!==null && position!==undefined && !active) segment[edge]=position;
  }
  function clearSource() {selectedFile=null;sourceDuration=null;sourcePosition=null;recordedSegments=[];recording=false;}
  type SegmentDraft = {id:number;start:number|undefined;end:number|undefined;crop?:CropFields;cropButton?:HTMLButtonElement;input?:HTMLInputElement};
  let manualSegments=$state<SegmentDraft[]>([{id:0,start:0,end:5}]);
  let recordedSegments=$state<SegmentDraft[]>([]);
  let recordMode=$state(false), recording=$state(false);
  let recordCrop=$state<CropFields>({x:0,y:0,width:100,height:100});
  const segments=$derived(recordMode ? recordedSegments : manualSegments);
  function setSegments(value:SegmentDraft[]) {if(recordMode)recordedSegments=value;else manualSegments=value;}
  function addRecorded(segment:{start_ms:number;end_ms:number;crop:VideoCrop|null}) {
    if(!recordMode || active || recordedSegments.length>=64)return;
    const crop=segment.crop;
    recordedSegments=[...recordedSegments,{id:nextSegmentId++,start:segment.start_ms/1000,end:segment.end_ms/1000,
      ...(crop ? {crop:{x:crop.x*100,y:crop.y*100,width:Math.min(crop.width*100,100-crop.x*100),height:Math.min(crop.height*100,100-crop.y*100)}} : {})}];
    if(cutDuration>=180000) {if(existing)void prepareExisting();else if(selectedFile)void uploadFile(selectedFile);}
  }

  let nextSegmentId=1;
  let cropSegmentId=$state<number|null>(null);
  const cropSegment=$derived(segments.find(s=>s.id===cropSegmentId));
  const previewCrop=$derived(recordMode ? normalizedCrop(recordCrop) : cropSegment?.crop ? normalizedCrop(cropSegment.crop) : null);
  function chooseCrop(id:number) {
    const segment=segments.find(s=>s.id===id);
    if(active || !segment) return;
    segment.crop ??= {x:0,y:0,width:100,height:100};
    cropSegmentId=id;
  }
  async function resetCrop(id:number) {
    const segment=segments.find(s=>s.id===id);
    if(active || !segment) return;
    segment.crop=undefined;
    if(cropSegmentId===id)cropSegmentId=null;
    await tick();
    if(mounted && !active)segment.cropButton?.focus();
  }
  function updateCrop(crop:VideoCrop) {
    if(active || recording || (!recordMode && !cropSegment?.crop)) return;
    const percent=(value:number)=>Math.round(value*10000)/100;
    const value={x:percent(crop.x),y:percent(crop.y),width:percent(crop.width),height:percent(crop.height)};
    if(recordMode)recordCrop=value;else if(cropSegment)cropSegment.crop=value;
  }

  let addSegmentButton = $state<HTMLButtonElement>();
  let recordModeSelect=$state<HTMLSelectElement>();
  const cut = $derived({segments:segments.map(s=>({start_ms:Math.round((s.start ?? NaN)*1000),end_ms:Math.round((s.end ?? NaN)*1000),...(s.crop ? {crop:normalizedCrop(s.crop)} : {})}))});
  const cutDuration = $derived(cut.segments.reduce((n,s)=>n+s.end_ms-s.start_ms,0));
  const cutValid = $derived(!cutEnabled || (segments.every(s=>!s.crop || normalizedCrop(s.crop)!==null) && cut.segments.length>0 && cut.segments.length<=64 && cutDuration<=180000 && cut.segments.every(s=>Number.isSafeInteger(s.start_ms) && Number.isSafeInteger(s.end_ms) && s.start_ms>=0 && s.start_ms<s.end_ms && s.end_ms<=3600000 && (sourceDuration===null || s.end_ms<=Math.floor(sourceDuration*1000)))));
  let pollError = $state('');
  let reads = 0;
  let ready = $state(false), uploading = $state(false), cancelRequested = $state(false), confirming=$state(false);
  let importTarget: string | null = null;
  const active = $derived(uploading || (job !== null && !terminal(job.phase) && !sourceReady));
  $effect(()=>{const busy=active || sourceReady || recording;untrack(()=>onbusy(busy));});
  async function removeSegment(id:number) {
    if(active || recording || (!recordMode && segments.length===1)) return;
    const index=segments.findIndex(s=>s.id===id);
    if(index<0) return;
    const neighbour=segments[index+1] ?? segments[index-1];
    if(cropSegmentId===id)cropSegmentId=null;
    setSegments(segments.filter(s=>s.id!==id));
    await tick();
    if(mounted && !active) (neighbour?.input ?? addSegmentButton ?? recordModeSelect)?.focus();
  }
  function accept(next: Job | null) {
    job = next;
    if(next?.id===ownedId && next?.phase==='source_ready' && initializedRangeId!==next.id) {
      initializedRangeId=next.id;resetRange(next.metadata);
    }
    if (next?.phase === 'complete' && announced !== next.id && (!existing || next.id===ownedId)) { announced = next.id; onimport(next.key, next.id === ownedId ? importTarget : null); }
  }
  async function refresh() {
    const request = ++reads;
    try {
      const next = await invoke<Job | null>('media_status');
      if (!mounted || uploading || request !== reads) return;
      accept(next); ready = true; pollError = '';
    } catch (e) { if (mounted && request === reads) { ready = false; pollError = message(e); } }
  }
  function confirmationFailed(id:string, cause:unknown) {
    // Polling may already have resolved the outcome or moved to another job.
    if(mounted && job?.id===id && job.phase!=='complete')
      error=`${message(cause)} Проверьте состояние импорта перед повторным подтверждением.`;
  }
  async function confirmReview() {
    if(!job || job.phase!=='review_ready' || confirming || cancelRequested || !ready)return;
    const id=job.id;
    confirming=true;reads++;error='';
    try {
      await invoke('media_confirm',{id});
    } catch(e) {
      confirmationFailed(id,e);
    } finally {
      if(mounted) {confirming=false;await refresh();}
    }
  }
  async function cancel() {
    cancelRequested = true; reads++;
    const id = ownedId ?? job?.id;
    if (!id) return;
    try {
      const next = await invoke<Job>('media_cancel', { id });
      if (mounted) accept(next);
    } catch (e) {
      // begin may still be in flight; the upload loop cancels again after its reply.
      if (mounted && !uploading) error = message(e);
    }
  }
  async function importFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0]; input.value = '';
    if (!file || active || sourceReady || recording || !ready) return;
    error = '';
    if(editing)clearSource();
    if (!/\.(gif|mp4)$/i.test(file.name) || file.size === 0 || file.size > 256 * 1024 * 1024) {
      error = 'Выберите непустой GIF или MP4 размером до 256 MiB.'; return;
    }
    if(rangeEnabled) {clearSource();selectedFile=file;await uploadFile(file,true);return;}
    if(editing) {clearSource();selectedFile=file;return;}
    await uploadFile(file);
  }
  async function uploadFile(file:File,inspectOnly=false) {
    if(active || sourceReady || recording || !ready || (!inspectOnly && (!effectsValid || !cutValid || !rangeValid))) return;
    let inspectionStarted=false;
    error='';
    const id = crypto.randomUUID().replaceAll('-', '');
    reads++; ownedId = id; importTarget = device; uploading = true; cancelRequested = false;
    try {
      const initial = await invoke<Job>('media_begin', { id, name: file.name, total: file.size, fit, cut:cutEnabled ? cut : null, ...(adjustments ? {adjustments} : {}) });
      if (mounted) accept(initial);
      for (let offset = 0; offset < file.size; offset += 256 * 1024) {
        if (!mounted || cancelRequested) { await invoke('media_cancel', { id }); return; }
        const bytes = Array.from(new Uint8Array(await file.slice(offset, offset + 256 * 1024).arrayBuffer()));
        if (!mounted || cancelRequested) { await invoke('media_cancel', { id }); return; }
        const next = await invoke<Job>('media_append', { id, offset, bytes });
        if (mounted) accept(next);
      }
      if (!mounted || cancelRequested) { await invoke('media_cancel', { id }); return; }
      if(inspectOnly) {
        inspectionStarted=true;
        const inspected=await invoke<Job>('media_inspect',{id});
        if(!mounted || cancelRequested)await invoke('media_cancel',{id});
        else accept(inspected);
      } else await invoke('media_start', { id, review:editing });
    } catch (e) {
      if (mounted && !cancelRequested) error = `${message(e)} Проверьте состояние импорта и библиотеку перед повторной загрузкой.`;
      // A lost upload acknowledgement must not leave staging permanently admitted.
      // Publishing rejects cancellation; the owned native task still records its result.
      if(!inspectionStarted)try { await invoke('media_cancel', { id }); } catch { /* recover through status */ }
    } finally {
      uploading = false;
      if(inspectionStarted) {
        if(!mounted || cancelRequested)await invoke('media_cancel',{id}).catch(()=>{});
        else await refresh();
      }
    }
  }
  async function prepareInspected() {
    if(!sourceReady || !job || active || !ready || !rangeValid || !effectsValid)return;
    const id=job.id;
    uploading=true;reads++;error='';cancelRequested=false;
    try {await invoke('media_prepare_inspected',{id,fit,range,...(adjustments ? {adjustments}:{})});}
    catch(e) {if(mounted)error=`${message(e)} Проверьте состояние подготовки.`;}
    finally {
      uploading=false;
      if(!mounted || cancelRequested)await invoke('media_cancel',{id}).catch(()=>{});
      if(mounted)await refresh();
    }
  }
  async function prepareExisting() {
    if(!existing || active || recording || !ready || !effectsValid || !cutValid || !rangeValid)return;
    const id=crypto.randomUUID().replaceAll('-','');
    reads++;ownedId=id;importTarget=device;uploading=true;cancelRequested=false;error='';
    try {
      await invoke('media_source_prepare',{sourceId:existing.id,id,name:existing.name.replace(/\.[^.]+$/,'')+'.mp4',fit,cut:cutEnabled ? cut : null,...(range ? {range}:{}),...(adjustments ? {adjustments} : {})});
    } catch(e) {
      if(mounted && !cancelRequested)error=`${message(e)} Проверьте состояние подготовки.`;
    } finally {
      uploading=false;
      if(!mounted || cancelRequested)await invoke('media_cancel',{id}).catch(()=>{});
      if(mounted)await refresh();
    }
  }
  onMount(() => {
    mounted = true;
    if(existing)cutEnabled=true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() { await refresh(); if (mounted) timer = setTimeout(poll, 400); }
    void poll();
    return () => {
      mounted = false; cancelRequested = true; clearTimeout(timer);
      const id = ownedId ?? job?.id;
      if (id && (uploading || (job && !terminal(job.phase) && job.phase !== 'publishing'))) {
        void invoke('media_cancel', { id }).catch(() => {});
      }
    };
  });
</script>

<section aria-label="{t("Импорт ролика")}" class="media-import">
  {#if existing}<p>{t("Редактирование «")}{existing.name}{t("». Исходный файл останется доступен другим темам и профилям.")}</p>{:else}<p>{t("GIF или MP4 до 1 часа и 256 MiB. Подготовленный ролик также должен укладываться в 256 MiB. Импорт сохраняет ролик; запуск на LCD выбирается отдельно.")}</p>{/if}
  <label>{t("Размещение видео")}
    <select bind:value={fit} disabled={active || recording || !ready}>
      <option value="contain">{t("Целиком, с чёрными полями")}</option>
      <option value="cover">{t("Заполнить экран, обрезать края")}</option>
      <option value="stretch">{t("AutoFill — растянуть весь кадр")}</option>
    </select>
  </label>
  {#if fit==='stretch'}<p>{t("AutoFill заполняет экран всем кадром без обрезки; пропорции могут измениться.")}</p>{/if}
  <label><input type="checkbox" bind:checked={effectsEnabled} disabled={active || recording || !ready} onchange={()=>{if(!cutEnabled && !effectsEnabled && !rangeEnabled)clearSource();}} />{t("Цветокоррекция видео")}</label>
  {#if effectsEnabled}
    <fieldset disabled={active || recording || !ready}>
      <legend>{t("Цветокоррекция всего ролика")}</legend>
      <label>{t("Контраст видео")}<input type="range" min="0" max="255" step="1" bind:value={contrast} /></label>
      <label>{t("Контраст видео — значение")}<input type="number" min="0" max="255" step="1" bind:value={contrast} /></label>
      <label>{t("Яркость видео")}<input type="range" min="0" max="255" step="1" bind:value={brightness} /></label>
      <label>{t("Яркость видео — значение")}<input type="number" min="0" max="255" step="1" bind:value={brightness} /></label>
      <button onclick={()=>{contrast=0;brightness=0;}}>{t("Сбросить цветокоррекцию")}</button>
      <p>{t("0 — без изменения. Яркость осветляет изображение до белого. Настройки применяются ко всем выбранным фрагментам.")}</p>
      {#if !effectsValid}<p role="alert">{t("Укажите целые значения яркости и контраста от 0 до 255.")}</p>{/if}
    </fieldset>
  {/if}
  <label><input type="checkbox" bind:checked={cutEnabled} onchange={()=>{if(cutEnabled)rangeEnabled=false;else if(!effectsEnabled && !rangeEnabled)clearSource();}} disabled={active || sourceReady || recording || !ready} />{t("Выбрать фрагменты видео")}</label>
  <label><input type="checkbox" bind:checked={rangeEnabled} onchange={toggleRange} disabled={active || sourceReady || recording || !ready} />{t("Один диапазон VideoEdit")}</label>
  {#if rangeEnabled}
    <p>{t("Начало и конец одного диапазона. Новый файл сначала загружается для проверки длительности и частоты кадров; кодирование начнётся по отдельной кнопке.")}</p>
    {#if rangeBounds}
      <fieldset disabled={active || recording || !ready}>
        <legend>{t("Диапазон VideoEdit")}</legend>
        <p>{t("Частота:")} {rangeBounds.fps.toFixed(3)} {t("FPS. Конец — до")} {rangeBounds.maximumSeconds.toFixed(6)} {t("с; минимальная длина —")} {rangeBounds.minimumSpanSeconds.toFixed(6)} {t("с.")}</p>
        <label>{t("Начало VideoEdit")}<input type="range" min="0" max={rangeBounds.maximumSeconds} step="any" bind:value={rangeStart} /></label>
        <label>{t("Начало VideoEdit, с")}<input type="number" min="0" max={rangeBounds.maximumSeconds} step="any" bind:value={rangeStart} /></label>
        <label>{t("Конец VideoEdit")}<input type="range" min="0" max={rangeBounds.maximumSeconds} step="any" bind:value={rangeEnd} /></label>
        <label>{t("Конец VideoEdit, с")}<input type="number" min="0" max={rangeBounds.maximumSeconds} step="any" bind:value={rangeEnd} /></label>
        <button disabled={sourcePosition===null} onclick={()=>markRange('start')}>{t("Начало диапазона по текущему кадру")}</button>
        <button disabled={sourcePosition===null} onclick={()=>markRange('end')}>{t("Конец диапазона по текущему кадру")}</button>
        <button onclick={()=>resetRange()}>{t("Весь доступный диапазон VideoEdit")}</button>
        {#if !rangeValid}<p role="alert">{t("Диапазон должен быть не короче двух исходных кадров и не выходить за указанный конец.")}</p>{/if}
      </fieldset>
    {:else if sourceReady || existing}<p role="alert">{t("Нет допустимой длительности или частоты кадров для VideoEdit, либо ролик слишком короткий. Для нового файла отмените импорт перед выбором другого режима.")}</p>
    {:else}<p>{t("Выберите GIF/MP4 для проверки исходника.")}</p>{/if}
  {/if}
  {#if cutEnabled}
    <p>{t("Выбор файла открывает просмотр. После подготовки можно посмотреть готовый ролик; сохранение в библиотеку потребует подтверждения.")}</p>
    <label>{t("Способ выбора фрагментов")}<select bind:this={recordModeSelect} bind:value={recordMode} disabled={active || recording || !ready} onchange={()=>cropSegmentId=null}>
      <option value={false}>{t("Время вручную")}</option><option value={true}>{t("Запись при просмотре")}</option>
    </select></label>
    {#if recordMode}
      <fieldset disabled={active || recording || !ready}>
        <legend>{t("Область следующей записи, % исходника")}</legend>
        {#each [{key:'x',label:'Слева'},{key:'y',label:'Сверху'},{key:'width',label:'Ширина'},{key:'height',label:'Высота'}] as field (field.key)}
          <label>{field.label}{t(", % — запись")}<input type="number" min="0" max="100" step="0.01" bind:value={recordCrop[field.key as keyof CropFields]} /></label>
        {/each}
      </fieldset>
      {#if !previewCrop}<p role="alert">{t("Область записи должна быть непустой и помещаться внутри исходника.")}</p>{/if}
      <p>{t("«Записать фрагмент» начинает просмотр с текущей позиции; «Пауза записи» добавляет выбранный интервал. Между записями можно перемотать видео и изменить область. Ручной список сохраняется отдельно. При достижении 180 секунд подготовка результата начнётся автоматически; сохранение в библиотеку потребует подтверждения.")}</p>
    {/if}
    {#if cropSegment && !recordMode}<p role="status">{t("Область для фрагмента")} {segments.indexOf(cropSegment)+1}{t(". Выберите её указателем под видео или задайте проценты в полях фрагмента.")}</p>{/if}
    <fieldset disabled={active || recording || !ready}>
      <legend>{t("Фрагменты в порядке воспроизведения")}</legend>
      <p>{t("Укажите начало и конец в секундах. Фрагменты соединяются по порядку без звука, суммарно до 3 минут. До 64 фрагментов за импорт. Конец каждого фрагмента проверяется по длительности выбранного файла.")}</p>
      {#each segments as segment, i (segment.id)}
        <div class="segment">
          {#if recordMode}
            <p>{t("Фрагмент")} {i+1}: {segment.start?.toFixed(3)}–{segment.end?.toFixed(3)} {t("с. Область:")} {segment.crop ? `${segment.crop.x}%, ${segment.crop.y}%, ${segment.crop.width}% × ${segment.crop.height}%` : "" + t("весь кадр") + ""}.</p>
          {:else}
          <label>{t("Начало фрагмента")} {i+1}{t(", с")}<input type="number" min="0" max="3600" step="0.001" bind:value={segment.start} bind:this={segment.input} /></label>
          <label>{t("Конец фрагмента")} {i+1}{t(", с")}<input type="number" min="0" max="3600" step="0.001" bind:value={segment.end} /></label>
          <button disabled={sourcePosition===null} onclick={()=>markSegment(segment,'start')}>{t("Начало по текущему кадру")} {i+1}</button>
          <button disabled={sourcePosition===null} onclick={()=>markSegment(segment,'end')}>{t("Конец по текущему кадру")} {i+1}</button>
          <button bind:this={segment.cropButton} aria-pressed={cropSegmentId===segment.id} onclick={()=>chooseCrop(segment.id)}>{t("Выбрать область фрагмента")} {i+1}</button>
          {#if segment.crop}
            <fieldset aria-label={`Область фрагмента ${i+1}`}>
              <legend>{t("Область фрагмента")} {i+1}{t(", % исходника")}</legend>
              {#each [{key:'x',label:'Слева'},{key:'y',label:'Сверху'},{key:'width',label:'Ширина'},{key:'height',label:'Высота'}] as field (field.key)}
                <label>{field.label}{t(", % — фрагмент")} {i+1}<input type="number" min="0" max="100" step="0.01" bind:value={segment.crop[field.key as keyof CropFields]} /></label>
              {/each}
              {#if !normalizedCrop(segment.crop)}<p role="alert">{t("Область должна иметь положительный размер и помещаться внутри исходника.")}</p>{/if}
              <button onclick={()=>resetCrop(segment.id)}>{t("Весь кадр — фрагмент")} {i+1}</button>
            </fieldset>
          {/if}
          {/if}
          <button disabled={!recordMode && segments.length===1} onclick={()=>removeSegment(segment.id)}>{t("Удалить фрагмент")} {i+1}</button>
        </div>
      {/each}
      {#if !recordMode}<button bind:this={addSegmentButton} disabled={segments.length>=64} onclick={()=>setSegments([...segments,{id:nextSegmentId++,start:0,end:5}])}>{t("Добавить фрагмент")}</button>{/if}
      {#if cutValid}<p>{t("Выбрано:")} {(cutDuration/1000).toFixed(3)} {t("с.")}</p>
      {:else if recordMode && segments.length===0}<p>{t("Пока нет записанных фрагментов.")}</p>
      {:else}<p role="alert">{t("Укажите начало меньше конца, в пределах часа. Общая длительность — до 180 секунд; конец не должен превышать длительность выбранного видео.")}</p>{/if}
    </fieldset>
  {/if}
  {#if editing || existing}
    {#if selectedFile || existing}
      {#key selectedFile ?? existing?.id}
        <VideoSourcePreview bind:this={sourcePreview} file={selectedFile ?? undefined} external={existing} crop={cutEnabled ? previewCrop : null} {fit} {adjustments} oncrop={updateCrop} recordEnabled={cutEnabled && recordMode} remainingMs={180000-cutDuration} canRecord={effectsValid && recordedSegments.length<64 && previewCrop!==null} onrecord={addRecorded} onrecording={(value)=>recording=value} disabled={active} onposition={(value)=>sourcePosition=value} onmetadata={(value)=>sourceDuration=value} />
      {/key}
    {/if}
  {/if}
  {#if !existing}<label>{t("Импортировать GIF/MP4")}
    <input type="file" accept=".gif,.mp4,image/gif,video/mp4" disabled={active || sourceReady || recording || !ready} onchange={importFile} />
  </label>
  {/if}
  {#if existing}
    <button disabled={active || recording || !ready || !effectsValid || !cutValid || !rangeValid} onclick={prepareExisting}>{t("Подготовить изменения ролика")}</button>
  {:else if rangeEnabled}
    {#if sourceReady}<button disabled={active || !ready || !rangeValid || !effectsValid} onclick={prepareInspected}>{t("Подготовить диапазон VideoEdit")}</button>{/if}
  {:else if editing && selectedFile}
    <button disabled={active || recording || !ready || !effectsValid || !cutValid} onclick={()=>{if(selectedFile)void uploadFile(selectedFile);}}>{cutEnabled ? "" + t("Подготовить выбранные фрагменты") + "" : "" + t("Подготовить ролик") + ""}</button>
  {/if}
  {#if job}
    <p role="status" aria-live="polite">{t(labels[job.phase])}: {job.name}</p>
    {#if job.phase === 'uploading'}<progress value={job.received} max={job.total} aria-label="{t("Загрузка файла")}"></progress>{/if}
    {#if job.phase==='review_ready'}
      {#key job.id}<PreparedVideoPreview id={job.id} disabled={confirming || cancelRequested} />{/key}
      <button disabled={confirming || cancelRequested || !ready} onclick={confirmReview}>{t("Сохранить готовый ролик")}</button>
    {/if}
    {#if job.message}<p role="alert">{job.message}</p>{/if}
    {#if job.phase === 'publishing'}<p>{t("Сохранение уже началось. Дождитесь результата перед повторным импортом.")}</p>{/if}
  {/if}
  {#if active || sourceReady}<button disabled={cancelRequested || job?.phase === 'publishing'} onclick={cancel}>{t("Отменить импорт")}</button>{/if}
  {#if error || pollError}<p role="alert">{t(error || pollError)}</p><button onclick={refresh}>{t("Проверить импорт")}</button>{/if}
</section>
<style>
  .media-import { margin:16px 0; padding:12px; border:1px solid #33445b; border-radius:8px; }
  label { display:grid; gap:6px; margin:10px 0; }
  fieldset { min-width:0; }
  .segment { margin:12px 0; }
  input, select, progress { min-width:0; max-width:100%; }
  p { overflow-wrap:anywhere; }
</style>
