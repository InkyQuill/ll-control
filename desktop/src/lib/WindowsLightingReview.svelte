<script lang="ts">
  import { t } from './i18n.svelte';
  import type { WindowsLightingReview, WindowsColor } from './windows-control';
  let { review }: { review: WindowsLightingReview } = $props();
  const palette = (colors: WindowsColor[]) =>
    colors
      .map((c) => `#${[c.r, c.g, c.b].map((v) => v.toString(16).padStart(2, '0')).join('')}`)
      .join(' · ') || '—';
</script>

<details class="windows-lighting-review">
  <summary
    >{t(
      'Review RGB: fans {0}, pump {1}',
      review.fan_modes.length,
      review.pump_modes.length,
    )}</summary
  >
  <p>
    {t(
      'Topology: {0} mm · manual LEDs {1} · sync {2}',
      review.fan_topology.size,
      review.fan_topology.manual_led_count,
      review.fan_topology.sync_to_pump ? t('On') : t('Off'),
    )}
  </p>
  <h3>{t('Fans')}</h3>
  {#each review.fan_modes as mode (mode.mode)}<p>
      <strong>{mode.mode}</strong> · {t('Brightness')}
      {mode.brightness_percent}% · {t('Speed')}
      {mode.speed_percent}% · {t('Direction')}
      {mode.direction}<br /><code>{palette([mode.color, ...(mode.additional_colors ?? [])])}</code>
    </p>{/each}
  <h3>{t('Pump')}</h3>
  {#each review.pump_modes as mode (mode.mode)}
    {@const dynamic = review.dynamic_rgb_profiles.modes.find((p) => p.mode === mode.mode)}
    <details class="mode">
      <summary>{mode.mode} · {mode.dynamic_mode ? t('Dynamic mode') : t('Manual mode')}</summary>
      <p>
        {t('Manual settings')} · {t('Brightness')}
        {mode.brightness_percent}% · {t('Speed')}
        {mode.speed_percent}% · {t('Direction')}
        {mode.direction}<br /><code>{palette([mode.color, ...(mode.additional_colors ?? [])])}</code
        >
      </p>
      {#each dynamic?.bindings ?? [] as binding, i (`${binding.sensor_id}-${binding.unit === 'celsius' ? '°C' : binding.unit === 'percent' ? '%' : 'RPM'}`)}
        <div class="binding">
          <p>
            <code>{binding.sensor_id}</code> · {binding.unit === 'celsius'
              ? '°C'
              : binding.unit === 'percent'
                ? '%'
                : 'RPM'}
            {#if i === (dynamic?.bindings.length ?? 0) - 1}<strong>· {t('Selected source')}</strong
              >{/if}
          </p>
          <p>
            {binding.low_value} → {binding.high_value} · {binding.brightness_percent ?? '—'}% · {binding.direction ??
              '—'}<br />{t('Low palette')}: <code>{palette(binding.low_colors)}</code><br />{t(
              'High palette',
            )}:
            <code>{palette(binding.high_colors)}</code>{#if binding.low_speed_percent !== null}<br
              />{t('Speed')}: {binding.low_speed_percent}% → {binding.high_speed_percent}%{/if}
          </p>
        </div>
      {/each}
    </details>
  {/each}
</details>

<style>
  details {
    margin-block: 16px;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  .mode,
  .binding {
    border-top: 1px solid var(--border);
    padding-block: 12px;
  }
  .binding {
    margin-left: 12px;
  }
  p {
    line-height: 1.6;
    overflow-wrap: anywhere;
  }
  code {
    user-select: text;
    overflow-wrap: anywhere;
  }
  h3 {
    margin-top: 24px;
  }
</style>
