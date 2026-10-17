<script lang="ts">
  import { ValidationError, type UiDiagnostic } from '$lib/validation-error';
  import {t, diagnosticText} from '$lib/i18n.svelte';
  import {syncNativeLanguage} from '$lib/native-language';
  $effect(()=>syncNativeLanguage(presentation.language));
  import '$lib/studio.css';
  import DeviceOverview from '$lib/DeviceOverview.svelte';
  import { deviceCapabilities } from '$lib/device-capabilities';
  import LcdPreview from '$lib/LcdPreview.svelte';
  import TextStyleControls from '$lib/TextStyleControls.svelte';
  import CanvasEditor from '$lib/CanvasEditor.svelte';
  import { editableLayer, type EditorPreview, type LayerBounds, type QuarterTurn } from '$lib/canvas-geometry';
  let collectionEpoch = $state(0), resetEpoch = $state(0);
  import { addImportedToCollection, imageReplacementSnapshot, replaceCollectionImage, ImportedImageMembershipError } from '$lib/collection-membership';
  import FullGroupArchive from '$lib/FullGroupArchive.svelte';
  import DividerEditor from '$lib/DividerEditor.svelte';
  import ScreenReset from '$lib/ScreenReset.svelte';
  import ScreenCollections from '$lib/ScreenCollections.svelte';
  import ImageGallery from '$lib/ImageGallery.svelte';
  import ImageEditor from '$lib/ImageEditor.svelte';
  import MediaLibrary from '$lib/MediaLibrary.svelte';
  import MediaThumbnail from '$lib/MediaThumbnail.svelte';
  import FontImport from '$lib/FontImport.svelte';
  import type { FontChoice } from '$lib/font-upload';
  let importedFonts = $state<FontChoice[]>([]);
  import {themePages, switchLcdPage, isProcedural} from '$lib/lcd-pages';
  import {assertWeatherDraft} from '$lib/weather-settings';
  import PaletteEditor from '$lib/PaletteEditor.svelte';
  import WeatherEditor from '$lib/WeatherEditor.svelte';
  import BackgroundEditor from '$lib/BackgroundEditor.svelte';
  import BasicEditor from '$lib/BasicEditor.svelte';
  import SensorDefaultsEditor from '$lib/SensorDefaults.svelte';
  import Profiles from '$lib/Profiles.svelte';
  import AssetCleanup from '$lib/AssetCleanup.svelte';
  import Lighting from '$lib/Lighting.svelte';
  import Settings from '$lib/Settings.svelte';
  import FpsSettings from '$lib/FpsSettings.svelte';
  import FirmwareInfo from '$lib/FirmwareInfo.svelte';
  import FirmwareRecovery from '$lib/FirmwareRecovery.svelte';
  import DesktopSettings from '$lib/DesktopSettings.svelte';
  import SystemSpec from '$lib/SystemSpec.svelte';
  import DisplaySettings from '$lib/DisplaySettings.svelte';
  import {presentation} from '$lib/presentation.svelte';
  import { presets, presetTheme, formatSensor, type PresetId } from '$lib/theme-presets';
  import Cooling from '$lib/Cooling.svelte';
  import { onMount, untrack, tick } from 'svelte';
  import { rpc, message, importImageAsset, hex, rgb, style, initialTheme, type Device, type Sensor, type Theme, type Layer, type LcdState } from '$lib/api';
  $effect(()=>{document.documentElement.lang=presentation.language==='en-US'?'en':'ru';});
  let devices = $state<Device[]>([]), sensors = $state<Sensor[]>([]);
  let device = $state(''), tab = $state<'display' | 'sensors' | 'cooling' | 'lighting' | 'settings' | 'spec' | 'overview'>('overview');
  let lcdPanel = $state<'composition' | 'library' | 'presets'>('composition');
  let lightingVisited = $state(false);
  $effect(() => { if (tab === 'lighting') lightingVisited = true; });
  let lastDevice = $state<Device | undefined>();
  const selectedDevice = $derived(devices.find(d => d.id === device) ?? (lastDevice?.id === device ? lastDevice : undefined));
  const capabilities = $derived(deviceCapabilities(selectedDevice));
  function openDevice(id: string, section: 'display' | 'lighting' | 'cooling' | 'settings') { selectDevice(id); tab = section; }
  let profileMediaKey = $state(''), loadedMediaKey = $state('');
  let theme = $state<Theme>(initialTheme(t('Мой экран'))), lcd = $state<LcdState | null>(null);
  function lcdSourceLabel(source: string | null | undefined): string {
    switch (source) {
      case 'Idle': return t('Нет программного потока');
      case 'Jpeg': return t('Изображение');
      case 'DiagnosticClip': return t('Видео');
      case 'Theme': return t('Тема LCD');
      default: return source ?? '—';
    }
  }
  const draftVideoKeys = $derived([...new Set(themePages(theme).flatMap(page=>page.background_video?[page.background_video]:[]))]);
  const draftImageKeys = $derived([...new Set(themePages(theme).flatMap(page=>[...(page.background_image?[page.background_image.asset]:[]),...page.layers.filter(layer=>layer.kind==='image').map(layer=>layer.asset)]))]);
  const fontChoices = $derived.by(() => {
    const fonts = new Map(importedFonts.map(font => [font.key, font]));
    for (const layer of themePages(theme).flatMap(page=>page.layers)) {
      if (layer.kind === 'divider' || layer.kind === 'image' || layer.kind==='weather' || layer.kind==='meteor' || layer.kind==='snow3d' || layer.kind==='fireworks') continue;
      const styles = [layer.style];
      if ((layer.kind === 'sensor' || layer.kind === 'sensor_cycle') && layer.parts) styles.push(layer.parts.label, layer.parts.unit);
      for (const style of styles) {
        if (style.font_asset && !fonts.has(style.font_asset)) fonts.set(style.font_asset, {key:style.font_asset, name:t('Пользовательский ({0})',style.font_asset.slice(0,8))});
      }
    }
    return [...fonts.values()];
  });
  type LcdOperation = 'load-imported' | 'load' | 'apply' | 'localui' | 'media' | 'image-import' | 'image-membership' | 'image-replacement';
  type LcdFailure = { context: LcdOperation; detail: UiDiagnostic };
  let error = $state<string | LcdFailure>(''), notice = $state(''), connected = $state(false), actionBusy = $state(false), resetBusy = $state(false), groupBusy = $state(false);
  function errorText(value: string | LcdFailure): string {
    if (typeof value === 'string') return t(value);
    switch (value.context) {
      case 'load-imported': return t('Не удалось загрузить импортированный редактор: {0}. Черновик сохранён; повторите загрузку явно.', diagnosticText(value.detail));
      case 'load': return t('Не удалось загрузить активную тему: {0}. Черновик сохранён; повторите загрузку явно.', diagnosticText(value.detail));
      case 'apply': return t('Применение темы не подтверждено: {0}. Проверьте текущее состояние LCD перед повторным Apply.', diagnosticText(value.detail));
      case 'localui': return t('Возврат LocalUI не подтверждён: {0}. Проверьте текущее состояние LCD перед повтором.', diagnosticText(value.detail));
      case 'media': return t('Выбор ролика не подтверждён: {0}. Проверьте текущее состояние LCD перед повтором.', diagnosticText(value.detail));
      case 'image-import': return t('Импорт изображения не подтверждён: {0}. Проверьте каталог перед повторной загрузкой.', diagnosticText(value.detail));
      case 'image-membership': return t('Файл сохранён в каталоге, но добавление в коллекцию не подтверждено: {0}. Проверьте коллекции перед повтором.', diagnosticText(value.detail));
      case 'image-replacement': return t('Замена не подтверждена: {0}. Проверьте каталог и коллекции перед повтором.', diagnosticText(value.detail));
    }
  }
  const busy = $derived(actionBusy || resetBusy || groupBusy);
  let connectionError = $state('');
  let selectionRevision = $state(0), refreshRevision = 0, lcdRevision = 0;
  const deviceAvailable = $derived(connected && devices.some(d => d.id === device));
  function selectDevice(value: string) {
    lastDevice = devices.find(d => d.id === value);
    device = value; selectionRevision++; lcdRevision++; lcd = null; notice = ''; error = '';
  }
  let preview = $state(''), selectedSensor = $state('');
  let presetId = $state<PresetId>('focus'), presetBindings = $state<string[]>([]);
  let presetRequired = $derived(presets.find(p => p.id === presetId)!.sensors);
  async function refresh() {
    const request = ++refreshRevision;
    let stateSelection: number | undefined;
    try {
      const [nextDevices, nextSensors] = await Promise.all([rpc<Device[]>('list_devices'), rpc<Sensor[]>('list_sensors')]);
      if (request !== refreshRevision) return;
      devices = nextDevices; sensors = nextSensors; connected = true; connectionError = '';
      // Retain a missing device selection; never redirect Apply to another device.
      if (!device && devices.length) selectDevice(devices[0].id);
      const target = device, selection = selectionRevision, revision = lcdRevision;
      stateSelection = selection;
      const state = devices.some(d => d.id === target && deviceCapabilities(d).display) ? await rpc<LcdState>('get_lcd_state', { dev: target }) : null;
      if (request === refreshRevision && selection === selectionRevision && revision === lcdRevision) lcd = state;
    } catch (e) {
      if (request !== refreshRevision || (stateSelection !== undefined && stateSelection !== selectionRevision)) return;
      // Invalidate operations from the old connection, even if the device ID survives.
      if (connected) { selectionRevision++; lcdRevision++; notice = ''; error = ''; }
      connected = false; lcd = null;
      sensors = sensors.map(s => ({ ...s, value: null, status: 'unavailable' }));
      connectionError = message(e);
    }
  }
  async function loadTheme(imported = false) {
    const target = device, selection = selectionRevision, draft = JSON.stringify(theme);
    await action(async () => {
      const saved = await rpc<Theme | null>(imported ? 'get_imported_lcd_editor' : 'get_lcd_theme', { dev: target });
      if (selection !== selectionRevision) return;
      if (draft !== JSON.stringify(theme)) {
        notice = 'Черновик изменился. Загруженная тема не заменила ваши правки.';
        return;
      }
      if (!saved) { error = imported ? 'Нет импортированного basic-редактора для загрузки.' : 'На устройстве нет активной темы'; return; }
      theme = saved;
      loadedMediaKey = '';
      if (imported) notice = 'Импортированный редактор загружен в черновик. Применение — отдельное действие.';
    }, () => selection === selectionRevision, imported ? 'load-imported' : 'load');
  }
  async function selectMedia(key: string) {
    const target = device, selection = selectionRevision;
    lcdRevision++;
    await action(async () => {
      const result = await rpc<LcdState>('set_lcd_media', {dev: target, key});
      if (selection !== selectionRevision) return;
      lcdRevision++; lcd = result;
      notice = 'Ролик выбран. Сохранение настроек поставлено в очередь.';
    }, () => selection === selectionRevision, 'media');
  }
  async function applyTheme() {
    const target = device, selection = selectionRevision, draft = $state.snapshot(theme);
    lcdRevision++;
    await action(async () => {
      // Local draft validation is app-owned UI text; daemon errors remain data.
      try { assertWeatherDraft(draft); } catch (e) { error = message(e); return; }
      const result = await rpc<LcdState>('set_lcd_theme', { dev: target, theme: draft });
      if (selection !== selectionRevision) return;
      lcdRevision++; lcd = result;
      notice = 'Тема отправлена. Сохранение настроек поставлено в очередь.';
    }, () => selection === selectionRevision, 'apply');
  }
  async function clearTheme() {
    const target = device, selection = selectionRevision;
    lcdRevision++;
    await action(async () => {
      await rpc('clear_lcd', { dev: target });
      if (selection !== selectionRevision) return;
      lcdRevision++; await refresh();
      if (selection === selectionRevision) notice = 'Запрошен LocalUI. Автовосстановление темы и ролика отключено.';
    }, () => selection === selectionRevision, 'localui');
  }
  onMount(() => {
    if (new URLSearchParams(location.search).get('view') === 'display') tab = 'display';
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() { await refresh(); if (!stopped) timer = setTimeout(poll, 1500); }
    void poll();
    return () => { stopped = true; clearTimeout(timer); if (preview) URL.revokeObjectURL(preview); };
  });
  async function action(run: () => Promise<void>, current: () => boolean, context: LcdOperation) {
    actionBusy = true; error = ''; notice = '';
    try { await run(); } catch (e) { if (current()) error = { context: e instanceof ImportedImageMembershipError ? 'image-membership' : context, detail: e instanceof ImportedImageMembershipError ? e.detail : e instanceof ValidationError ? e : message(e) }; } finally { actionBusy = false; }
  }
  let livePreview=$state(false), previewRequest=$state(0), previewBusy=$state(false), previewError=$state('');
  let canvasEditing=$state(false), canvasBounds=$state<(LayerBounds|null)[]>([]), canvasDraft=$state(''), selectedLayer=$state(-1);
  const layerIdentities=new WeakMap<Layer,number>();
  let nextLayerIdentity=0;
  function layerIdentity(layer:Layer):number {
    let id=layerIdentities.get(layer);
    if(id===undefined){id=nextLayerIdentity++;layerIdentities.set(layer,id);}
    return id;
  }
  const canvasLayerKeys=$derived(theme.layers.map(layerIdentity));
  const canvasRotation=$derived((lcd?.requested.rotation_degrees ?? 0) as QuarterTurn);
  let undoDrafts=$state<string[]>([]), redoDrafts=$state<string[]>([]);
  let historyDraft=untrack(()=>JSON.stringify(theme));
  function rememberDraft(next:string) {
    if(next===historyDraft)return;
    undoDrafts=[...undoDrafts,historyDraft].slice(-50);
    while(undoDrafts.length>1 && undoDrafts.reduce((size,draft)=>size+draft.length,0)>8_000_000)undoDrafts.shift();
    redoDrafts=[];historyDraft=next;
  }
  $effect(()=>{const next=JSON.stringify(theme);untrack(()=>rememberDraft(next));});
  function restoreDraft(redo=false) {
    rememberDraft(JSON.stringify(theme));
    const source=redo?redoDrafts:undoDrafts, next=source.at(-1);if(!next)return;
    if(redo){redoDrafts=source.slice(0,-1);undoDrafts=[...undoDrafts,historyDraft];}
    else{undoDrafts=source.slice(0,-1);redoDrafts=[...redoDrafts,historyDraft];}
    historyDraft=next;theme=JSON.parse(next);selectedLayer=-1;
  }
  function selectCanvasLayer(index:number) {selectedLayer=index;lcdPanel='composition';}
  function changeCanvasLayer(index:number,layer:Layer) {
    if(index<0||index>=theme.layers.length)return;
    const identity=layerIdentity(theme.layers[index]);
    theme.layers[index]=layer;layerIdentities.set(theme.layers[index],identity);selectedLayer=index;
  }
  const previewDraft=$derived(JSON.stringify(theme));
  let previewGeneration=0, lastPreviewRequest=0;
  let previewJob: {generation:number;dev:string;theme:Theme;live:boolean;started:number;editor:boolean}|null=null;
  let previewTimer:ReturnType<typeof setTimeout>|undefined;
  let previewFlight=false;
  $effect(()=>{
    const draft=previewDraft, dev=device, online=connected, visible=tab==='display', live=livePreview, request=previewRequest, editor=canvasEditing;
    const manual=request!==lastPreviewRequest;lastPreviewRequest=request;
    const generation=++previewGeneration;
    clearTimeout(previewTimer);
    if(!editor || !online || !visible) {untrack(()=>{if(preview) URL.revokeObjectURL(preview);});preview='';}
    previewError='';canvasDraft='';
    previewJob=online && visible && (live||manual||editor) ? {generation,dev,theme:JSON.parse(draft),live,editor,started:performance.now()} : null;
    if(previewJob && (live||manual||editor)) previewTimer=setTimeout(runPreview,250);
    return()=>{previewJob=null;clearTimeout(previewTimer);};
  });
  async function runPreview() {
    if(previewFlight || !previewJob)return;
    const job=previewJob;previewFlight=true;previewBusy=true;
    try {
      const params={dev:job.dev||null,theme:job.theme,elapsed_seconds:Math.floor((performance.now()-job.started)/1000)};
      const result=job.editor?await rpc<EditorPreview>('preview_lcd_editor',params):await rpc<number[]>('preview_lcd_theme',params);
      const bytes=Array.isArray(result)?result:result.jpeg;
      if(previewJob?.generation!==job.generation)return;
      const url=URL.createObjectURL(new Blob([new Uint8Array(bytes)],{type:'image/jpeg'}));
      if(preview)URL.revokeObjectURL(preview);
      preview=url;previewError='';
      canvasBounds=Array.isArray(result)?[]:result.layers;canvasDraft=job.editor?JSON.stringify(job.theme):'';
      if(!job.live&&!job.editor)notice='Preview обновлён. На устройство ничего не отправлено.';
    } catch(e) {
      if(previewJob?.generation===job.generation){previewError=message(e);if(preview)URL.revokeObjectURL(preview);preview='';}
    } finally {
      previewFlight=false;previewBusy=false;
      if(previewJob && (previewJob.live || previewJob.generation!==job.generation)) {
        clearTimeout(previewTimer);previewTimer=setTimeout(runPreview,previewJob.generation===job.generation?1000:250);
      }
    }
  }
  function addSensor() {
    const sensor = sensors.find(s => s.id === selectedSensor); if (!sensor) return;
    theme.layers.push({ kind: 'sensor', sensor_id: sensor.id, label: sensor.label, unit: sensor.unit, temperature_unit: 'system', decimals: 1, style: style(210) });
  }
  async function importImage(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0]; if (!file) return;
    const restoreFocus = document.activeElement === input;
    const target = device, selection = selectionRevision;
    await action(async () => {
      const asset = await importImageAsset(file);
      if (target) {
        try { await addImportedToCollection(target, 'image', asset); }
        catch (e) { collectionEpoch++; throw new ImportedImageMembershipError(e instanceof ValidationError ? e : message(e)); }
      }
      collectionEpoch++;
      if (selection !== selectionRevision) return;
      prependLayer({ kind: 'image', asset, x: 0, y: 0, width: 480, height: 480, fit: 'crop', opacity: 255 });
      notice = 'Изображение добавлено в черновик темы.';
    }, () => selection === selectionRevision, 'image-import');
    input.value = '';
    await tick();
    // Disabling the picker during import can drop its focus; keep a user's new target.
    if (restoreFocus && input.isConnected && document.hasFocus()
        && (document.activeElement === document.body || (input.disabled && document.activeElement === input))) {
      const target = input.disabled ? editorElement?.querySelector<HTMLElement>('.layer h2') : input;
      target?.focus();
    }
  }
  async function publishEditedImage(png: number[], name: string) {
    if (busy || !deviceAvailable || theme.layers.length >= 32) throw new ValidationError('Вывод недоступен или достигнут лимит слоёв.');
    const target = device, selection = selectionRevision;
    actionBusy = true;
    try {
      const asset = await importImageAsset(new File([new Uint8Array(png)], name, {type:'image/png'}));
      try { await addImportedToCollection(target, 'image', asset); }
      catch (e) { throw new ImportedImageMembershipError(e instanceof ValidationError ? e : message(e)); }
      if (selection !== selectionRevision) return 'Результат сохранён в галерее. Выбор устройства или соединение изменились; черновик не изменён.';
      if (theme.layers.length >= 32) return 'Результат сохранён в галерее. Достигнут лимит 32 слоёв; черновик не изменён.';
      prependLayer({kind:'image',asset,x:0,y:0,width:480,height:480,fit:'crop',opacity:255});
      return 'Результат сохранён и добавлен в черновик. Для вывода нажмите «Применить к LCD».';
    } finally { collectionEpoch++; actionBusy = false; }
  }
  async function publishGalleryEdit(key: string, context: 'image'|'background'|'watermark', png: number[], name: string): Promise<string> {
    if (busy || !deviceAvailable) throw new ValidationError('Устройство недоступно.');
    const target = device, selection = selectionRevision;
    actionBusy = true; error = ''; notice = '';
    try {
      const current = await imageReplacementSnapshot(target,key,context);
      const asset = await importImageAsset(new File([new Uint8Array(png)],name,{type:'image/png'}));
      await replaceCollectionImage(target,current,context,key,asset);
      // Updating the selected context does not rewrite saved profiles or other
      // gallery contexts. Matching references in the current draft are explicit.
      if (selection === selectionRevision) {
        if (context === 'background' && theme.background_image?.asset === key) theme.background_image.asset = asset;
        if (context !== 'background') for (const layer of theme.layers) {
          if (layer.kind === 'image' && layer.asset === key && (context === 'watermark') === !!layer.watermark_position) layer.asset = asset;
        }
      }
      const result = 'Запись коллекции заменена. Прежний файл и сохранённые темы сохранены; изменения LCD требуют применения темы.';
      if (selection === selectionRevision) notice = result;
      return result;
    } catch(e) { if (selection === selectionRevision) error = {context:'image-replacement',detail:e instanceof ValidationError ? e : message(e)}; throw e; }
    finally { collectionEpoch++; actionBusy = false; }
  }
  let editorElement: HTMLElement | undefined = $state();
  async function focusLayer(index: number) {
    const editor = editorElement, layers = theme.layers;
    await tick();
    if (!editor?.isConnected || editor !== editorElement || layers !== theme.layers) return;
    const items = editor.querySelectorAll<HTMLElement>('.layer h2');
    const target = items[Math.min(index, items.length - 1)] ?? editor.querySelector<HTMLElement>('[data-add-text]');
    target?.focus();
  }
  async function removeCycleSensor(layer: Extract<Layer, { kind: 'sensor_cycle' }>, index: number, button: HTMLButtonElement) {
    if (layer.sensors.length <= 1) return;
    const container = button.closest('article'), sensors = layer.sensors;
    sensors.splice(index, 1);
    await tick();
    // A concurrent theme load/removal must not focus controls from an obsolete draft.
    if (!container?.isConnected || !theme.layers.includes(layer) || sensors !== layer.sensors) return;
    const sources = container.querySelectorAll<HTMLSelectElement>('[data-cycle-source]');
    sources[Math.min(index, sources.length - 1)]?.focus();
  }
  function prependLayer(layer:Layer) {
    const selected=theme.layers[selectedLayer];
    theme.layers.splice(isProcedural(theme.layers[0])?1:0,0,layer);
    if(selected)selectedLayer=theme.layers.indexOf(selected);
  }
  function removeLayer(index: number) {
    if(isProcedural(theme.layers[index])) {
      try {theme=switchLcdPage($state.snapshot(theme),'basic');error='';}
      catch(e) {error=message(e);return;}
    } else theme.layers.splice(index, 1);
    selectedLayer=Math.min(index,theme.layers.length-1);
    void focusLayer(index);
  }
  function move(index: number, step: number) {
    const target = index + step;
    const first=isProcedural(theme.layers[0])?1:0;
    if (target < first || index < first || target >= theme.layers.length) return;
    [theme.layers[index], theme.layers[target]] = [theme.layers[target], theme.layers[index]];
    selectedLayer=target;
    void focusLayer(target);
  }
  const labels: Record<Layer['kind'], string> = $derived({ divider:t('Разделитель'),fireworks:t('Фейерверк'), snow3d:t('Объёмный снег'), meteor:t('Метеоры'), weather:t('Эффект погоды'), sensor_cycle:t('Смена датчиков'), text:t('Текст'), clock:t('Часы'), sensor:t('Датчик'), image:t('Изображение') });
</script>
<svelte:head><title>L-Connect Linux</title></svelte:head>
<a class="skip-link" href="#main-content">{t("Перейти к содержимому")}</a>
{#snippet capabilityLinks()}
      {#if capabilities.display || (!selectedDevice && !devices.length)}<button class="device-section" class:active={tab === 'display'} aria-current={tab === 'display' ? 'page' : undefined} onclick={() => tab = 'display'}>{t("Экран помпы")}</button>{/if}
      {#if capabilities.cooling}<button class="device-section" class:active={tab === 'cooling'} aria-current={tab === 'cooling' ? 'page' : undefined} onclick={() => tab = 'cooling'}>{t("Охлаждение")}</button>{/if}
      {#if capabilities.lighting}<button class="device-section" class:active={tab === 'lighting'} aria-current={tab === 'lighting' ? 'page' : undefined} onclick={() => tab = 'lighting'}>{t("Подсветка")}</button>{/if}
{/snippet}
<div class="shell">
  <aside>
    <div class="brand">L<span>·</span>CONNECT<small>LINUX</small></div>
    <nav aria-label={t("Разделы")}>
      <button class:active={tab === 'overview'} aria-current={tab === 'overview' ? 'page' : undefined} onclick={() => tab = 'overview'}>{t("Обзор")}</button>
      <p class="nav-label">{t("Ваши устройства")}</p>
      {#each devices as d (d.id)}
        <button class="device-nav" class:active={device === d.id && ['display','lighting','cooling'].includes(tab)} disabled={busy} onclick={() => openDevice(d.id, deviceCapabilities(d).lighting ? 'lighting' : 'settings')}>{d.display_name}</button>
        {#if device === d.id}{@render capabilityLinks()}{/if}
      {/each}
      {#if device && !devices.some(d => d.id === device)}<p class="disconnected-device">{selectedDevice?.display_name ?? device} · {t("Недоступен")}</p>{/if}
      {#if !devices.some(d => d.id === device)}{@render capabilityLinks()}{/if}
      <div class="global-nav">
        <button class:active={tab === 'spec'} aria-current={tab === 'spec' ? 'page' : undefined} onclick={() => tab = 'spec'}>{t("Сведения о системе")}</button>
        <button class:active={tab === 'sensors'} aria-current={tab === 'sensors' ? 'page' : undefined} onclick={() => tab = 'sensors'}>{t("Датчики")}</button>
        <button class:active={tab === 'settings'} aria-current={tab === 'settings' ? 'page' : undefined} onclick={() => tab = 'settings'}>{t("Настройки")}</button>
      </div>
    </nav>
    <div id="connection-status" class="connection" aria-live="polite" aria-atomic="true"><span class="connection-dot" class:online={connected} aria-hidden="true"></span>{#key connected}<p>{connected ? t("Daemon подключён") : t("Нет связи с daemon")}</p>{/key}</div>
    <p class="aside-note">{t("Настройки применяются только кнопкой. Закрытие окна не останавливает экран.")}</p>
  </aside>
  <main id="main-content" tabindex="-1">
    <header><div><h1>{tab === 'overview' ? t("Ваши устройства") : tab === 'display' ? t("Экран помпы") : tab === 'cooling' ? t("Охлаждение") : tab === 'lighting' ? t("Подсветка") : tab === 'settings' ? t("Настройки") : tab === 'spec' ? t("Сведения о системе") : t("Датчики")}</h1><p class="page-description">{['display','cooling','lighting'].includes(tab) ? selectedDevice?.display_name ?? t("Нет устройства") : tab === 'overview' ? t("Настройте свет, охлаждение и оформление своего компьютера.") : t("L-Connect Linux")}</p></div>
      {#if tab !== 'overview'}<label class="device-picker">{t("Устройство")}<select aria-label={t("Устройство")} value={device} onchange={e => selectDevice(e.currentTarget.value)} disabled={busy || !connected || !devices.length}><option value="">{t("Нет устройства")}</option>{#if device && !devices.some(d => d.id === device)}<option value={device}>{t("Выбранное устройство недоступно")}</option>{/if}{#each devices as d (d.id)}<option value={d.id}>{d.display_name}</option>{/each}</select></label>{/if}
    </header>
    <div hidden={tab !== 'settings' || !capabilities.display}><FullGroupArchive {device} {connected} {sensors} fonts={fontChoices} geometry={capabilities.display} disabled={actionBusy||resetBusy||!deviceAvailable} onbusy={value=>groupBusy=value} onchange={target=>{collectionEpoch++;resetEpoch++;if(target===device){lcdRevision++;void refresh();}}}/></div>

    <div hidden={tab !== 'settings'}><DisplaySettings {connected}/></div>
    <div hidden={tab !== 'sensors'}><SensorDefaultsEditor {connected} active={tab === 'sensors'} /></div>
    <div id="persistence-status" aria-live="polite" aria-atomic="true">
      {#if lcd?.persistence && !lcd.persistence.last_error}
        {#if lcd.persistence.pending}<p class="notice">{t("Настройки ожидают записи на диск.")}</p>
        {:else if lcd.persistence.saved}<p class="notice">{t("Последняя запись настроек завершена.")}</p>
        {/if}
      {/if}
    </div>
    {#if lcd?.persistence?.last_error}<p role="alert">{t("Не удалось сохранить настройки:")} {lcd.persistence.last_error}{t(". Daemon повторит запись.")}</p>{/if}
    {#if connectionError}<div class="error" role="alert">{t('Не удалось подключиться к daemon: {0}', connectionError)}<button onclick={() => refresh()}>{t("Повторить подключение")}</button></div>{/if}
    {#if error}<div class="error" role="alert">{errorText(error)}</div>{/if}
    {#if connected && device && !deviceAvailable}<p role="status">{t("Выбранное устройство недоступно. Черновик сохранён; выберите устройство явно или дождитесь подключения.")}</p>{/if}
    {#if notice}<p class="notice" role="status">{t(notice)}</p>{/if}
    {#if lightingVisited}<div hidden={tab !== 'lighting'}><Lighting {device} connected={deviceAvailable && capabilities.lighting && tab === 'lighting'} {sensors}/></div>{/if}
    {#if tab === 'overview'}
      <DeviceOverview {devices} {connected} disabled={busy} onopen={openDevice}/>
    {:else if ['display','cooling','lighting'].includes(tab) && selectedDevice && !capabilities[tab === 'display' ? 'display' : tab === 'cooling' ? 'cooling' : 'lighting']}
      <p>{t('Для этого семейства пока нет доступных редакторов.')}</p>
    {:else if tab === 'spec'}
      <SystemSpec {connected}/>
    {:else if tab === 'settings'}
      <DesktopSettings/>
      <FpsSettings {connected}/>
      {#key connected}<FirmwareRecovery/>{/key}
      {#if capabilities.display}<Settings {device} connected={deviceAvailable} {lcd}/>{/if}
      {#if capabilities.display}{#key `${selectionRevision}:${device}:${deviceAvailable}`}<FirmwareInfo {device} connected={deviceAvailable}/>{/key}{/if}
    {:else if tab === 'lighting'}
      <!-- The mounted studio above retains device-scoped RGB drafts. -->
    {:else if tab === 'cooling'}
      <Cooling {device} {sensors} connected={deviceAvailable}/>
    {:else if tab === 'sensors'}
      <section class="sensors" aria-label="{t("Показания датчиков")}">{#each sensors as sensor (sensor.id)}
        <article><h2>{sensor.label}</h2><strong>{formatSensor(sensor,presentation.settings?.temperature_unit)}</strong><p>{sensor.status} · {sensor.age_ms ?? '—'} ms</p><code>{sensor.id}</code></article>
      {/each}</section>
      {#if !sensors.length}<p>{t("Показания появятся после подключения daemon.")}</p>{/if}
    {:else}
      <div class="workspace">
        <section class="visual" class:canvas-mode={canvasEditing} aria-label="{t("Предпросмотр LCD")}">
          <LcdPreview geometry={capabilities.display}>{#if canvasEditing}<CanvasEditor image={preview} keys={canvasLayerKeys} layers={theme.layers} bounds={canvasBounds} selected={selectedLayer} rotation={canvasRotation} ready={canvasDraft===previewDraft && connected} {labels} onselect={selectCanvasLayer} onchange={changeCanvasLayer} onorder={move}/>{:else if preview}<img src={preview} alt="{t("Подготовленная композиция LCD — preview, не подтверждение вывода на устройство")}"/>{:else}<div class="empty"><span>{capabilities.display?.width ?? 480} × {capabilities.display?.height ?? 480}</span><p>{t("Создайте тему")}<br/>{t("и обновите preview")}</p></div>{/if}</LcdPreview>
          <div class="status"><span>{t("Источник:")} {lcdSourceLabel(lcd?.source)}</span><span>{lcd?.last_error ?? t('Preview не подтверждает физический вывод')}</span></div>
          <label><input type="checkbox" bind:checked={canvasEditing}/>{t('Редактировать на холсте')}</label>
          {#if canvasEditing}
            <details><summary>{t('Управление холстом')}</summary>
            <p>{t('Стрелки — перемещение; Shift + стрелки — размер; Ctrl + стрелки — шаг 10; Alt + стрелки — порядок. Escape — отмена жеста.')}</p>
            <p>{t('Размер текста меняет шрифт и текстовую рамку. Изображения масштабируются пропорционально. Фон и эффекты неподвижны.')}</p></details>
            <label>{t('Выбранный слой')}<select aria-label={t('Выбранный слой')} value={selectedLayer} onchange={event=>selectCanvasLayer(Number(event.currentTarget.value))}>
              <option value={-1}>{t('Выберите слой')}</option>
              {#each theme.layers as layer,index}<option value={index}>{index+1}. {labels[layer.kind]}{editableLayer(layer)?'':` (${t('Неподвижный фон')})`}</option>{/each}
            </select></label>
            <button disabled={selectedLayer<0} onclick={()=>void focusLayer(selectedLayer)}>{t('Свойства выбранного слоя')}</button>
          {/if}
          <div class="actions"><button disabled={!undoDrafts.length} onclick={()=>restoreDraft()}>{t('Отменить правку')}</button><button disabled={!redoDrafts.length} onclick={()=>restoreDraft(true)}>{t('Повторить правку')}</button></div>
          <label><input type="checkbox" bind:checked={livePreview}/>{t("Непрерывный preview")}</label>
          {#if previewError}<p role="alert">{t('Не удалось создать preview: {0}', previewError)}</p>{/if}
          <div class="actions"><button disabled={previewBusy || !connected} onclick={() => {notice='';previewRequest++;}}>{t("Обновить preview")}</button><button class="primary" disabled={busy || !deviceAvailable} onclick={applyTheme}>{t("Применить к LCD")}</button></div>
          <button disabled={busy || !deviceAvailable} onclick={() => loadTheme()}>{t("Загрузить активную тему")}</button>
          <button disabled={busy || !deviceAvailable} onclick={() => loadTheme(true)}>{t("Загрузить импортированный редактор")}</button>
          <button disabled={busy || !deviceAvailable} onclick={clearTheme}>{t("Вернуть LocalUI")}</button>
        </section>
        <section class="editor" aria-label="{t("Редактор темы")}" bind:this={editorElement}>
          <div class="row"><label>{t("Название")}<input bind:value={theme.name} maxlength="128"/></label></div>
          <Profiles refreshEpoch={resetEpoch} group="screen" mediaKey={profileMediaKey} {connected} disabled={busy} draft={() => { const draft=$state.snapshot(theme);assertWeatherDraft(draft);return {kind:'screen',value:draft}; }} onload={content => { if (content.kind === 'screen_media') { lcdPanel = 'composition'; loadedMediaKey = content.value; notice = 'Ролик из профиля загружен. Применение — отдельное действие.'; } else if (content.kind === 'screen') { loadedMediaKey = ''; theme = content.value; notice = ''; if (preview) URL.revokeObjectURL(preview); preview = ''; } }}/>
          <div class="editor-tabs" aria-label={t('Рабочая область LCD')}><button aria-pressed={lcdPanel === 'presets'} onclick={() => lcdPanel = 'presets'}>{t('Готовые темы')}</button><button aria-pressed={lcdPanel === 'composition'} onclick={() => lcdPanel = 'composition'}>{t('Композиция')}</button><button aria-pressed={lcdPanel === 'library'} onclick={() => lcdPanel = 'library'}>{t('Библиотека')}</button></div>
          <div data-studio-panel="presets" hidden={lcdPanel !== 'presets'}>
          <section class="preset-picker"><h2>{t("Готовые темы — редактируемые раскладки")}</h2>
            <label>{t("Раскладка")}<select aria-label="{t("Раскладка")}" bind:value={presetId} onchange={() => presetBindings = []}>{#each presets as preset}<option value={preset.id}>{preset.name} · {t(preset.description)}</option>{/each}</select></label>
            {#if presetRequired}<p>{t("Выберите")} {presetRequired} {t("датчиков. Порядок выбора задаёт расположение.")}</p><div class="preset-sensors">{#each sensors as sensor (sensor.id)}<label><input type="checkbox" checked={presetBindings.includes(sensor.id)} onchange={e => { presetBindings = e.currentTarget.checked ? [...presetBindings, sensor.id] : presetBindings.filter(id => id !== sensor.id); }} disabled={!presetBindings.includes(sensor.id) && presetBindings.length >= presetRequired}/>{sensor.label} · {sensor.unit}</label>{/each}</div>{/if}
            <button disabled={presetBindings.length !== presetRequired || busy} onclick={() => { try { theme = presetTheme(presetId, presetBindings.map(id => { const sensor = sensors.find(s => s.id === id); if (!sensor) throw new Error('Датчик исчез из каталога'); return sensor; })); lcdPanel = 'composition'; notice = 'Раскладка заменила локальный черновик. Настройте её и нажмите Apply для отправки.'; error = ''; } catch (e) { error = message(e); } }}>{t("Создать черновик из раскладки")}</button>
          </section>
          </div>
          <div data-studio-panel="library" hidden={lcdPanel !== 'library'}>
          <FontImport {connected} onimport={font => { importedFonts = [...importedFonts.filter(f => f.key !== font.key), font]; }}/>
          <AssetCleanup kind="font" {connected} disabled={busy} keep={fontChoices.map(font => font.key)}/>
          <ScreenCollections refreshEpoch={resetEpoch} {device} connected={deviceAvailable} disabled={busy} onchange={() => collectionEpoch++}/>
          <ImageGallery onedit={publishGalleryEdit} {device} {collectionEpoch} connected={deviceAvailable} disabled={busy} addDisabled={theme.layers.length >= 32} keep={draftImageKeys} onadd={(asset, context) => {
            if (busy) return;
            if (context === 'background') {
              theme = { ...theme, version: isProcedural(theme.layers[0])?3:1, background_video: undefined, background_image: {asset, fit:'crop'} };
              loadedMediaKey = '';
              notice = 'Изображение выбрано фоном черновика. Слои сохранены; примените тему для вывода на LCD.';
              return;
            }
            if (theme.layers.length >= 32) return;
            if (context === 'watermark') {
              theme.layers.push({kind:'image', asset, x:0, y:0, width:144, height:144, fit:'contain_native', opacity:255, watermark_position:{horizontal:50,vertical:50}});
              notice = 'Водяной знак добавлен в черновик темы.';
            } else {
              prependLayer({ kind:'image', asset, x:0, y:0, width:480, height:480, fit:'crop', opacity:255 });
              notice = 'Сохранённое изображение добавлено в черновик темы.';
            }
          }}/>
          <AssetCleanup {connected} disabled={busy} keep={draftImageKeys}/>
          <MediaLibrary keep={draftVideoKeys} {device} {collectionEpoch} {connected} available={deviceAvailable} disabled={busy} onselect={selectMedia} onstop={clearTheme} onbackground={key => {
            theme = { ...theme, version: isProcedural(theme.layers[0])?3:2, background_image: undefined, background_video: key };
            loadedMediaKey = '';
            notice = 'Видео добавлено в черновик фоном. Слои сохранены; примените тему для запуска на LCD.';
          }} onchoice={key => profileMediaKey = key} ondeleted={key => { if (loadedMediaKey === key) loadedMediaKey = ''; }} />

          </div>
          <div data-studio-panel="composition" hidden={lcdPanel !== 'composition'}>
          {#if loadedMediaKey}<section aria-label="{t("Ролик из профиля")}"><MediaThumbnail clipKey={loadedMediaKey} name={t('Ролик из профиля')}/><button disabled={busy || !deviceAvailable} onclick={() => selectMedia(loadedMediaKey)}>{t("Применить ролик из профиля")}</button><button onclick={() => loadedMediaKey = ''}>{t("Закрыть ролик из профиля")}</button></section>{/if}<BasicEditor bind:theme disabled={busy} onchange={() => loadedMediaKey = ''}/><WeatherEditor bind:theme {sensors} disabled={busy}/><PaletteEditor bind:theme {sensors} disabled={busy}/><BackgroundEditor bind:theme/>
          {#key device}<ImageEditor disabled={busy || !deviceAvailable || theme.layers.length >= 32} onpublish={publishEditedImage}/>{/key}
          <div class="toolbar"><button disabled={busy || theme.layers.length >= 32} onclick={() => theme.layers.push({kind:'divider',axis:'horizontal',x:34,y:240,length:412,thickness:2,color:[144,177,201],opacity:100,shadow:{x:0,y:0,blur:5,color:[0,0,0],opacity:100}})}>{t('+ Разделитель')}</button><button data-add-text disabled={theme.layers.length >= 32} onclick={() => theme.layers.push({ kind: 'text', text: t('Новый текст'), style: style(180) })}>{t("+ Текст")}</button><button disabled={theme.layers.length >= 32} onclick={() => theme.layers.push({ kind: 'clock', twenty_four_hour: true, show_seconds: true, style: style(300) })}>{t("+ Часы")}</button><label class="file">{t("+ Изображение")}<input type="file" accept="image/png,image/jpeg,image/webp,image/bmp" onchange={importImage} disabled={busy || theme.layers.length >= 32}/></label></div>
          <div class="row"><label>{t("Привязать датчик")}<select bind:value={selectedSensor}><option value="">{t("Выберите источник")}</option>{#each sensors as s}<option value={s.id}>{s.label} · {s.unit}</option>{/each}</select></label><button disabled={!selectedSensor || theme.layers.length >= 32} onclick={addSensor}>{t("Добавить")}</button></div>
          {#each theme.layers as layer, i (layer)}
            <article class="layer" class:canvas-selected={canvasEditing && selectedLayer===i} onfocusin={()=>selectedLayer=i} aria-labelledby={`layer-heading-${i}`}><div class="layer-head"><h2 id={`layer-heading-${i}`} tabindex="-1">{i + 1}. {labels[layer.kind]}</h2><div><button aria-label="{t("Слой ниже")}" disabled={i === 0 || (i === 1 && isProcedural(theme.layers[0]))} onclick={() => move(i, -1)}>↑</button><button aria-label="{t("Слой выше")}" disabled={isProcedural(layer) || i === theme.layers.length - 1} onclick={() => move(i, 1)}>↓</button><button aria-label="{t("Удалить слой")}" onclick={() => removeLayer(i)}>×</button></div></div>
              {#if layer.kind==='divider'}<DividerEditor bind:layer={()=>layer,value=>theme.layers[i]=value}/>{/if}
              {#if layer.kind==='meteor'}<p>{t("Настройки MeteorShower находятся в блоке «Метеоры».")}</p>{/if}
              {#if layer.kind==='weather'}<p>{t("Настройки Rain/Snow находятся в блоке «Анимация LCD».")}</p>{/if}
              {#if layer.kind === 'text'}<label>{t("Текст")}<textarea bind:value={layer.text} maxlength="2048"></textarea></label>{/if}
              {#if layer.kind === 'sensor'}<label>{t('Источник датчика')}<select aria-label={t('Источник датчика')} value={layer.sensor_id} onchange={event=>{const source=sensors.find(s=>s.id===event.currentTarget.value);if(source){layer.sensor_id=source.id;layer.unit=source.unit;}}}>
                {#each sensors as source}<option value={source.id}>{source.label} · {source.unit}</option>{/each}
                {#if !sensors.some(source=>source.id===layer.sensor_id)}<option value={layer.sensor_id}>{t('Недоступен:')} {layer.sensor_id}</option>{/if}
              </select></label><label>{t("Подпись")}<input bind:value={layer.label}/></label><code>{layer.sensor_id}</code><div class="row"><label>{t("Точность")}<input type="number" bind:value={layer.decimals} min="0" max="2"/></label>{#if layer.unit === 'celsius'}<label>{t("Единица")}<select bind:value={layer.temperature_unit}><option value="system">{t("Общие настройки")}</option><option value="celsius">°C</option><option value="fahrenheit">°F</option></select></label>{/if}</div>{/if}
              {#if layer.kind === 'sensor_cycle'}
                <label>{t("Интервал переключения, с")}<input type="number" min="1" max="3600" bind:value={layer.interval_seconds}/></label>
                <button disabled={layer.sensors.length>=16} onclick={()=>layer.sensors.push({...layer.sensors[layer.sensors.length-1]})}>{t("Добавить показатель в цикл")}</button>
                {#each layer.sensors as sensor,j}
                  <button disabled={layer.sensors.length===1} onclick={event=>removeCycleSensor(layer,j,event.currentTarget)}>{t("Удалить показатель")} {j+1}</button>
                  <label>{t("Источник")} {j+1}<select data-cycle-source value={sensor.sensor_id} onchange={event=>{const selected=sensors.find(s=>s.id===event.currentTarget.value);if(selected){sensor.sensor_id=selected.id;sensor.unit=selected.unit;}}}>
                    {#each sensors as source}<option value={source.id}>{source.label} · {source.unit}</option>{/each}
                    {#if !sensors.some(s=>s.id===sensor.sensor_id)}<option value={sensor.sensor_id}>{t("Недоступен:")} {sensor.sensor_id}</option>{/if}
                  </select></label>
                  <label>{t("Подпись")} {j+1}<input bind:value={sensor.label}/></label>
                  <label>{t("Точность")} {j+1}<input type="number" min="0" max="2" bind:value={sensor.decimals}/></label>
                  {#if sensor.unit === 'celsius'}<label>{t("Единица")} {j+1}<select bind:value={sensor.temperature_unit}><option value="system">{t("Общие настройки")}</option><option value="celsius">°C</option><option value="fahrenheit">°F</option></select></label>{/if}
                {/each}
              {/if}
              {#if layer.kind === 'sensor' || layer.kind === 'sensor_cycle'}
                <label><input type="checkbox" checked={!!layer.parts} onchange={e => {layer.parts=e.currentTarget.checked?{label:{...$state.snapshot(layer.style),y:Math.max(0,layer.style.y-45),size:22},unit:{...$state.snapshot(layer.style),y:Math.min(479,layer.style.y+65),size:22}}:null;}}/>{t("Раздельные стили подписи и единицы")}</label>
                {#if layer.parts}
                  {#each ['label','unit'] as part (part)}
                    <fieldset><legend>{part === 'label' ? "" + t("Подпись датчика") + "" : "" + t("Единица датчика") + ""}</legend>
                      <TextStyleControls bind:style={layer.parts[part as 'label'|'unit']} fonts={fontChoices}/>
                    </fieldset>
                  {/each}
                {/if}
              {/if}
              {#if layer.kind === 'clock'}<label>{t("Содержимое часов")}<select aria-label="{t("Содержимое часов")}" value={layer.format ?? 'time'} onchange={e => {layer.format=e.currentTarget.value as typeof layer.format;}}><option value="time">{t("Время")}</option><option value="time_without_period">{t("Время без AM/PM")}</option><option value="date">{t("Дата YYYY/MM/DD")}</option><option value="period">A.M. / P.M.</option></select></label><div class="row"><label><input type="checkbox" bind:checked={layer.twenty_four_hour}/>{t("24 часа")}</label><label><input type="checkbox" bind:checked={layer.show_seconds}/>{t("Секунды")}</label></div>{/if}
              {#if layer.kind === 'image'}
                <label><input type="checkbox" checked={!!layer.watermark_position} onchange={e => layer.watermark_position=e.currentTarget.checked?{horizontal:50,vertical:50}:null}/>{t("Положение watermark как в L-Connect")}</label>
                {#if layer.watermark_position}
                  <p>{t("Центр изображения задаётся в процентах экрана. Значение 0 размещает центр в 22 px от края, как в L-Connect.")}</p>
                  <label>{t("Watermark по горизонтали")}<input type="number" min="0" max="100" bind:value={layer.watermark_position.horizontal}/></label>
                  <label>{t("Watermark по вертикали")}<input type="number" min="0" max="100" bind:value={layer.watermark_position.vertical}/></label>
                {/if}
                <label>{t("Масштаб")}<select bind:value={layer.fit}><option value="crop">{t("Заполнить с обрезкой")}</option><option value="contain">{t("Вписать")}</option><option value="contain_native">{t("Вписать без увеличения (watermark)")}</option><option value="stretch">{t("Растянуть")}</option></select></label>
                <label><input type="checkbox" checked={!!layer.shadow} onchange={e => layer.shadow=e.currentTarget.checked?{x:2,y:2,blur:2,color:[0,0,0],opacity:60}:null}/>{t("Тень изображения")}</label>
                {#if layer.shadow}
                  <div class="row">
                    <label>{t("Смещение тени изображения X")}<input type="number" min="-32" max="32" bind:value={layer.shadow.x}/></label>
                    <label>{t("Смещение тени изображения Y")}<input type="number" min="-32" max="32" bind:value={layer.shadow.y}/></label>
                    <label>{t("Размытие тени изображения")}<input type="number" min="0" max="16" bind:value={layer.shadow.blur}/></label>
                    <label>{t("Непрозрачность тени изображения, %")}<input type="number" min="0" max="100" bind:value={layer.shadow.opacity}/></label>
                    <label>{t("Цвет тени изображения")}<input type="color" value={hex(layer.shadow.color)} oninput={e=>{if(layer.shadow)layer.shadow.color=rgb(e.currentTarget.value);}}/></label>
                  </div>
                {/if}
                <label>{t("Непрозрачность")}<input type="range" min="0" max="255" bind:value={layer.opacity}/></label>
                <div class="row">{#each ['x', 'y', 'width', 'height'] as key}<label>{key}<input type="number" disabled={!!layer.watermark_position && (key === 'x' || key === 'y')} min="0" max="480" value={layer[key as 'x' | 'y' | 'width' | 'height']} oninput={e => { if (layer.kind === 'image') layer[key as 'x' | 'y' | 'width' | 'height'] = Number(e.currentTarget.value); }}/></label>{/each}</div>
              {:else if layer.kind !== 'divider' && layer.kind !== 'weather' && layer.kind !== 'meteor' && layer.kind !== 'snow3d' && layer.kind !== 'fireworks'}
                <TextStyleControls bind:style={layer.style} fonts={fontChoices}/>
              {/if}
            </article>
          {/each}
          </div>
        </section>
      </div>
    {/if}
    <div hidden={tab !== 'display' || !capabilities.display}><ScreenReset {device} connected={deviceAvailable} active={tab === 'display'} disabled={actionBusy} onbusy={value => resetBusy = value} onchange={target => { collectionEpoch++; resetEpoch++; if (target === device) { lcdRevision++; void refresh(); } }}/></div>
  </main>
</div>
<style>
  .visual.canvas-mode{max-height:calc(100vh - 48px);overflow:auto;grid-auto-rows:max-content;align-content:start;}
  @media(max-width:760px){.visual.canvas-mode{max-height:none;overflow:visible;}}
  .canvas-selected{outline:2px solid #7ce9ff;outline-offset:3px;}
.shell{display:grid;grid-template-columns:230px minmax(0,1fr);min-height:100vh}aside{padding:30px 18px;background:var(--sidebar);border-right:1px solid var(--border);display:flex;flex-direction:column}.brand{font-family:Exo,sans-serif;font-size:24px;letter-spacing:-.025em;margin:0 12px 36px}.brand span{color:var(--accent)}.brand small{display:block;font-size:11px;letter-spacing:.12em;color:var(--text-muted);margin-top:8px}nav,.global-nav{display:grid;gap:6px}nav button{text-align:left;background:transparent;border-color:transparent}.nav-label{font-size:12px;color:var(--text-muted);margin:24px 12px 8px}.device-section{padding-left:24px;font-size:13px}nav button.active{background:var(--accent-surface);color:var(--accent-text)}.global-nav{border-top:1px solid var(--border);padding-top:20px;margin-top:20px}.connection{display:flex;align-items:center;gap:8px;margin:38px 10px 0;font-size:12px}.connection p{margin:0}.connection-dot{width:6px;height:6px;border-radius:50%;background:var(--danger)}.connection-dot.online{background:var(--success)}.aside-note,.disconnected-device{font-size:12px;line-height:1.7;color:var(--text-muted);padding:0 10px}.aside-note{margin-top:14px}main{padding:36px 40px;min-width:0;max-width:1700px;width:100%;margin-inline:auto}header{display:flex;gap:24px;align-items:center;justify-content:space-between;margin-bottom:30px;padding-bottom:24px;border-bottom:1px solid var(--border)}h1{font-size:32px;margin:0 0 10px}.page-description{margin:0;color:var(--text-muted);line-height:1.6}.device-picker{max-width:270px}.workspace{display:grid;grid-template-columns:minmax(280px,.9fr) minmax(360px,1.1fr);gap:32px}.visual{position:sticky;top:24px;align-self:start;display:grid;gap:16px;padding:24px;background:var(--canvas);border-radius:14px}.empty{text-align:center;color:var(--text-muted)}.empty span{font-family:Exo,sans-serif;font-size:28px}.status{display:grid;gap:6px;color:var(--text-muted);font-size:12px;line-height:1.5;overflow-wrap:anywhere}.actions,.row,.toolbar{display:flex;gap:12px;align-items:end;flex-wrap:wrap;margin-bottom:12px}.editor{min-width:0}.editor-tabs{display:flex;gap:8px;border-bottom:1px solid var(--border);padding-bottom:14px;margin-bottom:20px}.editor-tabs button[aria-pressed=true]{background:var(--accent-surface);color:var(--accent-text);border-color:var(--accent)}.layer{border-top:1px solid var(--border);padding:24px 0;margin-top:24px;display:grid;gap:14px}.layer-head{display:flex;align-items:center;justify-content:space-between;gap:10px}.layer-head button{padding:5px 9px;margin-left:4px}.layer-head h2{margin:0}.file{border:1px solid var(--border-strong);border-radius:8px;padding:10px;cursor:pointer}.file input{max-width:160px;font-size:12px}.error{background:var(--danger-surface);border:1px solid var(--danger);padding:16px;border-radius:8px;margin-bottom:16px;overflow-wrap:anywhere}.error button{margin-left:12px}.notice{color:var(--success);line-height:1.6}.sensors{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:0 24px}.sensors article{border-top:1px solid var(--border);padding:24px 0;min-width:0}.sensors strong{font-size:30px;font-variant-numeric:tabular-nums}.sensors p{color:var(--text-muted);font-size:12px}.skip-link{position:fixed;top:12px;left:12px;z-index:100;background:var(--accent);color:var(--accent-ink);padding:12px 18px;border-radius:8px;transform:translateY(-200%)}.skip-link:focus{transform:translateY(0)}
@media(max-width:1100px){.shell{grid-template-columns:190px minmax(0,1fr)}main{padding:24px}.workspace{grid-template-columns:minmax(0,1fr)}.visual{position:static;max-width:560px;width:100%;justify-self:center}}
@media(max-width:760px){.shell{grid-template-columns:minmax(0,1fr)}aside{padding:16px;border-right:0;border-bottom:1px solid var(--border)}.brand{margin-bottom:16px}nav{grid-template-columns:repeat(auto-fit,minmax(130px,1fr))}.nav-label,.global-nav{grid-column:1/-1}.global-nav{display:flex;flex-wrap:wrap;margin-top:10px;padding-top:10px}.connection{margin-top:16px}.aside-note{margin-bottom:0}main{padding:16px}header{flex-wrap:wrap;gap:16px}.device-picker{flex-basis:100%;max-width:none}h1{font-size:28px}.layer-head{flex-wrap:wrap}.visual{padding:18px}}
</style>
