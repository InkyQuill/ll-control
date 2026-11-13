<script lang="ts">
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  import { onMount } from 'svelte';
  import type { RecordingSession } from './screen-record';
  import type { VideoCrop } from './video-crop';
  let {
    session,
    crop,
    disabled = false,
    onchange,
  }: {
    session: RecordingSession;
    crop: VideoCrop | null;
    disabled?: boolean;
    onchange: (region: VideoCrop) => void;
  } = $props();
  let picture = $state<HTMLImageElement>(),
    source = $state<HTMLCanvasElement>(),
    result = $state<HTMLCanvasElement>();
  let loading = $state(false),
    error = $state('');
  const width = $derived(picture?.naturalWidth ?? 1),
    height = $derived(picture?.naturalHeight ?? 1);
  let active = $state(false),
    wanted = $state(true);
  let revision = 0,
    ownedUrl: string | undefined;
  let drag: { id: number; x: number; y: number } | null = null;
  async function load() {
    if (loading || disabled) return;
    const attempt = ++revision;
    loading = true;
    error = '';
    picture = undefined;
    if (ownedUrl) {
      URL.revokeObjectURL(ownedUrl);
      ownedUrl = undefined;
    }
    let url: string | undefined;
    try {
      const bytes = await session.snapshot();
      if (!active || attempt !== revision || !bytes) return;
      url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }));
      const image = new Image();
      image.src = url;
      await image.decode();
      if (!active || attempt !== revision) return;
      if (
        !image.naturalWidth ||
        !image.naturalHeight ||
        image.naturalWidth > 640 ||
        image.naturalHeight > 640
      )
        throw new ValidationError('Unexpected preview dimensions');
      ownedUrl = url;
      picture = image;
    } catch {
      if (active && attempt === revision)
        error = 'Could not load the source frame. Retry or enter the region coordinates.';
    } finally {
      if (url && url !== ownedUrl) URL.revokeObjectURL(url);
      if (active && attempt === revision) loading = false;
    }
  }
  function paint() {
    if (!picture || !source || !result) return;
    const full = source.getContext('2d'),
      selected = result.getContext('2d');
    if (!full || !selected) return;
    full.drawImage(picture, 0, 0, width, height);
    selected.clearRect(0, 0, width, height);
    if (crop) {
      full.strokeStyle = '#fff';
      full.lineWidth = 3;
      full.strokeRect(crop.x * width, crop.y * height, crop.width * width, crop.height * height);
      full.strokeStyle = '#000';
      full.lineWidth = 1;
      full.strokeRect(crop.x * width, crop.y * height, crop.width * width, crop.height * height);
      selected.drawImage(
        picture,
        crop.x * width,
        crop.y * height,
        crop.width * width,
        crop.height * height,
        0,
        0,
        width,
        height,
      );
    }
  }
  // Canvas is an external drawing surface; repaint from the current frame/draft.
  $effect(() => {
    void picture;
    void crop;
    void source;
    void result;
    void width;
    void height;
    paint();
  });
  $effect(() => {
    if (disabled) drag = null;
  });
  $effect(() => {
    if (active && wanted && !disabled) {
      wanted = false;
      void load();
    }
  });
  function point(event: PointerEvent) {
    const bounds = source!.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
      y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)),
    };
  }
  function start(event: PointerEvent) {
    if (disabled || loading || !picture || !source || event.button !== 0) return;
    const button = event.currentTarget as HTMLButtonElement;
    button.focus();
    drag = { id: event.pointerId, ...point(event) };
    button.setPointerCapture(event.pointerId);
  }
  function select(event: PointerEvent) {
    if (disabled || loading || !picture || !drag || drag.id !== event.pointerId) return;
    const button = event.currentTarget as HTMLButtonElement;
    const captured = button.hasPointerCapture(event.pointerId);
    // A missed release must not extend the last selection on the next move.
    // Normal pointerup has buttons=0 and still finalizes the captured gesture.
    if (!captured || (event.type === 'pointermove' && (event.buttons & 1) === 0)) {
      drag = null;
      if (captured) button.releasePointerCapture(event.pointerId);
      return;
    }
    const end = point(event),
      w = Math.abs(end.x - drag.x),
      h = Math.abs(end.y - drag.y);
    if (w > 0 && h > 0)
      onchange({ x: Math.min(end.x, drag.x), y: Math.min(end.y, drag.y), width: w, height: h });
  }
  function finish(event: PointerEvent) {
    if (!drag || drag.id !== event.pointerId) return;
    select(event);
    drag = null;
    const button = event.currentTarget as HTMLButtonElement;
    if (button.hasPointerCapture(event.pointerId)) button.releasePointerCapture(event.pointerId);
  }
  function move(event: KeyboardEvent) {
    if (
      disabled ||
      loading ||
      !crop ||
      !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)
    )
      return;
    event.preventDefault();
    const step = event.shiftKey ? 0.1 : 0.01;
    const dx = event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0;
    const dy = event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0;
    onchange({
      ...crop,
      x: Math.max(0, Math.min(1 - crop.width, crop.x + dx)),
      y: Math.max(0, Math.min(1 - crop.height, crop.y + dy)),
    });
  }
  onMount(() => {
    active = true;
    return () => {
      active = false;
      revision++;
      drag = null;
      if (ownedUrl) URL.revokeObjectURL(ownedUrl);
    };
  });
</script>

<section aria-label={t('Frame for capture region selection')}>
  <p>
    {t(
      'The latest available full source frame. Drag from corner to corner; arrow keys move the region by 1%, or 10% with Shift. Adjust dimensions in the fields below.',
    )}
  </p>
  {#if loading}<p role="status">{t('Loading source frame…')}</p>{/if}
  {#if error}<p role="status">{t(error)}</p>{/if}
  {#if picture}
    <button
      class="source"
      disabled={disabled || loading}
      aria-label={t('Capture region selection on frame')}
      onpointerdown={start}
      onpointermove={select}
      onpointerup={finish}
      onpointercancel={() => (drag = null)}
      onlostpointercapture={() => (drag = null)}
      onkeydown={move}
    >
      <canvas bind:this={source} {width} {height} aria-hidden="true"></canvas>
    </button>
    <p>{t('Selected region stretched to the recording size:')}</p>
    <canvas bind:this={result} {width} {height} aria-label={t('Capture region preview')}></canvas>
  {/if}
  <button disabled={disabled || loading} onclick={load}>{t('Refresh source frame')}</button>
</section>

<style>
  canvas {
    display: block;
    max-width: 100%;
    height: auto;
    background: #000;
  }
  .source {
    display: block;
    width: fit-content;
    max-width: 100%;
    padding: 0;
    border: 0;
    touch-action: none;
    cursor: crosshair;
  }
  .source:focus-visible {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }
  p {
    overflow-wrap: anywhere;
  }
</style>
