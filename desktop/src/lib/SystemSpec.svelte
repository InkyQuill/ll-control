<script lang="ts">
  import { t } from './i18n.svelte';
  import { presentation } from './presentation.svelte';
  import { untrack } from 'svelte';
  import { rpc, message, type SystemSpec, type SpecKind, type SpecValue } from './api';
  let { connected }: { connected: boolean } = $props();
  const kinds: { kind: SpecKind; label: string }[] = [
    { kind: 'cpu', label: 'CPU' },
    { kind: 'gpu', label: 'GPU' },
    { kind: 'memory', label: 'Memory' },
    { kind: 'monitors', label: 'Monitors' },
    { kind: 'motherboard', label: 'Motherboard' },
    { kind: 'storage', label: 'Storage' },
  ];
  const labels: Record<string, string> = {
    model: 'Model',
    vendor: 'Manufacturer',
    manufacturer: 'Manufacturer',
    socket: 'Socket / package',
    logical_threads: 'Logical threads',
    physical_cores: 'Physical cores',
    vendor_id: 'PCI Vendor ID',
    device_id: 'PCI Device ID',
    driver: 'Driver',
    revision: 'Revision',
    os_visible_capacity: 'OS-visible memory',
    reported_modules: 'Accessible module records',
    part_number: 'Module part number',
    locator: 'Location',
    bank: 'Bank',
    memory_type: 'Memory type',
    capacity: 'Capacity',
    rated_transfer_rate: 'Rated transfer rate',
    configured_transfer_rate: 'Configured transfer rate',
    connector: 'Connector',
    product_id: 'Product ID',
    physical_size: 'Physical size',
    detailed_timing: 'First EDID detailed timing',
    version: 'Version',
    bios_vendor: 'BIOS vendor',
    bios_version: 'BIOS version',
    bios_date: 'BIOS date',
    device: 'Device',
    firmware: 'Firmware',
    media: 'Media type',
  };
  let snapshot = $state<SystemSpec | null>(null),
    kind = $state<SpecKind>('cpu'),
    selectedId = $state('');
  type SpecError = { kind: 'unsupported' } | { kind: 'backend'; diagnostic: string };
  let busy = $state(false),
    error = $state<SpecError | null>(null),
    fresh = $state(false);
  let generation = 0;
  const group = $derived(snapshot?.groups.find((g) => g.kind === kind));
  const item = $derived(group?.items.find((i) => i.id === selectedId) ?? group?.items[0]);
  function format(value: SpecValue): string {
    if (value.kind === 'text') return value.value;
    if (value.kind === 'count') return value.value.toLocaleString(presentation.language);
    if (value.kind === 'hertz') return `${value.value.toLocaleString(presentation.language)} Hz`;
    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
    const index =
      value.value > 0
        ? Math.min(Math.floor(Math.log(value.value) / Math.log(1024)), units.length - 1)
        : 0;
    return `${(value.value / 1024 ** index).toLocaleString(presentation.language, { maximumFractionDigits: 2 })} ${units[index]}`;
  }
  async function load(refresh: boolean) {
    const request = ++generation;
    busy = true;
    error = null;
    try {
      const next = await rpc<SystemSpec>('get_system_spec', { refresh });
      if (request !== generation) return;
      if (
        !next ||
        next.version !== 1 ||
        next.groups.length !== 6 ||
        kinds.some((k) => next.groups.filter((g) => g.kind === k.kind).length !== 1)
      ) {
        error = { kind: 'unsupported' };
        fresh = false;
        return;
      }
      snapshot = next;
      fresh = true;
    } catch (e) {
      if (request === generation) {
        error = { kind: 'backend', diagnostic: message(e) };
        fresh = false;
      }
    } finally {
      if (request === generation) busy = false;
    }
  }
  $effect(() => {
    const online = connected;
    generation++;
    fresh = false;
    busy = false;
    if (!online) return;
    const timer = setTimeout(() => {
      void untrack(() => load(false));
    }, 0);
    return () => {
      clearTimeout(timer);
      generation++;
    };
  });
</script>

<section aria-label={t('System information')} aria-busy={busy}>
  <div class="heading">
    <h2>{t('System information')}</h2>
    <button disabled={!connected || busy} onclick={() => load(true)}
      >{t('Refresh information')}</button
    >
  </div>
  <p>{t('Static hardware information. No pump connection is required.')}</p>
  {#if !connected}<p role="status">
      {t('The daemon is disconnected. Retained system information may be out of date.')}
    </p>{/if}
  {#if error}<p role="alert">
      {t('Could not read system information:')}
      {error.kind === 'unsupported'
        ? t('Unsupported system information response')
        : error.diagnostic}{t('. Retry the refresh.')}
    </p>{/if}
  {#if snapshot && !fresh && connected}<p role="status">
      {t('Showing previous information; its freshness is unconfirmed.')}
    </p>{/if}
  {#if busy}<p role="status">{t('Reading hardware information…')}</p>{/if}
  <div class="groups" role="group" aria-label={t('Hardware categories')}>
    {#each kinds as category (category.kind)}<button
        aria-pressed={kind === category.kind}
        onclick={() => {
          kind = category.kind;
          selectedId = '';
        }}>{t(category.label)}</button
      >{/each}
  </div>
  {#if snapshot}
    <p class="updated">
      {t('Collected:')}
      {new Date(snapshot.collected_at_unix_ms).toLocaleString(presentation.language)}
    </p>
    {#if !group?.items.length}<p role="status">
        {t('Information in this category is unavailable.')}
      </p>
    {:else}
      <label
        >{t('Component')}<select
          value={item?.id}
          onchange={(event) => (selectedId = event.currentTarget.value)}
          >{#each group.items as entry (entry.id)}<option value={entry.id}>{entry.title}</option
            >{/each}</select
        ></label
      >
      {#if item}<h3>{item.title}</h3>
        <dl>
          {#each item.fields as field (field.key)}<div>
              <dt>{Object.hasOwn(labels, field.key) ? t(labels[field.key]) : field.key}</dt>
              <dd>
                <span
                  >{field.value === null ? '' + t('Not available') + '' : format(field.value)}</span
                ><small>{t('Source:')} {field.source}</small>
              </dd>
            </div>{/each}
        </dl>{/if}
      {#if kind === 'memory'}<p>
          {t(
            'OS-visible memory differs from the total installed capacity. Module details are shown only when SMBIOS is accessible.',
          )}
        </p>{/if}
    {/if}
  {:else if !busy && connected && !error}<p>
      {t('System information has not been loaded yet.')}
    </p>{/if}
</section>

<style>
  section {
    max-width: 900px;
  }
  .heading {
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
  }
  .heading h2 {
    margin: 0;
  }
  .groups {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 24px 0;
  }
  button[aria-pressed='true'] {
    background: var(--accent);
    border-color: var(--accent);
  }
  .updated,
  small {
    color: var(--text-muted);
    font-size: 12px;
  }
  label {
    max-width: 580px;
  }
  dl {
    margin: 20px 0;
  }
  dl > div {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) minmax(0, 2fr);
    gap: 18px;
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  small {
    display: block;
    margin-top: 5px;
  }
  p {
    line-height: 1.6;
  }
</style>
