<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import {t} from './i18n.svelte';
  let {feature,message,label='Экспериментально'}:{feature:string;message:string;label?:string}=$props();
</script>
<span class="experimental-notice" data-status="experimental" data-feature={feature}>
  <span class="experimental-badge">{t(label)}</span>
  <StudioPopover label={t('Ограничения экспериментальной функции')} icon="help"><p>{t(message)}</p></StudioPopover>
</span>
<style>
  .experimental-notice{display:inline-flex;align-items:center;gap:4px;width:max-content;max-width:100%}.experimental-badge{padding:3px 8px;border-radius:999px;background:var(--danger-surface);color:var(--danger);font-size:11px;font-weight:600;letter-spacing:.02em}
</style>
