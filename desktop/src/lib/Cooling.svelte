<script lang="ts">
  import { t } from './i18n.svelte';
  import TemperatureInput from './TemperatureInput.svelte';
  import { presentation } from './presentation.svelte';
  import { temperatureSuffix } from './temperature';
  const displayUnit = $derived(presentation.settings?.temperature_unit ?? 'celsius');
  const displaySuffix = $derived(temperatureSuffix(displayUnit));
  import { tick } from 'svelte';
  import Profiles from './Profiles.svelte';
  import CoolingModes from './CoolingModes.svelte';
  import { coolingPolicyKey, type CoolingModeReference } from './cooling-modes';
  let loadedMode = $state.raw<CoolingModeReference | null>(null);
  let draftSession = 0;
  const matchesStored = $derived.by(() => {
    try {
      return !!loadedMode && coolingPolicyKey(policy()) === coolingPolicyKey(loadedMode.policy);
    } catch {
      return false;
    }
  });
  import { ValidationError } from './validation-error';
  import {
    rpc,
    message,
    coolingPreset,
    type Sensor,
    type CoolingPolicy,
    type CoolingState,
  } from './api';
  let { device, sensors, connected }: { device: string; sensors: Sensor[]; connected: boolean } =
    $props();
  let target = $state<'pump' | 'fan'>('pump');
  let mode = $state<CoolingPolicy['mode']>('fixed');
  let percent = $state(80),
    rpm = $state(2200),
    sensorId = $state(''),
    hysteresis = $state(2),
    fallback = $state(100);
  let editor: HTMLElement | undefined;
  let points = $state([
    { celsius: 30, value: 40 },
    { celsius: 70, value: 100 },
  ]);
  type Failure = { context: 'preset' | 'apply'; detail: string };
  let cooling = $state<CoolingState | null>(null),
    error = $state<Failure | ValidationError | null>(null),
    notice = $state(''),
    busy = $state(false);
  function errorText(failure: Failure | ValidationError): string {
    if (failure instanceof ValidationError) return t(failure.resource, ...failure.values);
    return failure.context === 'preset'
      ? t('Could not load the cooling preset: {0}. The draft was preserved.', failure.detail)
      : t(
          'Cooling application was not confirmed: {0}. Check the daemon state before applying again.',
          failure.detail,
        );
  }
  let revision = 0,
    generation = 0;
  $effect(() => {
    void device;
    void target;
    loadedMode = null;
  });
  let pollError = $state('');
  let current = $derived(cooling?.[target]);
  let curve = $derived(['curve', 'fan_curve', 'curve_rpm'].includes(mode));
  let rpmMode = $derived(mode === 'fixed_rpm' || mode === 'curve_rpm');
  let minDuty = $derived(target === 'pump' ? 20 : 10);
  let fixedValue = $derived(rpmMode ? rpm : percent);
  let fixedMin = $derived(rpmMode ? 800 : minDuty);
  let fixedMax = $derived(rpmMode ? 3600 : 100);
  let fixedInvalid = $derived(
    !Number.isInteger(fixedValue) || fixedValue < fixedMin || fixedValue > fixedMax,
  );
  let temperatures = $derived(sensors.filter((s) => s.unit === 'celsius'));
  let graph = $derived(
    points
      .map(
        (p) =>
          `${(Math.max(-50, Math.min(150, p.celsius)) + 50) * 1.5},${120 - Math.max(0, Math.min(1, p.value / (rpmMode ? 3600 : 100))) * 120}`,
      )
      .join(' '),
  );
  $effect(() => {
    const dev = device,
      online = connected;
    generation++;
    revision++;
    cooling = null;
    pollError = '';
    notice = '';
    error = null;
    if (!dev || !online) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      const started = revision;
      try {
        if (!busy) {
          const result = await rpc<CoolingState>('get_cooling_state', { dev });
          if (!cancelled && started === revision) {
            cooling = result;
            pollError = '';
          }
        }
      } catch (e) {
        if (!cancelled && started === revision) {
          cooling = null;
          pollError = message(e);
        }
      }
      if (!cancelled) timer = setTimeout(refresh, 1500);
    }
    timer = setTimeout(refresh, 0);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });
  async function removePoint(index: number) {
    if (points.length <= 2 || index < 0 || index >= points.length) return;
    const draft = points,
      selected = target,
      selectedMode = mode,
      owner = editor,
      started = generation;
    points.splice(index, 1);
    await tick();
    if (
      !owner?.isConnected ||
      owner !== editor ||
      started !== generation ||
      draft !== points ||
      selected !== target ||
      selectedMode !== mode ||
      !curve
    )
      return;
    const fields = owner.querySelectorAll<HTMLInputElement>('[data-curve-temperature]');
    fields[Math.min(index, fields.length - 1)]?.focus();
  }
  function load(policy: CoolingPolicy) {
    draftSession++;
    loadedMode = null;
    mode = policy.mode;
    if ('percent' in policy) percent = policy.percent;
    if ('rpm' in policy) rpm = policy.rpm;
    if ('sensor_id' in policy) {
      sensorId = policy.sensor_id;
      hysteresis = policy.hysteresis_c;
      fallback = policy.fallback_percent;
      points = policy.points.map((p) => ({
        celsius: p.celsius,
        value: 'rpm' in p ? p.rpm : p.percent,
      }));
    }
  }
  function draftKey() {
    return JSON.stringify({ mode, percent, rpm, sensorId, hysteresis, fallback, points });
  }
  async function preset(name: string) {
    const started = generation,
      draft = draftKey();
    busy = true;
    error = null;
    notice = '';
    try {
      const result = await coolingPreset(target, name, sensorId);
      if (started !== generation) return;
      if (draft !== draftKey()) {
        notice = 'The draft changed. The preset did not replace your edits.';
        return;
      }
      load(result);
      notice = 'Preset loaded into the editor. Select Apply cooling to send it.';
    } catch (e) {
      if (started === generation) error = { context: 'preset', detail: message(e) };
    } finally {
      busy = false;
    }
  }
  function changeMode() {
    points =
      mode === 'curve_rpm'
        ? [
            { celsius: 30, value: 2200 },
            { celsius: 70, value: 3600 },
          ]
        : [
            { celsius: 30, value: 40 },
            { celsius: 70, value: 100 },
          ];
  }
  function policy(): CoolingPolicy {
    if (!curve && fixedInvalid)
      throw new ValidationError('Enter an integer from {0} to {1}', [fixedMin, fixedMax]);
    if (mode === 'fixed' || mode === 'motherboard') return { mode, percent };
    if (mode === 'fixed_rpm') return { mode, rpm };
    if (!sensorId) throw new ValidationError('Select a temperature sensor');
    if (!Number.isFinite(hysteresis) || hysteresis < 0 || hysteresis > 10)
      throw new ValidationError('Hysteresis must be between 0 and 10 °C');
    if (!Number.isInteger(fallback) || fallback < minDuty || fallback > 100)
      throw new ValidationError('Fallback PWM must be an integer from {0} to 100%', [minDuty]);
    if (
      points.some(
        (p) =>
          p.celsius < -50 ||
          p.celsius > 150 ||
          (mode !== 'curve' && !Number.isInteger(p.celsius)) ||
          !Number.isInteger(p.value) ||
          p.value < fixedMin ||
          p.value > fixedMax,
      )
    )
      throw new ValidationError('Check the ranges and integer values of curve points');
    if (
      points.length < 2 ||
      points.length > 16 ||
      points.some(
        (p, i) =>
          !Number.isFinite(p.celsius) ||
          !Number.isInteger(p.value) ||
          (i > 0 && (p.celsius <= points[i - 1].celsius || p.value < points[i - 1].value)),
      )
    )
      throw new ValidationError('Temperatures must increase and output must not decrease');
    const shared = { sensor_id: sensorId, hysteresis_c: hysteresis, fallback_percent: fallback };
    return mode === 'curve_rpm'
      ? { mode, ...shared, points: points.map((p) => ({ celsius: p.celsius, rpm: p.value })) }
      : { mode, ...shared, points: points.map((p) => ({ celsius: p.celsius, percent: p.value })) };
  }
  async function apply() {
    busy = true;
    error = null;
    notice = '';
    const dev = device,
      selected = target,
      started = generation;
    revision++;
    try {
      const requested = policy(),
        source =
          loadedMode && coolingPolicyKey(requested) === coolingPolicyKey(loadedMode.policy)
            ? loadedMode
            : null;
      const result = source
        ? await rpc<CoolingState>('apply_cooling_mode', {
            dev,
            target: selected,
            expected_revision: source.revision,
            id: source.id,
          })
        : await rpc<CoolingState>('set_cooling_policy', {
            dev,
            target: selected,
            policy: requested,
          });
      if (started !== generation) return;
      revision++;
      cooling = result;
      if (result[selected].last_error) throw new Error(result[selected].last_error!);
      notice =
        'The daemon accepted the policy. Applied PWM is shown separately; measured RPM is on the Sensors page.';
    } catch (e) {
      if (started === generation)
        error = e instanceof ValidationError ? e : { context: 'apply', detail: message(e) };
    } finally {
      revision++;
      busy = false;
    }
  }
</script>

<section aria-label={t('Cooling')} bind:this={editor}>
  <div class="row">
    <label
      >{t('Channel')}<select
        bind:value={target}
        onchange={() => {
          mode = 'fixed';
          notice = '';
          error = null;
        }}
        disabled={busy}
        ><option value="pump">{t('Pump')}</option><option value="fan">{t('LCDV2 fans')}</option
        ></select
      ></label
    >
    <label
      >{t('Mode')}<select bind:value={mode} onchange={changeMode} disabled={busy}
        ><option value="fixed">{t('Fixed PWM')}</option><option
          value={target === 'fan' ? 'fan_curve' : 'curve'}>{t('Temperature PWM curve')}</option
        >{#if target === 'pump'}<option value="fixed_rpm">{t('Target RPM')}</option><option
            value="curve_rpm">{t('Temperature RPM curve')}</option
          ><option value="motherboard">{t('Motherboard source')}</option>{/if}</select
      ></label
    >
  </div>
  {#key target}<Profiles
      resetSensor={sensorId}
      group={target}
      {connected}
      disabled={busy}
      draft={() => ({ kind: target, value: policy() })}
      onload={(content) => {
        if (content.kind === 'fan' || content.kind === 'pump') load(content.value);
      }}
    />{/key}
  <CoolingModes
    {device}
    {target}
    {connected}
    {sensors}
    disabled={busy}
    draft={policy}
    defaultSensor={() => sensorId}
    draftStamp={draftKey}
    onrestore={(policy) => {
      load(policy);
      error = null;
      notice = '';
    }}
    onload={(policy, reference) => {
      load(policy);
      loadedMode = reference;
      error = null;
      notice = '';
    }}
    draftContext={() => draftSession}
    onsaved={(reference) => {
      loadedMode = reference;
    }}
  />
  {#if loadedMode}<p class="mode-context">
      {matchesStored
        ? t(
            'The draft matches the loaded mode. Apply will check that the stored snapshot is still current.',
          )
        : t(
            'The draft has changed. Apply will use it as custom settings; the stored mode stays unchanged.',
          )}
    </p>{/if}
  <article class="summary">
    <h2>
      {target === 'pump' ? t('Daemon state · pump') : t('Daemon state · fans')}
    </h2>
    <div class="row">
      <strong>{t('Applied PWM: {0}%', current?.applied_percent ?? '—')}</strong><span
        >{current?.motherboard ? 'Motherboard' : 'MCU'}</span
      ><span
        >{current?.fallback_active
          ? '' + t('Fallback active') + ''
          : '' + t('Fallback inactive') + ''}</span
      >
    </div>
    {#if current?.mode_id}<p>
        {t('Requested device mode: {0}', current.mode_id)}
      </p>{/if}{#if current?.last_error}<p role="alert">
        {t('Cooling control error: {0}', current.last_error)}
      </p>{/if}<button
      disabled={!current?.requested || busy}
      onclick={() => {
        if (current?.requested) load(current.requested);
      }}>{t('Load current policy')}</button
    >
  </article>
  {#if target === 'fan'}<p>
      {t(
        'This channel controls fans connected to the LCDV2. Motherboard fans are controlled separately.',
      )}
    </p>{/if}
  <label
    >{t('Temperature source')}<select bind:value={sensorId}
      ><option value="">{t('Select a sensor for the curve or preset')}</option
      >{#each temperatures as sensor (sensor.id)}<option value={sensor.id}
          >{sensor.label} · {sensor.status}</option
        >{/each}{#if sensorId && !temperatures.some((s) => s.id === sensorId)}<option
          value={sensorId}>{t('Unavailable:')} {sensorId}</option
        >{/if}</select
    ></label
  >
  <div class="presets">
    {#each target === 'pump' ? ['quiet', 'performance'] : ['quiet', 'standard', 'high', 'full'] as name (name)}<button
        disabled={!sensorId || busy}
        onclick={() => preset(name)}>{name}</button
      >{/each}
  </div>
  {#if !curve}<label
      >{rpmMode ? '' + t('Target RPM (reference mapping)') + '' : 'PWM, %'}
      <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK exposes number as textbox without Value; native AT-SPI regression verifies this workaround.) -->
      <input
        type="number"
        role="spinbutton"
        min={fixedMin}
        max={fixedMax}
        value={fixedValue}
        aria-valuemin={fixedMin}
        aria-valuemax={fixedMax}
        aria-valuenow={Number.isFinite(fixedValue) ? fixedValue : undefined}
        aria-invalid={fixedInvalid}
        oninput={(e) => {
          if (rpmMode) rpm = e.currentTarget.valueAsNumber;
          else percent = e.currentTarget.valueAsNumber;
        }}
      /></label
    >{:else}
    <svg viewBox="-20 -10 330 155" role="img" aria-label={t('Temperature curve draft')}
      ><path d="M0 0V120H300" stroke="#677c98" fill="none" /><polyline
        points={graph}
        fill="none"
        stroke="var(--accent)"
        stroke-width="3"
      />{#each points as p (p)}<circle
          cx={(Math.max(-50, Math.min(150, p.celsius)) + 50) * 1.5}
          cy={120 - Math.max(0, Math.min(1, p.value / (rpmMode ? 3600 : 100))) * 120}
          r="4"
          fill="var(--accent-text)"
        />{/each}<text x="120" y="140" fill="var(--text-muted)" font-size="10"
        >{t('Temperature')} {displaySuffix}</text
      ></svg
    >
    {#each points as p, i (p)}<div class="row">
        <label
          >{t('Point')}
          {i + 1}: {displaySuffix}
          <TemperatureInput
            curve
            bind:value={p.celsius}
            min={-50}
            max={150}
            step={mode === 'curve' ? 'any' : 1}
            invalid={(mode !== 'curve' && !Number.isInteger(p.celsius)) ||
              (i > 0 && p.celsius <= points[i - 1].celsius)}
          /></label
        ><label
          >{rpmMode ? 'RPM' : 'PWM %'}
          <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface; verified by AT-SPI.) -->
          <input
            type="number"
            role="spinbutton"
            bind:value={p.value}
            min={fixedMin}
            max={fixedMax}
            aria-valuemin={fixedMin}
            aria-valuemax={fixedMax}
            aria-valuenow={Number.isFinite(p.value) ? p.value : undefined}
            aria-invalid={!Number.isInteger(p.value) || p.value < fixedMin || p.value > fixedMax}
          /></label
        ><button
          aria-label={t('Delete point {0}', i + 1)}
          disabled={points.length <= 2}
          onclick={() => removePoint(i)}>×</button
        >
      </div>{/each}
    <button
      disabled={points.length >= 16}
      onclick={() => {
        const last = points.at(-1)!;
        points.push({ celsius: last.celsius + 5, value: last.value });
      }}>{t('Add point')}</button
    >
    <div class="row">
      <label
        >{t('Hysteresis,')}
        {displaySuffix}
        <TemperatureInput bind:value={hysteresis} min={0} max={10} difference /></label
      ><label
        >Fallback PWM, %
        <!-- svelte-ignore a11y_no_redundant_roles (WebKitGTK requires explicit spinbutton for the native Value interface; verified by AT-SPI.) -->
        <input
          type="number"
          role="spinbutton"
          bind:value={fallback}
          min={minDuty}
          max="100"
          aria-valuemin={minDuty}
          aria-valuemax="100"
          aria-valuenow={Number.isFinite(fallback) ? fallback : undefined}
          aria-invalid={!Number.isInteger(fallback) || fallback < minDuty || fallback > 100}
        /></label
      >
    </div>
  {/if}
  {#if pollError}<p role="alert">
      {t('Could not refresh cooling state: {0}', pollError)}
    </p>{/if}
  {#if error}<p class="error" role="alert">{errorText(error)}</p>{/if}{#if notice}<p role="status">
      {t(notice)}
    </p>{/if}
  <button class="apply" disabled={busy || !device || !connected} onclick={apply}
    >{t('Apply cooling')}</button
  >
</section>

<style>
  section {
    max-width: 760px;
    display: grid;
    gap: 18px;
  }
  h2 {
    font-size: 16px;
  }
  .row {
    display: flex;
    gap: 14px;
    align-items: end;
  }
  .summary {
    border: 1px solid var(--border);
    background: var(--surface);
    border-radius: 12px;
    padding: 20px;
  }
  .summary button {
    margin-top: 18px;
  }
  .presets {
    display: flex;
    gap: 10px;
  }
  p {
    line-height: 1.6;
    color: var(--text-muted);
  }
  .mode-context {
    max-width: 65ch;
  }
  .error {
    color: var(--danger);
  }
  svg {
    width: 100%;
    max-height: 210px;
    background: var(--canvas);
    border-radius: 10px;
    padding: 15px;
  }
  .apply {
    background: var(--accent);
    justify-self: start;
  }
  strong {
    font-size: 22px;
  }
  .summary span {
    font-size: 12px;
  }
  input {
    max-width: 300px;
  }
</style>
