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
    { key: 'temperature', label: 'Температура' },
    { key: 'clock', label: 'Частота' },
    { key: 'load', label: 'Загрузка' },
    { key: 'power', label: 'Мощность' },
    { key: 'fan', label: 'Вентилятор' },
    { key: 'voltage', label: 'Напряжение' },
  ];
  const gpuFields: { key: GpuSourceField; label: string }[] = [
    ...fields,
    { key: 'memory_used', label: 'Использование VRAM' },
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
    notice = 'Выбор очищен в черновике. Для сохранения нажмите «Применить источники».';
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
      notice = 'Источники сохранены. Явные привязки LCD, охлаждения и RGB не изменены.';
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

<section aria-label={t('Источники системных датчиков')}>
  <h2>{t('Источники системных датчиков')}</h2>
  <p>
    {t(
      'Выберите источники CPU и видеокарту для общесистемных показателей. Неподдерживаемые показатели останутся без значения.',
    )}
  </p>
  {#if readError}<p role="alert">
      {t('Не удалось прочитать настройки:')} <span>{readError}</span>
    </p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if mustReload}<p role="alert">
      {t('Проверьте сохранённый выбор перед повторной попыткой.')}
    </p>{/if}
  {#if notice}<p role="status">{t(notice)}</p>{/if}
  {#if conflict}<p role="alert">
      {t(
        'Настройки изменились извне. Ваш черновик сохранён; загрузите актуальный выбор перед применением.',
      )}
    </p>{/if}
  {#if draft}
    <p>
      {dirty
        ? '' + t('Есть несохранённые изменения.') + ''
        : '' + t('Показан сохранённый выбор.') + ''}
    </p>
    <fieldset disabled={busy}>
      <legend>CPU</legend>
      <p id="cpu-fan-help">
        {t(
          'Вентилятор CPU назначается вручную: выберите канал, к которому он подключён. Название датчика не подтверждает подключение; рекомендуемый выбор оставляет это поле пустым.',
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
            <option value="">{t('Не выбран')}</option>
            {#if draft.cpu[field.key] && !available.some((c) => c.reading.id === draft?.cpu[field.key])}<option
                value={draft.cpu[field.key]}>{t('Недоступен:')} {draft.cpu[field.key]}</option
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
        >{t('Системная видеокарта')}<select
          value={draft.gpu?.device_id ?? ''}
          onchange={(e) => chooseGpu(e.currentTarget.value)}
        >
          <option value="">{t('Не выбрана')}</option>
          {#if draft.gpu && !gpuIds.includes(draft.gpu.device_id)}<option
              value={draft.gpu.device_id}>{t('Недоступна:')} {draft.gpu.device_id}</option
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
              <option value="">{t('Не выбран')}</option>
              {#if draft.gpu[field.key] && !available.some((c) => c.reading.id === draft?.gpu?.[field.key])}<option
                  value={draft.gpu[field.key]}>{t('Недоступен:')} {draft.gpu[field.key]}</option
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
        onclick={() => void apply()}>{t('Применить источники')}</button
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
        }}>{t('Загрузить сохранённый выбор')}</button
      >
      <button
        disabled={busy || !connected || !fresh || !snapshot?.recommended}
        onclick={() => {
          if (snapshot?.recommended) {
            draft = copy(snapshot.recommended);
            error = '';
            notice =
              'Рекомендуемые источники загружены в черновик. Проверьте выбор и нажмите Apply для сохранения.';
          }
        }}>{t('Восстановить рекомендуемый выбор')}</button
      >
      <button disabled={busy} onclick={clearDraft}>{t('Очистить выбор в черновике')}</button>
    </div>
  {:else}<p>{t('Загрузка настроек источников…')}</p>{/if}
  {#if snapshot}<h3>{t('Сохранённые системные показатели')}</h3>
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
