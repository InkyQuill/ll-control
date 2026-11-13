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
  label={kind === 'font' ? t('Clean up unused fonts') : t('Clean up unused images')}
  text
>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t('Saved profiles, the current draft and')}
      {kind === 'font' ? '' + t('fonts') + '' : '' + t('images') + ''}{t(
        'used in this daemon session are retained. Session resources can be cleaned up after restarting if they are no longer used.',
      )}
    </p></StudioPopover
  >
  <button disabled={!connected || disabled || busy} onclick={() => scan(false)}
    >{kind === 'font' ? t('Check unused fonts') : t('Check unused images')}</button
  >
  {#if report}
    <p role="status">
      {deleted
        ? t('Deleted: {0}', report.deleted.length)
        : t('Removable: {0}', report.candidates.length)}
      {t('· In profiles/settings:')}
      {report.retained_referenced}
      {t('· In this session:')}
      {report.retained_session}
      {t('· In the draft:')}
      {report.retained_draft}
    </p>
    {#if !deleted && report.candidates.length}<button
        disabled={!connected || disabled || busy}
        onclick={() => scan(true)}
        >{kind === 'font' ? t('Delete unused fonts') : t('Delete unused images')}</button
      >{/if}
    {#each report.failures as failure (failure.key)}<p role="alert">
        {failure.key}: {failure.error}
      </p>{/each}
  {/if}
  {#if error}<p role="alert">
      {failedDelete ? t('Cleanup result is unconfirmed:') : t('Could not scan for unused assets:')}
      <span>{error}</span>
    </p>{/if}
</StudioPopover>

<style>
</style>
