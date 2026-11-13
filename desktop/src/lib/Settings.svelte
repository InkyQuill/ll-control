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
      notice = 'Настройка отправлена. Остальные параметры LCD сохранены.';
    } catch (e) {
      if (started === generation) error = message(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label={t('Настройки LCD')}>
  <h2>{t('Яркость и ориентация')}</h2>
  <p>
    {t('Текущее requested:')}
    {lcd?.requested.brightness_percent ?? '—'}% · {lcd?.requested.rotation_degrees ?? '—'}{t(
      '°. Источник:',
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
    }}>{t('Загрузить текущие значения')}</button
  >
  <label
    >{t('Яркость LCD:')}
    {brightness}%<input type="range" min="0" max="100" bind:value={brightness} /></label
  >
  <button
    disabled={busy || !connected || !device}
    onclick={() => apply({ brightness_percent: brightness })}>{t('Применить яркость')}</button
  >
  <label
    >{t('Ориентация')}<select bind:value={rotation}
      >{#each [0, 90, 180, 270] as angle (angle)}<option value={angle}>{angle}°</option
        >{/each}</select
    ></label
  >
  <button
    disabled={busy || !connected || !device}
    onclick={() => apply({ rotation_degrees: rotation })}>{t('Применить поворот')}</button
  >
  {#if lcd?.last_error}<p role="alert">{lcd.last_error}</p>{/if}
  {#if error}<p role="alert">
      {t('Не удалось отправить настройку LCD:')} <span>{error}</span>
    </p>{/if}{#if notice}<p role="status">{t(notice)}</p>{/if}
  <h2>{t('Работа в фоне')}</h2>
  <p>
    {t(
      'Настройки и тема хранятся в daemon. Закрытие окна их не останавливает. Подключение: локальный Unix socket из XDG_RUNTIME_DIR.',
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
