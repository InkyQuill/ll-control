<script lang="ts">
  import StudioPopover from './StudioPopover.svelte';
  import { t } from './i18n.svelte';
  import { tick, untrack } from 'svelte';
  import {
    rpc,
    message,
    type ScreenCollections,
    type ScreenCollectionsState,
    type CollectionContext,
    type CollectionAsset,
  } from './api';
  let {
    refreshEpoch = 0,
    device,
    connected,
    disabled = false,
    onchange = () => {},
  }: {
    refreshEpoch?: number;
    onchange?: () => void;
    device: string;
    connected: boolean;
    disabled?: boolean;
  } = $props();
  interface CatalogAsset extends CollectionAsset {
    name: string;
    is_default: boolean;
    contexts?: CollectionContext[];
  }
  let open = $state(false),
    busy = $state(false),
    loading = $state(false),
    needsReadback = $state(true);
  let currentState = $state<ScreenCollectionsState | null>(null),
    draft = $state<ScreenCollections | null>(null);
  let assets = $state<CatalogAsset[]>([]),
    context = $state<CollectionContext>('image'),
    choice = $state('');
  type Notice = { kind: 'migrated' | 'saved' } | { kind: 'unconfirmed'; diagnostic: string };
  let error = $state(''),
    notice = $state<Notice | null>(null);
  let refreshButton: HTMLButtonElement | undefined = $state();
  let generation = 0,
    request = 0;
  const members = $derived<CollectionAsset[]>(
    draft
      ? context === 'background'
        ? draft.backgrounds
        : (context === 'image' ? draft.images : draft.watermarks).map((key) => ({
            kind: 'image',
            key,
          }))
      : [],
  );
  const identity = (asset: CollectionAsset) => `${asset.kind}:${asset.key}`;
  const candidates = $derived(
    assets.filter(
      (asset) =>
        (context === 'background' || asset.kind === 'image') &&
        (!asset.contexts || asset.contexts.includes(context)) &&
        !members.some((member) => identity(member) === identity(asset)),
    ),
  );
  const changed = $derived(
    JSON.stringify(draft) !== JSON.stringify(currentState?.collections ?? null),
  );
  const locked = $derived(
    !connected ||
      disabled ||
      busy ||
      loading ||
      needsReadback ||
      !!currentState?.persistence.pending,
  );
  const name = (asset: CollectionAsset) =>
    assets.find((item) => identity(item) === identity(asset))?.name ?? asset.key;
  // Only canonical membership is protected. Midnight's background membership
  // can come from legacy migration and is removable without deleting its file.
  const protectedMember = (asset: CollectionAsset) =>
    asset.kind === 'video'
      ? asset.key === 'bcf5a102bf2a426286d34310cad4a001'
      : asset.key === 'builtin-orbit-mark-v1' ||
        (context === 'image' && asset.key === 'builtin-midnight-v1');
  async function refresh(epoch = generation) {
    const current = ++request,
      target = device;
    loading = true;
    needsReadback = true;
    try {
      const [result, images, videos] = await Promise.all([
        rpc<ScreenCollectionsState>('get_screen_collections', { dev: target }),
        rpc<
          {
            key: string;
            name: string | null;
            is_default?: boolean;
            contexts?: CollectionContext[];
          }[]
        >('list_named_theme_assets'),
        rpc<{ key: string; name: string; is_default?: boolean }[]>('list_prepared_clips'),
      ]);
      if (epoch !== generation || current !== request) return;
      currentState = result;
      draft = result.collections ? structuredClone(result.collections) : null;
      assets = [
        ...images.map((item) => ({
          ...item,
          kind: 'image' as const,
          name: item.name ?? item.key,
          is_default: !!item.is_default,
        })),
        ...videos.map((item) => ({
          ...item,
          kind: 'video' as const,
          is_default: !!item.is_default,
        })),
      ];
      choice = '';
      needsReadback = false;
      error = '';
    } catch (e) {
      if (epoch === generation && current === request) error = message(e);
    } finally {
      if (epoch === generation && current === request) loading = false;
    }
  }
  $effect(() => {
    void refreshEpoch;
    void device;
    void connected;
    void open;
    const epoch = ++generation;
    currentState = null;
    draft = null;
    assets = [];
    error = '';
    notice = null;
    needsReadback = true;
    loading = false;
    if (open && connected) untrack(() => void refresh(epoch));
  });
  function replaceMembers(next: CollectionAsset[]) {
    if (!draft || locked) return;
    draft =
      context === 'background'
        ? { ...draft, backgrounds: next }
        : context === 'image'
          ? { ...draft, images: next.map((item) => item.key) }
          : { ...draft, watermarks: next.map((item) => item.key) };
    choice = '';
  }
  async function remove(asset: CollectionAsset) {
    const epoch = generation,
      selectedContext = context;
    replaceMembers(members.filter((item) => identity(item) !== identity(asset)));
    await tick();
    if (epoch === generation && selectedContext === context) refreshButton?.focus();
  }
  async function save(migrate = false) {
    if (locked || !currentState || (!migrate && (!draft || !changed))) return;
    const epoch = generation,
      target = device;
    const next = draft ? { ...$state.snapshot(draft), revision: draft.revision + 1 } : null;
    busy = true;
    needsReadback = true;
    error = '';
    notice = null;
    let outcome: Notice | null = null;
    try {
      if (migrate) await rpc('migrate_screen_collections', { dev: target });
      else
        await rpc('set_screen_collections', {
          dev: target,
          expected_revision: currentState.collections!.revision,
          collections: next,
        });
      outcome = { kind: migrate ? 'migrated' : 'saved' };
    } catch (e) {
      outcome = { kind: 'unconfirmed', diagnostic: message(e) };
    } finally {
      // A reopened panel may have read the old value while this write ran.
      const settledGeneration = generation;
      if (open && connected) await refresh(settledGeneration);
      busy = false;
      onchange();
      if (epoch === generation) {
        notice = outcome;
        await tick();
        if (epoch === generation) refreshButton?.focus();
      }
    }
  }
</script>

<StudioPopover label={`${t('LCD collections')}`} bind:open text wide>
  <StudioPopover label={t('Help')} icon="help"
    ><p>
      {t(
        'Lists for this device. Removing an item from a list keeps its file in the catalog and does not change the current theme.',
      )}
    </p></StudioPopover
  >
  <button
    bind:this={refreshButton}
    disabled={!connected || busy || loading}
    onclick={() => refresh()}>{t('Refresh collections')}</button
  >
  {#if loading}<p role="status">{t('Reading collections…')}</p>{/if}
  {#if error}<p role="alert">{t('Could not check the collections:')} <span>{error}</span></p>{/if}
  {#if notice}<p role="status">
      {#if notice.kind === 'unconfirmed'}{t('The write result needs checking:')}
        <span>{notice.diagnostic}</span>{:else}{notice.kind === 'migrated'
          ? t('Catalog migrated to collections.')
          : t('Collection membership saved.')}{/if}
    </p>{/if}
  {#if currentState?.persistence.pending}<p role="alert">
      {t('Collection changes are waiting to be written.')}
      {currentState.persistence.last_error ?? ''}
      {t('Refresh collections to check their state.')}
    </p>{/if}
  {#if currentState && !draft}
    <StudioPopover label={t('Help')} icon="help"
      ><p>
        {t(
          'The shared catalog is in use. Migration retains available images and videos in separate lists.',
        )}
      </p></StudioPopover
    >
    <button disabled={locked} onclick={() => save(true)}
      >{t('Migrate catalog to collections')}</button
    >
  {:else if draft}
    <label
      >{t('Collection')}<select bind:value={context} onchange={() => (choice = '')}
        ><option value="image">{t('Images')}</option><option value="background"
          >{t('Backgrounds')}</option
        ><option value="watermark">{t('Watermarks')}</option></select
      ></label
    >
    <ul>
      {#each members as asset (identity(asset))}<li>
          <span>{name(asset)}</span><button
            disabled={locked || protectedMember(asset)}
            aria-label={t('Remove from collection: {0}', name(asset))}
            onclick={() => remove(asset)}>{t('Remove from list')}</button
          >{#if protectedMember(asset)}<span>{t('Built-in item')}</span>{/if}
        </li>{/each}
    </ul>
    <label
      >{t('Add from catalog')}<select bind:value={choice} disabled={locked}
        ><option value="">{t('Select a file')}</option
        >{#each candidates as asset (identity(asset))}<option value={identity(asset)}
            >{name(asset)} · {asset.kind === 'video'
              ? '' + t('Video') + ''
              : '' + t('Image') + ''}</option
          >{/each}</select
      ></label
    >
    <button
      disabled={locked || !choice || members.length >= 256}
      onclick={() => {
        const asset = candidates.find((item) => identity(item) === choice);
        if (asset) replaceMembers([...members, { kind: asset.kind, key: asset.key }]);
      }}>{t('Add to list')}</button
    >
    <button disabled={locked || !changed} onclick={() => save()}>{t('Save collections')}</button>
    {#if changed}<p role="status">
        {t('The collection draft changed. Save it, or refresh collections to discard the changes.')}
      </p>{/if}
  {/if}
</StudioPopover>

<style>
  label {
    display: grid;
    gap: 0.3rem;
    margin-block: 0.6rem;
  }
  ul {
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin-block: 0.4rem;
  }
  li span:first-child {
    flex: 1;
    overflow-wrap: anywhere;
  }
  select {
    max-width: 100%;
  }
  button {
    margin: 0.2rem;
  }
</style>
