<script lang="ts">
  import type { DisplayGeometry } from './device-capabilities';
  let { display = null }: { display?: DisplayGeometry | null } = $props();
</script>

<svg viewBox="0 0 160 120" fill="none" aria-hidden="true">
  <rect
    x="30"
    y="10"
    width="100"
    height="100"
    rx="18"
    fill="var(--surface-raised)"
    stroke="var(--border-strong)"
  />
  {#if display?.shape === 'square'}
    <rect
      data-device-screen="square"
      x="44"
      y="24"
      width="72"
      height="72"
      fill="var(--canvas)"
      stroke="var(--accent)"
      stroke-width="2"
    />
  {:else}
    <circle
      data-device-screen={display?.shape}
      cx="80"
      cy="60"
      r="36"
      fill="var(--canvas)"
      stroke="var(--accent)"
      stroke-width="2"
    />
  {/if}
  {#if display}<path d="M61 51h38M61 61h25M61 71h32" stroke="var(--accent)" stroke-width="3" />
  {:else}<circle cx="80" cy="60" r="10" stroke="var(--accent)" /><path
      d="M80 24v26m36 10H90M80 96V70M44 60h26"
      stroke="var(--border-strong)"
      stroke-width="6"
    />{/if}
</svg>

<style>
  svg {
    width: 100%;
    height: 100%;
    max-height: 190px;
  }
</style>
