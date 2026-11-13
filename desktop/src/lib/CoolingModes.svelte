<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { t, diagnosticText } from './i18n.svelte';
  import { rpc, message, coolingModeDefault, type CoolingPolicy, type Sensor } from './api';
  import {
    coolingModes,
    coolingPolicyKey,
    coolingModeLabel,
    defaultCoolingModeName,
    supportsCoolingModeDefault,
    type DeviceCoolingModes,
    type DeviceCoolingMode,
    type CoolingModeReference,
  } from './cooling-modes';
  import { ValidationError, type UiDiagnostic } from './validation-error';
  let {
    device,
    target,
    connected,
    disabled = false,
    onload,
    draft,
    onsaved,
    draftContext = () => 0,
    sensors = [],
    defaultSensor,
    draftStamp,
    onrestore,
  }: {
    device: string;
    target: 'fan' | 'pump';
    connected: boolean;
    disabled?: boolean;
    onload: (policy: CoolingPolicy, reference: CoolingModeReference) => void;
    draft: () => CoolingPolicy;
    onsaved?: (reference: CoolingModeReference) => void;
    draftContext?: () => number;
    sensors?: Sensor[];
    defaultSensor: () => string;
    draftStamp: () => string;
    onrestore: (policy: CoolingPolicy) => void;
  } = $props();
  let library = $state.raw<DeviceCoolingModes | null>(null),
    loaded = $state(false),
    loading = $state(false);
  let selected = $state(''),
    error = $state<UiDiagnostic | null>(null);
  let editName = $state(''),
    editId = $state(''),
    editRevision = $state(''),
    saving = $state(false),
    errorContext = $state<'read' | 'save' | 'create' | 'restore'>('read');
  let loadedProfile = $state.raw<DeviceCoolingMode | null>(null),
    creationNotice = $state<'created' | 'found' | null>(null);
  let createSensor = $state(''),
    pendingCreate = $state(false),
    creating = $state(false);
  let editDefaultLabel = $state(''),
    editUsesDefaultName = $state(false);
  let restoring = $state(false),
    restoreNotice = $state<'name' | 'profile' | 'changed' | null>(null);
  const temperatures = $derived(sensors.filter((sensor) => sensor.unit === 'celsius'));
  let readButton: HTMLButtonElement | undefined;
  let modeSelect = $state<HTMLSelectElement | undefined>(undefined);
  type SaveAttempt = {
    dev: string;
    target: 'fan' | 'pump';
    revision: string;
    profile: DeviceCoolingMode;
    context: number;
  };
  let pendingSave = $state.raw<SaveAttempt | null>(null),
    savedSnapshot = $state.raw<DeviceCoolingMode | null>(null);
  let generation = 0;
  const modes = $derived(library?.[target] ?? []);
  const chosen = $derived(modes.find((mode) => mode.id === selected));
  const factorySupported = $derived(!!chosen && supportsCoolingModeDefault(chosen.id, target));
  const factoryLabel = $derived(
    chosen && factorySupported
      ? coolingModeLabel({
          ...chosen,
          name: defaultCoolingModeName(chosen.id)!,
          use_default_name: true,
        })
      : '',
  );
  const factoryNeedsSource = $derived(!!chosen && !(target === 'pump' && chosen.id === 'FixRPM'));
  const factorySourceKnown = $derived(
    sensors.some((sensor) => sensor.id === defaultSensor() && sensor.unit === 'celsius'),
  );
  const conflict = $derived(
    !pendingSave && !!editId && !!library && library.revision !== editRevision,
  );
  const draftChanged = $derived.by(() => {
    if (!savedSnapshot) return false;
    try {
      return (
        (savedSnapshot.use_default_name
          ? !editUsesDefaultName || editName !== editDefaultLabel
          : editName !== coolingModeLabel(savedSnapshot)) ||
        coolingPolicyKey(draft()) !== coolingPolicyKey(savedSnapshot.policy)
      );
    } catch {
      return true;
    }
  });
  let scope = '';
  $effect(() => {
    const nextScope = JSON.stringify([device, target]),
      online = connected;
    generation++;
    untrack(() => {
      library = null;
      loaded = false;
      loading = false;
      saving = false;
      loadedProfile = null;
      error = null;
      if (scope !== nextScope) {
        selected = '';
        editName = '';
        editId = '';
        editRevision = '';
        pendingSave = null;
        savedSnapshot = null;
        errorContext = 'read';
        pendingCreate = false;
        createSensor = '';
        editUsesDefaultName = false;
        editDefaultLabel = '';
      } else if (pendingSave || pendingCreate) {
        errorContext = pendingCreate ? 'create' : 'save';
        error = pendingCreate
          ? new ValidationError(
              'The creation response is unconfirmed. Read device modes after reconnecting.',
            )
          : new ValidationError(
              'The save response is unconfirmed. Read device modes after reconnecting.',
            );
      }
      scope = nextScope;
      restoring = false;
      restoreNotice = null;
      creating = false;
      creationNotice = null;
      // Keep an unresolved save across connection changes. Only an explicit read
      // of this same device/channel can reconcile it; never replay the write.
      if (!online) loaded = false;
    });
    return () => {
      generation++;
    };
  });
  async function refresh() {
    if (!device || !connected || disabled || loading || saving || restoring) return;
    const started = ++generation,
      dev = device,
      channel = target;
    const current = () =>
      started === generation && device === dev && target === channel && connected;
    const attempt = pendingSave,
      creation = pendingCreate;
    loading = true;
    loaded = false;
    loadedProfile = null;
    error = null;
    errorContext = 'read';
    try {
      const value = await rpc<unknown>('get_cooling_modes', { dev });
      if (!current()) return;
      const snapshot = coolingModes(value);
      if (attempt) reconcile(snapshot, attempt, true);
      else if (creation) reconcileCreation(snapshot, true, false);
      else {
        library = snapshot;
        loaded = true;
        if (!snapshot?.[channel].some((mode) => mode.id === selected)) {
          selected = '';
          editId = '';
          editRevision = '';
        }
      }
    } catch (cause) {
      if (current()) {
        library = null;
        loaded = false;
        error = cause instanceof ValidationError ? cause : message(cause);
      }
    } finally {
      if (current()) loading = false;
    }
  }
  function load() {
    if (
      !chosen ||
      !library ||
      disabled ||
      loading ||
      saving ||
      restoring ||
      pendingSave ||
      pendingCreate ||
      !connected ||
      !device
    )
      return;
    onload(chosen.policy, {
      id: chosen.id,
      revision: library.revision,
      policy: structuredClone(chosen.policy),
    });
    loadedProfile = chosen;
    creationNotice = null;
    restoreNotice = null;
    editName = coolingModeLabel(chosen);
    editUsesDefaultName = !!chosen.use_default_name;
    editDefaultLabel = editName;
    editId = chosen.id;
    editRevision = library.revision;
    savedSnapshot = null;
    error = null;
  }
  function choose(id: string) {
    restoreNotice = null;
    creationNotice = null;
    selected = id;
    editUsesDefaultName = false;
    editDefaultLabel = '';
    editName = modes.find((mode) => mode.id === id)?.name ?? '';
    editId = '';
    editRevision = '';
    loadedProfile = null;
    savedSnapshot = null;
    error = null;
  }
  function reconcile(snapshot: DeviceCoolingModes | null, attempt: SaveAttempt, explicit: boolean) {
    library = snapshot;
    loaded = true;
    selected = attempt.profile.id;
    errorContext = 'save';
    const mode = snapshot?.[attempt.target].find((mode) => mode.id === attempt.profile.id);
    const durable =
      !!snapshot && !snapshot.persistence.pending && snapshot.persistence.last_error === null;
    if (
      mode &&
      durable &&
      mode.name === attempt.profile.name &&
      !!mode.use_default_name === !!attempt.profile.use_default_name &&
      coolingPolicyKey(mode.policy) === coolingPolicyKey(attempt.profile.policy)
    ) {
      pendingSave = null;
      savedSnapshot = attempt.profile;
      editId = mode.id;
      editRevision = snapshot!.revision;
      loadedProfile = null;
      error = null;
      if (attempt.context === draftContext())
        onsaved?.({
          id: mode.id,
          revision: snapshot!.revision,
          policy: structuredClone(attempt.profile.policy),
        });
      return;
    }
    if (snapshot && !durable)
      error = snapshot.persistence.last_error
        ? new ValidationError('Settings storage is unconfirmed: {0}', [
            snapshot.persistence.last_error,
          ])
        : new ValidationError('Settings are still being saved. Read them again.');
    else
      error = new ValidationError(
        'The current mode differs from the submitted snapshot. Your draft is preserved. Read the modes before saving again.',
      );
    if (explicit && (durable || snapshot === null)) {
      pendingSave = null;
      if (!mode) {
        selected = '';
        editId = '';
        editRevision = '';
      }
    }
  }
  function reconcileCreation(
    snapshot: DeviceCoolingModes | null,
    explicit: boolean,
    reply: boolean,
  ) {
    library = snapshot;
    loaded = true;
    errorContext = 'create';
    if (snapshot && !snapshot.persistence.pending && snapshot.persistence.last_error === null) {
      pendingCreate = false;
      creationNotice = reply ? 'created' : 'found';
      error = null;
      return;
    }
    if (snapshot)
      error = snapshot.persistence.last_error
        ? new ValidationError('Settings storage is unconfirmed: {0}', [
            snapshot.persistence.last_error,
          ])
        : new ValidationError('Settings are still being saved. Read them again.');
    else {
      error = new ValidationError(
        'No mode bank is available yet. Read the data before trying to create it again.',
      );
      if (explicit) {
        pendingCreate = false;
        error = new ValidationError(
          'The bank is absent. Check the selected source before trying to create it again.',
        );
      }
    }
  }
  async function initialize() {
    if (
      !loaded ||
      library !== null ||
      disabled ||
      !connected ||
      !device ||
      loading ||
      saving ||
      restoring ||
      pendingSave ||
      pendingCreate
    )
      return;
    if (!temperatures.some((sensor) => sensor.id === createSensor)) return;
    const started = ++generation,
      dev = device,
      channel = target,
      sensor = createSensor,
      focused = document.activeElement;
    const current = () =>
      started === generation && device === dev && target === channel && connected;
    saving = true;
    creating = true;
    pendingCreate = true;
    error = null;
    errorContext = 'create';
    creationNotice = null;
    try {
      const snapshot = coolingModes(
        await rpc<unknown>('initialize_cooling_modes', { dev, sensor }),
      );
      if (current()) reconcileCreation(snapshot, false, true);
    } catch (cause) {
      if (!current()) return;
      try {
        const snapshot = coolingModes(await rpc<unknown>('get_cooling_modes', { dev }));
        if (current()) reconcileCreation(snapshot, false, false);
      } catch {
        if (current()) error = cause instanceof ValidationError ? cause : message(cause);
      }
    } finally {
      if (current()) {
        saving = false;
        creating = false;
        await tick();
        if (
          current() &&
          (document.activeElement === focused || document.activeElement === document.body)
        ) {
          const destination = pendingCreate ? readButton : (modeSelect ?? readButton);
          if (destination && !destination.disabled) destination.focus();
        }
      }
    }
  }
  function restoreName() {
    if (
      !chosen ||
      editId !== chosen.id ||
      !factorySupported ||
      disabled ||
      loading ||
      saving ||
      restoring ||
      pendingSave ||
      pendingCreate ||
      !connected
    )
      return;
    editName = factoryLabel;
    editDefaultLabel = editName;
    editUsesDefaultName = true;
    savedSnapshot = null;
    loadedProfile = null;
    error = null;
    restoreNotice = 'name';
  }
  async function restoreMode() {
    if (
      !chosen ||
      !library ||
      editId !== chosen.id ||
      !factorySupported ||
      disabled ||
      loading ||
      saving ||
      restoring ||
      pendingSave ||
      pendingCreate ||
      !connected ||
      (factoryNeedsSource && !factorySourceKnown)
    )
      return;
    const started = ++generation,
      dev = device,
      channel = target,
      id = chosen.id,
      sensor = defaultSensor();
    const stamp = draftStamp(),
      context = draftContext(),
      name = editName,
      usesDefault = editUsesDefaultName;
    const current = () =>
      started === generation && device === dev && target === channel && connected;
    restoring = true;
    error = null;
    errorContext = 'restore';
    restoreNotice = null;
    try {
      const value = await coolingModeDefault(channel, id, sensor);
      if (!current()) return;
      let profile: DeviceCoolingMode | undefined;
      try {
        const bank = coolingModes({
          version: 1,
          revision: '0'.repeat(64),
          fan: channel === 'fan' ? [value] : [],
          pump: channel === 'pump' ? [value] : [],
          persistence: { pending: false, saved: false, last_error: null },
        });
        profile = bank?.[channel][0];
      } catch {
        throw new ValidationError(
          'The app returned an invalid factory mode. Your draft is preserved.',
        );
      }
      if (!profile || profile.id !== id || !profile.use_default_name)
        throw new ValidationError(
          'The app returned an invalid factory mode. Your draft is preserved.',
        );
      if (
        draftStamp() !== stamp ||
        draftContext() !== context ||
        editName !== name ||
        editUsesDefaultName !== usesDefault
      ) {
        restoreNotice = 'changed';
        return;
      }
      onrestore(profile.policy);
      editName = coolingModeLabel(profile);
      editDefaultLabel = editName;
      editUsesDefaultName = true;
      savedSnapshot = null;
      loadedProfile = null;
      restoreNotice = 'profile';
    } catch (cause) {
      if (current()) error = cause instanceof ValidationError ? cause : message(cause);
    } finally {
      if (current()) restoring = false;
    }
  }
  async function save() {
    if (
      !chosen ||
      !library ||
      editId !== chosen.id ||
      pendingSave ||
      pendingCreate ||
      disabled ||
      loading ||
      saving ||
      restoring ||
      !connected
    )
      return;
    error = null;
    errorContext = 'save';
    let profile: DeviceCoolingMode;
    try {
      if (
        /^\p{White_Space}*$/u.test(editName) ||
        new TextEncoder().encode(editName).length > 128 ||
        /\p{Cc}/u.test(editName)
      )
        throw new ValidationError('Check the mode name: use a short name with valid characters.');
      const factory = editUsesDefaultName && editName === editDefaultLabel;
      profile = {
        id: chosen.id,
        name: factory ? defaultCoolingModeName(chosen.id)! : editName,
        policy: structuredClone(draft()),
        ...(factory ? { use_default_name: true } : {}),
      };
    } catch (cause) {
      error = cause instanceof ValidationError ? cause : message(cause);
      return;
    }
    const attempt: SaveAttempt = {
      dev: device,
      target,
      revision: library.revision,
      profile,
      context: draftContext(),
    };
    const started = ++generation;
    const current = () =>
      started === generation && device === attempt.dev && target === attempt.target && connected;
    saving = true;
    restoreNotice = null;
    pendingSave = attempt;
    savedSnapshot = null;
    loadedProfile = null;
    try {
      const snapshot = coolingModes(
        await rpc<unknown>('save_cooling_mode', {
          dev: attempt.dev,
          target: attempt.target,
          expected_revision: attempt.revision,
          profile,
        }),
      );
      if (current()) reconcile(snapshot, attempt, false);
    } catch (cause) {
      if (!current()) return;
      try {
        const snapshot = coolingModes(
          await rpc<unknown>('get_cooling_modes', { dev: attempt.dev }),
        );
        if (current()) reconcile(snapshot, attempt, false);
      } catch {
        if (current()) error = cause instanceof ValidationError ? cause : message(cause);
      }
    } finally {
      if (current()) {
        saving = false;
        if (pendingSave) {
          await tick();
          if (current()) readButton?.focus();
        }
      }
    }
  }
</script>

<details class="device-modes">
  <summary>{t('Saved device modes')}</summary>
  <p>{t('Load a mode into the draft to review or edit it before Apply.')}</p>
  <div class="actions">
    <button
      bind:this={readButton}
      disabled={disabled || saving || restoring || !connected || !device}
      aria-disabled={loading}
      aria-busy={loading}
      onclick={refresh}>{loading ? t('Loading…') : t('Read device modes')}</button
    >
    {#if loaded && modes.length}
      <label
        >{t('Saved mode')}<select
          bind:this={modeSelect}
          value={selected}
          onchange={(e) => choose(e.currentTarget.value)}
          disabled={disabled ||
            loading ||
            saving ||
            restoring ||
            !!pendingSave ||
            pendingCreate ||
            !connected}
        >
          <option value="">{t('Choose a mode')}</option>
          {#each modes as mode (mode.id)}<option value={mode.id}
              >{coolingModeLabel(mode)}{mode.id !== mode.name ? ` (${mode.id})` : ''}</option
            >{/each}
        </select></label
      >
      <button
        disabled={disabled ||
          loading ||
          saving ||
          restoring ||
          !!pendingSave ||
          pendingCreate ||
          !connected ||
          !chosen}
        onclick={load}>{t('Load mode into draft')}</button
      >
    {/if}
  </div>
  {#if loaded && library === null}
    <p>
      {t(
        'Create the standard 8 fan and 3 pump modes. Choose a temperature source explicitly; running settings will stay unchanged.',
      )}
    </p>
    <div class="actions">
      <label
        >{t('Source for standard modes')}<select
          bind:value={createSensor}
          disabled={disabled || loading || saving || pendingCreate || !connected}
        >
          <option value="">{t('Choose a temperature source')}</option>
          {#each temperatures as sensor (sensor.id)}<option value={sensor.id}
              >{sensor.label} · {sensor.status}</option
            >{/each}
        </select></label
      >
      <button
        disabled={disabled ||
          loading ||
          !connected ||
          (pendingCreate && !creating) ||
          !temperatures.some((sensor) => sensor.id === createSensor)}
        aria-disabled={creating}
        aria-busy={creating}
        onclick={initialize}>{creating ? t('Creating…') : t('Create standard modes')}</button
      >
    </div>
  {/if}
  {#if creationNotice}<p role="status">
      {creationNotice === 'created'
        ? t(
            'Standard modes were saved. Running settings are unchanged; choose a mode and load it into the draft.',
          )
        : t('Stored modes were found. Review the list before loading; no second write was sent.')}
    </p>{/if}
  {#if chosen && editId === chosen.id}
    <div class="actions edit">
      <label
        >{t('Mode name')}<input
          bind:value={editName}
          oninput={() => {
            editUsesDefaultName = false;
          }}
          disabled={disabled || loading || saving || !!pendingSave || !connected}
        /></label
      >
      <button
        disabled={disabled || loading || restoring || !connected || (!!pendingSave && !saving)}
        aria-disabled={saving}
        aria-busy={saving}
        onclick={save}
        >{saving
          ? t('Saving…')
          : conflict
            ? t('Overwrite updated mode with draft')
            : t('Save mode snapshot')}</button
      >
    </div>
    {#if factorySupported}
      <div class="actions restore">
        <button
          disabled={disabled ||
            loading ||
            saving ||
            restoring ||
            !!pendingSave ||
            pendingCreate ||
            !connected ||
            (editUsesDefaultName && editName === factoryLabel)}
          onclick={restoreName}>{t('Restore default name')}</button
        >
        <button
          disabled={disabled ||
            loading ||
            saving ||
            !!pendingSave ||
            pendingCreate ||
            !connected ||
            (factoryNeedsSource && !factorySourceKnown)}
          aria-disabled={restoring}
          aria-busy={restoring}
          onclick={restoreMode}
          >{restoring ? t('Preparing…') : t('Load factory settings into draft')}</button
        >
      </div>
      {#if factoryNeedsSource && !factorySourceKnown}<p>
          {t('Choose a temperature source in the editor to restore the factory curve.')}
        </p>{/if}
    {/if}
    <p>
      {t(
        'Save updates the name and stored draft snapshot. The running cooling policy stays unchanged.',
      )}
    </p>
    {#if conflict}<p>
        {t('Stored data changed. Load the latest mode or explicitly overwrite it with your draft.')}
      </p>{/if}
  {/if}
  {#if loaded && !modes.length}<p role="status">
      {t('No saved modes for this device yet. Use presets or user profiles.')}
    </p>{/if}
  {#if loaded && modes.length && !loadedProfile && !savedSnapshot}<p role="status">
      {t('{0} saved modes available.', modes.length)}
    </p>{/if}
  {#if error}<p class="error" role="alert">
      {errorContext === 'restore'
        ? t('Could not prepare factory mode: {0}. Your draft is preserved.', diagnosticText(error))
        : errorContext === 'create'
          ? t('Mode creation is unconfirmed: {0}', diagnosticText(error))
          : errorContext === 'save'
            ? t('Mode save is unconfirmed: {0}', diagnosticText(error))
            : t('Could not read cooling modes: {0}', diagnosticText(error))}
    </p>{/if}
  {#if restoreNotice}<p role="status">
      {restoreNotice === 'name'
        ? t('The default name was restored in the draft. Select Save to store it.')
        : restoreNotice === 'profile'
          ? t(
              'Factory settings were loaded into the draft. Save stores them in the mode; Apply sends the current draft.',
            )
          : t('The draft changed. Factory settings did not replace your edits.')}
    </p>{/if}
  {#if savedSnapshot}<p role="status">
      {draftChanged
        ? t(
            'Mode snapshot “{0}” was saved. Your current draft has further edits.',
            coolingModeLabel(savedSnapshot),
          )
        : t(
            'Mode snapshot “{0}” was saved. The running policy is unchanged.',
            coolingModeLabel(savedSnapshot),
          )}
    </p>{/if}
  {#if loadedProfile}<p role="status">
      {t(
        'Mode “{0}” loaded into the draft. Select “Apply cooling” to send it.',
        coolingModeLabel(loadedProfile),
      )}
    </p>{/if}
</details>

<style>
  .device-modes {
    border-block: 1px solid var(--border);
    padding: 14px 0;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  p {
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-muted);
    max-width: 65ch;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: end;
  }
  .edit,
  .restore {
    margin-top: 16px;
  }
  label {
    display: grid;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
  }
  select {
    max-width: 100%;
  }
  button[aria-disabled='true'] {
    color: var(--text-muted);
    cursor: progress;
  }
  .error {
    color: var(--danger);
    overflow-wrap: anywhere;
  }
</style>
