<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import { onMount, tick, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import VideoSourcePreview from './VideoSourcePreview.svelte';
  import PreparedVideoPreview from './PreparedVideoPreview.svelte';
  import { normalizedCrop, type CropFields, type VideoCrop } from './video-crop';
  import { message } from './api';
  import { validVideoAmount } from './video-adjustments';
  import { videoEditBounds, validVideoRange, type SourceMetadata } from './video-range';
  let {
    device = null,
    existing,
    onbusy = () => {},
    onimport,
  }: {
    existing?: { id: string; name: string; url: string; metadata?: SourceMetadata | null };
    onbusy?: (busy: boolean) => void;
    device?: string | null;
    onimport: (key: string | null, device: string | null) => void;
  } = $props();
  type Phase =
    | 'uploading'
    | 'inspecting'
    | 'source_ready'
    | 'encoding'
    | 'validating'
    | 'remuxing'
    | 'review_ready'
    | 'cancelling'
    | 'transferring'
    | 'publishing'
    | 'complete'
    | 'cancelled'
    | 'failed';
  interface Job {
    id: string;
    name: string;
    total: number;
    received: number;
    phase: Phase;
    key: string | null;
    message: string | null;
    metadata?: SourceMetadata | null;
  }
  const terminal = (phase: Phase) => ['complete', 'cancelled', 'failed'].includes(phase);
  const labels: Record<Phase, string> = {
    uploading: 'File upload',
    inspecting: 'Inspecting file',
    source_ready: 'Source inspected',
    encoding: 'Encoding for LCD',
    validating: 'Checking result',
    remuxing: 'Preparing preview',
    review_ready: 'Video ready for review',
    cancelling: 'Cancelling import',
    transferring: 'Transferring video to library',
    publishing: 'Saving to library',
    complete: 'Video saved to library',
    cancelled: 'Import cancelled',
    failed: 'Import incomplete',
  };
  type ImportFailure = {
    detail: string;
    context: 'direct' | 'confirm' | 'upload' | 'prepare' | 'validation';
  };
  let job = $state<Job | null>(null),
    error = $state<ImportFailure | null>(null),
    fit = $state<'contain' | 'cover' | 'stretch'>('contain');
  function setError(cause: unknown, context: ImportFailure['context'] = 'direct') {
    error = { detail: message(cause), context };
  }
  function errorText(): string {
    if (error?.context === 'validation') return t('Select a nonempty GIF or MP4 up to 256 MiB.');
    if (error?.context === 'confirm')
      return t('{0} Check the import status before confirming again.', error.detail);
    if (error?.context === 'upload')
      return t('{0} Check the import status and library before uploading again.', error.detail);
    if (error?.context === 'prepare') return t('{0} Check the preparation status.', error.detail);
    // Unknown diagnostics are data, including exact resource-key collisions.
    return error?.detail || pollError;
  }
  let mounted = false,
    announced: string | null = null;
  let ownedId = $state<string | null>(null);
  const sourceReady = $derived(job?.phase === 'source_ready' && job.id === ownedId);
  let sourcePreview = $state<{
    currentPosition: () => number | null;
    currentExactPosition: () => number | null;
  }>();
  let cutEnabled = $state(false),
    effectsEnabled = $state(false),
    rangeEnabled = $state(false);
  let contrast = $state<number | undefined>(0),
    brightness = $state<number | undefined>(0);
  const effectsValid = $derived(
    !effectsEnabled || (validVideoAmount(contrast) && validVideoAmount(brightness)),
  );
  const adjustments = $derived(
    effectsEnabled && validVideoAmount(contrast) && validVideoAmount(brightness)
      ? { contrast, brightness }
      : null,
  );
  const editing = $derived(cutEnabled || effectsEnabled || rangeEnabled);
  let rangeStart = $state<number | undefined>(0),
    rangeEnd = $state<number | undefined>();
  let initializedRangeId: string | null = null;
  const rangeMetadata = $derived(existing?.metadata ?? (sourceReady ? job?.metadata : null));
  const rangeBounds = $derived(videoEditBounds(rangeMetadata));
  const rangeValid = $derived(!rangeEnabled || validVideoRange(rangeStart, rangeEnd, rangeBounds));
  const range = $derived(
    rangeEnabled && rangeValid ? { startSeconds: rangeStart!, endSeconds: rangeEnd! } : null,
  );
  function resetRange(metadata: SourceMetadata | null | undefined = rangeMetadata) {
    rangeStart = 0;
    rangeEnd = videoEditBounds(metadata)?.maximumSeconds;
  }
  function toggleRange() {
    if (rangeEnabled) {
      cutEnabled = false;
      recordMode = false;
      cropSegmentId = null;
      resetRange();
      if (!existing) clearSource();
    } else if (!cutEnabled && !effectsEnabled && !rangeEnabled) clearSource();
  }
  function markRange(edge: 'start' | 'end') {
    const position = sourcePreview?.currentExactPosition();
    if (position === null || position === undefined || active || !rangeBounds) return;
    const value = Math.min(position, rangeBounds.maximumSeconds);
    if (edge === 'start') rangeStart = value;
    else rangeEnd = value;
  }
  let selectedFile = $state<File | null>(null),
    sourceDuration = $state<number | null>(null),
    sourcePosition = $state<number | null>(null);
  function markSegment(
    segment: { start: number | undefined; end: number | undefined },
    edge: 'start' | 'end',
  ) {
    const position = sourcePreview?.currentPosition();
    if (position !== null && position !== undefined && !active) segment[edge] = position;
  }
  function clearSource() {
    selectedFile = null;
    sourceDuration = null;
    sourcePosition = null;
    recordedSegments = [];
    recording = false;
  }
  type SegmentDraft = {
    id: number;
    start: number | undefined;
    end: number | undefined;
    crop?: CropFields;
    cropButton?: HTMLButtonElement;
    input?: HTMLInputElement;
  };
  let manualSegments = $state<SegmentDraft[]>([{ id: 0, start: 0, end: 5 }]);
  let recordedSegments = $state<SegmentDraft[]>([]);
  let recordMode = $state(false),
    recording = $state(false);
  let recordCrop = $state<CropFields>({ x: 0, y: 0, width: 100, height: 100 });
  const segments = $derived(recordMode ? recordedSegments : manualSegments);
  function setSegments(value: SegmentDraft[]) {
    if (recordMode) recordedSegments = value;
    else manualSegments = value;
  }
  function addRecorded(segment: { start_ms: number; end_ms: number; crop: VideoCrop | null }) {
    if (!recordMode || active || recordedSegments.length >= 64) return;
    const crop = segment.crop;
    recordedSegments = [
      ...recordedSegments,
      {
        id: nextSegmentId++,
        start: segment.start_ms / 1000,
        end: segment.end_ms / 1000,
        ...(crop
          ? {
              crop: {
                x: crop.x * 100,
                y: crop.y * 100,
                width: Math.min(crop.width * 100, 100 - crop.x * 100),
                height: Math.min(crop.height * 100, 100 - crop.y * 100),
              },
            }
          : {}),
      },
    ];
    if (cutDuration >= 180000) {
      if (existing) void prepareExisting();
      else if (selectedFile) void uploadFile(selectedFile);
    }
  }

  let nextSegmentId = 1;
  let cropSegmentId = $state<number | null>(null);
  const cropSegment = $derived(segments.find((s) => s.id === cropSegmentId));
  const previewCrop = $derived(
    recordMode
      ? normalizedCrop(recordCrop)
      : cropSegment?.crop
        ? normalizedCrop(cropSegment.crop)
        : null,
  );
  function chooseCrop(id: number) {
    const segment = segments.find((s) => s.id === id);
    if (active || !segment) return;
    segment.crop ??= { x: 0, y: 0, width: 100, height: 100 };
    cropSegmentId = id;
  }
  async function resetCrop(id: number) {
    const segment = segments.find((s) => s.id === id);
    if (active || !segment) return;
    segment.crop = undefined;
    if (cropSegmentId === id) cropSegmentId = null;
    await tick();
    if (mounted && !active) segment.cropButton?.focus();
  }
  function updateCrop(crop: VideoCrop) {
    if (active || recording || (!recordMode && !cropSegment?.crop)) return;
    const percent = (value: number) => Math.round(value * 10000) / 100;
    const value = {
      x: percent(crop.x),
      y: percent(crop.y),
      width: percent(crop.width),
      height: percent(crop.height),
    };
    if (recordMode) recordCrop = value;
    else if (cropSegment) cropSegment.crop = value;
  }

  let addSegmentButton = $state<HTMLButtonElement>();
  let recordModeSelect = $state<HTMLSelectElement>();
  const cut = $derived({
    segments: segments.map((s) => ({
      start_ms: Math.round((s.start ?? NaN) * 1000),
      end_ms: Math.round((s.end ?? NaN) * 1000),
      ...(s.crop ? { crop: normalizedCrop(s.crop) } : {}),
    })),
  });
  const cutDuration = $derived(cut.segments.reduce((n, s) => n + s.end_ms - s.start_ms, 0));
  const cutValid = $derived(
    !cutEnabled ||
      (segments.every((s) => !s.crop || normalizedCrop(s.crop) !== null) &&
        cut.segments.length > 0 &&
        cut.segments.length <= 64 &&
        cutDuration <= 180000 &&
        cut.segments.every(
          (s) =>
            Number.isSafeInteger(s.start_ms) &&
            Number.isSafeInteger(s.end_ms) &&
            s.start_ms >= 0 &&
            s.start_ms < s.end_ms &&
            s.end_ms <= 3600000 &&
            (sourceDuration === null || s.end_ms <= Math.floor(sourceDuration * 1000)),
        )),
  );
  let pollError = $state('');
  let reads = 0;
  let ready = $state(false),
    uploading = $state(false),
    cancelRequested = $state(false),
    cancelling = $state(false),
    confirming = $state(false);
  let cancelTarget: string | null = null;
  let importTarget: string | null = null;
  const active = $derived(
    cancelling || uploading || (job !== null && !terminal(job.phase) && !sourceReady),
  );
  $effect(() => {
    const busy = active || sourceReady || recording;
    untrack(() => onbusy(busy));
  });
  async function removeSegment(id: number) {
    if (active || recording || (!recordMode && segments.length === 1)) return;
    const index = segments.findIndex((s) => s.id === id);
    if (index < 0) return;
    const neighbour = segments[index + 1] ?? segments[index - 1];
    if (cropSegmentId === id) cropSegmentId = null;
    setSegments(segments.filter((s) => s.id !== id));
    await tick();
    if (mounted && !active) (neighbour?.input ?? addSegmentButton ?? recordModeSelect)?.focus();
  }
  function accept(next: Job | null) {
    job = next;
    if (next?.id === ownedId && next?.phase === 'source_ready' && initializedRangeId !== next.id) {
      initializedRangeId = next.id;
      resetRange(next.metadata);
    }
    if (next?.phase === 'complete' && announced !== next.id && (!existing || next.id === ownedId)) {
      announced = next.id;
      onimport(next.key, next.id === ownedId ? importTarget : null);
    }
  }
  async function refresh() {
    const request = ++reads;
    try {
      const next = await invoke<Job | null>('media_status');
      if (!mounted || uploading || request !== reads) return;
      accept(next);
      ready = true;
      pollError = '';
      // A failed/ambiguous cancellation is retriable only after readback of
      // that same job. Never unlock a newer job or overlap the pending RPC.
      if (
        cancelRequested &&
        !cancelling &&
        next?.id === cancelTarget &&
        next.phase !== 'publishing'
      ) {
        cancelRequested = false;
        cancelTarget = null;
      }
    } catch (e) {
      if (mounted && request === reads) {
        ready = false;
        pollError = message(e);
      }
    }
  }
  function confirmationFailed(id: string, cause: unknown) {
    // Polling may already have resolved the outcome or moved to another job.
    if (mounted && job?.id === id && job.phase !== 'complete') setError(cause, 'confirm');
  }
  async function confirmReview() {
    if (!job || job.phase !== 'review_ready' || confirming || cancelRequested || !ready) return;
    const id = job.id;
    confirming = true;
    reads++;
    error = null;
    try {
      await invoke('media_confirm', { id });
    } catch (e) {
      confirmationFailed(id, e);
    } finally {
      if (mounted) {
        confirming = false;
        await refresh();
      }
    }
  }
  async function cancel() {
    if (cancelling) return;
    cancelRequested = true;
    reads++;
    error = null;
    const id = ownedId ?? job?.id;
    if (!id) return;
    cancelTarget = id;
    cancelling = true;
    try {
      const next = await invoke<Job>('media_cancel', { id });
      if (mounted) accept(next);
    } catch (e) {
      // begin may still be in flight; the upload loop cancels again after its reply.
      if (mounted && !uploading) setError(e);
    } finally {
      cancelling = false;
      if (mounted && !uploading) {
        ready = false;
        await refresh();
      }
    }
  }
  async function importFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file || active || sourceReady || recording || !ready) return;
    error = null;
    if (editing) clearSource();
    if (!/\.(gif|mp4)$/i.test(file.name) || file.size === 0 || file.size > 256 * 1024 * 1024) {
      setError('', 'validation');
      return;
    }
    if (rangeEnabled) {
      clearSource();
      selectedFile = file;
      await uploadFile(file, true);
      return;
    }
    if (editing) {
      clearSource();
      selectedFile = file;
      return;
    }
    await uploadFile(file);
  }
  async function uploadFile(file: File, inspectOnly = false) {
    if (
      active ||
      sourceReady ||
      recording ||
      !ready ||
      (!inspectOnly && (!effectsValid || !cutValid || !rangeValid))
    )
      return;
    let inspectionStarted = false;
    error = null;
    const id = crypto.randomUUID().replaceAll('-', '');
    reads++;
    ownedId = id;
    importTarget = device;
    uploading = true;
    cancelRequested = false;
    cancelTarget = null;
    try {
      const initial = await invoke<Job>('media_begin', {
        id,
        name: file.name,
        total: file.size,
        fit,
        cut: cutEnabled ? cut : null,
        ...(adjustments ? { adjustments } : {}),
      });
      if (mounted) accept(initial);
      for (let offset = 0; offset < file.size; offset += 256 * 1024) {
        if (!mounted || cancelRequested) {
          await invoke('media_cancel', { id });
          return;
        }
        const bytes = Array.from(
          new Uint8Array(await file.slice(offset, offset + 256 * 1024).arrayBuffer()),
        );
        if (!mounted || cancelRequested) {
          await invoke('media_cancel', { id });
          return;
        }
        const next = await invoke<Job>('media_append', { id, offset, bytes });
        if (mounted) accept(next);
      }
      if (!mounted || cancelRequested) {
        await invoke('media_cancel', { id });
        return;
      }
      if (inspectOnly) {
        inspectionStarted = true;
        const inspected = await invoke<Job>('media_inspect', { id });
        if (!mounted || cancelRequested) await invoke('media_cancel', { id });
        else accept(inspected);
      } else await invoke('media_start', { id, review: editing });
    } catch (e) {
      if (mounted && !cancelRequested) setError(e, 'upload');
      // A lost upload acknowledgement must not leave staging permanently admitted.
      // Publishing rejects cancellation; the owned native task still records its result.
      if (!inspectionStarted)
        try {
          await invoke('media_cancel', { id });
        } catch {
          /* recover through status */
        }
    } finally {
      uploading = false;
      if (inspectionStarted) {
        if (!mounted || cancelRequested) await invoke('media_cancel', { id }).catch(() => {});
        else await refresh();
      }
    }
  }
  async function prepareInspected() {
    if (!sourceReady || !job || active || !ready || !rangeValid || !effectsValid) return;
    const id = job.id;
    uploading = true;
    reads++;
    error = null;
    cancelRequested = false;
    cancelTarget = null;
    try {
      await invoke('media_prepare_inspected', {
        id,
        fit,
        range,
        ...(adjustments ? { adjustments } : {}),
      });
    } catch (e) {
      if (mounted) setError(e, 'prepare');
    } finally {
      uploading = false;
      if (!mounted || cancelRequested) await invoke('media_cancel', { id }).catch(() => {});
      if (mounted) await refresh();
    }
  }
  async function prepareExisting() {
    if (!existing || active || recording || !ready || !effectsValid || !cutValid || !rangeValid)
      return;
    const id = crypto.randomUUID().replaceAll('-', '');
    reads++;
    ownedId = id;
    importTarget = device;
    uploading = true;
    cancelRequested = false;
    cancelTarget = null;
    error = null;
    try {
      await invoke('media_source_prepare', {
        sourceId: existing.id,
        id,
        name: existing.name.replace(/\.[^.]+$/, '') + '.mp4',
        fit,
        cut: cutEnabled ? cut : null,
        ...(range ? { range } : {}),
        ...(adjustments ? { adjustments } : {}),
      });
    } catch (e) {
      if (mounted && !cancelRequested) setError(e, 'prepare');
    } finally {
      uploading = false;
      if (!mounted || cancelRequested) await invoke('media_cancel', { id }).catch(() => {});
      if (mounted) await refresh();
    }
  }
  onMount(() => {
    mounted = true;
    if (existing) cutEnabled = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      await refresh();
      if (mounted) timer = setTimeout(poll, 400);
    }
    void poll();
    return () => {
      mounted = false;
      cancelRequested = true;
      clearTimeout(timer);
      const id = ownedId ?? job?.id;
      if (id && (uploading || (job && !terminal(job.phase) && job.phase !== 'publishing'))) {
        void invoke('media_cancel', { id }).catch(() => {});
      }
    };
  });
</script>

<section aria-label={t('Video import')} class="media-import">
  {#if existing}<p>
      {t('Editing «')}{existing.name}{t(
        '». The original file remains available to other themes and profiles.',
      )}
    </p>{:else}<StudioPopover label={t('Help')} icon="help"
      ><p>
        {t(
          'GIF or MP4, up to 1 hour and 256 MiB. The prepared video must also fit within 256 MiB. Import saves the video; playback on the LCD is selected separately.',
        )}
      </p></StudioPopover
    >{/if}
  <label
    >{t('Video placement')}
    <select bind:value={fit} disabled={active || recording || !ready}>
      <option value="contain">{t('Fit entire frame with black borders')}</option>
      <option value="cover">{t('Fill screen and crop edges')}</option>
      <option value="stretch">{t('AutoFill — stretch the entire frame')}</option>
    </select>
  </label>
  {#if fit === 'stretch'}<p>
      {t(
        'AutoFill fills the screen with the entire frame without cropping; aspect ratio may change.',
      )}
    </p>{/if}
  <label
    ><input
      type="checkbox"
      bind:checked={effectsEnabled}
      disabled={active || recording || !ready}
      onchange={() => {
        if (!cutEnabled && !effectsEnabled && !rangeEnabled) clearSource();
      }}
    />{t('Video color correction')}</label
  >
  {#if effectsEnabled}
    <fieldset disabled={active || recording || !ready}>
      <legend>{t('Color correction for the entire video')}</legend>
      <label
        >{t('Video contrast')}<input
          type="range"
          min="0"
          max="255"
          step="1"
          bind:value={contrast}
        /></label
      >
      <label
        >{t('Video contrast value')}<input
          type="number"
          min="0"
          max="255"
          step="1"
          bind:value={contrast}
        /></label
      >
      <label
        >{t('Video brightness')}<input
          type="range"
          min="0"
          max="255"
          step="1"
          bind:value={brightness}
        /></label
      >
      <label
        >{t('Video brightness value')}<input
          type="number"
          min="0"
          max="255"
          step="1"
          bind:value={brightness}
        /></label
      >
      <button
        onclick={() => {
          contrast = 0;
          brightness = 0;
        }}>{t('Reset color correction')}</button
      >
      <p>
        {t(
          '0 leaves the image unchanged. Brightness lightens it towards white. Settings apply to all selected segments.',
        )}
      </p>
      {#if !effectsValid}<p role="alert">
          {t('Enter integer brightness and contrast values from 0 to 255.')}
        </p>{/if}
    </fieldset>
  {/if}
  <label
    ><input
      type="checkbox"
      bind:checked={cutEnabled}
      onchange={() => {
        if (cutEnabled) rangeEnabled = false;
        else if (!effectsEnabled && !rangeEnabled) clearSource();
      }}
      disabled={active || sourceReady || recording || !ready}
    />{t('Select video segments')}</label
  >
  <label
    ><input
      type="checkbox"
      bind:checked={rangeEnabled}
      onchange={toggleRange}
      disabled={active || sourceReady || recording || !ready}
    />{t('Single VideoEdit range')}</label
  >
  {#if rangeEnabled}
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t(
          'Start and end of a single range. A new file is uploaded first to inspect duration and frame rate; encoding starts separately.',
        )}
      </p></StudioPopover
    >
    {#if rangeBounds}
      <fieldset disabled={active || recording || !ready}>
        <legend>{t('VideoEdit range')}</legend>
        <p>
          {t('Frame rate:')}
          {rangeBounds.fps.toFixed(3)}
          {t('FPS. End must not exceed')}
          {rangeBounds.maximumSeconds.toFixed(6)}
          {t('s; minimum length is')}
          {rangeBounds.minimumSpanSeconds.toFixed(6)}
          {t('s.')}
        </p>
        <label
          >{t('VideoEdit start')}<input
            type="range"
            min="0"
            max={rangeBounds.maximumSeconds}
            step="any"
            bind:value={rangeStart}
          /></label
        >
        <label
          >{t('VideoEdit start, s')}<input
            type="number"
            min="0"
            max={rangeBounds.maximumSeconds}
            step="any"
            bind:value={rangeStart}
          /></label
        >
        <label
          >{t('VideoEdit end')}<input
            type="range"
            min="0"
            max={rangeBounds.maximumSeconds}
            step="any"
            bind:value={rangeEnd}
          /></label
        >
        <label
          >{t('VideoEdit end, s')}<input
            type="number"
            min="0"
            max={rangeBounds.maximumSeconds}
            step="any"
            bind:value={rangeEnd}
          /></label
        >
        <button disabled={sourcePosition === null} onclick={() => markRange('start')}
          >{t('Set range start to current frame')}</button
        >
        <button disabled={sourcePosition === null} onclick={() => markRange('end')}
          >{t('Set range end to current frame')}</button
        >
        <button onclick={() => resetRange()}>{t('Entire available VideoEdit range')}</button>
        {#if !rangeValid}<p role="alert">
            {t('The range must span at least two source frames and stay within the indicated end.')}
          </p>{/if}
      </fieldset>
    {:else if sourceReady || existing}<p role="alert">
        {t(
          'VideoEdit has no valid duration or frame rate, or the video is too short. Cancel import before choosing another mode for a new file.',
        )}
      </p>
    {:else}<p>{t('Select a GIF/MP4 to inspect the source.')}</p>{/if}
  {/if}
  {#if cutEnabled}
    <p>
      {t(
        'Selecting a file opens a preview. After preparation you can review the result; saving it to the library requires confirmation.',
      )}
    </p>
    <label
      >{t('Segment selection method')}<select
        bind:this={recordModeSelect}
        bind:value={recordMode}
        disabled={active || recording || !ready}
        onchange={() => (cropSegmentId = null)}
      >
        <option value={false}>{t('Enter times manually')}</option><option value={true}
          >{t('Record during playback')}</option
        >
      </select></label
    >
    {#if recordMode}
      <fieldset disabled={active || recording || !ready}>
        <legend>{t('Next recording region, % of source')}</legend>
        {#each [{ key: 'x', label: 'Left' }, { key: 'y', label: 'Top' }, { key: 'width', label: 'Width' }, { key: 'height', label: 'Height' }] as field (field.key)}
          <label
            >{t(field.label)}{t(', % — recording')}<input
              type="number"
              min="0"
              max="100"
              step="0.01"
              bind:value={recordCrop[field.key as keyof CropFields]}
            /></label
          >
        {/each}
      </fieldset>
      {#if !previewCrop}<p role="alert">
          {t('The recording region must be nonempty and fit within the source.')}
        </p>{/if}
      <p>
        {t(
          'Record segment starts playback from the current position; Pause recording adds the selected interval. Between recordings you can seek and adjust the region. The manual list is retained separately. At 180 seconds, result preparation starts automatically; saving to the library requires confirmation.',
        )}
      </p>
    {/if}
    {#if cropSegment && !recordMode}<p role="status">
        {t('Region for segment')}
        {segments.indexOf(cropSegment) + 1}{t(
          '. Drag it below the video or enter percentages in the segment fields.',
        )}
      </p>{/if}
    <fieldset disabled={active || recording || !ready}>
      <legend>{t('Segments in playback order')}</legend>
      <StudioPopover label={t('Help')} icon="help"
        ><p>
          {t(
            'Enter start and end times in seconds. Segments are joined in order without audio, up to 3 minutes total. Up to 64 segments per import. Each segment end is checked against the selected file duration.',
          )}
        </p></StudioPopover
      >
      {#each segments as segment, i (segment.id)}
        <div class="segment">
          {#if recordMode}
            <p>
              {t('Segment')}
              {i + 1}: {segment.start?.toFixed(3)}–{segment.end?.toFixed(3)}
              {t('s. Region:')}
              {segment.crop
                ? `${segment.crop.x}%, ${segment.crop.y}%, ${segment.crop.width}% × ${segment.crop.height}%`
                : '' + t('entire frame') + ''}.
            </p>
          {:else}
            <label
              >{t('Segment start')}
              {i + 1}{t(', s')}<input
                type="number"
                min="0"
                max="3600"
                step="0.001"
                bind:value={segment.start}
                bind:this={segment.input}
              /></label
            >
            <label
              >{t('Segment end')}
              {i + 1}{t(', s')}<input
                type="number"
                min="0"
                max="3600"
                step="0.001"
                bind:value={segment.end}
              /></label
            >
            <button disabled={sourcePosition === null} onclick={() => markSegment(segment, 'start')}
              >{t('Set start to current frame')} {i + 1}</button
            >
            <button disabled={sourcePosition === null} onclick={() => markSegment(segment, 'end')}
              >{t('Set end to current frame')} {i + 1}</button
            >
            <button
              bind:this={segment.cropButton}
              aria-pressed={cropSegmentId === segment.id}
              onclick={() => chooseCrop(segment.id)}>{t('Choose segment region')} {i + 1}</button
            >
            {#if segment.crop}
              <fieldset aria-label={t('Segment region {0}', i + 1)}>
                <legend>{t('Segment region')} {i + 1}{t(', % of source')}</legend>
                {#each [{ key: 'x', label: 'Left' }, { key: 'y', label: 'Top' }, { key: 'width', label: 'Width' }, { key: 'height', label: 'Height' }] as field (field.key)}
                  <label
                    >{t(field.label)}{t(', % — segment')}
                    {i + 1}<input
                      type="number"
                      min="0"
                      max="100"
                      step="0.01"
                      bind:value={segment.crop[field.key as keyof CropFields]}
                    /></label
                  >
                {/each}
                {#if !normalizedCrop(segment.crop)}<p role="alert">
                    {t('The region must have positive dimensions and fit within the source.')}
                  </p>{/if}
                <button onclick={() => resetCrop(segment.id)}
                  >{t('Entire frame — segment')} {i + 1}</button
                >
              </fieldset>
            {/if}
          {/if}
          <button
            disabled={!recordMode && segments.length === 1}
            onclick={() => removeSegment(segment.id)}>{t('Delete segment')} {i + 1}</button
          >
        </div>
      {/each}
      {#if !recordMode}<button
          bind:this={addSegmentButton}
          disabled={segments.length >= 64}
          onclick={() => setSegments([...segments, { id: nextSegmentId++, start: 0, end: 5 }])}
          >{t('Add segment')}</button
        >{/if}
      {#if cutValid}<p>{t('Selected:')} {(cutDuration / 1000).toFixed(3)} {t('s.')}</p>
      {:else if recordMode && segments.length === 0}<p>{t('No recorded segments yet.')}</p>
      {:else}<p role="alert">
          {t(
            'The start must precede the end, within one hour. Total duration is limited to 180 seconds; the end must not exceed the selected video duration.',
          )}
        </p>{/if}
    </fieldset>
  {/if}
  {#if editing || existing}
    {#if selectedFile || existing}
      {#key selectedFile ?? existing?.id}
        <VideoSourcePreview
          bind:this={sourcePreview}
          file={selectedFile ?? undefined}
          external={existing}
          crop={cutEnabled ? previewCrop : null}
          {fit}
          {adjustments}
          oncrop={updateCrop}
          recordEnabled={cutEnabled && recordMode}
          remainingMs={180000 - cutDuration}
          canRecord={effectsValid && recordedSegments.length < 64 && previewCrop !== null}
          onrecord={addRecorded}
          onrecording={(value) => (recording = value)}
          disabled={active}
          onposition={(value) => (sourcePosition = value)}
          onmetadata={(value) => (sourceDuration = value)}
        />
      {/key}
    {/if}
  {/if}
  {#if !existing}<label
      >{t('Import GIF/MP4')}
      <input
        type="file"
        accept=".gif,.mp4,image/gif,video/mp4"
        disabled={active || sourceReady || recording || !ready}
        onchange={importFile}
      />
    </label>
  {/if}
  {#if existing}
    <button
      disabled={active || recording || !ready || !effectsValid || !cutValid || !rangeValid}
      onclick={prepareExisting}>{t('Prepare video edits')}</button
    >
  {:else if rangeEnabled}
    {#if sourceReady}<button
        disabled={active || !ready || !rangeValid || !effectsValid}
        onclick={prepareInspected}>{t('Prepare VideoEdit range')}</button
      >{/if}
  {:else if editing && selectedFile}
    <button
      disabled={active || recording || !ready || !effectsValid || !cutValid}
      onclick={() => {
        if (selectedFile) void uploadFile(selectedFile);
      }}
      >{cutEnabled
        ? '' + t('Prepare selected segments') + ''
        : '' + t('Prepare video') + ''}</button
    >
  {/if}
  {#if job}
    <p role="status" aria-live="polite">{t(labels[job.phase])}: {job.name}</p>
    {#if job.phase === 'uploading'}<progress
        value={job.received}
        max={job.total}
        aria-label={t('File upload')}
      ></progress>{/if}
    {#if job.phase === 'review_ready'}
      {#key job.id}<PreparedVideoPreview
          id={job.id}
          disabled={confirming || cancelRequested}
        />{/key}
      <button disabled={confirming || cancelRequested || !ready} onclick={confirmReview}
        >{t('Save prepared video')}</button
      >
    {/if}
    {#if job.message}<p role="alert">{job.message}</p>{/if}
    {#if job.phase === 'publishing'}<p>
        {t('Saving has already started. Wait for the result before importing again.')}
      </p>{/if}
  {/if}
  {#if active || sourceReady}<button
      disabled={cancelRequested || job?.phase === 'publishing'}
      onclick={cancel}>{t('Cancel import')}</button
    >{/if}
  {#if error || pollError}<p role="alert">{errorText()}</p>
    <button onclick={refresh}>{t('Check import')}</button>{/if}
</section>

<style>
  .media-import {
    margin: 16px 0;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  label {
    display: grid;
    gap: 6px;
    margin: 10px 0;
  }
  fieldset {
    min-width: 0;
  }
  .segment {
    margin: 12px 0;
  }
  input,
  select,
  progress {
    min-width: 0;
    max-width: 100%;
  }
  p {
    overflow-wrap: anywhere;
  }
</style>
