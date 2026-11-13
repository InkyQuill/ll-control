<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { ValidationError } from './validation-error';
  import { t } from './i18n.svelte';
  import { tick } from 'svelte';
  import ProfileArchive from './ProfileArchive.svelte';
  import {
    rpc,
    message,
    profileGroup,
    type ProfileContent,
    type ProfileGroup,
    type ProfileLibrary,
    type ProfileEdit,
  } from './api';
  let {
    refreshEpoch = 0,
    group,
    connected,
    disabled = false,
    resetSensor = '',
    mediaKey = '',
    draft,
    onload,
  }: {
    refreshEpoch?: number;
    group: ProfileGroup;
    connected: boolean;
    disabled?: boolean;
    resetSensor?: string;
    mediaKey?: string;
    draft: () => ProfileContent;
    onload: (content: ProfileContent) => void;
  } = $props();
  let library = $state<ProfileLibrary | null>(null),
    slot = $state(0),
    name = $state('');
  let needsRefresh = $state(false);
  let confirmGroupReset = $state(false);
  let refreshButton: HTMLButtonElement | undefined = $state();
  let groupResetButton: HTMLButtonElement | undefined = $state();
  $effect(() => {
    void group;
    void connected;
    void resetSensor;
    confirmGroupReset = false;
  });
  let archiveBusy = $state(false);
  type Failure = { operation: 'refresh' | 'edit'; detail: string };
  let busy = $state(false),
    error = $state<Failure | ValidationError | null>(null),
    notice = $state('');
  function errorText(failure: Failure | ValidationError): string {
    if (failure instanceof ValidationError) return t(failure.resource, ...failure.values);
    return failure.operation === 'refresh'
      ? t('Could not load the profile list: {0}', failure.detail)
      : t(
          'Profile changes were not confirmed: {0}. Check the library before writing again.',
          failure.detail,
        );
  }
  let selected = $derived(
    library?.profiles.find((p) => p.slot === slot && profileGroup(p.content) === group),
  );
  let unavailable = $derived(busy || archiveBusy || disabled || !connected || needsRefresh);
  let generation = 0;
  $effect(() => {
    void refreshEpoch;
    void group;
    void connected;
    generation++;
    library = null;
    needsRefresh = false;
    error = null;
    notice = '';
    return () => {
      generation++;
    };
  });
  async function refresh() {
    const started = generation;
    busy = true;
    error = null;
    notice = '';
    confirmGroupReset = false;
    try {
      const result = await rpc<ProfileLibrary>('list_profiles');
      if (started === generation) {
        library = result;
        needsRefresh = false;
      }
    } catch (e) {
      if (started === generation) {
        error = { operation: 'refresh', detail: message(e) };
        library = null;
      }
    } finally {
      busy = false;
    }
  }
  async function edit(
    action: 'save' | 'save_media' | 'rename' | 'delete' | 'reset' | 'reset_group',
  ) {
    if (!library || unavailable) return;
    const started = generation;
    busy = true;
    error = null;
    notice = '';
    try {
      let change: ProfileEdit;
      if (action === 'reset_group')
        change = { action, group, sensor_id: group === 'screen' ? null : resetSensor };
      else if (action === 'reset')
        change = { action, group, slot, sensor_id: group === 'screen' ? null : resetSensor };
      else if (action === 'save' || action === 'save_media') {
        const content: ProfileContent =
          action === 'save_media' ? { kind: 'screen_media', value: mediaKey } : draft();
        if (profileGroup(content) !== group)
          throw new ValidationError('The profile group changed; save again');
        change = { action: 'save', profile: { slot, name, content } };
      } else if (action === 'rename') change = { action, group, slot, name };
      else change = { action, group, slot };
      const result = await rpc<ProfileLibrary>('edit_profile', {
        expected_revision: library.revision,
        edit: change,
      });
      if (started !== generation) return;
      library = result;
      notice = 'Library saved. Device settings were not changed.';
    } catch (e) {
      if (started === generation) {
        error = e instanceof ValidationError ? e : { operation: 'edit', detail: message(e) };
        // A failed reply does not prove the mutation failed. Require readback.
        needsRefresh = true;
      }
    } finally {
      busy = false;
    }
    if (action === 'reset_group' && started === generation) {
      confirmGroupReset = false;
      await tick();
      if (started !== generation) return;
      const target = needsRefresh ? refreshButton : groupResetButton;
      if (target?.isConnected) target.focus();
    }
  }
</script>

<StudioPopover label={`${t('Saved profiles')}`} text wide>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t('Load a profile into the editor, then apply it with the regular Apply button.')}
    </p></StudioPopover
  >
  <button bind:this={refreshButton} disabled={busy || disabled || !connected} onclick={refresh}
    >{t('Refresh profile list')}</button
  >
  {#if library}
    <label
      >{t('Profile slot')}<select
        bind:value={slot}
        disabled={unavailable}
        onchange={(event) => {
          const next = Number(event.currentTarget.value);
          name =
            library?.profiles.find((p) => p.slot === next && profileGroup(p.content) === group)
              ?.name ?? '';
          notice = '';
        }}
      >
        {#each Array.from({ length: group === 'screen' ? 8 : 4 }, (_, i) => i) as index (index)}
          <option value={index}
            >{index + 1} · {library.profiles.find(
              (p) => p.slot === index && profileGroup(p.content) === group,
            )?.name ?? t('Empty')}</option
          >
        {/each}
      </select></label
    >
    <label
      >{t('Profile name')}<input bind:value={name} maxlength="128" disabled={unavailable} /></label
    >
    <div class="buttons">
      <button
        disabled={unavailable || !selected}
        onclick={() => {
          if (selected) {
            onload(structuredClone($state.snapshot(selected.content)));
            name = selected.name;
            notice = 'Profile loaded into the editor. Applying is a separate action.';
          }
        }}>{t('Load profile into editor')}</button
      >
      <button disabled={unavailable || !name.trim()} onclick={() => edit('save')}
        >{selected
          ? '' + t('Replace profile with draft') + ''
          : '' + t('Save draft to slot') + ''}</button
      >
      {#if group === 'screen' && mediaKey}<button
          disabled={unavailable || !name.trim()}
          onclick={() => edit('save_media')}
          >{selected
            ? '' + t('Replace profile with selected video') + ''
            : '' + t('Save selected video to slot') + ''}</button
        >{/if}
      <button disabled={unavailable || !selected || !name.trim()} onclick={() => edit('rename')}
        >{t('Rename profile')}</button
      >
      <button disabled={unavailable || !selected} onclick={() => edit('delete')}
        >{t('Delete profile')}</button
      >
      <button
        disabled={unavailable || (group !== 'screen' && !resetSensor)}
        onclick={() => edit('reset')}>{t('Reset slot to defaults')}</button
      >
      <button
        bind:this={groupResetButton}
        disabled={unavailable || (group !== 'screen' && !resetSensor)}
        onclick={() => {
          confirmGroupReset = true;
        }}>{t('Reset all group slots…')}</button
      >
    </div>
    {#if confirmGroupReset}
      <div role="group" aria-label={t('Confirm group reset')}>
        <p>
          {t('Names and contents will be replaced for all')}
          {group === 'screen' ? 8 : 4}
          {t('slots of the')}
          {group === 'screen'
            ? '' + t('screen') + ''
            : group === 'fan'
              ? '' + t('fans') + ''
              : '' + t('pump') + ''}. {group === 'screen'
            ? '' + t('Each slot will contain a basic clock theme.') + ''
            : t('Each slot will contain a Quiet curve using sensor {0}.', resetSensor)}
          {t('Active device settings, other groups and library files are preserved.')}
        </p>
        <button
          disabled={unavailable || (group !== 'screen' && !resetSensor)}
          onclick={() => edit('reset_group')}>{t('Confirm reset of all slots')}</button
        >
        <button
          disabled={busy}
          onclick={() => {
            confirmGroupReset = false;
            groupResetButton?.focus();
          }}>{t('Cancel group reset')}</button
        >
      </div>
    {/if}
  {/if}
  {#if library}<p>
      {group === 'screen'
        ? '' + t('Reset replaces the slot name and content with a basic clock theme.') + ''
        : '' +
          t('Reset replaces the slot with a Quiet curve using the sensor selected in the editor.') +
          ''}
      {t('Device settings change only after loading and applying the profile.')}
    </p>{/if}
  {#if library}<ProfileArchive
      {group}
      {slot}
      revision={library.revision}
      name={selected?.name ?? name}
      hasProfile={!!selected}
      {connected}
      disabled={busy || disabled || needsRefresh}
      bind:busy={archiveBusy}
      onrequireRefresh={() => (needsRefresh = true)}
      onlibrary={(result, target) => {
        if (result.revision < (library?.revision ?? 0)) return;
        library = result;
        needsRefresh = false;
        if (target.group === group && target.slot === slot)
          name =
            result.profiles.find((p) => p.slot === slot && profileGroup(p.content) === group)
              ?.name ?? name;
      }}
    />{/if}
  {#if error}<p role="alert">{errorText(error)} {t('Refresh the list before retrying.')}</p>{/if}
  {#if notice}<p role="status">{t(notice)}</p>{/if}
</StudioPopover>

<style>
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
