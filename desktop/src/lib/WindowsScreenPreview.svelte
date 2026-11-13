<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  import { rpc, message } from './api';
  import LcdPreview from './LcdPreview.svelte';
  import type { DisplayGeometry } from './device-capabilities';
  import type { GroupTarget } from './group-archive';
  let {
    target,
    geometry,
    enabled,
    onbusy,
  }: {
    target: GroupTarget;
    geometry: DisplayGeometry | null;
    enabled: boolean;
    onbusy: (busy: boolean) => void;
  } = $props();
  let source = $state<'scene' | 'editor'>('scene'),
    url = $state(''),
    error = $state(''),
    busy = $state(false);
  let epoch = 0,
    alive = true;
  function clear() {
    if (url) URL.revokeObjectURL(url);
    url = '';
  }
  const scopeKey = $derived(JSON.stringify([source, target.id, target.dev, enabled]));
  $effect(() => {
    void scopeKey;
    epoch++;
    untrack(clear);
  });
  onDestroy(() => {
    alive = false;
    epoch++;
    clear();
    onbusy(false);
  });
  async function preview() {
    if (!enabled || busy) return;
    const current = ++epoch,
      request = { dev: target.dev, id: target.id, source };
    busy = true;
    onbusy(true);
    error = '';
    clear();
    let candidate = '';
    try {
      const bytes = await rpc<number[]>('preview_windows_screen_upload', request);
      if (!alive || current !== epoch) return;
      if (
        !Array.isArray(bytes) ||
        bytes.length > 1024 * 1024 ||
        bytes[0] !== 255 ||
        bytes[1] !== 216 ||
        bytes.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
      )
        throw new ValidationError('Invalid preview frame.');
      candidate = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }));
      const image = new Image();
      image.src = candidate;
      await image.decode();
      if (image.naturalWidth !== 480 || image.naturalHeight !== 480)
        throw new ValidationError('Invalid preview dimensions.');
      if (alive && current === epoch) {
        url = candidate;
        candidate = '';
      }
    } catch (e) {
      if (alive && current === epoch) error = message(e);
    } finally {
      if (candidate) URL.revokeObjectURL(candidate);
      if (alive) {
        busy = false;
        onbusy(false);
      }
    }
  }
</script>

<section class="group-preview" aria-label={t('Prepared screen preview')}>
  <h3>{t('Prepared frame')}</h3>
  <label
    >{t('Preview scene')}<select
      aria-label={t('Preview scene')}
      bind:value={source}
      disabled={!enabled || busy}
      ><option value="scene">{t('Current Windows scene')}</option><option value="editor"
        >{t('Imported editor')}</option
      ></select
    ></label
  >
  <button disabled={!enabled || busy} onclick={preview}>{t('Show prepared frame')}</button>
  {#if busy}<p role="status">{t('Rendering a frame from private preparation…')}</p>{/if}
  {#if url}<LcdPreview {geometry}
      ><img src={url} alt={t('Frame before Apply; physical output is unverified')} /></LcdPreview
    >{/if}
  <p>
    {t(
      'First video frame and current sensor readings, with LCD rotation. Preview does not publish files or change the screen.',
    )}
  </p>
  {#if error}<p role="alert">{t('Could not render the prepared frame: {0}', error)}</p>{/if}
</section>

<style>
  .group-preview {
    display: grid;
    gap: 14px;
    border-top: 1px solid var(--border);
    padding-top: 20px;
    margin-top: 20px;
  }
  .group-preview h3,
  .group-preview p {
    margin: 0;
  }
  .group-preview button {
    justify-self: start;
  }
  .group-preview p {
    color: var(--text-muted);
    line-height: 1.5;
  }
  .group-preview [role='alert'] {
    color: var(--danger);
  }
</style>
