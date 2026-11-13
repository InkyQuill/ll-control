<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import { rpc, message } from './api';
  interface Report {
    candidates: string[];
    deleted: string[];
    retained_referenced: number;
    retained_session: number;
    retained_draft: number;
    skipped: number;
    failures: { key: string; error: string }[];
  }
  let {
    connected,
    disabled = false,
    keep,
    kind = 'image',
  }: { connected: boolean; disabled?: boolean; keep: string[]; kind?: 'image' | 'font' } = $props();
  let report = $state<Report | null>(null),
    error = $state(''),
    busy = $state(false),
    deleted = $state(false),
    failedDelete = $state(false);
  const keepKey = $derived(JSON.stringify([...new Set(keep)].sort()));
  let generation = 0;
  $effect(() => {
    void connected;
    void keepKey;
    void kind;
    generation++;
    report = null;
    error = '';
    deleted = false;
    failedDelete = false;
    return () => {
      generation++;
    };
  });
  async function scan(remove: boolean) {
    const started = generation;
    busy = true;
    error = '';
    failedDelete = remove;
    try {
      const result = await rpc<Report>(
        kind === 'font' ? 'cleanup_theme_fonts' : 'cleanup_theme_assets',
        { delete: remove, keep: [...new Set(keep)] },
      );
      if (started !== generation) return;
      report = result;
      deleted = remove;
    } catch (e) {
      if (started === generation) {
        error = message(e);
        report = null;
      }
    } finally {
      busy = false;
    }
  }
</script>

<StudioPopover
  label={`${t('Очистка неиспользуемых')} ${kind === 'font' ? '' + t('шрифтов') + '' : '' + t('изображений') + ''}`}
  text
>
  <StudioPopover label={t('Справка')} icon="help"
    ><p>
      {t('Сохранённые профили, текущий черновик и')}
      {kind === 'font' ? '' + t('шрифты') + '' : '' + t('изображения') + ''}{t(
        ', использованные в этом запуске daemon, сохраняются. Последние можно очистить после перезапуска, если они больше не используются.',
      )}
    </p></StudioPopover
  >
  <button disabled={!connected || disabled || busy} onclick={() => scan(false)}
    >{t('Проверить неиспользуемые')}
    {kind === 'font' ? '' + t('шрифты') + '' : '' + t('изображения') + ''}</button
  >
  {#if report}
    <p role="status">
      {deleted
        ? t('Удалено: {0}', report.deleted.length)
        : t('Можно удалить: {0}', report.candidates.length)}
      {t('· В профилях/настройках:')}
      {report.retained_referenced}
      {t('· В этом запуске:')}
      {report.retained_session}
      {t('· В черновике:')}
      {report.retained_draft}
    </p>
    {#if !deleted && report.candidates.length}<button
        disabled={!connected || disabled || busy}
        onclick={() => scan(true)}
        >{t('Удалить неиспользуемые')}
        {kind === 'font' ? '' + t('шрифты') + '' : '' + t('изображения') + ''}</button
      >{/if}
    {#each report.failures as failure (failure.key)}<p role="alert">
        {failure.key}: {failure.error}
      </p>{/each}
  {/if}
  {#if error}<p role="alert">
      {failedDelete ? t('Результат очистки не подтверждён:') : t('Не удалось проверить очистку:')}
      <span>{error}</span>
    </p>{/if}
</StudioPopover>

<style>
</style>
