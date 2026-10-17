<script lang="ts">
  import type { Device } from './api';
  import { deviceCapabilities } from './device-capabilities';
  import DeviceGlyph from './DeviceGlyph.svelte';
  import { t } from './i18n.svelte';
  let { devices, connected, disabled, onopen }: { devices: Device[]; connected: boolean; disabled: boolean; onopen: (id: string, section: 'display' | 'lighting' | 'cooling' | 'settings') => void } = $props();
</script>
<section class="devices" aria-label={t('Ваши устройства')}>
  {#each devices as device (device.id)}
    {@const capabilities = deviceCapabilities(device)}
    <article class="device-card">
      <div class="device-art"><DeviceGlyph display={capabilities.display}/></div>
      <div class="device-description"><h2>{device.display_name}</h2><p>{connected ? t('Устройство подключено') : t('Ожидание подключения')}</p>
        <div class="device-actions">
          {#if capabilities.lighting}<button disabled={disabled || !connected} onclick={() => onopen(device.id, 'lighting')}>{t('Подсветка')}</button>{/if}
          {#if capabilities.cooling}<button disabled={disabled || !connected} onclick={() => onopen(device.id, 'cooling')}>{t('Охлаждение')}</button>{/if}
          {#if capabilities.display}<button disabled={disabled || !connected} onclick={() => onopen(device.id, 'display')}>{t('Экран помпы')}</button>{/if}
          {#if !capabilities.display && !capabilities.lighting && !capabilities.cooling}<p>{t('Для этого семейства пока нет доступных редакторов.')}</p>{/if}
        </div>
      </div>
    </article>
  {:else}
    <div class="empty-devices"><h2>{t('Здесь появятся ваши устройства')}</h2><p>{connected ? t('Подключите поддерживаемое устройство Lian Li. Сведения о системе и настройки доступны без устройства.') : t('Ожидаем подключение к службе. Ваши сохранённые настройки остаются на компьютере.')}</p></div>
  {/each}
</section>
<style>
.devices{display:grid;gap:24px}.device-card{display:grid;grid-template-columns:minmax(180px,35%) 1fr;background:var(--surface);border-radius:14px;overflow:hidden;min-height:240px}.device-art{background:var(--canvas);padding:30px;display:grid;place-items:center}.device-description{padding:32px;align-self:center}.device-description h2{font-size:24px;margin:0 0 12px}.device-description p,.empty-devices p{color:var(--text-muted);line-height:1.7}.device-actions{display:flex;gap:10px;flex-wrap:wrap;margin-top:24px}.empty-devices{max-width:640px;padding:32px 0}.empty-devices h2{font-size:24px}@media(max-width:760px){.device-card{grid-template-columns:1fr}.device-art{height:170px}.device-description{padding:24px}}
</style>
