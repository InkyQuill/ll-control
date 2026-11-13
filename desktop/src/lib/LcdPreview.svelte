<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { DisplayGeometry } from './device-capabilities';

  let { geometry, children }: { geometry: DisplayGeometry | null; children: Snippet } = $props();
</script>

<div
  class="lcd"
  data-lcd-shape={geometry?.shape ?? 'unspecified'}
  style:aspect-ratio={geometry ? `${geometry.width} / ${geometry.height}` : '1'}
>
  {@render children()}
</div>

<style>
  .lcd {
    width: 100%;
    max-width: 400px;
    justify-self: center;
    border-radius: 0;
    overflow: hidden;
    border: 8px solid var(--border);
    background: #090c14;
    display: grid;
    place-items: center;
  }
  .lcd[data-lcd-shape='round'] {
    border-radius: 50%;
  }
  .lcd :global(img) {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  @media (max-width: 1100px) {
    .lcd {
      max-width: 300px;
    }
  }
</style>
