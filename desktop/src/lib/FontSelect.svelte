<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import StudioPopover from './StudioPopover.svelte';
  import {
    systemFonts,
    systemFontFile,
    importedSystemFonts,
    type SystemFont,
  } from './system-fonts';
  import { uploadFont } from './font-upload';
  import { message } from './api';
  import { t } from './i18n.svelte';
  import type { Style } from './api';
  import type { FontChoice } from './font-upload';
  let {
    style,
    fonts,
    onchange,
  }: {
    style: Style;
    fonts: FontChoice[];
    onchange: (font: Style['font'], asset: string | undefined) => void;
  } = $props();
  let installed = $state<SystemFont[]>([]),
    importing = $state(false),
    failure = $state('');
  let request: AbortController | undefined;
  onMount(() => {
    let alive = true;
    void systemFonts()
      .then((fonts) => {
        if (alive) installed = fonts;
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  });
  onDestroy(() => request?.abort());
  async function select(value: string) {
    if (importing) return;
    failure = '';
    if (!value.startsWith('system:')) {
      if (value.startsWith('asset:')) onchange(style.font, value.slice(6));
      else onchange(value as Style['font'], undefined);
      return;
    }
    const font = installed.find((font) => font.id === value.slice(7));
    if (!font) return;
    const owner = style,
      controller = new AbortController();
    request = controller;
    importing = true;
    try {
      const file = await systemFontFile(font);
      controller.signal.throwIfAborted();
      const asset = await uploadFont(file, controller.signal, () => {});
      if (owner === style && !controller.signal.aborted) {
        importedSystemFonts.set(asset, font.name);
        onchange(style.font, asset);
      }
    } catch (error) {
      if (!controller.signal.aborted) failure = message(error);
    } finally {
      importing = false;
      request = undefined;
    }
  }
  const id = $props.id();
  const bundled = [
    ['sans', 'DejaVu Sans'],
    ['serif', 'DejaVu Serif'],
    ['mono', 'DejaVu Sans Mono'],
    ['noto_sans_tc_regular', 'Noto Sans TC Regular'],
    ['noto_sans_tc_bold', 'Noto Sans TC Bold'],
    ['abril_fatface', 'Abril Fatface'],
    ['caprasimo', 'Caprasimo'],
    ['cubic11', 'Cubic 11'],
  ] as const;
</script>

<div class="font-heading">
  <label for={id}>{t('Шрифт')}</label><StudioPopover label={t('Справка о шрифтах')} icon="help"
    ><p>
      {t(
        'Системный шрифт копируется в библиотеку темы. Поддерживаются TTF и OTF до 16 MiB; коллекции TTC недоступны.',
      )}
    </p></StudioPopover
  >
</div>
<select
  {id}
  value={style.font_asset ? `asset:${style.font_asset}` : (style.font ?? 'sans')}
  disabled={importing}
  onchange={async (event) => {
    const input = event.currentTarget;
    await select(input.value);
    input.value = style.font_asset ? `asset:${style.font_asset}` : (style.font ?? 'sans');
  }}
>
  {#each bundled as [value, name] (value)}<option {value}>{name}</option>{/each}
  {#each fonts as font (font.key)}<option value={`asset:${font.key}`}
      >{importedSystemFonts.get(font.key) ?? font.name}</option
    >{/each}
  {#if style.font_asset && !fonts.some((f) => f.key === style.font_asset)}<option
      value={`asset:${style.font_asset}`}
      >{importedSystemFonts.get(style.font_asset) ??
        `${t('Пользовательский (')}${style.font_asset.slice(0, 8)})`}</option
    >{/if}
  {#if installed.length}<optgroup label={t('Системные шрифты')}
      >{#each installed as font (font.id)}<option value={`system:${font.id}`}>{font.name}</option
        >{/each}</optgroup
    >{/if}
</select>

{#if importing}<span role="status">{t('Загрузка шрифта')}</span>{/if}
{#if failure}<p role="alert">{t('Не удалось загрузить шрифт:')} {failure}</p>{/if}

<style>
  .font-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 8px;
  }
  .font-heading label {
    margin: 0;
  }
</style>
