<script lang="ts">
  import { t } from './i18n.svelte';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  let { id, disabled = false }: { id: string; disabled?: boolean } = $props();
  let player = $state<HTMLVideoElement>(),
    url = $state(''),
    failed = $state(false);
  onMount(() => {
    let mounted = true;
    void invoke<string>('media_playback_url', { id, kind: 'review' })
      .then((value) => {
        if (mounted) url = value;
      })
      .catch(() => {
        if (mounted) failed = true;
      });
    return () => {
      mounted = false;
      player?.pause();
      player?.removeAttribute('src');
      player?.load();
    };
  });
  $effect(() => {
    if (disabled) player?.pause();
  });
</script>

<section aria-label={t('Проверка готового ролика')}>
  <p>
    {t(
      'Готовый ролик после обрезки и склейки. Проверьте результат и подтвердите сохранение в библиотеку.',
    )}
  </p>
  {#if url}
    <video
      crossorigin="anonymous"
      bind:this={player}
      src={url}
      muted
      playsinline
      controls={!disabled}
      preload="metadata"
      aria-label={t('Готовый ролик')}
      onerror={() => (failed = true)}
    ></video>
  {/if}
  {#if failed}<p role="status">
      {t(
        'Не удалось воспроизвести готовый ролик в этом окне. Можно сохранить подготовленный файл или отменить импорт.',
      )}
    </p>{/if}
</section>

<style>
  video {
    display: block;
    width: 100%;
    max-width: 480px;
    max-height: 480px;
    background: #000;
  }
</style>
