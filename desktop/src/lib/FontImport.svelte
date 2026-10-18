<script lang="ts">
  import StudioPopover from "./StudioPopover.svelte";
  import {t} from './i18n.svelte';
  import {ValidationError} from './validation-error';
  import { onDestroy, tick } from 'svelte';
  import fontLicenses from './assets/font-licenses.txt?raw';
  import { message } from './api';
  import { uploadFont, type FontChoice } from './font-upload';
  let { connected, onimport }: { connected: boolean; onimport: (font: FontChoice) => void } = $props();
  let uploading = $state(false), received = $state(0), size = $state(0), error = $state<ValidationError|string|null>(null), status = $state<'uploaded'|'cancelled'|''>('');
  let controller: AbortController | undefined;
  let alive = true;
  let fileInput = $state<HTMLInputElement>();
  let cancelButton = $state<HTMLButtonElement>();
  $effect(() => { if (!connected) controller?.abort(); });
  onDestroy(() => { alive = false; controller?.abort(); });
  async function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const restorePickerFocus = document.activeElement === input;
    const file = input.files?.[0]; input.value = '';
    if (!file || uploading || !connected) return;
    const request = new AbortController(); controller = request;
    uploading = true; received = 0; size = file.size; error = null; status = '';
    try {
      const key = await uploadFont(file, request.signal, value => { if (alive) received = value; });
      if (!alive || request.signal.aborted) return;
      onimport({ key, name: file.name });
      status = 'uploaded';
    } catch (e) {
      if (alive) {
        if (request.signal.aborted) status = 'cancelled';
        else error = e instanceof ValidationError?e:message(e);
      }
    } finally {
      const restoreFocus = alive && document.activeElement === cancelButton;
      if (alive) uploading = false;
      controller = undefined;
      await tick();
      // Restore only focus lost with our removed control, never a user's new target.
      if ((restoreFocus || restorePickerFocus) && alive && connected && !uploading
          && document.hasFocus() && fileInput?.isConnected
          && document.activeElement === document.body) fileInput.focus();
    }
  }
</script>
<StudioPopover label={`${t("Пользовательские шрифты")}`} text>
  <StudioPopover label={t("Справка")} icon="help"><p>{t("TTF/OTF, до 16 MiB. Загрузка не меняет тему и экран. Коллекции TTC и веб-шрифты не поддерживаются.")}</p></StudioPopover>
  <StudioPopover label={`${t("Лицензии встроенных шрифтов")}`} text><pre>{fontLicenses}</pre></StudioPopover>
  <label>{t("Загрузить шрифт")}<input bind:this={fileInput} type="file" accept=".ttf,.otf" disabled={!connected || uploading} onchange={choose}/></label>
  {#if uploading}<progress max={size} value={received} aria-label="{t("Загрузка шрифта")}"></progress><button bind:this={cancelButton} onclick={() => controller?.abort()}>{t("Отменить загрузку шрифта")}</button>{/if}
  {#if status}<p role="status">{status==='uploaded'?t('Шрифт загружен. Выберите его в настройках слоя, затем примените тему.'):t('Загрузка отменена. Тема не изменена.')}</p>{/if}
  {#if error}<p role="alert">{#if error instanceof ValidationError}{t(error.resource,...error.values)}{:else}{t('Не удалось загрузить шрифт:')} <span>{error}</span>{/if}</p>{/if}
</StudioPopover>
<style>pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px}p{color:var(--text-muted);line-height:1.6}label{display:grid;gap:8px}progress{width:100%;margin:10px 0}[role=alert]{color:var(--danger)}</style>
