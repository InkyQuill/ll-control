<script lang="ts">
  import {t} from './i18n.svelte';
  import type { Style } from './api';
  import type { FontChoice } from './font-upload';
  let { style, fonts, onchange }: { style: Style; fonts: FontChoice[]; onchange: (font: Style['font'], asset: string | undefined) => void } = $props();
  const bundled = [ ['sans','DejaVu Sans'], ['serif','DejaVu Serif'], ['mono','DejaVu Sans Mono'], ['noto_sans_tc_regular','Noto Sans TC Regular'], ['noto_sans_tc_bold','Noto Sans TC Bold'], ['abril_fatface','Abril Fatface'], ['caprasimo','Caprasimo'], ['cubic11','Cubic 11'] ] as const;
</script>
<label>{t("Шрифт")}<select value={style.font_asset ? `asset:${style.font_asset}` : style.font ?? 'sans'} onchange={e => {
  const value = e.currentTarget.value;
  if (value.startsWith('asset:')) onchange(style.font, value.slice(6));
  else onchange(value as Style['font'], undefined);
}}>
  {#each bundled as [value,name] (value)}<option {value}>{name}</option>{/each}
  {#each fonts as font (font.key)}<option value={`asset:${font.key}`}>{font.name}</option>{/each}
  {#if style.font_asset && !fonts.some(f => f.key === style.font_asset)}<option value={`asset:${style.font_asset}`}>{t("Пользовательский (")}{style.font_asset.slice(0,8)})</option>{/if}
</select></label>
