<script lang="ts">
  import { t } from './i18n.svelte';
  let {
    kind,
    color,
    brightness,
    disabled,
    colorCount,
  }: {
    kind: 'pump' | 'fan';
    color: string;
    brightness: number;
    disabled: boolean;
    colorCount: number | null;
  } = $props();
</script>

<div class="lighting-preview">
  <svg viewBox="0 0 440 200" role="img" aria-label={t('Схема цвета выбранной зоны')}>
    {#if kind === 'pump'}
      <rect
        x="150"
        y="30"
        width="140"
        height="140"
        rx="24"
        fill="var(--surface-raised)"
        stroke="var(--border-strong)"
      />
      <circle
        cx="220"
        cy="100"
        r="52"
        fill="var(--canvas)"
        stroke={disabled || !colorCount ? 'var(--border-strong)' : color}
        stroke-width="5"
        opacity={disabled ? 1 : 0.25 + (Math.max(0, Math.min(100, brightness)) / 100) * 0.75}
      />
    {:else}
      {#each [0, 1, 2] as fan (fan)}
        <g transform={`translate(${fan * 140 + 10},35)`}>
          <rect
            width="130"
            height="130"
            rx="12"
            fill="var(--surface-raised)"
            stroke="var(--border-strong)"
          />
          <circle
            cx="65"
            cy="65"
            r="48"
            fill="var(--canvas)"
            stroke={disabled || !colorCount ? 'var(--border-strong)' : color}
            stroke-width="4"
            opacity={disabled ? 1 : 0.25 + (Math.max(0, Math.min(100, brightness)) / 100) * 0.75}
          />
          <circle cx="65" cy="65" r="13" fill="var(--surface)" stroke="var(--border-strong)" />
        </g>
      {/each}
    {/if}
  </svg>
  <p>
    {t(
      'Схема цвета, не симуляция эффекта. Фактическое число вентиляторов и расположение LED могут отличаться.',
    )}
  </p>
</div>

<style>
  .lighting-preview {
    background: var(--canvas);
    border-radius: 14px;
    padding: 24px;
    align-self: start;
    position: sticky;
    top: 24px;
  }
  .lighting-preview svg {
    width: 100%;
    max-height: 300px;
  }
  .lighting-preview p {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.7;
    max-width: 48ch;
    margin: 20px auto 0;
  }
  @media (max-width: 1100px) {
    .lighting-preview {
      position: static;
    }
  }
</style>
