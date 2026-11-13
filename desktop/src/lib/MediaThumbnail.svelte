<script lang="ts">
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  import { requestThumbnail } from './media-thumbnails';
  import { message } from './api';
  let {
    clipKey,
    name,
    compact = false,
  }: { clipKey: string; name: string; compact?: boolean } = $props();
  // Metadata polling replaces gallery rows; only a new immutable key needs decoding.
  const requestedClipKey = $derived(clipKey);
  let source = $state(''),
    error = $state<ValidationError | string | null>(null),
    loading = $state(false),
    retry = $state(0);
  $effect(() => {
    const key = requestedClipKey;
    void retry;
    const controller = new AbortController();
    let active = true,
      objectUrl = '';
    source = '';
    error = null;
    loading = true;
    async function load() {
      try {
        const bytes = await requestThumbnail(key, controller.signal);
        if (!active) return;
        objectUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/png' }));
        source = objectUrl;
      } catch (e) {
        if (active) error = e instanceof ValidationError ? e : message(e);
      } finally {
        if (active) loading = false;
      }
    }
    void load();
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      controller.abort();
    };
  });
  function imageFailed(event: Event) {
    if (!source || (event.currentTarget as HTMLImageElement).getAttribute('src') !== source) return;
    error = new ValidationError('Could not display the thumbnail. Retry the request.');
    source = '';
  }
</script>

<figure class:compact>
  {#if source}{#key source}<img
        src={source}
        alt={t('First frame: {0}', name)}
        width="144"
        height="144"
        onerror={imageFailed}
      />{/key}{/if}
  {#if loading}<p role="status">{t('Preparing thumbnail…')}</p>{/if}
  {#if error}<p role="status">
      {t('Thumbnail unavailable:')}
      {#if error instanceof ValidationError}{t(error.resource, ...error.values)}{:else}<span
          >{error}</span
        >{/if}
    </p>{/if}
  {#if !compact || error}<button disabled={loading} onclick={() => retry++}
      >{compact ? t('Retry thumbnail: {0}', name) : '' + t('Refresh thumbnail') + ''}</button
    >{/if}
  <figcaption>{t('First video frame. Preview does not start the LCD.')}</figcaption>
</figure>

<style>
  figure {
    margin: 12px 0;
  }
  .compact {
    margin: 8px 0;
  }
  .compact figcaption {
    font-size: 12px;
  }
  img {
    display: block;
    max-width: 100%;
    height: auto;
    border-radius: 8px;
  }
  figcaption,
  p {
    overflow-wrap: anywhere;
    margin: 8px 0;
  }
</style>
