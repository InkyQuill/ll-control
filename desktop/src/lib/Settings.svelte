<script lang="ts">
  import { t } from './i18n.svelte';
  import { rpc, message, type LcdState } from './api';
  let { device, connected, lcd }: { device: string; connected: boolean; lcd: LcdState | null } =
    $props();
  let brightness = $state(100),
    rotation = $state(0),
    busy = $state(false),
    error = $state(''),
    notice = $state('');
  let generation = 0;
  $effect(() => {
    void device;
    void connected;
    generation++;
    error = '';
    notice = '';
    return () => {
      generation++;
    };
  });
  async function apply(patch: { brightness_percent?: number; rotation_degrees?: number }) {
    const started = generation,
      dev = device;
    busy = true;
    error = '';
    notice = '';
    try {
      await rpc('patch_lcd_config', { dev, patch });
      if (started !== generation) return;
      notice = 'Setting sent. Other LCD settings were preserved.';
    } catch (e) {
      if (started === generation) error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label={t('LCD settings')}>
  <h2>{t('Brightness and orientation')}</h2>
  <p>
    {t('Requested settings:')}
    {lcd?.requested.brightness_percent ?? '—'}% · {lcd?.requested.rotation_degrees ?? '—'}{t(
      '°. Source:',
    )}
    {lcd?.source ?? '—'}.
  </p>
  <button
    disabled={!lcd || busy}
    onclick={() => {
      if (lcd) {
        brightness = lcd.requested.brightness_percent;
        rotation = lcd.requested.rotation_degrees;
      }
    }}>{t('Load current values')}</button
  >
  <label
    >{t('LCD brightness:')}
    {brightness}%<input type="range" min="0" max="100" bind:value={brightness} /></label
  >
  <button
    disabled={busy || !connected || !device}
    onclick={() => apply({ brightness_percent: brightness })}>{t('Apply brightness')}</button
  >
  <label
    >{t('Orientation')}<select bind:value={rotation}
      >{#each [0, 90, 180, 270] as angle (angle)}<option value={angle}>{angle}°</option
        >{/each}</select
    ></label
  >
  <button
    disabled={busy || !connected || !device}
    onclick={() => apply({ rotation_degrees: rotation })}>{t('Apply rotation')}</button
  >
  {#if lcd?.last_error}<p role="alert">{lcd.last_error}</p>{/if}
  {#if error}<p role="alert">
      {t('Could not send the LCD setting:')} <span>{error}</span>
    </p>{/if}{#if notice}<p role="status">{t(notice)}</p>{/if}
  <h2>{t('Background operation')}</h2>
  <p>
    {t(
      'Settings and the theme are stored by the daemon and keep running when this window closes. Connection: the local Unix socket in XDG_RUNTIME_DIR.',
    )}
  </p>
</section>

<style>
  section {
    max-width: 620px;
    display: grid;
    gap: 20px;
  }
  button {
    justify-self: start;
  }
  p {
    color: var(--text-muted);
    line-height: 1.7;
  }
  h2 {
    font-size: 19px;
  }
  input,
  select {
    max-width: 400px;
  }
  [role='alert'] {
    color: var(--danger);
  }
  [role='status'] {
    color: var(--success);
  }
</style>
