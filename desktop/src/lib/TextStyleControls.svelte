<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import type { Style } from './api';
  import FontSelect from './FontSelect.svelte';
  let { style = $bindable(), fonts }: { style: Style; fonts: { key: string; name: string }[] } =
    $props();
  const hex = (color: number[]) =>
    '#' + color.map((value) => value.toString(16).padStart(2, '0')).join('');
  const rgb = (color: string) =>
    [1, 3, 5].map((index) => parseInt(color.slice(index, index + 2), 16));
</script>

<div class="row placement">
  <label>X<input type="number" min="0" max="479" bind:value={style.x} /></label><label
    >Y<input type="number" min="0" max="479" bind:value={style.y} /></label
  ><label>{t('Size')}<input type="number" min="8" max="160" bind:value={style.size} /></label><label
    >{t('Color')}<input
      type="color"
      value={hex(style.color)}
      oninput={(e) => {
        style.color = rgb(e.currentTarget.value);
      }}
    /></label
  >
</div>
<FontSelect
  {style}
  {fonts}
  onchange={(font, asset) => {
    style.font = font;
    style.font_asset = asset;
  }}
/>
<label
  >{t('Text opacity, %')}<input
    type="number"
    min="0"
    max="100"
    value={style.opacity ?? 100}
    oninput={(e) => (style.opacity = Number(e.currentTarget.value))}
  /></label
>
<label
  >{t('Alignment')}<select bind:value={style.align}
    ><option value="left">{t('Left')}</option><option value="center">{t('Centered')}</option><option
      value="right">{t('Right')}</option
    ></select
  ></label
>

<StudioPopover label={`${t('Advanced text settings')}`}>
  <label
    >{t('Vertical anchor')}<select
      aria-label={t('Vertical anchor')}
      value={style.vertical_align ?? 'top'}
      onchange={(e) =>
        (style.vertical_align = e.currentTarget.value as typeof style.vertical_align)}
      ><option value="top">{t('Top')}</option><option value="center">{t('Center')}</option><option
        value="bottom">{t('Bottom')}</option
      ></select
    ></label
  >
  <label
    ><input
      type="checkbox"
      checked={style.max_width != null}
      onchange={(e) => (style.max_width = e.currentTarget.checked ? 358 : null)}
    />{t('Wrap text to width')}</label
  >
  {#if style.max_width != null}<label
      >{t('Text width, px')}<input
        type="number"
        min="1"
        max="480"
        bind:value={style.max_width}
      /></label
    >{/if}
  <label
    ><input
      type="checkbox"
      checked={style.line_box ?? false}
      onchange={(e) => (style.line_box = e.currentTarget.checked)}
    />{t('Line box height (CSS)')}</label
  >
  <label
    ><input
      type="checkbox"
      checked={style.line_height != null}
      onchange={(e) => (style.line_height = e.currentTarget.checked ? 58 : null)}
    />{t('Set line spacing')}</label
  >
  {#if style.line_height != null}<label
      >{t('Line spacing, px')}<input
        type="number"
        min="8"
        max="320"
        bind:value={style.line_height}
      /></label
    >{/if}
  <label
    ><input
      type="checkbox"
      checked={!!style.shadow}
      onchange={(e) =>
        (style.shadow = e.currentTarget.checked
          ? { x: 2, y: 2, blur: 2, color: [0, 0, 0], opacity: 60 }
          : null)}
    />{t('Text shadow')}</label
  >
  {#if style.shadow}
    <div class="row">
      <label
        >{t('Shadow offset X')}<input
          type="number"
          min="-32"
          max="32"
          bind:value={style.shadow.x}
        /></label
      >
      <label
        >{t('Shadow offset Y')}<input
          type="number"
          min="-32"
          max="32"
          bind:value={style.shadow.y}
        /></label
      >
      <label
        >{t('Shadow blur')}<input
          type="number"
          min="0"
          max="16"
          bind:value={style.shadow.blur}
        /></label
      >
      <label
        >{t('Shadow opacity, %')}<input
          type="number"
          min="0"
          max="100"
          bind:value={style.shadow.opacity}
        /></label
      >
      <label
        >{t('Shadow color')}<input
          type="color"
          value={hex(style.shadow.color)}
          oninput={(e) => {
            if (style.shadow) style.shadow.color = rgb(e.currentTarget.value);
          }}
        /></label
      >
    </div>
  {/if}
</StudioPopover>

<style>
  .row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    align-items: end;
    margin-bottom: 8px;
  }
  .placement {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
  .row > label {
    min-width: 0;
  }
</style>
