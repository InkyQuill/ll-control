<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import {
    ArchiveUiError,
    archiveError,
    archiveMessage,
    archiveDiagnostic,
    type ArchiveMessage,
  } from './archive-messages';
  import { presentation } from './presentation.svelte';
  import { untrack, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { type ProfileGroup, type ProfileLibrary } from './api';
  import {
    archiveTerminal,
    archivePhase,
    profileGroupNames,
    readPendingArchive,
    pendingArchiveKey,
    type PendingArchive,
    type ArchiveStatus,
  } from './profile-archive';
  let {
    group,
    slot,
    revision,
    name,
    hasProfile,
    connected,
    disabled = false,
    busy = $bindable(false),
    onlibrary,
    onrequireRefresh,
  }: {
    group: ProfileGroup;
    slot: number;
    revision: number;
    name: string;
    hasProfile: boolean;
    connected: boolean;
    disabled?: boolean;
    busy?: boolean;
    onlibrary: (library: ProfileLibrary, target: PendingArchive) => void;
    onrequireRefresh: () => void;
  } = $props();
  const initialPending = readPendingArchive();
  let open = $state(false),
    job = $state<ArchiveStatus | null>(null),
    pending = $state<PendingArchive | null>(initialPending);
  let requesting = $state(false);
  let recoveryId = $state(initialPending?.id ?? '');
  let error = $state<string | ArchiveMessage>('');
  let notice = $state<{ source: string; values: unknown[] } | null>(null);
  let mutation = 0,
    view = 0,
    ownedView = -1,
    ownedId = '',
    handled = '';
  const active = $derived(requesting || (!!job && !archiveTerminal(job.phase)));
  const unavailable = $derived(disabled || !connected || active || !!pending);
  const cancellable = $derived(
    !!job &&
      !archiveTerminal(job.phase) &&
      !['committing', 'recovering', 'cancelling'].includes(job.phase),
  );
  const recoveryValid = $derived(/^[a-f0-9]{32}$/.test(recoveryId));
  $effect(() => {
    void group;
    void connected;
    untrack(() => {
      view++;
      ownedView = -1;
      error = '';
      notice = null;
    });
  });
  $effect(() => {
    busy = active;
  });
  onDestroy(() => {
    busy = false;
  });
  function keep(value: PendingArchive | null) {
    const current = readPendingArchive();
    if (value && current && current.id !== value.id) {
      pending = current;
      recoveryId = current.id;
      throw new ArchiveUiError('pending_profile_operation');
    }
    if (value) localStorage.setItem(pendingArchiveKey, JSON.stringify(value));
    pending = value;
  }
  function forget(id: string) {
    if (readPendingArchive()?.id === id) localStorage.removeItem(pendingArchiveKey);
    pending = readPendingArchive();
  }
  function accept(status: ArchiveStatus | null) {
    job = status;
    if (!status || !archiveTerminal(status.phase)) return;
    const key = JSON.stringify([
      status.id,
      status.phase,
      status.result?.state.committed_revision,
      status.result?.state.library?.revision,
      status.message,
    ]);
    if (handled === key) return;
    handled = key;
    if (status.phase === 'uncertain') {
      if (status.id === pending?.id) onrequireRefresh();
      return;
    }
    if (status.phase === 'failed') {
      if (status.id === pending?.id) {
        forget(status.id);
        if (status.result) onrequireRefresh();
      }
      return;
    }
    if (status.phase === 'cancelled') {
      if (status.id === pending?.id) forget(status.id);
      error = '';
      notice = { source: 'Transfer cancelled.', values: [] };
      return;
    }
    if (status.phase === 'complete') {
      error = '';
      if (status.id === pending?.id) forget(status.id);
      const result = status.result?.state;
      if (
        status.mode === 'import' &&
        result?.library &&
        status.id === ownedId &&
        ownedView === view &&
        status.group === group
      ) {
        onlibrary(result.library, { id: status.id, group: status.group, slot: status.slot });
      }
      notice =
        status.mode === 'export'
          ? { source: 'Archive saved to file.', values: [] }
          : {
              source: 'Import confirmed: slot {0}. Device settings have not been applied.',
              values: [status.slot + 1],
            };
    }
  }
  $effect(() => {
    const online = connected,
      visible = open || active;
    if (!online || !visible) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const started = mutation;
      try {
        const status = await invoke<ArchiveStatus | null>('profile_archive_status');
        if (!cancelled && started === mutation) accept(status);
      } catch (e) {
        if (!cancelled && started === mutation) error = archiveError(e);
      }
      if (!cancelled) timer = setTimeout(poll, 500);
    }
    timer = setTimeout(poll, 0);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });
  async function start(mode: 'import' | 'export') {
    if (unavailable || (mode === 'export' && !hasProfile)) return;
    const id = crypto.randomUUID().replaceAll('-', ''),
      epoch = view,
      target = { id, group, slot };
    mutation++;
    requesting = true;
    error = '';
    notice = null;
    handled = '';
    ownedId = id;
    ownedView = epoch;
    try {
      const current = await invoke<ArchiveStatus | null>('profile_archive_status');
      if (epoch !== view) return;
      if (current && (!archiveTerminal(current.phase) || current.phase === 'uncertain')) {
        keep({ id: current.id, group: current.group, slot: current.slot });
        recoveryId = current.id;
        accept(current);
        return;
      }
      keep(target);
      recoveryId = id;
      const status = await invoke<ArchiveStatus>('profile_archive_start', {
        language: presentation.language,
        request: {
          id,
          mode,
          group,
          slot,
          expectedRevision: revision,
          suggestedName: `${name.trim() || 'profile'}.lcp`,
        },
      });
      if (epoch === view) accept(status);
    } catch (e) {
      if (epoch === view) {
        error = archiveError(e);
        onrequireRefresh();
      }
    } finally {
      requesting = false;
      mutation++;
    }
  }
  async function cancel() {
    if (!job || !cancellable) return;
    const id = job.id,
      epoch = view;
    mutation++;
    error = '';
    try {
      const status = await invoke<ArchiveStatus>('profile_archive_cancel', { id });
      if (epoch === view) accept(status);
    } catch (e) {
      if (epoch === view) error = archiveError(e);
    } finally {
      mutation++;
    }
  }
  async function recover(action: 'status' | 'finish' | 'cancel') {
    if (active || !connected || !recoveryValid || (action === 'finish' && disabled)) return;
    const id = recoveryId,
      epoch = view,
      target = pending?.id === id ? pending : { id, group, slot };
    mutation++;
    requesting = true;
    error = '';
    notice = null;
    handled = '';
    ownedId = id;
    ownedView = epoch;
    try {
      keep(target);
      const status = await invoke<ArchiveStatus>('profile_archive_recover', {
        id,
        action,
        group: target.group,
        slot: target.slot,
      });
      if (epoch === view) accept(status);
    } catch (e) {
      if (epoch === view) {
        error = archiveError(e);
        onrequireRefresh();
      }
    } finally {
      requesting = false;
      mutation++;
    }
  }
  async function dismiss() {
    if (active || disabled) return;
    const id = job?.id ?? pending?.id;
    if (!id) return;
    const epoch = view;
    mutation++;
    error = '';
    try {
      await invoke('profile_archive_dismiss', { id });
      if (epoch === view) {
        forget(id);
        job = null;
        notice = {
          source: 'Details closed. The list shows the current slot contents.',
          values: [],
        };
      }
    } catch (e) {
      if (epoch === view) error = archiveError(e);
    } finally {
      mutation++;
    }
  }
</script>

<StudioPopover label={`${t('Profile import and export')}`} bind:open text wide>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t(
        'An LL-Control archive (.lcp) contains one slot and its images, videos and fonts. Import replaces the selected slot. Applying to a device is a separate action.',
      )}
    </p></StudioPopover
  >
  <div class="buttons">
    <button disabled={unavailable || !hasProfile} onclick={() => start('export')}
      >{t('Export slot to archive…')}</button
    >
    <button disabled={unavailable} onclick={() => start('import')}
      >{t('Import archive into slot…')}</button
    >
  </div>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t('Choose a new filename for export: existing files are never replaced.')}
    </p></StudioPopover
  >
  {#if job}
    <p role="status">
      {t(
        '{0} · {1} · slot {2}',
        t(archivePhase[job.phase]),
        t(profileGroupNames[job.group]),
        job.slot + 1,
      )}{job.file_name ? ` · ${job.file_name}` : ''}
    </p>
    {#if active}<progress
        aria-label={t('Archive transfer')}
        max={job.total || 1}
        value={job.total ? job.transferred : undefined}
      ></progress>
      <p>
        {job.transferred.toLocaleString(presentation.language)} / {job.total
          ? job.total.toLocaleString(presentation.language)
          : '…'}
        {t('bytes')}
      </p>{/if}
    {#if cancellable}<button onclick={cancel}>{t('Cancel archive transfer')}</button>{/if}
    {#if job.phase === 'committing'}<p>
        {t(
          'Saving has already started. Wait for the result; if disconnected, read it back using the operation ID.',
        )}
      </p>{/if}
    {#if job.message || job.message_code}<p
        role={job.phase === 'failed' || job.phase === 'uncertain' ? 'alert' : 'status'}
      >
        {archiveMessage(job, presentation.language)}
      </p>
      {#if archiveDiagnostic(job, presentation.language)}<StudioPopover
          label={`${t('Technical diagnostics')}`}
          text><pre>{archiveDiagnostic(job, presentation.language)}</pre></StudioPopover
        >{/if}{/if}
  {/if}
  {#if pending}<p>
      {t('An unfinished operation ID was saved:')} <code>{pending.id}</code>{t(
        '. Before starting a new operation, read the result and refresh the profile list.',
      )}
    </p>{/if}
  <label
    >{t('Operation ID for recovery')}<input
      bind:value={recoveryId}
      maxlength="32"
      spellcheck="false"
      disabled={active}
    /></label
  >
  <div class="buttons">
    <button disabled={active || !connected || !recoveryValid} onclick={() => recover('status')}
      >{t('Read archive result')}</button
    >
    {#if job?.result?.state.phase === 'uploading' && job.result.state.received === job.result.state.size}<button
        disabled={active || !connected || disabled}
        onclick={() => recover('finish')}>{t('Finish uploaded archive')}</button
      >{/if}
    {#if job?.phase === 'uncertain'}<button
        disabled={!connected || !recoveryValid}
        onclick={() => recover('cancel')}>{t('Confirm transfer cancellation')}</button
      >{/if}
    {#if pending || (job && archiveTerminal(job.phase))}<button
        disabled={active || disabled}
        onclick={dismiss}>{t('Close archive details')}</button
      >{/if}
  </div>
  {#if error}<p role="alert">{archiveMessage(error, presentation.language)}</p>
    {#if archiveDiagnostic(error, presentation.language)}<StudioPopover
        label={`${t('Technical diagnostics')}`}
        text><pre>{archiveDiagnostic(error, presentation.language)}</pre></StudioPopover
      >{/if}{/if}
  {#if notice}<p role="status">{t(notice.source, ...notice.values)}</p>{/if}
</StudioPopover>

<style>
  pre {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
  p {
    line-height: 1.5;
  }
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
  progress {
    width: 100%;
  }
  [role='alert'] {
    color: var(--danger);
  }
  code {
    user-select: all;
    overflow-wrap: anywhere;
  }
</style>
