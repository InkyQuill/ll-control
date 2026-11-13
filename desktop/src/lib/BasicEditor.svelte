<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import type { BasicBackground, BasicOverlay, Theme } from './api';
  import { basicOverlays, createBasicEditor, selectBasicEditor } from './basic-editor';
  import { lcdPageKind } from './lcd-pages';
  import { t } from './i18n.svelte';
  import { ValidationError } from './validation-error';
  let {
    theme = $bindable(),
    disabled = false,
    onchange = () => {},
  }: { theme: Theme; disabled?: boolean; onchange?: () => void } = $props();
  const editor = $derived(lcdPageKind(theme) === 'basic' ? theme.basic_editor : null);
  const selected = $derived(
    editor?.banks.find((bank) => bank.background === editor.selected_background),
  );
  const banks = $derived(
    (['image_video', 'background_color'] as const).filter((kind) =>
      editor?.banks.some((bank) => bank.background === kind),
    ),
  );
  const availableOverlays = $derived(
    basicOverlays.filter((kind) => selected?.overlays.some((item) => item.overlay === kind)),
  );
  let failure = $state.raw<{ owner: Theme; error: ValidationError } | null>(null);
  const error = $derived(failure?.owner === theme ? failure.error : null);
  const backgrounds = $derived({
    image_video: t('Image or video'),
    background_color: t('Color and gradients'),
  });
  const overlays = $derived({
    none: t('No text'),
    slider_sensor: t('Single sensor'),
    duo_sensor: t('Two sensors'),
    three_sensor: t('Three sensors'),
    four_sensor: t('Four sensors'),
    time: t('Clock'),
    custom: t('Custom text'),
  });
  function create() {
    if (disabled) return;
    try {
      theme = createBasicEditor($state.snapshot(theme));
      failure = null;
      onchange();
    } catch (error) {
      failure = {
        owner: theme,
        error:
          error instanceof ValidationError
            ? error
            : new ValidationError('Could not create variants for this theme.'),
      };
    }
  }
  function select(background: BasicBackground, overlay: BasicOverlay) {
    if (disabled) return false;
    try {
      theme = selectBasicEditor($state.snapshot(theme), background, overlay);
      failure = null;
      onchange();
      return true;
    } catch (error) {
      failure = {
        owner: theme,
        error:
          error instanceof ValidationError
            ? error
            : new ValidationError(
                'Could not switch presets. Your draft is preserved; reload the theme.',
              ),
      };
      return false;
    }
  }
</script>

{#if !editor && lcdPageKind(theme) === 'basic'}
  <fieldset class="basic-presets" {disabled}>
    <legend>{t('Background and text presets')}</legend>
    <p>{t('Create independent variants from the current draft and edit each one separately.')}</p>
    <button type="button" onclick={create}>{t('Create variants')}</button>
    {#if error}<p role="alert">{t(error.resource, ...error.values)}</p>{/if}
  </fieldset>
{:else if editor && selected}
  <fieldset class="basic-presets" {disabled}>
    <legend>{t('Background and text presets')}</legend>
    <label
      >{t('Background preset')}
      <select
        value={editor.selected_background}
        onchange={(event) => {
          const bank = editor.banks.find((bank) => bank.background === event.currentTarget.value);
          if (bank && !select(bank.background, bank.selected_overlay))
            event.currentTarget.value = editor.selected_background;
        }}
      >
        {#each banks as background (background)}<option value={background}
            >{backgrounds[background]}</option
          >{/each}
      </select>
    </label>
    <label
      >{t('Text preset')}
      <select
        value={selected.selected_overlay}
        onchange={(event) => {
          const item = selected.overlays.find((item) => item.overlay === event.currentTarget.value);
          if (item && !select(selected.background, item.overlay))
            event.currentTarget.value = selected.selected_overlay;
        }}
      >
        {#each availableOverlays as overlay (overlay)}<option value={overlay}
            >{overlays[overlay]}</option
          >{/each}
      </select>
    </label>
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t('Each preset keeps its edits. Changes stay in the draft until you apply them.')}
      </p></StudioPopover
    >
    {#if error}<p role="alert">{t(error.resource, ...error.values)}</p>{/if}
  </fieldset>
{/if}

<style>
  .basic-presets {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 10rem), 1fr));
    gap: 12px 16px;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    margin: 16px 0;
    min-width: 0;
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
    width: 100%;
    min-width: 0;
  }
  p {
    grid-column: 1/-1;
    margin: 0;
    color: var(--text-muted);
    line-height: 1.5;
  }
  p[role='alert'] {
    color: var(--danger);
  }
</style>
