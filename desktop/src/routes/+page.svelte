<script lang="ts">
  import StudioIcon from '$lib/StudioIcon.svelte';
  import StudioPopover from '$lib/StudioPopover.svelte';
  import { ValidationError, type UiDiagnostic } from '$lib/validation-error';
  import { t, diagnosticText } from '$lib/i18n.svelte';
  import { syncNativeLanguage } from '$lib/native-language';
  $effect(() => syncNativeLanguage(presentation.language));
  import '$lib/studio.css';
  import DeviceOverview from '$lib/DeviceOverview.svelte';
  import { deviceCapabilities } from '$lib/device-capabilities';
  import LcdPreview from '$lib/LcdPreview.svelte';
  import CanvasEditor from '$lib/CanvasEditor.svelte';
  import LayerInspector from '$lib/LayerInspector.svelte';
  import LayerStack from '$lib/LayerStack.svelte';
  import {
    editableLayer,
    type EditorPreview,
    type LayerBounds,
    type QuarterTurn,
  } from '$lib/canvas-geometry';
  let collectionEpoch = $state(0),
    resetEpoch = $state(0);
  import {
    addImportedToCollection,
    imageReplacementSnapshot,
    replaceCollectionImage,
    ImportedImageMembershipError,
  } from '$lib/collection-membership';
  import ImageGallery from '$lib/ImageGallery.svelte';
  import ImageEditor from '$lib/ImageEditor.svelte';
  import MediaThumbnail from '$lib/MediaThumbnail.svelte';
  import FontImport from '$lib/FontImport.svelte';
  import type { FontChoice } from '$lib/font-upload';
  let importedFonts = $state<FontChoice[]>([]);
  import { themePages, switchLcdPage, isProcedural } from '$lib/lcd-pages';
  import { assertWeatherDraft } from '$lib/weather-settings';
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
  import { presentation } from '$lib/presentation.svelte';
  import { presets, presetTheme, formatSensor, type PresetId } from '$lib/theme-presets';
  import Cooling from '$lib/Cooling.svelte';
  import { onMount, untrack, tick } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import {
    rpc,
    message,
    importImageAsset,
    style,
    initialTheme,
    type Device,
    type Sensor,
    type Theme,
    type Layer,
    type LcdState,
  } from '$lib/api';
  $effect(() => {
    document.documentElement.lang = presentation.language === 'en-US' ? 'en' : 'ru';
  });
  let devices = $state<Device[]>([]),
    sensors = $state<Sensor[]>([]);
  let device = $state(''),
    tab = $state<'display' | 'sensors' | 'cooling' | 'lighting' | 'settings' | 'spec' | 'overview'>(
      'overview',
    );
  let lcdPanel = $state<'composition' | 'library' | 'presets'>('composition');
  let lightingVisited = $state(false);
  $effect(() => {
    if (tab === 'lighting') lightingVisited = true;
  });
  let lastDevice = $state<Device | undefined>();
  const selectedDevice = $derived(
    devices.find((d) => d.id === device) ?? (lastDevice?.id === device ? lastDevice : undefined),
  );
  const capabilities = $derived(deviceCapabilities(selectedDevice));
  function openDevice(id: string, section: 'display' | 'lighting' | 'cooling' | 'settings') {
    selectDevice(id);
    tab = section;
  }
  let profileMediaKey = $state(''),
    loadedMediaKey = $state('');
  let theme = $state<Theme>(initialTheme(t('My screen'))),
    lcd = $state<LcdState | null>(null);
  function lcdSourceLabel(source: string | null | undefined): string {
    switch (source) {
      case 'Idle':
        return t('No software stream');
      case 'Jpeg':
        return t('Image');
      case 'DiagnosticClip':
        return t('Video');
      case 'Theme':
        return t('LCD theme');
      default:
        return source ?? '—';
    }
  }
  const draftVideoKeys = $derived([
    ...new Set(
      themePages(theme).flatMap((page) => (page.background_video ? [page.background_video] : [])),
    ),
  ]);
  const draftImageKeys = $derived([
    ...new Set(
      themePages(theme).flatMap((page) => [
        ...(page.background_image ? [page.background_image.asset] : []),
        ...page.layers.filter((layer) => layer.kind === 'image').map((layer) => layer.asset),
      ]),
    ),
  ]);
  const fontChoices = $derived.by(() => {
    const fonts = new SvelteMap(importedFonts.map((font) => [font.key, font]));
    for (const layer of themePages(theme).flatMap((page) => page.layers)) {
      if (
        layer.kind === 'divider' ||
        layer.kind === 'image' ||
        layer.kind === 'weather' ||
        layer.kind === 'meteor' ||
        layer.kind === 'snow3d' ||
        layer.kind === 'fireworks'
      )
        continue;
      const styles = [layer.style];
      if ((layer.kind === 'sensor' || layer.kind === 'sensor_cycle') && layer.parts)
        styles.push(layer.parts.label, layer.parts.unit);
      for (const style of styles) {
        if (style.font_asset && !fonts.has(style.font_asset))
          fonts.set(style.font_asset, {
            key: style.font_asset,
            name: t('Custom ({0})', style.font_asset.slice(0, 8)),
          });
      }
    }
    return [...fonts.values()];
  });
  type LcdOperation =
    | 'load-imported'
    | 'load'
    | 'apply'
    | 'localui'
    | 'media'
    | 'image-import'
    | 'image-membership'
    | 'image-replacement';
  type LcdFailure = { context: LcdOperation; detail: UiDiagnostic };
  let error = $state<string | LcdFailure>(''),
    notice = $state(''),
    connected = $state(false),
    actionBusy = $state(false),
    resetBusy = $state(false),
    groupBusy = $state(false);
  function errorText(value: string | LcdFailure): string {
    if (typeof value === 'string') return t(value);
    switch (value.context) {
      case 'load-imported':
        return t(
          'Could not load the imported editor: {0}. Your draft was preserved; retry loading explicitly.',
          diagnosticText(value.detail),
        );
      case 'load':
        return t(
          'Could not load the active theme: {0}. Your draft is preserved; retry loading explicitly.',
          diagnosticText(value.detail),
        );
      case 'apply':
        return t(
          'Theme application was not confirmed: {0}. Check the current LCD state before applying again.',
          diagnosticText(value.detail),
        );
      case 'localui':
        return t(
          'LocalUI restoration was not confirmed: {0}. Check the current LCD state before retrying.',
          diagnosticText(value.detail),
        );
      case 'media':
        return t(
          'Video selection was not confirmed: {0}. Check the current LCD state before retrying.',
          diagnosticText(value.detail),
        );
      case 'image-import':
        return t(
          'Image import was not confirmed: {0}. Check the catalog before uploading again.',
          diagnosticText(value.detail),
        );
      case 'image-membership':
        return t(
          'The file was saved to the catalog, but adding it to the collection was not confirmed: {0}. Check the collections before retrying.',
          diagnosticText(value.detail),
        );
      case 'image-replacement':
        return t(
          'Replacement was not confirmed: {0}. Check the catalog and collections before retrying.',
          diagnosticText(value.detail),
        );
    }
  }
  const busy = $derived(actionBusy || resetBusy || groupBusy);
  let connectionError = $state('');
  let selectionRevision = $state(0),
    refreshRevision = 0,
    lcdRevision = 0;
  const deviceAvailable = $derived(connected && devices.some((d) => d.id === device));
  function selectDevice(value: string) {
    lastDevice = devices.find((d) => d.id === value);
    device = value;
    selectionRevision++;
    lcdRevision++;
    lcd = null;
    notice = '';
    error = '';
  }
  let editorImages = $state<string[]>([]),
    editorBackground = $state('');
  const rasterCache = new SvelteMap<string, string>();
  function clearEditorImages() {
    if (editorBackground) URL.revokeObjectURL(editorBackground);
    editorImages = [];
    editorBackground = '';
  }
  let preview = $state(''),
    selectedSensor = $state('');
  let presetId = $state<PresetId>('focus'),
    presetBindings = $state<string[]>([]);
  let presetRequired = $derived(presets.find((p) => p.id === presetId)!.sensors);
  async function refresh() {
    const request = ++refreshRevision;
    let stateSelection: number | undefined;
    try {
      const [nextDevices, nextSensors] = await Promise.all([
        rpc<Device[]>('list_devices'),
        rpc<Sensor[]>('list_sensors'),
      ]);
      if (request !== refreshRevision) return;
      devices = nextDevices;
      sensors = nextSensors;
      connected = true;
      connectionError = '';
      // Retain a missing device selection; never redirect Apply to another device.
      if (!device && devices.length) selectDevice(devices[0].id);
      const target = device,
        selection = selectionRevision,
        revision = lcdRevision;
      stateSelection = selection;
      const state = devices.some((d) => d.id === target && deviceCapabilities(d).display)
        ? await rpc<LcdState>('get_lcd_state', { dev: target })
        : null;
      if (
        request === refreshRevision &&
        selection === selectionRevision &&
        revision === lcdRevision
      )
        lcd = state;
    } catch (e) {
      if (
        request !== refreshRevision ||
        (stateSelection !== undefined && stateSelection !== selectionRevision)
      )
        return;
      // Invalidate operations from the old connection, even if the device ID survives.
      if (connected) {
        selectionRevision++;
        lcdRevision++;
        notice = '';
        error = '';
      }
      connected = false;
      lcd = null;
      sensors = sensors.map((s) => ({ ...s, value: null, status: 'unavailable' }));
      connectionError = message(e);
    }
  }
  async function loadTheme(imported = false) {
    const target = device,
      selection = selectionRevision,
      draft = JSON.stringify(theme);
    await action(
      async () => {
        const saved = await rpc<Theme | null>(
          imported ? 'get_imported_lcd_editor' : 'get_lcd_theme',
          { dev: target },
        );
        if (selection !== selectionRevision) return;
        if (draft !== JSON.stringify(theme)) {
          notice = 'The draft changed. The loaded theme did not replace your edits.';
          return;
        }
        if (!saved) {
          error = imported
            ? 'No imported basic editor is available to load.'
            : 'There is no active theme on the device';
          return;
        }
        theme = saved;
        selectedLayer = -1;
        loadedMediaKey = '';
        if (imported)
          notice = 'Imported editor loaded into the draft. Applying is a separate action.';
      },
      () => selection === selectionRevision,
      imported ? 'load-imported' : 'load',
    );
  }
  async function selectMedia(key: string) {
    const target = device,
      selection = selectionRevision;
    lcdRevision++;
    await action(
      async () => {
        const result = await rpc<LcdState>('set_lcd_media', { dev: target, key });
        if (selection !== selectionRevision) return;
        lcdRevision++;
        lcd = result;
        notice = 'Video selected. Settings are queued for saving.';
      },
      () => selection === selectionRevision,
      'media',
    );
  }
  async function applyTheme() {
    const target = device,
      selection = selectionRevision,
      draft = $state.snapshot(theme),
      displayPatch = { ...pendingDisplay[device] };
    lcdRevision++;
    await action(
      async () => {
        // Local draft validation is app-owned UI text; daemon errors remain data.
        try {
          assertWeatherDraft(draft);
        } catch (e) {
          error = message(e);
          return;
        }
        let result = await rpc<LcdState>('set_lcd_theme', { dev: target, theme: draft });
        if (Object.keys(displayPatch).length) {
          if (selection !== selectionRevision) return;
          try {
            result = await rpc<LcdState>('patch_lcd_config', { dev: target, patch: displayPatch });
            const pending = pendingDisplay[target];
            if (
              pending &&
              pending.rotation_degrees === displayPatch.rotation_degrees &&
              pending.brightness_percent === displayPatch.brightness_percent
            )
              delete pendingDisplay[target];
          } catch (cause) {
            try {
              const state = await rpc<LcdState>('get_lcd_state', { dev: target });
              if (selection === selectionRevision) lcd = state;
            } catch {
              /* Keep the pending draft when readback is unavailable. */
            }
            throw new ValidationError('Theme sent, but display settings are unconfirmed: {0}', [
              message(cause),
            ]);
          }
        }
        if (selection !== selectionRevision) return;
        lcdRevision++;
        lcd = result;
        notice = 'Theme sent. Settings are queued for saving.';
      },
      () => selection === selectionRevision,
      'apply',
    );
  }
  async function clearTheme() {
    const target = device,
      selection = selectionRevision;
    lcdRevision++;
    await action(
      async () => {
        await rpc('clear_lcd', { dev: target });
        if (selection !== selectionRevision) return;
        lcdRevision++;
        await refresh();
        if (selection === selectionRevision)
          notice = 'LocalUI requested. Automatic theme and video restoration is disabled.';
      },
      () => selection === selectionRevision,
      'localui',
    );
  }
  onMount(() => {
    if (new URLSearchParams(location.search).get('view') === 'display') tab = 'display';
    const visibility = () => (windowVisible = !document.hidden);
    visibility();
    document.addEventListener('visibilitychange', visibility);
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      await refresh();
      if (!stopped) timer = setTimeout(poll, 1500);
    }
    void poll();
    return () => {
      document.removeEventListener('visibilitychange', visibility);
      clearEditorImages();
      for (const url of rasterCache.values()) URL.revokeObjectURL(url);
      rasterCache.clear();
      stopped = true;
      clearTimeout(timer);
      if (preview) URL.revokeObjectURL(preview);
    };
  });
  async function action(run: () => Promise<void>, current: () => boolean, context: LcdOperation) {
    actionBusy = true;
    error = '';
    notice = '';
    try {
      await run();
    } catch (e) {
      if (current())
        error = {
          context: e instanceof ImportedImageMembershipError ? 'image-membership' : context,
          detail:
            e instanceof ImportedImageMembershipError
              ? e.detail
              : e instanceof ValidationError
                ? e
                : message(e),
        };
    } finally {
      actionBusy = false;
    }
  }
  let windowVisible = $state(true);
  let animationOrigin = performance.now(),
    animationElapsed = 0;
  function toggleAnimation() {
    if (livePreview) animationElapsed += performance.now() - animationOrigin;
    else animationOrigin = performance.now();
    livePreview = !livePreview;
  }
  let livePreview = $state(true),
    previewRequest = $state(0),
    previewError = $state('');
  const canvasEditing = true;
  let canvasBounds = $state<(LayerBounds | null)[]>([]),
    canvasDraft = $state(''),
    selectedLayer = $state(-1);
  let propertiesOpen = $state(true),
    addOpen = $state(false);
  $effect(() => {
    if (selectedLayer >= theme.layers.length) selectedLayer = -1;
  });
  const layerIdentities = new WeakMap<Layer, number>();
  let nextLayerIdentity = 0;
  function layerIdentity(layer: Layer): number {
    let id = layerIdentities.get(layer);
    if (id === undefined) {
      id = nextLayerIdentity++;
      layerIdentities.set(layer, id);
    }
    return id;
  }
  const canvasLayerKeys = $derived(theme.layers.map(layerIdentity));
  let pendingDisplay = $state<
    Record<string, { rotation_degrees?: QuarterTurn; brightness_percent?: number }>
  >({});
  const canvasRotation = $derived(
    pendingDisplay[device]?.rotation_degrees ??
      ((lcd?.requested?.rotation_degrees ?? 0) as QuarterTurn),
  );
  const displayBrightness = $derived(
    pendingDisplay[device]?.brightness_percent ?? lcd?.requested?.brightness_percent ?? 100,
  );
  function stageDisplay(patch: { rotation_degrees?: QuarterTurn; brightness_percent?: number }) {
    pendingDisplay[device] = { ...pendingDisplay[device], ...patch };
  }
  let undoDrafts = $state<string[]>([]),
    redoDrafts = $state<string[]>([]);
  let historyDraft = untrack(() => JSON.stringify(theme));
  function rememberDraft(next: string) {
    if (next === historyDraft) return;
    undoDrafts = [...undoDrafts, historyDraft].slice(-50);
    while (
      undoDrafts.length > 1 &&
      undoDrafts.reduce((size, draft) => size + draft.length, 0) > 8_000_000
    )
      undoDrafts.shift();
    redoDrafts = [];
    historyDraft = next;
  }
  $effect(() => {
    const next = JSON.stringify(theme);
    untrack(() => rememberDraft(next));
  });
  function restoreDraft(redo = false) {
    rememberDraft(JSON.stringify(theme));
    const source = redo ? redoDrafts : undoDrafts,
      next = source.at(-1);
    if (!next) return;
    if (redo) {
      redoDrafts = source.slice(0, -1);
      undoDrafts = [...undoDrafts, historyDraft];
    } else {
      undoDrafts = source.slice(0, -1);
      redoDrafts = [...redoDrafts, historyDraft];
    }
    const selectedId = theme.layers[selectedLayer]?.editor?.id;
    historyDraft = next;
    theme = JSON.parse(next);
    const restored = selectedId
      ? theme.layers.findIndex((layer) => layer.editor?.id === selectedId)
      : -1;
    selectedLayer = restored >= 0 ? restored : Math.min(selectedLayer, theme.layers.length - 1);
  }
  function selectCanvasLayer(index: number) {
    selectedLayer = index;
    propertiesOpen = true;
    lcdPanel = 'composition';
  }
  async function selectStackLayer(index: number) {
    selectCanvasLayer(index);
    await tick();
    if (selectedLayer !== index || !propertiesOpen) return;
    const target =
      editorElement?.querySelector<HTMLElement>('.inspector:not([hidden]) h2') ??
      editorElement?.querySelector<HTMLElement>('.pane-tabs button[aria-pressed="true"]');
    target?.focus();
  }
  function changeCanvasLayer(index: number, layer: Layer) {
    if (index < 0 || index >= theme.layers.length) return;
    const identity = layerIdentity(theme.layers[index]);
    theme.layers[index] = layer;
    layerIdentities.set(theme.layers[index], identity);
    selectedLayer = index;
  }
  const previewDraft = $derived(JSON.stringify(theme));
  let previewGeneration = 0,
    lastPreviewRequest = 0;
  let previewJob: {
    generation: number;
    dev: string;
    theme: Theme;
    live: boolean;
    started: number;
    editor: boolean;
  } | null = null;
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let previewFlight = false;
  $effect(() => {
    const draft = previewDraft,
      dev = device,
      online = connected,
      visible = tab === 'display' && windowVisible,
      live = livePreview,
      request = previewRequest,
      editor = canvasEditing;
    const manual = request !== lastPreviewRequest;
    lastPreviewRequest = request;
    const generation = ++previewGeneration;
    clearTimeout(previewTimer);
    if (!editor || !online || !visible) {
      untrack(() => {
        if (preview) URL.revokeObjectURL(preview);
      });
      preview = '';
    }
    previewError = '';
    canvasDraft = '';
    previewJob =
      online && visible && (live || manual || editor)
        ? {
            generation,
            dev,
            theme: JSON.parse(draft),
            live,
            editor,
            started: animationOrigin - animationElapsed,
          }
        : null;
    if (previewJob && (live || manual || editor)) previewTimer = setTimeout(runPreview, 50);
    return () => {
      previewJob = null;
      clearTimeout(previewTimer);
    };
  });
  async function runPreview() {
    if (previewFlight || !previewJob) return;
    const job = previewJob,
      frameStarted = performance.now();
    previewFlight = true;
    try {
      const params = {
        dev: job.dev || null,
        theme: job.theme,
        elapsed_millis: Math.floor(job.live ? performance.now() - job.started : animationElapsed),
      };
      const result = job.editor
        ? await rpc<EditorPreview>('preview_lcd_editor', params)
        : await rpc<number[]>('preview_lcd_theme', params);
      const bytes = Array.isArray(result) ? result : result.jpeg;
      if (previewJob?.generation !== job.generation) return;
      const url = bytes.length
        ? URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }))
        : '';
      if (preview) URL.revokeObjectURL(preview);
      preview = url;
      previewError = '';
      canvasBounds = Array.isArray(result) ? [] : result.layers;
      if (!Array.isArray(result) && result.background?.length) {
        const backgroundUrl = URL.createObjectURL(
          new Blob([new Uint8Array(result.background)], { type: 'image/jpeg' }),
        );
        const nextImages: string[] = [];
        try {
          for (let index = 0; index < job.theme.layers.length; index++) {
            const layer = job.theme.layers[index];
            if (layer.editor?.hidden || !editableLayer(layer)) {
              nextImages.push('');
              continue;
            }
            const dynamic =
              layer.kind === 'clock' || layer.kind === 'sensor' || layer.kind === 'sensor_cycle';
            const signature = JSON.stringify([
              layer,
              dynamic ? Math.floor(Date.now() / 1000) : 0,
              dynamic ? sensors : [],
              presentation.settings?.temperature_unit,
            ]);
            let raster = rasterCache.get(signature);
            if (!raster) {
              const part = await rpc<EditorPreview>('preview_lcd_editor', {
                ...params,
                layer_index: index,
              });
              if (previewJob?.generation !== job.generation) return;
              const bytes = part.layer_images?.[index];
              raster = bytes?.length
                ? URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/png' }))
                : '';
              if (raster) {
                for (const [key, url] of rasterCache) {
                  if (rasterCache.size < 64) break;
                  if (!editorImages.includes(url) && !nextImages.includes(url)) {
                    URL.revokeObjectURL(url);
                    rasterCache.delete(key);
                  }
                }
                rasterCache.set(signature, raster);
              }
            }
            nextImages.push(raster);
          }
          if (previewJob?.generation !== job.generation) return;
          clearEditorImages();
          editorImages = nextImages;
          editorBackground = backgroundUrl;
          for (const [key, url] of rasterCache) {
            if (rasterCache.size <= 64) break;
            if (!nextImages.includes(url)) {
              URL.revokeObjectURL(url);
              rasterCache.delete(key);
            }
          }
        } finally {
          if (editorBackground !== backgroundUrl) URL.revokeObjectURL(backgroundUrl);
        }
      }
      canvasDraft = job.editor ? JSON.stringify(job.theme) : '';
      if (!job.live && !job.editor) notice = 'Preview updated. Nothing was sent to the device.';
    } catch (e) {
      if (previewJob?.generation === job.generation) {
        previewError = message(e);
        if (preview) URL.revokeObjectURL(preview);
        preview = '';
      }
    } finally {
      previewFlight = false;
      if (previewJob && (previewJob.live || previewJob.generation !== job.generation)) {
        clearTimeout(previewTimer);
        previewTimer = setTimeout(
          runPreview,
          previewJob.generation === job.generation
            ? !previewError &&
              (job.theme.background_video ||
                job.theme.layers.some((layer) =>
                  ['weather', 'meteor', 'snow3d', 'fireworks'].includes(layer.kind),
                ))
              ? Math.max(0, 1000 / 24 - (performance.now() - frameStarted))
              : 1000
            : 50,
        );
      }
    }
  }
  function layerMetadata(index: number) {
    const layer = theme.layers[index];
    return (
      layer.editor ??
      (layer.editor = { id: crypto.randomUUID(), name: '', hidden: false, locked: false })
    );
  }
  function addLayer(layer: Layer) {
    if (theme.layers.length >= 32) return;
    layer.editor = { id: crypto.randomUUID(), name: '', hidden: false, locked: false };
    theme.layers.push(layer);
    addOpen = false;
    propertiesOpen = true;
    selectedLayer = theme.layers.length - 1;
    lcdPanel = 'composition';
  }
  function addSensor() {
    const sensor = sensors.find((s) => s.id === selectedSensor);
    if (!sensor) return;
    addLayer({
      kind: 'sensor',
      sensor_id: sensor.id,
      label: sensor.label,
      unit: sensor.unit,
      temperature_unit: 'system',
      decimals: 1,
      style: style(210),
    });
  }
  async function importImage(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const restoreFocus = document.activeElement === input;
    const target = device,
      selection = selectionRevision;
    await action(
      async () => {
        const asset = await importImageAsset(file);
        if (target) {
          try {
            await addImportedToCollection(target, 'image', asset);
          } catch (e) {
            collectionEpoch++;
            throw new ImportedImageMembershipError(e instanceof ValidationError ? e : message(e));
          }
        }
        collectionEpoch++;
        if (selection !== selectionRevision) return;
        prependLayer({
          kind: 'image',
          asset,
          x: 0,
          y: 0,
          width: 480,
          height: 480,
          fit: 'crop',
          opacity: 255,
        });
        // Keep an explicitly moved focus target in the add menu; otherwise reveal the new layer.
        if (
          document.activeElement === input ||
          document.activeElement === document.body ||
          document.activeElement?.getAttribute('aria-label') === t('Add layer')
        )
          addOpen = false;
        notice = 'Image added to the theme draft.';
      },
      () => selection === selectionRevision,
      'image-import',
    );
    input.value = '';
    await tick();
    // Disabling the picker during import can drop its focus; keep a user's new target.
    if (
      restoreFocus &&
      input.isConnected &&
      document.hasFocus() &&
      (document.activeElement === document.body ||
        (input.disabled && document.activeElement === input) ||
        (!addOpen &&
          (document.activeElement === input ||
            document.activeElement?.getAttribute('aria-label') === t('Add layer'))))
    ) {
      const target =
        !addOpen || input.disabled
          ? (editorElement?.querySelector<HTMLElement>('.layer h2') ??
            editorElement?.querySelector<HTMLElement>(
              '[data-studio-panel="composition"] > .popover-trigger',
            ))
          : input;
      target?.focus();
    }
  }
  async function publishEditedImage(png: number[], name: string) {
    if (busy || !deviceAvailable || theme.layers.length >= 32)
      throw new ValidationError('Output is unavailable or the layer limit has been reached.');
    const target = device,
      selection = selectionRevision;
    actionBusy = true;
    try {
      const asset = await importImageAsset(
        new File([new Uint8Array(png)], name, { type: 'image/png' }),
      );
      try {
        await addImportedToCollection(target, 'image', asset);
      } catch (e) {
        throw new ImportedImageMembershipError(e instanceof ValidationError ? e : message(e));
      }
      if (selection !== selectionRevision)
        return 'Result saved to the gallery. The device selection or connection changed; the draft was preserved.';
      if (theme.layers.length >= 32)
        return 'Result saved to the gallery. The 32-layer limit was reached; the draft was preserved.';
      prependLayer({
        kind: 'image',
        asset,
        x: 0,
        y: 0,
        width: 480,
        height: 480,
        fit: 'crop',
        opacity: 255,
      });
      return 'Result saved and added to the draft. Select Apply to LCD to display it.';
    } finally {
      collectionEpoch++;
      actionBusy = false;
    }
  }
  async function publishGalleryEdit(
    key: string,
    context: 'image' | 'background' | 'watermark',
    png: number[],
    name: string,
  ): Promise<string> {
    if (busy || !deviceAvailable) throw new ValidationError('The device is unavailable.');
    const target = device,
      selection = selectionRevision;
    actionBusy = true;
    error = '';
    notice = '';
    try {
      const current = await imageReplacementSnapshot(target, key, context);
      const asset = await importImageAsset(
        new File([new Uint8Array(png)], name, { type: 'image/png' }),
      );
      await replaceCollectionImage(target, current, context, key, asset);
      // Updating the selected context does not rewrite saved profiles or other
      // gallery contexts. Matching references in the current draft are explicit.
      if (selection === selectionRevision) {
        if (context === 'background' && theme.background_image?.asset === key)
          theme.background_image.asset = asset;
        if (context !== 'background')
          for (const layer of theme.layers) {
            if (
              layer.kind === 'image' &&
              layer.asset === key &&
              (context === 'watermark') === !!layer.watermark_position
            )
              layer.asset = asset;
          }
      }
      const result =
        'Collection entry replaced. The previous file and saved themes were preserved; apply the theme to change the LCD.';
      if (selection === selectionRevision) notice = result;
      return result;
    } catch (e) {
      if (selection === selectionRevision)
        error = {
          context: 'image-replacement',
          detail: e instanceof ValidationError ? e : message(e),
        };
      throw e;
    } finally {
      collectionEpoch++;
      actionBusy = false;
    }
  }
  let editorElement: HTMLElement | undefined = $state();
  async function focusLayer() {
    const editor = editorElement,
      layers = theme.layers;
    await tick();
    if (!editor?.isConnected || editor !== editorElement || layers !== theme.layers) return;
    const items = editor.querySelectorAll<HTMLElement>('.layer h2');
    const target =
      (propertiesOpen
        ? items[0]
        : editor.querySelector<HTMLElement>(`.layer-row[data-layer-index="${selectedLayer}"]`)) ??
      editor.querySelector<HTMLElement>('[data-studio-panel="composition"] > .popover-trigger');
    target?.focus();
  }
  function prependLayer(layer: Layer) {
    layer.editor = { id: crypto.randomUUID(), name: '', hidden: false, locked: false };
    const selected = theme.layers[selectedLayer];
    theme.layers.splice(isProcedural(theme.layers[0]) ? 1 : 0, 0, layer);
    if (selected) selectedLayer = theme.layers.indexOf(selected);
  }
  function removeLayer(index: number) {
    if (index < 0 || index >= theme.layers.length) return;
    if (isProcedural(theme.layers[index])) {
      try {
        theme = switchLcdPage($state.snapshot(theme), 'basic');
        selectedLayer = -1;
        error = '';
        return;
      } catch (e) {
        error = message(e);
        return;
      }
    } else theme.layers.splice(index, 1);
    selectedLayer = Math.min(index, theme.layers.length - 1);
    void focusLayer();
  }
  function reorderLayer(index: number, target: number) {
    const first = isProcedural(theme.layers[0]) ? 1 : 0;
    if (
      index >= theme.layers.length ||
      index < first ||
      target < first ||
      target >= theme.layers.length ||
      index === target
    )
      return;
    const [layer] = theme.layers.splice(index, 1);
    theme.layers.splice(target, 0, layer);
    selectedLayer = target;
    void focusLayer();
  }
  function move(index: number, step: number) {
    reorderLayer(index, index + step);
  }
  function duplicateLayer(index: number) {
    if (
      index < 0 ||
      index >= theme.layers.length ||
      theme.layers.length >= 32 ||
      isProcedural(theme.layers[index])
    )
      return;
    const copy = structuredClone($state.snapshot(theme.layers[index]));
    copy.editor = {
      ...copy.editor,
      id: crypto.randomUUID(),
      name: copy.editor?.name ?? '',
      hidden: copy.editor?.hidden ?? false,
      locked: false,
    };
    theme.layers.splice(index + 1, 0, copy);
    selectedLayer = index + 1;
  }
  const labels: Record<Layer['kind'], string> = $derived({
    divider: t('Divider'),
    fireworks: t('Firework'),
    snow3d: t('Volumetric snow'),
    meteor: t('Meteors'),
    weather: t('Weather effect'),
    sensor_cycle: t('Sensor cycle'),
    text: t('Text'),
    clock: t('Clock'),
    sensor: t('Sensor'),
    image: t('Image'),
  });
</script>

<svelte:head><title>LL-Control</title></svelte:head>
<a class="skip-link" href="#main-content">{t('Skip to content')}</a>
{#snippet capabilityLinks()}
  {#if capabilities.display || (!selectedDevice && !devices.length)}<button
      class="device-section"
      class:active={tab === 'display'}
      aria-current={tab === 'display' ? 'page' : undefined}
      onclick={() => (tab = 'display')}>{t('Pump screen')}</button
    >{/if}
  {#if capabilities.cooling}<button
      class="device-section"
      class:active={tab === 'cooling'}
      aria-current={tab === 'cooling' ? 'page' : undefined}
      onclick={() => (tab = 'cooling')}>{t('Cooling')}</button
    >{/if}
  {#if capabilities.lighting}<button
      class="device-section"
      class:active={tab === 'lighting'}
      aria-current={tab === 'lighting' ? 'page' : undefined}
      onclick={() => (tab = 'lighting')}>{t('Lighting')}</button
    >{/if}
{/snippet}
<div class="shell">
  <aside>
    <div class="brand">
      <img src="/ll-control.svg" width="32" height="32" alt="" /><span>LL-Control</span>
    </div>
    <nav aria-label={t('Sections')}>
      <button
        class:active={tab === 'overview'}
        aria-current={tab === 'overview' ? 'page' : undefined}
        onclick={() => (tab = 'overview')}>{t('Overview')}</button
      >
      <p class="nav-label">{t('Your devices')}</p>
      {#each devices as d (d.id)}
        <button
          class="device-nav"
          class:active={device === d.id && ['display', 'lighting', 'cooling'].includes(tab)}
          disabled={busy}
          onclick={() => openDevice(d.id, deviceCapabilities(d).lighting ? 'lighting' : 'settings')}
          >{d.display_name}</button
        >
        {#if device === d.id}{@render capabilityLinks()}{/if}
      {/each}
      {#if device && !devices.some((d) => d.id === device)}<p class="disconnected-device">
          {selectedDevice?.display_name ?? device} · {t('Unavailable')}
        </p>{/if}
      {#if !devices.some((d) => d.id === device)}{@render capabilityLinks()}{/if}
      <div class="global-nav">
        <button
          class:active={tab === 'spec'}
          aria-current={tab === 'spec' ? 'page' : undefined}
          onclick={() => (tab = 'spec')}>{t('System information')}</button
        >
        <button
          class:active={tab === 'sensors'}
          aria-current={tab === 'sensors' ? 'page' : undefined}
          onclick={() => (tab = 'sensors')}>{t('Sensors')}</button
        >
        <button
          class:active={tab === 'settings'}
          aria-current={tab === 'settings' ? 'page' : undefined}
          onclick={() => (tab = 'settings')}>{t('Settings')}</button
        >
      </div>
    </nav>
    <div id="connection-status" class="connection" aria-live="polite" aria-atomic="true">
      <span class="connection-dot" class:online={connected} aria-hidden="true"
      ></span>{#key connected}<p>
          {connected ? t('Daemon connected') : t('Daemon disconnected')}
        </p>{/key}
    </div>
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t('Settings are applied explicitly. Closing the window does not stop the screen.')}
      </p></StudioPopover
    >
  </aside>
  <main id="main-content" tabindex="-1">
    <header>
      <div>
        <h1>
          {tab === 'overview'
            ? t('Your devices')
            : tab === 'display'
              ? t('Pump screen')
              : tab === 'cooling'
                ? t('Cooling')
                : tab === 'lighting'
                  ? t('Lighting')
                  : tab === 'settings'
                    ? t('Settings')
                    : tab === 'spec'
                      ? t('System information')
                      : t('Sensors')}
        </h1>
        <p class="page-description">
          {['display', 'cooling', 'lighting'].includes(tab)
            ? (selectedDevice?.display_name ?? t('No device'))
            : tab === 'overview'
              ? t('Make the lighting, cooling and look of your PC your own.')
              : t('LL-Control')}
        </p>
      </div>
      {#if tab !== 'overview'}<label class="device-picker"
          >{t('Device')}<select
            aria-label={t('Device')}
            value={device}
            onchange={(e) => selectDevice(e.currentTarget.value)}
            disabled={busy || !connected || !devices.length}
            ><option value="">{t('No device')}</option
            >{#if device && !devices.some((d) => d.id === device)}<option value={device}
                >{t('Selected device unavailable')}</option
              >{/if}{#each devices as d (d.id)}<option value={d.id}>{d.display_name}</option
              >{/each}</select
          ></label
        >{/if}
    </header>
    <div hidden={tab !== 'settings' || !capabilities.display}>
      {#await import('$lib/FullGroupArchive.svelte')}
        <p role="status">{t('Loading…')}</p>
      {:then { default: FullGroupArchive }}
        <FullGroupArchive
          {device}
          {connected}
          {sensors}
          fonts={fontChoices}
          geometry={capabilities.display}
          disabled={actionBusy || resetBusy || !deviceAvailable}
          onbusy={(value) => (groupBusy = value)}
          onchange={(target) => {
            collectionEpoch++;
            resetEpoch++;
            if (target === device) {
              lcdRevision++;
              void refresh();
            }
          }}
        />
      {/await}
    </div>

    <div hidden={tab !== 'settings'}><DisplaySettings {connected} /></div>
    <div hidden={tab !== 'sensors'}>
      <SensorDefaultsEditor {connected} active={tab === 'sensors'} />
    </div>
    <div id="persistence-status" aria-live="polite" aria-atomic="true">
      {#if lcd?.persistence && !lcd.persistence.last_error}
        {#if lcd.persistence.pending}<p class="notice">
            {t('Settings are waiting to be written to disk.')}
          </p>
        {:else if lcd.persistence.saved}<p class="notice">
            {t('The last settings write completed.')}
          </p>
        {/if}
      {/if}
    </div>
    {#if lcd?.persistence?.last_error}<p role="alert">
        {t('Could not save settings:')}
        {lcd.persistence.last_error}{t('. The daemon will retry the write.')}
      </p>{/if}
    {#if connectionError}<div class="error" role="alert">
        {t('Could not connect to the daemon: {0}', connectionError)}<button
          onclick={() => refresh()}>{t('Reconnect')}</button
        >
      </div>{/if}
    {#if error}<div class="error" role="alert">{errorText(error)}</div>{/if}
    {#if connected && device && !deviceAvailable}<p role="status">
        {t(
          'The selected device is unavailable. Your draft was preserved; select a device explicitly or wait for it to reconnect.',
        )}
      </p>{/if}
    {#if notice}<p class="notice" role="status">{t(notice)}</p>{/if}
    {#if lightingVisited}<div hidden={tab !== 'lighting'}>
        <Lighting
          {device}
          connected={deviceAvailable && capabilities.lighting && tab === 'lighting'}
          {sensors}
        />
      </div>{/if}
    {#if tab === 'overview'}
      <DeviceOverview {devices} {connected} disabled={busy} onopen={openDevice} />
    {:else if ['display', 'cooling', 'lighting'].includes(tab) && selectedDevice && !capabilities[tab === 'display' ? 'display' : tab === 'cooling' ? 'cooling' : 'lighting']}
      <p>{t('No editors are available for this device family yet.')}</p>
    {:else if tab === 'spec'}
      <SystemSpec {connected} />
    {:else if tab === 'settings'}
      <DesktopSettings />
      <FpsSettings {connected} />
      {#key connected}<FirmwareRecovery />{/key}
      {#if capabilities.display}<Settings {device} connected={deviceAvailable} {lcd} />{/if}
      {#if capabilities.display}{#key `${selectionRevision}:${device}:${deviceAvailable}`}<FirmwareInfo
            {device}
            connected={deviceAvailable}
          />{/key}{/if}
    {:else if tab === 'lighting'}
      <!-- The mounted studio above retains device-scoped RGB drafts. -->
    {:else if tab === 'cooling'}
      <Cooling {device} {sensors} connected={deviceAvailable} />
    {:else if tab === 'sensors'}
      <section class="sensors" aria-label={t('Sensor readings')}>
        {#each sensors as sensor (sensor.id)}
          <article>
            <h2>{sensor.label}</h2>
            <strong>{formatSensor(sensor, presentation.settings?.temperature_unit)}</strong>
            <p>{sensor.status} · {sensor.age_ms ?? '—'} ms</p>
            <code>{sensor.id}</code>
          </article>
        {/each}
      </section>
      {#if !sensors.length}<p>{t('Readings will appear when the daemon connects.')}</p>{/if}
    {:else}
      <div class="workspace">
        <div class="document-head">
          <div class="row">
            <label>{t('Name')}<input bind:value={theme.name} maxlength="128" /></label>
          </div>
          <div class="profiles-disclosure">
            <Profiles
              refreshEpoch={resetEpoch}
              group="screen"
              mediaKey={profileMediaKey}
              {connected}
              disabled={busy}
              draft={() => {
                const draft = $state.snapshot(theme);
                assertWeatherDraft(draft);
                return { kind: 'screen', value: draft };
              }}
              onload={(content) => {
                if (content.kind === 'screen_media') {
                  lcdPanel = 'composition';
                  loadedMediaKey = content.value;
                  notice = 'Profile video loaded. Applying it is a separate action.';
                } else if (content.kind === 'screen') {
                  loadedMediaKey = '';
                  theme = content.value;
                  selectedLayer = -1;
                  notice = '';
                  if (preview) URL.revokeObjectURL(preview);
                  preview = '';
                }
              }}
            />
          </div>
          <div class="editor-tabs" aria-label={t('LCD workspace')}>
            <button aria-pressed={lcdPanel === 'presets'} onclick={() => (lcdPanel = 'presets')}
              >{t('Theme presets')}</button
            ><button
              aria-pressed={lcdPanel === 'composition'}
              onclick={() => (lcdPanel = 'composition')}>{t('Composition')}</button
            ><button aria-pressed={lcdPanel === 'library'} onclick={() => (lcdPanel = 'library')}
              >{t('Library')}</button
            >
          </div>
        </div>
        <section class="visual" class:canvas-mode={canvasEditing} aria-label={t('LCD preview')}>
          <div class="canvas-toolbar">
            <button class="primary" disabled={busy || !deviceAvailable} onclick={applyTheme}
              >{t('Apply to LCD')}</button
            >
            <div class="orientation-control" title={t('Screen orientation')}>
              <StudioIcon name="rotate" /><select
                aria-label={t('Screen orientation')}
                value={canvasRotation}
                onchange={(event) =>
                  stageDisplay({
                    rotation_degrees: Number(event.currentTarget.value) as QuarterTurn,
                  })}
                >{#each [0, 90, 180, 270] as angle (angle)}<option value={angle}>{angle}°</option
                  >{/each}</select
              >
            </div>
          </div>
          <LcdPreview geometry={capabilities.display}
            >{#if canvasEditing}<CanvasEditor
                image={preview}
                images={editorImages}
                background={editorBackground}
                keys={canvasLayerKeys}
                layers={theme.layers}
                bounds={canvasBounds}
                selected={selectedLayer}
                rotation={canvasRotation}
                ready={canvasDraft === previewDraft && connected}
                {labels}
                onselect={selectCanvasLayer}
                onchange={changeCanvasLayer}
                onorder={move}
              />{:else if preview}<img
                src={preview}
                alt={t('Prepared LCD composition — preview does not confirm output on the device')}
              />{:else}<div class="empty">
                <span
                  >{capabilities.display?.width ?? 480} × {capabilities.display?.height ??
                    480}</span
                >
                <p>{t('Create a theme')}<br />{t('and refresh the preview')}</p>
              </div>{/if}</LcdPreview
          >
          <div class="status">
            <span>{t('Source:')} {lcdSourceLabel(lcd?.source)}</span>{#if lcd?.last_error}<span
                role="alert">{lcd.last_error}</span
              >{:else}<StudioPopover label={t('LCD preview')} icon="help"
                ><p>{t('Preview does not confirm physical output')}</p></StudioPopover
              >{/if}
          </div>
          <div class="canvas-actions">
            {#if canvasEditing}
              <StudioPopover label={t('Canvas controls')} icon="help">
                <p>
                  {t(
                    'Arrows move; Shift + arrows resize; Ctrl + arrows use a 10px step; Alt + arrows reorder. Escape cancels a gesture.',
                  )}
                </p>
                <p>
                  {t(
                    'Text resize scales the font and text box. Images scale proportionally. Backgrounds and effects stay fixed.',
                  )}
                </p></StudioPopover
              >
            {/if}
            <button
              disabled={!undoDrafts.length}
              onclick={() => restoreDraft()}
              class="icon-action"
              aria-label={t('Undo edit')}
              title={t('Undo edit')}><StudioIcon name="undo" /></button
            ><button
              disabled={!redoDrafts.length}
              onclick={() => restoreDraft(true)}
              class="icon-action"
              aria-label={t('Redo edit')}
              title={t('Redo edit')}><StudioIcon name="redo" /></button
            >
            <button
              class="icon-action"
              aria-pressed={!livePreview}
              aria-label={livePreview ? t('Pause animation') : t('Resume animation')}
              title={livePreview ? t('Pause animation') : t('Resume animation')}
              onclick={toggleAnimation}><StudioIcon name={livePreview ? 'pause' : 'play'} /></button
            >
            {#if previewError}<p role="alert">
                {t('Could not render the preview: {0}', previewError)}
              </p>{/if}

            <StudioPopover label={t('Display settings')} icon="settings"
              ><label
                >{t('Screen brightness, %')}<input
                  type="range"
                  min="0"
                  max="100"
                  value={displayBrightness}
                  oninput={(event) =>
                    stageDisplay({ brightness_percent: Number(event.currentTarget.value) })}
                /><output>{displayBrightness}%</output></label
              >
              <p>{t('Display settings will change on Apply.')}</p></StudioPopover
            >
            {#if Object.keys(pendingDisplay[device] ?? {}).length}<p class="status">
                {t('Display settings will change on Apply.')}
              </p>{/if}
            <StudioPopover label={t('Load or stop')} icon="download"
              ><div class="document-actions">
                <button disabled={busy || !deviceAvailable} onclick={() => loadTheme()}
                  >{t('Load active theme')}</button
                >
                <button disabled={busy || !deviceAvailable} onclick={() => loadTheme(true)}
                  >{t('Load imported editor')}</button
                >
                <button disabled={busy || !deviceAvailable} onclick={clearTheme}
                  >{t('Restore LocalUI')}</button
                >
              </div></StudioPopover
            >
          </div>
        </section>
        <section class="editor" aria-label={t('Theme editor')} bind:this={editorElement}>
          <div data-studio-panel="presets" hidden={lcdPanel !== 'presets'}>
            <section class="preset-picker">
              <h2>{t('Theme presets — editable layouts')}</h2>
              <div class="preset-gallery" aria-label={t('Theme presets')}>
                {#each presets as preset (preset.id)}
                  <button
                    type="button"
                    class:chosen={presetId === preset.id}
                    aria-pressed={presetId === preset.id}
                    onclick={() => {
                      presetId = preset.id;
                      presetBindings = [];
                    }}
                  >
                    <span class="preset-thumbnail {preset.id}" aria-hidden="true">
                      {#each Array.from({ length: Math.max(preset.sensors, 1) }, (_, index) => index) as index (index)}<span
                        ></span>{/each}
                      <i class="thumbnail-clock"></i>
                    </span>
                    <strong>{preset.name}</strong>
                    <small>{t(preset.description)}</small>
                  </button>
                {/each}
              </div>
              {#if presetRequired}<p>
                  {t('Select')}
                  {presetRequired}
                  {t('sensors. Selection order determines their positions.')}
                </p>
                <div class="preset-sensors">
                  {#each sensors as sensor (sensor.id)}<label
                      ><input
                        type="checkbox"
                        checked={presetBindings.includes(sensor.id)}
                        onchange={(e) => {
                          presetBindings = e.currentTarget.checked
                            ? [...presetBindings, sensor.id]
                            : presetBindings.filter((id) => id !== sensor.id);
                        }}
                        disabled={!presetBindings.includes(sensor.id) &&
                          presetBindings.length >= presetRequired}
                      />{sensor.label} · {sensor.unit}</label
                    >{/each}
                </div>{/if}
              <button
                disabled={presetBindings.length !== presetRequired || busy}
                onclick={() => {
                  try {
                    theme = presetTheme(
                      presetId,
                      presetBindings.map((id) => {
                        const sensor = sensors.find((s) => s.id === id);
                        if (!sensor) throw new ValidationError('Sensor missing from the catalog');
                        return sensor;
                      }),
                    );
                    selectedLayer = -1;
                    lcdPanel = 'composition';
                    notice =
                      'The layout replaced the local draft. Customize it and select Apply to send it.';
                    error = '';
                  } catch (e) {
                    error = message(e);
                  }
                }}>{t('Create a draft from this layout')}</button
              >
            </section>
          </div>
          <div data-studio-panel="library" hidden={lcdPanel !== 'library'}>
            <FontImport
              {connected}
              onimport={(font) => {
                importedFonts = [...importedFonts.filter((f) => f.key !== font.key), font];
              }}
            />
            <AssetCleanup
              kind="font"
              {connected}
              disabled={busy}
              keep={fontChoices.map((font) => font.key)}
            />
            {#await import('$lib/ScreenCollections.svelte')}
              <p role="status">{t('Loading…')}</p>
            {:then { default: ScreenCollections }}
              <ScreenCollections
                refreshEpoch={resetEpoch}
                {device}
                connected={deviceAvailable}
                disabled={busy}
                onchange={() => collectionEpoch++}
              />
            {/await}
            <ImageGallery
              onedit={publishGalleryEdit}
              {device}
              {collectionEpoch}
              connected={deviceAvailable}
              disabled={busy}
              addDisabled={theme.layers.length >= 32}
              keep={draftImageKeys}
              onadd={(asset, context) => {
                if (busy) return;
                if (context === 'background') {
                  theme = {
                    ...theme,
                    version: isProcedural(theme.layers[0]) ? 3 : 1,
                    background_video: undefined,
                    background_image: { asset, fit: 'crop' },
                  };
                  loadedMediaKey = '';
                  notice =
                    'Image selected as the draft background. Layers were preserved; apply the theme to show it on the LCD.';
                  return;
                }
                if (theme.layers.length >= 32) return;
                if (context === 'watermark') {
                  addLayer({
                    kind: 'image',
                    asset,
                    x: 0,
                    y: 0,
                    width: 144,
                    height: 144,
                    fit: 'contain_native',
                    opacity: 255,
                    watermark_position: { horizontal: 50, vertical: 50 },
                  });
                  notice = 'Watermark added to the theme draft.';
                } else {
                  prependLayer({
                    kind: 'image',
                    asset,
                    x: 0,
                    y: 0,
                    width: 480,
                    height: 480,
                    fit: 'crop',
                    opacity: 255,
                  });
                  notice = 'Saved image added to the theme draft.';
                }
              }}
            />
            <AssetCleanup {connected} disabled={busy} keep={draftImageKeys} />
            {#await import('$lib/MediaLibrary.svelte')}
              <p role="status">{t('Loading…')}</p>
            {:then { default: MediaLibrary }}
              <MediaLibrary
                keep={draftVideoKeys}
                {device}
                {collectionEpoch}
                {connected}
                available={deviceAvailable}
                disabled={busy}
                onselect={selectMedia}
                onstop={clearTheme}
                onbackground={(key) => {
                  theme = {
                    ...theme,
                    version: isProcedural(theme.layers[0]) ? 3 : 2,
                    background_image: undefined,
                    background_video: key,
                  };
                  loadedMediaKey = '';
                  notice =
                    'Video added as the draft background. Layers were preserved; apply the theme to play it on the LCD.';
                }}
                onchoice={(key) => (profileMediaKey = key)}
                ondeleted={(key) => {
                  if (loadedMediaKey === key) loadedMediaKey = '';
                }}
              />
            {/await}
          </div>
          <div data-studio-panel="composition" hidden={lcdPanel !== 'composition'}>
            <StudioPopover label={t('Add layer')} icon="plus" bind:open={addOpen}>
              <div class="toolbar">
                <button
                  disabled={busy || theme.layers.length >= 32}
                  onclick={() =>
                    addLayer({
                      kind: 'divider',
                      axis: 'horizontal',
                      x: 34,
                      y: 240,
                      length: 412,
                      thickness: 2,
                      color: [144, 177, 201],
                      opacity: 100,
                      shadow: { x: 0, y: 0, blur: 5, color: [0, 0, 0], opacity: 100 },
                    })}>{t('+ Divider')}</button
                ><button
                  data-add-text
                  disabled={theme.layers.length >= 32}
                  onclick={() => addLayer({ kind: 'text', text: t('New text'), style: style(180) })}
                  >{t('+ Text')}</button
                ><button
                  disabled={theme.layers.length >= 32}
                  onclick={() =>
                    addLayer({
                      kind: 'clock',
                      twenty_four_hour: true,
                      show_seconds: true,
                      style: style(300),
                    })}>{t('+ Clock')}</button
                ><label class="file"
                  >{t('+ Image')}<input
                    type="file"
                    accept="image/png,image/jpeg,image/webp,image/bmp"
                    onchange={importImage}
                    disabled={busy || theme.layers.length >= 32}
                  /></label
                >
              </div>
              <div class="row">
                <label
                  >{t('Bind sensor')}<select bind:value={selectedSensor}
                    ><option value="">{t('Select a source')}</option
                    >{#each sensors as s (s.id)}<option value={s.id}>{s.label} · {s.unit}</option
                      >{/each}</select
                  ></label
                ><button disabled={!selectedSensor || theme.layers.length >= 32} onclick={addSensor}
                  >{t('Add')}</button
                >
              </div>
            </StudioPopover>
            <div class="pane-tabs">
              <button aria-pressed={!propertiesOpen} onclick={() => (propertiesOpen = false)}
                >{t('Layers')}</button
              ><button aria-pressed={propertiesOpen} onclick={() => (propertiesOpen = true)}
                >{t('Properties')}</button
              >
            </div>
            <LayerStack
              collapsed={propertiesOpen}
              layers={theme.layers}
              {labels}
              selected={selectedLayer}
              onselect={selectStackLayer}
              onmove={reorderLayer}
              onduplicate={duplicateLayer}
              onremove={removeLayer}
              onvisibility={(index) => {
                const meta = layerMetadata(index);
                meta.hidden = !meta.hidden;
              }}
              onlock={(index) => {
                const meta = layerMetadata(index);
                meta.locked = !meta.locked;
              }}
            />
            {#if selectedLayer >= 0 && theme.layers[selectedLayer] && !isProcedural(theme.layers[selectedLayer])}
              <article
                hidden={!propertiesOpen}
                class="layer inspector"
                data-layer-index={selectedLayer}
                aria-label={t('Selected layer properties')}
              >
                <div class="inspector-heading">
                  <h2 tabindex="-1">{labels[theme.layers[selectedLayer].kind]}</h2>
                  <StudioPopover label={t('Rename layer')} icon="edit"
                    ><label
                      >{t('Layer name')}<input
                        value={theme.layers[selectedLayer].editor?.name ?? ''}
                        maxlength="128"
                        oninput={(event) =>
                          (layerMetadata(selectedLayer).name = event.currentTarget.value)}
                      /></label
                    ></StudioPopover
                  >
                </div>
                <fieldset
                  class="layer-fields"
                  disabled={theme.layers[selectedLayer].editor?.locked}
                >
                  <LayerInspector
                    bind:value={theme.layers[selectedLayer]}
                    {sensors}
                    {fontChoices}
                  />
                </fieldset>
              </article>
            {:else}
              <div hidden={!propertiesOpen}>
                {#if loadedMediaKey}<section aria-label={t('Profile video')}>
                    <MediaThumbnail clipKey={loadedMediaKey} name={t('Profile video')} /><button
                      disabled={busy || !deviceAvailable}
                      onclick={() => selectMedia(loadedMediaKey)}>{t('Apply profile video')}</button
                    ><button onclick={() => (loadedMediaKey = '')}
                      >{t('Close profile video')}</button
                    >
                  </section>{/if}<BasicEditor
                  bind:theme
                  disabled={busy}
                  onchange={() => (loadedMediaKey = '')}
                />{#await import('$lib/WeatherEditor.svelte')}
                  <p role="status">{t('Loading…')}</p>
                {:then { default: WeatherEditor }}
                  <WeatherEditor bind:theme {sensors} disabled={busy} />
                {/await}{#await import('$lib/PaletteEditor.svelte')}
                  <p role="status">{t('Loading…')}</p>
                {:then { default: PaletteEditor }}
                  <PaletteEditor bind:theme {sensors} disabled={busy} />
                {/await}<BackgroundEditor bind:theme />
              </div>
            {/if}
            <div
              hidden={!propertiesOpen ||
                (selectedLayer >= 0 && !isProcedural(theme.layers[selectedLayer]))}
            >
              {#key device}<ImageEditor
                  disabled={busy || !deviceAvailable || theme.layers.length >= 32}
                  onpublish={publishEditedImage}
                />{/key}
            </div>
          </div>
        </section>
      </div>
    {/if}
    <div hidden={tab !== 'display' || !capabilities.display}>
      {#await import('$lib/ScreenReset.svelte')}
        <p role="status">{t('Loading…')}</p>
      {:then { default: ScreenReset }}
        <ScreenReset
          {device}
          connected={deviceAvailable}
          active={tab === 'display'}
          disabled={actionBusy}
          onbusy={(value) => (resetBusy = value)}
          onchange={(target) => {
            collectionEpoch++;
            resetEpoch++;
            if (target === device) {
              lcdRevision++;
              void refresh();
            }
          }}
        />
      {/await}
    </div>
  </main>
</div>

<style>
  .pane-tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 8px;
  }
  .pane-tabs button {
    flex: 1;
  }
  .pane-tabs button[aria-pressed='true'] {
    background: var(--accent-surface);
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .inspector[hidden] {
    display: none;
  }
  .document-head {
    grid-column: 1/-1;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
  }
  .document-head > .row {
    flex: 1;
    min-width: 160px;
    margin: 0;
  }
  .document-head .editor-tabs {
    margin: 0;
    padding: 0;
    border: 0;
  }
  .document-head :global(details) {
    margin: 0;
  }
  .document-actions {
    display: grid;
    gap: 8px;
  }
  .file input {
    width: 100%;
  }
  .toolbar > .file {
    flex-basis: 100%;
  }
  .inspector-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .inspector-heading h2 {
    margin: 0;
  }
  .canvas-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
  }
  .canvas-actions > p {
    flex-basis: 100%;
  }
  .canvas-toolbar {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 8px;
  }
  .canvas-toolbar > .primary {
    align-self: center;
  }
  .orientation-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .orientation-control select {
    width: 90px;
  }

  .visual.canvas-mode {
    grid-auto-rows: max-content;
    align-content: start;
  }
  .visual.canvas-mode :global(.lcd) {
    max-width: min(400px, max(240px, calc(100vh - 440px)));
  }

  .shell {
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    min-height: 100vh;
  }
  aside {
    padding: 30px 18px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: Exo, sans-serif;
    font-size: 19px;
    letter-spacing: -0.025em;
    margin: 0 0 36px;
    white-space: nowrap;
  }
  .brand img {
    flex: none;
  }
  nav,
  .global-nav {
    display: grid;
    gap: 6px;
  }
  nav button {
    text-align: left;
    background: transparent;
    border-color: transparent;
  }
  .nav-label {
    font-size: 12px;
    color: var(--text-muted);
    margin: 24px 12px 8px;
  }
  .device-section {
    padding-left: 24px;
    font-size: 13px;
  }
  nav button.active {
    background: var(--accent-surface);
    color: var(--accent-text);
  }
  .global-nav {
    border-top: 1px solid var(--border);
    padding-top: 20px;
    margin-top: 20px;
  }
  .connection {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 38px 10px 0;
    font-size: 12px;
  }
  .connection p {
    margin: 0;
  }
  .connection-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--danger);
  }
  .connection-dot.online {
    background: var(--success);
  }
  .disconnected-device {
    font-size: 12px;
    line-height: 1.7;
    color: var(--text-muted);
    padding: 0 10px;
  }
  main {
    padding: 24px;
    min-width: 0;
    max-width: 1700px;
    width: 100%;
    margin-inline: auto;
  }
  header {
    display: flex;
    gap: 24px;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 30px;
    padding-bottom: 24px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    font-size: 32px;
    margin: 0 0 10px;
  }
  .page-description {
    margin: 0;
    color: var(--text-muted);
    line-height: 1.6;
  }
  .device-picker {
    max-width: 270px;
  }
  .workspace {
    display: grid;
    grid-template-columns: minmax(280px, 1fr) minmax(300px, 360px);
    gap: 20px;
  }
  .visual {
    position: sticky;
    top: 16px;
    align-self: start;
    display: grid;
    gap: 10px;
    padding: 16px;
    background: var(--canvas);
    border-radius: 14px;
  }
  .empty {
    text-align: center;
    color: var(--text-muted);
  }
  .empty span {
    font-family: Exo, sans-serif;
    font-size: 28px;
  }
  .status {
    display: grid;
    gap: 6px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .row,
  .toolbar {
    display: flex;
    gap: 12px;
    align-items: end;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .editor {
    min-width: 0;
  }
  .editor-tabs {
    display: flex;
    gap: 8px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 14px;
    margin-bottom: 20px;
  }
  .editor-tabs button[aria-pressed='true'] {
    background: var(--accent-surface);
    color: var(--accent-text);
    border-color: var(--accent);
  }
  .layer {
    padding: 12px 0;
    margin-top: 8px;
    display: grid;
    gap: 8px;
  }
  .layer-fields {
    border: 0;
    padding: 0;
    margin: 0;
  }
  .inspector {
    max-height: calc(100vh - 480px);
    min-height: 260px;
    overflow: auto;
    scrollbar-gutter: stable;
  }
  .file {
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    padding: 10px;
    cursor: pointer;
  }
  .file input {
    max-width: 160px;
    font-size: 12px;
  }
  .error {
    background: var(--danger-surface);
    border: 1px solid var(--danger);
    padding: 16px;
    border-radius: 8px;
    margin-bottom: 16px;
    overflow-wrap: anywhere;
  }
  .error button {
    margin-left: 12px;
  }
  .notice {
    color: var(--success);
    line-height: 1.6;
  }
  .sensors {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 0 24px;
  }
  .sensors article {
    border-top: 1px solid var(--border);
    padding: 24px 0;
    min-width: 0;
  }
  .sensors strong {
    font-size: 30px;
    font-variant-numeric: tabular-nums;
  }
  .sensors p {
    color: var(--text-muted);
    font-size: 12px;
  }
  .skip-link {
    position: fixed;
    top: 12px;
    left: 12px;
    z-index: 100;
    background: var(--accent);
    color: var(--accent-ink);
    padding: 12px 18px;
    border-radius: 8px;
    transform: translateY(-200%);
  }
  .skip-link:focus {
    transform: translateY(0);
  }
  @media (min-width: 981px) {
    header {
      margin-bottom: 16px;
      padding-bottom: 16px;
    }
    h1 {
      font-size: 26px;
      margin-bottom: 4px;
    }
    .workspace {
      gap: 12px;
    }
    .visual {
      padding: 12px;
    }
    .document-head {
      gap: 8px;
    }
    .document-head > .row {
      max-width: 210px;
    }
    .editor :global(.stack) {
      margin-block: 8px;
      padding-block: 8px;
    }
    .editor :global(.rows) {
      max-height: calc(100vh - 440px);
      min-height: 250px;
    }
    .editor :global(.layer-row) {
      padding: 4px 8px;
      min-height: 28px;
    }
    .editor :global(.stack footer) {
      padding-top: 6px;
    }
    .visual .status {
      max-height: 48px;
      overflow: auto;
    }
  }
  @media (max-width: 980px) {
    .shell {
      grid-template-columns: 190px minmax(0, 1fr);
    }
    main {
      padding: 24px;
    }
    .workspace {
      grid-template-columns: minmax(0, 1fr);
    }
    .visual {
      position: static;
      max-width: 560px;
      width: 100%;
      justify-self: center;
    }
  }
  @media (max-width: 760px) {
    .shell {
      grid-template-columns: minmax(0, 1fr);
    }
    aside {
      padding: 16px;
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }
    .brand {
      margin-bottom: 16px;
    }
    nav {
      grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    }
    .nav-label,
    .global-nav {
      grid-column: 1/-1;
    }
    .global-nav {
      display: flex;
      flex-wrap: wrap;
      margin-top: 10px;
      padding-top: 10px;
    }
    .connection {
      margin-top: 16px;
    }
    main {
      padding: 16px;
    }
    header {
      flex-wrap: wrap;
      gap: 16px;
    }
    .device-picker {
      flex-basis: 100%;
      max-width: none;
    }
    h1 {
      font-size: 28px;
    }
    .visual {
      padding: 18px;
    }
  }

  .preset-gallery {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
    gap: 0.75rem;
    margin: 1rem 0;
  }
  .preset-gallery button {
    display: grid;
    gap: 0.4rem;
    text-align: left;
    padding: 0.7rem;
  }
  .preset-gallery button.chosen {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .preset-gallery small {
    color: var(--text-muted);
  }
  .preset-thumbnail {
    aspect-ratio: 1;
    max-height: 7rem;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    place-content: center;
    gap: 0.35rem;
    padding: 1rem;
    background: #0c121e;
    border-radius: 0.5rem;
  }
  .preset-thumbnail span {
    display: block;
    height: 0.5rem;
    border-radius: 1rem;
    background: #90b1c9;
  }
  .preset-thumbnail {
    position: relative;
  }
  .preset-thumbnail.focus span {
    grid-column: 1 / -1;
    height: 1rem;
  }
  .preset-thumbnail.duo {
    grid-template-columns: 1fr;
  }
  .preset-thumbnail.triple {
    grid-template-columns: 1fr;
  }
  .preset-thumbnail.cycle span:not(:first-child) {
    opacity: 0.25;
  }
  .preset-thumbnail.clock span {
    grid-column: 1 / -1;
    height: 1rem;
  }
  .thumbnail-clock {
    position: absolute;
    bottom: 12%;
    left: 35%;
    width: 30%;
    height: 0.25rem;
    background: #90b1c9;
    border-radius: 1rem;
  }
  .preset-thumbnail.clock .thumbnail-clock {
    bottom: 25%;
  }
</style>
