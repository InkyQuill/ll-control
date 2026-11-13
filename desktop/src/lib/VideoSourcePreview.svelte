<script lang="ts">
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  import { onMount } from 'svelte';
  import type { VideoCrop } from './video-crop';
  import { videoChannelLut, type VideoAdjustments } from './video-adjustments';
  let {
    file,
    external,
    disabled = false,
    onposition,
    onmetadata,
    crop = null,
    fit = 'contain',
    adjustments = null,
    oncrop,
    recordEnabled = false,
    remainingMs = 180000,
    canRecord = true,
    onrecord,
    onrecording,
  }: {
    file?: File;
    external?: { url: string; name: string };
    disabled?: boolean;
    adjustments?: VideoAdjustments | null;
    crop?: VideoCrop | null;
    fit?: 'contain' | 'cover' | 'stretch';
    oncrop?: (crop: VideoCrop) => void;
    recordEnabled?: boolean;
    remainingMs?: number;
    canRecord?: boolean;
    onrecord?: (segment: { start_ms: number; end_ms: number; crop: VideoCrop | null }) => void;
    onrecording?: (active: boolean) => void;
    onposition: (seconds: number | null) => void;
    onmetadata: (seconds: number | null) => void;
  } = $props();
  let player = $state<HTMLVideoElement>();
  let url = $state(''),
    duration = $state(0),
    position = $state(0),
    failed = $state(false),
    seeking = $state(false);
  let sourceCanvas = $state<HTMLCanvasElement>(),
    croppedCanvas = $state<HTMLCanvasElement>();
  let sourceWidth = $state(0),
    sourceHeight = $state(0),
    hasFrame = $state(false);
  let drag: { id: number; x: number; y: number } | null = null;
  const canvasWidth = $derived(
    sourceWidth && sourceHeight
      ? Math.max(1, Math.round(Math.min(480 / sourceWidth, 360 / sourceHeight) * sourceWidth))
      : 480,
  );
  const canvasHeight = $derived(
    sourceWidth && sourceHeight
      ? Math.max(1, Math.round(Math.min(480 / sourceWidth, 360 / sourceHeight) * sourceHeight))
      : 480,
  );
  let mounted = false;
  let recording = $state<{ start: number; limit: number; crop: VideoCrop | null } | null>(null);
  type RecordingNotice =
    'playback-failed' | 'limit' | 'added' | 'empty' | 'stopped' | 'source-failed' | 'seeked';
  let recordNotice = $state<RecordingNotice | null>(null);
  function recordingNoticeText() {
    switch (recordNotice) {
      case 'playback-failed':
        return t('Could not start playback. No segment was added.');
      case 'limit':
        return t('Reached the end of the video or the selected-segment limit.');
      case 'added':
        return t('Segment added.');
      case 'empty':
        return t('Empty segment was not added.');
      case 'stopped':
        return t('Recording stopped.');
      case 'source-failed':
        return t('Source unavailable; the unfinished segment was not added.');
      case 'seeked':
        return t(
          'The playback position changed during recording; the unfinished segment was not added.',
        );
      default:
        return '';
    }
  }
  async function startRecord() {
    const seconds = currentPosition();
    if (
      !recordEnabled ||
      !canRecord ||
      disabled ||
      recording ||
      seconds === null ||
      !player ||
      remainingMs <= 0
    )
      return;
    const start = Math.round(seconds * 1000),
      limit = Math.min(Math.floor(duration * 1000), start + remainingMs);
    if (start >= limit) return;
    recording = { start, limit, crop: crop ? { ...crop } : null };
    const attempt = recording,
      video = player;
    recordNotice = null;
    onrecording?.(true);
    try {
      await video.play();
      if (!mounted || !recording) video.pause();
    } catch {
      if (mounted && recording === attempt) {
        recording = null;
        onrecording?.(false);
        recordNotice = 'playback-failed';
      }
    }
  }
  function finishRecord() {
    const captured = recording;
    if (!mounted || !captured || !player) return;
    recording = null;
    player.pause();
    const end = Math.min(captured.limit, Math.round(player.currentTime * 1000));
    onrecording?.(false);
    if (Number.isFinite(end) && end > captured.start) {
      onrecord?.({ start_ms: captured.start, end_ms: end, crop: captured.crop });
      recordNotice = end === captured.limit ? 'limit' : 'added';
    } else recordNotice = 'empty';
  }
  function discardRecord(reason: RecordingNotice) {
    if (!recording) return;
    recording = null;
    player?.pause();
    onrecording?.(false);
    recordNotice = reason;
  }

  const lut = $derived(adjustments ? videoChannelLut(adjustments) : null);
  let effectCanvas: HTMLCanvasElement | undefined;
  let paintError = $state(false);
  function paint() {
    try {
      paintFrame();
      paintError = false;
    } catch {
      paintError = true;
    }
  }
  function paintFrame() {
    if (
      !mounted ||
      !player ||
      player.readyState < 2 ||
      (!crop && !adjustments) ||
      !croppedCanvas ||
      failed
    )
      return;
    const sourceContext = sourceCanvas?.getContext('2d'),
      result = croppedCanvas.getContext('2d');
    if (!result) return;
    if (crop && sourceCanvas && sourceContext) {
      sourceContext.drawImage(player, 0, 0, sourceCanvas.width, sourceCanvas.height);
      sourceContext.strokeStyle = '#fff';
      sourceContext.lineWidth = 3;
      sourceContext.strokeRect(
        crop.x * sourceCanvas.width,
        crop.y * sourceCanvas.height,
        crop.width * sourceCanvas.width,
        crop.height * sourceCanvas.height,
      );
      sourceContext.strokeStyle = '#000';
      sourceContext.lineWidth = 1;
      sourceContext.strokeRect(
        crop.x * sourceCanvas.width,
        crop.y * sourceCanvas.height,
        crop.width * sourceCanvas.width,
        crop.height * sourceCanvas.height,
      );
    }
    let frame: CanvasImageSource = player;
    if (lut && adjustments && (adjustments.contrast !== 0 || adjustments.brightness !== 0)) {
      if (player.videoWidth > 4096 || player.videoHeight > 4096)
        throw new ValidationError('Preview dimensions exceed preparation budget');
      effectCanvas ??= document.createElement('canvas');
      if (effectCanvas.width !== player.videoWidth) effectCanvas.width = player.videoWidth;
      if (effectCanvas.height !== player.videoHeight) effectCanvas.height = player.videoHeight;
      const context = effectCanvas.getContext('2d', { willReadFrequently: true });
      if (!context) throw new ValidationError('Canvas unavailable');
      context.drawImage(player, 0, 0);
      const pixels = context.getImageData(0, 0, effectCanvas.width, effectCanvas.height);
      for (let i = 0; i < pixels.data.length; i += 4) {
        pixels.data[i] = lut[pixels.data[i]];
        pixels.data[i + 1] = lut[pixels.data[i + 1]];
        pixels.data[i + 2] = lut[pixels.data[i + 2]];
      }
      context.putImageData(pixels, 0, 0);
      frame = effectCanvas;
    }
    const region = crop ?? { x: 0, y: 0, width: 1, height: 1 };
    const x = Math.floor(player.videoWidth * region.x),
      y = Math.floor(player.videoHeight * region.y);
    const width = Math.max(1, Math.floor(player.videoWidth * region.width)),
      height = Math.max(1, Math.floor(player.videoHeight * region.height));
    const scale =
      fit === 'contain' ? Math.min(480 / width, 480 / height) : Math.max(480 / width, 480 / height);
    result.fillStyle = '#000';
    result.fillRect(0, 0, 480, 480);
    if (fit === 'stretch') result.drawImage(frame, x, y, width, height, 0, 0, 480, 480);
    else
      result.drawImage(
        frame,
        x,
        y,
        width,
        height,
        (480 - width * scale) / 2,
        (480 - height * scale) / 2,
        width * scale,
        height * scale,
      );
  }
  // Canvas is an external drawing surface. Repaint when its inputs or frame change.
  $effect(() => {
    void position;
    void crop;
    void fit;
    void adjustments;
    void hasFrame;
    void canvasWidth;
    void canvasHeight;
    paint();
  });
  function point(event: PointerEvent): { x: number; y: number } {
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    return {
      x:
        Math.round(Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)) * 10000) /
        10000,
      y:
        Math.round(Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)) * 10000) /
        10000,
    };
  }
  function beginCrop(event: PointerEvent) {
    if (disabled || recording || !crop || event.button !== 0 || !hasFrame) return;
    drag = { id: event.pointerId, ...point(event) };
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function finishCrop(event: PointerEvent) {
    const start = drag;
    drag = null;
    if (disabled || recording || !crop || !start || start.id !== event.pointerId) return;
    const button = event.currentTarget as HTMLButtonElement;
    if (!button.hasPointerCapture(event.pointerId)) return;
    button.releasePointerCapture(event.pointerId);
    const end = point(event),
      width = Math.round(Math.abs(end.x - start.x) * 10000) / 10000,
      height = Math.round(Math.abs(end.y - start.y) * 10000) / 10000;
    if (width > 0 && height > 0)
      oncrop?.({ x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width, height });
  }
  function moveCrop(event: KeyboardEvent) {
    if (
      disabled ||
      recording ||
      !crop ||
      !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)
    )
      return;
    event.preventDefault();
    const dx = event.key === 'ArrowLeft' ? -0.01 : event.key === 'ArrowRight' ? 0.01 : 0;
    const dy = event.key === 'ArrowUp' ? -0.01 : event.key === 'ArrowDown' ? 0.01 : 0;
    oncrop?.({
      ...crop,
      x: Math.max(0, Math.min(1 - crop.width, crop.x + dx)),
      y: Math.max(0, Math.min(1 - crop.height, crop.y + dy)),
    });
  }

  const playable = $derived(duration > 0 && !failed);
  onMount(() => {
    mounted = true;
    const source = external?.url ?? (file ? URL.createObjectURL(file) : '');
    url = source;
    return () => {
      mounted = false;
      recording = null;
      onrecording?.(false);
      player?.pause();
      player?.removeAttribute('src');
      player?.load();
      if (!external) URL.revokeObjectURL(source);
    };
  });
  $effect(() => {
    if (disabled || !recordEnabled) discardRecord('stopped');
    if (disabled) player?.pause();
  });
  export function currentExactPosition(): number | null {
    if (!mounted || disabled || !playable || !player || seeking || player.seeking) return null;
    return Number.isFinite(player.currentTime) ? player.currentTime : null;
  }
  export function currentPosition(): number | null {
    if (!mounted || disabled || !playable || !player || seeking || player.seeking) return null;
    const value = player.currentTime;
    return Number.isFinite(value)
      ? Math.min(Math.floor(duration * 1000), Math.round(value * 1000)) / 1000
      : null;
  }
  function metadata(event: Event) {
    const target = event.currentTarget as HTMLVideoElement;
    if (!mounted || target !== player || target.currentSrc !== url) return;
    const value = target.duration;
    if (!Number.isFinite(value) || value <= 0) {
      fail();
      return;
    }
    sourceWidth = target.videoWidth;
    sourceHeight = target.videoHeight;
    duration = value;
    failed = false;
    onmetadata(value);
    update(event);
  }
  function update(event: Event) {
    const target = event.currentTarget as HTMLVideoElement;
    if (!mounted || target !== player || target.currentSrc !== url || !playable) return;
    position = target.currentTime;
    hasFrame = target.readyState >= 2;
    paint();
    if (recording && Math.round(position * 1000) >= recording.limit) finishRecord();
    onposition(seeking ? null : Math.round(position * 1000) / 1000);
  }
  function fail() {
    if (!mounted) return;
    discardRecord('source-failed');
    failed = true;
    hasFrame = false;
    duration = 0;
    onposition(null);
    onmetadata(null);
  }
  function seek(event: Event) {
    if (disabled || recording || !playable || !player) return;
    const value = (event.currentTarget as HTMLInputElement).valueAsNumber;
    if (Number.isFinite(value)) {
      seeking = true;
      onposition(null);
      try {
        player.currentTime = Math.max(0, Math.min(value, duration));
      } catch {
        fail();
      }
    }
  }
</script>

<section aria-label={t('Source video preview')}>
  <p>
    {t('Source file:')}
    {external?.name ?? file?.name}{t(
      '. Preview is silent; LCD placement is applied during import.',
    )}
  </p>
  {#if url}
    <video
      crossorigin="anonymous"
      bind:this={player}
      src={url}
      controls={!disabled && !recording}
      muted
      playsinline
      preload="auto"
      aria-label={t('Source video')}
      onloadedmetadata={metadata}
      onloadeddata={update}
      ontimeupdate={update}
      onseeking={() => {
        discardRecord('seeked');
        seeking = true;
        onposition(null);
      }}
      onpause={() => {
        if (player?.paused) finishRecord();
      }}
      onended={finishRecord}
      onseeked={(event) => {
        seeking = false;
        update(event);
      }}
      onerror={fail}
    ></video>
  {/if}
  {#if playable}
    <label
      >{t('Source video position')}
      <input
        type="range"
        min="0"
        max={duration}
        step="0.001"
        value={position}
        disabled={disabled || !!recording}
        oninput={seek}
      />
    </label>
    <p>{t('Position:')} {position.toFixed(3)} {t('s /')} {duration.toFixed(3)} {t('s.')}</p>
  {/if}
  {#if recordEnabled}
    <button
      disabled={disabled ||
        (!recording &&
          (!canRecord || !playable || seeking || remainingMs <= 0 || position >= duration))}
      onclick={() => (recording ? finishRecord() : startRecord())}
      >{recording ? '' + t('Pause recording') + '' : '' + t('Record segment') + ''}</button
    >
    {#if recording}<p role="status">{t('Recording a segment.')}</p>
      <p>
        {t('Recorded:')}
        {(Math.max(0, Math.min(position * 1000, recording.limit) - recording.start) / 1000).toFixed(
          3,
        )}
        {t('s.')}
      </p>{/if}
    {#if recordNotice}<p role="status">{recordingNoticeText()}</p>{/if}
  {/if}
  {#if crop && playable && hasFrame}
    <p>
      {t(
        'Drag from one corner of the region to the other. Arrow keys move the region; exact dimensions are available in the segment fields.',
      )}
    </p>
    <button
      class="crop-source"
      disabled={disabled || !!recording}
      aria-label={t('Video region selection')}
      onpointerdown={beginCrop}
      onpointerup={finishCrop}
      onpointercancel={() => (drag = null)}
      onlostpointercapture={() => (drag = null)}
      onkeydown={moveCrop}
    >
      <canvas bind:this={sourceCanvas} width={canvasWidth} height={canvasHeight} aria-hidden="true"
      ></canvas>
    </button>
  {/if}
  {#if crop || adjustments}
    <p>
      {t(
        'Frame after color correction, cropping and placement. This is a local preview; the final video is prepared during import.',
      )}
    </p>
    <canvas bind:this={croppedCanvas} width="480" height="480" aria-label={t('Cropped frame')}
    ></canvas>
  {/if}
  {#if paintError}<p role="status">
      {t('Color preview is unavailable. Check the prepared video after conversion.')}
    </p>{/if}
  {#if failed}
    <p role="status">
      {t(
        'This file cannot be previewed. You can enter times manually; the file is checked separately during import.',
      )}
    </p>
  {/if}
</section>

<style>
  video {
    display: block;
    width: 100%;
    max-width: 480px;
    max-height: 360px;
    background: #000;
  }
  input {
    width: 100%;
    min-width: 0;
  }
  canvas {
    display: block;
    max-width: 100%;
    height: auto;
  }
  .crop-source {
    display: block;
    padding: 0;
    border: 0;
    max-width: 100%;
    touch-action: none;
    cursor: crosshair;
  }
  .crop-source:focus-visible {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }
  p {
    overflow-wrap: anywhere;
  }
</style>
