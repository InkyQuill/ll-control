<script lang="ts">
  import { hex, rgb, type Layer } from './api';
  import { t } from './i18n.svelte';
  let { layer = $bindable() }: { layer: Extract<Layer, { kind: 'divider' }> } = $props();
</script>

<div class="divider-controls">
  <label
    >{t('Orientation')}<select bind:value={layer.axis}
      ><option value="horizontal">{t('Horizontal')}</option><option value="vertical"
        >{t('Vertical')}</option
      ></select
    ></label
  >
  <div class="row">
    <label>X<input type="number" min="0" max="480" step="0.5" bind:value={layer.x} /></label>
    <label>Y<input type="number" min="0" max="480" step="0.5" bind:value={layer.y} /></label>
    <label
      >{t('Length, px')}<input
        type="number"
        min="0.25"
        max="480"
        step="0.5"
        bind:value={layer.length}
      /></label
    >
    <label
      >{t('Thickness, px')}<input
        type="number"
        min="0.25"
        max="16"
        step="0.25"
        bind:value={layer.thickness}
      /></label
    >
    <label
      >{t('Color')}<input
        type="color"
        value={hex(layer.color)}
        oninput={(e) => (layer.color = rgb(e.currentTarget.value))}
      /></label
    >
  </div>
  <label
    >{t('Divider opacity, %')}<input
      type="number"
      min="0"
      max="100"
      bind:value={layer.opacity}
    /></label
  >
  <label
    ><input
      type="checkbox"
      checked={!!layer.shadow}
      onchange={(e) =>
        (layer.shadow = e.currentTarget.checked
          ? { x: 0, y: 0, blur: 5, color: [0, 0, 0], opacity: 100 }
          : null)}
    />{t('Divider shadow')}</label
  >
  {#if layer.shadow}<div class="row">
      <label
        >{t('Shadow offset X')}<input
          type="number"
          min="-32"
          max="32"
          bind:value={layer.shadow.x}
        /></label
      >
      <label
        >{t('Shadow offset Y')}<input
          type="number"
          min="-32"
          max="32"
          bind:value={layer.shadow.y}
        /></label
      >
      <label
        >{t('Shadow blur')}<input
          type="number"
          min="0"
          max="16"
          bind:value={layer.shadow.blur}
        /></label
      >
      <label
        >{t('Shadow opacity, %')}<input
          type="number"
          min="0"
          max="100"
          bind:value={layer.shadow.opacity}
        /></label
      >
      <label
        >{t('Shadow color')}<input
          type="color"
          value={hex(layer.shadow.color)}
          oninput={(e) => {
            if (layer.shadow) layer.shadow.color = rgb(e.currentTarget.value);
          }}
        /></label
      >
    </div>{/if}
</div>

<style>
  .divider-controls {
    display: grid;
    gap: 14px;
  }
  .row {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .row label {
    flex: 1;
    min-width: 92px;
  }
</style>
