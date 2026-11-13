<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import ExperimentalNotice from './ExperimentalNotice.svelte';
  import { ValidationError, type UiDiagnostic } from './validation-error';
  import { t, diagnosticText } from './i18n.svelte';
  import { untrack, tick } from 'svelte';
  import {
    videoReplacementSnapshot,
    replaceCollectionVideo,
    addImportedToCollection,
    readCollectionMembership,
    videoInCollection,
  } from './collection-membership';
  import GalleryVideoEditor from './GalleryVideoEditor.svelte';
  import MediaImport from './MediaImport.svelte';
  let captureDevice = $state<string | null>(null);
  $effect(() => {
    if (captureDevice && captureDevice !== device) captureDevice = null;
  });
  import MediaGallery from './MediaGallery.svelte';
  import MediaThumbnail from './MediaThumbnail.svelte';
  import { rpc, message, type PreparedClipInfo, type ScreenCollections } from './api';
  type Diagnostic = { detail: UiDiagnostic };
  type ImportFailure = Diagnostic & { context: 'edit' | 'membership' | 'replacement' };
  function importNoticeText(value: string | ImportFailure): string {
    if (typeof value === 'string') return t(value);
    switch (value.context) {
      case 'edit':
        return t('Could not open the video editor: {0}', diagnosticText(value.detail));
      case 'membership':
        return t(
          'The video was saved to the catalog, but adding it to the collection was not confirmed: {0}. Check the collections.',
          diagnosticText(value.detail),
        );
      case 'replacement':
        return t(
          'The new video was saved to the catalog, but replacement in the collection was not confirmed: {0}. Refresh the collections; it will not be retried automatically.',
          diagnosticText(value.detail),
        );
    }
  }
  let {
    device,
    collectionEpoch = 0,
    connected,
    available,
    disabled = false,
    keep = [],
    onselect,
    onstop,
    onbackground,
    onchoice = () => {},
    ondeleted = () => {},
  }: {
    device: string;
    collectionEpoch?: number;
    connected: boolean;
    available: boolean;
    disabled?: boolean;
    keep?: string[];
    onbackground?: (key: string) => void;
    onchoice?: (key: string) => void;
    ondeleted?: (key: string) => void;
    onselect: (key: string) => Promise<void>;
    onstop: () => Promise<void>;
  } = $props();
  let importBusy = $state(false),
    editBusy = $state(false);
  let editing = $state<{
    device: string;
    key: string;
    name: string;
    snapshot: ScreenCollections;
  } | null>(null);
  let editButton = $state<HTMLButtonElement>();
  let restoreEditFocus = $state(false);
  async function beginEdit() {
    if (
      captureDevice ||
      importBusy ||
      editBusy ||
      editing ||
      builtinChoice ||
      !selectable ||
      !ready ||
      disabled
    )
      return;
    const target = device,
      key = choice,
      name = clips.find((c) => c.key === key)?.name ?? t('Clip'),
      epoch = generation;
    editBusy = true;
    importNotice = '';
    try {
      const snapshot = await videoReplacementSnapshot(target, key);
      if (epoch === generation && target === device)
        editing = { device: target, key, name, snapshot };
    } catch (e) {
      if (epoch === generation)
        importNotice = { context: 'edit', detail: e instanceof ValidationError ? e : message(e) };
    } finally {
      editBusy = false;
    }
  }
  async function closeEditor() {
    editing = null;
    await tick();
    restoreEditFocus = true;
  }
  $effect(() => {
    // Closing mounts the import panel, whose status readback can refresh the
    // catalog. Wait until the edit action is actually enabled before focusing.
    const enabled =
      !disabled &&
      !importBusy &&
      !editBusy &&
      !editing &&
      !deleting &&
      !loading &&
      ready &&
      selectable &&
      !!choice &&
      !builtinChoice;
    if (!restoreEditFocus || !editButton) return;
    if (document.activeElement !== document.body && document.activeElement !== editButton) {
      restoreEditFocus = false;
      return;
    }
    if (enabled) {
      editButton.focus();
      restoreEditFocus = false;
    }
  });
  async function saveEdit(key: string) {
    const edit = editing,
      epoch = generation;
    if (!edit || editBusy) return;
    editBusy = true;
    try {
      await replaceCollectionVideo(edit.device, edit.snapshot, edit.key, key);
      if (epoch === generation && edit.device === device) {
        await refresh(epoch);
        if (epoch === generation) {
          choice = key;
          importNotice =
            'Video replaced in the collection. The original file was retained for other references.';
        }
      }
    } catch (e) {
      if (epoch === generation && edit.device === device)
        importNotice = {
          context: 'replacement',
          detail: e instanceof ValidationError ? e : message(e),
        };
    } finally {
      editBusy = false;
      if (editing === edit) await closeEditor();
    }
  }
  $effect(() => {
    if (editing && editing.device !== device) editing = null;
  });
  let importNotice = $state<string | ImportFailure>('');
  async function imported(key: string | null, target: string | null) {
    // A freshly mounted import panel can report the completed gallery job we
    // just loaded. Its unowned notification does not change this catalog.
    if (!target && key && clips.some((clip) => clip.key === key)) return;
    const epoch = generation;
    try {
      if (key && target) await addImportedToCollection(target, 'video', key);
    } catch (e) {
      if (epoch === generation)
        importNotice = {
          context: 'membership',
          detail: e instanceof ValidationError ? e : message(e),
        };
    } finally {
      if (epoch === generation) await refresh(epoch);
    }
  }
  let gallery = $state(false),
    catalogMode = $state(false);
  let membership = $state<ScreenCollections | null>(null);
  let open = $state(false),
    clips = $state<PreparedClipInfo[]>([]);
  let selected = $state<string | null>(null),
    choice = $state('');
  const selectable = $derived(videoInCollection(membership, choice));
  let loading = $state(false),
    ready = $state(false),
    error = $state<UiDiagnostic>('');
  let confirmDelete = $state(''),
    deleting = $state(false),
    deleteNotice = $state<string | Diagnostic>('');
  let clipSelect: HTMLSelectElement | undefined = $state();
  let refreshButton: HTMLButtonElement | undefined = $state();
  let generation = 0,
    request = 0;
  // Keep the selected clip available to the separate profile popover after dismissal.
  $effect(() => {
    const key = choice;
    if (open) untrack(() => onchoice(key));
  });
  $effect(() => {
    void device;
    void connected;
    void available;
    void collectionEpoch;
    untrack(() => onchoice(''));
  });
  const retainedChoice = $derived(keep.includes(choice));
  $effect(() => {
    if (confirmDelete && keep.includes(confirmDelete)) confirmDelete = '';
  });
  const builtinChoice = $derived(clips.find((clip) => clip.key === choice)?.is_default === true);
  const selectedName = $derived(clips.find((clip) => clip.key === selected)?.name);
  async function refresh(epoch = generation) {
    const current = ++request,
      target = device,
      canReadSelection = available;
    loading = true;
    ready = false;
    try {
      const [catalog, active, collections] = await Promise.all([
        rpc<PreparedClipInfo[]>('list_prepared_clips'),
        canReadSelection
          ? rpc<string | null>('get_lcd_media', { dev: target })
          : Promise.resolve(null),
        readCollectionMembership(target),
      ]);
      if (epoch !== generation || current !== request) return;
      membership = collections;
      const items = catalogMode
        ? catalog
        : catalog.filter((clip) => videoInCollection(collections, clip.key));
      clips = items;
      selected = active;
      ready = true;
      error = '';
      if (!items.some((clip) => clip.key === choice))
        choice = items.some((clip) => clip.key === active) ? active! : (items[0]?.key ?? '');
    } catch (e) {
      if (epoch !== generation || current !== request) return;
      error = e instanceof ValidationError ? e : message(e);
      ready = false;
      clips = [];
      selected = null;
      choice = '';
    } finally {
      if (epoch === generation && current === request) loading = false;
    }
  }
  async function removeClip() {
    if (
      builtinChoice ||
      retainedChoice ||
      deleting ||
      !connected ||
      disabled ||
      loading ||
      !ready ||
      !!error ||
      !choice ||
      confirmDelete !== choice
    )
      return;
    const key = choice,
      epoch = generation;
    deleting = true;
    confirmDelete = '';
    deleteNotice = '';
    try {
      await rpc<void>('delete_prepared_clip', { key });
      if (epoch === generation) deleteNotice = 'Video deleted.';
    } catch (e) {
      if (epoch === generation)
        deleteNotice = { detail: e instanceof ValidationError ? e : message(e) };
    } finally {
      if (epoch === generation) {
        deleting = false;
        await refresh(epoch);
        if (epoch === generation && ready && !clips.some((clip) => clip.key === key))
          ondeleted(key);
        await tick();
        if (epoch === generation) (clipSelect ?? refreshButton)?.focus();
      }
    }
  }
  $effect(() => {
    const online = connected,
      visible = open;
    void device;
    void available;
    void collectionEpoch;
    void catalogMode;
    const epoch = ++generation;
    clips = [];
    selected = null;
    choice = '';
    error = '';
    loading = false;
    ready = false;
    deleting = false;
    confirmDelete = '';
    deleteNotice = '';
    importNotice = '';
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    async function poll() {
      await refresh(epoch);
      if (!stopped) timer = setTimeout(poll, 1500);
    }
    if (online && visible)
      untrack(() => {
        void poll();
      });
    return () => {
      stopped = true;
      generation++;
      request++;
      clearTimeout(timer);
    };
  });
</script>

<StudioPopover label={`${t('Video library')}`} bind:open text wide>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t(
        'The selected video loops and resumes after restart. Applying a theme or restoring LocalUI clears this selection.',
      )}
    </p></StudioPopover
  >
  {#if !connected}<p role="status">
      {t('The library is unavailable while the daemon is disconnected.')}
    </p>
  {:else}
    {#if open && !editing && !captureDevice}<MediaImport
        onbusy={(value) => (importBusy = value)}
        device={available ? device : null}
        onimport={(key, target) => {
          if (target) importNotice = '';
          void imported(key, target);
        }}
      />{/if}
    {#if open && editing}
      {#key editing.key}<GalleryVideoEditor
          clipKey={editing.key}
          name={editing.name}
          device={editing.device}
          saving={editBusy}
          onclose={() => {
            if (!editBusy) void closeEditor();
          }}
          onsave={(key) => void saveEdit(key)}
        />{/key}
    {/if}
    {#if open && captureDevice}
      {#key captureDevice}{#await import('./ScreenRecorder.svelte')}
          <p role="status">{t('Loading…')}</p>
        {:then { default: ScreenRecorder }}
          <ScreenRecorder
            device={captureDevice}
            onclose={() => (captureDevice = null)}
            onsave={async (key, target) => {
              await imported(key, target);
            }}
          />
        {/await}{/key}
    {:else if open && !editing}
      <ExperimentalNotice
        feature="recording"
        message="Screen capture and recording are not verified in every supported session. Use at your own risk."
      />
      <button
        disabled={disabled || importBusy || editBusy || !available}
        onclick={() => (captureDevice = device)}>{t('Record screen…')}</button
      >
    {/if}
    <button bind:this={refreshButton} disabled={disabled || loading} onclick={() => refresh()}
      >{t('Refresh videos')}</button
    >
    {#if loading && !clips.length}<p role="status">{t('Loading videos…')}</p>{/if}
    {#if error}<p role="alert">{t('Could not load videos:')} {diagnosticText(error)}</p>
    {:else if !loading && !clips.length}<p>{t('No saved videos yet.')}</p>{/if}
    <label
      ><input type="checkbox" bind:checked={catalogMode} />{t(
        'Show all video files for management',
      )}</label
    >
    {#if clips.length}
      <label
        >{t('Saved video')}
        <select
          bind:this={clipSelect}
          bind:value={choice}
          disabled={disabled || deleting || loading || !ready}
          onchange={() => {
            confirmDelete = '';
          }}
        >
          {#each clips as clip (clip.key)}<option value={clip.key}
              >{clip.name} · {(clip.frames / clip.fps).toFixed(1)} {t('s')}</option
            >{/each}
        </select>
      </label>
      <label><input type="checkbox" bind:checked={gallery} />{t('Show video cards')}</label>
      {#if open && gallery}<MediaGallery
          {clips}
          {choice}
          disabled={disabled || deleting || loading || !ready}
          onchoose={(key) => {
            choice = key;
            confirmDelete = '';
          }}
        />{/if}
      {#if open && choice && !gallery}<MediaThumbnail
          clipKey={choice}
          name={clips.find((clip) => clip.key === choice)?.name ?? t('Clip')}
        />{/if}
      {#if !selectable}<StudioPopover label={t('Help')} icon="help"
          ><p>
            {t(
              'This file is outside the background collection. Add it through LCD collections to play it.',
            )}
          </p></StudioPopover
        >{/if}
      {#if onbackground}<button
          disabled={!selectable || disabled || deleting || loading || !choice || !!error}
          onclick={() => onbackground?.(choice)}>{t('Use as theme background')}</button
        >{/if}
      <button
        disabled={!selectable ||
          !available ||
          disabled ||
          deleting ||
          loading ||
          !ready ||
          !choice ||
          !!error}
        onclick={() => onselect(choice)}>{t('Select video for LCD')}</button
      >
      <button
        bind:this={editButton}
        disabled={!!captureDevice ||
          disabled ||
          importBusy ||
          editBusy ||
          !!editing ||
          deleting ||
          loading ||
          !ready ||
          !selectable ||
          !choice ||
          builtinChoice}
        onclick={beginEdit}>{t('Edit selected video')}</button
      >
      {#if builtinChoice}<p>{t('Built-in videos cannot be deleted or edited.')}</p>{/if}
      <button
        disabled={disabled ||
          deleting ||
          loading ||
          !choice ||
          !!error ||
          builtinChoice ||
          retainedChoice ||
          choice === selected}
        onclick={() => {
          if (keep.includes(choice)) return;
          confirmDelete = choice;
          deleteNotice = '';
        }}>{t('Delete video…')}</button
      >
      {#if confirmDelete && confirmDelete === choice}
        <p>
          {t('Delete «')}{clips.find((clip) => clip.key === confirmDelete)?.name}{t(
            '» from the library? The original file will remain on disk.',
          )}
        </p>
        <button
          disabled={disabled || deleting || loading || !ready || retainedChoice}
          onclick={removeClip}>{t('Confirm video deletion')}</button
        >
        <button onclick={() => (confirmDelete = '')}>{t('Cancel deletion')}</button>
      {/if}
      {#if retainedChoice}<p>
          {t('The video is used by an LCD page draft. Replace its background before deleting.')}
        </p>{/if}
      {#if choice === selected}<p>
          {t('Stop the video first to clear the saved selection.')}
        </p>{/if}
    {/if}
    {#if importNotice}<p role="alert">{importNoticeText(importNotice)}</p>{/if}
    {#if deleteNotice}<p role="status">
        {typeof deleteNotice === 'string'
          ? t(deleteNotice)
          : t(
              'Deletion was not confirmed: {0}. Check the updated list before retrying.',
              diagnosticText(deleteNotice.detail),
            )}
      </p>{/if}
    {#if ready && available}<p role="status" aria-live="polite" aria-atomic="true">
        {selected
          ? t('Saved selection: {0}', selectedName ?? t('video is unavailable in the library'))
          : '' + t('No video selected for automatic restore.') + ''}
      </p>{/if}
    <button disabled={!available || disabled} onclick={onstop}
      >{t('Stop video and restore LocalUI')}</button
    >
    {#if !available}<StudioPopover label={t('Help')} icon="help"
        ><p>{t('Select an available device for playback.')}</p></StudioPopover
      >{/if}
  {/if}
</StudioPopover>

<style>
  label {
    display: grid;
    gap: 6px;
    margin: 12px 0;
  }
  select {
    min-width: 0;
    width: 100%;
  }
  p {
    overflow-wrap: anywhere;
  }
  button {
    margin: 4px 8px 4px 0;
  }
</style>
