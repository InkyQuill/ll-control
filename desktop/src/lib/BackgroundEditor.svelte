<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import { isProcedural } from './lcd-pages';
  import MediaThumbnail from './MediaThumbnail.svelte';
  import { hex, rgb, type Theme, type BackgroundMode, type GradientColors } from './api';
  let { theme = $bindable() }: { theme: Theme } = $props();
  const mode = $derived(
    theme.background_video
      ? 'video'
      : theme.background_image
        ? 'image'
        : (theme.background_fill?.mode ?? 'solid'),
  );
  const gradient = $derived(
    mode === 'solid' || mode === 'video' || mode === 'image' ? null : theme.background_fill?.[mode],
  );
  function select(value: string) {
    if (!['solid', 'horizontal', 'vertical', 'radial'].includes(value)) return;
    theme.version = theme.layers.some(isProcedural) ? 3 : 1;
    delete theme.background_video;
    delete theme.background_image;
    const defaults = (): GradientColors => ({
      colors: [
        [52, 230, 209],
        [26, 115, 105],
      ],
      flip: false,
    });
    theme.background_fill ??= {
      mode: 'solid',
      horizontal: defaults(),
      vertical: defaults(),
      radial: defaults(),
    };
    theme.background_fill.mode = value as BackgroundMode;
  }
</script>

<fieldset>
  <legend>{t('LCD background')}</legend>
  <label
    >{t('Background type')}<select value={mode} onchange={(e) => select(e.currentTarget.value)}>
      {#if theme.background_image}<option value="image">{t('Library image')}</option>{/if}
      {#if theme.background_video}<option value="video">{t('Library video')}</option>{/if}
      <option value="solid">{t('Solid color')}</option><option value="horizontal"
        >{t('Horizontal gradient')}</option
      >
      <option value="vertical">{t('Vertical gradient')}</option><option value="radial"
        >{t('Radial gradient')}</option
      >
    </select></label
  >
  {#if theme.background_video}
    <MediaThumbnail clipKey={theme.background_video} name={t('Background video')} />
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t(
          'Video plays on the canvas. Open the video library to replace it. Apply to update the LCD.',
        )}
      </p></StudioPopover
    >
  {:else if theme.background_image}
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t(
          'An image is selected as the theme background. Replace it in the image library using the Background destination.',
        )}
      </p></StudioPopover
    >
    <label
      >{t('Background placement')}<select bind:value={theme.background_image.fit}>
        <option value="crop">{t('Fill and crop')}</option><option value="contain">{t('Fit')}</option
        >
        <option value="contain_native">{t('Fit without upscaling')}</option><option value="stretch"
          >{t('Stretch')}</option
        >
      </select></label
    >
  {:else if gradient}
    <label
      >{t('First background color')}<input
        type="color"
        value={hex(gradient.colors[0])}
        oninput={(e) => {
          if (gradient) gradient.colors[0] = rgb(e.currentTarget.value);
        }}
      /></label
    >
    <label
      >{t('Second background color')}<input
        type="color"
        value={hex(gradient.colors[1])}
        oninput={(e) => {
          if (gradient) gradient.colors[1] = rgb(e.currentTarget.value);
        }}
      /></label
    >
    <label
      ><input
        type="checkbox"
        checked={gradient.flip}
        onchange={(e) => {
          if (gradient) gradient.flip = e.currentTarget.checked;
        }}
      />{t('Reverse gradient')}</label
    >
  {:else}
    <label
      >{t('Background')}<input
        type="color"
        value={hex(theme.background)}
        oninput={(e) => (theme.background = rgb(e.currentTarget.value))}
      /></label
    >
  {/if}
</fieldset>

<style>
  fieldset {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 10rem), 1fr));
    gap: 16px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    margin: 16px 0;
  }
  legend {
    padding: 0 6px;
  }
  label {
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  select {
    min-width: 0;
    width: 100%;
  }
  p {
    grid-column: 1/-1;
    margin: 0;
    line-height: 1.5;
  }
</style>
