<script lang="ts">
  import type { Device } from './api';
  import { deviceCapabilities } from './device-capabilities';
  import DeviceGlyph from './DeviceGlyph.svelte';
  import ExperimentalNotice from './ExperimentalNotice.svelte';
  import { t } from './i18n.svelte';
  let {
    devices,
    connected,
    disabled,
    onopen,
  }: {
    devices: Device[];
    connected: boolean;
    disabled: boolean;
    onopen: (id: string, section: 'display' | 'lighting' | 'cooling' | 'settings') => void;
  } = $props();
</script>

<section class="devices" aria-label={t('Your devices')}>
  {#each devices as device (device.id)}
    {@const capabilities = deviceCapabilities(device)}
    <article class="device-card">
      <div class="device-art"><DeviceGlyph display={capabilities.display} /></div>
      <div class="device-description">
        <h2>{device.display_name}</h2>
        <p>{connected ? t('Device connected') : t('Waiting for connection')}</p>
        {#if device.vid != null && device.pid != null}<p class="device-identity">
            USB {device.vid.toString(16).padStart(4, '0')}:{device.pid
              .toString(16)
              .padStart(4, '0')}{#if device.serial}
              · {t('Serial number')}: {device.serial}{/if}
          </p>{/if}
        {#if device.family === 'galahad2-lcdv2'}<p>
            {t(
              'Controls the pump, its display, and fans connected to the LCDV2. Separate UNI FAN TL controllers are not supported yet.',
            )}
          </p>{/if}
        <ExperimentalNotice
          feature="device"
          label="Verification incomplete"
          message="This device has not completed full hardware verification. Use its controls at your own risk."
        />
        <div class="device-actions">
          {#if capabilities.lighting}<button
              disabled={disabled || !connected}
              onclick={() => onopen(device.id, 'lighting')}>{t('Lighting')}</button
            >{/if}
          {#if capabilities.cooling}<button
              disabled={disabled || !connected}
              onclick={() => onopen(device.id, 'cooling')}>{t('Cooling')}</button
            >{/if}
          {#if capabilities.display}<button
              disabled={disabled || !connected}
              onclick={() => onopen(device.id, 'display')}>{t('Pump screen')}</button
            >{/if}
          {#if !capabilities.display && !capabilities.lighting && !capabilities.cooling}<p>
              {t('No editors are available for this device family yet.')}
            </p>{/if}
        </div>
      </div>
    </article>
  {:else}
    <div class="empty-devices">
      <h2>{t('Your devices will appear here')}</h2>
      <p>
        {connected
          ? t(
              'Connect a supported Lian Li device. System information and settings are available without a device.',
            )
          : t('Waiting for the service. Your saved settings remain on this computer.')}
      </p>
    </div>
  {/each}
</section>

<style>
  .devices {
    display: grid;
    gap: 24px;
  }
  .device-card {
    display: grid;
    grid-template-columns: minmax(180px, 35%) 1fr;
    background: var(--surface);
    border-radius: 14px;
    overflow: hidden;
    min-height: 240px;
  }
  .device-art {
    background: var(--canvas);
    padding: 30px;
    display: grid;
    place-items: center;
  }
  .device-description {
    padding: 32px;
    align-self: center;
  }
  .device-description h2 {
    font-size: 24px;
    margin: 0 0 12px;
  }
  .device-description p,
  .empty-devices p {
    color: var(--text-muted);
    line-height: 1.7;
  }
  .device-identity {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .device-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 24px;
  }
  .empty-devices {
    max-width: 640px;
    padding: 32px 0;
  }
  .empty-devices h2 {
    font-size: 24px;
  }
  @media (max-width: 760px) {
    .device-card {
      grid-template-columns: 1fr;
    }
    .device-art {
      height: 170px;
    }
    .device-description {
      padding: 24px;
    }
  }
</style>
