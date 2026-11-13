<script lang="ts">
  import { t } from './i18n.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { message } from './api';
  import { ValidationError } from './validation-error';
  interface Preferences {
    closeToTray: boolean;
    startMinimized: boolean;
    autostart: boolean;
  }
  interface Snapshot {
    revision: number;
    preferences: Preferences;
    registration: {
      present: boolean;
      enabled: boolean | null;
      managed: boolean;
      problem: string | null;
    };
    integrationError: string | null;
    trayReady: boolean;
    trayError: string | null;
    trayErrorCode?: string | null;
  }
  let open = $state(false),
    busy = $state(false),
    snapshot = $state<Snapshot | null>(null),
    draft = $state<Preferences | null>(null),
    base = $state<number | null>(null),
    error = $state<ValidationError | string | null>(null),
    readError = $state<ValidationError | string | null>(null),
    notice = $state('');
  let epoch = 0;
  const stale = $derived(snapshot !== null && base !== snapshot.revision);
  const registrationPending = $derived(
    !!snapshot && snapshot.registration.enabled !== snapshot.preferences.autostart,
  );
  function valid(value: Snapshot | null): value is Snapshot {
    return (
      !!value && typeof value.revision === 'number' && !!value.preferences && !!value.registration
    );
  }
  async function read(reload = false) {
    const generation = epoch;
    try {
      const value = await invoke<Snapshot | null>('desktop_settings_get');
      if (generation !== epoch || !open) return;
      if (!valid(value)) throw new ValidationError('Application settings are unavailable.');
      snapshot = value;
      if (reload || draft === null) {
        draft = { ...value.preferences };
        base = value.revision;
      }
      readError = null;
      if (reload) error = null;
    } catch (e) {
      if (generation === epoch && open) readError = e instanceof ValidationError ? e : message(e);
    }
  }
  $effect(() => {
    if (!open) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      if (!busy) await read();
      if (!stopped) timer = setTimeout(poll, 2000);
    }
    timer = setTimeout(poll, 0);
    return () => {
      stopped = true;
      clearTimeout(timer);
      epoch++;
    };
  });
  async function save() {
    if (busy || !draft || base === null || stale) return;
    epoch++;
    busy = true;
    error = null;
    notice = '';
    try {
      const value = await invoke<Snapshot>('desktop_settings_save', {
        expectedRevision: base,
        preferences: $state.snapshot(draft),
      });
      if (!valid(value))
        throw new ValidationError(
          'The save response could not be confirmed. Reload the saved state.',
        );
      snapshot = value;
      draft = { ...value.preferences };
      base = value.revision;
      notice = value.integrationError
        ? 'Preferences saved. Autostart registration needs another check.'
        : 'Application settings saved.';
    } catch (e) {
      error = e instanceof ValidationError ? e : message(e);
    } finally {
      busy = false;
    }
  }
  async function exit() {
    if (busy) return;
    busy = true;
    error = null;
    try {
      await invoke('desktop_exit');
      notice = 'Finishing active operations before exit…';
    } catch (e) {
      error = e instanceof ValidationError ? e : message(e);
      busy = false;
    }
  }
</script>

<details ontoggle={(event) => (open = event.currentTarget.open)}>
  <summary>{t('Application and autostart')}</summary>
  <p>
    {t(
      'These preferences control the application window. The daemon keeps controlling cooling and the LCD after the GUI exits.',
    )}
  </p>
  {#if snapshot && draft}
    <label
      ><input type="checkbox" bind:checked={draft.closeToTray} disabled={busy} />{t(
        'Minimize to tray when closing the window',
      )}</label
    >
    <label
      ><input type="checkbox" bind:checked={draft.startMinimized} disabled={busy} />{t(
        'Start minimized',
      )}</label
    >
    <label
      ><input type="checkbox" bind:checked={draft.autostart} disabled={busy} />{t(
        'Start the application at login',
      )}</label
    >
    <p role="status">
      Tray: {snapshot.trayReady ? '' + t('available') + '' : '' + t('unavailable') + ''}. {snapshot.trayReady
        ? '' + t('Show and Exit are available in the tray menu.') + ''
        : '' +
          t(
            'The window will remain accessible. Closing it will finish active operations and exit the GUI.',
          ) +
          ''}
    </p>
    <p role="status">
      {t('Autostart registration:')}
      {snapshot.registration.enabled === null
        ? '' + t('state unconfirmed') + ''
        : snapshot.registration.enabled
          ? '' + t('enabled') + ''
          : '' + t('off') + ''}.
    </p>
    {#if snapshot.trayErrorCode === 'host_unconfirmed'}
      <p>{t('Tray host registration for this application is unconfirmed.')}</p>
    {:else}
      {#if snapshot.trayErrorCode === 'unavailable'}<p>
          {t('Could not set up the tray icon.')}
        </p>{/if}
      {#if snapshot.trayError}<p>{snapshot.trayError}</p>{/if}
    {/if}
    {#if snapshot.registration.problem}<p role="alert">{snapshot.registration.problem}</p>{/if}
    {#if snapshot.integrationError}<p role="alert">
        {snapshot.integrationError}
      </p>{:else if registrationPending}<p role="alert">
        {t(
          'The saved autostart preference does not match a confirmed registration. Resolve the issue and retry registration.',
        )}
      </p>{/if}
    {#if stale}<p role="alert">
        {t('Settings changed in another window. Reload before saving.')}
      </p>{/if}
    <button disabled={busy || stale} onclick={save}
      >{snapshot.integrationError || registrationPending
        ? '' + t('Save and retry registration') + ''
        : '' + t('Save application settings') + ''}</button
    >
  {/if}
  <button disabled={busy} onclick={() => read(true)}>{t('Reload application settings')}</button>
  <button disabled={busy} onclick={exit}>{t('Exit application')}</button>
  {#if notice}<p role="status">{t(notice)}</p>{/if}
  {#if readError}<p role="alert">
      {readError instanceof ValidationError
        ? t(readError.resource, ...readError.values)
        : readError}
    </p>{/if}
  {#if error}<p role="alert">
      {error instanceof ValidationError ? t(error.resource, ...error.values) : error}
    </p>{/if}
</details>

<style>
  details {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 18px;
    margin-block: 16px;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  label {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-block: 14px;
  }
  input[type='checkbox'] {
    width: 18px;
    height: 18px;
    flex: none;
  }
  p {
    line-height: 1.5;
  }
  button {
    margin: 6px 8px 6px 0;
  }
  [role='alert'] {
    color: var(--danger);
  }
</style>
