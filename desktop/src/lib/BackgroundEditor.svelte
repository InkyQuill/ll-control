<script lang="ts">
  import {t} from './i18n.svelte';
  import {isProcedural} from './lcd-pages';
  import MediaThumbnail from './MediaThumbnail.svelte';
  import { hex, rgb, type Theme, type BackgroundMode, type GradientColors } from './api';
  let { theme = $bindable() }: { theme: Theme } = $props();
  const mode = $derived(theme.background_video ? 'video' : theme.background_image ? 'image' : theme.background_fill?.mode ?? 'solid');
  const gradient = $derived(mode === 'solid' || mode === 'video' || mode === 'image' ? null : theme.background_fill?.[mode]);
  function select(value: string) {
    if (!['solid', 'horizontal', 'vertical', 'radial'].includes(value)) return;
    theme.version = theme.layers.some(isProcedural)?3:1;
    delete theme.background_video;
    delete theme.background_image;
    const defaults = (): GradientColors => ({ colors: [[52,230,209], [26,115,105]], flip: false });
    theme.background_fill ??= { mode: 'solid', horizontal: defaults(), vertical: defaults(), radial: defaults() };
    theme.background_fill.mode = value as BackgroundMode;
  }
</script>
<fieldset>
  <legend>{t("Фон LCD")}</legend>
  <label>{t("Тип фона")}<select value={mode} onchange={e => select(e.currentTarget.value)}>
    {#if theme.background_image}<option value="image">{t("Изображение из библиотеки")}</option>{/if}
    {#if theme.background_video}<option value="video">{t("Видео из библиотеки")}</option>{/if}
    <option value="solid">{t("Сплошной цвет")}</option><option value="horizontal">{t("Горизонтальный градиент")}</option>
    <option value="vertical">{t("Вертикальный градиент")}</option><option value="radial">{t("Радиальный градиент")}</option>
  </select></label>
  {#if theme.background_video}
    <MediaThumbnail clipKey={theme.background_video} name={t('Видео фона')}/>
    <p>{t("Видео выбрано фоном темы. Для замены используйте библиотеку роликов. Preview показывает первый кадр с текущими показаниями; движение начнётся после применения к LCD.")}</p>
  {:else if theme.background_image}
    <p>{t("Изображение выбрано фоном темы. Замените его в библиотеке изображений, выбрав назначение «Фон».")}</p>
    <label>{t("Размещение фона")}<select bind:value={theme.background_image.fit}>
      <option value="crop">{t("Заполнить с обрезкой")}</option><option value="contain">{t("Вписать")}</option>
      <option value="contain_native">{t("Вписать без увеличения")}</option><option value="stretch">{t("Растянуть")}</option>
    </select></label>
  {:else if gradient}
    <label>{t("Первый цвет фона")}<input type="color" value={hex(gradient.colors[0])} oninput={e => { if (gradient) gradient.colors[0] = rgb(e.currentTarget.value); }}/></label>
    <label>{t("Второй цвет фона")}<input type="color" value={hex(gradient.colors[1])} oninput={e => { if (gradient) gradient.colors[1] = rgb(e.currentTarget.value); }}/></label>
    <label><input type="checkbox" checked={gradient.flip} onchange={e => { if (gradient) gradient.flip = e.currentTarget.checked; }}/>{t("Обратить градиент")}</label>
  {:else}
    <label>{t("Фон")}<input type="color" value={hex(theme.background)} oninput={e => theme.background = rgb(e.currentTarget.value)}/></label>
  {/if}
</fieldset>
<style>
  fieldset{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,10rem),1fr));gap:16px;border:1px solid var(--border);border-radius:8px;padding:16px;margin:16px 0}
  legend{padding:0 6px}label{display:grid;gap:8px;min-width:0}select{min-width:0;width:100%}p{grid-column:1/-1;margin:0;line-height:1.5}
</style>
