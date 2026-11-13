<script lang="ts">
  import { t } from './i18n.svelte';
  import {
    rpc,
    message,
    type SensorDefaults,
    type SensorDefaultsState,
    type DefaultSensorRole,
    type CpuSourceField,
    type GpuSourceField,
  } from './api';
  import { presentation } from './presentation.svelte';
  import { formatSensor } from './theme-presets';
  let { connected, active }: { connected: boolean; active: boolean } = $props();
  const fields: { key: CpuSourceField; label: string }[] = [
    { key: 'temperature', label: 'Temperature' },
    { key: 'clock', label: 'Frequency' },
    { key: 'load', label: 'Load' },
    { key: 'power', label: 'Power' },
    { key: 'fan', label: 'Fan' },
    { key: 'voltage', label: 'Voltage' },
  ];
  const gpuFields: { key: GpuSourceField; label: string }[] = [
    ...fields,
    { key: 'memory_used', label: 'VRAM usage' },
  ];
  let snapshot = $state<SensorDefaultsState | null>(null),
    draft = $state<SensorDefaults | null>(null);
  let fresh = $state(false),
    mustReload = $state(false);
  let busy = $state(false),
    error = $state(''),
    readError = $state(''),
    notice = $state('');
  let generation = 0,
    readRevision = 0;
  const gpuIds = $derived([
    ...new Set(snapshot?.candidates.flatMap((c) => (c.gpu_id ? [c.gpu_id] : [])) ?? []),
  ]);
  const conflict = $derived(!!draft && !!snapshot && draft.revision !== snapshot.config.revision);
  const dirty = $derived(
    !!draft && !!snapshot && JSON.stringify(draft) !== JSON.stringify(snapshot.config),
  );
  function choices(role: DefaultSensorRole, gpu: string | null = null) {
    return snapshot?.candidates.filter((c) => c.role === role && c.gpu_id === gpu) ?? [];
  }
  function copy(config: SensorDefaults): SensorDefaults {
    return JSON.parse(JSON.stringify(config));
  }
  function chooseGpu(id: string) {
    if (!draft) return;
    draft.gpu = id
      ? {
          device_id: id,
          temperature: null,
          clock: null,
          load: null,
          power: null,
          fan: null,
          voltage: null,
          memory_used: null,
        }
      : null;
  }
  function clearDraft() {
    if (!draft) return;
    draft.cpu = {
      temperature: null,
      clock: null,
      load: null,
      power: null,
      fan: null,
      voltage: null,
    };
    draft.gpu = null;
    notice = 'Draft selection cleared. Select Apply sources to save it.';
  }
  $effect(() => {
    const online = connected,
      visible = active,
      epoch = ++generation;
    fresh = false;
    if (!online || !visible) return;
    let alive = true,
      polling = false;
    async function poll() {
      if (polling || busy) return;
      polling = true;
      const request = ++readRevision;
      try {
        const next = await rpc<SensorDefaultsState>('get_sensor_defaults');
        if (!alive || epoch !== generation || request !== readRevision) return;
        snapshot = next;
        fresh = true;
        readError = '';
        if (!draft) draft = copy(next.config);
      } catch (e) {
        if (alive && epoch === generation && request === readRevision) {
          fresh = false;
          readError = message(e);
        }
      } finally {
        polling = false;
      }
    }
    // Defer the initial async call so its snapshot reads do not become effect dependencies.
    const first = setTimeout(() => void poll(), 0);
    const timer = setInterval(() => void poll(), 1000);
    return () => {
      alive = false;
      clearTimeout(first);
      clearInterval(timer);
    };
  });
  async function apply() {
    if (!draft || busy || !connected || !fresh || conflict || readError || mustReload) return;
    const epoch = generation,
      config = copy(draft);
    readRevision++;
    fresh = false;
    busy = true;
    error = '';
    notice = '';
    try {
      const saved = await rpc<SensorDefaults>('set_sensor_defaults', { config });
      if (epoch !== generation) return;
      draft = copy(saved);
      if (snapshot) snapshot.config = copy(saved);
      notice = 'Sources saved. Explicit LCD, cooling and RGB bindings were preserved.';
    } catch (e) {
      if (epoch === generation) {
        error = message(e);
        mustReload = true;
      }
    } finally {
      busy = false;
    }
  }
</script>

<section aria-label={t('System sensor sources')}>
  <h2>{t('System sensor sources')}</h2>
  <p>
    {t(
      'Select CPU sources and a graphics card for system readings. Unsupported readings remain unavailable.',
    )}
  </p>
  {#if readError}<p role="alert">
      {t('Could not read settings:')} <span>{readError}</span>
    </p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if mustReload}<p role="alert">
      {t('Check the saved selection before trying again.')}
    </p>{/if}
  {#if notice}<p role="status">{t(notice)}</p>{/if}
  {#if conflict}<p role="alert">
      {t(
        'Settings changed elsewhere. Your draft was preserved; reload the current selection before applying.',
      )}
    </p>{/if}
  {#if draft}
    <p>
      {dirty
        ? '' + t('There are unsaved changes.') + ''
        : '' + t('Showing the saved selection.') + ''}
    </p>
    <fieldset disabled={busy}>
      <legend>CPU</legend>
      <p id="cpu-fan-help">
        {t(
          'Assign the CPU fan manually to the channel it is connected to. A sensor label does not confirm wiring; the recommended selection leaves this field empty.',
        )}
      </p>
      {#each fields as field (field.key)}
        {@const role = `cpu_${field.key}` as DefaultSensorRole}
        {@const available = choices(role)}
        <label
          >CPU · {t(field.label)}<select
            aria-describedby={field.key === 'fan' ? 'cpu-fan-help' : undefined}
            value={draft.cpu[field.key] ?? ''}
            onchange={(e) => {
              if (draft) draft.cpu[field.key] = e.currentTarget.value || null;
            }}
          >
            <option value="">{t('Not selected')}</option>
            {#if draft.cpu[field.key] && !available.some((c) => c.reading.id === draft?.cpu[field.key])}<option
                value={draft.cpu[field.key]}>{t('Unavailable:')} {draft.cpu[field.key]}</option
              >{/if}
            {#each available as candidate (candidate.reading.id)}<option
                value={candidate.reading.id}
                >{candidate.reading.label} · {candidate.reading.id}</option
              >{/each}
          </select></label
        >
      {/each}
    </fieldset>
    <fieldset disabled={busy}>
      <legend>GPU</legend>
      <label
        >{t('System graphics card')}<select
          value={draft.gpu?.device_id ?? ''}
          onchange={(e) => chooseGpu(e.currentTarget.value)}
        >
          <option value="">{t('No selection')}</option>
          {#if draft.gpu && !gpuIds.includes(draft.gpu.device_id)}<option
              value={draft.gpu.device_id}
              >{t('Currently unavailable:')} {draft.gpu.device_id}</option
            >{/if}
          {#each gpuIds as id (id)}<option value={id}>{id}</option>{/each}
        </select></label
      >
      {#if draft.gpu}{#each gpuFields as field (field.key)}
          {@const role = `gpu_${field.key}` as DefaultSensorRole}
          {@const available = choices(role, draft.gpu.device_id)}
          <label
            >GPU · {t(field.label)}<select
              value={draft.gpu[field.key] ?? ''}
              onchange={(e) => {
                if (draft?.gpu) draft.gpu[field.key] = e.currentTarget.value || null;
              }}
            >
              <option value="">{t('Not selected')}</option>
              {#if draft.gpu[field.key] && !available.some((c) => c.reading.id === draft?.gpu?.[field.key])}<option
                  value={draft.gpu[field.key]}>{t('Unavailable:')} {draft.gpu[field.key]}</option
                >{/if}
              {#each available as candidate (candidate.reading.id)}<option
                  value={candidate.reading.id}
                  >{candidate.reading.label} · {candidate.reading.id}</option
                >{/each}
            </select></label
          >
        {/each}{/if}
    </fieldset>
    <div class="actions">
      <button
        disabled={busy || !connected || !fresh || !!readError || conflict || mustReload}
        onclick={() => void apply()}>{t('Apply sources')}</button
      >
      <button
        disabled={busy || !snapshot || !connected || !fresh || !!readError}
        onclick={() => {
          if (snapshot) {
            draft = copy(snapshot.config);
            error = '';
            notice = '';
            mustReload = false;
          }
        }}>{t('Reload saved selection')}</button
      >
      <button
        disabled={busy || !connected || !fresh || !snapshot?.recommended}
        onclick={() => {
          if (snapshot?.recommended) {
            draft = copy(snapshot.recommended);
            error = '';
            notice =
              'Recommended sources loaded into the draft. Review the selection and select Apply to save.';
          }
        }}>{t('Restore recommended selection')}</button
      >
      <button disabled={busy} onclick={clearDraft}>{t('Clear draft selection')}</button>
    </div>
  {:else}<p>{t('Loading source settings…')}</p>{/if}
  {#if snapshot}<h3>{t('Saved system readings')}</h3>
    <dl>
      {#each snapshot.values as value (value.role)}<div>
          <dt>{value.role}</dt>
          <dd>
            {connected && fresh && !readError && value.reading
              ? formatSensor(value.reading, presentation.settings?.temperature_unit)
              : '—'}
          </dd>
        </div>{/each}
    </dl>{/if}
</section>

<style>
  section {
    margin-bottom: 24px;
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px;
  }
  fieldset {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(210px, 100%), 1fr));
    gap: 14px;
    margin: 16px 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
    min-width: 0;
  }
  legend {
    padding: 0 6px;
  }
  .actions {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  p {
    line-height: 1.6;
    color: var(--text-muted);
  }
  [role='alert'] {
    color: var(--danger);
  }
  [role='status'] {
    color: var(--success);
  }
  dl {
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 6px 0;
    font-size: 20px;
  }
</style>
